/*
 * Normalises the size of every cursor Steam Remote Play streams.
 *
 * The macOS Remote Play client stretches each cursor bitmap it receives to a
 * fixed on-screen box, ignoring the bitmap's size, the host's UI scale and the
 * stream resolution. A cursor's apparent size therefore depends only on how
 * much of its bitmap the visible glyph fills, which makes applications that
 * ship their own cursors (rather than using the Xcursor theme) appear
 * enormous.
 *
 * Steam reads every cursor through XFixesGetCursorImage, whatever set it, so
 * rescaling there covers all of them uniformly: the glyph is resampled to
 * STEAM_REMOTE_CURSOR_GLYPH pixels and placed at the origin of a
 * STEAM_REMOTE_CURSOR_CANVAS square, leaving the ratio - and so the size the
 * client draws - constant.
 *
 * Steam's launcher clears LD_PRELOAD, so this is installed through
 * /etc/ld.so.preload instead. That applies container-wide, hence the shim stays
 * inert unless STEAM_REMOTE_CURSOR_GLYPH names a size.
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
	unsigned short x, y;
	unsigned short width, height;
	unsigned short xhot, yhot;
	unsigned long cursor_serial;
	unsigned long *pixels;
	unsigned long atom;
	const char *name;
} CursorImage;

static CursorImage *(*next_get_cursor_image)(void *display);
static unsigned int canvas_size = 96;
static unsigned int glyph_size;

static unsigned int env_size(const char *name, unsigned int fallback)
{
	const char *value = getenv(name);
	char *end = NULL;
	unsigned long parsed;

	if (value == NULL || *value == '\0')
		return fallback;
	parsed = strtoul(value, &end, 10);
	if (end == NULL || *end != '\0' || parsed == 0 || parsed > 512)
		return fallback;
	return (unsigned int)parsed;
}

__attribute__((constructor)) static void shim_init(void)
{
	canvas_size = env_size("STEAM_REMOTE_CURSOR_CANVAS", canvas_size);
	glyph_size = env_size("STEAM_REMOTE_CURSOR_GLYPH", 0);
	if (glyph_size > canvas_size)
		glyph_size = canvas_size;
}

/* Bounding box of the visible glyph, so padding in the source is ignored. */
static int glyph_bounds(const CursorImage *image, unsigned int *out_w, unsigned int *out_h)
{
	unsigned int min_x = image->width, max_x = 0;
	unsigned int min_y = image->height, max_y = 0;
	unsigned int x, y;

	for (y = 0; y < image->height; y++) {
		for (x = 0; x < image->width; x++) {
			if ((image->pixels[(size_t)y * image->width + x] >> 24) & 0xff) {
				if (x < min_x) min_x = x;
				if (x > max_x) max_x = x;
				if (y < min_y) min_y = y;
				if (y > max_y) max_y = y;
			}
		}
	}
	if (max_x < min_x || max_y < min_y)
		return 0;
	*out_w = max_x - min_x + 1;
	*out_h = max_y - min_y + 1;
	return 1;
}

/* Box-filter resample; cursor pixels are premultiplied, so plain averaging is correct. */
static void resample(const CursorImage *src, unsigned long *dst, unsigned int dst_w,
		     unsigned int dst_h, unsigned int stride)
{
	unsigned int dx, dy;

	for (dy = 0; dy < dst_h; dy++) {
		unsigned int y0 = (unsigned int)((size_t)dy * src->height / dst_h);
		unsigned int y1 = (unsigned int)((size_t)(dy + 1) * src->height / dst_h);
		if (y1 <= y0) y1 = y0 + 1;
		if (y1 > src->height) y1 = src->height;

		for (dx = 0; dx < dst_w; dx++) {
			unsigned int x0 = (unsigned int)((size_t)dx * src->width / dst_w);
			unsigned int x1 = (unsigned int)((size_t)(dx + 1) * src->width / dst_w);
			unsigned long acc[4] = { 0, 0, 0, 0 };
			unsigned int count = 0, x, y, c;

			if (x1 <= x0) x1 = x0 + 1;
			if (x1 > src->width) x1 = src->width;

			for (y = y0; y < y1; y++) {
				for (x = x0; x < x1; x++) {
					unsigned long p = src->pixels[(size_t)y * src->width + x];
					for (c = 0; c < 4; c++)
						acc[c] += (p >> (c * 8)) & 0xff;
					count++;
				}
			}
			if (count == 0)
				continue;
			dst[(size_t)dy * stride + dx] =
				((acc[0] / count) & 0xff) |
				(((acc[1] / count) & 0xff) << 8) |
				(((acc[2] / count) & 0xff) << 16) |
				(((acc[3] / count) & 0xff) << 24);
		}
	}
}

CursorImage *XFixesGetCursorImage(void *display)
{
	CursorImage *src, *out;
	unsigned int bounds_w, bounds_h, longest, dst_w, dst_h, name_len;
	unsigned char *block;
	double scale;

	if (next_get_cursor_image == NULL) {
		next_get_cursor_image = dlsym(RTLD_NEXT, "XFixesGetCursorImage");
		if (next_get_cursor_image == NULL)
			return NULL;
	}

	src = next_get_cursor_image(display);
	if (src == NULL || src->pixels == NULL || src->width == 0 || src->height == 0)
		return src;
	if (glyph_size == 0 || !glyph_bounds(src, &bounds_w, &bounds_h))
		return src;

	longest = bounds_w > bounds_h ? bounds_w : bounds_h;
	if (longest <= glyph_size)
		return src; /* already small enough; never upscale */

	scale = (double)glyph_size / (double)longest;
	dst_w = (unsigned int)(src->width * scale + 0.5);
	dst_h = (unsigned int)(src->height * scale + 0.5);
	if (dst_w == 0) dst_w = 1;
	if (dst_h == 0) dst_h = 1;
	if (dst_w > canvas_size) dst_w = canvas_size;
	if (dst_h > canvas_size) dst_h = canvas_size;

	name_len = src->name != NULL ? (unsigned int)strlen(src->name) : 0;
	block = calloc(1, sizeof(CursorImage) +
			  (size_t)canvas_size * canvas_size * sizeof(unsigned long) +
			  name_len + 1);
	if (block == NULL)
		return src;

	out = (CursorImage *)block;
	out->pixels = (unsigned long *)(block + sizeof(CursorImage));
	out->x = src->x;
	out->y = src->y;
	out->width = (unsigned short)canvas_size;
	out->height = (unsigned short)canvas_size;
	out->xhot = (unsigned short)(src->xhot * scale + 0.5);
	out->yhot = (unsigned short)(src->yhot * scale + 0.5);
	if (out->xhot >= canvas_size) out->xhot = (unsigned short)(canvas_size - 1);
	if (out->yhot >= canvas_size) out->yhot = (unsigned short)(canvas_size - 1);
	out->cursor_serial = src->cursor_serial;
	out->atom = src->atom;
	if (src->name != NULL) {
		char *name = (char *)out->pixels + (size_t)canvas_size * canvas_size * sizeof(unsigned long);
		memcpy(name, src->name, name_len + 1);
		out->name = name;
	}

	resample(src, out->pixels, dst_w, dst_h, canvas_size);
	free(src);
	return out;
}
