# Configuration

Configuration is deliberately small: three environment variables that set the
maximum render size and refresh rate. The defaults suit a 4K@60 client —
override only what your client or network needs.

| Variable | Default | Meaning |
| --- | ---: | --- |
| `STEAM_REMOTE_WIDTH` | `3840` | Maximum render width in pixels |
| `STEAM_REMOTE_HEIGHT` | `2160` | Maximum render height in pixels |
| `STEAM_REMOTE_FPS` | `60` | Maximum refresh rate |

A fourth switch, `STEAM_REMOTE_AUTO_UPDATE`, controls
[idle-time Steam client updates](./auto-updates.md)
(default on; set `0` to disable).

For a 1080p client:

```bash
podman run -d \
  --name steam-remote \
  --env STEAM_REMOTE_WIDTH=1920 \
  --env STEAM_REMOTE_HEIGHT=1080 \
  ... \
  ghcr.io/jasperaelvoet/steam-remote-docker:latest
```

(With Compose, set them under `environment:`; with Quadlet, use
`Environment=` lines.)

## How the ceiling behaves

These values are a **ceiling**, not a fixed output size. Steam negotiates the
actual capture size and frame rate with the connected client:

- Gamescope fits the negotiated capture within the configured render size
  while preserving its aspect ratio. A client with a different aspect ratio
  gets letterboxing or pillarboxing.
- The client frame rate cannot exceed `STEAM_REMOTE_FPS`.
- Proton games are render-capped at `STEAM_REMOTE_FPS` (via DXVK and
  vkd3d-proton), so they don't burn GPU on frames the stream can never
  deliver. A native title with vsync off can still render faster than the
  stream; the extra frames are discarded at capture, not streamed.

So a 4K ceiling serves 1080p clients fine — lowering it only saves GPU work
by capping what Gamescope will ever render.

:::tip[Pin the client resolution]
Keep the Steam Link client at a fixed resolution. Mid-session resolution
changes are also the one situation where the zero-copy capture path has a
known transient failure. See
[Advanced tuning](./reference/environment.md#advanced-tuning).
:::

## Everything else

There is intentionally nothing else to configure at this level. Steam
settings (streaming bitrate, controller configuration, library) are managed
inside Steam and persist in the [data folder](./data-and-backups.md). A few
expert escape hatches for the streaming pipeline exist — see the
[environment variable reference](./reference/environment.md) — but the
defaults are the supported configuration.
