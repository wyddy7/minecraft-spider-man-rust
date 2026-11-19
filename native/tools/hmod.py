"""Turns the generated bundle (native/mixins/spider_man/, from release.py) into a native `.hmod` project.

Horizon's mod API merge (launcher 0.2.70+) retired `.hnmod`: a native mod is now a flat Cargo project with one
`horizon-mod.toml` that names the mod and pins the Horizon build, its game changes as `[[native.mixin]]` and
`[[native.module]]` rows, and a crate whose `impl Hooks` holds what the mod API itself offers (here: the mod's keys
in Controls, its chat command). This writes that project to native/hmod/ (generated; edit the sources, not it):

  horizon-mod.toml   the mod's identity, the pins, the rows (from horizon-mixin.toml, renamed to the new tables)
  Cargo.toml         the mod's crate, on the pinned SDK in .horizon/sdk/
  src/*.rs, assets/  the crate and its textures: native/hmod-src/
  src/game/*.rs      the module and helpers, compiled inside the game's function copies, never part of the crate
  LICENSE

Usage: hmod.py (after tools/release.sh --generate-only); the pins come from compatibility.json unless
--horizon / --mixins name others.
"""

import argparse
import json
import re
import shutil
import tomllib
from pathlib import Path

NATIVE = Path(__file__).resolve().parent.parent
BUNDLE = NATIVE / "mixins" / "spider_man"
SRC = NATIVE / "hmod-src"
OUT = NATIVE / "hmod"
GAME = "26.3"

HEADER = """id = "spider_man"
name = "Spider-Man"
version = "{version}"
description = "Web swinging, zips, wall running and parkour."
authors = ["wyddy7"]
api = "0.2"
side = "client"
capabilities = ["registries"]

[native]
game = "{game}"
horizon = "{horizon}"
mixins = "{mixins}"
"""

CARGO = """[package]
name = "spider_man"
version = "{version}"
edition = "2024"

[lib]
path = "src/lib.rs"

[dependencies]
horizon-mod-api = {{ path = ".horizon/sdk/api/crates/horizon-mod-api" }}

# Its own workspace: the repository's workspaces above it are the development setup.
[workspace]
"""


def toml_value(value):
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'
    if isinstance(value, list):
        if value and isinstance(value[0], dict):
            return "[\n" + "".join(f"  {toml_value(v)},\n" for v in value) + "]"
        return "[" + ", ".join(toml_value(v) for v in value) + "]"
    if isinstance(value, dict):
        return "{ " + ", ".join(f"{k} = {toml_value(v)}" for k, v in value.items()) + " }"
    raise TypeError(value)


def table(name, row):
    return f"[[{name}]]\n" + "".join(f"{k} = {toml_value(v)}\n" for k, v in row.items()) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    compat = json.loads((NATIVE / "compatibility.json").read_text())
    parser.add_argument("--horizon", default=compat["horizon_revision"])
    parser.add_argument("--mixins", default=compat["horizon_mixins_revision"])
    args = parser.parse_args()
    for sha in (args.horizon, args.mixins):
        if not re.fullmatch(r"[0-9a-f]{40}", sha):
            raise SystemExit("pins must be full 40-character commits")
    old = tomllib.loads((BUNDLE / "horizon-mixin.toml").read_text())
    version = old["mod"]["version"]
    if OUT.exists():
        for child in OUT.iterdir():
            if child.name not in (".horizon", "target"):
                shutil.rmtree(child) if child.is_dir() else child.unlink()
    (OUT / "src" / "game").mkdir(parents=True)
    rows = [HEADER.format(version=version, game=GAME, horizon=args.horizon, mixins=args.mixins), "\n"]
    for module in old.get("module", []):
        module = dict(module)
        source = module["source"]
        shutil.copy(BUNDLE / source, OUT / "src" / "game" / source)
        module["source"] = f"src/game/{source}"
        rows.append(table("native.module", module))
    for mixin in old.get("injection", []):
        mixin = dict(mixin)
        helper = mixin["helper"]
        shutil.copy(BUNDLE / helper, OUT / "src" / "game" / helper)
        mixin["helper"] = f"src/game/{helper}"
        rows.append(table("native.mixin", mixin))
    (OUT / "horizon-mod.toml").write_text("".join(rows).rstrip("\n") + "\n")
    (OUT / "Cargo.toml").write_text(CARGO.format(version=version))
    for source in sorted(SRC.glob("*.rs")):
        shutil.copy(source, OUT / "src" / source.name)
    if (SRC / "assets").is_dir():
        shutil.copytree(SRC / "assets", OUT / "assets")
    shutil.copy(NATIVE.parent / "LICENSE", OUT / "LICENSE")
    (OUT / ".gitignore").write_text("/target\n/.horizon\n")
    print(OUT)


if __name__ == "__main__":
    main()
