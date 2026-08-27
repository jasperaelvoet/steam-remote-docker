# Troubleshooting

Start every investigation the same way:

```sh
podman exec steam-remote steam-remote status
podman logs --tail 200 steam-remote
```

`status` never fails; it tells you which readiness check is down and what the
lifecycle thinks is happening. The sections below map symptoms to causes.

## Steam Link cannot find the host

Discovery and streaming need the host's real network identity and open ports.

- Confirm the container runs with `--network host`.
- Allow UDP `27031-27036` and TCP `27036` through the host firewall
  ([details](/reference/networking)).
- Check that the Remote Play listener is up: `status` should show
  `remote play: true`. If Steam is up but the listener is not, you are usually
  not logged in yet — Remote Play only listens for a logged-in account with
  Remote Play enabled.
- Discovery is broadcast-based; the client must be on the same L2 network, or
  you add the host by IP in the Steam Link app.

## `health` reports unhealthy

Each check maps to one subsystem:

| Failing check | Look at |
| --- | --- |
| `gamescope` | The compositor died — logs will show why; usually GPU access (`--privileged`, AMD GPU present, `/dev/dri` on the host) |
| `pipewire` / `pulse` | The audio/video plumbing sockets are missing — the session likely failed early; read the startup logs |
| `steam` | Steam exited — its own log output precedes this |
| `remote_play` | Nothing listening on TCP `27036` — usually not logged in, or Remote Play disabled in Steam settings |
| `lifecycle` | The idle controller cannot detect activity or control Gamescope — see below |

During the first five minutes after start the container health check is in its
start period; Steam's first login and update can legitimately take a while.

## Lifecycle state is `error`

`error` means detection or control is degraded — **streaming still works**,
the session just stays at full rate as a fail-safe. The `status` indicators
show which detector is `unknown`:

- `streaming: unknown` — the PipeWire capture node or socket query failed
- `game running: unknown` — `/proc` was not readable for process environments
- `update running: unknown` — Steam's content log could not be read

A persistent `error` right after startup commonly resolves once Steam finishes
its first update and creates its log files.

## No audio on the client

The image routes all audio through a virtual 8-channel sink
(`Steam_Stream_Audio`) that is set as the default; Steam captures it for the
stream. If a game has no sound:

- Check the game isn't targeting a non-default device inside the streamed
  session's audio settings.
- Confirm `pulse: true` in `status` — without the PipeWire-Pulse socket,
  games find no audio server at all.

There is deliberately no host audio device passthrough; the only audio path is
the stream.

## Stream drops when changing resolution mid-session

The zero-copy capture path has one known transient failure: a VA surface
allocation during a mid-session client resolution change. Pin the Steam Link
client to a fixed resolution, or set `STEAM_REMOTE_ZERO_COPY=0` to use the
always-safe shared-memory path at ~3x the capture CPU cost. See
[Advanced tuning](/reference/environment#advanced-tuning).

## Controllers do not work

Controller emulation needs `/dev/uinput` and `/dev/uhid` from the host, which
`--privileged` provides. If a controller pairs but games ignore it, verify the
devices exist on the host (`ls /dev/uinput /dev/uhid`) and that no host
process (another Steam instance, an input remapper) has claimed them.

## The container keeps restarting

The session treats every core process as required: if Gamescope, Steam, DBus,
or PipeWire exits, the entrypoint logs
`steam-remote: a required process exited` and stops, letting your restart
policy bring the whole session back cleanly rather than limping along. The
process that actually failed logs immediately before that line.

## Cursor looks wrong on the client

Streamed cursor size is normalized by a shim (glyph rescaled onto a fixed
canvas) precisely because Remote Play clients stretch whatever cursor bitmap
they receive. If a specific application's cursor still misbehaves, the
[advanced cursor variables](/reference/environment#advanced-tuning) adjust the
canvas and glyph sizes.

## Still stuck?

Open an issue with the output of:

```sh
podman exec steam-remote steam-remote status --json
podman logs --tail 500 steam-remote
```

at [github.com/jasperaelvoet/steam-remote-docker](https://github.com/jasperaelvoet/steam-remote-docker/issues).
