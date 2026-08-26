use std::collections::BTreeMap;
use std::path::Path;

use crate::paths::{DESKTOP_USER, HOME};

pub fn base(runtime_dir: &Path, dbus_address: Option<&str>) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    env.insert(
        "PATH".into(),
        "/usr/local/sbin:/usr/local/bin:/usr/bin".into(),
    );
    env.insert("HOME".into(), HOME.into());
    env.insert("USER".into(), DESKTOP_USER.into());
    env.insert("LOGNAME".into(), DESKTOP_USER.into());
    env.insert("SHELL".into(), "/bin/bash".into());
    env.insert("LANG".into(), "en_US.UTF-8".into());
    env.insert("LC_CTYPE".into(), "en_US.UTF-8".into());
    env.insert("LC_MESSAGES".into(), "en_US.UTF-8".into());
    env.insert("LANGUAGE".into(), "en_US".into());
    env.insert("XDG_RUNTIME_DIR".into(), runtime_dir.display().to_string());
    env.insert("XDG_CONFIG_HOME".into(), format!("{HOME}/.config"));
    env.insert("XDG_DATA_HOME".into(), format!("{HOME}/.local/share"));
    env.insert("XDG_CACHE_HOME".into(), format!("{HOME}/.cache"));
    env.insert(
        "PULSE_SERVER".into(),
        format!("unix:{}/pulse/native", runtime_dir.display()),
    );
    env.insert("STEAM_RUNTIME".into(), "1".into());
    env.insert("SRT_URLOPEN_PREFER_STEAM".into(), "1".into());
    env.insert("GIO_USE_NETWORK_MONITOR".into(), "base".into());
    env.insert(
        "RES_OPTIONS".into(),
        "timeout:1 attempts:2 rotate single-request-reopen".into(),
    );
    if let Some(address) = dbus_address {
        env.insert("DBUS_SESSION_BUS_ADDRESS".into(), address.into());
    }
    for key in [
        "AMD_VULKAN_ICD",
        "DRI_PRIME",
        "LIBVA_DRIVER_NAME",
        "MESA_VK_DEVICE_SELECT",
        "RADV_PERFTEST",
        "STEAM_REMOTE_STEAMRT3",
        "STEAM_REMOTE_STEAM_CHANNEL",
        "VK_DRIVER_FILES",
    ] {
        if let Ok(value) = std::env::var(key) {
            if !value.trim().is_empty() {
                env.insert(key.into(), value);
            }
        }
    }
    if let Ok(value) = std::env::var("STEAM_REMOTE_PIPEWIRE_DEBUG") {
        if !value.trim().is_empty() {
            env.insert("PIPEWIRE_DEBUG".into(), value);
        }
    }
    if let Ok(value) = std::env::var("STEAM_REMOTE_WIREPLUMBER_LOG_LEVEL") {
        if !value.trim().is_empty() {
            env.insert("WIREPLUMBER_LOG_LEVEL".into(), value);
        }
    }
    env
}

pub fn kwin(mut env: BTreeMap<String, String>) -> BTreeMap<String, String> {
    env.insert("XDG_CURRENT_DESKTOP".into(), "KDE".into());
    env.insert("XDG_SESSION_DESKTOP".into(), "KDE".into());
    env.insert("XDG_SESSION_TYPE".into(), "wayland".into());
    env.insert("QT_QPA_PLATFORM".into(), "wayland".into());
    env.insert("SDL_VIDEODRIVER".into(), "wayland,x11".into());
    env.insert("KWIN_COMPOSE".into(), "O2ES".into());
    env
}

pub fn gamescope(
    mut env: BTreeMap<String, String>,
    outer_wayland_socket: &str,
    stats_fifo: &Path,
) -> BTreeMap<String, String> {
    env.insert("WAYLAND_DISPLAY".into(), outer_wayland_socket.into());
    env.insert("XDG_CURRENT_DESKTOP".into(), "gamescope".into());
    env.insert("XDG_SESSION_DESKTOP".into(), "gamescope".into());
    env.insert("XDG_SESSION_TYPE".into(), "wayland".into());
    env.insert("QT_QPA_PLATFORM".into(), "wayland".into());
    env.insert("SDL_VIDEODRIVER".into(), "wayland,x11".into());
    env.insert("GAMESCOPE_STATS".into(), stats_fifo.display().to_string());
    env.insert("ENABLE_GAMESCOPE_WSI".into(), "1".into());
    env
}

pub fn steam_x11(
    mut env: BTreeMap<String, String>,
    display: &str,
    gamescope_wayland: Option<&str>,
    stats_fifo: Option<&Path>,
) -> BTreeMap<String, String> {
    env.remove("WAYLAND_DISPLAY");
    env.insert("DISPLAY".into(), display.into());
    env.insert("XDG_SESSION_TYPE".into(), "x11".into());
    env.insert("QT_QPA_PLATFORM".into(), "xcb".into());
    env.insert("GDK_BACKEND".into(), "x11".into());
    env.insert("SDL_VIDEODRIVER".into(), "x11".into());
    env.insert("MOZ_ENABLE_WAYLAND".into(), "0".into());
    env.insert("QT_IM_MODULE".into(), "steam".into());
    env.insert("GTK_IM_MODULE".into(), "Steam".into());
    if let Some(wayland) = gamescope_wayland {
        env.insert("GAMESCOPE_WAYLAND_DISPLAY".into(), wayland.into());
        env.insert("STEAM_GAMESCOPE_FANCY_SCALING_SUPPORT".into(), "1".into());
        env.insert("STEAM_DISABLE_MANGOAPP_ATOM_WORKAROUND".into(), "1".into());
    }
    if let Some(path) = stats_fifo {
        env.insert("GAMESCOPE_STATS".into(), path.display().to_string());
    }
    env
}
