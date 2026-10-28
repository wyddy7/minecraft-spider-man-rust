# minecraft-spider-man-rust

Spider-Man movement for [Horizon](https://github.com/horizon-rust/horizon), Minecraft: Java Edition rewritten in
Rust: web swinging, zips, wall running and parkour, tuned against Marvel's Spider-Man 2 gameplay. A Horizon mod in
Rust, compiled to WebAssembly (`.hmod`) and run in Horizon's mod sandbox.

A demonstration of what a movement mod can do on Horizon's proposed `motion` API. Fan-made. Not affiliated with
Marvel, Sony Interactive Entertainment, Insomniac Games, Mojang or Microsoft.

## Status

0.1.0. Plays in single player and in a LAN world you host. Multiplayer on servers is the plan, the way every
movement mod does it: the server runs the mod too and lets its players swing (it needs server-side support in
Horizon, proposed next).

The mod needs Horizon's experimental `motion` capability, which is proposed upstream and not merged yet: until it
is, build Horizon from the fork [`wyddy7/horizon`](https://github.com/wyddy7/horizon), branch `motion-api`.

**Native mod (in progress, 0.2.0).** Horizon is moving to native mods (horizon-mixins). The same mod also builds
as a native bundle on an unmodified Horizon v0.2.96 — no fork needed. Installing it with the Horizon launcher:
[native/README.md](native/README.md#install-with-the-horizon-launcher).

## Controls

| Input | Does |
|---|---|
| hold **R** | web: it flies for a moment, then a pendulum swing over the street; A/D lean the anchor to a side, W pumps. On the ground with an anchor in reach it jumps into the swing |
| release **R** | let go: a boost (more on the upswing), a forward flip, a spread-eagle fall |
| hold **Z** | zip to the crosshair (reach 250), riding an arc to the point and hanging there while Z is held; let go to hop on |
| **Space** | on a web: held swings faster, let go jumps off · on a zip: held hauls, let go jumps off · at the zip point: launch · on a wall: kick off |
| into a wall with **R** held | on the wall: WASD walk it like a floor, relative to the camera; sprint runs; nothing held clings; the top edge is vaulted |
| **W** into an obstacle | parkour by its height: a step or speed vault (1 block), a hand vault (2), a run up the face (3–5); long tops are landed on and run across |

R and Z are listed in Options › Controls › Key Binds under "Spider-Man" and can be rebound there; running uses the
game's own sprint binding. Third person (F5) is the intended view: a chase camera follows the swing, and on tall
walls it turns so the wall becomes the floor.

Settings: Options › Mods › Spider-Man › Settings — the web delay (how long a web flies before it bites; 0.5 by
default) and jumping into a swing from the ground. They are kept between games.

Chat commands (`/swingmode ...`): `on` (default) · `off` · `delay 0..2` · `jump on|off` (the settings page's) ·
`view side|front|back|off` (a fixed camera for looking at poses) · `debug`.

## Build

You need Rust (stable, via rustup) with the `wasm32-unknown-unknown` target, and your own copy of Minecraft 26.3 for
Horizon to prepare its resources from ([Horizon's build guide](https://github.com/horizon-rust/horizon#build-from-source)).

```sh
# Horizon with the motion capability, beside this repo
git clone -b motion-api https://github.com/wyddy7/horizon
cd horizon
cargo build --profile fast -p horizon-client --features mods
cargo build --profile fast -p cargo-horizon-mod
cd ..

# The mod
git clone https://github.com/wyddy7/minecraft-spider-man-rust
cd minecraft-spider-man-rust
../horizon/target/fast/cargo-horizon-mod horizon-mod build   # target/horizon-mod/swing.hmod
```

Play it as a dev mod (reloaded whenever you rebuild it):

```sh
cd ../horizon
./target/fast/horizon-ctl launch --world <World> --dev-mod ../minecraft-spider-man-rust/target/horizon-mod/swing.hmod
```

A city map makes the most of it. For the look, use any Spider-Man skin.

## How it works

`src/lib.rs` holds the mod's entry points, keys and camera; `src/spidey.rs` the web mode (swing, zip, wall run,
parkour, poses); `src/sense.rs` the rays the mod asks the game to cast each tick (anchor fan, walls, the ground
ahead). Everything the mod does to the player goes through the `motion` capability: velocity, fall reset, the
player model's pose, ropes, the chase camera and wind.

## Contributing

Pull requests only; see [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE).
