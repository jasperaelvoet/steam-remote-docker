use anyhow::{Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::SessionMode;
use crate::environment;
use crate::paths::{session_file, DEFAULT_WAYLAND_SOCKET, REMOTE_PLAY_PORT};
use crate::process::{command_available, process_running, run_as_user, steam_running};
use crate::session::SessionMetadata;

#[derive(Debug, Serialize)]
pub struct HealthReport {
    pub healthy: bool,
    pub mode: Option<SessionMode>,
    pub session: bool,
    pub kwin: bool,
    pub kwin_socket: bool,
    pub pipewire: bool,
    pub pipewire_pulse: bool,
    pub gamescope: Option<bool>,
    pub gamescope_capture: Option<bool>,
    pub steam: Option<bool>,
    pub remote_play_tcp: Option<bool>,
    pub gpu_render_node: bool,
    pub vaapi_encode: bool,
}

impl HealthReport {
    pub fn gather(runtime_dir: &Path) -> Self {
        let metadata = read_metadata(runtime_dir).ok();
        let session = metadata
            .as_ref()
            .is_some_and(|metadata| pid_running(metadata.pid));
        let wayland_socket = metadata
            .as_ref()
            .map(|value| value.wayland_socket.as_str())
            .unwrap_or(DEFAULT_WAYLAND_SOCKET);
        let kwin = process_running("kwin_wayland");
        let kwin_socket = runtime_dir.join(wayland_socket).exists();
        let audio_required = metadata
            .as_ref()
            .map(|value| value.audio_enabled)
            .unwrap_or(true);
        let pipewire = process_running("pipewire") && runtime_dir.join("pipewire-0").exists();
        let pipewire_pulse =
            process_running("pipewire-pulse") && runtime_dir.join("pulse/native").exists();
        let mode = metadata.as_ref().map(|value| value.mode);
        let gamescope = (mode == Some(SessionMode::Gamescope))
            .then(|| process_running("gamescope") || process_running("gamescope-wl"));
        let gamescope_capture =
            (mode == Some(SessionMode::Gamescope)).then(|| capture_node_present(runtime_dir));
        let steam_required = metadata
            .as_ref()
            .map(|value| value.steam_enabled)
            .unwrap_or(true);
        let steam = steam_required.then(steam_running);
        let remote_play_tcp = steam_required.then(remote_play_ready);
        let render_node = render_node();
        let gpu_render_node = render_node.is_some();
        let vaapi_encode = render_node
            .as_deref()
            .is_some_and(|node| vaapi_encode_ready(runtime_dir, node));

        let healthy = session
            && kwin
            && kwin_socket
            && (!audio_required || (pipewire && pipewire_pulse))
            && gamescope.unwrap_or(true)
            && gamescope_capture.unwrap_or(true)
            && steam.unwrap_or(true)
            && remote_play_tcp.unwrap_or(true)
            && gpu_render_node
            && vaapi_encode;
        Self {
            healthy,
            mode,
            session,
            kwin,
            kwin_socket,
            pipewire,
            pipewire_pulse,
            gamescope,
            gamescope_capture,
            steam,
            remote_play_tcp,
            gpu_render_node,
            vaapi_encode,
        }
    }

    pub fn print(&self, json: bool) -> Result<()> {
        if json {
            println!("{}", serde_json::to_string_pretty(self)?);
            return Ok(());
        }
        println!("overall: {}", word(self.healthy));
        println!("session: {}", word(self.session));
        println!("kwin: {}", word(self.kwin && self.kwin_socket));
        println!("pipewire: {}", word(self.pipewire && self.pipewire_pulse));
        if let Some(value) = self.gamescope {
            println!("gamescope: {}", word(value));
        }
        if let Some(value) = self.gamescope_capture {
            println!("gamescope capture: {}", word(value));
        }
        if let Some(value) = self.steam {
            println!("steam: {}", word(value));
        }
        if let Some(value) = self.remote_play_tcp {
            println!("remote play tcp/{REMOTE_PLAY_PORT}: {}", word(value));
        }
        println!("gpu render node: {}", word(self.gpu_render_node));
        println!("vaapi encode: {}", word(self.vaapi_encode));
        Ok(())
    }
}

pub fn read_metadata(runtime_dir: &Path) -> Result<SessionMetadata> {
    let path = session_file(runtime_dir);
    let data = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_slice(&data).with_context(|| format!("parsing {}", path.display()))
}

fn pid_running(pid: u32) -> bool {
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), None).is_ok()
}

fn capture_node_present(runtime_dir: &Path) -> bool {
    if !command_available("pw-dump") || !runtime_dir.join("pipewire-0").exists() {
        return false;
    }
    let env = environment::base(runtime_dir, None);
    run_as_user(&["pw-dump"], &env)
        .map(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .to_ascii_lowercase()
                    .contains("gamescope")
        })
        .unwrap_or(false)
}

fn remote_play_ready() -> bool {
    let address = format!("127.0.0.1:{REMOTE_PLAY_PORT}");
    let Ok(address) = address.parse() else {
        return false;
    };
    std::net::TcpStream::connect_timeout(&address, Duration::from_millis(350)).is_ok()
}

fn render_node() -> Option<PathBuf> {
    std::fs::read_dir("/dev/dri")
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("renderD"))
        })
}

fn vaapi_encode_ready(runtime_dir: &Path, render_node: &Path) -> bool {
    if !command_available("vainfo") {
        return false;
    }
    let env = environment::base(runtime_dir, None);
    let device = render_node.to_string_lossy();
    run_as_user(&["vainfo", "--display", "drm", "--device", &device], &env)
        .map(|output| {
            let text = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
            .to_ascii_lowercase();
            output.status.success() && text.contains("vaentrypointencslice")
        })
        .unwrap_or(false)
}

fn word(value: bool) -> &'static str {
    if value {
        "ready"
    } else {
        "not ready"
    }
}
