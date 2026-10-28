# Contributing

Changes come in as pull requests only; nobody pushes to `main`, maintainers included.

## A pull request

- **One change per pull request**, with what and why. For anything that moves or looks different, a short clip or
  screenshots in third person (F5): movement is judged by eye.
- **Name the reference** a feel change follows: a moment of Spider-Man 2 gameplay (video and time), another game,
  or a playtest note. Numbers that were tuned say so in a comment.
- **Builds clean**: `cargo-horizon-mod horizon-mod build` with no warnings.
- **Played**: in a test world, not your own survival world. Check the things your change can break: a swing, a zip,
  a wall run, a vault, and that standing still afterwards plays the game's own animations again (`/swingmode debug`
  shows `posed 0` once standing).

## What the mod may not do

- Nothing that changes what a server sees from the player beyond what the `motion` capability allows: no packet
  tricks, no fall-damage workarounds outside the player's own world.
- No code, models or animations taken from other mods or games. References are watched, not copied.

## Licence

By contributing you agree that your contribution is licensed under the [MIT License](LICENSE).
