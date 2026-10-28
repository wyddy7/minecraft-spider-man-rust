//! Swing: Spider-Man web swinging for Horizon, on the `motion` capability
//! (or, built as a native mod, the same capability as game-side hooks).
//!
//! The mod drives the player's velocity itself from the first hook until
//! the feet touch the ground: each tick it takes the real displacement of
//! the last tick (so walls and floors still stop you), adds its own gravity
//! and forces, and hands the game the result. The game's air drag (0.91
//! per tick sideways) never applies in flight, so speed carries from one
//! swing to the next.
//!
//! - Web (Marvel's Spider-Man, 2018): hold R. The anchor is picked above and
//!   ahead from a fan of rays, biased by A/D; the swing is a pendulum with a
//!   short tug when the web bites; with no A/D the arc is pulled straight
//!   along the view; W pumps; letting go on the upswing throws you up and
//!   forward.
use horizon_mod_sdk::{
    Mod, Registration, chat, game,
    hud::{self, Corner, Line},
    input,
    motion::{self, Vec3 as V},
    render, storage, ui,
};
use std::cell::RefCell;
use std::ops::{Add, Mul, Sub};

mod sense;
mod spidey;

use sense::{Probe, Sense};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct P {
    x: f64,
    y: f64,
    z: f64,
}
impl P {
    const fn new(x: f64, y: f64, z: f64) -> Self {
        P { x, y, z }
    }
    fn of(v: &V) -> Self {
        P::new(v.x, v.y, v.z)
    }
    fn wit(self) -> V {
        V {
            x: self.x,
            y: self.y,
            z: self.z,
        }
    }
    fn dot(self, o: P) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    fn cross(self, o: P) -> P {
        P::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    fn len(self) -> f64 {
        self.dot(self).sqrt()
    }
    fn norm(self) -> P {
        let l = self.len();
        if l < 1e-9 { P::default() } else { self * (1.0 / l) }
    }
    fn flat(self) -> P {
        P::new(self.x, 0.0, self.z)
    }
    fn clamp(self, max: f64) -> P {
        let l = self.len();
        if l > max { self * (max / l) } else { self }
    }
}
impl Add for P {
    type Output = P;
    fn add(self, o: P) -> P {
        P::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for P {
    type Output = P;
    fn sub(self, o: P) -> P {
        P::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f64> for P {
    type Output = P;
    fn mul(self, k: f64) -> P {
        P::new(self.x * k, self.y * k, self.z * k)
    }
}
const UP: P = P::new(0.0, 1.0, 0.0);

/// The wind's loudness (muffled, no louder than Danny's AoT): it
/// starts at `WIND_FROM` and is full at `WIND_FULL` blocks a tick.
const WIND_FROM: f64 = 0.35;
const WIND_FULL: f64 = 1.5;
const WIND_MAX: f32 = 0.3;

/// The chase camera: distance on the ground and in flight, the ease per
/// tick, the height above the eyes; the fixed view's distance.
/// The camera rumble: from and full speed, blocks per tick.
const SHAKE_FROM: f64 = 1.0;
const SHAKE_FULL: f64 = 1.75;
/// Half the full rumble: the full one was too much in playtests.
const SHAKE_SCALE: f64 = 0.5;
/// Our full rumble in `render.shake`'s strength (its 1 turns the view 3
/// degrees; ours was 0.45), and the jolt added every tick: each fades over
/// `SHAKE_JOLT` seconds, so about 2.2 of them overlap at once.
const SHAKE_STRENGTH: f64 = 0.15;
const SHAKE_JOLT: f32 = 0.25;
const SHAKE_OVERLAP: f64 = 2.2;
const CHASE_GROUND: f64 = 6.0;
const CHASE_FLY: f64 = 8.0;
const CHASE_EASE: f64 = 0.08;
const CHASE_HEIGHT: f32 = 1.0;
const VIEW_DISTANCE: f64 = 5.0;

/// Where the cables pull on the body: about the chest, above the feet.
const CHEST: f64 = 1.1;

mod web {
    /// Gravity on the web, blocks per tick squared (tuned): heavier than
    /// vanilla's 0.08 so a 20-block arc takes about 2 s a side.
    pub const GRAVITY: f64 = 0.11;
    /// Gravity between webs, as vanilla's.
    pub const FALL: f64 = 0.08;
    pub const DAMP: f64 = 0.995;
    pub const MIN_ROPE: f64 = 6.0;
    /// As far as the rays reach (Danny's AoT's 250); the picker still likes
    /// a rope near `IDEAL_ROPE` and takes a far one when nothing is nearer.
    pub const MAX_ROPE: f64 = 250.0;
    pub const IDEAL_ROPE: f64 = 20.0;
    /// How fast the score drops away from the ideal length, per block.
    pub const LENGTH_FALLOFF: f64 = 48.0;
    pub const MIN_HEIGHT: f64 = 3.0;
    /// The tug when the web bites: the rope shortens to this share over
    /// `TUG_TICKS`.
    pub const TUG: f64 = 0.88;
    pub const TUG_TICKS: f64 = 16.0;
    pub const PUMP: f64 = 0.025;
    /// With no A/D, this share of the speed across the view is taken off
    /// each tick: the arc runs straight down the street.
    pub const STRAIGHTEN: f64 = 0.06;
    pub const ATTACH_KICK: f64 = 0.12;
    pub const RELEASE_FORWARD: f64 = 0.22;
    pub const RELEASE_UP: f64 = 0.26;
    pub const RELEASE_UP_FALLING: f64 = 0.12;
    pub const MAX_SPEED: f64 = 2.2;
    pub const COLOR: u32 = 0xEE_EEEE;
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Web,
    Off,
}

struct Swing {
    mode: Mode,
    /// The chest last tick while the mod drives.
    last_chest: Option<P>,
    now: u64,
    last_panel: Vec<String>,
    /// A fixed camera for checking poses (`/swingmode view`), and the chase
    /// camera's distance now.
    view: Option<motion::Camera>,
    chase: f64,
    /// The `web` mode, built to the reference gameplay.
    spidey: spidey::Spidey,
    /// The rays asked of the game, and this tick's view from them.
    rays: sense::Rays,
    sense: Option<Sense>,
}

/// The mod's own keys (`input.register-key`: the player rebinds them on the
/// mod's page in Options > Mods), as the names the code reads.
const MOD_KEYS: [(&str, &str, &str, &str); 2] = [
    ("web", "Web swing", "key.keyboard.r", "KeyR"),
    ("zip", "Web zip", "key.keyboard.z", "KeyZ"),
];
/// The game's movement mappings, as bound by the player (`input.mapping`).
const MAPPINGS: [(&str, &str); 7] = [
    ("key.forward", "KeyW"),
    ("key.left", "KeyA"),
    ("key.back", "KeyS"),
    ("key.right", "KeyD"),
    ("key.jump", "Space"),
    ("key.sneak", "ShiftLeft"),
    ("key.sprint", "ControlLeft"),
];

/// The settings page in Options > Mods (capability `ui`), kept with
/// capability `storage` under `SETTINGS`: the web delay steps a button
/// cycles through, and the buttons' ids.
const SETTINGS: &str = "settings";
const DELAY_STEPS: [f64; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
const BUTTON_DELAY: u32 = 1;
const BUTTON_JUMP: u32 = 2;

thread_local! {
    /// What is held this tick, by the names above: the mod's keys from
    /// `input`'s key events, the movement mappings read each tick.
    static HELD: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

/// Reads this tick's keys into `HELD`.
fn read_keys() {
    HELD.with_borrow_mut(|held| {
        for event in input::poll() {
            let (name, down) = match event {
                input::Event::KeyDown(n) => (n, true),
                input::Event::KeyUp(n) => (n, false),
                _ => continue,
            };
            let Some(&(_, _, _, code)) = MOD_KEYS.iter().find(|k| k.0 == name) else {
                continue;
            };
            held.retain(|k| *k != code);
            if down {
                held.push(code);
            }
        }
        held.retain(|k| !MAPPINGS.iter().any(|m| m.1 == *k));
        for (mapping, code) in MAPPINGS {
            if input::mapping(mapping).is_some_and(|m| m.down) {
                held.push(code);
            }
        }
    });
}

fn key(_state: &Sense, name: &str) -> bool {
    HELD.with_borrow(|held| held.iter().any(|k| *k == name))
}

/// 1 for a held key, 0 otherwise.
fn axis(state: &Sense, name: &str) -> f64 {
    f64::from(u8::from(key(state, name)))
}

/// The fan's best web anchor: high enough, ahead, near the ideal length,
/// and toward the side A or D asks for.
fn pick_anchor(state: &Sense, chest: P, side: f64) -> Option<P> {
    let ahead = P::of(&state.look).flat().norm();
    let right = ahead.cross(UP).norm();
    state
        .fan
        .iter()
        .chain(std::iter::once(&state.aim))
        .filter_map(|Probe { hit, .. }| hit.as_ref().map(P::of))
        .filter_map(|a| {
            let rope = a - chest;
            let length = rope.len();
            let flat = rope.flat().norm();
            let forward = flat.dot(ahead);
            let ok = (web::MIN_ROPE..=web::MAX_ROPE).contains(&length)
                && rope.y >= web::MIN_HEIGHT
                && forward > -0.2;
            let score = forward * 2.0 + (rope.y / 25.0).min(1.0)
                - (length - web::IDEAL_ROPE).abs() / web::LENGTH_FALLOFF
                + flat.dot(right) * side;
            ok.then_some((score, a))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, a)| a)
}

impl Swing {
    /// The settings kept from the last time, if any.
    fn load_settings(&mut self) {
        let Some(bytes) = storage::get(SETTINGS) else {
            return;
        };
        for line in String::from_utf8_lossy(&bytes).lines() {
            match line.split_once('=') {
                Some(("delay", v)) => {
                    if let Ok(d) = v.trim().parse::<f64>() {
                        self.spidey.web_delay = d.clamp(0.0, 2.0);
                    }
                }
                Some(("jump", v)) => self.spidey.jump_start = v.trim() != "off",
                _ => {}
            }
        }
    }

    fn save_settings(&self) {
        let text = format!(
            "delay={}\njump={}\n",
            self.spidey.web_delay,
            if self.spidey.jump_start { "on" } else { "off" }
        );
        if let Err(e) = storage::set(SETTINGS, text.as_bytes()) {
            horizon_mod_sdk::log::write(horizon_mod_sdk::log::Level::Warn, &e);
        }
    }

    /// The settings page's widgets, as they stand.
    fn show_settings(&self) {
        let button = |id, label: String| ui::Widget::Button(ui::ButtonWidget { id, label });
        let text = |id, text: &str| {
            ui::Widget::Text(ui::TextWidget {
                id,
                text: text.into(),
                color: 0,
            })
        };
        ui::set_settings(&[
            button(
                BUTTON_DELAY,
                format!("Web delay: {:.2}", self.spidey.web_delay),
            ),
            text(10, "How long a web flies before it bites: 1 as in Spider-Man 2, 0 at once."),
            button(
                BUTTON_JUMP,
                format!(
                    "Jump into a swing from the ground: {}",
                    if self.spidey.jump_start { "On" } else { "Off" }
                ),
            ),
        ]);
    }

    fn panel(&mut self) {
        let lines: Vec<String> = match self.mode {
            Mode::Off => Vec::new(),
            Mode::Web => vec!["Web: hold R, A/D side. Hold Z: zip, Space hauls".into()],
        };
        if lines != self.last_panel {
            if lines.is_empty() {
                hud::clear();
            } else {
                hud::set_panel(
                    Corner::BottomLeft,
                    &lines
                        .iter()
                        .map(|t| Line {
                            text: t.clone(),
                            color: 0xDD_DDDD,
                        })
                        .collect::<Vec<_>>(),
                );
            }
            self.last_panel = lines;
        }
    }
}

impl Mod for Swing {
    fn init() -> Result<(Self, Registration), String> {
        Ok((
            Swing {
                mode: Mode::Web,
                last_chest: None,
                now: 100,
                last_panel: Vec::new(),
                spidey: spidey::Spidey::default(),
                rays: sense::Rays::default(),
                sense: None,
                view: None,
                chase: CHASE_GROUND,
            },
            Registration::new().command("swingmode").on_tick(),
        ))
        .map(|(mut swing, registration)| {
            swing.load_settings();
            swing.show_settings();
            (swing, registration)
        })
        .inspect(|_| {
            for (name, label, key, _) in MOD_KEYS {
                if let Err(e) = input::register_key(name, label, key) {
                    horizon_mod_sdk::log::write(horizon_mod_sdk::log::Level::Warn, &e);
                }
            }
        })
    }

    fn tick(&mut self, _tick: u64) {
        self.now += 1;
        read_keys();
        self.panel();
        let (Some(st), Some(player)) = (motion::get(), game::get_player()) else {
            self.last_chest = None;
            self.sense = None;
            return;
        };
        let state = self.rays.read(&st, player.y);
        self.rays.ask(&state, player.yaw);
        self.sense = Some(state.clone());
        let chest = P::new(player.x, player.y + CHEST, player.z);
        // The camera: a fixed view if one is set, else in web mode a chase
        // camera that backs off in flight.
        let target = if self.spidey.driving { CHASE_FLY } else { CHASE_GROUND };
        self.chase += (target - self.chase) * CHASE_EASE;
        // The rumble grows with speed: none below ~20 m/s, full at ~35.
        let speed = P::of(&state.velocity).len();
        let speed_shake = ((speed - SHAKE_FROM) / (SHAKE_FULL - SHAKE_FROM)).clamp(0.0, 1.0);
        // A slap jolts the camera, fading over ~0.5 s (full strength, unlike
        // the halved speed rumble).
        self.spidey.impact *= 0.85;
        let shake = speed_shake.max(self.spidey.impact / SHAKE_SCALE).min(1.0 / SHAKE_SCALE);
        let wall_cam = if self.mode == Mode::Web {
            self.spidey.wall_camera(&state)
        } else {
            (None, 0.0)
        };
        let camera = self.view.clone().or_else(|| {
            (self.mode == Mode::Web).then_some(motion::Camera::Chase(motion::ChaseCamera {
                distance: self.chase as f32,
                height: CHASE_HEIGHT,
                side: 0.0,
                wall: wall_cam.0.map(P::wit),
                wall_blend: wall_cam.1,
            }))
        });
        let rumble = shake * SHAKE_SCALE * SHAKE_STRENGTH / SHAKE_OVERLAP;
        if rumble > 0.0 && self.mode == Mode::Web && self.view.is_none() {
            render::shake(rumble as f32, SHAKE_JOLT);
        }
        motion::set_camera(camera);
        if self.mode == Mode::Web {
            let v = match self.last_chest {
                Some(last) if self.spidey.driving => chest - last,
                _ => P::of(&state.velocity),
            };
            if let Some(v) = self.spidey.tick(&state, chest, v) {
                motion::set_velocity(v.wit());
            }
            if self.spidey.safe {
                motion::reset_fall();
            }
            self.last_chest = self.spidey.driving.then_some(chest);
            self.spidey.pose(&state, chest, player.yaw);
            // Wind, quiet: none below ~7 m/s, up to 0.3 of the elytra loop at 30 m/s.
            let speed = P::of(&state.velocity).len();
            if !state.on_ground {
                motion::set_wind(((speed - WIND_FROM) / (WIND_FULL - WIND_FROM)).clamp(0.0, 1.0) as f32 * WIND_MAX);
            }
            motion::set_ropes(&self.spidey.ropes(chest, player.yaw));
            return;
        }
        // Off: the game moves the player as always.
        self.last_chest = None;
        motion::set_ropes(&[]);
    }

    fn ui_select(&mut self, id: u32, _row: u32) {
        match id {
            BUTTON_DELAY => {
                // The next step up, back to 0 after the last.
                let now = self.spidey.web_delay;
                self.spidey.web_delay = DELAY_STEPS
                    .iter()
                    .copied()
                    .find(|d| *d > now + 1e-6)
                    .unwrap_or(DELAY_STEPS[0]);
            }
            BUTTON_JUMP => self.spidey.jump_start = !self.spidey.jump_start,
            _ => return,
        }
        self.save_settings();
        self.show_settings();
    }

    fn ui_settings_opened(&mut self) {
        self.show_settings();
    }

    fn command(&mut self, _name: &str, args: &str) -> Result<Option<String>, String> {
        if let Some(rest) = args.trim().strip_prefix("view") {
            // A fixed camera beside the player, for checking poses.
            let Some(p) = game::get_player() else {
                return Err("No player".into());
            };
            let chest = P::new(p.x, p.y + 1.0, p.z);
            let yaw = f64::from(p.yaw).to_radians();
            let ahead = P::new(-yaw.sin(), 0.0, yaw.cos());
            let right = ahead.cross(UP).norm();
            let at = match rest.trim() {
                "side" | "" => chest + right * VIEW_DISTANCE,
                "front" => chest + ahead * VIEW_DISTANCE,
                "back" => chest - ahead * VIEW_DISTANCE,
                "off" => {
                    self.view = None;
                    return Ok(Some("View off".into()));
                }
                other => return Err(format!("/swingmode view side|front|back|off, not {other}")),
            };
            self.view = Some(motion::Camera::Fixed(motion::FixedCamera {
                at: at.wit(),
                look_at: chest.wit(),
            }));
            return Ok(Some("Fixed view on: /swingmode view off".into()));
        }
        if args.trim() == "debug" {
            let st = self.sense.as_ref();
            return Ok(Some(format!(
                "aim {:?} fan {} wall {:?} above {:?} phase-wallrun {} blend {:.2} posed {} profile {:?} last vault {}",
                st.and_then(|s| s.aim.hit.as_ref().map(|h| ((P::of(h) - P::of(&s.eye)).len() * 10.0).round() / 10.0)),
                st.map_or(0, |s| s.fan.iter().filter(|p| p.hit.is_some()).count()),
                st.and_then(|s| s.wall.as_ref().map(|w| (w.x, w.z))),
                st.map(|s| s.wall_above),
                self.spidey.wall_running(),
                self.spidey.cam_blend(),
                self.spidey.posed(),
                st.map(|s| s.profile.iter().map(|h| h.map_or(-9.0, |h| (h * 10.0).round() / 10.0)).collect::<Vec<_>>()),
                self.spidey.vault_note
            )));
        }
        if let Some(rest) = args.trim().strip_prefix("pose") {
            self.spidey.set_debug_pose(rest.trim())?;
            return Ok(Some(format!("Pose held: {}", rest.trim())));
        }
        if let Some(rest) = args.trim().strip_prefix("delay") {
            let rest = rest.trim();
            if !rest.is_empty() {
                let k: f64 = rest
                    .parse()
                    .ok()
                    .filter(|k: &f64| (0.0..=2.0).contains(k))
                    .ok_or_else(|| format!("/swingmode delay 0..2 (1 = full, 0.5 = half), not {rest}"))?;
                self.spidey.web_delay = k;
                self.save_settings();
            }
            return Ok(Some(format!(
                "Web flight before it bites: {:.2} of full (/swingmode delay 0..2)",
                self.spidey.web_delay
            )));
        }
        if let Some(rest) = args.trim().strip_prefix("jump") {
            self.spidey.jump_start = match rest.trim() {
                "on" | "" => true,
                "off" => false,
                other => return Err(format!("/swingmode jump on|off, not {other}")),
            };
            self.save_settings();
            return Ok(Some(format!(
                "Jump into a swing from the ground: {}",
                if self.spidey.jump_start { "on" } else { "off" }
            )));
        }
        // The commands mirror the settings (`delay`, `jump`) plus ones with no
        // button (`view`, `pose`, `debug`); `on`/`off` switch the mod.
        self.mode = match args.trim() {
            "on" | "web" => Mode::Web,
            "off" => Mode::Off,
            "" => {
                return Ok(Some(
                    "/swingmode on|off · delay 0..2 · jump on|off (the settings page's) · view side|front|back|off · debug"
                        .into(),
                ));
            }
            other => return Err(format!("/swingmode on|off|delay|jump|view|debug, not {other}")),
        };
        self.last_chest = None;
        self.spidey.reset();
        chat::show(match self.mode {
            Mode::Web => "On: hold R to swing, let go to flip. Hold Z to zip to the crosshair, Space hauls",
            Mode::Off => "Off",
        });
        Ok(None)
    }
}

horizon_mod_sdk::export_mod!(Swing);
