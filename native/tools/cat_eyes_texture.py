"""The cat eyes' palette (native/hmod-src/assets/spider_man/textures/entity/cat_eyes.png), 8 x 11.

Experiment. The eyes are built from flat shapes with straight edges, each face one flat colour,
so they stay crisp at any distance. This texture holds only the colours: one row per cat coat, in
`catpack::VARIANTS`' order, and in each row (`catpack::Slot`):

0 the iris, 1 its lower band, 2 the pupil, 3 the pupil's lower part, 4 the face (the lids),
5 the second eye's iris and 6 its band (the same as 0 and 1 but for the white cat's odd eyes).

The iris and pupil are matched to each coat (green on the tabby and the ginger, blue on the
siamese and the ragdoll, copper on the greys and the persian, odd eyes on the white); the face is
the commonest colour of the front of the game's own head texture of that coat.

Usage: uv run --with pillow python native/tools/cat_eyes_texture.py
"""

from pathlib import Path

from PIL import Image

OUT = Path(__file__).resolve().parents[1] / "hmod-src/assets/spider_man/textures/entity/cat_eyes.png"

GREEN = (150, 206, 72)
YELLOW = (240, 222, 78)
BLUE = (96, 176, 236)
COPPER = (232, 150, 52)
AMBER = (236, 184, 60)
GOLD = (246, 206, 70)
LIME = (206, 220, 84)
DARK, WINE, NAVY, BROWN = (20, 15, 18), (62, 26, 34), (20, 26, 52), (40, 26, 18)

# (iris, pupil, pupil's lower part, face, second iris)
COATS = {
    "tabby": (GREEN, DARK, BROWN, (209, 183, 161), None),
    "black": (YELLOW, DARK, WINE, (28, 24, 39), None),
    "red": (GREEN, DARK, BROWN, (234, 169, 57), None),
    "siamese": (BLUE, DARK, NAVY, (85, 68, 59), None),
    "british_shorthair": (COPPER, DARK, BROWN, (188, 188, 188), None),
    "calico": (AMBER, DARK, BROWN, (220, 222, 222), None),
    "persian": (COPPER, DARK, BROWN, (255, 224, 178), None),
    "ragdoll": (BLUE, DARK, NAVY, (255, 255, 255), None),
    "white": (BLUE, DARK, NAVY, (253, 249, 251), YELLOW),
    "jellie": (LIME, DARK, BROWN, (253, 249, 251), None),
    "all_black": (GOLD, DARK, WINE, (22, 21, 36), None),
}


def band(colour):
    """The iris's foot: the iris's own colour (a darker foot read as a stripe under the eye)."""
    return colour


img = Image.new("RGBA", (8, len(COATS)), (0, 0, 0, 0))
for row, (iris, pupil, low, face, second) in enumerate(COATS.values()):
    second = second or iris
    for x, colour in enumerate([iris, band(iris), pupil, low, face, second, band(second)]):
        img.putpixel((x, row), (*colour, 255))
img.save(OUT)
print(OUT, img.size)
