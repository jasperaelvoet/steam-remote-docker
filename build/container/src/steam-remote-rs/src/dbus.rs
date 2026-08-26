use anyhow::{anyhow, Result};
use std::fs;
use std::path::Path;
use std::time::Duration;

use crate::environment;
use crate::process::{run, run_as_user};

pub struct Buses {
    pub session_address: String,
    session_pid: Option<u32>,
    system_pid: Option<u32>,
}

impl Buses {
    pub fn start(runtime_dir: &Path) -> Result<Self> {
        let session = start_session(runtime_dir)?;
        let system_pid = start_system()?;
        Ok(Self {
            session_address: session.0,
            session_pid: session.1,
            system_pid,
        })
    }

    pub fn stop(&self) {
        for pid in [self.session_pid, self.system_pid].into_iter().flatten() {
            let _ = nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid as i32),
                nix::sys::signal::Signal::SIGTERM,
            );
        }
    }
}

fn start_session(runtime_dir: &Path) -> Result<(String, Option<u32>)> {
    let socket = runtime_dir.join("bus");
    let _ = fs::remove_file(&socket);
    let address = format!("--address=unix:path={}", socket.display());
    let env = environment::base(runtime_dir, None);
    let output = run_as_user(
        &[
            "dbus-daemon",
            "--session",
            "--fork",
            &address,
            "--print-address",
            "--print-pid",
        ],
        &env,
    )?;
    if !output.status.success() {
        return Err(anyhow!(
            "session D-Bus failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let values: Vec<&str> = std::str::from_utf8(&output.stdout)?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if values.len() < 2 {
        return Err(anyhow!("session D-Bus did not return its address and pid"));
    }
    wait_for_path(&socket, Duration::from_secs(5))?;
    Ok((values[0].into(), values[1].parse().ok()))
}

fn start_system() -> Result<Option<u32>> {
    fs::create_dir_all("/run/dbus").ok();
    if Path::new("/run/dbus/system_bus_socket").exists() {
        return Ok(None);
    }
    let output = run(&["dbus-daemon", "--system", "--fork", "--print-pid"])?;
    if !output.status.success() {
        return Err(anyhow!(
            "system D-Bus failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let mut lines = std::str::from_utf8(&output.stdout)?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let pid = lines.next_back().and_then(|value| value.parse().ok());
    Ok(pid)
}

fn wait_for_path(path: &Path, timeout: Duration) -> Result<()> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if path.exists() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(anyhow!("timed out waiting for {}", path.display()))
}
