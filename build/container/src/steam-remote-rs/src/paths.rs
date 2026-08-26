use std::path::{Path, PathBuf};

pub const DESKTOP_USER: &str = "retro";
pub const DESKTOP_UID: u32 = 1000;
pub const DESKTOP_GID: u32 = 1000;
pub const USER_DATA: &str = "/mnt/user_data";
pub const HOME: &str = "/home/retro";
pub const DEFAULT_RUNTIME_DIR: &str = "/run/user/1000";
pub const DEFAULT_WAYLAND_SOCKET: &str = "steam-remote-wayland";
pub const DEFAULT_ADMIN_PORT: u16 = 5900;
pub const REMOTE_PLAY_PORT: u16 = 27036;
pub const AUDIO_SINK_NAME: &str = "steam_remote_audio";
pub const AUDIO_SINK_DESCRIPTION: &str = "Steam_Remote_Audio";

pub fn persistent_home() -> PathBuf {
    Path::new(USER_DATA).join("home").join(DESKTOP_USER)
}

pub fn persistent_machine_id() -> PathBuf {
    Path::new(USER_DATA).join("machine-id")
}

pub fn log_dir() -> PathBuf {
    Path::new(USER_DATA)
        .join("var")
        .join("log")
        .join("steam-remote")
}

pub fn session_file(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join("steam-remote-session.json")
}

pub fn admin_pid_file(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join("steam-remote-admin.pid")
}
