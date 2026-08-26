---
name: steam-remote-runtime-validation
description: Validate a built or running steam-remote-docker image on a Linux AMD host, including persistence and a real Steam Link stream.
---

# Steam Remote runtime validation

Use this skill only when a Linux AMD host and the target container are in
scope. Treat `/mnt/data` as irreplaceable user data and never remove or
rewrite it during validation.

Check the running stack with `steam-remote health`, then verify the evidence
that process checks cannot prove:

1. Vulkan and VA-API select the intended AMD render device.
2. PipeWire-Pulse responds and Gamescope publishes a capture node.
3. Steam listens on TCP and UDP port 27036.
4. A real Steam Link client receives hardware-encoded video and audio and can
   send controller input.
5. Reconnecting and replacing the container preserves the Steam library,
   login, saves, and settings.

Report source checks, image-build results, container health, and real-stream
results separately. Mark any step that was not run instead of inferring it.
