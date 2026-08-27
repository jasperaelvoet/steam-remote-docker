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
  --tmpfs /run:rw,exec,nosuid,size=1g,mode=755 \
  --tmpfs /tmp:rw,exec,nosuid,size=8g,mode=1777 \
  --tmpfs /var/tmp:rw,exec,nosuid,size=2g,mode=1777 \
  --tmpfs /var/lib/xkb:rw,exec,nosuid,size=64m,mode=1777 \
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
| `--tmpfs …` | Writable scratch space; see below |
| `--volume …:/mnt/data` | The single persistent folder — the Steam user's home |

### Why the tmpfs mounts?

They exist *because of* `--read-only`: with an immutable root filesystem,
every path the session writes to must be mounted writable. Each one has a
job:

| Mount | What writes there |
| --- | --- |
| `/run` | Runtime sockets and state: DBus, PipeWire, the session runtime directory, and the lifecycle state file. `exec` because a small generated Xwayland wrapper lives (and runs) here |
| `/tmp` | Steam's and games' scratch space, plus the X11 sockets. The big one — hence 8g |
| `/var/tmp` | Slower-churn temp files some games and tools use |
| `/var/lib/xkb` | The X server's compiled keyboard-map cache |

The `size=` values are **ceilings, not allocations** — tmpfs uses RAM only
for what's actually stored. If you drop `--read-only`, you can drop all four
`--tmpfs` flags too; you lose the immutability guarantee but nothing else.
