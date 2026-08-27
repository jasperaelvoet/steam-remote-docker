# Idle lifecycle

An always-on Steam host spends most of its time waiting. Rendering the Steam UI
at full rate around the clock wastes GPU power, so the image ships a lifecycle
controller that parks Gamescope when nothing needs it — without ever stopping
Steam, games, or downloads.

## What counts as activity

Gamescope stays at the configured full rate while **any** of these is true:

- A Remote Play stream is connected (an established connection on TCP `27036`,
  or Gamescope's PipeWire capture node is running)
- A Steam game is running (any process with a `SteamAppId` or `SteamGameId`
  environment)
- A download, update, validation, or patch operation is actively running
  (tracked from Steam's content log)

Two consequences worth noting:

- **A disconnected game keeps the session active.** If you drop the stream
  mid-game, the game continues running at full rate so the same session can
  reconnect.
- **Paused or queued updates do not keep the session active.** Only operations
  that are actually running count.

## States

The controller re-evaluates once per second and moves through four states:

| State | Meaning | Healthy? |
| --- | --- | :-: |
| `active` | A stream, game, or update is running; Gamescope runs at full rate | ✅ |
| `waiting` | No activity; the five-minute idle countdown is running at full rate | ✅ |
| `parked` | Idle for five minutes; Gamescope is limited to 1 FPS | ✅ |
| `error` | Activity detection or Gamescope control is unavailable; full rate is kept | ❌ |

```
             activity            5 min quiet
  ┌────────┐ ─────────▶ ┌─────────┐ ─────────▶ ┌────────┐
  │ active │            │ waiting │            │ parked │
  └────────┘ ◀───────── └─────────┘            └────────┘
       ▲       quiet                               │
       └───────────────────────────────────────────┘
                     any activity
```

Parking only adjusts Gamescope's frame limiter (via `gamescopectl`). Steam
keeps running, downloads keep downloading, and a new Steam Link connection or
any other activity restores the configured rate automatically — typically
within a second.

## Failing safe

The controller is designed to never get in Steam's way. If activity detection
returns an unknown result, or Gamescope refuses the limiter change, the session
**stays at full rate** and the lifecycle reports `error` instead of
interrupting Steam. The container's health check then reports unhealthy so you
notice, while streaming continues to work.

`error` is the only unhealthy lifecycle state — `parked` is a normal, healthy
condition.

## Observing it

Both diagnostics report the lifecycle state and the individual activity
indicators:

```sh
podman exec steam-remote steam-remote status
```

```
healthy: true
...
lifecycle: parked
lifecycle controller: true
streaming: false
game running: false
update running: false
```

Log lines also narrate transitions:

```
steam-remote: idle countdown started (300s)
steam-remote: Gamescope parked at 1 FPS
steam-remote: Gamescope active (stream)
```

For the implementation details — how each detector works and how the
supervisor is wired — see [Architecture](/internals/architecture#the-lifecycle-supervisor).
