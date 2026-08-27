# Operations

Day-to-day operation is four commands:

```bash
podman logs -f steam-remote
podman exec steam-remote steam-remote status
podman exec steam-remote steam-remote health --json
podman stop steam-remote
```

(Swap in `docker`, `docker compose`, or `systemctl`/`journalctl` for your
[setup method](./setup/docker.md).)

## Logs

The session logs to the container's stdout. The image's own events carry a
`steam-remote:` prefix — startup, readiness, lifecycle transitions —
interleaved with output from Steam, Gamescope, and PipeWire:

```
steam-remote: starting gamescope
steam-remote: ready at 3840x2160@60
steam-remote: idle countdown started (300s)
steam-remote: Gamescope parked at 1 FPS
steam-remote: Gamescope active (stream)
```

## `status` vs `health`

Both report the same checks and indicators, as text or `--json`. The
difference is the exit code:

- **`status`** always exits `0` — use it interactively and in dashboards.
- **`health`** exits nonzero unless everything is ready — use it for
  alerting. It also backs the image's built-in `HEALTHCHECK` (30s interval,
  5m start period), so `podman ps` shows container health with no extra
  setup.

A session is healthy when Gamescope, PipeWire, PipeWire-Pulse, Steam, the
Remote Play listener, and the lifecycle controller are all up. `parked` is
healthy; only `error` is not. Full output schema in the
[CLI reference](./reference/cli.md).

## Stopping and restarting

```bash
podman stop steam-remote
```

The image shuts the session down cleanly on `SIGTERM`. Steam's state is
already on disk in the data folder, so a restart brings back the same login
and library.

:::warning[Stop before touching the data]
Always stop the container before backing up, moving, or inspecting the data
folder — Steam keeps databases open while running. See
[Data & backups](./data-and-backups.md).
:::

## Updating the image

All mutable state lives in the data folder, so updating is
replace-and-restart:

```bash
podman pull ghcr.io/jasperaelvoet/steam-remote-docker:latest
podman stop steam-remote
podman rm steam-remote
podman run -d ... # same flags as before
```

Or let it happen automatically — image pulls gated on the session being
idle, and Steam client updates handled entirely by the container itself: see
[Automatic updates](./auto-updates.md).

Library, login, saves, and settings survive — they live in the volume, not
the image. Steam client updates also land in the volume, so Steam doesn't
re-download itself after an image update.
