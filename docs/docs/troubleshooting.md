# Troubleshooting

Start every investigation the same way:

```bash
podman exec steam-remote steam-remote status
podman logs --tail 200 steam-remote
```

`status` never fails; it tells you which check is down and what the lifecycle
thinks is happening. Then find your symptom below.

## Steam Link cannot find the host

- Confirm the container runs with host networking (`--network host` /
  `network_mode: host` / `Network=host`).
- Allow UDP `27031-27036` and TCP `27036` through the host firewall
  ([details](./networking.md)).
- Check `status` shows `remote play: true`. If Steam is up but the listener
  isn't, you're usually not logged in yet — Remote Play only listens for a
  logged-in account.
- Discovery is broadcast-based: the client must be on the same network, or
  you add the host by IP in the Steam Link app.

## `health` reports unhealthy

Each failing check points at one subsystem:

| Failing check | Look at |
| --- | --- |
| `gamescope` | The compositor died — usually GPU access: privileged mode, AMD GPU present, `/dev/dri` on the host |
| `pipewire` / `pulse` | Audio/video plumbing sockets missing — the session likely failed early; read the startup logs |
| `steam` | Steam exited — its own log output comes right before |
| `remote_play` | Nothing listening on TCP `27036` — usually not logged in, or Remote Play disabled in Steam settings |
| `lifecycle` | The idle controller can't detect activity or control Gamescope — see below |

The health check has a five-minute grace period after start; Steam's first
login and self-update can legitimately take a while.

## Lifecycle state is `error`

`error` means detection or control is degraded — **streaming still works**;
the session just stays at full rate as a fail-safe. The `status` indicators
show which detector is `unknown`:

- `streaming: unknown` — the PipeWire capture node query failed
- `game running: unknown` — process environments weren't readable
- `update running: unknown` — Steam's content log couldn't be read

A persistent `error` right after startup usually resolves once Steam finishes
its first update and creates its log files.

## No audio on the client

All audio flows through a virtual sink (`Steam_Stream_Audio`) that is the
default device; Steam captures it for the stream. If a game is silent:

- Check the game isn't targeting a non-default audio device in the streamed
  session.
- Confirm `pulse: true` in `status` — without that socket, games find no
  audio server at all.

There is deliberately no host audio passthrough; the only audio path is the
stream.

## Stream drops when changing resolution mid-session

The zero-copy capture path has one known transient failure: a VA surface
allocation during a mid-session client resolution change. Pin the Steam Link
client to a fixed resolution, or set `STEAM_REMOTE_ZERO_COPY=0` to use the
always-safe shared-memory path at ~3x the capture CPU cost. See
[Advanced tuning](./reference/environment.md#advanced-tuning).

## Controllers don't work

Controller emulation needs `/dev/uinput` and `/dev/uhid` from the host, which
privileged mode provides. If a controller pairs but games ignore it, verify
the devices exist on the host (`ls /dev/uinput /dev/uhid`) and that no host
process (another Steam instance, an input remapper) has claimed them.

## The container keeps restarting

Every core process is required: if Gamescope, Steam, DBus, or PipeWire exits,
the entrypoint logs `steam-remote: a required process exited` and stops, so
your restart policy brings the whole session back cleanly instead of limping
along. The process that actually failed logs immediately before that line.

## Cursor looks wrong on the client

Streamed cursor size is normalized by a shim (the glyph is rescaled onto a
fixed canvas) because Remote Play clients stretch whatever cursor bitmap they
receive. If a specific application's cursor still misbehaves, the
[advanced cursor variables](./reference/environment.md#advanced-tuning)
adjust the canvas and glyph sizes.

## Still stuck?

Open an issue with the output of:

```bash
podman exec steam-remote steam-remote status --json
podman logs --tail 500 steam-remote
```

at [github.com/jasperaelvoet/steam-remote-docker](https://github.com/jasperaelvoet/steam-remote-docker/issues).
