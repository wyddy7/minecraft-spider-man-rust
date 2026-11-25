# Changelog

## 0.3.0 — 2026-11-26

- Swing animation: on a web the hand on the web's side holds it and the limbs play a take (web shot, downswing,
  kick, upswing); nine takes, mirrored by the side the web is stuck on. The head is posed in flight. Changes of
  pose cross-fade, the landing comes upright as one turn, and between ticks the body and each part blend as
  rotations.
- After a release each take goes on with its own flip, dive or tuck, most of them then with their own flight for
  another two seconds: no cut to a single falling pose. A release thrown well upward on two of the takes hangs head down at the
  top and arches forward. A level or falling release never turns the body upside down.
- Free flight: with no take left to play the body lies along the flight path, diving when you fall forward and
  level when you fly level; the spread-eagle is kept for a slow or straight-down fall.
- A web shot out of the air throws the arm at the anchor.
- Armour, the held item, the cape, elytra and worn layers (the cat backpack) stay on and follow the body in the
  mod's poses: swinging, flips, wall runs.
- `/swingmode anim on|off` switches the takes; `/swingmode take 0..8` plays one take on every web.
- `/swingmode view … follow` carries the debug view along with the player; `/swingmode row TAKE ROW` holds one
  row of a take, for looking at poses.

## 0.2.8 — 2026-11-24

- Back catpack: its messages are sent and taken only while your game is connected to its own world, checked at
  each message; leaving a world stops them at once instead of a tick later. Nothing changes in play.

## 0.2.7 — 2026-11-24

- Back catpack, tightened for the mod registry. What you see is the same: your own tamed cat rides in the bag, a
  kitten grows there, and sneak-use on top of a block sets it down with its name, collar, voice and owner.
- A bag now keeps only what makes the cat yours (its kind, voice, name, collar, age, health and owner) and the cat
  comes out only for its owner, built from those fields alone: an item made with commands can no longer put
  anything else into the world. Only the mod's own bag item counts as a bag. Bags filled in 0.2.6 keep their cats.
- Single player for real: the catpack does nothing on someone else's server or for a guest of a LAN world, on
  either side, and its messages go only between your game and your own world.
- Leaving a world forgets everything the catpack remembered there, so a worn bag can no longer follow you into
  another world. The bag on your back is kept in the part of your save that survives a death, so with
  `keep_inventory` it stays on after a respawn and a reload too.
- The bag's item picture is the bag alone (lid shut when empty, open with a cat in it); its tooltip still says
  what it holds, and the cat is still drawn alive on your back and in your hand.
- The test command `/catpack` is gone, and the mod reads no file named by an environment variable.

## 0.2.6 — 2026-11-24

- Back catpack: a backpack that carries your cat. Craft it (leather, string, a chest) or take it from the
  mod's creative tab. Use it on your own tamed cat and the cat rides in it, a kitten grows there, and sneak-use
  on top of a block sets the same cat down there, as a bucket empties, with its name, collar and owner.
- The bag is worn in a back slot beside the inventory (the chest slot stays free): sneak and use it, click the
  slot with it, or shift-click it; G opens it (27 slots), Shift+G takes it off. Its tooltip shows what it holds.
- The cat looks around, kneads and breathes, hides in the bag when its carrier is hurt, and is alive in a hand
  too. A bag does not burn, drops with its cat when its wearer dies, and stays on with `keep_inventory`. A bag
  on the ground never despawns, so a cat dropped at death or thrown away waits for you.
- Back catpack is single player only for now: it works in your own world; servers and guests of a LAN world
  come later.
- Camera distance: a slider on the mod's page (Options > Mods > Spider-Man > Config, 2 to 12 blocks, 6 as
  before) and `/swingmode camera`; in flight the camera still backs off 2 blocks more.
- "Reset all to defaults" on the mod's page puts the web delay and the camera distance back (the rows show it the
  next time the page opens).
- Long flights go faster: after 3 s in the air the top speed rises over 5 s by up to 30 % more, for as long as
  the flight keeps its speed; landing resets it. The web's pull is unchanged, so the start feels the same. Pick
  the raise on the mod's page ("Long-flight speed bonus %", 0 to 200) or with `/swingmode speed`; 0 is the old
  top speed. "Reset all to defaults" puts it back to 30.
- `/swingmode speedo` shows a speedometer in the corner: the speed now, the top of this flight, the best since
  the game started, the current top speed and the time in the air.
- The web delay row's default is 0.5 again, the mod's own default (0.2.5's row said 1).
- `/swingmode view` takes an angle from behind, a distance and a height: `/swingmode view 45 2.5 1.4` looks at
  the player from behind and to the right, close up, for checking poses and what the player wears.

## 0.2.5 — 2026-11-18

- A `.hmod`, the native mod format of launchers 0.2.70 and later, which no longer load `.hnmod`. On Horizon's
  Nightly of 2026-11-18 (`0b14f8a`) and horizon-mixins `e59da90`.
- R and Z are in Controls with the game's own keys; the web delay is a slider on the mod's page (Options > Mods >
  Spider-Man > Config) and follows `/swingmode delay`; `/swingmode` is taken by the mod in your own world only.
- The fall is still forgotten while a web holds, through Horizon's fall hook and only for your own player: mobs
  and other players landing at the same moment are hurt as always.

## 0.2.4 — 2026-11-14

- On Horizon's Nightly of 2026-11-13 (`9f42064`), still with horizon-mixins `5a64af9`: 0.2.3 no longer compiled
  there (the wind sound's random source moved).
- A vault off a roof gives the fall back once its flip is done: WASD steers and a web can be shot on the way
  down (the whole drop used to follow the vault's line).

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
- The old grapple mode is gone; `/swingmode on|off` switches the mod, the other subcommands mirror its settings.
- A development loop with live reload: the game loads the mod as a library and picks up a rebuild within about a
  second (`native/tools/watch.sh`).
- Parkour only while R is held: on the ground R with W runs about twice the sprint, vaults and shoots no
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
  a second grapple mode (`/swingmode odm`).
