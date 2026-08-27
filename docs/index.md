---
layout: home

hero:
  name: Steam Remote Play container
  text: A headless Steam host for Steam Link
  tagline: One Gamescope session, PipeWire audio, and AMD hardware acceleration in a read-only OCI image. Steam owns discovery, pairing, streaming, audio, and input.
  image:
    src: /logo.svg
    alt: Steam Remote Play container
  actions:
    - theme: brand
      text: Get started
      link: /guide/getting-started
    - theme: alt
      text: How it works
      link: /internals/architecture
    - theme: alt
      text: GitHub
      link: https://github.com/jasperaelvoet/steam-remote-docker

features:
  - icon: 🎮
    title: Opinionated by design
    details: >
      No alternate session modes, recovery services, or per-game workarounds.
      One supported runtime path: headless Gamescope, PipeWire, and Steam's
      gamepad UI, with a 3840x2160@60 render ceiling.
  - icon: 🔒
    title: Immutable runtime
    details: >
      The Arch Linux image is read-only. System packages live in the
      Containerfile; Steam updates, games, saves, and settings live in a single
      persistent volume mounted at /mnt/data.
  - icon: 🔋
    title: Adaptive idle lifecycle
    details: >
      Gamescope runs at full rate while a stream, game, or download is active,
      then parks at 1 FPS after five minutes of quiet. Parking never stops
      Steam, games, or downloads, and activity restores full rate automatically.
  - icon: ⚡
    title: Zero-copy capture
    details: >
      A patched Gamescope steers Steam onto a zero-copy BGRx DMA-BUF capture
      that the GPU converts in place — roughly three times less CPU than the
      shared-memory path.
  - icon: 🩺
    title: Observable out of the box
    details: >
      steam-remote status and steam-remote health report readiness, lifecycle
      state, and streaming, game, update, and controller indicators as text or
      JSON, wired into the container health check.
  - icon: 🖱️
    title: Normalized streaming quirks
    details: >
      Ships targeted fixes for remote streaming: a cursor shim that keeps every
      streamed cursor the same apparent size, real DPI on the virtual display,
      and an 8-channel null audio sink for the stream.
---
