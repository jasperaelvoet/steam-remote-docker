# Architecture

One container, one session, one supported path. The image is an immutable
Arch Linux root with a single entrypoint script,
[`container/steam-remote.sh`](https://github.com/jasperaelvoet/steam-remote-docker/blob/main/container/steam-remote.sh),
that assembles the session and supervises it.

## Process tree

```
catatonit (PID 1, reaps orphans, forwards signals)
└── steam-remote run
    ├── dbus-daemon --system
    ├── dbus-daemon --session          (as steam)
    ├── pipewire                       (as steam)
    ├── wireplumber                    (as steam)
    ├── pipewire-pulse                 (as steam)
    ├── gamescope --backend headless   (as steam)
    │   ├── Xwayland ×2 (DPI-corrected wrapper)
    │   └── steam -pipewire-dmabuf -gamepadui
    │       └── games…
    └── lifecycle supervisor           (auxiliary shell loop)
```

Startup is strictly ordered: each stage waits for the previous one's socket
(system bus, session bus, PipeWire, PipeWire-Pulse, then Gamescope's Wayland
socket) before continuing. When everything is up, the entrypoint prints
`steam-remote: ready at WxH@FPS` and waits.

**Every core process is required.** If any of them exits unexpectedly, `run`
logs `a required process exited` and returns nonzero, stopping the container.
There is deliberately no partial restart — the only in-place restart is the
deliberate, whole-session one used for
[idle-time updates](../auto-updates.md); anything else is
left to the container restart policy. The lifecycle supervisor is the one
*auxiliary* process: it can die without taking the session down (health
degrades instead).

## Privilege model

The container starts as root, prepares the runtime, then runs everything
session-related as the unprivileged `steam` user (UID/GID `1000`) via
`setpriv` with a minimal, explicit environment. Root retains only PID 1, the
system bus, and the lifecycle supervisor. Runtime preparation:

- Creates `/mnt/data` and state directories with `steam` ownership
- Generates a persistent `machine-id` in the volume on first run and
  bind-mounts it over `/etc/machine-id`, so Steam sees a stable machine
  identity across image replacements
- Opens up `/dev/dri/*`, `/dev/input/event*`, `/dev/uinput`, and `/dev/uhid`
  for the session user

## Read-only root

The image declares nothing writable except `/mnt/data`. The entrypoint
mounts its own tmpfs scratch space for `/run`, `/tmp`, `/var/tmp`, and
`/var/lib/xkb` at startup — possible because the container runs privileged —
so no `--tmpfs` flags are required
([what each holds](../setup/docker.md#scratch-space-is-automatic)). The split is an
enforced invariant: system software changes require an image rebuild, and
Steam's own updates land in the volume. This is what makes image updates a
pull-and-replace operation with no migration steps.

## The session

Gamescope runs headless (no physical display) at the configured render
ceiling, with `--force-windows-fullscreen`, real-time scheduling (`--rt`,
enabled by `cap_sys_nice` on the binary), and two Xwayland servers — one for
Steam's main UI, one keeping overlays and popups routed correctly. Steam runs
in gamepad UI mode (`-gamepadui`) with PipeWire DMA-BUF capture (`-pipewire-dmabuf`), which
is what Remote Play streams.

The Xwayland instances are launched through a generated wrapper that passes
an explicit `-dpi` matching the advertised physical display size — Xwayland
derives its screen millimeters from DPI once at startup and never updates
them, and without this Steam scales its UI and streamed cursor for 96 DPI.

Audio is a virtual 8-channel 48kHz null sink (`Steam_Stream_Audio`) set as
the default sink. Games play into it, Steam captures it for the stream; there
is no host audio device.

The Gamescope build itself is patched for this headless streaming use case —
see [Streaming pipeline](./streaming.md).

## The lifecycle supervisor

The supervisor is a once-per-second loop in the entrypoint script that drives
the [idle lifecycle](../idle-lifecycle.md). Each tick it:

1. **Detects streaming** — established TCP connections on `27036`, the state
   of Gamescope's PipeWire `Video/Source` node (`pw-cli`), and growth of
   Steam's `streaming_log.txt` (session requests arrive over UDP and leave
   no connection state, but Steam logs them instantly — this is what wakes a
   hibernated session before Steam needs its frozen helpers).
2. **Detects games** — scans `/proc/*/environ` for
   `SteamAppId`/`SteamGameId`.
3. **Detects updates** — tails Steam's `content_log.txt` incrementally
   (tracking inode and offset, so log rotation is handled) and maintains the
   set of app IDs with an operation actually in the `Running` state.
4. **Combines** the three signals: any `unknown` wins over `true`, `true`
   over `false` — uncertainty always fails toward full rate.
5. **Drives the limiter** — on transition, sets Gamescope's FPS cap via
   `gamescopectl debug_set_fps_limit` (configured FPS when active, 1 FPS when
   parked), retrying every 5 seconds on failure.
6. **Publishes state** — atomically writes
   `state/streaming/game/update/controller/heartbeat` to
   `/run/steam-remote/lifecycle`, which is what `status` and `health` read. A
   heartbeat older than 10 seconds marks the controller unhealthy.
7. **Hibernates the parked session** — once the parked limiter is confirmed,
   it freezes Gamescope and the Steam web helper with `SIGSTOP`; the main
   Steam process and Xwayland keep running so the host stays discoverable
   (steam is an X11 client — a frozen X server stalls its discovery
   responder) and the kernel still completes incoming connections. Any wake
   signal — or any detector uncertainty — thaws everything with `SIGCONT`
   before the limiter is raised.
8. **Schedules update restarts** — once the session has been parked for an
   hour and at least 24 hours have passed since the last restart (stamp in
   the data folder), it drops a restart marker and terminates the session's
   processes. The entrypoint sees the marker, cleans up, and runs the whole
   session again in-process — Steam applies pending client updates on the
   way up. `STEAM_REMOTE_AUTO_UPDATE=0` disables this step.

The design constraint throughout: the supervisor may only ever throttle a
provably idle session. Any detection or control failure keeps full rate and
surfaces as an unhealthy `error` state rather than touching Steam.

## Image build

The `Containerfile` is a three-stage build:

1. **gamescope-builder** — builds the patched Gamescope 3.16.26 package with
   `makepkg` from the vendored PKGBUILD and patches.
2. **cursor-shim-builder** — compiles the cursor-normalization shim for both
   x86-64 and i386 (parts of Steam are 32-bit), so `ld.so` can pick the right
   ABI via `$LIB`.
3. **Runtime** — Arch Linux with Steam from multilib, Mesa/RADV and VA-API
   for AMD, PipeWire, and the patched Gamescope installed over the stock
   package. The shim is registered in `/etc/ld.so.preload` (Steam's launcher
   clears `LD_PRELOAD`, so a preload file is the only reliable hook).

CI validates sources with `bun run check` and ShellCheck, then builds and
publishes `linux/amd64` to `ghcr.io/jasperaelvoet/steam-remote-docker` on
every push to `main`.
