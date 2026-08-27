# Automatic updates

Everything on the host can keep itself up to date without interrupting
anyone. Updates happen in two layers, and both only act while the session is
[parked](./idle-lifecycle.md#states) — no stream, no game, no download.

Set `STEAM_REMOTE_AUTO_UPDATE=0` to turn both layers off.

## Steam client updates (built in)

Steam applies its own pending client updates only when it launches — a host
that runs for months would never install them. The container handles this
itself: once the session has been parked for an hour, and at most once per
24 hours, it restarts Steam in place. Steam comes back a few seconds later
with updates applied and re-checks its game-update queue on the way up.
Nothing to configure.

## Container image updates

A container cannot replace its own image — that's the engine's job on the
host. What the image provides is the safety half: an **update gate**.

```bash
podman exec steam-remote steam-remote update-gate
```

It exits `0` only when the session is parked (safe to replace the
container), and `75` otherwise — the exact code that tells both Watchtower
and systemd to skip gracefully and try again later. Wire it into whichever
updater fits your setup:

### Docker / Docker Compose: Watchtower

Add [Watchtower](https://containrrr.dev/watchtower/) alongside the container
and point its pre-update lifecycle hook at the gate:

```yaml
services:
  steam-remote:
    # ... as in the setup guide ...
    labels:
      com.centurylinklabs.watchtower.lifecycle.pre-update: >-
        /usr/local/bin/steam-remote update-gate

  watchtower:
    image: containrrr/watchtower
    restart: unless-stopped
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
    environment:
      WATCHTOWER_LIFECYCLE_HOOKS: "true"
      WATCHTOWER_CLEANUP: "true"
      WATCHTOWER_POLL_INTERVAL: "3600"
```

Watchtower checks hourly; when a new image exists it runs the gate first. A
busy or merely-awake session answers `75` and the update waits for the next
check. Your data folder is untouched either way — an image swap keeps login,
library, and saves.

### Podman Quadlet: podman-auto-update

The [Quadlet guide](./setup/podman-quadlet.md)'s unit already carries
`AutoUpdate=registry`. Enable the update timer and gate it on the session
being parked with one drop-in,
`/etc/systemd/system/podman-auto-update.service.d/steam-remote-gate.conf`:

```ini
[Service]
ExecCondition=/usr/bin/podman exec steam-remote steam-remote update-gate
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now podman-auto-update.timer
```

The timer fires daily; the `ExecCondition` skips the whole run unless the
gate approves. Note the drop-in gates every auto-updated container on that
host — on a dedicated gaming host that's exactly what you want.

## Why this never interrupts anything

A parked session by definition has had no stream, no running game, and no
active download for at least five minutes — a disconnected-but-running game
keeps the session `active` and therefore blocks both layers. The gate closes
again the moment anything wakes the session.
