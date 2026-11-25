"""The cat eyes' palette (native/hmod-src/assets/spider_man/textures/entity/cat_eyes.png), 8 x 1.

Experiment. The eyes are built from flat shapes with straight edges (rectangles and four-cornered
lids whose corners move), each face a single flat colour, so they stay crisp at any distance: no
curve is ever rasterised into a staircase. This texture is only the colours, one texel each, in
the order `catpack.rs` names them (`PALETTE`):

0 iris, 1 the iris's lower band, 2 pupil, 3 pupil's lower part, 4 the siamese face, 5 the
black-and-white face. Face colours are sampled from the game's own cat textures.

Usage: uv run --with pillow python native/tools/cat_eyes_texture.py
"""

from pathlib import Path

from PIL import Image

OUT = Path(__file__).resolve().parents[1] / "hmod-src/assets/spider_man/textures/entity/cat_eyes.png"

COLOURS = [
    (240, 222, 78),
    (196, 150, 22),
    (20, 15, 18),
    (62, 26, 34),
    (49, 39, 32),
    (28, 24, 39),
]

img = Image.new("RGBA", (8, 1), (0, 0, 0, 0))
for x, colour in enumerate(COLOURS):
    img.putpixel((x, 0), (*colour, 255))
img.save(OUT)
print(OUT)
