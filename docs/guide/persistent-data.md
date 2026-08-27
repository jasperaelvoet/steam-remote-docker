# Persistent data & backups

The container has exactly one persistent location: the volume mounted at
`/mnt/data`, which is the `steam` user's home directory. Everything else is a
read-only image or throwaway tmpfs.

## What lives in the volume

- The Steam client installation and its self-updates
- Your Steam login and machine authorization
- The game library — installed games, shader caches, compatibility tools
- Saves and cloud-sync state
- Steam settings (streaming quality, controller configs, library layout)
- A stable per-install `machine-id`, so Steam Link pairing and Steam's device
  authorization survive container replacement

The split is strict by design: system packages belong in the image
(`Containerfile`), and anything Steam writes belongs in the volume. That is
what makes image updates safe — replacing the container never touches your
library or login.

## Backups

Stop the container first. Steam keeps databases and content logs open while
running, and a copy taken mid-write can be inconsistent:

```sh
podman stop steam-remote
tar -C "$PWD" -czf steam-data-$(date +%F).tar.gz steam-data
podman start steam-remote
```

Moving to another host is the same operation: stop, copy `steam-data`, run the
container there with the same volume mount.

::: danger Never delete /mnt/data contents from automation
This directory is your library, saves, and login. Treat it like a home
directory, because it is one.
:::

## Ownership and permissions

Inside the container the volume is owned by `steam` (UID/GID `1000`). The
entrypoint creates the directory structure it needs on startup with the right
ownership, so an empty host directory is all you need to provide. If you
pre-populate or restore data, keep it owned by UID `1000`.

## Sizing

Plan for your Steam library plus overhead: shader caches and client updates
add tens of gigabytes over time. The volume grows as Steam downloads; the
tmpfs mounts (`/tmp` at 8g being the largest) only hold transient session
data and reset on restart.
