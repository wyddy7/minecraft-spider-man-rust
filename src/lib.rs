//! Swing: web swinging and ODM gear for Horizon, on the local fork's
//! `motion` capability.
//!
//! The mod drives the player's velocity itself from the first hook until
//! the feet touch the ground: each tick it takes the real displacement of
//! the last tick (so walls and floors still stop you), adds its own gravity
//! and forces, and hands the game the result. The game's air drag (0.91
//! per tick sideways) never applies in flight, so speed carries from one
//! swing or hook to the next.
//!
//! - Web (Marvel's Spider-Man, 2018): hold R. The anchor is picked above and
//!   ahead from a fan of rays, biased by A/D; the swing is a pendulum with a
//!   short tug when the web bites; with no A/D the arc is pulled straight
//!   along the view; W pumps; letting go on the upswing throws you up and
//!   forward.
//! - ODM (as in Attack on Titan Tribute Game, keys as in Danny's AoT): G and
//!   H fire the left and right hooks, spread a little apart; they fly, bite
//!   and hold while the key is held (a tap still holds 0.3 s). Hooked with
//!   no gas, the cables pull softly and brake you moving away. Space burns
//!   gas: hooked, your speed is kept and only turned, to at most 88 degrees
//!   from the anchor (the midpoint of two hooks), plus a little more every
//!   tick, so you circle the anchor and gain speed; W reels in (a tighter
//!   angle: you dive at the anchor), S reels out, A/D push the orbit
//!   sideways. Unhooked, gas thrusts along WASD. Double-tap a direction in
//!   the air for a burst. Gravity halves while a hook is above you.
//!
//! Numbers come from a written description of AoTTG's ODM (Unity units per
//! 0.02 s, converted to blocks per 0.05 s tick); the ones marked "tuned"
//! are ours.
use horizon_mod_sdk::{
    Mod, Registration, chat, game,
    hud::{self, Corner, Line},
    input,
    motion::{self, Rope, Vec3 as V},
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
    /// `self` (a unit vector) turned toward the unit `to` by at most `max`
    /// radians, in the plane of the two.
    fn turn_toward(self, to: P, max: f64) -> P {
        let angle = self.dot(to).clamp(-1.0, 1.0).acos();
        if angle < 1e-6 {
            return self;
        }
        // The part of `to` across `self`: the plane's second axis.
        let across = (to - self * self.dot(to)).norm();
        if across.len() < 1e-9 {
            return self;
        }
        let t = angle.min(max);
        self * t.cos() + across * t.sin()
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

mod odm {
    /// AoTTG: gravity 20 u/s², 10 with an anchor above you.
    pub const GRAVITY: f64 = 0.05;
    pub const GRAVITY_ANCHORED: f64 = 0.025;
    pub const DAMP: f64 = 0.998;
    /// 150 u/s, plus your speed along the shot.
    pub const HOOK_SPEED: f64 = 7.5;
    /// A tapped hook still holds this long after it is fired: 0.3 s.
    pub const MIN_HOLD: u32 = 6;
    /// The soft pull toward the anchor with no gas: 10 u/s², 20 on one hook.
    pub const PULL: f64 = 0.025;
    pub const PULL_SINGLE: f64 = 0.05;
    /// Moving away from the anchor with no gas: -2·v per second.
    pub const BRAKE: f64 = 0.9;
    /// The orbit: the speed turned to at most 88.2 degrees from the anchor,
    /// times (1 + reel), reel in -0.8..0.8; +0.1 u/s every 0.02 s.
    pub const ORBIT_ANGLE: f64 = 1.539_38;
    pub const REEL_IN: f64 = -0.8;
    pub const REEL_OUT: f64 = 0.8;
    pub const SPEED_GAIN: f64 = 0.0125;
    /// A/D sideways push in the orbit (tuned).
    pub const STRAFE: f64 = 0.03;
    /// When a hook bites: a shove toward the anchor and a lift (tuned).
    pub const BITE: f64 = 0.1;
    pub const BITE_UP: f64 = 0.05;
    /// Unhooked gas along WASD: 30 u/s².
    pub const THRUST: f64 = 0.075;
    /// Burst: +40 u/s, 4 gas, 0.3 s apart; the speed above the speed before
    /// it bleeds off at 1.7·v per second for 0.5 s.
    pub const BURST: f64 = 2.0;
    pub const BURST_GAS: f64 = 4.0;
    pub const BURST_COOLDOWN: u32 = 6;
    pub const BURST_DECAY: f64 = 0.915;
    pub const BURST_TICKS: u32 = 10;
    /// Two presses of a direction this close: a burst (tuned).
    pub const DOUBLE_TAP: u64 = 6;
    /// Gas: 100, 0.2 a hook; per tick of gas and the refill on the ground
    /// are tuned (AoTTG refills at supply points).
    pub const TANK: f64 = 100.0;
    pub const HOOK_GAS: f64 = 0.2;
    pub const GAS_PER_TICK: f64 = 0.15;
    pub const REFILL: f64 = 1.0;
    pub const MAX_SPEED: f64 = 3.0;
    pub const COLOR: u32 = 0x40_454D;
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Web,
    Odm,
    Off,
}

#[derive(Clone, Copy)]
struct Hook {
    anchor: P,
    /// Ticks until the hook bites, and the whole flight.
    flying: u32,
    flight: u32,
    /// Ticks since it was fired.
    age: u32,
}
impl Hook {
    fn holds(&self) -> bool {
        self.flying == 0
    }
    /// Where the cable's end is drawn: in flight, part way from the eyes.
    fn end(&self, eye: P) -> P {
        if self.holds() {
            self.anchor
        } else {
            let done = 1.0 - f64::from(self.flying) / f64::from(self.flight.max(1));
            eye + (self.anchor - eye) * done
        }
    }
}

const DIRS: [&str; 4] = ["KeyW", "KeyA", "KeyS", "KeyD"];

struct Swing {
    mode: Mode,
    /// Web: one rope. ODM: left (G) and right (H).
    hooks: [Option<Hook>; 2],
    /// Hook keys held last tick.
    held: [bool; 2],
    /// The chest last tick while the mod drives.
    last_chest: Option<P>,
    /// From the first hook until the feet touch the ground: the mod drives
    /// the velocity and there is no fall damage.
    flying: bool,
    gas: f64,
    /// WASD held last tick, and the tick each was last pressed.
    dirs_held: [bool; 4],
    pressed_at: [u64; 4],
    burst_cooldown: u32,
    /// The burst's leftover: ticks, and the speed before it.
    burst_left: u32,
    burst_base: f64,
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
const MOD_KEYS: [(&str, &str, &str, &str); 4] = [
    ("web", "Web swing", "key.keyboard.r", "KeyR"),
    ("zip", "Web zip", "key.keyboard.z", "KeyZ"),
    ("hook_left", "ODM left hook", "key.keyboard.g", "KeyG"),
    ("hook_right", "ODM right hook", "key.keyboard.h", "KeyH"),
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

    fn active(&self) -> bool {
        self.hooks.iter().any(Option::is_some)
    }

    fn tick_odm(&mut self, state: &Sense, chest: P, mut v: P) -> P {
        let look = P::of(&state.look);
        let right = look.cross(UP).norm();
        let gas = key(state, "Space") && !state.on_ground && self.gas > 0.0;
        // The hooks: fired on press along the spread rays, held while the key is.
        let keys = [key(state, "KeyG"), key(state, "KeyH")];
        let aims = [&state.aim_left, &state.aim_right];
        for side in 0..2 {
            if keys[side] && !self.held[side] && self.gas >= odm::HOOK_GAS {
                if let Some(hit) = aims[side].hit.as_ref().or(state.aim.hit.as_ref()) {
                    let anchor = P::of(hit);
                    let shot = anchor - chest;
                    let speed = odm::HOOK_SPEED + v.dot(shot.norm()).max(0.0);
                    let flight = (shot.len() / speed).ceil().max(1.0) as u32;
                    self.hooks[side] = Some(Hook {
                        anchor,
                        flying: flight,
                        flight,
                        age: 0,
                    });
                    self.gas -= odm::HOOK_GAS;
                    self.flying = true;
                }
            }
            let other_holds = self.hooks[1 - side].is_some_and(|h| h.holds());
            if let Some(h) = self.hooks[side].as_mut() {
                h.age += 1;
                if h.flying > 0 {
                    h.flying -= 1;
                    if h.flying == 0 {
                        // The bite: a shove toward the anchor and a lift.
                        let k = if other_holds { 0.8 } else { 1.0 };
                        v = v + (h.anchor - chest).norm() * (odm::BITE * k) + UP * odm::BITE_UP;
                    }
                }
                if !keys[side] && h.age >= odm::MIN_HOLD {
                    self.hooks[side] = None;
                }
            }
            self.held[side] = keys[side];
        }
        let holding: Vec<P> = self
            .hooks
            .iter()
            .flatten()
            .filter(|h| h.holds())
            .map(|h| h.anchor)
            .collect();
        let anchor = match holding.as_slice() {
            [a] => Some(*a),
            [a, b] => Some((*a + *b) * 0.5),
            _ => None,
        };
        let gravity = if anchor.is_some_and(|a| a.y > chest.y) {
            odm::GRAVITY_ANCHORED
        } else {
            odm::GRAVITY
        };
        v = v * odm::DAMP - UP * gravity;
        match anchor {
            Some(a) if gas => {
                // The orbit: keep the speed, turn it to at most the angle
                // from the anchor, a little faster every tick.
                let to = (a - chest).norm();
                let reel = if key(state, "KeyW") {
                    odm::REEL_IN
                } else if key(state, "KeyS") {
                    odm::REEL_OUT
                } else {
                    0.0
                };
                let dir = if v.len() > 0.05 { v.norm() } else { look };
                let speed = v.len() + odm::SPEED_GAIN;
                v = to.turn_toward(dir, odm::ORBIT_ANGLE * (1.0 + reel)) * speed;
                v = v + right * ((axis(state, "KeyD") - axis(state, "KeyA")) * odm::STRAFE);
                self.gas -= odm::GAS_PER_TICK;
            }
            Some(a) => {
                let to = (a - chest).norm();
                let pull = if holding.len() == 1 {
                    odm::PULL_SINGLE
                } else {
                    odm::PULL
                };
                v = v + to * pull;
                if v.dot(to) < 0.0 {
                    v = v * odm::BRAKE;
                }
            }
            None if gas => {
                let wasd = look * (axis(state, "KeyW") - axis(state, "KeyS"))
                    + right * (axis(state, "KeyD") - axis(state, "KeyA"));
                let dir = if wasd.len() > 0.0 { wasd.norm() } else { look };
                v = v + dir * odm::THRUST;
                self.gas -= odm::GAS_PER_TICK;
            }
            None => {}
        }
        // Burst: a direction pressed twice quickly in the air.
        self.burst_cooldown = self.burst_cooldown.saturating_sub(1);
        for (i, name) in DIRS.iter().enumerate() {
            let down = key(state, name);
            if down && !self.dirs_held[i] {
                let quick = self.now - self.pressed_at[i] <= odm::DOUBLE_TAP;
                if quick
                    && !state.on_ground
                    && self.burst_cooldown == 0
                    && self.gas >= odm::BURST_GAS
                {
                    let dir = [look, right * -1.0, look * -1.0, right][i];
                    self.burst_base = v.len();
                    v = v + dir * odm::BURST;
                    self.gas -= odm::BURST_GAS;
                    self.burst_cooldown = odm::BURST_COOLDOWN;
                    self.burst_left = odm::BURST_TICKS;
                    self.flying = true;
                }
                self.pressed_at[i] = self.now;
            }
            self.dirs_held[i] = down;
        }
        if self.burst_left > 0 {
            self.burst_left -= 1;
            let speed = v.len();
            if speed > self.burst_base {
                v = v.norm() * (self.burst_base + (speed - self.burst_base) * odm::BURST_DECAY);
            }
        }
        self.gas = self.gas.max(0.0);
        if state.on_ground {
            self.gas = (self.gas + odm::REFILL).min(odm::TANK);
        }
        v
    }

    fn ropes(&self, eye: P) -> Vec<Rope> {
        let color = if self.mode == Mode::Web {
            web::COLOR
        } else {
            odm::COLOR
        };
        self.hooks
            .iter()
            .flatten()
            .map(|h| Rope {
                to: h.end(eye).wit(),
                color,
                progress: 1.0,
                wave: 0.0,
                from: None,
            })
            .collect()
    }

    fn panel(&mut self) {
        let lines: Vec<String> = match self.mode {
            Mode::Off => Vec::new(),
            Mode::Web => vec!["Web: hold R, A/D side. Hold Z: pull, A/D orbit, Space yank".into()],
            Mode::Odm => {
                let bar = (self.gas / odm::TANK * 20.0).round() as usize;
                vec![
                    "ODM: G / H hooks, Space gas, W/S reel, A/D orbit".into(),
                    format!("Gas [{}{}]", "|".repeat(bar), ".".repeat(20 - bar)),
                ]
            }
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
                hooks: [None, None],
                held: [false; 2],
                last_chest: None,
                flying: false,
                gas: odm::TANK,
                dirs_held: [false; 4],
                pressed_at: [0; 4],
                burst_cooldown: 0,
                burst_left: 0,
                burst_base: 0.0,
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
            self.hooks = [None, None];
            self.last_chest = None;
            self.flying = false;
            self.sense = None;
            return;
        };
        let state = self.rays.read(&st, player.y);
        self.rays.ask(&state, player.yaw);
        self.sense = Some(state.clone());
        let chest = P::new(player.x, player.y + CHEST, player.z);
        let eye = P::of(&state.eye);
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
        // In flight, the velocity is last tick's real displacement (walls cut
        // it); otherwise the game's own.
        let v = match self.last_chest {
            Some(last) if self.flying => chest - last,
            _ => P::of(&state.velocity),
        };
        let v = match self.mode {
            Mode::Off | Mode::Web => v,
            Mode::Odm => self.tick_odm(&state, chest, v),
        };
        if self.flying && state.on_ground && !self.active() && self.burst_left == 0 {
            self.flying = false;
        }
        if self.flying && self.mode != Mode::Off {
            let max = if self.mode == Mode::Web {
                web::MAX_SPEED
            } else {
                odm::MAX_SPEED
            };
            motion::set_velocity(v.clamp(max).wit());
            motion::reset_fall();
            self.last_chest = Some(chest);
        } else {
            self.last_chest = None;
        }
        motion::set_ropes(&self.ropes(eye));
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
        self.mode = match args.trim() {
            "web" => Mode::Web,
            "odm" => Mode::Odm,
            "off" => Mode::Off,
            "" => {
                return Ok(Some(
                    "/swingmode web: R swing, hold Z web pull, Space yank. /swingmode odm: G and H hooks, Space gas. /swingmode off."
                        .into(),
                ));
            }
            other => return Err(format!("Unknown mode {other}: web, odm or off")),
        };
        self.hooks = [None, None];
        self.held = [false; 2];
        self.flying = false;
        self.spidey.reset();
        chat::show(match self.mode {
            Mode::Web => "Web: hold R to swing, let go to flip. Hold Z: web pull, A/D orbit, Space yanks the web",
            Mode::Odm => "ODM gear: G and H fire hooks, Space burns gas, W/S reel",
            Mode::Off => "Off",
        });
        Ok(None)
    }
}

horizon_mod_sdk::export_mod!(Swing);
