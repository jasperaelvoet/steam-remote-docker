use anyhow::{anyhow, Context, Result};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Child;

use crate::config::RunArgs;
use crate::config::SessionMode;
use crate::environment;
use crate::paths::log_dir;
use crate::process::{chown_user, spawn_as_user, terminate};

pub struct Kwin {
    pub child: Option<Child>,
}

impl Kwin {
    pub async fn start(args: &RunArgs, env: &BTreeMap<String, String>) -> Result<Self> {
        let socket_path = args.runtime_dir.join(&args.wayland_socket);
        let scale = args.scale.parse::<f64>().unwrap_or(1.0);
        let logical_width = ((args.width as f64) / scale).round().max(1.0) as u32;
        let logical_height = ((args.height as f64) / scale).round().max(1.0) as u32;
        let width = logical_width.to_string();
        let height = logical_height.to_string();
        let command = [
            "kwin_wayland",
            "--virtual",
            "--width",
            &width,
            "--height",
            &height,
            "--scale",
            &args.scale,
            "--socket",
            &args.wayland_socket,
            "--no-lockscreen",
            "--xwayland",
        ];
        let mut child = Some(spawn_as_user(&command, env, "kwin-wayland.log")?);
        if !wait_for_path(
            &socket_path,
            child.as_mut(),
            Duration::from_secs(args.ready_timeout),
        )
        .await
        {
            terminate(&mut child).await;
            return Err(anyhow!(
                "KWin did not create {}; see {}",
                socket_path.display(),
                log_dir().join("kwin-wayland.log").display()
            ));
        }
        if args.session_mode == SessionMode::X11 {
            let display = std::env::var("STEAM_REMOTE_X11_DISPLAY").unwrap_or_else(|_| ":0".into());
            let number = display
                .strip_prefix(':')
                .and_then(|value| value.split('.').next())
                .filter(|value| {
                    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
                });
            let Some(number) = number else {
                terminate(&mut child).await;
                return Err(anyhow!("STEAM_REMOTE_X11_DISPLAY must look like :0"));
            };
            let x11_socket = Path::new("/tmp/.X11-unix").join(format!("X{number}"));
            if !wait_for_path(
                &x11_socket,
                child.as_mut(),
                Duration::from_secs(args.ready_timeout),
            )
            .await
            {
                terminate(&mut child).await;
                return Err(anyhow!(
                    "KWin did not start Xwayland on {display} (expected {})",
                    x11_socket.display()
                ));
            }
        }
        Ok(Self { child })
    }
}

pub struct Gamescope {
    pub child: Option<Child>,
    pub x_display: String,
    pub wayland_display: String,
    pub stats_fifo: PathBuf,
    directory: PathBuf,
    _stats_guard: File,
}

impl Gamescope {
    pub async fn start(args: &RunArgs, base_env: &BTreeMap<String, String>) -> Result<Self> {
        let directory = args
            .runtime_dir
            .join(format!("gamescope.{}", std::process::id()));
        if directory.exists() {
            fs::remove_dir_all(&directory).ok();
        }
        fs::create_dir(&directory)?;
        chown_user(&directory);
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;

        let startup_fifo = directory.join("startup.socket");
        let stats_fifo = directory.join("stats.pipe");
        make_fifo(&startup_fifo)?;
        make_fifo(&stats_fifo)?;
        let mut startup_reader = open_fifo(&startup_fifo)?;
        let stats_guard = open_fifo(&stats_fifo)?;

        let env = environment::gamescope(base_env.clone(), &args.wayland_socket, &stats_fifo);
        let startup = startup_fifo.display().to_string();
        let stats = stats_fifo.display().to_string();
        let width = args.width.to_string();
        let height = args.height.to_string();
        let fps = args.fps.to_string();
        let command = [
            "gamescope",
            "-e",
            "-b",
            "--force-grab-cursor",
            "-R",
            &startup,
            "-T",
            &stats,
            "-W",
            &width,
            "-H",
            &height,
            "-w",
            &width,
            "-h",
            &height,
            "-r",
            &fps,
        ];
        let mut child = Some(spawn_as_user(&command, &env, "gamescope.log")?);
        let (x_display, wayland_display) = match read_startup_response(
            &mut startup_reader,
            child.as_mut(),
            Duration::from_secs(args.ready_timeout),
        )
        .await
        {
            Ok(value) => value,
            Err(error) => {
                terminate(&mut child).await;
                fs::remove_dir_all(&directory).ok();
                return Err(error.context(format!(
                    "Gamescope startup failed; see {}",
                    log_dir().join("gamescope.log").display()
                )));
            }
        };

        let session_link = args.runtime_dir.join("gamescope-stats");
        let _ = fs::remove_file(&session_link);
        #[cfg(target_family = "unix")]
        let _ = std::os::unix::fs::symlink(&directory, &session_link);
        chown_user(&session_link);

        Ok(Self {
            child,
            x_display,
            wayland_display,
            stats_fifo,
            directory,
            _stats_guard: stats_guard,
        })
    }

    pub fn steam_env(&self, base_env: &BTreeMap<String, String>) -> BTreeMap<String, String> {
        environment::steam_x11(
            base_env.clone(),
            &self.x_display,
            Some(&self.wayland_display),
            Some(&self.stats_fifo),
        )
    }

    pub fn cleanup(&self, runtime_dir: &Path) {
        let _ = fs::remove_file(runtime_dir.join("gamescope-stats"));
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn make_fifo(path: &Path) -> Result<()> {
    nix::unistd::mkfifo(path, nix::sys::stat::Mode::from_bits_truncate(0o600))
        .with_context(|| format!("creating FIFO {}", path.display()))?;
    chown_user(path);
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn open_fifo(path: &Path) -> Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .with_context(|| format!("opening FIFO {}", path.display()))
}

async fn read_startup_response(
    reader: &mut File,
    child: Option<&mut Child>,
    timeout: Duration,
) -> Result<(String, String)> {
    let deadline = std::time::Instant::now() + timeout;
    let mut child = child;
    let mut collected = String::new();
    while std::time::Instant::now() < deadline {
        let mut bytes = [0_u8; 256];
        match reader.read(&mut bytes) {
            Ok(0) => {}
            Ok(read) => collected.push_str(&String::from_utf8_lossy(&bytes[..read])),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error.into()),
        }
        let values: Vec<&str> = collected.split_whitespace().collect();
        if values.len() >= 2 {
            return Ok((values[0].into(), values[1].into()));
        }
        if child
            .as_deref_mut()
            .and_then(|child| child.try_wait().ok().flatten())
            .is_some()
        {
            return Err(anyhow!("Gamescope exited before reporting its displays"));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err(anyhow!(
        "timed out waiting for Gamescope to report its displays"
    ))
}

async fn wait_for_path(path: &Path, child: Option<&mut Child>, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    let mut child = child;
    while std::time::Instant::now() < deadline {
        if path.exists() {
            return true;
        }
        if child
            .as_deref_mut()
            .and_then(|child| child.try_wait().ok().flatten())
            .is_some()
        {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}
