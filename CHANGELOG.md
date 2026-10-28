# Changelog

## Unreleased (0.2.0)

- A native Horizon mod (`native/`): the same `src/` as a horizon-mixins bundle on Horizon v0.2.96, no fork needed.
  Swing, zip, wall run, parkour, poses, web strands, chase camera, shake, `/swingmode`, and the fall reset on
  your own world's server, wind. Not yet: the wall-as-floor roll, Controls rows, the settings page, the HUD panel.
- A development loop with live reload: the game loads the mod as a library and picks up a rebuild within about a
  second (`native/tools/watch.sh`).

## 0.1.0 — 2026-10-28

First release, single player and your own LAN world, on Horizon's proposed `motion` capability.

- Web swinging: a short web flight (`/swingmode delay`), a pendulum over the street with an invisible floor, A/D
  anchor side, W pump, Space to speed up or jump off, a release boost with a flip and a spread-eagle fall.
- Zips to the crosshair over an arc, hanging at the point, point launch.
- Wall walking and running as on a floor, a wall-as-floor camera on tall walls, kick-off, slaps at speed.
- Ground and roof parkour by obstacle height: step and speed vault, hand vault, run up the face; roof runs after a
  landing.
- Poses for the swing, flips, falls, vaults and the wall run; a chase camera with a speed rumble; wind.
- Keys in Controls (R web, Z zip), rebindable; a settings page (web delay, jump into a swing), kept between games;
  an ODM gear mode (`/swingmode odm`).
