# Changelog

## 0.2.3 — 2026-11-11

- The fall is forgotten from the moment a web or a zip bites, not from the shot: a web still flying when you hit
  the ground no longer saves you from a long drop (its flight time follows the web delay). A web that bites in
  time still lands you softly from any height.

## 0.2.2 — 2026-11-11

- On Horizon's Nightly of 2026-11-08 (`fdc87ae`) and horizon-mixins `5a64af9` (thanks to marshaaaal).
- A web or a zip shot while falling no longer hurts: in your own world the server now forgets the fall from before
  the web, as Slow Falling does, instead of landing it at the web (a drop of ten blocks and a web took about 7
  health). A plain fall still hurts as before.

## 0.2.1 — 2026-11-10

- A hint for a first web: "Hold R and jump to swing, or hold Z to zip" (naming the keys they are on) above the
  hotbar, from the first time the mod runs in a world of yours until the first web or zip, then never again. It steps
  aside after a minute of play and returns with the next start of the game.
- A web shot and a zip have a sound: two of the game's soul sand steps, pitched up, as the mod's own sound event
  `spider_man:web.shoot` (no file shipped).

## 0.2.0 — 2026-11-04

- On Horizon's Nightly of 2026-11-03 and horizon-mixins 0.6. The mod id is `spider_man`, as Horizon's registry
  requires, and `native/tools/package.py` makes its `.hnmod`; install it from the launcher's Mods page.
- Through Horizon's stable hook points: R and Z rebindable and the web delay on the mod's page in Options > Mods,
  which also turns the mod off; `/swingmode` completed in the chat box; the rumble scaled by Distortion Effects.
- The wall camera's whole-view turn ships in the release too, not only the development build.
- A native Horizon mod (`native/`): the same `src/` as a horizon-mixins bundle on an unmodified Horizon.
  Swing, zip, wall run, parkour, poses, web strands, chase camera, shake, `/swingmode`, and the fall reset on
  your own world's server, wind. Not yet: the HUD panel (the help shows in chat).
- The ODM gear mode is gone; `/swingmode on|off` switches the mod, the other subcommands mirror its settings.
- A development loop with live reload: the game loads the mod as a library and picks up a rebuild within about a
  second (`native/tools/watch.sh`).
- Parkour only while R is held, as R2 on PS4: on the ground R with W runs about twice the sprint, vaults and shoots no
  web; without R the ground plays as vanilla. R held from the ground fires the web after a jump or a fall off an edge.
  The ground jump-into-swing and its setting (`/swingmode jump`) are gone.
- On a wall R held runs too (no double W press); a run up into an overhang or a terrace floor goes out from under
  it and up past its lip instead of sticking.
- Wall camera, after Catlike Coding's orbit camera: the wall-as-floor turn is decided as you
  land on a wall and held until you leave it, eased at a capped speed; the view is built from the mouse angles (no
  jumps looking straight up or down); on a turned wall W goes where the view points along it, on a low wall W goes up;
  a mouse left alone lets the view settle behind the run, a little above the back.

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
