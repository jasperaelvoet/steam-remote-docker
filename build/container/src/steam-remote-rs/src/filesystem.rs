use anyhow::{anyhow, Context, Result};
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::paths::{
    log_dir, persistent_home, persistent_machine_id, DESKTOP_GID, DESKTOP_UID, DESKTOP_USER, HOME,
    USER_DATA,
};
use crate::process::{chown_user, run, run_quiet};

pub fn prepare(runtime_dir: &Path, wayland_socket: &str) -> Result<()> {
    if !nix::unistd::Uid::effective().is_root() {
        return Err(anyhow!("steam-remote run must start as root"));
    }
    validate_desktop_user()?;
    prepare_machine_id()?;
    prepare_home()?;
    prepare_runtime(runtime_dir, wayland_socket)?;
    prepare_devices();
    Ok(())
}

fn validate_desktop_user() -> Result<()> {
    let user = nix::unistd::User::from_name(DESKTOP_USER)?
        .ok_or_else(|| anyhow!("runtime user {DESKTOP_USER:?} does not exist"))?;
    if user.uid.as_raw() != DESKTOP_UID || user.gid.as_raw() != DESKTOP_GID {
        return Err(anyhow!(
            "runtime user {DESKTOP_USER:?} must have uid:gid {DESKTOP_UID}:{DESKTOP_GID}"
        ));
    }
    Ok(())
}

fn path_is_mountpoint(path: &Path) -> bool {
    run_quiet(&["mountpoint", "-q", &path.to_string_lossy()])
}

