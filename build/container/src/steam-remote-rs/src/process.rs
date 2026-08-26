use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Once;
use std::time::Duration;
use tokio::process::Child;

use crate::paths::{log_dir, DESKTOP_GID, DESKTOP_UID};

static REAPER_STARTED: Once = Once::new();

pub fn start_child_reaper() {
    REAPER_STARTED.call_once(|| {
        let _ = std::thread::Builder::new()
            .name("steam-remote-reaper".into())
            .spawn(|| loop {
                reap_orphans();
                std::thread::sleep(Duration::from_secs(5));
            });
    });
}

fn reap_orphans() {
    loop {
        match nix::sys::wait::waitpid(
            nix::unistd::Pid::from_raw(-1),
            Some(nix::sys::wait::WaitPidFlag::WNOHANG),
        ) {
            Ok(nix::sys::wait::WaitStatus::StillAlive) => break,
            Ok(_) => continue,
            Err(nix::errno::Errno::ECHILD) | Err(_) => break,
        }
    }
}

pub fn command_available(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).any(|dir| dir.join(name).is_file()))
        .unwrap_or(false)
}

pub fn run(command: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new(command[0])
        .args(&command[1..])
        .stdin(Stdio::null())
        .output()
}

pub fn run_quiet(command: &[&str]) -> bool {
    Command::new(command[0])
        .args(&command[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn user_argv(command: &[&str], env: &BTreeMap<String, String>) -> Vec<String> {
    let mut argv = vec![
        "setpriv".into(),
        "--reuid".into(),
        DESKTOP_UID.to_string(),
        "--regid".into(),
        DESKTOP_GID.to_string(),
        "--init-groups".into(),
        "--".into(),
        "env".into(),
        "-i".into(),
    ];
    argv.extend(env.iter().map(|(key, value)| format!("{key}={value}")));
    argv.extend(command.iter().map(|value| value.to_string()));
    argv
}

pub fn run_as_user(
    command: &[&str],
    env: &BTreeMap<String, String>,
) -> std::io::Result<std::process::Output> {
    let argv = user_argv(command, env);
    Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .output()
}

fn max_log_bytes() -> u64 {
    std::env::var("STEAM_REMOTE_LOG_MAX_BYTES")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value >= 1024 * 1024)
        .unwrap_or(16 * 1024 * 1024)
}

fn rotate_log(path: &Path) {
    let Ok(metadata) = std::fs::metadata(path) else {
        return;
    };
    if metadata.len() < max_log_bytes() {
        return;
    }
    let rotated = path.with_extension(format!(
        "{}.1",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("log")
    ));
    let _ = std::fs::remove_file(&rotated);
    let _ = std::fs::rename(path, rotated);
}

pub fn open_log(log_name: &str) -> Result<std::fs::File> {
    let path = log_dir().join(log_name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    rotate_log(&path);
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("opening {}", path.display()))
}

fn start_new_session(command: &mut tokio::process::Command) {
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

pub fn spawn_as_user(
    command: &[&str],
    env: &BTreeMap<String, String>,
    log_name: &str,
) -> Result<Child> {
    let stdout = open_log(log_name)?;
    let stderr = stdout.try_clone()?;
    let argv = user_argv(command, env);
    let mut process = tokio::process::Command::new(&argv[0]);
    start_new_session(&mut process);
    process
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .with_context(|| format!("spawning {}", command.join(" ")))
}

pub fn process_running(name: &str) -> bool {
    run_quiet(&["pgrep", "-u", &DESKTOP_UID.to_string(), "-x", name])
}

pub fn steam_running() -> bool {
    process_running("steam") || process_running("steamwebhelper")
}

pub fn pid_is_x11vnc(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        let path = format!("/proc/{pid}/cmdline");
        std::fs::read(path)
            .map(|bytes| bytes.windows(6).any(|part| part == b"x11vnc"))
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        false
    }
}

fn signal_group(pid: u32, signal: nix::sys::signal::Signal) {
    let group = nix::unistd::Pid::from_raw(-(pid as i32));
    if nix::sys::signal::kill(group, signal).is_err() {
        let _ = nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), signal);
    }
}

pub async fn terminate(child: &mut Option<Child>) {
    let Some(mut child) = child.take() else {
        return;
    };
    if child.try_wait().ok().flatten().is_some() {
        return;
    }
    if let Some(pid) = child.id() {
        signal_group(pid, nix::sys::signal::Signal::SIGTERM);
    }
    if tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .is_err()
    {
        if let Some(pid) = child.id() {
            signal_group(pid, nix::sys::signal::Signal::SIGKILL);
        } else {
            let _ = child.kill().await;
        }
        let _ = child.wait().await;
    }
}

pub fn kill_leftovers() {
    for name in [
        "steam",
        "steamwebhelper",
        "gamescope",
        "x11vnc",
        "kwin_wayland",
        "Xwayland",
        "pipewire-pulse",
        "wireplumber",
        "pipewire",
    ] {
        let _ = run(&["pkill", "-TERM", "-u", &DESKTOP_UID.to_string(), "-x", name]);
    }
}

pub fn chown_user(path: &Path) {
    let owner = format!("{DESKTOP_UID}:{DESKTOP_GID}");
    let path = path.to_string_lossy();
    let _ = run_quiet(&["chown", &owner, &path]);
}
