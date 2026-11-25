"""The cat's eyes texture (native/hmod-src/assets/spider_man/textures/entity/cat_eyes.png), 8x16.

Colours measured on the reference footage (1080p, the eye at rest): the iris is a vertical ramp,
a dark olive band at the top under the lid, a yellow that turns warmer and more saturated downward,
and a dark amber band at the bottom. Columns 0-3 hold the ramp (the iris quad spans them, so each
row is a sixteenth of the eye's height); texel (4, 0) is the eye's sides, (5, 0) the pupil.
UV places match `eyes()` in native/hmod-src/catpack.rs.

Usage: uv run --with pillow python native/tools/cat_eyes_texture.py
"""

from pathlib import Path

from PIL import Image

OUT = Path(__file__).resolve().parents[1] / "hmod-src/assets/spider_man/textures/entity/cat_eyes.png"

TOP = [(98, 95, 26), (104, 101, 29), (140, 134, 46)]
BOTTOM = [(150, 126, 18), (110, 87, 0), (98, 80, 2)]
BODY_FROM, BODY_TO = (204, 196, 90), (199, 184, 44)
SIDE = (82, 68, 8)
PUPIL = (21, 14, 16)

body = 16 - len(TOP) - len(BOTTOM)
ramp = TOP + [
    tuple(round(a + (b - a) * i / (body - 1)) for a, b in zip(BODY_FROM, BODY_TO)) for i in range(body)
] + BOTTOM

img = Image.new("RGBA", (8, 16), (0, 0, 0, 0))
for y, colour in enumerate(ramp):
    for x in range(4):
        img.putpixel((x, y), (*colour, 255))
img.putpixel((4, 0), (*SIDE, 255))
img.putpixel((5, 0), (*PUPIL, 255))
img.save(OUT)
print(OUT)
