# Steam Remote Docker

An always-on, headless Steam host for streaming games with Steam Link. The OCI
container runs an immutable Arch Linux userspace with a virtual KWin display,
Gamescope, PipeWire, and Steam Big Picture in one supervised session. Steam's
built-in Remote Play stack handles discovery, pairing, video, audio, and input;
Sunshine, Moonlight, and Wolf are not involved.

```text
Steam Link client
       |
       | Steam Remote Play
       v
Steam Big Picture -> Gamescope -> KWin virtual output
       |                 |
       +------ PipeWire -+
                       |
                  host AMD GPU
```

The project uses Podman and a system-level Quadlet. It is intended for a single
persistent Steam account and one active streaming session at a time.

## Requirements

- An x86-64 Linux host with systemd, Podman 5 or newer, and Quadlet support.
- A supported GPU exposed through `/dev/dri`. The image is optimized for AMD
  RADV and VA-API, including the matching 32-bit libraries required by Steam
  and Proton.
- `/dev/uinput`, `/dev/uhid`, and access to `/dev/input` for Steam Link input.
- Host networking. Steam Remote Play discovery does not work reliably through
  an ordinary container network.
- A persistent directory with enough space for the Steam library.

Steam Remote Play uses UDP 27031-27036 and TCP 27036. Permit those ports in the
host firewall for the networks from which clients may connect. Remote Play
Anywhere also requires normal outbound internet access.

## Build and install

The included Quadlet is deliberately privileged because GPU and virtual-input
access varies across kernels and nested container hosts. Only run this image on
a trusted machine.

Review `deploy/steam-remote.container` first. Its `/mnt/dev` and `/mnt/udev`
device mirrors match the target Proxmox LXC. On an ordinary Linux host, remove
those two volume lines and keep the explicit `AddDevice=` entries for
`/dev/dri`, `/dev/uinput`, and `/dev/uhid`.

The default persistent host path is `/srv/steam-remote`:

```sh
sudo install -d -m 0755 /srv/steam-remote
sudo make build
sudo make install-quadlet
sudo make start
sudo make logs
```

The generated service is `steam-remote.service`. Quadlet generators apply the
unit's `[Install]` section at boot; generated services are started directly and
must not be enabled with `systemctl enable`.

Open Steam Link on a client on the same LAN and select the host. Steam starts in
Big Picture and remains running between connections. The first login may
require Steam Guard; use the recovery console described below.

## Persistent data

`/mnt/user_data` is the container's only persistent application-data mount. The
Quadlet maps `/srv/steam-remote` there, and the runtime exposes the persistent
home at the stable in-session path `/home/retro`:

```text
/srv/steam-remote/
├── home/retro/       Steam account, library, games, saves, and settings
├── machine-id        Stable container identity
└── var/log/steam-remote/
```

Steam client updates, compatibility tools, games, saves, shader caches,
pairing state, and login state all remain in this directory when the image is
replaced. Back it up only while the service is stopped. This repository
intentionally provides no command that deletes it.

## Configuration

Set environment values in `deploy/steam-remote.container`, then run
`sudo make install-quadlet restart`:

| Variable | Image default | Bundled Quadlet | Purpose |
| --- | --- | --- | --- |
| `STEAM_REMOTE_WIDTH` | `1920` | `3840` | Virtual display width in pixels |
| `STEAM_REMOTE_HEIGHT` | `1080` | `2160` | Virtual display height in pixels |
| `STEAM_REMOTE_FPS` | `60` | `60` | Virtual display refresh rate |
| `STEAM_REMOTE_SCALE` | `auto` | `2` | KWin scale; `auto` selects 1, 1.5, or 2 from the resolution |
| `STEAM_REMOTE_SESSION_MODE` | `gamescope` | `gamescope` | Normal capture path; use `x11` for compatibility testing |
| `STEAM_STARTUP_ARGS` | `-bigpicture` | `-bigpicture` | Arguments passed to Steam |
| `STEAM_REMOTE_WAYLAND_SOCKET` | `steam-remote-wayland` | image default | Outer KWin Wayland socket name |
| `STEAM_REMOTE_READY_TIMEOUT` | `30` | image default | Startup readiness timeout in seconds |
| `STEAM_REMOTE_ADMIN_PORT` | `5900` | image default | Loopback-only recovery console port |
| `STEAM_REMOTE_X11_DISPLAY` | `:0` | image default | KWin Xwayland display used by `x11` mode |
| `STEAM_REMOTE_LOG_MAX_BYTES` | `16777216` | image default | Per-process log rotation threshold; minimum 1 MiB |
| `LIBVA_DRIVER_NAME` | `radeonsi` | `radeonsi` | VA-API driver used for AMD encoding |
| `AMD_VULKAN_ICD` | `RADV` | `RADV` | AMD Vulkan implementation |

