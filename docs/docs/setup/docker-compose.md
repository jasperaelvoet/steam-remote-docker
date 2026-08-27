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
    volumes:
      - ./steam-data:/mnt/data:rw
    # Optional — defaults are for a 4K@60 client:
    # environment:
    #   STEAM_REMOTE_WIDTH: "1920"
    #   STEAM_REMOTE_HEIGHT: "1080"
    #   STEAM_REMOTE_FPS: "60"
```

Every entry maps 1:1 to a `docker run` flag — see
[why each flag is needed](./docker.md#why-each-flag). Writable scratch space
is [handled inside the container](./docker.md#scratch-space-is-automatic), so
no `tmpfs:` section is needed.

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

To update the image automatically — only while nobody is playing — add
Watchtower with the built-in idle gate: see
[Automatic updates](../auto-updates.md#docker--docker-compose-watchtower).

The health check is built into the image, so `docker compose ps` shows
`healthy`/`unhealthy` without any extra configuration.
