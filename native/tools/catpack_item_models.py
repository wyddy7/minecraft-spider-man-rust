"""The cat backpack as an item: 3D item models for the inventory, the hand and the ground.

Writes, under native/hmod-src/assets/:
  minecraft/atlases/items.json            the bag's texture and the game's cat textures join the
                                          item atlas, so item models can be painted with them
  spider_man/models/item/catpack*.json    the empty bag (lid shut), the bag with a grown cat, the
                                          bag with a kitten; one child model per cat variant
  spider_man/items/catpack*.json          the item definitions a stack names in `minecraft:item_model`

The boxes, their places on the textures and the cat's pose are those of `bag()`, `cat()`,
`kitten()` and the poses in native/hmod-src/catpack.rs: keep the two in step. An item model has
no springs: the cat is held in its resting pose, its turns rounded to what one axis can say.

Frames. The Rust model is the entity frame (+y down, the back towards +z, texels). An item model
is +y up in a 16-texel cell; the bag's front faces south (+z), as flat items do. A point (x, y, z)
of the bag lands at (8 - x, 13 - y, 2.5 + z): a half turn about z, so nothing is mirrored.

Usage: python3 native/tools/catpack_item_models.py
"""

import json
from pathlib import Path

ASSETS = Path(__file__).resolve().parents[1] / "hmod-src/assets"
VARIANTS = [
    "tabby", "black", "red", "siamese", "british_shorthair", "calico", "persian", "ragdoll",
    "white", "jellie", "all_black",
]
BAG_TEX = [64, 32]


def cell(p):
    """A bag-frame point in the item model's cell."""
    return [8 - p[0], 13 - p[1], 2.5 + p[2]]


def faces(u, v, w, h, d, tex, turned=False, mirrored=False):
    """Box UV at (u, v) for a w×h×d box, as item-model faces. `turned`: the box belongs to the
    cat, which stands half round about y in the bag (its north face looks south)."""
    kx, ky = 16 / tex[0], 16 / tex[1]
    u1, u2, u3, u4, u5 = u + d, u + d + w, u + d + w + w, u + d + w + d, u + d + w + d + w
    v1, v2 = v + d, v + d + h
    # The entity box's faces by its own names.
    box = {
        "top": [u1, v, u2, v1], "bottom": [u2, v1, u3, v],
        "west": [u, v1, u1, v2], "north": [u1, v1, u2, v2],
        "east": [u2, v1, u4, v2], "south": [u4, v1, u5, v2],
    }
    if mirrored:
        box["west"], box["east"] = box["east"], box["west"]
        for name in box:
            a = box[name]
            box[name] = [a[2], a[1], a[0], a[3]]
    # Which item face shows which of them: the half turn about z swaps west and east; the cat's
    # own half turn about y swaps them back and swaps north and south.
    if turned:
        show = {"up": "top", "down": "bottom", "north": "south", "south": "north", "west": "west", "east": "east"}
        caps_reversed = False
    else:
        show = {"up": "top", "down": "bottom", "north": "north", "south": "south", "west": "east", "east": "west"}
        caps_reversed = True
    out = {}
    for item_face, name in show.items():
        a = list(box[name])
        if item_face in ("up", "down") and caps_reversed:
            a = [a[2], a[3], a[0], a[1]]
        out[item_face] = {"uv": [a[0] * kx, a[1] * ky, a[2] * kx, a[3] * ky], "texture": "#T"}
    return out


def element(origin, size, f, texture, rotation=None):
    a = cell(origin)
    b = cell([origin[0] + size[0], origin[1] + size[1], origin[2] + size[2]])
    lo = [min(a[i], b[i]) for i in range(3)]
    hi = [max(a[i], b[i]) for i in range(3)]
    for face in f.values():
        face["texture"] = texture
    e = {"from": [round(c, 4) for c in lo], "to": [round(c, 4) for c in hi], "faces": f}
    if rotation:
        pivot, axis, angle = rotation
        e["rotation"] = {"origin": [round(c, 4) for c in cell(pivot)], "axis": axis, "angle": angle}
    return e


