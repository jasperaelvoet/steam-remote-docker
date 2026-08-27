---
slug: /
---

# Introduction

**steam-remote-docker** turns a Linux machine with an AMD GPU into an
always-on Steam Remote Play host. You run one container; your TV, phone,
laptop, or Steam Deck connects to it with the Steam Link app.

Inside the container, Steam runs in a headless [Gamescope](https://github.com/ValveSoftware/gamescope)
session — no monitor, no desktop — with PipeWire audio and AMD hardware video
encoding. Steam itself handles discovery, pairing, streaming, audio, and
input, exactly like a normal gaming PC would.

It is deliberately simple and opinionated:

- **One session, one way to run it.** No alternate modes, recovery services,
  or per-game workarounds.
- **Immutable image, one data folder.** The container's filesystem is
  read-only. Everything that changes — your login, library, games, saves,
  settings — lives in a single folder you mount at `/mnt/data`.
- **Power-friendly.** After five minutes with no stream, game, or download,
  the session drops to 1 FPS until something happens again. Nothing is ever
  stopped or killed. See [Idle lifecycle](./idle-lifecycle.md).

## What you need

- A **Linux** host with an **AMD GPU** (the image ships Mesa RADV for Vulkan
  and VA-API for hardware encoding — no NVIDIA or Intel support)
- **Podman** or **Docker**
- `/dev/uinput` and `/dev/uhid` on the host (present on any normal kernel;
  used for controller input)
- UDP `27031-27036` and TCP `27036` open in the host firewall
  ([details](./networking.md))

## Quick start

The fastest path, with plain Podman:

```bash
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

Then open Steam Link on your client device and pair. Full walkthroughs,
including what every flag does and why:

- [Docker](./setup/docker.md)
- [Docker Compose](./setup/docker-compose.md)
- [Podman Quadlet](./setup/podman-quadlet.md) — run it as a systemd service
- [First login & verify](./setup/first-login.md) — the steps after any of the
  above

## Where to go next

| I want to… | Read |
| --- | --- |
| Change resolution or FPS | [Configuration](./configuration.md) |
| Understand the 1 FPS idle mode | [Idle lifecycle](./idle-lifecycle.md) |
| Check health, read logs, update the image | [Operations](./operations.md) |
| Back up or move my library | [Data & backups](./data-and-backups.md) |
| Fix something | [Troubleshooting](./troubleshooting.md) |
| Know how it works inside | [Internals](./internals/architecture.md) |
