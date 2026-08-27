# Docker Compose

The same container, declared in a `compose.yaml`. Works with Docker Compose
and with `podman-compose`.

## 1. Create the project

```bash
mkdir steam-remote && cd steam-remote
mkdir steam-data
```

## 2. Write `compose.yaml`

```yaml
services:
  steam-remote:
    image: ghcr.io/jasperaelvoet/steam-remote-docker:latest
    container_name: steam-remote
    restart: unless-stopped
    privileged: true
    network_mode: host
    ipc: host
    read_only: true
    tmpfs:
      - /run:rw,exec,nosuid,size=1g,mode=755
      - /tmp:rw,exec,nosuid,size=8g,mode=1777
      - /var/tmp:rw,exec,nosuid,size=2g,mode=1777
      - /var/lib/xkb:rw,exec,nosuid,size=64m,mode=1777
    volumes:
      - ./steam-data:/mnt/data:rw
    # Optional — defaults are for a 4K@60 client:
    # environment:
    #   STEAM_REMOTE_WIDTH: "1920"
    #   STEAM_REMOTE_HEIGHT: "1080"
    #   STEAM_REMOTE_FPS: "60"
```

Every entry maps 1:1 to a `docker run` flag — see
[why each flag is needed](./docker.md#why-each-flag), including
[what the tmpfs mounts are for](./docker.md#why-the-tmpfs-mounts).

## 3. Start it

```bash
docker compose up -d
```

Then continue with [First login & verify](./first-login.md).

## Day-to-day

```bash
docker compose logs -f            # watch the session
docker compose down               # stop (data is safe in ./steam-data)
docker compose pull && docker compose up -d   # update the image
```

The health check is built into the image, so `docker compose ps` shows
`healthy`/`unhealthy` without any extra configuration.