For temporary audio diagnostics, `STEAM_REMOTE_PIPEWIRE_DEBUG` and
`STEAM_REMOTE_WIREPLUMBER_LOG_LEVEL` pass their non-empty values to the
respective session processes. Leave them unset in normal operation.

Package installation at runtime is blocked. Add packages to
`build/container/Containerfile` and rebuild so the operating system remains
reproducible. Never put Steam credentials, SSH passwords, or Steam Guard codes
in environment variables or images.

## Operations and diagnostics

```sh
sudo make service-status  # systemd and container state
sudo make status          # component report; always exits successfully
sudo make health          # same checks; fails while unhealthy
sudo make restart
sudo make stop
```

Both diagnostics accept `--json` when invoked directly:

```sh
sudo podman exec steam-remote steam-remote status --json
sudo podman exec steam-remote steam-remote health --json
```

The checks cover the KWin socket and process, PipeWire/PipeWire-Pulse,
Gamescope and its capture node when selected, Steam, the render device, and the
Remote Play TCP listener. The image also uses `steam-remote health` as its OCI
health check.

For lower-level verification:

```sh
sudo podman exec steam-remote vulkaninfo --summary
sudo podman exec steam-remote vainfo
sudo podman exec steam-remote pactl info
sudo podman exec steam-remote pw-cli list-objects Node
```

Steam's streaming log is kept in the persistent Steam home. After a real test
connection, confirm that Steam selected the Gamescope PipeWire source and an
AMD hardware encoder. If Gamescope capture cannot produce video on a specific
driver version, set `STEAM_REMOTE_SESSION_MODE=x11`, reinstall the Quadlet,
restart the service, and repeat the complete video, audio, and input test.

## Recovery console

The optional VNC console is off during normal operation and binds only to host
loopback because the container uses host networking:

```sh
sudo podman exec steam-remote steam-remote admin start
```

From a workstation, forward that loopback socket through SSH:

```sh
ssh -N -L 5900:127.0.0.1:5900 root@your-server
```

Connect a VNC viewer to `127.0.0.1:5900`, complete Steam login or recovery, and
stop the console afterward:

```sh
sudo podman exec steam-remote steam-remote admin stop
```

Use `--port` or `STEAM_REMOTE_ADMIN_PORT` to select a different loopback port.
Do not expose the recovery console through a public address or firewall rule.

## Updates

Build locally from the checked-out source:

```sh
sudo podman build --platform linux/amd64 \
  --build-arg VCS_REF="$(git rev-parse HEAD)" \
  --tag localhost/steam-remote-docker:latest \
  --file build/container/Containerfile .
```

For production, prefer an immutable GHCR tag or digest. Change `Image=` in the
Quadlet, then reload and restart:

```sh
sudo make install-quadlet restart
```

Stop Steam and back up `/srv/steam-remote` before a migration. Keep the previous
image until a real Steam Link test has verified video, audio, controller input,
several reconnects, and recovery after a service restart.

## Image publishing

Pushes to `main` use Buildah and Podman to publish `linux/amd64` OCI images to
GitHub Container Registry with both mutable and immutable tags:

```text
ghcr.io/jasperaelvoet/steam-remote-docker:latest
ghcr.io/jasperaelvoet/steam-remote-docker:sha-<full-commit>
```

Use the SHA tag for deployments. After the first workflow run, ensure the GHCR
package visibility is public if anonymous pulls are desired.

## Limitations

- This is a single-session appliance, not a Games-on-Whales replacement or a
  multi-tenant game launcher.
- Steam must already be logged in and running for Steam Link discovery.
- Steam Remote Play behavior can change with Steam client, Gamescope, Mesa, or
  Proton updates. Test before deleting a known-working image.
- The container needs broad GPU and input-device access. Treat it as trusted
  infrastructure and keep the host and image current.
- A physical monitor or dummy plug is not required; KWin owns a virtual output.

## License

[MIT](LICENSE)
