# Configuration

The configuration surface is intentionally small: three environment variables
that set the maximum Gamescope render size and refresh rate. The defaults are
intended for a 4K Steam Link client — override only what the client or network
requires.

| Variable | Default | Meaning |
| --- | ---: | --- |
| `STEAM_REMOTE_WIDTH` | `3840` | Maximum render width in pixels |
| `STEAM_REMOTE_HEIGHT` | `2160` | Maximum render height in pixels |
| `STEAM_REMOTE_FPS` | `60` | Maximum refresh rate |

For example, for a 1080p client:

```sh
podman run -d \
  --name steam-remote \
  --env STEAM_REMOTE_WIDTH=1920 \
  --env STEAM_REMOTE_HEIGHT=1080 \
  ... \
  ghcr.io/jasperaelvoet/steam-remote-docker:latest
```

## How the ceiling behaves

These values are a **ceiling**, not a fixed output size. Steam negotiates the
actual PipeWire capture size and encoding frame rate with the connected client:

- Gamescope fits the negotiated capture within the configured render size while
  preserving its aspect ratio. A client with a different aspect ratio receives
  letterboxing or pillarboxing.
- The client frame rate cannot exceed `STEAM_REMOTE_FPS`.

So a 4K ceiling serves 1080p clients fine — lowering it only saves GPU work by
capping what Gamescope will ever render.

::: tip Pin the client resolution
Keeping the Steam Link client at a fixed resolution avoids mid-session
resolution changes, which is also the one situation where the zero-copy capture
path has a known transient failure. See
[Advanced tuning](/reference/environment#advanced-tuning).
:::

## Everything else

There is deliberately nothing else to configure at this level. Steam settings
(streaming bitrate, controller configuration, library) are managed inside Steam
itself and persist in [`/mnt/data`](/guide/persistent-data). A handful of
expert escape hatches for the streaming pipeline exist and are documented in
the [environment variable reference](/reference/environment), but the defaults
are the supported configuration.
