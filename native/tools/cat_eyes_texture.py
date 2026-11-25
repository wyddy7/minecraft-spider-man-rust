"""The cat eyes' atlas (native/hmod-src/assets/spider_man/textures/entity/cat_eyes.png).

Experiment. The eyes are drawn as textures, not built from moving boxes: every state is a frame,
painted at about 12 texels to a model pixel so a lid's curve and a pupil's glint are drawn shapes,
and frames are many enough (16 for the top lid) that switching them each frame reads as smooth
motion. One cell is one eye, 24 x 27 (the eye is 2 x 2.25 model pixels).

Row 0: the iris (cell 0: a rounded yellow shape, its rim darker, a ramp from a pale top to a
warmer bottom) and the pupil (cell 1, its top-left 16 x 16: a rounded dark shape, wine in its lower
part, a glint at its top left). Then, for each coat the eyes are tried on (the siamese, the
black-and-white), four rows in that coat's face colour: the top lid closing in 16 frames (two rows),
the bottom lid rising in 8, and a brow slanting down to the eye's inner (right) side in 8. Each lid
has a lighter rim along its edge so its curve reads on a dark face. UV places match catpack.rs.

Usage: uv run --with pillow python native/tools/cat_eyes_texture.py
"""

from pathlib import Path

from PIL import Image

OUT = Path(__file__).resolve().parents[1] / "hmod-src/assets/spider_man/textures/entity/cat_eyes.png"
W, H = 24, 27
COLS, ROWS = 10, 9
TOP_FRAMES, BOTTOM_FRAMES, BROW_FRAMES = 16, 8, 8

# The two coats' faces (sampled from the game's textures) and a lighter rim for each lid's edge.
COATS = [((49, 39, 32), (96, 78, 66)), ((28, 24, 39), (66, 60, 84))]
IRIS_TOP, IRIS_BOTTOM, IRIS_RIM = (250, 240, 128), (230, 196, 44), (176, 124, 12)
PUPIL, PUPIL_LOW, GLINT = (18, 14, 18), (66, 26, 36), (250, 250, 244)

img = Image.new("RGBA", (COLS * W, ROWS * H), (0, 0, 0, 0))
px = img.load()


def rounded(x: float, y: float, w: int, h: int, r: float) -> bool:
    """Whether the pixel (x, y) lies in a w x h rectangle with corners rounded by r."""
    cx, cy = x + 0.5, y + 0.5
    dx = max(r - cx, 0.0, cx - (w - r))
    dy = max(r - cy, 0.0, cy - (h - r))
    return dx * dx + dy * dy <= r * r


def mix(a, b, t):
    return tuple(round(p + (q - p) * t) for p, q in zip(a, b))


# The iris.
for y in range(H):
    for x in range(W):
        if not rounded(x, y, W, H, 7.0):
            continue
        inside = all(rounded(x + dx, y + dy, W, H, 7.0) for dx, dy in [(-1, 0), (1, 0), (0, -1), (0, 1)])
        colour = mix(IRIS_TOP, IRIS_BOTTOM, y / (H - 1)) if inside else IRIS_RIM
        px[x, y] = (*colour, 255)

# The pupil, 16 x 16 at cell 1.
for y in range(16):
    for x in range(16):
        if rounded(x, y, 16, 16, 6.0):
            px[W + x, y] = (*(PUPIL_LOW if y >= 10 else PUPIL), 255)
for x, y in [(3, 3), (4, 3), (3, 4), (4, 4), (5, 3), (3, 5)]:
    px[W + x, y] = (*GLINT, 255)


def paint(cell: int, row: int, face, rim, inside) -> None:
    """A lid frame: `inside(x, y)` is how far a pixel centre is inside the lid (> 0 covered)."""
    ox, oy = cell * W, row * H
    for y in range(H):
        for x in range(W):
            d = inside(x + 0.5, y + 0.5)
            if d > 0:
                px[ox + x, oy + y] = (*(rim if d <= 1.6 else face), 255)


def arc(x: float) -> float:
    """1 at the eye's middle, 0 at its sides."""
    return 1 - ((x - W / 2) / (W / 2)) ** 2


for coat, (face, rim) in enumerate(COATS):
    base = 1 + coat * 4
    for i in range(TOP_FRAMES):
        t = (i + 1) / TOP_FRAMES
        # The upper lid: an arc bulging down at the middle, past the bottom when shut.
        paint(i % COLS, base + i // COLS, face, rim, lambda x, y, t=t: t * (H + 4) - 4 + 4 * arc(x) - y)
    for i in range(BOTTOM_FRAMES):
        t = (i + 1) / BOTTOM_FRAMES
        paint(i, base + 2, face, rim, lambda x, y, t=t: y - (H - t * H * 0.6 - 3 * t * arc(x)))
    for i in range(BROW_FRAMES):
        t = (i + 1) / BROW_FRAMES
        # Down to the inner (right) side: high at the outer corner, past the middle at the inner.
        paint(i, base + 3, face, rim, lambda x, y, t=t: t * (2 + H * 0.62 * (x / W) ** 1.3) - y)

OUT.parent.mkdir(parents=True, exist_ok=True)
img.save(OUT)
print(OUT, img.size)
