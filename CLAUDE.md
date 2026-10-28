# Agent notes

How an agent works on this mod. The README says what it is and how to build it.

## Layout

- `src/lib.rs` — entry points, keys (`input`), the settings page (`ui`, kept with `storage`), the chase camera,
  the rumble (`render.shake`), `/swingmode`.
- `src/spidey.rs` — the web mode: swing, zip, wall run, parkour, poses.
- `src/sense.rs` — the rays asked of the game each tick and how their hits are read. The game casts them a tick
  later, so the layout `ask` sends and the one `read` expects must stay in step.
- `tools/` — `run_take.sh` records a take, `front.sh` brings the game window forward. `HORIZON=` points them at a
  Horizon checkout (default: beside this repo).
- Local only, git-ignored: `docs/DECISIONS.md` (every decision with who made it: read it before changing a
  mechanic, add a row for each new one), `docs/video-spec.md` (reference notes), `ref/` (reference footage).

## Building and running

- Horizon (the `motion-api` branch) lives in `../horizon`. The mod: `../horizon/target/fast/cargo-horizon-mod
  horizon-mod build`. A game launched with `--dev-mod` reloads it on every rebuild.
- A Horizon change needs `cargo build --profile fast -p horizon-client --features mods` and a relaunch.

## Testing

- Only in test worlds (a city map, a flat test ground), never in a personal survival world. If the game is open
  somewhere else, close it and relaunch in a test world.
- After every launch press F5 once: checks and screenshots are taken in third person.
- Drive the game with `horizon-ctl key/look/chat/screenshot`. The mod's commands go through `chat`
  (`/swingmode debug`); `horizon-ctl command` goes straight to the server and the mod never sees it.
- Before driving the game, check nobody is playing: `horizon-ctl events` shows screens opened by someone else.
- After any pose change: `/swingmode debug` shows `posed 0` once standing, or the game's own animations stay off.
- A feel change is judged against the reference footage and by the person playing, not by numbers alone.

## Rules

- Pull requests only (CONTRIBUTING.md). Nothing is pushed or published without the maintainer's go.
- Public repo: no personal details, private paths or quotes in code, commits or docs. Comments say what and why
  in neutral terms.
- No code, models or animations from other mods or games; watch references, write our own.
- Nothing that changes what a server sees beyond the `motion` capability's own rules.
