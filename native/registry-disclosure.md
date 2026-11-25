# Spider-Man: what the mod sends, reads and writes (0.3.0)

For the registry listing. Everything below happens only in the player's own world (single player, or a LAN world
they host): the game's own server thread runs in the same game and the connection is to it. On anyone else's
server, and for a guest of a LAN world, the mod sends nothing, answers nothing and changes nothing.

## Network: two payload channels of the cat backpack

Both run between the player's game and the server that same game runs. Neither leaves the computer.

The channel identifiers `spider_man:catpack_state` and `spider_man:catpack_ask` are advertised during connection
setup, as every registered payload channel is; bag contents and bag actions travel only on the integrated
connection. The game checks that connection at every send and every receipt, and from the moment a connection ends
(logging out, disconnecting, the level unloading) nothing is sent or taken.

- `spider_man:catpack_state`, server to client. Sent only to the world's own player (the one playing it here),
  never to a LAN guest or another player.
  - `back`: what the player's own back slot holds (whether a bag is worn, a kitten or a grown cat, its cat's
    variant, and the bag as an item stack for the inventory's back slot). Sent when it changes and about once a
    second.
  - `window`: the bag's storage window, open or shut, with the stacks in the bag and in the player's inventory.
- `spider_man:catpack_ask`, client to server. Sent only by the player's own game to its own world's server, and
  heard there only from that player.
  - `off`, `open`, `close`, `hand`, `slot`, `noslot`, `thrown`, and `wear`, `swap`, `put`, `take` with a slot
    number. An ask names slots only: the server checks each against what it holds itself.

## Vanilla packets the mod changes

- The movement packet the player's game sends its own world's server: while a web holds, it says the player is on
  the ground, so no fall builds up. Never on another server.

No other network traffic: no chat lines or commands are sent on the player's behalf (`/swingmode` is read by the mod
and never reaches a server), no sockets, no HTTP, no telemetry.

## Files

- Reads and writes only its settings, in the game's folder: `mod-storage/spider-man/` (the hint line, the mod's
  page values). The path is fixed; no environment variable chooses a file.
- No other file is read or written. Saves are the game's own: the bag on the back is kept in the player's data
  (`PlayerPersisted.spider_man_back`), a bag's cat and items in the bag item's `custom_data`.
