# Idle lifecycle

An always-on host spends most of its time waiting, and rendering the Steam UI
at full rate around the clock wastes GPU power. The image ships a lifecycle
controller that parks Gamescope at 1 FPS when nothing needs it — without ever
stopping Steam, games, or downloads.

## What counts as activity

Gamescope stays at the configured full rate while **any** of these is true:

- A Remote Play stream is connected
- A Steam game is running
- A download, update, validation, or patch operation is actively running

Two details worth knowing:

- **A disconnected game keeps the session active.** Drop the stream mid-game
  and the game keeps running at full rate, so you can reconnect to the same
  session.
- **Paused or queued downloads don't count.** Only operations actually
  running keep the session active.

## States

The controller re-evaluates once per second:

| State | Meaning | Healthy? |
| --- | --- | :-: |
| `active` | A stream, game, or update is running — full rate | ✅ |
| `waiting` | Nothing active — the five-minute countdown is running, still at full rate | ✅ |
| `parked` | Idle for five minutes — Gamescope limited to 1 FPS | ✅ |
| `error` | Detection or control unavailable — full rate kept as a fail-safe | ❌ |

```
             activity            5 min quiet
  ┌────────┐ ─────────▶ ┌─────────┐ ─────────▶ ┌────────┐
  │ active │            │ waiting │            │ parked │
  └────────┘ ◀───────── └─────────┘            └────────┘
       ▲       quiet                               │
       └───────────────────────────────────────────┘
                     any activity
```

Parking never kills anything. Steam keeps running, downloads keep
downloading, and any new activity — like a Steam Link connection — restores
full rate automatically, typically within a second.

## Hibernation

Parking goes further than a 1 FPS limiter: once the parked limiter is
confirmed, the session **hibernates**. Everything not needed to accept a new
Remote Play session — Gamescope and Steam's UI helper — is frozen in place
with `SIGSTOP`. Frozen processes use zero CPU and GPU but keep all their
state in RAM, so there is nothing to reload on wake.

The main Steam process and Xwayland stay running, so the host remains
discoverable and keeps accepting connections — Steam is an X11 client, and
freezing its X server would stall the thread that answers discovery probes.
With the UI helper frozen nothing draws, so the running Xwayland costs
effectively nothing. When a client connects (or any other activity appears),
the controller thaws everything with `SIGCONT` *before* restoring the full
frame rate — wake is effectively instant. Detection uncertainty also thaws
the session: when in doubt, everything runs.

## Failing safe

The controller may only ever throttle a *provably idle* session. If activity
detection returns an unknown result, or Gamescope refuses the limiter change,
the session **stays at full rate** and the lifecycle reports `error` instead
of interrupting Steam. Streaming keeps working; the container health check
turns unhealthy so you notice.

`error` is the only unhealthy lifecycle state — `parked` is normal and
healthy.

## Updates happen here too

The parked state doubles as the safe window for updates: Steam client
updates are applied by an in-place session restart, and container image
updates are approved through the `update-gate` command — both only while
parked, both covered in [Automatic updates](./auto-updates.md).

## Observing it

```bash
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

The logs narrate transitions too:

```
steam-remote: idle countdown started (300s)
steam-remote: Gamescope parked at 1 FPS
steam-remote: Gamescope active (stream)
```

How the detectors actually work is covered in
[Architecture](./internals/architecture.md#the-lifecycle-supervisor).
