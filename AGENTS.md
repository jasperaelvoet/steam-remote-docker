# Agent Notes

This project is an always-on Arch Linux Steam Remote Play OCI container. It
streams directly to Steam Link and is deployed through a system-level Podman
Quadlet; it does not use Sunshine, Moonlight, or Wolf.

## Working Rules

- Do not commit unless the user explicitly asks for a commit.
- Do not store credentials, Steam Guard codes, SSH targets, passwords, or other
  private machine details in the repository.
- Preserve the single persistent application-data mount: all mutable Steam
  state lives under `/mnt/user_data`, with the persistent home at
  `/mnt/user_data/home/retro` and its in-session path at `/home/retro`.
- Never add a target that deletes persistent user data. Stop the service before
  moving, backing up, or restoring a Steam library.
- Keep the base image immutable. System packages belong in the Containerfile;
  Steam updates, games, compatibility tools, saves, and settings belong in the
  persistent home.
- Prefer global session fixes over per-game wrappers. Do not add game-specific
  workarounds to the default path.
- Preserve host networking and the one-session model.
- Keep the recovery VNC server disabled by default and bound to loopback only.
- Keep Podman/Buildah and the system Quadlet as the repository's single build
  and deployment surface.

## Runtime Shape

- The supervisor initializes the persistent home and machine identity, then
  starts D-Bus, PipeWire/PipeWire-Pulse, KWin on its virtual backend, nested
  Gamescope, and Steam Big Picture.
- Gamescope, Steam, and PipeWire share one user runtime directory so Steam can
  capture the Gamescope PipeWire node.
- `STEAM_REMOTE_SESSION_MODE=gamescope` is the normal path. `x11` is an explicit
  compatibility fallback, not an automatic silent downgrade.
- Steam stays alive between clients. The supervisor restarts Steam with bounded
  backoff and exits on critical compositor or audio failure so systemd can
  restart the appliance.
- AMD RADV and VA-API are the primary graphics/encode path. Preserve the
  corresponding 32-bit packages for Steam and Proton.
- Runtime diagnostics are `steam-remote status [--json]` and `steam-remote
  health [--json]`.
- The optional `steam-remote admin start|stop|status` console is for login and
  recovery through an SSH tunnel only.

## Development Commands

- `make build` builds `localhost/steam-remote-docker:latest` with Podman.
- `make install-quadlet` installs the system Quadlet and reloads systemd; invoke
  it as root or through `sudo make`.
- `make start`, `stop`, `restart`, `logs`, and `service-status` operate on the
  generated systemd service.
- `make status` and `make health` inspect the running container.
- `make check` runs non-mutating source and Quadlet checks available on the
  current host.

## Verification

Before handing off meaningful changes, run at minimum:

- `make check`
- `cargo check --locked --manifest-path
  build/container/src/steam-remote-rs/Cargo.toml`
- Shell syntax checks for every executable shell script under
  `build/container/bin` and `scripts`.
- A Quadlet generator dry run when `podman-system-generator` is available.
- `podman build --platform linux/amd64 --file
  build/container/Containerfile --tag localhost/steam-remote-docker:test .`
  when image contents change.

For runtime changes, validate on a Linux host:

- `steam-remote health` succeeds and the OCI health state becomes healthy.
- `vulkaninfo --summary` and `vainfo` see the intended GPU.
- PipeWire-Pulse is reachable and Gamescope publishes a capture node.
- Steam listens for Remote Play on TCP/UDP 27036.
- A real Steam Link session has hardware-encoded video, audio, controller input,
  three reconnect cycles, and recovery after a service restart.
- Steam's library and login remain intact after replacing the image.
