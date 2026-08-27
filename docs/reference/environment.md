# Environment variables

## Display configuration

These three variables are the supported configuration surface. They set the
maximum Gamescope render size and refresh rate; Steam negotiates the actual
capture size and frame rate with the client within this ceiling.

| Variable | Default | Meaning |
| --- | ---: | --- |
| `STEAM_REMOTE_WIDTH` | `3840` | Maximum render width in pixels |
| `STEAM_REMOTE_HEIGHT` | `2160` | Maximum render height in pixels |
| `STEAM_REMOTE_FPS` | `60` | Maximum refresh rate; also the rate restored when the session leaves the parked state |

See [Configuration](/guide/configuration) for how the ceiling interacts with
client negotiation, aspect ratios, and letterboxing.

## Advanced tuning

::: warning Escape hatches, not configuration
The defaults below are the tested, intended behavior. These variables exist so
a specific client or workload quirk can be worked around without rebuilding
the image. Reach for them only when a symptom points here.
:::

| Variable | Default | Meaning |
| --- | ---: | --- |
| `STEAM_REMOTE_ZERO_COPY` | `1` | Steers Steam onto the zero-copy BGRx DMA-BUF capture path (~3x less capture CPU). Set to `0` to revert to the always-safe NV12 shared-memory path |
| `STEAM_REMOTE_PHYS_MM` | `596x335` | Physical size (mm) advertised for the virtual display. Determines the DPI that Steam and Xwayland derive, and therefore host-side UI and cursor scale |
| `STEAM_REMOTE_CURSOR_CANVAS` | `96` | Side of the square bitmap every streamed cursor is placed on, in pixels |
| `STEAM_REMOTE_CURSOR_GLYPH` | `30` | Size the visible cursor glyph is resampled to within that canvas — effectively the cursor's apparent size on the client |

### About the zero-copy path

Withholding NV12 forces Steam onto a zero-copy BGRx DMA-BUF capture, which the
GPU converts to NV12 in place — roughly three times less CPU than the NV12
shared-memory path. The only observed failure is a transient VA surface
allocation during a **mid-session client resolution change**, which pinning
the client resolution avoids. If your client changes resolution mid-session
and the stream drops, set `STEAM_REMOTE_ZERO_COPY=0`.
See [Streaming pipeline](/internals/streaming#zero-copy-capture) for the
mechanism.

### About the cursor variables

Remote Play clients stretch every cursor bitmap they receive to a fixed
on-screen box, so a cursor's apparent size depends only on how much of its
bitmap the glyph fills. The image normalizes this by resampling every cursor's
glyph to `CURSOR_GLYPH` pixels on a `CURSOR_CANVAS`-sized square, keeping the
ratio — and therefore the size the client draws — constant. A larger glyph
value means a bigger cursor on the client. Both accept `1`–`512`, and the
glyph is clamped to the canvas.
See [Streaming pipeline](/internals/streaming#cursor-normalization).

## Environment set by the image

For completeness, the image itself sets `LANG`/`LC_ALL` to `en_US.UTF-8` and
`AMD_VULKAN_ICD=RADV` (the AMD Vulkan driver selection). These are part of the
image contract, not knobs.
