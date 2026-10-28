// What game and plug exchange each tick: Horizon's `motion` capability as
// data. Names follow the mod SDK's generated types, so the SDK side can
// re-export them as they are.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// A ray the game casts at the start of every tick (see `Output::rays`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ray {
    /// From the eyes, else from the feet; then moved by `offset`.
    pub from_eyes: bool,
    pub offset: Vec3,
    /// Unit direction.
    pub dir: Vec3,
    pub distance: f64,
    /// Against collision boxes, else against outlines.
    pub solid: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// Blocks per tick.
    pub velocity: Vec3,
    pub on_ground: bool,
    pub eye: Vec3,
    /// Unit view vector.
    pub look: Vec3,
    /// For each ray of the last `rays`, cast at the start of this tick.
    pub hits: Vec<Option<f64>>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Player {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub health: f32,
    pub food: u32,
    pub dimension: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rope {
    pub to: Vec3,
    /// 0xRRGGBB.
    pub color: u32,
    pub progress: f32,
    pub wave: f32,
    pub from: Option<Vec3>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartPose {
    pub part: String,
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
    pub pivot_y: Option<f32>,
    pub pivot_z: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pose {
    pub parts: Vec<PartPose>,
    pub pitch: f32,
    pub roll: f32,
    pub twist: f32,
    pub facing: Option<f32>,
    pub shift: Option<Vec3>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChaseCamera {
    pub distance: f32,
    pub height: f32,
    pub side: f32,
    pub wall: Option<Vec3>,
    pub wall_blend: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FixedCamera {
    pub at: Vec3,
    pub look_at: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Camera {
    Chase(ChaseCamera),
    Fixed(FixedCamera),
}

/// A sticky setting: left as it was, or set (possibly to none) this tick.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Set<T> {
    #[default]
    Keep,
    To(T),
}

/// What the game gives the plug each tick.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Input {
    pub tick: u64,
    /// None on the title screen and on another server.
    pub state: Option<State>,
    pub player: Option<Player>,
    /// Physical keys held while playing, as winit names them (`KeyR`).
    pub keys: Vec<String>,
    /// Chat commands for the plug since last tick: name, then the rest.
    pub commands: Vec<(String, String)>,
}

/// What the plug asks of the game after a tick. One-tick requests
/// (`velocity`, `reset_fall`, `shake`, `chat`) apply once; the `Set` ones
/// stay until set again.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Output {
    /// Replaces the player's velocity for its next move.
    pub velocity: Option<Vec3>,
    /// Forget the fall so far, for this tick.
    pub reset_fall: bool,
    pub rays: Set<Vec<Ray>>,
    pub ropes: Set<Vec<Rope>>,
    /// None gives the player's model back to the game's animation.
    pub pose: Set<Option<Pose>>,
    /// None gives the game's own third-person camera.
    pub camera: Set<Option<Camera>>,
    /// Wind loudness, 0..0.5 of the elytra loop.
    pub wind: Set<f32>,
    /// Strength and seconds.
    pub shake: Option<(f32, f32)>,
    pub chat: Vec<String>,
    /// The mod's HUD panel lines (text, 0xRRGGBB); none clears it.
    pub panel: Set<Option<Vec<(String, u32)>>>,
}

/// The sticky part of every output so far: what per-frame hooks draw.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Current {
    pub rays: Vec<Ray>,
    pub ropes: Vec<Rope>,
    pub pose: Option<Pose>,
    pub camera: Option<Camera>,
    pub wind: f32,
    pub panel: Option<Vec<(String, u32)>>,
}

impl Current {
    pub fn apply(&mut self, out: &Output) {
        fn set<T: Clone>(slot: &mut T, value: &Set<T>) {
            if let Set::To(v) = value {
                *slot = v.clone();
            }
        }
        set(&mut self.rays, &out.rays);
        set(&mut self.ropes, &out.ropes);
        set(&mut self.pose, &out.pose);
        set(&mut self.camera, &out.camera);
        set(&mut self.wind, &out.wind);
        set(&mut self.panel, &out.panel);
    }
}

// dev-only:begin (the library boundary; a shipped mod calls the mod directly)
/// The plug's exported entry: JSON `Input` in, JSON `Output` out, the
/// output written through `write` (called once, with `ctx`).
pub type PlugCall = unsafe extern "C" fn(
    input: *const u8,
    input_len: usize,
    write: extern "C" fn(ctx: *mut core::ffi::c_void, bytes: *const u8, len: usize),
    ctx: *mut core::ffi::c_void,
);

pub const PLUG_SYMBOL: &core::ffi::CStr = c"horizon_plug_call";
// dev-only:end
