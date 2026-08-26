use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::time::{Duration, Instant};
use tokio::process::Child;

use crate::audio::AudioStack;
use crate::compositor::{Gamescope, Kwin};
use crate::config::{RunArgs, SessionMode};
use crate::dbus::Buses;
use crate::environment;
use crate::paths::{session_file, DESKTOP_GID, DESKTOP_UID};
use crate::process::{self, spawn_as_user, terminate};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub pid: u32,
    pub mode: SessionMode,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub scale: String,
    pub runtime_dir: String,
    pub wayland_socket: String,
    pub x_display: String,
    pub gamescope_wayland_display: Option<String>,
    pub audio_enabled: bool,
    pub steam_enabled: bool,
}

pub struct RuntimeSession {
    args: RunArgs,
    buses: Buses,
    audio: Option<AudioStack>,
    kwin: Kwin,
    gamescope: Option<Gamescope>,
    steam_env: BTreeMap<String, String>,
    steam: Option<Child>,
    steam_started_at: Option<Instant>,
}

impl RuntimeSession {
    pub async fn start(args: RunArgs) -> Result<Self> {
        process::kill_leftovers();
        tokio::time::sleep(Duration::from_millis(500)).await;
        crate::filesystem::prepare(&args.runtime_dir, &args.wayland_socket)?;
        process::start_child_reaper();
        crate::filesystem::start_input_watcher();

        let buses = Buses::start(&args.runtime_dir).context("starting D-Bus")?;
        let base_env = environment::base(&args.runtime_dir, Some(&buses.session_address));

        let audio = if args.no_audio {
            None
        } else {
            match AudioStack::start(&args.runtime_dir, &base_env).await {
                Ok(stack) => Some(stack),
                Err(error) => {
                    buses.stop();
                    return Err(error.context("starting the common PipeWire stack"));
                }
            }
        };

        let kwin_env = environment::kwin(base_env.clone());
        let kwin = match Kwin::start(&args, &kwin_env).await {
            Ok(kwin) => kwin,
            Err(error) => {
                let mut audio = audio;
                if let Some(stack) = audio.as_mut() {
                    stack.stop().await;
                }
                buses.stop();
                return Err(error.context("starting KWin's virtual display"));
            }
        };

        let mut kwin = kwin;
        let gamescope = if args.session_mode == SessionMode::Gamescope {
            match Gamescope::start(&args, &base_env).await {
                Ok(gamescope) => Some(gamescope),
                Err(error) => {
                    terminate(&mut kwin.child).await;
                    let mut audio = audio;
                    if let Some(stack) = audio.as_mut() {
                        stack.stop().await;
                    }
                    buses.stop();
                    return Err(error.context("starting nested Gamescope"));
                }
            }
        } else {
            None
        };

        let fallback_display =
            std::env::var("STEAM_REMOTE_X11_DISPLAY").unwrap_or_else(|_| ":0".into());
        let steam_env = match gamescope.as_ref() {
            Some(gamescope) => gamescope.steam_env(&base_env),
            None => environment::steam_x11(base_env, &fallback_display, None, None),
        };
        let metadata = SessionMetadata {
            pid: std::process::id(),
            mode: args.session_mode,
            width: args.width,
            height: args.height,
            fps: args.fps,
            scale: args.scale.clone(),
            runtime_dir: args.runtime_dir.display().to_string(),
            wayland_socket: args.wayland_socket.clone(),
            x_display: steam_env
                .get("DISPLAY")
                .cloned()
                .unwrap_or(fallback_display),
            gamescope_wayland_display: steam_env.get("GAMESCOPE_WAYLAND_DISPLAY").cloned(),
            audio_enabled: !args.no_audio,
            steam_enabled: !args.no_steam,
        };
        let mut session = Self {
            args,
            buses,
            audio,
            kwin,
            gamescope,
            steam_env,
            steam: None,
            steam_started_at: None,
        };
        if let Err(error) = write_metadata(&session.args, &metadata) {
            session.shutdown().await;
            return Err(error.context("writing runtime session metadata"));
        }
        if !session.args.no_steam {
            if let Err(error) = session.spawn_steam() {
                session.shutdown().await;
                return Err(error);
            }
        }
        eprintln!(
            "steam-remote: session ready: {}x{}@{} mode={} display={}",
            metadata.width, metadata.height, metadata.fps, metadata.mode, metadata.x_display
        );
        Ok(session)
    }

