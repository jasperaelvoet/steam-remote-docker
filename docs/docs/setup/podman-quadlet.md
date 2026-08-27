# Podman Quadlet

Quadlet runs the container as a proper systemd service: it starts at boot,
restarts on failure, and logs to the journal. Needs Podman 4.4 or newer.

:::note Run it rootful
Use a **system** unit (root). The container needs `--privileged` device
access (`/dev/dri`, `/dev/uinput`, `/dev/uhid`), which doesn't map cleanly
into a rootless user namespace.
:::

## 1. Create the data folder

```bash
sudo mkdir -p /var/lib/steam-remote/steam-data
```

## 2. Write the unit

Create `/etc/containers/systemd/steam-remote.container`:

```ini
[Unit]
Description=Steam Remote Play container
Wants=network-online.target
After=network-online.target

[Container]
Image=ghcr.io/jasperaelvoet/steam-remote-docker:latest
ContainerName=steam-remote
AutoUpdate=registry
Network=host
ReadOnly=true
PodmanArgs=--privileged --ipc host
Volume=/var/lib/steam-remote/steam-data:/mnt/data:rw
# Optional — defaults are for a 4K@60 client:
# Environment=STEAM_REMOTE_WIDTH=1920
# Environment=STEAM_REMOTE_HEIGHT=1080

[Service]
Restart=always
# First start pulls the image and lets Steam self-update; give it room.
TimeoutStartSec=900

[Install]
WantedBy=multi-user.target
```

Every key maps 1:1 to a `docker run`/`podman run` flag — see
[why each flag is needed](./docker.md#why-each-flag). Writable scratch space
is [handled inside the container](./docker.md#scratch-space-is-automatic), so
no `Tmpfs=` lines are needed.

## 3. Start it

Quadlet generates the service when systemd reloads:

```bash
sudo systemctl daemon-reload
sudo systemctl start steam-remote.service
```

It is enabled at boot automatically via the `[Install]` section. Then
continue with [First login & verify](./first-login.md) — for exec commands,
prefix with `sudo`, e.g.
`sudo podman exec steam-remote steam-remote status`.

## Day-to-day

```bash
sudo journalctl -fu steam-remote.service    # watch the session
sudo systemctl stop steam-remote.service    # stop (data is safe)
sudo systemctl restart steam-remote.service
```

### Automatic image updates

`AutoUpdate=registry` marks the container for Podman's auto-update machinery.
Enable the timer — and gate it so updates only happen while the session is
idle — as shown in
[Automatic updates](../auto-updates.md#podman-quadlet-podman-auto-update).
Your library and login are untouched by updates; they live in
`/var/lib/steam-remote/steam-data`, not the image.
