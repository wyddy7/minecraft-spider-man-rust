"""The cat's eyes texture (native/hmod-src/assets/spider_man/textures/entity/cat_eyes.png), 8x16.

Colours measured on the reference footage (1080p, the eye at rest): the iris is a vertical ramp,
a dark olive band at the top under the lid, a yellow that turns warmer and more saturated downward,
and a dark amber band at the bottom. Columns 0-3 hold the ramp (the iris quad spans them, so each
row is a sixteenth of the eye's height); texel (4, 0) is the eye's sides, (5, 0) and (5, 1) the pupil (black above, dark wine below).
UV places match `eyes()` in native/hmod-src/catpack.rs.

Usage: uv run --with pillow python native/tools/cat_eyes_texture.py
"""

from pathlib import Path

from PIL import Image

OUT = Path(__file__).resolve().parents[1] / "hmod-src/assets/spider_man/textures/entity/cat_eyes.png"

# No dark band at the top: it read as a heavy lid (a tired, stoned look). Brighter than the
# footage's sampled values, as the game's entity shading darkens a face that looks up and away.
TOP = [(248, 238, 120)]
BOTTOM = [(214, 172, 24), (168, 128, 8)]
BODY_FROM, BODY_TO = (244, 230, 100), (228, 200, 46)
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
img.putpixel((5, 1), (58, 24, 32, 255))
img.save(OUT)
print(OUT)
