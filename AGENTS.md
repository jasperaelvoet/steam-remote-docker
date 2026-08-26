# Agent Notes

This repository builds one always-on Steam Remote Play OCI image. It is a
plain container with no host deployment manager in scope.

## Invariants

- Do not commit unless the user explicitly asks.
- Never store credentials, Steam Guard codes, passwords, hostnames, or private
  machine details in the repository.
- `/mnt/data` is the only persistent application-data mount and is the
  `steam` user's home. Never add a command that deletes this data.
- Keep system packages in `Containerfile`. Steam updates, games, compatibility
  tools, saves, and settings belong in the persistent home.
- Preserve host networking, the read-only root, AMD RADV/VA-API support, and
  the single-session model.
- Keep one supported runtime path: headless Gamescope, PipeWire, and Steam's
  gamepad UI. Do not add alternate sessions, recovery
  servers, or per-game wrappers.
- Defaults are `3840x2160@60`. The three documented display environment
  variables are the entire configuration surface.

## Repository shape

- `Containerfile` defines the immutable Arch Linux image.
- `container/steam-remote.sh` prepares the persistent home and starts the session.
- `scripts/*.ts` provide development checks and image builds through Bun.
- `.github/workflows/container.yml` validates and publishes the image.

Runtime diagnostics are `steam-remote status [--json]` and `steam-remote
health [--json]`.

## Development

- `bun run check` runs syntax, lint, and repository-shape checks.
- `bun run build` builds `localhost/steam-remote-docker:latest` with Podman.

Before handing off image or runtime changes, run `bun run check`, ShellCheck every
executable shell script, and build the `linux/amd64` image when Podman is
available.

Runtime validation requires Linux: confirm Vulkan and VA-API reach the AMD GPU,
PipeWire-Pulse is reachable, Gamescope publishes a capture node, Steam listens
on TCP/UDP 27036, and a Steam Link session has hardware-encoded video, audio,
controller input, reconnects cleanly, and retains the library and login after
replacing the image.
