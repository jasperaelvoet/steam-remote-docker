# Data & backups

The container has exactly one persistent location: the volume mounted at
`/mnt/data`, which is the `steam` user's home directory. Everything else is
the read-only image or throwaway tmpfs.

## What lives in the data folder

- The Steam client installation and its self-updates
- Your Steam login and machine authorization
- The game library — installed games, shader caches, compatibility tools
- Saves and cloud-sync state
- Steam settings (streaming quality, controller configs, library layout)
- A stable per-install `machine-id`, so Steam Link pairing survives container
  replacement

The split is strict by design: system packages belong in the image, anything
Steam writes belongs in the volume. That's what makes image updates a
pull-and-replace with no migration steps.

## Backups

Stop the container first — Steam keeps databases open while running, and a
copy taken mid-write can be inconsistent:

```bash
podman stop steam-remote
tar -C "$PWD" -czf steam-data-$(date +%F).tar.gz steam-data
podman start steam-remote
```

Moving to another machine is the same operation: stop, copy the folder, run
the container there with the same volume mount.

:::danger[Never let automation delete /mnt/data contents]
This folder is your library, saves, and login. Treat it like a home
directory, because it is one.
:::

## Ownership

Inside the container the volume is owned by `steam` (UID/GID `1000`). An
empty host directory is all you need to provide — the container creates the
structure it needs with the right ownership on startup. If you restore or
pre-populate data, keep it owned by UID `1000`.

## Sizing

Plan for your Steam library plus overhead — shader caches and client updates
add tens of gigabytes over time. The tmpfs mounts only hold transient session
data and reset on every restart.
