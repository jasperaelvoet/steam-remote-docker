# First login & verify

The same steps apply whether you started the container with Docker, Compose,
or Quadlet. Substitute `docker`/`sudo podman` for `podman` as appropriate.

## 1. Wait for the session

```bash
podman logs -f steam-remote
```

The line to look for:

```
steam-remote: ready at 3840x2160@60
```

The very first start takes longer — Steam downloads and installs its own
updates into the data folder before the session is fully usable.

## 2. Pair a client

1. Open the **Steam Link** app on your TV, phone, or other device.
2. Let it discover the host (same network), or add the host by IP.
3. Enter the PIN Steam Link shows — the confirmation dialog appears inside
   the streamed session.
4. Log in to your Steam account through the streamed UI.

Your login, library, and settings persist in the data folder from here on —
restarts and image updates won't touch them.

## 3. Verify

```bash
podman exec steam-remote steam-remote status
```

Everything should read `true` (lifecycle may say `active`, `waiting`, or
`parked` — all healthy):

```
healthy: true
gamescope: true
pipewire: true
pulse: true
steam: true
remote play: true
lifecycle: active
...
```

`remote play: false` while everything else is `true` usually just means
you're not logged in yet — Steam only opens the Remote Play listener for a
logged-in account.

## Next steps

- Lower the resolution ceiling for a non-4K client:
  [Configuration](../configuration.md)
- Day-to-day commands: [Operations](../operations.md)
- Something not working: [Troubleshooting](../troubleshooting.md)
