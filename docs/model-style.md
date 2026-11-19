# Model and texture style

Every model and texture this mod adds must look like it shipped with Minecraft. These are the rules we
hold ourselves to, in our own words, after the community's
[Minecraft style guide](https://www.blockbench.net/wiki/guides/minecraft-style-guide) (Blockbench wiki), plus
what we learned checking them in the game.

## Shape

- **One texture pixel is one model pixel.** Boxes have whole-pixel sizes, and textures are never stretched or
  squashed on a face. A part scaled up or down shows pixels of another size next to the game's ("mixels"): avoid
  it. A slight inflation (a sleeve over an arm) is the one accepted exception.
- **The model gives the outline, the texture gives the detail.** Few boxes; a strap, a zip, a stitch is painted,
  not built.
- **Flat parts are planes** (a box with no depth) with transparent pixels, not stacks of tiny boxes.
- **No staircases for slopes or curves.** A slope is one turned box; a round thing is one box.
- **Proportions are part of the message.** A big head reads as cute, big arms as strong. Size small things for
  recognisability next to the bulky player model, not to true scale.

## Texture

- **Few shades per material, on a ramp.** Start from the middle shade, add one shadow and one highlight, then
  more only if needed. Shift hue as well as brightness between steps (cooler shadows, warmer highlights).
- **Light comes from the top left.** On an entity the top and the front are brighter than the bottom and the
  back.
- **Every pixel is placed on purpose:** no brush noise, no random speckle, no pillow shading (dark rim round a
  light middle), no banding. Dithering only sparingly and in a regular pattern.
- **Outlines** (on items) are clearly darker than the middle shade and follow the light.

## Checking in the game

- Judge a model only in the game, in third person, from the back, both sides and above (`horizon-ctl look` +
  `screenshot`; the game draws no frames while its window is covered, so bring it forward first:
  `tools/front.sh`).
- **Put the vanilla thing next to ours at the same distance from the camera** (`/summon` it beside the player,
  level with them): a mob further away looks smaller and misleads the size check.
- Compare against the reference footage side by side, and let the person playing judge the feel.

## What we know about our own models

- **Cat backpack** (`native/hmod-src/catpack.rs`): the cat is the game's own model (kitten or grown cat) with
  a vanilla texture, so every cat variant works. The game's kitten already has a very large head; scaling the
  head further makes it look wrong. A whole-cat scale below 1 breaks pixel size against the player (accepted
  for now in development at 0.8, not settled).
- **Layers vanish in the mod's own poses.** While the mod poses the player (swing, wall run, flips) it draws
  the player itself, and layers added to the player (the cat backpack, other mods' worn things) aren't drawn.
  Open work: a setting to keep them, then drawing them in the posed model.
- **The cat's head can pass through the player's head** when the player looks up: the head turns, the bag
  doesn't. Known; to handle when the bag gets its motion.
