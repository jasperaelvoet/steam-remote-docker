---
name: steam-remote-image-maintenance
description: Modify or review this repository's Steam Remote Play container image, runtime entrypoint, package set, or OCI publishing workflow.
---

# Steam Remote image maintenance

Read the repository `AGENTS.md` before changing the image or runtime.

Keep the result as one opinionated container and preserve these boundaries:

- `/mnt/data` remains the only persistent application-data mount.
- The root filesystem remains read-only at runtime.
- Gamescope, PipeWire, and Steam form one headless session on host networking.
- Configuration stays limited to width, height, and refresh rate.
- System dependencies belong in `Containerfile`; mutable Steam state belongs
  in the persistent home.

After changes, run `bun run check`. Build the `linux/amd64` image when image
contents changed and Podman is available. Do not claim runtime success from
source checks alone.
