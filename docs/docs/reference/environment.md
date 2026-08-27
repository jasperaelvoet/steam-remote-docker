# Environment variables

## Display configuration

These three variables are the supported configuration surface. They set the
maximum Gamescope render size and refresh rate; Steam negotiates the actual
capture size and frame rate with the client within this ceiling.

| Variable | Default | Meaning |
| --- | ---: | --- |
| `STEAM_REMOTE_WIDTH` | `3840` | Maximum render width in pixels |
| `STEAM_REMOTE_HEIGHT` | `2160` | Maximum render height in pixels |
| `STEAM_REMOTE_FPS` | `60` | Maximum refresh rate; also the render cap applied to Proton games and the rate restored when the session leaves the parked state |

See [Configuration](../configuration.md) for how the ceiling interacts with
client negotiation, aspect ratios, and letterboxing.

## Updates

| Variable | Default | Meaning |
| --- | ---: | --- |
| `STEAM_REMOTE_AUTO_UPDATE` | `1` | Apply Steam client updates while parked (at most one session restart per day) and let `steam-remote update-gate` approve container image updates. `0` disables both |

See [Automatic updates](../auto-updates.md) for when exactly updates happen
and why they can never interrupt anything.

## Advanced tuning

:::warning[Escape hatches, not configuration]
The defaults below are the tested, intended behavior. These variables exist
so a specific client or workload quirk can be worked around without
rebuilding the image. Reach for them only when a symptom points here.
:::

| Variable | Default | Meaning |
| --- | ---: | --- |
| `STEAM_REMOTE_ZERO_COPY` | `1` | Shares GPU-converted NV12 frames with the encoder as DMA-BUFs — no CPU copies in the capture path. Set to `0` to revert to the always-safe NV12 shared-memory path |
| `STEAM_REMOTE_PHYS_MM` | `596x335` | Physical size (mm) advertised for the virtual display. Determines the DPI that Steam and Xwayland derive, and therefore host-side UI and cursor scale |
| `STEAM_REMOTE_CURSOR_CANVAS` | `96` | Side of the square bitmap every streamed cursor is placed on, in pixels |
| `STEAM_REMOTE_CURSOR_GLYPH` | `30` | Size the visible cursor glyph is resampled to within that canvas — effectively the cursor's apparent size on the client |

### About the zero-copy path

Gamescope converts each frame to NV12 on the GPU and hands Steam that exact
buffer as a DMA-BUF; the hardware encoder imports it directly. If streaming
misbehaves in a way that points at capture (broken image, stalls, very low
frame rate), `STEAM_REMOTE_ZERO_COPY=0` is the clean way to take the whole
DMA-BUF path out of the equation. See
[Streaming pipeline](../internals/streaming.md#zero-copy-capture) for the
mechanism.

### About the cursor variables

Remote Play clients stretch every cursor bitmap they receive to a fixed
on-screen box, so a cursor's apparent size depends only on how much of its
bitmap the glyph fills. The image normalizes this by resampling every
cursor's glyph to `CURSOR_GLYPH` pixels on a `CURSOR_CANVAS`-sized square,
keeping the ratio — and therefore the size the client draws — constant. A
larger glyph value means a bigger cursor on the client. Both accept `1`–`512`,
and the glyph is clamped to the canvas. See
[Streaming pipeline](../internals/streaming.md#cursor-normalization).

## Environment set by the image

For completeness, the image itself sets `LANG`/`LC_ALL` to `en_US.UTF-8` and
`AMD_VULKAN_ICD=RADV` (the AMD Vulkan driver selection). These are part of
the image contract, not knobs.