def bag_box(u, v, origin, size, mirrored=False, rotation=None):
    return element(origin, size, faces(u, v, size[0], size[1], size[2], BAG_TEX, mirrored=mirrored), "#bag", rotation)


def cat_box(tex, u, v, origin, size, at, scale, rotation=None, f=None):
    """A box of the cat's own frame (looking along -z), drawn at `at` of the bag at `scale`."""
    x0 = at[0] - scale * (origin[0] + size[0])
    z0 = at[2] - scale * (origin[2] + size[2])
    place = [x0, at[1] + scale * origin[1], z0]
    extent = [scale * size[0], scale * size[1], scale * size[2]]
    f = f or faces(u, v, size[0], size[1], size[2], tex, turned=True)
    return element(place, extent, f, "#cat", rotation)


def shell():
    return [
        bag_box(0, 0, [-4, 4, 2], [8, 8, 5]),
        bag_box(26, 0, [-3, 1, 2], [6, 3, 1]),
        bag_box(26, 4, [-3, 1, 6], [6, 3, 1]),
        bag_box(40, 0, [-3, 1, 3], [1, 3, 4]),
        bag_box(40, 0, [2, 1, 3], [1, 3, 4], mirrored=True),
        bag_box(0, 13, [-3, 7, 7], [6, 5, 2]),
        bag_box(50, 0, [-5, 7, 3], [1, 4, 3]),
        bag_box(50, 0, [4, 7, 3], [1, 4, 3], mirrored=True),
        bag_box(58, 0, [-1, 0, 2], [2, 1, 1]),
    ]


def lid_faces(outside, lining):
    """The lid's six faces: `outside` and `lining` name the two faces that show them; the edges
    take a strip of the outside."""
    kx, ky = 16 / BAG_TEX[0], 16 / BAG_TEX[1]
    canvas = [31 * kx, 8 * ky, 37 * kx, 13 * ky]
    inner = [37 * kx, 8 * ky, 43 * kx, 13 * ky]
    edge = [31 * kx, 8 * ky, 37 * kx, 9 * ky]
    f = {name: {"uv": edge} for name in ("up", "down", "north", "south", "west", "east")}
    f[outside] = {"uv": canvas}
    f[lining] = {"uv": inner}
    return f


def lid_shut():
    """The lid lying over the well."""
    return element([-3, 0, 2], [6, 1, 5], lid_faces("up", "down"), "#bag")


def lid_open():
    """The lid standing up behind the cat, lining towards the front."""
    return element([-3, -4, 1], [6, 5, 1], lid_faces("north", "south"), "#bag")


def grown_cat():
    tex, s = [64, 32], 0.6
    head_at, turn = [0, -0.5, 5.8], ([0, -0.5, 5.8], "y", 22.5)
    paw = {"uv": [42 * 16 / 64, 9 * 16 / 32, 44 * 16 / 64, 12 * 16 / 32]}
    tip = {"uv": [44 * 16 / 64, 0, 46 * 16 / 64, 1 * 16 / 32]}
    edge = {"uv": [41 * 16 / 64, 9 * 16 / 32, 42 * 16 / 64, 12 * 16 / 32], "rotation": 90}

    def paw_at(x, y, z):
        f = {"up": dict(paw), "down": dict(paw), "south": dict(tip), "north": dict(tip), "west": dict(edge), "east": dict(edge)}
        return element([x - 0.6, y, z], [1.2, 0.6, 1.8], f, "#cat", ([x, y, z], "x", 22.5))

    return [
        cat_box(tex, 23, 3, [-2, -5, -1.5], [4, 5, 3], [0, 4, 5.0], s),
        cat_box(tex, 0, 0, [-2.5, -2, -2.5], [5, 4, 5], head_at, s, turn),
        cat_box(tex, 0, 24, [-1.5, 0, -3.5], [3, 2, 2], head_at, s, turn),
        cat_box(tex, 0, 10, [-2, -3, 0.5], [1, 1, 2], head_at, s, turn),
        cat_box(tex, 6, 10, [1, -3, 0.5], [1, 1, 2], head_at, s, turn),
        paw_at(-0.8, 0.5, 6.3),
        paw_at(0.8, 0.4, 6.2),
    ]


