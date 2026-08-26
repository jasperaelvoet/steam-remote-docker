use anyhow::{anyhow, Result};
use clap::{Args, ValueEnum};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::paths::{DEFAULT_RUNTIME_DIR, DEFAULT_WAYLAND_SOCKET};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum SessionMode {
    Gamescope,
    X11,
}

impl std::fmt::Display for SessionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gamescope => write!(f, "gamescope"),
            Self::X11 => write!(f, "x11"),
        }
    }
}

#[derive(Debug, Clone, Args)]
pub struct RunArgs {
    #[arg(long, env = "XDG_RUNTIME_DIR", default_value = DEFAULT_RUNTIME_DIR)]
    pub runtime_dir: PathBuf,

    #[arg(
        long = "wayland-socket",
        env = "STEAM_REMOTE_WAYLAND_SOCKET",
        default_value = DEFAULT_WAYLAND_SOCKET
    )]
    pub wayland_socket: String,

    #[arg(long, env = "STEAM_REMOTE_WIDTH", default_value_t = 1920)]
    pub width: u32,

    #[arg(long, env = "STEAM_REMOTE_HEIGHT", default_value_t = 1080)]
    pub height: u32,

    #[arg(long, env = "STEAM_REMOTE_FPS", default_value_t = 60)]
    pub fps: u32,

    #[arg(long, env = "STEAM_REMOTE_SCALE", default_value = "auto")]
    pub scale: String,

    #[arg(
        long,
        env = "STEAM_REMOTE_SESSION_MODE",
        value_enum,
        default_value_t = SessionMode::Gamescope
    )]
    pub session_mode: SessionMode,

    #[arg(long, env = "STEAM_REMOTE_READY_TIMEOUT", default_value_t = 30)]
    pub ready_timeout: u64,

    /// Run the stack for a bounded time; intended for pre-cutover image tests.
    #[arg(long, default_value_t = 0)]
    pub smoke_seconds: u64,

    /// Start the display/audio stack without touching the persistent Steam client.
    #[arg(long, default_value_t = false)]
    pub no_steam: bool,

    /// Skip PipeWire. Intended only for development diagnostics.
    #[arg(long, hide = true, default_value_t = false)]
    pub no_audio: bool,
}

impl RunArgs {
    pub fn validate(mut self) -> Result<Self> {
        if !(320..=8192).contains(&self.width) {
            return Err(anyhow!("width must be between 320 and 8192"));
        }
        if !(200..=8192).contains(&self.height) {
            return Err(anyhow!("height must be between 200 and 8192"));
        }
        if !(1..=240).contains(&self.fps) {
            return Err(anyhow!("fps must be between 1 and 240"));
        }
        if self.ready_timeout == 0 || self.ready_timeout > 300 {
            return Err(anyhow!("ready-timeout must be between 1 and 300 seconds"));
        }
        self.scale = resolve_scale(self.width, self.height, &self.scale)?;
        if self.wayland_socket.contains('/') || self.wayland_socket.trim().is_empty() {
            return Err(anyhow!("wayland-socket must be a non-empty socket name"));
        }
        Ok(self)
    }

    pub fn steam_args(&self) -> Vec<String> {
        std::env::var("STEAM_STARTUP_ARGS")
            .unwrap_or_else(|_| "-bigpicture".into())
            .split_ascii_whitespace()
            .map(str::to_owned)
            .collect()
    }
}

fn resolve_scale(width: u32, height: u32, requested: &str) -> Result<String> {
    if requested.trim().eq_ignore_ascii_case("auto") {
        return Ok(if width >= 3840 || height >= 2160 {
            "2"
        } else if width >= 2560 || height >= 1440 {
            "1.5"
        } else {
            "1"
        }
        .into());
    }
    let value: f64 = requested
        .trim()
        .parse()
        .map_err(|_| anyhow!("scale must be auto or a number between 0.5 and 4"))?;
    if !(0.5..=4.0).contains(&value) {
        return Err(anyhow!("scale must be between 0.5 and 4"));
    }
    Ok(format_number(value))
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_scale_tracks_output_size() {
        assert_eq!(resolve_scale(1920, 1080, "auto").unwrap(), "1");
        assert_eq!(resolve_scale(2560, 1440, "auto").unwrap(), "1.5");
        assert_eq!(resolve_scale(3840, 2160, "auto").unwrap(), "2");
    }

    #[test]
    fn rejects_unsafe_scale_values() {
        assert!(resolve_scale(1920, 1080, "0").is_err());
        assert!(resolve_scale(1920, 1080, "5").is_err());
        assert!(resolve_scale(1920, 1080, "large").is_err());
    }
}
