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
# The lining: a grey mesh, lighter than the canvas so the open mouth reads. The zip: Miles's red.
LINING = [(58, 60, 72), (74, 76, 90), (92, 94, 108)]
TRIM = [(122, 24, 28), (176, 40, 38)]
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


def box(u, v, w, h, d, ramp, inside=()):
    """Vanilla box UV at (u, v) for a box w×h×d in the body's frame (+y down, +z away from the
    back): the top and the bottom on the first row, then west, north (towards the player), east,
    south (seen from behind). Top brightest, bottom darkest. Faces named in `inside` are the bag's
    lining. Returns the top-left corners of the north and the south face."""
    w, h, d = int(w), int(h), int(d)
    faces = {
        "top": (u + d, v, w, d, 3),
        "bottom": (u + d + w, v, w, d, 0),
        "west": (u, v + d, d, h, 1),
        "north": (u + d, v + d, w, h, 2),
        "east": (u + d + w, v + d, d, h, 1),
        "south": (u + d + w + d, v + d, w, h, 2),
    }
    for name, (x, y, fw, fh, base) in faces.items():
        if name in inside:
            face(x, y, fw, fh, LINING, 1)
        else:
            face(x, y, fw, fh, ramp, base)
    return (u + d, v + d, u + 2 * d + w, v + d)


def dot(x, y, c):
    px[x, y] = c + (255,)


# The trunk, 8×8×5. Its south face is seen from behind: a webbing strap down the middle, a zip
# across the top. Its top is the floor of the well the cat sits in: lining, with a canvas border.
tnx, tny, tsx, tsy = box(0, 0, 8, 8, 5, BAG)
fill(5 + 1, 0 + 1, 6, 4, LINING[0])
fill(tsx + 3, tsy, 2, 8, STRAP[2])
fill(tsx + 3, tsy, 1, 8, STRAP[3])
for i in range(tsx, tsx + 8):
    if i not in (tsx + 3, tsx + 4):
        dot(i, tsy + 1, STITCH if i % 2 == 0 else BAG[1])
# The top section's back, 6×3×1: lining towards the cat.
box(26, 0, 6, 3, 1, BAG, inside=("south",))
# Its lip, 6×1×1: the red zip along the top.
box(26, 4, 6, 1, 1, RIM, inside=("north",))
for i in range(27, 27 + 6):
    dot(i, 4, TRIM[0])
# The flap, 6×2×1, unzipped and folded forward: what shows is its lining (south), edged by the
# red zip; its canvas side (north) lies towards the trunk.
_, _, fx, fy = box(26, 6, 6, 2, 1, BAG, inside=("south",))
for i in range(fx, fx + 6):
    dot(i, fy + 1, TRIM[0])
# A cheek of the top section, 1×3×4: canvas outside (its west face; the other is mirrored),
# lining inside, the zip along its top.
_, cy, _, _ = box(40, 0, 1, 3, 4, BAG, inside=("east",))
for i in range(40, 44):
    dot(i, cy, TRIM[0])
# The front pocket, 6×5×2: a zip line under its top with a red pull, cord across the front.
_, _, qx, qy = box(26, 9, 6, 5, 2, RIM)
for i in range(qx, qx + 6):
    dot(i, qy + 1, BAG[0])
dot(qx + 4, qy + 1, TRIM[1])
for k in range(4):
    dot(qx + 1 + k, qy + 3, STITCH)
# A side pocket, 1×4×3: mesh, an elastic edge on top.
_, sy2, _, _ = box(50, 0, 1, 4, 3, RIM)
for i in range(50, 53):
    dot(i, sy2, STRAP[4])
    dot(i, sy2 + 2, BAG[1])
# The grab handle, 2×1×1.
box(58, 0, 2, 1, 1, STRAP)
# Straps: thin black webbing.
box(32, 24, 1, 1, 5, STRAP)
box(48, 24, 1, 7, 1, STRAP)

OUT.parent.mkdir(parents=True, exist_ok=True)
img.save(OUT)
print(OUT)