def kitten():
    tex, s = [32, 32], 0.5
    head_at, turn = [0, 0.2, 5.7], ([0, 0.2, 5.7], "y", 22.5)
    k = 16 / 32

    def r(a, b, c, d):
        return {"uv": [a * k, b * k, c * k, d * k]}

    # The trunk's faces, cut from the front end of the texture's lying body (as `kitten()` lays them).
    body = {
        "south": r(11, 15, 15, 11), "north": r(7, 15, 11, 11),
        "west": {**r(3, 15, 7, 18), "rotation": 90}, "east": {**r(11, 15, 15, 18), "rotation": 270},
        "up": r(7, 15, 11, 18), "down": r(11, 8, 15, 11),
    }
    return [
        cat_box(tex, 0, 8, [-2, -4, -1.5], [4, 4, 3], [0, 3, 5.0], s, f=body),
        cat_box(tex, 0, 0, [-2.5, -2, -2], [5, 4, 4], head_at, s, turn),
        cat_box(tex, 18, 3, [-1.5, 0, -3], [3, 2, 1], head_at, s, turn),
        cat_box(tex, 18, 0, [-2, -3, 0], [1, 1, 2], head_at, s, turn),
        cat_box(tex, 24, 0, [1, -3, 0], [1, 1, 2], head_at, s, turn),
    ]


# How the item sits in each place it is drawn. The bag's front faces south.
DISPLAY = {
    "gui": {"rotation": [18, 28, 0], "translation": [0, 0.25, 0], "scale": [0.92, 0.92, 0.92]},
    "ground": {"rotation": [0, 0, 0], "translation": [0, 2, 0], "scale": [0.5, 0.5, 0.5]},
    "fixed": {"rotation": [0, 180, 0], "translation": [0, 0, 0], "scale": [0.9, 0.9, 0.9]},
    "head": {"rotation": [0, 180, 0], "translation": [0, 8, 0], "scale": [1, 1, 1]},
    "thirdperson_righthand": {"rotation": [75, 0, 0], "translation": [0, 2.5, 1], "scale": [0.5, 0.5, 0.5]},
    "thirdperson_lefthand": {"rotation": [75, 0, 0], "translation": [0, 2.5, 1], "scale": [0.5, 0.5, 0.5]},
    "firstperson_righthand": {"rotation": [0, -30, 0], "translation": [1, 2, -1], "scale": [0.6, 0.6, 0.6]},
    "firstperson_lefthand": {"rotation": [0, 30, 0], "translation": [1, 2, -1], "scale": [0.6, 0.6, 0.6]},
}


def write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=1) + "\n")


def main():
    bag = "spider_man:entity/catpack"
    write(ASSETS / "minecraft/atlases/items.json", {"sources": [
        {"type": "minecraft:single", "resource": bag},
        {"type": "minecraft:directory", "source": "entity/cat", "prefix": "entity/cat/"},
    ]})
    models = ASSETS / "spider_man/models/item"
    items = ASSETS / "spider_man/items"
    write(models / "catpack.json", {
        "textures": {"bag": bag, "particle": bag}, "display": DISPLAY, "elements": shell() + [lid_shut()],
    })
    write(items / "catpack.json", {"model": {"type": "minecraft:model", "model": "spider_man:item/catpack"}})
    for kind, parts, suffix in [("cat", grown_cat(), ""), ("kitten", kitten(), "_baby")]:
        write(models / f"catpack_{kind}.json", {
            "textures": {"bag": bag, "particle": bag}, "display": DISPLAY,
            "elements": shell() + [lid_open()] + parts,
        })
        for variant in VARIANTS:
            name = f"catpack_{kind}_{variant}"
            write(models / f"{name}.json", {
                "parent": f"spider_man:item/catpack_{kind}",
                "textures": {"cat": f"minecraft:entity/cat/cat_{variant}{suffix}"},
            })
            write(items / f"{name}.json", {"model": {"type": "minecraft:model", "model": f"spider_man:item/{name}"}})
    print(models)


if __name__ == "__main__":
    main()
