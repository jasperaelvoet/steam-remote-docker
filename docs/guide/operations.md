# Operations

Day-two operation is four commands:

```sh
podman logs -f steam-remote
podman exec steam-remote steam-remote status
podman exec steam-remote steam-remote health --json
podman stop steam-remote
```

## Logs

The session logs to the container's stdout with a `steam-remote:` prefix for
its own events — process startup, readiness, and lifecycle transitions —
interleaved with output from Steam, Gamescope, and PipeWire:

```
steam-remote: starting gamescope
steam-remote: ready at 3840x2160@60
steam-remote: idle countdown started (300s)
steam-remote: Gamescope parked at 1 FPS
steam-remote: Gamescope active (stream)
```

## `status` vs `health`

Both commands report the same readiness checks and lifecycle indicators, in
text or `--json`. The difference is the exit code:

- **`status`** always exits `0` — use it interactively or for dashboards.
- **`health`** exits nonzero unless everything is ready — use it for alerting
  and orchestration. It is also the image's built-in `HEALTHCHECK` (30s
  interval, 5m start period), so `podman ps` and `podman healthcheck run`
  reflect it.

A session is healthy when Gamescope, PipeWire, PipeWire-Pulse, Steam, the
Remote Play listener on port `27036`, and the lifecycle controller are all up.
The `parked` lifecycle state is healthy; only `error` is not. See the
[CLI reference](/reference/cli) for the full output schema.

```sh
podman exec steam-remote steam-remote health --json | jq .
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
    "state": "parked",
    "controller_healthy": true,
    "streaming": false,
    "game_running": false,
    "update_running": false
  }
}
```

## Stopping and restarting

```sh
podman stop steam-remote
```

The image traps `SIGTERM` and shuts the session down cleanly; `catatonit`
reaps everything else. Steam's state is already on disk in `/mnt/data`, so a
restart brings back the same login and library.

::: warning Stop before touching the data
Always stop the container before backing up, moving, or inspecting
`steam-data`. Steam keeps databases open while running. See
[Persistent data & backups](/guide/persistent-data).
:::

## Updating the image

All mutable state lives in the volume, so updating is replace-and-restart:

```sh
podman pull ghcr.io/jasperaelvoet/steam-remote-docker:latest
podman stop steam-remote
podman rm steam-remote
podman run -d ... # same flags as before
```

The library, login, saves, and settings survive because they live in
`steam-data`, not the image. Steam client updates also land in the volume, so
Steam does not re-download itself after an image update.

## Running as a service

The container is a plain OCI container with no host deployment manager in
scope — wire it into whatever supervises containers on your host (for example
a restart policy):

```sh
podman update --restart=always steam-remote
```

Because `health` is the image's `HEALTHCHECK`, any supervisor that reacts to
container health can restart the session on sustained failure.
