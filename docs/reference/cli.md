# `steam-remote` CLI

`steam-remote` is the container's single entrypoint and diagnostic tool,
installed at `/usr/local/bin/steam-remote`.

```
steam-remote {run|status|health} [--json]
```

## `steam-remote run`

Starts the session. This is the container's default command — you never run it
yourself. It prepares the persistent home, starts DBus, PipeWire, WirePlumber,
PipeWire-Pulse, the virtual audio sink, Gamescope, and Steam, then supervises
them. If any required process exits, `run` exits nonzero so the container
stops as a unit.

## `steam-remote status [--json]`

Reports readiness **without failing** — the exit code is always `0`. Use it
interactively and in dashboards.

```sh
podman exec steam-remote steam-remote status
```

```
healthy: true
gamescope: true
pipewire: true
pulse: true
steam: true
remote play: true
lifecycle: active
lifecycle controller: true
streaming: true
game running: false
update running: false
```

## `steam-remote health [--json]`

Same report as `status`, but the exit code is the verdict: `0` when healthy,
nonzero otherwise. This command backs the image's `HEALTHCHECK` (30-second
interval, 10-second timeout, 5-minute start period, 3 retries).

```sh
podman exec steam-remote steam-remote health --json
```

```json
{
  "healthy": true,
  "checks": {
    "gamescope": true,
    "pipewire": true,
    "pulse": true,
    "steam": true,
    "remote_play": true,
    "lifecycle": true
  },
  "lifecycle": {
    "state": "active",
    "controller_healthy": true,
    "streaming": true,
    "game_running": false,
    "update_running": false
  }
}
```

## Fields

### Readiness checks

`healthy` is true only when every check is true.

| Field | True when |
| --- | --- |
| `gamescope` | A Gamescope process is running as the `steam` user |
| `pipewire` | The PipeWire socket exists in the session runtime directory |
| `pulse` | The PipeWire-Pulse socket exists |
| `steam` | A Steam (or Steam web helper) process is running |
| `remote_play` | Something is listening on TCP `27036` |
| `lifecycle` | The lifecycle controller is healthy **and** its heartbeat is fresh (updated within the last 10 seconds) |

### Lifecycle block

| Field | Values | Meaning |
| --- | --- | --- |
| `state` | `active` \| `waiting` \| `parked` \| `error` | Current [lifecycle state](/guide/idle-lifecycle#states); `parked` is healthy |
| `controller_healthy` | `true` \| `false` | Whether the controller can detect activity and drive Gamescope's limiter |
| `streaming` | `true` \| `false` \| `null` | A Remote Play connection or running capture was detected (`null`/`unknown`: the detector could not tell) |
| `game_running` | `true` \| `false` \| `null` | A process with a Steam app identity is running |
| `update_running` | `true` \| `false` \| `null` | A download, update, validation, or patch is actively running |

In text output the three indicators print `unknown` instead of `null`.
