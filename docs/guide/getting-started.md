# Getting started

This image runs one headless [Gamescope](https://github.com/ValveSoftware/gamescope)
session with a `3840x2160@60` render ceiling, PipeWire audio, and AMD hardware
acceleration. You point a Steam Link client at it; Steam owns discovery,
pairing, streaming, audio, and input.

It is deliberately opinionated: there are no alternate session modes, recovery
services, or per-game workarounds.

## Requirements

The host needs:

- **Linux** with **Podman** (Docker also works for building; see
  [Development](/internals/development))
- An **AMD GPU** — the image ships Mesa RADV for Vulkan and VA-API for hardware
  video encoding
- `/dev/uinput` and `/dev/uhid` available on the host, for controller and input
  emulation
- **Host networking**, with UDP `27031-27036` and TCP `27036` allowed through
  the host firewall (see [Networking & ports](/reference/networking))

## Run the container

Create a data directory and start the container:

```sh
mkdir -p "$PWD/steam-data"

podman run -d \
  --name steam-remote \
  --privileged \
  --network host \
  --ipc host \
  --read-only \
  --tmpfs /run:rw,exec,nosuid,size=1g,mode=755 \
  --tmpfs /tmp:rw,exec,nosuid,size=8g,mode=1777 \
  --tmpfs /var/tmp:rw,exec,nosuid,size=2g,mode=1777 \
  --tmpfs /var/lib/xkb:rw,exec,nosuid,size=64m,mode=1777 \
  --volume "$PWD/steam-data:/mnt/data:rw" \
  ghcr.io/jasperaelvoet/steam-remote-docker:latest
```

Every flag is load-bearing:

| Flag | Why |
| --- | --- |
| `--privileged` | Direct access to the GPU (`/dev/dri`), input devices, `/dev/uinput`, and `/dev/uhid` |
| `--network host` | Steam Remote Play discovery and streaming require the host's network identity |
| `--ipc host` | Shared memory between Steam, Gamescope, and the GPU stack |
| `--read-only` | The root filesystem is immutable by design; all mutable state lives in tmpfs mounts and `/mnt/data` |
| `--tmpfs …` | Writable scratch space for the session, X11 sockets, and Steam's temporary files |
| `--volume …:/mnt/data` | The single persistent volume — the `steam` user's home |

## First login

Watch the logs until the session is ready:

```sh
podman logs -f steam-remote
```

The line `steam-remote: ready at 3840x2160@60` means Gamescope and Steam are
up. Then:

1. Open the Steam Link app on your client device.
2. Let it discover the host (same network, or add it by IP).
3. Pair with the PIN Steam Link shows — the host-side confirmation appears in
   the streamed session.
4. Log in to Steam through the streamed gamepad UI.

Your login, library, games, saves, and settings all persist in `steam-data`
across container restarts and image updates. See
[Persistent data & backups](/guide/persistent-data).

## Verify

```sh
podman exec steam-remote steam-remote status
podman exec steam-remote steam-remote health --json
```

`health` exits nonzero unless Gamescope, PipeWire, Steam, the Remote Play
listener, and the lifecycle controller are all ready. It also backs the image's
built-in `HEALTHCHECK`, so `podman ps` shows the container's health directly.
See the [CLI reference](/reference/cli).

## Next steps

- Tune the render ceiling for your client: [Configuration](/guide/configuration)
- Understand when the session idles down: [Idle lifecycle](/guide/idle-lifecycle)
- Day-two commands and monitoring: [Operations](/guide/operations)
