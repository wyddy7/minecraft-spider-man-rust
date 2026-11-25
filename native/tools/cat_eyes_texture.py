"""The cat's eyes texture (native/hmod-src/assets/spider_man/textures/entity/cat_eyes.png).

Experiment: big cartoon eyes over the game's cat head, each eye a 3x3x1 box with a pupil plane in
front of it. Painted by rule after docs/model-style.md: a short ramp that shifts hue, light from the
top left, every pixel placed on purpose. UV places match `eyes()` in native/hmod-src/catpack.rs:

- the eye box, 3x3x1 at (0, 0) in box-UV layout: the front is the iris, the other faces a dark rim;
- the round pupil, a 1x1 plane at (10, 0);
- the wide pupil, a 2x2 plane at (12, 0), with a glint in its top left;
- the slit pupil, a 1x3 plane at (16, 0).

Usage: uv run --with pillow python native/tools/cat_eyes_texture.py
"""

from pathlib import Path

from PIL import Image

OUT = Path(__file__).resolve().parents[1] / "hmod-src/assets/spider_man/textures/entity/cat_eyes.png"
W, H = 32, 16

# The iris, darkest to lightest: a cat's yellow-green, the shadow leaning green, the light warmer.
IRIS = [(172, 168, 40), (222, 206, 58), (244, 228, 96), (252, 244, 170)]
RIM = [(30, 28, 30), (44, 40, 40)]
PUPIL = (16, 16, 22)
GLINT = (236, 236, 228)

img = Image.new("RGBA", (W, H), (0, 0, 0, 0))
px = img.load()


def fill(x0: int, y0: int, w: int, h: int, colour: tuple[int, int, int]) -> None:
    for x in range(x0, x0 + w):
        for y in range(y0, y0 + h):
            px[x, y] = (*colour, 255)


# The eye box (w 3, h 3, d 1) at (0, 0): up (1,0) 3x1, down (4,0) 3x1, west (0,1) 1x3,
# north = front (1,1) 3x3, east (4,1) 1x3, south (5,1) 3x3.
fill(1, 0, 3, 1, RIM[1])
fill(4, 0, 3, 1, RIM[0])
fill(0, 1, 1, 3, RIM[1])
fill(4, 1, 1, 3, RIM[0])
fill(5, 1, 3, 3, RIM[0])
# The iris: lit from the top left, a shadow along the bottom and the right.
front = [
    [3, 2, 2],
    [2, 2, 2],
    [1, 1, 1],
]
for row, shades in enumerate(front):
    for col, shade in enumerate(shades):
        px[1 + col, 1 + row] = (*IRIS[shade], 255)

fill(10, 0, 1, 1, PUPIL)
fill(12, 0, 2, 2, PUPIL)
px[12, 0] = (*GLINT, 255)
fill(16, 0, 1, 3, PUPIL)

OUT.parent.mkdir(parents=True, exist_ok=True)
img.save(OUT)
print(OUT)