fn bind_mount(source: &Path, target: &Path) -> Result<()> {
    if path_is_mountpoint(target) {
        return Ok(());
    }
    let output = run(&[
        "mount",
        "--bind",
        &source.to_string_lossy(),
        &target.to_string_lossy(),
    ])?;
    if output.status.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "failed to bind {} to {}: {}",
            source.display(),
            target.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn valid_machine_id(value: &str) -> bool {
    let value = value.trim();
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn new_machine_id() -> Result<String> {
    let uuid = fs::read_to_string("/proc/sys/kernel/random/uuid")
        .context("reading a kernel-generated machine identity")?;
    let value = uuid.trim().replace('-', "");
    if !valid_machine_id(&value) {
        return Err(anyhow!("kernel returned an invalid machine identity"));
    }
    Ok(value)
}

fn prepare_machine_id() -> Result<()> {
    fs::create_dir_all(USER_DATA)?;
    let persistent = persistent_machine_id();
    if !persistent.exists() {
        // Each fresh data volume gets its own identity. A migration can seed
        // this path with the old container identity before first boot.
        let value = new_machine_id()?;
        fs::write(&persistent, format!("{value}\n"))?;
    }
    let persisted_value = fs::read_to_string(&persistent)?;
    if !valid_machine_id(&persisted_value) {
        return Err(anyhow!(
            "{} does not contain a valid 32-character machine id",
            persistent.display()
        ));
    }
    fs::set_permissions(&persistent, fs::Permissions::from_mode(0o444)).ok();

    let target = Path::new("/etc/machine-id");
    if !target.exists() {
        return Err(anyhow!(
            "/etc/machine-id is absent; the image must create it before enabling a read-only root"
        ));
    }
    bind_mount(&persistent, target)
}

fn create_user_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    chown_user(path);
    Ok(())
}

fn prepare_home() -> Result<()> {
    let persistent = persistent_home();
    create_user_dir(Path::new(USER_DATA))?;
    create_user_dir(&Path::new(USER_DATA).join("home"))?;
    create_user_dir(&persistent)?;
    fs::create_dir_all(HOME)?;
    bind_mount(&persistent, Path::new(HOME))?;

    for relative in [
        ".cache",
        ".config",
        ".local",
        ".local/share",
        "Desktop",
        "Documents",
        "Downloads",
        "Games",
        "Pictures",
        "Videos",
    ] {
        create_user_dir(&Path::new(HOME).join(relative))?;
    }
    create_user_dir(&Path::new(USER_DATA).join("var"))?;
    create_user_dir(&Path::new(USER_DATA).join("var/log"))?;
    create_user_dir(&log_dir())?;
    Ok(())
}

fn prepare_runtime(runtime_dir: &Path, wayland_socket: &str) -> Result<()> {
    fs::create_dir_all(runtime_dir)?;
    chown_user(runtime_dir);
    fs::set_permissions(runtime_dir, fs::Permissions::from_mode(0o700))?;

    let x11_dir = Path::new("/tmp/.X11-unix");
    fs::create_dir_all(x11_dir)?;
    fs::set_permissions(x11_dir, fs::Permissions::from_mode(0o1777))?;
    if !run_quiet(&["pgrep", "-u", &DESKTOP_UID.to_string(), "-x", "Xwayland"]) {
        if let Ok(entries) = fs::read_dir(x11_dir) {
            for entry in entries.flatten() {
                if entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with('X'))
                {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }

    for stale in [
        runtime_dir.join(wayland_socket),
        runtime_dir.join(format!("{wayland_socket}.lock")),
        runtime_dir.join("bus"),
        runtime_dir.join("steam-remote-session.json"),
        runtime_dir.join("steam-remote-admin.pid"),
    ] {
        let _ = fs::remove_file(stale);
    }
    Ok(())
}

fn prepare_devices() {
    start_udev();
    let mut devices: Vec<PathBuf> = vec!["/dev/uinput".into(), "/dev/uhid".into()];
    for directory in ["/dev/dri", "/dev/input"] {
        if let Ok(entries) = fs::read_dir(directory) {
            devices.extend(entries.flatten().map(|entry| entry.path()));
        }
    }
    for device in devices {
        let _ = fs::set_permissions(device, fs::Permissions::from_mode(0o666));
    }
}

fn start_udev() {
    let udevd = Path::new("/usr/lib/systemd/systemd-udevd");
    if udevd.exists() && !run_quiet(&["pgrep", "-x", "systemd-udevd"]) {
        let _ = run(&[udevd.to_string_lossy().as_ref(), "--daemon"]);
    }
    sync_input_device_nodes();
    let _ = run_quiet(&["udevadm", "control", "--reload-rules"]);
    let _ = run_quiet(&["udevadm", "trigger", "--action=add", "-s", "input"]);
}

fn event_node_name(name: &OsStr) -> bool {
    name.to_str().is_some_and(|name| name.starts_with("event"))
}

fn input_device_number(path: &Path) -> Option<libc::dev_t> {
    let value = fs::read_to_string(path.join("dev")).ok()?;
    let (major, minor) = value.trim().split_once(':')?;
    Some(libc::makedev(major.parse().ok()?, minor.parse().ok()?))
}

fn matching_device(path: &Path, device: libc::dev_t) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_char_device() && metadata.rdev() == device as u64)
        .unwrap_or(false)
}

pub fn sync_input_device_nodes() -> bool {
    let input_dir = Path::new("/dev/input");
    fs::create_dir_all(input_dir).ok();
    let mut live = HashSet::<OsString>::new();
    let mut changed = false;
    let Ok(entries) = fs::read_dir("/sys/class/input") else {
        return false;
    };
    let mut events: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.file_name().is_some_and(event_node_name))
        .collect();
    events.sort();

    for event in events {
        let Some(name) = event.file_name() else {
            continue;
        };
        let Some(device) = input_device_number(&event) else {
            continue;
        };
        live.insert(name.to_os_string());
        let node = input_dir.join(name);
        if node.exists() && !matching_device(&node, device) {
            changed |= fs::remove_file(&node).is_ok();
        }
        if !matching_device(&node, device) {
            use nix::sys::stat::{mknod, Mode, SFlag};
            changed |= mknod(
                &node,
                SFlag::S_IFCHR,
                Mode::from_bits_truncate(0o666),
                device,
            )
            .is_ok();
        }
        let _ = fs::set_permissions(node, fs::Permissions::from_mode(0o666));
    }

    if let Ok(entries) = fs::read_dir(input_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if event_node_name(&name) && !live.contains(&name) {
                changed |= fs::remove_file(entry.path()).is_ok();
            }
        }
    }
    changed
}

pub fn start_input_watcher() {
    let _ = std::thread::Builder::new()
        .name("steam-remote-input-watcher".into())
        .spawn(|| loop {
            if sync_input_device_nodes() {
                let _ = run_quiet(&["udevadm", "trigger", "--action=add", "-s", "input"]);
            }
            std::thread::sleep(std::time::Duration::from_secs(3));
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_machine_id_shape() {
        assert!(valid_machine_id("a8cc5b61d5ee4650ae555193b31fc370\n"));
        assert!(!valid_machine_id("a8cc5b61-d5ee-4650-ae55-5193b31fc370"));
        assert!(!valid_machine_id("not-a-machine-id"));
    }
}
