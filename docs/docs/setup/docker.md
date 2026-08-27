# Docker

Run the host with plain `docker run`. Everything here also applies to Podman —
swap `docker` for `podman`.

:::note
This needs Docker Engine on a **Linux host**. Docker Desktop on macOS or
Windows runs containers in a VM and cannot reach the GPU or the LAN the way
Remote Play needs.
:::

## 1. Create the data folder

All persistent state — Steam login, library, games, saves — lives in one
folder on the host:

```bash
mkdir -p "$PWD/steam-data"
```

## 2. Start the container

```bash
docker run -d \
  --name steam-remote \
  --restart unless-stopped \
  --privileged \
  --network host \
  --ipc host \
  --read-only \
  --volume "$PWD/steam-data:/mnt/data:rw" \
  ghcr.io/jasperaelvoet/steam-remote-docker:latest
```

For a non-4K client, add environment variables before the image name, for
example `--env STEAM_REMOTE_WIDTH=1920 --env STEAM_REMOTE_HEIGHT=1080`.
See [Configuration](../configuration.md).

Then continue with [First login & verify](./first-login.md).

## Why each flag

| Flag | Why it's needed |
| --- | --- |
| `--privileged` | Direct access to the GPU (`/dev/dri`), input devices, `/dev/uinput`, and `/dev/uhid` |
| `--network host` | Steam Link discovery and streaming need the host's real network identity; NAT breaks discovery |
| `--ipc host` | Shared memory between Steam, Gamescope, and the GPU driver |
| `--read-only` | The image is immutable by design — nothing can modify the system, and updates are clean image swaps |
| `--volume …:/mnt/data` | The single persistent folder — the Steam user's home |

### Scratch space is automatic

With a read-only root, every path the session writes to must be a writable
mount — but you don't have to provide any of them. Because the container
already runs privileged, the entrypoint mounts its own tmpfs scratch space at
startup, with sensible size ceilings:

| Mount | What writes there |
| --- | --- |
| `/run` (1g) | Runtime sockets and state: DBus, PipeWire, the session runtime directory, and the lifecycle state file |
| `/tmp` (8g) | Steam's and games' scratch space, plus the X11 sockets |
| `/var/tmp` (2g) | Slower-churn temp files some games and tools use |
| `/var/lib/xkb` (64m) | The X server's compiled keyboard-map cache |

The sizes are **ceilings, not allocations** — tmpfs uses RAM only for what's
actually stored, and everything in them resets on restart. Any `--tmpfs`
flags you pass yourself for these paths are simply mounted over inside the
container.