    fn spawn_steam(&mut self) -> Result<()> {
        let args = self.args.steam_args();
        let mut command = vec!["/usr/local/bin/steam-remote-steam".to_string()];
        command.extend(args);
        let references: Vec<&str> = command.iter().map(String::as_str).collect();
        self.steam = Some(
            spawn_as_user(&references, &self.steam_env, "steam.log")
                .context("starting Steam Big Picture")?,
        );
        self.steam_started_at = Some(Instant::now());
        Ok(())
    }

    pub async fn run(mut self) -> Result<()> {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
        let smoke_deadline = (self.args.smoke_seconds > 0)
            .then(|| tokio::time::Instant::now() + Duration::from_secs(self.args.smoke_seconds));
        let mut restart_at: Option<tokio::time::Instant> = None;
        let mut restart_attempt: u32 = 0;
        let mut failure: Option<anyhow::Error> = None;

        loop {
            tokio::select! {
                _ = sigterm.recv() => break,
                _ = sigint.recv() => break,
                _ = optional_deadline(smoke_deadline), if smoke_deadline.is_some() => break,
                _ = optional_deadline(restart_at), if restart_at.is_some() => {
                    restart_at = None;
                    if let Err(error) = self.spawn_steam() {
                        failure = Some(error);
                        break;
                    }
                }
                status = wait_child(&mut self.steam), if self.steam.is_some() => {
                    let status = match status {
                        Ok(status) => status,
                        Err(error) => {
                            failure = Some(error.into());
                            break;
                        }
                    };
                    self.steam = None;
                    let stable = self.steam_started_at
                        .take()
                        .is_some_and(|started| started.elapsed() >= Duration::from_secs(300));
                    restart_attempt = if stable { 0 } else { restart_attempt.saturating_add(1) };
                    let delay = 2_u64.pow(restart_attempt.min(4)).min(30);
                    eprintln!("steam-remote: Steam exited ({status}); restarting in {delay}s");
                    restart_at = Some(tokio::time::Instant::now() + Duration::from_secs(delay));
                }
                status = wait_child(&mut self.kwin.child), if self.kwin.child.is_some() => {
                    failure = Some(match status {
                        Ok(status) => anyhow!("KWin exited unexpectedly: {status}"),
                        Err(error) => error.into(),
                    });
                    break;
                }
                _ = tokio::time::sleep(Duration::from_millis(500)) => {
                    if let Some(gamescope) = self.gamescope.as_mut() {
                        if let Some(status) = gamescope.child.as_mut().and_then(|child| child.try_wait().ok().flatten()) {
                            failure = Some(anyhow!("Gamescope exited unexpectedly: {status}"));
                            break;
                        }
                    }
                    if let Some(audio) = self.audio.as_mut() {
                        for (name, child) in [
                            ("PipeWire", &mut audio.pipewire),
                            ("WirePlumber", &mut audio.wireplumber),
                            ("PipeWire-Pulse", &mut audio.pulse),
                        ] {
                            if let Some(status) = child.as_mut().and_then(|child| child.try_wait().ok().flatten()) {
                                failure = Some(anyhow!("{name} exited unexpectedly: {status}"));
                                break;
                            }
                        }
                        if failure.is_some() {
                            break;
                        }
                    }
                }
            }
        }

        self.shutdown().await;
        if let Some(error) = failure {
            Err(error)
        } else {
            Ok(())
        }
    }

    async fn shutdown(&mut self) {
        eprintln!("steam-remote: stopping session");
        terminate(&mut self.steam).await;
        if let Some(gamescope) = self.gamescope.as_mut() {
            terminate(&mut gamescope.child).await;
            gamescope.cleanup(&self.args.runtime_dir);
        }
        terminate(&mut self.kwin.child).await;
        if let Some(audio) = self.audio.as_mut() {
            audio.stop().await;
        }
        self.buses.stop();
        let _ = fs::remove_file(session_file(&self.args.runtime_dir));
        process::kill_leftovers();
    }
}

fn write_metadata(args: &RunArgs, metadata: &SessionMetadata) -> Result<()> {
    let path = session_file(&args.runtime_dir);
    fs::write(
        &path,
        format!("{}\n", serde_json::to_string_pretty(metadata)?),
    )?;
    let owner = format!("{DESKTOP_UID}:{DESKTOP_GID}");
    let _ = process::run_quiet(&["chown", &owner, &path.to_string_lossy()]);
    Ok(())
}

async fn wait_child(child: &mut Option<Child>) -> std::io::Result<std::process::ExitStatus> {
    match child.as_mut() {
        Some(child) => child.wait().await,
        None => std::future::pending().await,
    }
}

async fn optional_deadline(deadline: Option<tokio::time::Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}
