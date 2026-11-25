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

- **Cat backpack** (`native/hmod-src/catpack.rs`): the bag is a trunk as wide as the back with a narrower
  unzipped top section, a big front pocket, side pockets, a grab handle and a lid hinged on the top section's
  front wall (open it hangs out in front, shut it lies over the top). Its proportions were set by laying game
  shots over pictures of the bag in the source game, aligned on the bag's box
  (`~/games/catpack-review/v8/overlay.py` is the tool): judging side by side was not enough. The cat is our
  own small model on the game's cat texture (grown: 64×32, kitten: `_baby`, 32×32), so every variant works
  without a texture of ours: the game's own head, a short upright trunk, and for the grown cat two straight
  one-box paws (the game has no two-part limbs anywhere: checked in the 26.3 model classes). It sits on the
  trunk's top, so nothing floats. It moves on damped springs stepped in game ticks (`motion` in
  `catpack.rs`): it looks around, breathes and kneads at rest; its head lags behind the carrier's turns, bobs
  with their steps and jumps; when the carrier is hurt it starts, dives, the lid slams with a bounce, then it
  looks out from under the lifted lid and pops back out, head first, one paw after the other. The item is the same
  bag in 3D (`native/tools/catpack_item_models.py` writes the item models from the same boxes), empty with
  its lid shut or with its cat. The bag is an item of the mod's own (`spider_man:cat_backpack`, named Back
  catpack). The rules (a cat goes in, grows, comes out; the bag is worn in a back slot of the mod's own, so the
  chest slot stays free; it holds 27 stacks) are `catpack_rules.rs`, the messages between a server and its
  clients `catpack_net.rs`, the storage window `catpack_ui.rs`. **Not settled:** the whole-cat scale. In development the grown cat is drawn at 0.6 and the kitten
  at 0.5, the sizes the overlay gave; that breaks pixel size against the player (the cat's pixels are smaller
  than the player's and the bag's). At 1 the game's head (5 wide) is well over half the player's head.
- **What is worn stays on in the mod's own poses.** While the mod poses the player (swing, wall run, flips) the
  game still draws the player itself: the limbs get the pose the way Horizon's emotes do (`Mob::emote`), and the
  body's whole turn is given to the finished vertices after the game's draw (`turn_posed`; the game builds a
  model's root transform in its library, where the prebuilt game takes no mixin). So armour, the held item, the
  cape, elytra and the mods' layers (the cat backpack) follow the pose.
- **The cat's head can pass through the player's head** when the player looks up: the head turns, the bag
  doesn't. Known; to handle when the bag gets its motion.
