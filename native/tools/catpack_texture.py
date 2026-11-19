"""The cat backpack's texture (native/hmod-src/assets/spider_man/textures/entity/catpack.png).

Painted by rule, not by brush, after the Minecraft style guide (docs/model-style.md):
a few shades per material on a ramp that shifts hue (cool shadows, warmer highlights), light from
the top left, top and front brighter than bottom and back, no noise, deliberate detail pixels
(stitches, a zip, a buckle). The boxes and their UV places are those of `bag()` in
native/hmod-src/catpack.rs: each box is painted in vanilla's box-UV layout.

Usage: uv run --with pillow python native/tools/catpack_texture.py
"""

from pathlib import Path

from PIL import Image

OUT = Path(__file__).resolve().parents[1] / "hmod-src/assets/spider_man/textures/entity/catpack.png"
W, H = 64, 32

# Black canvas, as Miles's bag (Hot Toys' figure lists a black backpack). Darkest to lightest:
# the shadow leans blue, the highlight leans warm grey.
BAG = [(22, 23, 31), (34, 35, 44), (48, 48, 56), (64, 63, 70), (84, 82, 86)]
RIM = [(28, 29, 38), (42, 43, 52), (58, 58, 66), (76, 75, 82), (98, 96, 100)]
STRAP = [(16, 17, 23), (26, 27, 34), (38, 38, 46), (52, 52, 58), (66, 65, 70)]
INSIDE = (10, 10, 14)
STITCH = (104, 102, 106)
ZIP = (132, 128, 120)
METAL = [(96, 98, 108), (150, 152, 160), (205, 206, 210)]

img = Image.new("RGBA", (W, H), (0, 0, 0, 0))
px = img.load()


def fill(x, y, w, h, c):
    for j in range(y, y + h):
        for i in range(x, x + w):
            if 0 <= i < W and 0 <= j < H:
                px[i, j] = c + (255,)


def face(x, y, w, h, ramp, base):
    """A face lit from the top left: its base shade, a highlight on the top row and left
    column, a shadow on the bottom row and right column."""
    if w <= 0 or h <= 0:
        return
    fill(x, y, w, h, ramp[base])
    if h > 1:
        fill(x, y + h - 1, w, 1, ramp[max(base - 1, 0)])
        fill(x, y, w, 1, ramp[min(base + 1, len(ramp) - 1)])
    if w > 1:
        fill(x + w - 1, y, 1, h, ramp[max(base - 1, 0)])
        fill(x, y, 1, h, ramp[min(base + 1, len(ramp) - 1)])


def box(u, v, w, h, d, ramp):
    """Vanilla box UV at (u, v) for a box w×h×d: top and bottom on the first row, then east,
    north, west, south. Top brightest, then the faces, bottom darkest."""
    w, h, d = int(w), int(h), int(d)
    face(u + d, v, w, d, ramp, 3)  # top
    face(u + d + w, v, w, d, ramp, 0)  # bottom
    face(u, v + d, d, h, ramp, 1)  # east
    face(u + d, v + d, w, h, ramp, 2)  # north
    face(u + d + w, v + d, d, h, ramp, 1)  # west
    face(u + d + w + d, v + d, w, h, ramp, 2)  # south
    return (u + d, v + d, u + 2 * d + w, v + d)  # north and south faces' top-left corners


# The bag's body: 7×7×5. Its south face is the side seen from behind the player.
_, _, sx, sy = box(0, 0, 7, 7, 5, BAG)
for i in range(sx + 1, sx + 6, 2):  # stitches along the top and the bottom of the back
    px[i, sy + 1] = STITCH + (255,)
    px[i, sy + 5] = STITCH + (255,)
# Rims: 7×2×1 front and back, 1×2×3 sides. A zip line along the top of the rim.
nx, ny, rx, ry = box(0, 16, 7, 2, 1, RIM)
for i in range(rx, rx + 7):
    px[i, ry] = ZIP + (255,) if i % 2 == 0 else RIM[4] + (255,)
for i in range(nx, nx + 7):
    px[i, ny] = ZIP + (255,) if i % 2 == 0 else RIM[4] + (255,)
box(20, 16, 1, 2, 3, RIM)
fill(32, 16, 16, 4, INSIDE)  # the inside, seen from above
# Pocket 5×3×1: a stitched edge and a flap shadow under its top row.
_, _, px0, py0 = box(0, 24, 5, 3, 1, BAG)
for i in range(px0, px0 + 5):
    px[i, py0 + 1] = BAG[0] + (255,)
px[px0 + 2, py0 + 2] = STITCH + (255,)
# Buckle: a metal pixel or two.
fill(16, 24, 4, 2, METAL[1])
px[17, 24] = METAL[2] + (255,)
px[18, 25] = METAL[0] + (255,)
# Straps: thin black webbing.
box(32, 24, 1, 1, 5, STRAP)
box(48, 24, 1, 7, 1, STRAP)

OUT.parent.mkdir(parents=True, exist_ok=True)
img.save(OUT)
print(OUT)
