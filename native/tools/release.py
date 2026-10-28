"""Generates the shipping native bundle of the Spider-Man mod.

The development setup runs the mod as a library the game reloads (the socket,
horizon-dev-reload, which only our Horizon dev branch has). A shipped native
mod builds on a clean Horizon, and horizon-mixins takes one self-contained
module per bundle. So this writes `<out>/mixins/spider-man/`:

  spider_man.rs        the game-side hooks' module, with inline modules for the
                       ABI (`horizon_dev_reload`, now calling the mod in-process),
                       the SDK facade (`horizon_mod_sdk`) and the mod itself
                       (`spiderman`, its source files inlined, paths made absolute)
  *.rs                 the hook helpers, pointed at `crate::game::spider_man`
  horizon-mixin.toml   the development manifest minus development-only hooks,
                       pinned to `--horizon-revision`

Usage: release.py --dev-bundle DIR --mod-src DIR --out DIR --horizon-revision SHA
(the socket's ABI and the SDK facade are read from native/); tools/release.sh
runs it with this repository's paths.
"""

import argparse
import re
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MODULE = "spider_man"
BUNDLE = "spider-man"
BASE = f"crate::game::{MODULE}"
# Development-only hooks: they need a crate both the game and its library see.
DEV_ONLY_INJECTIONS = {"turned-view"}


def strip_dev_only(text):
    """Removes `// dev-only:begin` ... `// dev-only:end` blocks."""
    return re.sub(r"[ \t]*// dev-only:begin.*?// dev-only:end[^\n]*\n", "", text, flags=re.S)


def strip_inner_docs(text):
    """Inner doc comments (`//!`) are file attributes: not allowed in a payload."""
    return "".join(
        "//" + line[3:] if line.lstrip().startswith("//!") else line
        for line in text.splitlines(keepends=True)
    )


def indent(text, by="    "):
    return "".join(by + line if line.strip() else line for line in text.splitlines(keepends=True))


def abi_module():
    abi = strip_dev_only((ROOT / "dev-reload/src/abi.rs").read_text())
    return f"""pub mod horizon_dev_reload {{
    // The plug's data types, and the calls the hooks make, served in-process.
    pub mod abi {{
{indent(abi, "        ")}    }}

    static CURRENT: std::sync::Mutex<abi::Current> = std::sync::Mutex::new(abi::Current {{
        rays: Vec::new(),
        ropes: Vec::new(),
        pose: None,
        camera: None,
        wind: 0.0,
        panel: None,
    }});

    pub fn tick(input: &abi::Input) -> Option<abi::Output> {{
        let out = {BASE}::spiderman::plug_tick(input);
        if let Ok(mut current) = CURRENT.lock() {{
            current.apply(&out);
        }}
        Some(out)
    }}

    pub fn rays() -> Vec<abi::Ray> {{
        CURRENT.lock().map(|c| c.rays.clone()).unwrap_or_default()
    }}

    pub fn current() -> abi::Current {{
        CURRENT.lock().map(|c| c.clone()).unwrap_or_default()
    }}

    /// The mod is always there.
    pub fn loads() -> u32 {{
        1
    }}

    /// The wall camera's whole-view turn needs a crate both the game and its
    /// library see; a shipped mod keeps the game's own view.
    pub mod frame {{
        pub fn set_view_turn(_q: Option<[f32; 4]>) {{}}
    }}
}}
"""


def sdk_module():
    sdk = strip_dev_only((ROOT / "sdk/src/lib.rs").read_text())
    sdk = sdk.replace("horizon_dev_reload::", f"{BASE}::horizon_dev_reload::")
    sdk = strip_inner_docs(sdk)
    return f"pub mod horizon_mod_sdk {{\n{indent(sdk)}}}\n"


def mod_module(src):
    def load(name):
        text = strip_inner_docs((src / f"{name}.rs").read_text())
        # The mod's own paths first: the SDK's absolute path starts with crate:: too.
        text = re.sub(r"\bcrate::", f"{BASE}::spiderman::", text)
        text = text.replace("horizon_mod_sdk::", f"{BASE}::horizon_mod_sdk::")
        return text

    lib = load("lib")
    for name in re.findall(r"^mod (\w+);$", lib, flags=re.M):
        lib = re.sub(
            rf"^mod {name};$",
            f"mod {name} {{\n{indent(load(name))}}}",
            lib,
            flags=re.M,
        )
    entry = re.search(rf"^{re.escape(BASE)}::horizon_mod_sdk::export_mod!\((\w+)\);$", lib, flags=re.M)
    if not entry:
        raise SystemExit("the mod's export_mod! line was not found")
    ty = entry.group(1)
    lib = lib.replace(
        entry.group(0),
        f"""thread_local! {{
    static MOD: std::cell::RefCell<Option<{ty}>> = const {{ std::cell::RefCell::new(None) }};
}}

/// One tick of the mod, called by the game-side hooks.
pub fn plug_tick(
    input: &{BASE}::horizon_dev_reload::abi::Input,
) -> {BASE}::horizon_dev_reload::abi::Output {{
    MOD.with(|slot| {BASE}::horizon_mod_sdk::run_typed::<{ty}>(slot, input))
}}""",
    )
    return f"pub mod spiderman {{\n{indent(lib)}}}\n"


def manifest(dev, revision):
    text = dev.read_text()
    text = re.sub(r'^id = "spidey-socket"$', f'id = "{BUNDLE}"', text, count=1, flags=re.M)
    text = re.sub(r'horizon_revision = "[0-9a-f]+"', f'horizon_revision = "{revision}"', text)
    text = text.replace('name = "spidey_socket"', f'name = "{MODULE}"')
    text = text.replace('source = "socket.rs"', f'source = "{MODULE}.rs"')
    blocks = re.split(r"(?=^\[\[injection\]\]$)", text, flags=re.M)
    kept = [
        b for b in blocks
        if not any(re.search(rf'^id = "{i}"$', b, flags=re.M) for i in DEV_ONLY_INJECTIONS)
    ]
    return "".join(kept).rstrip("\n") + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--dev-bundle", type=Path, required=True)
    parser.add_argument("--mod-src", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--horizon-revision", required=True)
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9a-f]{40}", args.horizon_revision):
        raise SystemExit("--horizon-revision must be a full commit hash")
    out = args.out / "mixins" / BUNDLE
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    dev = args.dev_bundle
    game_side = (dev / "socket.rs").read_text()
    header = (
        "// Spider-Man: web swinging, wall running and parkour, as a native Horizon mod.\n"
        "// Generated by tools/release.py from the mod's sources and its game-side\n"
        "// hooks; edit those, not this file.\n\n"
    )
    module = header + game_side + "\n" + abi_module() + "\n" + sdk_module() + "\n" + mod_module(args.mod_src)
    (out / f"{MODULE}.rs").write_text(module)
    manifest_text = manifest(dev / "horizon-mixin.toml", args.horizon_revision)
    (out / "horizon-mixin.toml").write_text(manifest_text)
    for helper in sorted(set(re.findall(r'^helper = "([\w.]+)"$', manifest_text, flags=re.M))):
        text = (dev / helper).read_text().replace("crate::game::spidey_socket::", f"{BASE}::")
        (out / helper).write_text(text)
    print(out)


if __name__ == "__main__":
    main()
