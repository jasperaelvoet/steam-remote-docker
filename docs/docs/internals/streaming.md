# Streaming pipeline

Remote Play on a headless host has sharp edges that stock components don't
handle. The image ships three targeted fixes: a patched Gamescope (two
patches on top of 3.16.26) and an `XFixes` cursor shim. This page explains
each mechanism, mostly so future maintainers know why they exist.

## The path of a frame

```
game ── Vulkan/RADV ──▶ Gamescope (headless, composits at ≤ WxH@FPS)
                             │
                             ▼  PipeWire Video/Source node (NV12 DMA-BUF)
                        Steam (-pipewire-dmabuf)
                             │  VA-API hardware encode (H.264)
                             ▼
                        Remote Play ──▶ Steam Link client
```

Steam negotiates capture size and frame rate with the client; Gamescope fits
the capture inside the configured ceiling, preserving aspect ratio. The
encoder is AMD VA-API — the whole path from game render to encoded bitstream
can stay on the GPU. The codec is always H.264: the Linux host client offers
nothing else for Remote Play, regardless of the client's HEVC/AV1 preference.
Without the VA-API driver in the image, Steam silently falls back to software
x264 — the stream stats overlay shows which one is in use.

Gamescope's refresh rate paces compositing and capture, not the game's render
loop, so a game with vsync off would render past the stream rate and discard
the excess. The session therefore exports `DXVK_FRAME_RATE` and
`VKD3D_FRAME_RATE` at the configured FPS, capping Proton titles at the rate
the stream can actually deliver. Native titles that neither vsync nor read
those variables can still render uncapped; the extra frames are simply never
captured.

## Zero-copy capture

*Patch: `container/gamescope/pipewire-steam-capture.patch` — gated by
`STEAM_REMOTE_ZERO_COPY` (default on).*

Gamescope composites each frame, converts it to NV12 on the GPU, and shares
that exact buffer with Steam as a DMA-BUF. Steam imports it straight into
the VA-API encoder: no conversion left to do and no CPU copy anywhere in the
capture path.

Two things had to be fixed to get there:

- **Upstream's NV12 DMA-BUF advertisement was broken.** NV12 is one buffer
  but two image planes, and upstream described it as a single PipeWire data
  block (it carries a `TODO: support multi-planar DMA-BUF export`) —
  consumers rightly reject that import. The patch describes both planes as
  separate data blocks over the one BO, using the plane offsets and pitches
  Vulkan reports for the exported image.
- **Steam must be told to import.** The session launches Steam with
  `-pipewire-dmabuf` (not plain `-pipewire`), which enables its DMA-BUF
  import path for PipeWire capture.

The patch also deepens the buffer pools (6 DMA-BUF / 8 shared-memory
buffers) — the consumer holds frames through import and encode, and the
upstream default of four starves at 4K60.

`STEAM_REMOTE_ZERO_COPY=0` sets `GAMESCOPE_PIPEWIRE_NO_DMABUF`, which
withholds every DMA-BUF advertisement so Steam negotiates the always-safe
NV12 shared-memory path instead.

## Real DPI on a virtual output

*Patch: `container/gamescope/headless-output-phys-size.patch` — driven by
`STEAM_REMOTE_PHYS_MM` (default `596x335`, a 27" 16:9 panel).*

A headless Gamescope advertises a 0mm×0mm output, and X11 clients fall back
to assuming 96 DPI, which makes Steam scale its UI and cursor wrong for a 4K
stream. The patch lets `GAMESCOPE_HEADLESS_PHYS_MM` set a physical size on
the virtual output so clients derive a real DPI.

Xwayland needs the same information but takes a different route: it computes
its screen millimeters from its `-dpi` argument **once at startup** and never
updates them from the compositor. The entrypoint therefore generates an
Xwayland wrapper that passes a `-dpi` value computed from the configured
width and physical width, keeping both display servers in agreement.

## Cursor normalization

*Shim: `container/cursors/cursor-shim.c`, preloaded container-wide via
`/etc/ld.so.preload`.*

Remote Play clients (observed on macOS) stretch every cursor bitmap they
receive into a fixed on-screen box, ignoring the bitmap's size, the host's UI
scale, and the stream resolution. The apparent cursor size therefore depends
only on **how much of its bitmap the visible glyph fills** — so applications
that ship their own cursors instead of the Xcursor theme appear enormous.

Steam reads every cursor — whoever set it — through `XFixesGetCursorImage`,
so the shim interposes exactly that call: it resamples the visible glyph to
`STEAM_REMOTE_CURSOR_GLYPH` pixels (default 30) and places it at the origin
of a `STEAM_REMOTE_CURSOR_CANVAS`-sized square (default 96). The
glyph-to-canvas ratio — and therefore the size the client draws — stays
constant for every cursor.

Two implementation notes:

- Steam's launcher clears `LD_PRELOAD`, so the shim is installed through
  `/etc/ld.so.preload`, which applies container-wide. It stays inert unless
  `STEAM_REMOTE_CURSOR_GLYPH` names a size, so only the Gamescope/Steam
  session (which sets it) is affected.
- It is compiled for both x86-64 and i386 and installed under an `$LIB`
  path, because parts of Steam are 32-bit and `ld.so` expands `$LIB` per
  ABI.

## Audio

No patches here, just topology: a PipeWire null sink named
`Steam_Stream_Audio` (48kHz, 8 channels, full 7.1 map) is created at startup
and set as the default sink. Games and Steam route into it; Remote Play
captures the sink monitor and downmixes to whatever the client negotiates.
The 8-channel layout means surround-capable clients get real surround rather
than an upmixed stereo pair.
