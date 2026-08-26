use anyhow::{anyhow, Context, Result};
use clap::Subcommand;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use crate::environment;
use crate::health::read_metadata;
use crate::paths::{admin_pid_file, DEFAULT_ADMIN_PORT};
use crate::process::{command_available, pid_is_x11vnc, spawn_as_user};

#[derive(Debug, Clone, Subcommand)]
pub enum AdminAction {
    /// Start a loopback-only x11vnc recovery console.
    Start {
        #[arg(long, env = "STEAM_REMOTE_ADMIN_PORT", default_value_t = DEFAULT_ADMIN_PORT)]
        port: u16,
    },
    /// Stop the recovery console.
    Stop,
    /// Report whether the recovery console is running.
    Status,
}

pub async fn dispatch(runtime_dir: &Path, action: AdminAction) -> Result<()> {
    match action {
        AdminAction::Start { port } => start(runtime_dir, port).await,
        AdminAction::Stop => stop(runtime_dir),
        AdminAction::Status => {
            let running = active_pid(runtime_dir).is_some();
            println!(
                "admin console: {}",
                if running { "running" } else { "stopped" }
            );
            Ok(())
        }
    }
}

async fn start(runtime_dir: &Path, port: u16) -> Result<()> {
    if active_pid(runtime_dir).is_some() {
        println!("admin console is already running");
        return Ok(());
    }
    if !command_available("x11vnc") {
        return Err(anyhow!("x11vnc is not installed in this image"));
    }
    let metadata = read_metadata(runtime_dir)?;
    let mut env: BTreeMap<String, String> = environment::base(runtime_dir, None);
    env.insert("DISPLAY".into(), metadata.x_display.clone());
    env.insert("XDG_SESSION_TYPE".into(), "x11".into());
    let port = port.to_string();
    let command = [
        "x11vnc",
        "-display",
        &metadata.x_display,
        "-rfbport",
        &port,
        "-localhost",
        "-forever",
        "-shared",
        "-nopw",
        "-noxdamage",
        "-repeat",
    ];
    let mut child = spawn_as_user(&command, &env, "admin-x11vnc.log")?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    if let Some(status) = child.try_wait()? {
        return Err(anyhow!(
            "x11vnc exited immediately with {status}; inspect the admin-x11vnc log"
        ));
    }
    let pid = child
        .id()
        .ok_or_else(|| anyhow!("x11vnc did not report its pid"))?;
    let pid_file = admin_pid_file(runtime_dir);
    fs::write(&pid_file, format!("{pid}\n"))
        .with_context(|| format!("writing {}", pid_file.display()))?;
    drop(child);
    println!("admin console listening on 127.0.0.1:{port}");
    Ok(())
}

fn stop(runtime_dir: &Path) -> Result<()> {
    let path = admin_pid_file(runtime_dir);
    let Some(pid) = active_pid(runtime_dir) else {
        let _ = fs::remove_file(path);
        println!("admin console is already stopped");
        return Ok(());
    };
    let pid = nix::unistd::Pid::from_raw(pid as i32);
    let _ = nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGTERM);
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if nix::sys::signal::kill(pid, None).is_err() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if nix::sys::signal::kill(pid, None).is_ok() {
        let _ = nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGKILL);
    }
    let _ = fs::remove_file(path);
    println!("admin console stopped");
    Ok(())
}

fn active_pid(runtime_dir: &Path) -> Option<u32> {
    let path = admin_pid_file(runtime_dir);
    let pid = fs::read_to_string(path).ok()?.trim().parse().ok()?;
    pid_is_x11vnc(pid).then_some(pid)
}
