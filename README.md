# Steam Remote Play container

An opinionated, headless Steam host for Steam Link. The image runs one
headless Gamescope session with a `3840x2160@60` render ceiling, PipeWire audio,
and AMD hardware acceleration.

There are no alternate session modes, recovery services, or per-game
workarounds. Steam owns discovery, pairing, streaming, audio, and input.

Full documentation lives at
[jasperaelvoet.github.io/steam-remote-docker](https://jasperaelvoet.github.io/steam-remote-docker/).

## Run

The host needs Linux, Podman, an AMD GPU, and `/dev/uinput` plus `/dev/uhid`.
Steam Remote Play also needs host networking and UDP `27031-27036` plus TCP
`27036` allowed through the host firewall.

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

`steam-data` is mounted directly at `/mnt/data`, the `steam` user's home. It
contains the Steam library, client updates, login, games, saves, and settings.
Stop the container before backing it up or moving it.

The defaults are intended for a 4K Steam Link client. These values set the
maximum Gamescope render size and refresh rate; override only what the client
or network requires:

| Variable | Default |
| --- | ---: |
| `STEAM_REMOTE_WIDTH` | `3840` |
| `STEAM_REMOTE_HEIGHT` | `2160` |
| `STEAM_REMOTE_FPS` | `60` |

For example, add `--env STEAM_REMOTE_WIDTH=1920 --env
STEAM_REMOTE_HEIGHT=1080` for 1080p.

Steam negotiates the actual PipeWire capture size and encoding frame rate with
the connected client. Gamescope fits that capture within the configured render
ceiling while preserving its aspect ratio. Clients with a different aspect
ratio can therefore receive letterboxing or pillarboxing. The client frame rate
cannot exceed `STEAM_REMOTE_FPS`.

## Idle lifecycle

Gamescope stays at full rate while a Remote Play stream, a Steam game, or an
active download, update, validation, or patch operation is running. A
disconnected game continues running so the same session can reconnect. With no
activity, the image waits five minutes and then parks Gamescope at 1 FPS.

Parking never stops Gamescope, Steam, games, or downloads. A new connection or
other activity restores the configured rate automatically. Paused or queued
updates do not keep the session active. If activity detection or Gamescope
control is unavailable, the session stays at full rate and reports an unhealthy
lifecycle instead of interrupting Steam.

## Operate

```sh
podman logs -f steam-remote
podman exec steam-remote steam-remote status
podman exec steam-remote steam-remote health --json
podman stop steam-remote
```

`status` reports readiness without failing. Both text and JSON output include
the lifecycle state plus streaming, game, update, and controller indicators.
`health` exits nonzero unless Gamescope, PipeWire, Steam, the Remote Play
listener, and the lifecycle controller are all ready. The `parked` lifecycle
state is healthy.

## Build

```sh
bun install --frozen-lockfile
bun run check
bun run build
```

The development commands require Bun 1.4 or newer. `bun run build` uses
Podman by default; set `CONTAINER_ENGINE=docker` to use Docker.

The image is intentionally read-only. Add system packages to `Containerfile`;
Steam and game updates belong in `steam-data`.

## License

[MIT](LICENSE)
