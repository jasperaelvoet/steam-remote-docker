# Development

Development tooling runs through [Bun](https://bun.sh) (1.4 or newer). The
TypeScript scripts in `scripts/` are the whole toolchain — there is no other
build system.

```sh
bun install --frozen-lockfile
bun run check
bun run build
```

## Repository shape

```
Containerfile                  # the immutable Arch Linux image (3 stages)
container/
  steam-remote.sh              # entrypoint: session assembly, lifecycle, diagnostics
  gamescope/                   # PKGBUILD + patches for the custom Gamescope build
  cursors/cursor-shim.c        # streamed-cursor normalization shim
scripts/
  check.ts                     # syntax, lint, and repository-shape checks
  build.ts                     # container image build wrapper
  lifecycle.test.ts            # unit tests for the lifecycle logic
.github/workflows/container.yml  # validate + publish CI
```

## `bun run check`

Runs the full validation suite:

- `tsc --noEmit` over `scripts/`
- Repository-shape checks: required files exist and are non-empty, removed
  legacy paths stay removed, and a scan that keeps out-of-scope concepts
  (alternate streaming stacks, host deployment managers, …) from creeping back
  into the core files
- `bash -n` and ShellCheck on `container/steam-remote.sh` (ShellCheck is
  skipped with a warning if not installed) and `bash -n` on the PKGBUILD
- The lifecycle unit tests (`scripts/lifecycle.test.ts`), which exercise the
  entrypoint's parsing and state-machine functions by sourcing the script

## `bun run build`

Builds `localhost/steam-remote-docker:latest` for `linux/amd64` with Podman,
stamping the image with the current Git revision. Set
`CONTAINER_ENGINE=docker` to use Docker instead, and `IMAGE=` to override the
tag.

The Gamescope package is compiled from source inside the build's first stage,
so a cold build takes a while.

## Contribution invariants

The project maintains a small set of hard rules (see
[`AGENTS.md`](https://github.com/jasperaelvoet/steam-remote-docker/blob/main/AGENTS.md)):

- One supported runtime path: headless Gamescope, PipeWire, Steam's gamepad
  UI. No alternate sessions, recovery servers, or per-game wrappers.
- `/mnt/data` is the only persistent mount; nothing may ever delete its
  contents.
- System packages belong in `Containerfile`; everything Steam writes belongs
  in the persistent home.
- Defaults are `3840x2160@60`, and the three documented display variables are
  the entire supported configuration surface.
- Host networking, the read-only root, and AMD RADV/VA-API support are
  preserved.

## Validating runtime changes

Source checks run anywhere, but runtime validation requires Linux with an AMD
GPU. Before shipping image or runtime changes, confirm:

- Vulkan and VA-API reach the GPU inside the container
- PipeWire-Pulse is reachable and Gamescope publishes a capture node
- Steam listens on TCP/UDP `27036`
- A real Steam Link session gets hardware-encoded video, audio, and
  controller input, reconnects cleanly, and keeps its library and login after
  the image is replaced

## CI

Every push to `main` runs the check suite and, when it passes, builds and
publishes the `linux/amd64` image to
`ghcr.io/jasperaelvoet/steam-remote-docker` as `latest` plus a
`sha-<commit>` tag.

## Documentation

This site is built with [VitePress](https://vitepress.dev) from `docs/`:

```sh
bun run docs:dev     # local dev server with hot reload
bun run docs:build   # production build (also validates links)
bun run docs:preview # serve the production build
```

It deploys to GitHub Pages automatically on pushes to `main` that touch
`docs/`.
