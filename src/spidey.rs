//! The `web` mode, built to the reference gameplay (`docs/video-spec.md`):
//! the web flies before it holds, the body hangs along the rope, letting go
//! in the air turns into one forward flip and then a spread-eagle fall, and
//! Z zips to the aimed point with the body along the web.
use super::{P, UP, axis, key, pick_anchor, web};
use crate::sense::Sense;
use horizon_mod_sdk::motion::{self, PartPose, Rope};
use std::f32::consts::{PI, TAU};

/// How fast a web flies, blocks per tick: about 4 ticks (0.2 s) to a
/// 20-block anchor, the reference's 6–8 frames.
const WEB_SPEED: f64 = 5.0;
/// A far web still bites within 0.4 s (reach is 250 blocks).
const MAX_FLIGHT: f64 = 8.0;
/// A flight of `ticks` scaled by the player's delay setting; at least one
/// tick, since the bite happens when the count runs out.
fn flight_ticks(ticks: f64, delay: f64) -> u32 {
    (ticks * delay).ceil().max(1.0) as u32
}
/// How much of the anchor's sideways offset the swing keeps (the rest is
/// bent out, over the street), and how far below you the arc's bottom may
/// sink when the web bites.
const WALL_SIDE: f64 = 0.2;
const DROP: f64 = 4.0;
/// The web's stretch after it bites: how much of the overshoot is taken
/// back on the first tick, and the ticks until it is firm.
const STRETCH_MIN: f64 = 0.06;
const STRETCH_TICKS: f64 = 14.0;
/// The invisible floor: the arc's bottom (the feet) this high above the
/// ground at least, and how fast the rope shortens to keep it.
const CLEARANCE: f64 = 2.5;
const FLOOR_TUG: f64 = 0.35;
/// The most the rope shortens in a tick when it bites.
const TUG_MAX: f64 = 0.25;
/// The flying web's zigzag, blocks.
const WEB_WAVE: f32 = 0.35;
/// The zip: hold Z, the web flies to the aimed point, and when it bites
/// the old momentum goes (Danny's AoT recording, frames 2827–2847: on a new
/// hook the player all but stops, then goes for the new point) and you ride
/// an upper arc to the point (playtest: not a fall into the trees
/// but the top half of a parabola). Speed builds from `ZIP_START` by
/// `ZIP_RAMP` a tick to 1.5 blocks a tick (30 m/s, the recording's speed);
/// Space hauls on the web for more. The arc's height is a share of the
/// distance, within limits. Letting go of Z on the way keeps the momentum;
/// arriving hops on past the point.
const HOOK_SPEED: f64 = 7.5;
const ZIP_START: f64 = 0.3;
const ZIP_RAMP: f64 = 0.2;
const ZIP_PATH_SPEED: f64 = 1.5;
const ZIP_HAUL: f64 = 1.35;
/// The arc's shape (from a sketch): one throw-like curve whose
/// top is at the higher end. A point above you is reached from below,
/// arriving level with it (then a vault over the edge); a point below you
/// is left level and fallen onto. A point about level with you gets a bump
/// of `ARC_BUMP` of the distance (within limits), over the line, or under
/// it when you were already falling as the web bit.
const ARC_BUMP: f64 = 0.08;
const ARC_BUMP_MIN: f64 = 0.5;
const ARC_BUMP_MAX: f64 = 6.0;
const ZIP_CARRY: f64 = 0.6;
const ZIP_HOP: f64 = 0.45;
/// Point launch: forward (plus some of the arrival speed) and up.
const LAUNCH_FORWARD: f64 = 1.1;
const LAUNCH_UP: f64 = 0.95;
/// Space on a swing: the speed-up a tick; let go of Space: the jump off.
const SWING_BOOST: f64 = 0.04;
const JUMP_OFF_FORWARD: f64 = 0.3;
const JUMP_OFF_UP: f64 = 0.55;
/// A roof run after a landing: the speed kept, how long, the hop over a low
/// block, and the long jump off the roof on Space.
const ROOF_KEEP: f64 = 0.85;
const ROOF_TICKS: u32 = 30;
const ROOF_HOP: f64 = 0.55;
const ROOF_LEAP_UP: f64 = 0.75;
/// The parkour run on the ground (R and W held): its top speed (blocks a
/// tick; with the game's own push from W about 12 m/s, against its 5.6
/// sprint), the speed added a tick on
/// the way there, how far A/D lean it, and a jump's lift.
const RUN_SPEED: f64 = 0.5;
const RUN_ACCEL: f64 = 0.03;
const RUN_SIDE: f64 = 0.5;
const RUN_JUMP: f64 = 0.42;
/// Ground parkour: heights (blocks over the feet) that split the moves —
/// below `VAULT_MIN` the game's own step-up, above `VAULT_WALL` a building
/// (the wall needs R, decision 18); an obstacle this long is landed on and
/// run across; the rise speed of each move (the reference's ~0.25 s up 1–2
/// blocks, ~0.9 s up a 4-block trailer), the gap kept to the face while
/// rising, and the clearance over the top.
const VAULT_MIN: f64 = 0.6;
const GROUND_FRICTION: f64 = 0.546;
/// The arc over a short obstacle: a floatier jump (the reference's vault
/// sweeps nearly flat over the top); the drop after it falls as the game
/// does; off a hand vault or a climb, enough lift that the flip has time.
const ARC_GRAVITY: f64 = 0.05;
const DROP_GRAVITY: f64 = 0.08;
/// Ticks the drop with the flip should take (`AM6jwca0Akg` 171.6–171.9 s,
/// 151.5–151.9 s: ~0.4 s).
const FLIP_DROP: f64 = 8.0;
/// The speed over a short obstacle, at least.
const OVER_STEP: f64 = 0.32;
const OVER_HAND: f64 = 0.4;
const VAULT_STEP: f64 = 1.6;
const VAULT_HAND: f64 = 2.7;
const VAULT_WALL: f64 = 5.5;
const VAULT_LONG: f64 = 3.0;
const RISE_STEP: f64 = 0.4;
const RISE_HAND: f64 = 0.45;
const RISE_CLIMB: f64 = 0.25;
const FACE_GAP: f64 = 0.35;
const VAULT_CLEAR: f64 = 0.3;
/// Reaching a point this much higher than the start is a climb to a ledge:
/// the hop is a vault over its edge.
const VAULT_RISE: f64 = 1.5;
const ZIP_VAULT: f64 = 0.75;
/// Closer than this to the point, the zip is done.
const ZIP_HOLD: f64 = 1.5;
/// Running a wall on a zip: the least speed, and the push into the wall
/// that keeps the contact.
const WALL_RUN: f64 = 0.8;
const WALL_PRESS: f64 = 0.2;
/// Hanging at the point: how far below it the chest stays.
const ZIP_HANG: f64 = 1.0;
/// Hitting a wall in flight slides along it (the recording: up and along
/// the wall at 18–19 m/s instead of stopping): this share of the speed is
/// kept, turned along the wall, and up when there is nowhere else to go.
const WALL_KEEP: f64 = 0.9;
/// One forward flip after letting go, in ticks (reference: 15–25 frames).
const FLIP_TICKS: u32 = 14;
/// How much of the way to the new pose each tick covers.
const EASE: f32 = 0.35;
/// A pose left on the ground eases out for at most this many ticks.
const EASE_OUT_TICKS: u32 = 8;

struct Web {
    /// Where the web is stuck (drawn), and where the swing hangs from: the
    /// anchor moved sideways over the street, into the plane of flight.
    anchor: P,
    pivot: P,
    length: f64,
    target: f64,
    tug_step: f64,
    /// Ticks until it holds, and the whole flight.
    flying: u32,
    flight: u32,
    /// Ticks since it bit: the web is elastic at first.
    held: u32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    Ground,
    Air,
    Flip(u32),
    Fall,
    /// Running up a wall, toward the unit direction of the wall.
    WallRun(P),
    /// Smacked into a wall too fast (Web of Shadows): stuck flat for a
    /// moment, then sliding off; ticks so far.
    Slap(P, u32),
    /// Over an obstacle on the ground: the move, past the rise yet, ticks.
    Vault(Move, bool, u32),
}

/// Ground parkour by the obstacle's height (`AM6jwca0Akg`, video-spec
/// "Ground parkour"): a step / speed vault over ~1 block, a hand vault over
/// ~2, a run up the face over 3 and more.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Move {
    Step,
    Hand,
    Climb,
}

/// A vault under way: the flat direction, the feet at the start, how far
/// the obstacle's face and far end are along it, its top over the start, the
/// speed across, and whether it is long enough to land on.
#[derive(Clone, Copy)]
struct Vault {
    kind: Move,
    dir: P,
    start: P,
    front: f64,
    back: f64,
    top: f64,
    speed: f64,
    long: bool,
    ticks: u32,
    stuck: u32,
    /// Over a short one: the arc's vertical speed now (None: still rising).
    arc_vy: Option<f64>,
    /// Off a hand vault or a climb: ticks into the flip on the drop and how
    /// long the drop should take.
    drop: Option<(u32, f64, bool)>,
}

/// Wall run: climbing speed (1 block a tick, 20 m/s), the push into the
/// wall that keeps contact, sideways speed, the kick off it, and the vault
/// over the top edge.
const WALL_CLIMB: f64 = 1.0;
const WALL_HUG: f64 = 0.15;
const WALL_RAMP: f64 = 0.3;
/// Two W presses this close (ticks) sprint on a wall.
const SPRINT_TAP: u32 = 7;
/// Entry speed kept up to this times the run's, bled off per tick.
const WALL_ENTRY_BOOST: f64 = 1.6;
const WALL_BLEED: f64 = 0.03;
/// Parkour in flight: how far ahead (ticks) obstacles count, the height to
/// clear over one, the most lift a tick, and the duck under a ceiling.
const DODGE_TICKS: f64 = 12.0;
const DODGE_CLEAR: f64 = 2.2;
const DODGE_LIFT: f64 = 0.25;
const DODGE_DUCK: f64 = 0.04;
/// The slap: speed into the wall that makes one (blocks a tick, ~36 m/s;
/// 1.2 slapped too often in playtests),
/// ticks stuck, ticks sliding and the slide's speed.
const SLAP_SPEED: f64 = 1.8;
const SLAP_STICK: u32 = 10;
const SLAP_SLIDE: u32 = 12;
const SLAP_SLIDE_SPEED: f64 = 0.06;
/// The wall-as-floor camera: the wall above you that makes a run long
/// enough for it (blocks), and the turn per tick (0.6 s for the 90 degrees).
pub const LONG_WALL: f64 = 12.0;
pub const WALL_CAM_STEP: f32 = 1.0 / 12.0;
/// Walking the wall (R let go): blocks a tick (~5 m/s).
const WALL_WALK: f64 = 0.25;
/// The wall-run stride: ticks per step pair, leg and arm swing (radians),
/// and the lean into the wall.
/// How far the lying body is moved off the wall: the reach of the feet
/// from the middle (0.9) less the gap to the wall (half the 0.6 width).
const WALL_GAP: f64 = 0.3;
/// And up: the lying body sat at the feet's height, below the middle of
/// the player (screenshots 2026-10-28).
/// The body turned back 90 degrees, less a little lean of the head toward
/// the wall.
/// The body's turn off the wall (radians from upright): the torso low over
/// the wall, ~25 degrees from it, leaning into the run (from a sketch,
/// 2026-10-28; Spider-Man 2 426.5–428.9 s).
/// Standing off the wall (the body perpendicular to it), the lean into a
/// full run (radians), and limb swing per block walked (Minecraft's walk
/// animation runs ~0.67 radians per 1/4 block of movement).
const WALL_UPRIGHT: f32 = std::f32::consts::FRAC_PI_2;
const WALL_RUN_LEAN: f32 = 0.6;
const WALL_WALK_LEAN: f32 = 0.08;
/// Which way the roll tips for a lean to the right (checked on screen).
const WALL_LEAN_ROLL_SIGN: f32 = 1.0;
const LIMB_PER_BLOCK: f32 = 2.6;
const STRIDE_MAX_STEP: f32 = 0.95;
/// The scramble: legs swing about a raised knee, arms about a reach up the
/// wall (radians, `ModelPart` xRot; negative is forward and up).
const WALL_KICK: f64 = 0.7;
const WALL_KICK_UP: f64 = 0.45;
const LEDGE_OVER: f64 = 0.45;
/// Climbing into an overhang (a ledge or a slab over the head while the wall
/// goes on, or a terrace floor jutting out under the roof): this many ticks
/// pressed up against it, then out from the wall for `OVERHANG_OUT_TICKS` at
/// `OVERHANG_OUT` a tick, level (past a lip up to ~2 blocks deep; NYC's tower
/// terrace at -641 93 -88 is 2), then up past it and back in over it.
const OVERHANG_TICKS: u32 = 4;
const OVERHANG_OUT_TICKS: u32 = 10;
const OVERHANG_OUT: f64 = 0.3;
const OVERHANG_UP: f64 = 0.85;
const LEDGE_UP: f64 = 0.55;

/// R held from the ground fires the web once off it: on a jump, or with
/// more than this many blocks below the feet (a curb or a hop is less).
const ARM_DROP: f64 = 2.5;
/// A web shot within this many blocks of the ground gets the low arc.
const GROUND_ARC: f64 = 3.0;
/// When a web shot from the ground bites: a pull along it.
const GROUND_TUG: f64 = 0.7;
/// From the ground: the arc's bottom this far above the chest (a negative
/// drop), and the faster shortening.
const GROUND_DROP: f64 = -1.5;
const GROUND_TUG_MAX: f64 = 0.8;

pub struct Spidey {
    /// How long a web or a zip line flies before it bites, as a share of
    /// its natural flight (`/swingmode delay`): 1 = the reference's 6–8
    /// frames, 0 = it bites on the next tick. Default half: Spider-Man 2
    /// itself cuts the wait by about that much.
    pub web_delay: f64,
    /// A jolt of the camera that fades: a slap sets it to 1.
    pub impact: f64,
    cam_wall: Option<P>,
    cam_blend: f32,
    /// A phase whose pose is held for checking (`/swingmode pose`).
    debug: Option<Phase>,
    ground_start: bool,
    web: Option<Web>,
    zip: Option<P>,
    r_held: bool,
    /// R held from the ground: the web goes once you are off it.
    web_armed: bool,
    /// Off the ground by a jump (Space), not a step off an edge.
    jumped: bool,
    z_held: bool,
    phase: Phase,
    /// From the first web until the feet touch the ground: the mod drives.
    pub driving: bool,
    ticks: u32,
    /// The pose last sent: parts by name, then the whole body's pitch and roll.
    shown: Vec<(String, [f32; 3])>,
    body: [f32; 2],
    /// The velocity set last tick, to see what a wall took from it.
    asked: Option<P>,
    /// Space last tick on the wall, and the body's turn toward its run.
    wall_space: bool,
    run_twist: f32,
    /// Ticks the wall run has pushed up against something over the head,
    /// and the way out from under it: the wall, ticks gone.
    wall_blocked: u32,
    overhang: Option<(P, u32)>,
    /// Sprint on the wall: W last tick, the tick of the last W press, latched.
    w_was: bool,
    w_last: u32,
    sprint: bool,
    /// The velocity along the wall now.
    wall_vel: P,
    /// The wall walk's limb phase and the body's lean now.
    limb_phase: f32,
    wall_lean: f32,
    /// Ticks before the next hop may flip.
    hop_cool: u32,
    /// Space last tick (point launch), and the roof run: ticks left and speed.
    space_launch: bool,
    /// Space last tick (a release jumps off a web or a zip).
    space_was: bool,
    roof_ticks: u32,
    roof_speed: f64,
    /// The parkour run's speed now (see `ground_run`).
    run_speed: f64,
    /// The wall run's vertical speed now.
    climb: f64,
    /// The zip hook: ticks until it bites, its whole flight, and ticks since
    /// it bit.
    zip_flying: u32,
    zip_flight: u32,
    zip_ticks: u32,
    /// The arc: where it starts, its bump at the middle (signed), how far
    /// along (0..1), how long it is.
    arc_from: P,
    arc_height: f64,
    arc_t: f64,
    arc_len: f64,
    /// At the point, hanging: the arrival direction (level) and speed.
    zip_hold: Option<(P, f64)>,
    /// The zip's velocity asked last tick, and whether a wall is being run.
    zip_asked: Option<P>,
    zip_walled: bool,
    /// No fall damage: from a web or a hook until the feet touch the ground.
    pub safe: bool,
    /// A vault over an obstacle on the ground, under way.
    vault: Option<Vault>,
    /// Webs shot so far, for `/swingmode debug`.
    pub webs: u32,
    /// The last vault picked, for `/swingmode debug`.
    pub vault_note: String,
    /// Ticks a held pose has been easing out on the ground.
    ease_out: u32,
}

impl Default for Spidey {
    fn default() -> Self {
        Spidey {
            web_delay: 0.5,
            impact: 0.0,
            cam_wall: None,
            cam_blend: 0.0,
            debug: None,
            ground_start: false,
            web: None,
            zip: None,
            r_held: false,
            web_armed: false,
            jumped: false,
            z_held: false,
            phase: Phase::Ground,
            driving: false,
            ticks: 0,
            shown: Vec::new(),
            body: [0.0; 2],
            asked: None,
            climb: 0.0,
            wall_space: false,
            wall_blocked: 0,
            overhang: None,
            run_twist: 0.0,
            w_was: false,
            w_last: 0,
            sprint: false,
            wall_vel: P::default(),
            limb_phase: 0.0,
            wall_lean: std::f32::consts::FRAC_PI_2,
            hop_cool: 0,
            space_launch: false,
            space_was: false,
            roof_ticks: 0,
            roof_speed: 0.0,
            run_speed: 0.0,
            zip_flying: 0,
            zip_flight: 0,
            zip_ticks: 0,
            arc_from: P::default(),
            arc_height: 0.0,
            arc_t: 0.0,
            arc_len: 0.0,
            zip_hold: None,
            zip_asked: None,
            zip_walled: false,
            safe: false,
            vault: None,
            webs: 0,
            vault_note: String::new(),
            ease_out: 0,
        }
    }
}

/// The whole-body pitch and roll that turn the body's up onto `up`, for a
/// player facing `yaw` (the renderer turns the model by 180 - yaw first,
/// so the model's forward is -z and its right +x).
fn tilt(up: P, yaw: f32) -> [f32; 2] {
    let t = f64::from(180.0 - yaw).to_radians();
    // The inverse of the model's turn: rotate by -t about y.
    let (s, c) = ((-t).sin(), (-t).cos());
    let u = P::new(up.x * c + up.z * s, up.y, -up.x * s + up.z * c).norm();
    [u.z.atan2(u.y) as f32, (-u.x).clamp(-1.0, 1.0).asin() as f32]
}

/// The arm's xRot and yRot that point it along `dir` for a player facing
/// `yaw` (0 hangs down, -PI/2 points ahead).
fn aim_arm(dir: P, yaw: f32) -> (f32, f32) {
    let pitch = -(dir.y.clamp(-1.0, 1.0).asin() as f32) - PI / 2.0;
    let turn = dir.x.atan2(dir.z) as f32 + yaw.to_radians();
    let turn = (turn + PI).rem_euclid(TAU) - PI;
    (pitch, -turn.clamp(-1.2, 1.2))
}

/// A point of the posed body (`local`: model space from the feet, +x the
/// right side, -z ahead) in the world, from the feet: the whole-body turn
/// about the middle, then the model's turn to the player's facing.
fn body_point(local: P, body: [f32; 2], yaw: f32) -> P {
    let mid = P::new(0.0, 0.9, 0.0);
    let v = local - mid;
    let (pitch, roll) = (f64::from(body[0]), f64::from(body[1]));
    let v = P::new(v.x * roll.cos() - v.y * roll.sin(), v.x * roll.sin() + v.y * roll.cos(), v.z);
    let v = P::new(v.x, v.y * pitch.cos() - v.z * pitch.sin(), v.y * pitch.sin() + v.z * pitch.cos());
    let v = v + mid;
    let t = f64::from(180.0 - yaw).to_radians();
    P::new(v.x * t.cos() + v.z * t.sin(), v.y, -v.x * t.sin() + v.z * t.cos())
}

/// The right hand with the arm raised along the body, and the right shoulder.
const HAND_UP: P = P::new(0.22, 1.95, 0.0);
const SHOULDER: P = P::new(0.22, 1.29, 0.0);

/// A point of the zip's arc: across, straight from `from` to `to`; up and
/// down, a parabola with its top at the higher end, plus `bump` (signed) at
/// the middle.
fn arc_point(from: P, to: P, bump: f64, t: f64) -> P {
    let dy = to.y - from.y;
    let rise = if dy > 0.0 {
        dy * (1.0 - (1.0 - t) * (1.0 - t))
    } else {
        dy * t * t
    };
    let across = (to - from).flat() * t;
    P::new(from.x + across.x, from.y + rise + bump * 4.0 * t * (1.0 - t), from.z + across.z)
}

/// The bump for a point about level with you: it fades as the height
/// difference grows against the distance across.
fn arc_bump(from: P, to: P, falling: bool) -> f64 {
    let across = (to - from).flat().len();
    let level = (1.0 - (to.y - from.y).abs() / (across * 0.5 + 1.0)).max(0.0);
    let size = (across * ARC_BUMP).clamp(ARC_BUMP_MIN, ARC_BUMP_MAX) * level;
    if falling { -size } else { size }
}

/// The arc's length, summed over 16 pieces.
fn arc_length(from: P, to: P, height: f64) -> f64 {
    (0..16)
        .map(|i| {
            let (a, b) = (f64::from(i) / 16.0, f64::from(i + 1) / 16.0);
            (arc_point(from, to, height, b) - arc_point(from, to, height, a)).len()
        })
        .sum()
}

impl Spidey {
    /// The roof run: on the ground after a fast landing, W keeps the speed
    /// along the view, a low block is hopped, Space leaps off.
    fn roof_run(&mut self, state: &Sense, look: P) -> Option<P> {
        if self.roof_ticks == 0 || self.driving {
            return None;
        }
        if !key(state, "KeyW") || self.web.is_some() || self.zip.is_some() {
            self.roof_ticks = 0;
            return None;
        }
        self.roof_ticks -= 1;
        let dir = look.flat().norm();
        let speed = self.roof_speed;
        self.roof_speed *= 0.985;
        let mut v = dir * speed;
        if !state.on_ground {
            return None;
        }
        // A block at the feet or knees ahead within ~4 ticks: hop it, if R
        // (the parkour run) is held.
        let near = |i: usize| state.ahead.get(i).copied().flatten().is_some_and(|d| d / speed.max(0.1) < 4.0);
        if key(state, "KeyR") && near(1) && !near(3) {
            v = v + UP * ROOF_HOP;
        }
        if key(state, "Space") {
            self.roof_ticks = 0;
            self.driving = true;
            self.phase = Phase::Flip(0);
            v = dir * (speed + 0.2) + UP * ROOF_LEAP_UP;
        } else {
            v = v - UP * 0.08;
        }
        Some(v)
    }

    /// The parkour run: R and W held on the ground, no web, zip or vault.
    /// The speed is kept here, since the game's friction takes most of it
    /// off the velocity read back each tick; it builds from the walk to
    /// `RUN_SPEED` and A/D lean it sideways. Kept over a hop or a curb
    /// while R and W stay down.
    fn ground_run(&mut self, state: &Sense, v: P, ahead: P, side: f64) -> Option<P> {
        let held = self.web.is_none()
            && self.zip.is_none()
            && key(state, "KeyR")
            && key(state, "KeyW")
            && !key(state, "ShiftLeft");
        if !held {
            self.run_speed = 0.0;
        }
        if !held || !state.on_ground || self.driving || self.vault.is_some() || ahead.len() < 0.1 {
            return None;
        }
        self.run_speed = (self.run_speed.max(v.flat().len()) + RUN_ACCEL).min(RUN_SPEED);
        let right = ahead.cross(UP).norm();
        let mut out = (ahead + right * (side * RUN_SIDE)).norm() * self.run_speed;
        out.y = if key(state, "Space") { RUN_JUMP } else { v.y };
        Some(out)
    }

    /// Over a low obstacle, under an overhang, from the rays ahead.
    fn dodge(&mut self, state: &Sense, mut v: P) -> P {
        let flat = v.flat().len();
        if flat < 0.3 || state.ahead.len() < 5 {
            return v;
        }
        // Ticks until each ray's block; None: clear.
        let soon = |i: usize| state.ahead[i].map(|d| d / flat).filter(|t| *t < DODGE_TICKS);
        let low = soon(0).or(soon(1));
        let high = soon(3).or(soon(4));
        let mid = soon(2);
        match (low, mid, high) {
            // Blocks low, room above: up over it, in time.
            (Some(t), None, None) => {
                let need = DODGE_CLEAR / t.max(1.0) + 0.04 * t;
                if v.y < need {
                    v.y = v.y + (need - v.y).min(DODGE_LIFT);
                    if self.hop_cool == 0 {
                        self.phase = Phase::Flip(FLIP_TICKS / 2);
                        self.hop_cool = 20;
                    }
                }
            }
            // A ceiling ahead, room below: no rising into it, a little down.
            (None, None, Some(_)) | (None, Some(_), Some(_)) if v.y > -0.2 => {
                v.y = v.y.min(0.0) - DODGE_DUCK;
            }
            _ => {}
        }
        self.hop_cool = self.hop_cool.saturating_sub(1);
        v
    }

    /// Ground parkour: running into an obstacle starts a vault picked by its
    /// height and length, read off the ground profile ahead.
    fn vault_start(&mut self, state: &Sense, feet: P, look: P) {
        if self.vault.is_some() || self.debug.is_some() || !state.on_ground || self.driving {
            return;
        }
        // Never off a wall: the wall run has its own vault over the top.
        if matches!(self.phase, Phase::WallRun(_) | Phase::Slap(..)) {
            return;
        }
        if self.web.is_some() || self.zip.is_some() || !key(state, "KeyW") {
            return;
        }
        // R held (PS4's R2) is the parkour run, W the direction: walking,
        // running, or pressed against the face from a stop. Without R the
        // ground is vanilla (doors, steps); sneaking never vaults.
        if !key(state, "KeyR") || key(state, "ShiftLeft") || state.profile.is_empty() || look.flat().len() < 0.1 {
            return;
        }
        let v = P::of(&state.velocity);
        let flat = v.flat().len();
        let dir = if flat > 0.03 { v.flat().norm() } else { look.flat().norm() };
        // Going roughly where you look, not sliding past.
        if dir.dot(look.flat().norm()) < 0.7 {
            return;
        }
        let Some(first) = state.profile.iter().position(|h| h.is_some_and(|h| h > VAULT_MIN)) else {
            return;
        };
        let top = state.profile[first].unwrap_or(0.0);
        if top > VAULT_WALL {
            return;
        }
        let kind = if top <= VAULT_STEP {
            Move::Step
        } else if top <= VAULT_HAND {
            Move::Hand
        } else {
            Move::Climb
        };
        // On the ground the velocity read is after friction (0.6 · 0.91 of
        // the tick's move): the run's real speed is the move.
        let run = flat / GROUND_FRICTION;
        let speed = run.max(self.roof_speed * f64::from(u8::from(self.roof_ticks > 0))).max(0.22);
        // Sample i stands 0.5·(i+1) ahead: the face is about a quarter
        // block before the first sample on it.
        let front = 0.5 * (first + 1) as f64 - 0.25;
        let reach = match kind {
            Move::Step => 0.5 + speed * 4.0,
            Move::Hand => 0.6 + speed * 5.0,
            Move::Climb => 0.9 + speed * 2.0,
        };
        if front > reach {
            return;
        }
        // Its length: the samples on top at about the same height, and
        // whether the ground drops again behind it.
        let on_top = state.profile[first..]
            .iter()
            .take_while(|h| h.is_some_and(|h| (h - top).abs() < 0.6))
            .count();
        let after = state.profile.get(first + on_top).copied().flatten();
        // A higher step right behind (stoops, a parapet then a roof unit):
        // land on this one and go on from there, as the reference chains
        // pallets → dumpster (146.6–147.1 s).
        let higher = after.is_some_and(|h| h > top);
        let ends = first + on_top < state.profile.len() && !higher;
        let length = 0.5 * on_top as f64;
        if state.profile[first..first + on_top].iter().any(|h| h.is_some_and(|h| h > VAULT_WALL)) {
            return;
        }
        self.vault_note = format!(
            "{kind:?} top {top:.2} front {front:.2} back {:.2} long {} speed {speed:.2}",
            front + length,
            !ends || length >= VAULT_LONG
        );
        self.vault = Some(Vault {
            kind,
            dir,
            start: feet,
            front,
            back: front + length,
            top,
            speed,
            long: !ends || length >= VAULT_LONG,
            ticks: 0,
            stuck: 0,
            arc_vy: None,
            drop: None,
        });
        self.phase = Phase::Vault(kind, false, 0);
    }

    /// The vault's move this tick: up to the top keeping clear of the face,
    /// across, then off the far end (a flip after a hand vault or a climb)
    /// or onto a long top, running.
    fn vault_step(&mut self, state: &Sense, feet: P) -> Option<P> {
        let mut vt = self.vault?;
        if self.web.is_some() || self.zip.is_some() {
            self.vault = None;
            return None;
        }
        vt.ticks += 1;
        let along = (feet - vt.start).dot(vt.dir);
        let h = feet.y - vt.start.y;
        let clear = vt.top + if vt.long { 0.1 } else { VAULT_CLEAR };
        let actual = P::of(&state.velocity);
        let asked = self.asked;
        // Blocked: the game took most of the asked move for a few ticks.
        if let Some(a) = asked {
            let flat_cut = a.flat().len() > 0.1 && actual.flat().len() < a.flat().len() * 0.3;
            let up_cut = a.y > 0.1 && actual.y < a.y * 0.3;
            vt.stuck = if flat_cut || up_cut { vt.stuck + 1 } else { 0 };
        }
        if vt.ticks > 60 || vt.stuck >= 3 {
            self.vault = None;
            self.asked = None;
            return None;
        }
        if let Some((k, total, flip)) = vt.drop {
            // The drop off the far side: the game's gravity, one forward
            // flip spread over the fall (it was cut short at the landing).
            if state.on_ground && k > 1 {
                self.vault = None;
                self.asked = None;
                self.phase = Phase::Ground;
                return None;
            }
            let vy = vt.arc_vy.unwrap_or(0.0);
            vt.arc_vy = Some(vy - DROP_GRAVITY);
            vt.drop = Some((k + 1, total, flip));
            let turn = ((k as f64 / total) * FLIP_TICKS as f64).min(FLIP_TICKS as f64 - 1.0);
            self.phase = if flip { Phase::Flip(turn as u32) } else { Phase::Vault(vt.kind, true, vt.ticks + k) };
            self.vault = Some(vt);
            let out = vt.dir * vt.speed + UP * vy;
            self.asked = Some(out);
            return Some(out);
        }
        let rising = h < clear - 0.2 && vt.arc_vy.is_none();
        let out = if rising {
            let room = (vt.front - FACE_GAP - along).max(0.0);
            let fwd = room.min(vt.speed);
            let ticks = (room / vt.speed).max(1.0);
            let max = match vt.kind {
                Move::Step => RISE_STEP,
                Move::Hand => RISE_HAND,
                Move::Climb => RISE_CLIMB,
            };
            let up = ((clear - h) / ticks).clamp(0.12, max).min(clear - h + 0.02);
            vt.dir * fwd + UP * up
        } else if vt.long {
            if along > vt.front + 0.6 {
                // On top: the run goes on in the game's hands.
                self.vault = None;
                self.asked = None;
                self.phase = Phase::Ground;
                return Some(vt.dir * vt.speed);
            }
            vt.dir * vt.speed
        } else {
            // Over a short one: a jump's arc (game gravity) that clears the
            // far edge, then off it.
            // A push forward off the hands (the reference sweeps over in
            // ~0.25 s; a Minecraft sprint alone took 0.5 s over 2 blocks).
            let speed = vt.speed.max(if vt.kind == Move::Step { OVER_STEP } else { OVER_HAND });
            let vy = *vt.arc_vy.get_or_insert_with(|| {
                let t = ((vt.back + 0.4 - along) / speed).max(1.0);
                ((vt.top + 0.05 - h + ARC_GRAVITY / 2.0 * t * (t + 1.0)) / t).clamp(0.0, 0.5)
            });
            if along > vt.back + 0.4 || state.on_ground && vt.ticks > 3 && vy < 0.0 {
                if state.on_ground {
                    self.vault = None;
                    self.asked = None;
                    self.phase = Phase::Ground;
                    return Some(vt.dir * speed + UP * vy);
                }
                if vt.kind == Move::Step {
                    // Down the far side in the vault's pose until the feet
                    // land (cleared mid-air, the body snapped upright).
                    vt.speed = speed;
                    vt.arc_vy = Some(vy);
                    vt.drop = Some((0, 1.0, false));
                    self.vault = Some(vt);
                    let out = vt.dir * speed + UP * vy;
                    self.asked = Some(out);
                    return Some(out);
                }
                // A forward flip on the drop (SUV 171.6, truck bed 151.5),
                // a little lift so it has time to turn; the drop's length
                // from the height over the ground the run started on.
                let fall = h.max(0.5);
                let g = DROP_GRAVITY / 2.0;
                let lift = (g * FLIP_DROP * (FLIP_DROP + 1.0) - fall) / FLIP_DROP;
                let vy = vy.max(lift);
                let total = (vy + (vy * vy + 4.0 * g * fall).sqrt()) / (2.0 * g);
                vt.speed = speed;
                vt.arc_vy = Some(vy);
                vt.drop = Some((0, total.max(4.0), true));
                self.phase = Phase::Flip(0);
                self.vault = Some(vt);
                let out = vt.dir * speed + UP * vy;
                self.asked = Some(out);
                return Some(out);
            }
            vt.arc_vy = Some(vy - ARC_GRAVITY);
            vt.dir * speed + UP * vy
        };
        self.phase = Phase::Vault(vt.kind, !rising, vt.ticks);
        self.vault = Some(vt);
        self.asked = Some(out);
        Some(out)
    }

    fn wall_run(&mut self, state: &Sense, v: P, look: P, side: f64, space: bool) -> Option<P> {
        if self.debug.is_some() {
            // A held pose is only a picture: no wall-run physics.
            return None;
        }
        let wall = state.wall.as_ref().map(P::of);
        // Out from under an overhang, level, then up past its lip; the wall
        // run catches the wall above it on its own.
        if let Some((w, t)) = self.overhang {
            if t < OVERHANG_OUT_TICKS {
                self.overhang = Some((w, t + 1));
                // Level: what gravity takes off is given back.
                return Some(w * -OVERHANG_OUT + UP * web::FALL);
            }
            self.overhang = None;
            return Some(w * (OVERHANG_OUT * 0.3) + UP * OVERHANG_UP);
        }
        if let Phase::Slap(w, t) = self.phase {
            self.r_held = key(state, "KeyR");
            if t >= SLAP_STICK + SLAP_SLIDE || wall.is_none() {
                // Peel off: on with the fall, or into a run if R is held.
                self.phase = if key(state, "KeyR") && wall.is_some() { Phase::WallRun(w) } else { Phase::Fall };
                self.climb = 0.0;
                return Some(w * -0.15);
            }
            self.phase = Phase::Slap(w, t + 1);
            // Stuck, then sliding down slowly.
            let down = if t < SLAP_STICK { 0.0 } else { SLAP_SLIDE_SPEED };
            return Some(w * WALL_HUG - UP * down);
        }
        if let Phase::WallRun(dir) = self.phase {
            if let Some(w) = wall.filter(|w| w.dot(dir) > 0.5) {
                let pressed = space && !self.wall_space;
                self.wall_space = space;
                if pressed {
                    // Kick off: away from the wall and up (a press, not a hold:
                    // a Space held into the run must not throw you off).
                    self.phase = Phase::Flip(0);
                    return Some(w * -WALL_KICK + UP * WALL_KICK_UP);
                }
                let along = w.cross(UP).norm();
                // Up against an overhang: the run asks up, the move made
                // (`v`, the chest's step) gives nothing; out around its lip
                // and on up the wall above.
                if self.wall_vel.y > 0.1 && v.y < 0.02 {
                    self.wall_blocked += 1;
                } else {
                    self.wall_blocked = 0;
                }
                if self.wall_blocked >= OVERHANG_TICKS {
                    self.wall_blocked = 0;
                    self.overhang = Some((w, 0));
                    self.phase = Phase::Flip(0);
                }
                // On the wall WASD walk it as a floor (Web of Shadows): W up,
                // S down, A/D along; nothing held clings. R held runs, as on
                // the ground (PS4's R2): faster, and up with no key; letting
                // go walks again, only Space leaves the wall. Minecraft's
                // sprint runs too: Ctrl, or W pressed twice quickly.
                let w_down = key(state, "KeyW");
                if w_down && !self.w_was {
                    if self.ticks - self.w_last <= SPRINT_TAP {
                        self.sprint = true;
                    }
                    self.w_last = self.ticks;
                }
                if !w_down {
                    self.sprint = false;
                }
                self.w_was = w_down;
                self.r_held = key(state, "KeyR");
                let running = self.r_held || key(state, "ControlLeft") || self.sprint;
                // WASD follow the camera (Web of Shadows): forward is the
                // view laid flat on the wall, right is across it. Looking
                // straight at the wall, forward is up it.
                let n = w * -1.0;
                let flat_look = look - n * look.dot(n);
                let ahead_w = if flat_look.len() > 0.3 { flat_look.norm() } else { UP };
                let right_w = ahead_w.cross(n).norm();
                let fwd = axis(state, "KeyW") - axis(state, "KeyS");
                let mut dir = ahead_w * fwd + right_w * side;
                if running && dir.len() < 0.01 {
                    dir = ahead_w;
                }
                let speed = if running { WALL_CLIMB } else { WALL_WALK };
                let target = if dir.len() > 0.01 { dir.norm() * speed } else { P::default() };
                // Speeds ease in and out (~0.3 s from a fall to a run); the
                // entry speed above the run bleeds off slowly.
                let d = target - self.wall_vel;
                let step = if self.wall_vel.len() > speed && dir.len() > 0.01 { WALL_BLEED } else { WALL_RAMP };
                self.wall_vel = self.wall_vel + d.clamp(step);
                self.climb = self.wall_vel.len();
                // The body faces where it goes on the wall, eased.
                if self.climb > 0.05 {
                    let up = self.wall_vel.dot(UP);
                    let sideways = self.wall_vel.dot(along);
                    let twist = -sideways.atan2(up) as f32;
                    let turn = (twist - self.run_twist + PI).rem_euclid(TAU) - PI;
                    self.run_twist += turn * 0.35;
                }
                return Some(self.wall_vel + w * WALL_HUG);
            }
            // The wall ended: over the top edge if going up, else let go.
            self.phase = Phase::Flip(0);
            return Some(if v.y > 0.0 { dir * LEDGE_OVER + UP * LEDGE_UP } else { v });
        }
        let w = wall?;
        // Too fast into a wall: a slap, whatever keys are held.
        // Not on a zip: a zip into a wall runs up it (Danny's AoT).
        if !state.on_ground && self.driving && self.zip.is_none() && v.dot(w) > SLAP_SPEED {
            self.web = None;
            self.safe = true;
            self.phase = Phase::Slap(w, 0);
            self.impact = 1.0;
            return Some(w * WALL_HUG);
        }
        // Only in a flight a web or a zip started (an ordinary jump at a step
        // must stay a jump).
        let starts = !state.on_ground
            && self.driving
            && key(state, "KeyR")
            && self.zip.is_none()
            && look.flat().norm().dot(w) > 0.4
            && (v.dot(w) > 0.05 || key(state, "KeyW"));
        if !starts {
            return None;
        }
        self.web = None;
        self.driving = true;
        self.safe = true;
        self.phase = Phase::WallRun(w);
        self.r_held = true;
        // A Space held into the run is not a kick.
        self.wall_space = key(state, "Space");
        self.run_twist = 0.0;
        // Keep the tempo (the game: a swing into a wall becomes a run at its
        // speed): the speed you came in with, turned up the wall and along
        // it, at least the run's.
        let along_in = v - w * v.dot(w);
        let speed = v.len().clamp(WALL_CLIMB, WALL_CLIMB * WALL_ENTRY_BOOST);
        let dir = (UP * 1.5 + along_in.flat()).norm();
        self.wall_vel = dir * speed;
        self.climb = speed;
        Some(self.wall_vel + w * WALL_HUG)
    }
}

/// A posed part's name as the pose lists write it.
fn part_name(name: &str) -> &'static str {
    match name {
        "right_arm" => "right_arm",
        "left_arm" => "left_arm",
        "right_leg" => "right_leg",
        "left_leg" => "left_leg",
        _ => "head",
    }
}

/// An angle brought to -π..π.
fn wrap_angle(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

fn angle_step(from: f32, to: f32, k: f32) -> f32 {
    let d = (to - from + PI).rem_euclid(TAU) - PI;
    from + d * k
}

impl Spidey {
    /// The wall the camera turns onto during a long run, and how far it has
    /// turned (0..1).
    pub fn wall_camera(&mut self, state: &Sense) -> (Option<P>, f32) {
        let long = match self.phase {
            Phase::WallRun(w) if state.wall_above >= LONG_WALL => Some(w),
            _ => None,
        };
        if let Some(w) = long {
            self.cam_wall = Some(w);
        }
        let target = if long.is_some() { 1.0 } else { 0.0 };
        self.cam_blend += (target - self.cam_blend).clamp(-WALL_CAM_STEP, WALL_CAM_STEP);
        if self.cam_blend <= 0.0 {
            self.cam_wall = None;
        }
        // Eased in and out.
        let x = self.cam_blend;
        (self.cam_wall, x * x * (3.0 - 2.0 * x))
    }

    /// Parts the mod still poses (0: the game's own animation).
    pub fn posed(&self) -> usize {
        self.shown.len()
    }

    /// The movement state for `/swingmode debug`.
    pub fn note(&self) -> String {
        format!(
            "{:?} vault {} wall_vel {:.2} blocked {} driving {}",
            self.phase,
            self.vault.is_some(),
            self.wall_vel.y,
            self.wall_blocked,
            self.driving
        )
    }

    pub fn wall_running(&self) -> bool {
        matches!(self.phase, Phase::WallRun(_))
    }
    pub fn cam_blend(&self) -> f32 {
        self.cam_blend
    }

    pub fn reset(&mut self) {
        // The settings survive a reset; the rest starts over.
        let web_delay = self.web_delay;
        *self = Spidey::default();
        self.web_delay = web_delay;
        motion::clear_pose();
    }

    /// One tick: the velocity to set, if the mod drives this tick.
    pub fn tick(&mut self, state: &Sense, chest: P, mut v: P) -> Option<P> {
        self.ticks += 1;
        // A wall stopped part of last tick's move: carry the speed along it.
        if let Some(asked) = self.asked.filter(|_| self.driving) {
            let blocked_x = asked.x.abs() > 0.15 && v.x.abs() < asked.x.abs() * 0.3;
            let blocked_z = asked.z.abs() > 0.15 && v.z.abs() < asked.z.abs() * 0.3;
            if blocked_x || blocked_z {
                let speed = asked.len() * WALL_KEEP;
                let mut along = asked;
                if blocked_x {
                    along.x = 0.0;
                }
                if blocked_z {
                    along.z = 0.0;
                }
                // Straight into the wall: run up it.
                along.y = along.y.max(0.0) + speed * 0.5;
                v = along.norm() * speed;
            }
        }
        let look = P::of(&state.look);
        let ahead = look.flat().norm();
        let side = axis(state, "KeyD") - axis(state, "KeyA");
        // R (PS4's R2): in the air it shoots, let go on release; on the
        // ground it is the parkour run and shoots nothing. Held from the
        // ground, the web goes once you are off it: a jump, or off an edge
        // with a real drop below and falling (not a curb, a hop, a vault
        // or a ladder climbed).
        let r = key(state, "KeyR");
        // No web from a wall: R there was firing one at once (playtest).
        let on_wall = matches!(self.phase, Phase::WallRun(_) | Phase::Slap(..));
        let pressed = r && !self.r_held;
        // A vault disarms: one cut short in the air must not fire a web.
        if self.vault.is_some() {
            self.web_armed = false;
        }
        self.web_armed = r && !on_wall && (self.web_armed || pressed || state.on_ground);
        // A jump: Space on the ground this tick or the last (a quick tap can
        // be up before the feet leave it); kept while in the air.
        if state.on_ground {
            self.jumped = key(state, "Space") || self.space_was;
        }
        let drop = state.floor.map_or(f64::INFINITY, |f| chest.y - super::CHEST - f);
        let off = !state.on_ground
            && self.vault.is_none()
            && !matches!(self.phase, Phase::Vault(..))
            && self.web.is_none()
            && self.zip.is_none()
            && (self.jumped || (drop > ARM_DROP && state.velocity.y < 0.0));
        let fire = !state.on_ground && !on_wall && ((pressed && self.vault.is_none()) || (self.web_armed && off));
        if fire {
            // Nothing in reach: still armed, tried again next tick.
            if let Some(anchor) = pick_anchor(state, chest, side) {
                self.web_armed = false;
                self.webs += 1;
                let length = (anchor - chest).len();
                let flight = flight_ticks((length / WEB_SPEED).clamp(2.0, MAX_FLIGHT), self.web_delay);
                // Swing down the middle of the street (PS4: the arc is bent to
                // keep you straight, walls push you out): hang from a point
                // over your line of flight, keeping `WALL_SIDE` of the
                // sideways offset to the wall.
                let right = ahead.cross(UP).norm();
                let pivot = anchor - right * ((anchor - chest).dot(right) * (1.0 - WALL_SIDE));
                // Keep height: the bottom of the arc at most `DROP` below you.
                // The rope starts at the distance and the tug shortens it to
                // that, so there is no jolt.
                let length = (pivot - chest).len();
                let target = (length * web::TUG).min(pivot.y - (chest.y - DROP)).max(web::MIN_ROPE);
                self.web = Some(Web {
                    anchor,
                    pivot,
                    length,
                    target,
                    // Gentle: never more than `TUG_MAX` a tick (a long cut used to yank).
                    tug_step: ((length - target).max(0.0) / web::TUG_TICKS).min(TUG_MAX),
                    flying: flight,
                    flight,
                    held: 0,
                });
                self.zip = None;
                self.driving = true;
                // Just off the ground: a low arc that lifts you (PS4: the web
                // is shorter near the ground).
                if drop < GROUND_ARC {
                    self.ground_start = true;
                }
            }
        }
        if !r && self.r_held {
            if let Some(w) = self.web.take() {
                if w.flying == 0 {
                    let up = if v.y > 0.0 {
                        web::RELEASE_UP
                    } else {
                        web::RELEASE_UP_FALLING
                    };
                    v = v + ahead * web::RELEASE_FORWARD + UP * up;
                    if !state.on_ground {
                        self.phase = Phase::Flip(0);
                    }
                }
            }
        }
        self.r_held = r;
        // Z: a hook to the aimed point, held while the key is.
        let z = key(state, "KeyZ");
        if z && !self.z_held {
            if let Some(hit) = &state.aim.hit {
                let hit = P::of(hit);
                let flight = flight_ticks(((hit - chest).len() / HOOK_SPEED).clamp(1.0, MAX_FLIGHT), self.web_delay);
                self.zip = Some(hit);
                self.zip_flying = flight;
                self.zip_flight = flight;
                self.web = None;
                self.driving = false;
                self.safe = true;
            }
        }
        // Point launch (Spider-Man 2 `h_ToNJySgwE` 573.1–574.0 s, 429.8–430.1 s):
        // at the zip's point, Space throws you forward and up, with a flip.
        if key(state, "Space") && !self.space_launch {
            if let (Some(_), Some((tangent, speed))) = (self.zip, self.zip_hold) {
                self.zip = None;
                self.zip_hold = None;
                self.space_launch = true;
                self.z_held = z;
                self.phase = Phase::Flip(0);
                self.driving = true;
                let ahead = if look.flat().len() > 0.1 { look.flat().norm() } else { tangent };
                return Some(ahead * (LAUNCH_FORWARD + speed * 0.3) + UP * LAUNCH_UP);
            }
        }
        self.space_launch = key(state, "Space");
        if !z && self.z_held {
            if let Some(target) = self.zip.take() {
                if !state.on_ground {
                    self.phase = Phase::Flip(0);
                    // The flight goes on under the mod (its landing starts a
                    // roof run).
                    self.driving = true;
                }
                if let Some((tangent, speed)) = self.zip_hold.take() {
                    // Let go at the point: on past it with a hop, a vault
                    // over the edge if the point was a climb.
                    self.z_held = z;
                    let up = if target.y - self.arc_from.y > VAULT_RISE { ZIP_VAULT } else { ZIP_HOP };
                    return Some(tangent * (speed * ZIP_CARRY) + UP * up);
                }
            }
        }
        self.z_held = z;
        let space = key(state, "Space");
        // Space let go after being held on a web or a zip: let go of it with a
        // jump (a trick: forward and up, a flip); R still lets go plainly.
        let space_let_go = !space && self.space_was && !state.on_ground;
        self.space_was = space;
        if space_let_go && (self.web.as_ref().is_some_and(|w| w.flying == 0) || (self.zip.is_some() && self.zip_hold.is_none() && self.zip_flying == 0)) {
            self.web = None;
            self.zip = None;
            self.driving = true;
            self.phase = Phase::Flip(0);
            let ahead = if look.flat().len() > 0.1 { look.flat().norm() } else { v.flat().norm() };
            v = v + ahead * JUMP_OFF_FORWARD + UP * JUMP_OFF_UP;
            self.asked = None;
            return Some(v.clamp(web::MAX_SPEED));
        }
        // Parkour: running up a wall (Spider-Man 2, the Empire State run,
        // `h_ToNJySgwE` 425.6–433.8 s: ~3 s up the facade at ~20 m/s, a vault
        // over the ledge, on up the next tier). Flying into a wall while you
        // face it starts the run; A/D move along it; the wall ending at the
        // top is a vault over the edge; Space kicks off it.
        // Ground parkour by the obstacle's size (`AM6jwca0Akg` 145–172 s):
        // one probe of the ground ahead picks a step, a hand vault or a run
        // up the face, and whether to land on top. Space is ignored meanwhile.
        let feet = chest - UP * super::CHEST;
        self.vault_start(state, feet, look);
        if let Some(out) = self.vault_step(state, feet) {
            return Some(out);
        }
        if let Some(out) = self.wall_run(state, v, look, side, space) {
            self.asked = None;
            return Some(out);
        }

        if let Some(target) = self.zip {
            let mut v = P::of(&state.velocity);
            if self.zip_flying > 0 {
                // The web is still flying: the fall goes on as it was.
                self.zip_flying -= 1;
                if self.zip_flying == 0 {
                    // It bites: the old momentum goes, and the path is laid
                    // from here to the point over an upper arc.
                    self.arc_from = chest;
                    self.arc_height = arc_bump(chest, target, v.y < -0.3);
                    self.arc_t = 0.0;
                    self.arc_len = arc_length(chest, target, self.arc_height);
                    self.zip_hold = None;
                    self.zip_asked = None;
                    self.zip_walled = false;
                    self.zip_ticks = 0;
                    v = P::default();
                }
            } else {
                self.zip_ticks += 1;
                let top = ZIP_PATH_SPEED * if space { ZIP_HAUL } else { 1.0 };
                let speed = (ZIP_START + ZIP_RAMP * self.zip_ticks as f64).min(top);
                // A wall cut last tick's move (the game zeroes the part into a
                // wall): run up and along it toward the point, still pressing
                // into it so the contact holds; once past it, lay a new arc
                // from here (Danny's AoT recording: pulls turn into wall runs).
                if self.zip_hold.is_none() {
                    if let Some(asked) = self.zip_asked {
                        let cut = |a: f64, b: f64| a.abs() > 0.2 && b.abs() < a.abs() * 0.25;
                        let (bx, bz) = (cut(asked.x, v.x), cut(asked.z, v.z));
                        if bx || bz {
                            let mut to = target - chest;
                            if bx {
                                to.x = 0.0;
                            }
                            if bz {
                                to.z = 0.0;
                            }
                            if to.len() < 0.5 {
                                to = UP;
                            }
                            let mut run = to.norm() * speed.max(WALL_RUN);
                            if bx {
                                run.x = asked.x.signum() * WALL_PRESS;
                            }
                            if bz {
                                run.z = asked.z.signum() * WALL_PRESS;
                            }
                            self.zip_walled = true;
                            self.zip_asked = Some(run);
                            self.phase = Phase::Air;
                            self.asked = None;
                            return Some(run);
                        } else if self.zip_walled {
                            self.zip_walled = false;
                            self.arc_from = chest;
                            self.arc_height = arc_bump(chest, target, false);
                            self.arc_t = 0.0;
                            self.arc_len = arc_length(chest, target, self.arc_height);
                        }
                    }
                }
                let t = (self.arc_t + speed / self.arc_len.max(1.0)).min(1.0);
                let next = arc_point(self.arc_from, target, self.arc_height, t);
                let tangent = (next - arc_point(self.arc_from, target, self.arc_height, self.arc_t)).norm();
                self.arc_t = t;
                if self.zip_hold.is_some() || t >= 1.0 || (target - chest).len() < ZIP_HOLD {
                    // Arrived: hang at the point while Z is held; letting go
                    // hops on past it.
                    if self.zip_hold.is_none() {
                        self.zip_hold = Some((tangent.flat().norm(), speed));
                    }
                    v = ((target - UP * ZIP_HANG) - chest) * 0.3;
                } else {
                    // Head for the next point of the path; walls take only
                    // the part of the move that points into them.
                    v = next - chest;
                }
                self.zip_asked = Some(v);
            }
            self.phase = if self.zip.is_some() { Phase::Air } else { self.phase };
            self.asked = None;
            return Some(v.clamp(5.0));
        } else if let Some(w) = self.web.as_mut() {
            if w.flying > 0 {
                w.flying -= 1;
                // No shove when it bites (it jerked you toward the view):
                // only off the ground, a lift along your motion.
                if w.flying == 0 {
                    // It bites: the rope is as long as it is now (you kept
                    // falling while it flew), and the tug works from here.
                    let now = (w.pivot - chest).len();
                    // From the ground the arc's bottom stays above it (PS4:
                    // the web is shorter near the ground), and shortens faster.
                    let (drop, cap) = if self.ground_start { (GROUND_DROP, GROUND_TUG_MAX) } else { (DROP, TUG_MAX) };
                    w.length = w.length.max(now);
                    w.target = (now * web::TUG).min(w.pivot.y - (chest.y - drop)).max(web::MIN_ROPE);
                    w.tug_step = ((w.length - w.target).max(0.0) / web::TUG_TICKS).min(cap);
                }
                if w.flying == 0 && self.ground_start {
                    // Started from the ground: the web lifts you off it.
                    v = v + (w.pivot - chest).norm() * GROUND_TUG;
                    self.ground_start = false;
                }
                v = v * web::DAMP - UP * web::FALL;
            } else {
                if w.length > w.target {
                    w.length = (w.length - w.tug_step).max(w.target);
                }
                // An invisible floor (playtest): the arc's bottom stays
                // `CLEARANCE` above the ground under you, the rope shortening
                // gently when it would dip lower.
                if let Some(floor) = state.floor {
                    let longest = w.pivot.y - (floor + CLEARANCE + super::CHEST);
                    if w.length > longest {
                        w.length = (w.length - FLOOR_TUG).max(longest.max(web::MIN_ROPE));
                    }
                }
                let anchor = w.pivot;
                v = v * web::DAMP - UP * web::GRAVITY;
                if key(state, "KeyW") {
                    let rope = (anchor - chest).norm();
                    v = v + (look - rope * look.dot(rope)).norm() * web::PUMP;
                }
                // Space held: speed up along the swing.
                if space {
                    let rope = (anchor - chest).norm();
                    let along = (v - rope * v.dot(rope)).norm();
                    v = v + along * SWING_BOOST;
                }
                // Keep the flight straight along its own heading, not the view:
                // only the drift toward the wall the web is on is damped.
                if side == 0.0 {
                    let heading = v.flat().norm();
                    let right = heading.cross(UP).norm();
                    let toward = (w.anchor - chest).flat().dot(right).signum();
                    let drift = v.dot(right);
                    if drift * toward > 0.0 {
                        v = v - right * (drift * web::STRAIGHTEN);
                    }
                }
                // An elastic web (it jerked when a fast fall was caught in one
                // tick): only part of the overshoot is taken back at first,
                // all of it after `STRETCH_TICKS`.
                w.held += 1;
                let firm = (STRETCH_MIN + (1.0 - STRETCH_MIN) * w.held as f64 / STRETCH_TICKS).min(1.0);
                let length = w.length;
                let out = chest + v - anchor;
                if out.len() > length {
                    let target = anchor + out.norm() * length - chest;
                    v = v + (target - v) * firm;
                }
                self.phase = Phase::Air;
            }
        } else if self.driving {
            v = v * web::DAMP - UP * web::FALL;
        }
        // Phases without a web: the flip runs out into the fall.
        if self.web.is_none() && self.zip.is_none() {
            self.phase = match self.phase {
                _ if state.on_ground => Phase::Ground,
                Phase::Flip(t) if t + 1 >= FLIP_TICKS => Phase::Fall,
                Phase::Flip(t) => Phase::Flip(t + 1),
                Phase::Air | Phase::Ground if !self.driving => Phase::Ground,
                _ => Phase::Fall,
            };
        }
        // Swung onto a roof still on the web, running (W): the web lets go
        // and the landing becomes a roof run (it stopped dead).
        if state.on_ground && key(state, "KeyW") && self.web.as_ref().is_some_and(|w| w.flying == 0) && v.flat().len() > 0.45 {
            self.web = None;
            self.driving = true;
        }
        if self.driving && state.on_ground && self.web.is_none() && self.zip.is_none() {
            self.driving = false;
            // Landing fast: a roof run (Spider-Man 2 `h_ToNJySgwE` 430.8,
            // 572.0–573.0 s), if W is held.
            let flat = v.flat().len();
            if flat > 0.45 {
                self.roof_ticks = ROOF_TICKS;
                self.roof_speed = flat * ROOF_KEEP;
            }
        }
        if let Some(run) = self.roof_run(state, look) {
            return Some(run);
        }
        if self.web.is_some() {
            self.safe = true;
        }
        if state.on_ground && self.web.is_none() && self.zip.is_none() {
            self.safe = false;
        }
        if let Some(run) = self.ground_run(state, v, ahead, side) {
            return Some(run);
        }
        // Parkour in flight (Spider-Man 2 `rQVxkz7azJA` 87–91.5 s: low
        // under an overpass, over and through trees): what is ahead along the
        // flight decides a hop over or a duck under.
        if self.driving && !state.on_ground && self.zip.is_none() {
            v = self.dodge(state, v);
        }
        let out = self.driving.then_some(v.clamp(web::MAX_SPEED));
        self.asked = out;
        out
    }

    pub fn ropes(&self, chest: P, yaw: f32) -> Vec<Rope> {
        let mut out = Vec::new();
        // The raised hand; a flying web leaves the arm pointed at it.
        let hand = |to: P, flying: bool| {
            if flying {
                let shoulder = body_point(SHOULDER, self.body, yaw);
                let feet = chest - UP * super::CHEST;
                shoulder + (to - (feet + shoulder)).norm() * 0.7
            } else {
                body_point(HAND_UP, self.body, yaw)
            }
        };
        if let Some(w) = &self.web {
            let flying = w.flying > 0;
            out.push(Rope {
                to: w.anchor.wit(),
                color: web::COLOR,
                progress: if flying {
                    1.0 - w.flying as f32 / w.flight.max(1) as f32
                } else {
                    1.0
                },
                wave: if flying { WEB_WAVE } else { 0.0 },
                from: Some(hand(w.anchor, flying).wit()),
            });
        }
        if let Some(t) = self.zip {
            let flying = self.zip_flying > 0;
            out.push(Rope {
                to: t.wit(),
                color: web::COLOR,
                progress: if flying {
                    1.0 - self.zip_flying as f32 / self.zip_flight.max(1) as f32
                } else {
                    1.0
                },
                wave: 0.0,
                from: Some(hand(t, false).wit()),
            });
        }
        out
    }

    /// The pose for this tick, eased from the last one; None on the ground.
    /// `/swingmode pose NAME`: hold a phase's pose standing still, to check it.
    pub fn set_debug_pose(&mut self, name: &str) -> Result<(), String> {
        self.debug = match name {
            "wallrun" => Some(Phase::WallRun(P::new(0.0, 0.0, 1.0))),
            "fall" => Some(Phase::Fall),
            "flip" => Some(Phase::Flip(FLIP_TICKS / 3)),
            "step" => Some(Phase::Vault(Move::Step, true, 4)),
            "hand" => Some(Phase::Vault(Move::Hand, false, 3)),
            "tuck" => Some(Phase::Vault(Move::Hand, true, 6)),
            "climb" => Some(Phase::Vault(Move::Climb, false, 5)),
            "dive" => Some(Phase::Vault(Move::Climb, true, 14)),
            "off" | "" => None,
            other => {
                return Err(format!("/swingmode pose wallrun|fall|flip|step|hand|tuck|climb|dive|off, not {other}"));
            }
        };
        Ok(())
    }

    pub fn pose(&mut self, state: &Sense, chest: P, yaw: f32) {
        if let Some(d) = self.debug {
            self.phase = d;
        }
        if self.phase != Phase::Ground {
            self.ease_out = 0;
        }
        let v = P::of(&state.velocity);
        let bob = (self.ticks as f32 * 0.4).sin() * 0.08;
        // Target: parts (xRot, yRot, zRot) and the body's pitch and roll.
        let (parts, body, snap): (Vec<(&str, [f32; 3])>, [f32; 2], bool) = if let Some(t) = self.zip {
            let dir = (t - chest).norm();
            (
                vec![
                    ("right_arm", [-3.0, 0.0, 0.1]),
                    ("left_arm", [0.2, 0.0, -0.3]),
                    ("right_leg", [0.15, 0.0, 0.05]),
                    ("left_leg", [0.15, 0.0, -0.05]),
                ],
                tilt(dir, yaw),
                false,
            )
        } else if let Some(w) = self.web.as_ref() {
            if w.flying > 0 {
                // Shooting: the arm points at the anchor, the body as it was.
                let (pitch, turn) = aim_arm((w.anchor - chest).norm(), yaw);
                (
                    vec![
                        ("right_arm", [pitch, turn, 0.0]),
                        ("left_arm", [0.3, 0.0, -0.4]),
                        ("right_leg", [0.2, 0.0, 0.1]),
                        ("left_leg", [-0.2, 0.0, -0.1]),
                    ],
                    self.body,
                    false,
                )
            } else {
                // Hanging along the rope: head toward the anchor, one arm up.
                let rope = (w.anchor - chest).norm();
                (
                    vec![
                        ("right_arm", [-3.05, 0.0, 0.05]),
                        ("left_arm", [0.25, 0.0, -0.45]),
                        ("right_leg", [0.25, 0.0, 0.03]),
                        ("left_leg", [0.1, 0.0, -0.03]),
                    ],
                    tilt(rope, yaw),
                    false,
                )
            }
        } else {
            match self.phase {
                Phase::Ground => {
                    // Back to the game's own animation: a pose still held
                    // eases out over a few ticks first (it snapped upright).
                    // Angles by their turn, not their count: after a flip the
                    // pitch is a whole turn (-2π) and must read as upright —
                    // read raw it never settled and froze every animation
                    // after the first flip (playtest). At most
                    // `EASE_OUT_TICKS` either way.
                    self.body = self.body.map(wrap_angle);
                    for (_, a) in &mut self.shown {
                        *a = a.map(wrap_angle);
                    }
                    self.ease_out += 1;
                    let settled = self.body.iter().all(|a| a.abs() < 0.08)
                        && self.shown.iter().all(|(_, a)| a.iter().all(|x| x.abs() < 0.15));
                    if settled || self.ease_out > EASE_OUT_TICKS {
                        self.ease_out = 0;
                        if !self.shown.is_empty() {
                            self.shown.clear();
                            self.body = [0.0; 2];
                            motion::clear_pose();
                        }
                        return;
                    }
                    (
                        self.shown.iter().map(|(n, _)| (part_name(n), [0.0f32; 3])).collect(),
                        [0.0, 0.0],
                        false,
                    )
                }
                Phase::Flip(t) => {
                    // One forward turn, eased in and out, limbs tucked.
                    let x = t as f32 / FLIP_TICKS as f32;
                    let turn = -TAU * (x * x * (3.0 - 2.0 * x));
                    (
                        vec![
                            ("right_arm", [-0.9, 0.0, 0.4]),
                            ("left_arm", [-0.9, 0.0, -0.4]),
                            ("right_leg", [-1.5, 0.0, 0.1]),
                            ("left_leg", [-1.5, 0.0, -0.1]),
                        ],
                        [turn, 0.0],
                        true,
                    )
                }
                Phase::WallRun(_) => {
                    // Walking and running on the wall as on a floor (Web of
                    // Shadows): the body stands off the wall, Minecraft's own walk cycle driven by the distance
                    // covered, a small lean into the run when fast.
                    let speed = self.climb as f32;
                    // At most ~3 strides a second: faster read as shuffling
                    // (the pose is set 20 times a second).
                    self.limb_phase += (speed * LIMB_PER_BLOCK).min(STRIDE_MAX_STEP);
                    // Walk → sprint: the swing grows from a walk's (1.4) to a
                    // sprint's, arms wider, and the body leans into the run
                    // (SM2 426.5–428.9 s: chest well ahead of the feet).
                    let walk = (speed / 0.3).min(1.0);
                    let run = ((speed - 0.3) / (WALL_CLIMB as f32 - 0.3)).clamp(0.0, 1.0);
                    let phase = self.limb_phase.cos();
                    let legs = phase * (1.4 * walk + 0.9 * run);
                    let arms = phase * (1.2 * walk + 1.0 * run);
                    let lean = WALL_WALK_LEAN * walk + WALL_RUN_LEAN * run;
                    // The lean points where the body runs: split between the
                    // pitch and the roll by the body's turn on the wall (it
                    // tipped back when running anywhere but straight up).
                    let t = self.run_twist;
                    self.wall_lean = WALL_UPRIGHT - lean * t.cos();
                    let roll = WALL_LEAN_ROLL_SIGN * lean * t.sin();
                    (
                        vec![
                            ("right_leg", [legs - 0.15 * run, 0.0, 0.0]),
                            ("left_leg", [-legs - 0.15 * run, 0.0, 0.0]),
                            ("right_arm", [-arms, 0.0, 0.1 * run]),
                            ("left_arm", [arms, 0.0, -0.1 * run]),
                            ("head", [-0.2 - 0.3 * run, 0.0, 0.0]),
                        ],
                        [self.wall_lean, roll],
                        false,
                    )
                }
                Phase::Slap(..) => (
                    // Flat on the wall, an X: arms up and out, legs apart.
                    vec![
                        ("right_arm", [-0.3, 0.0, 2.3]),
                        ("left_arm", [-0.3, 0.0, -2.3]),
                        ("right_leg", [0.0, 0.0, 0.45]),
                        ("left_leg", [0.0, 0.0, -0.45]),
                        ("head", [0.0, 0.0, 0.0]),
                    ],
                    [0.0, 0.0],
                    true,
                ),
                Phase::Vault(kind, over, t) => {
                    // By concept (Minecraft limbs have no knees or elbows,
                    // decision 42), from `AM6jwca0Akg` 145.6–171.9 s.
                    let long = self.vault.is_some_and(|v| v.long);
                    let stride = (t as f32 * 0.9).cos();
                    match (kind, over) {
                        // Stepping up onto a long top: one big stride.
                        (_, false) if long && kind != Move::Climb => (
                            vec![
                                ("right_leg", [-1.4, 0.0, 0.0]),
                                ("left_leg", [0.6, 0.0, 0.0]),
                                ("right_arm", [0.8, 0.0, 0.1]),
                                ("left_arm", [-1.2, 0.0, -0.1]),
                            ],
                            [-0.2, 0.0],
                            false,
                        ),
                        // Speed vault: a hand on top, both legs swung over to
                        // the side, the body tipped over them.
                        (Move::Step, _) => (
                            vec![
                                ("right_leg", [-1.3, 0.0, 0.35]),
                                ("left_leg", [-1.1, 0.0, 0.55]),
                                ("left_arm", [-0.4, 0.0, -0.2]),
                                ("right_arm", [-0.6, 0.0, 1.3]),
                            ],
                            [0.1, if over { 0.55 } else { 0.3 }],
                            false,
                        ),
                        // Hand vault: hands planted ahead, legs pulled up;
                        // over the top a tuck.
                        (Move::Hand, false) => (
                            vec![
                                ("right_arm", [-1.0, 0.0, 0.2]),
                                ("left_arm", [-1.0, 0.0, -0.2]),
                                ("right_leg", [-1.6, 0.0, 0.15]),
                                ("left_leg", [-1.2, 0.0, -0.15]),
                            ],
                            [-0.4, 0.0],
                            false,
                        ),
                        (Move::Hand, true) => (
                            vec![
                                ("right_arm", [-0.6, 0.0, 0.3]),
                                ("left_arm", [-0.6, 0.0, -0.3]),
                                ("right_leg", [-1.8, 0.0, 0.1]),
                                ("left_leg", [-1.8, 0.0, -0.1]),
                            ],
                            [-0.7, 0.0],
                            false,
                        ),
                        // Up the face: big strides, hands reaching up in turn.
                        (Move::Climb, false) => (
                            vec![
                                ("right_leg", [-1.3 * stride - 0.3, 0.0, 0.0]),
                                ("left_leg", [1.3 * stride - 0.3, 0.0, 0.0]),
                                ("right_arm", [-2.6 + 0.5 * stride, 0.0, 0.15]),
                                ("left_arm", [-2.6 - 0.5 * stride, 0.0, -0.15]),
                                ("head", [-0.4, 0.0, 0.0]),
                            ],
                            [-0.15, 0.0],
                            false,
                        ),
                        // Over the top: a head-first dive, arms ahead.
                        (Move::Climb, true) => (
                            vec![
                                ("right_arm", [-3.0, 0.0, 0.15]),
                                ("left_arm", [-3.0, 0.0, -0.15]),
                                ("right_leg", [0.2, 0.0, 0.05]),
                                ("left_leg", [0.2, 0.0, -0.05]),
                                ("head", [0.0, 0.0, 0.0]),
                            ],
                            [-1.3, 0.0],
                            false,
                        ),
                    }
                }
                Phase::Air | Phase::Fall => {
                    // Spread-eagle: face down and forward, arms out and up,
                    // legs apart.
                    let _ = v;
                    (
                        vec![
                            ("right_arm", [-0.3, 0.0, 2.1 + bob]),
                            ("left_arm", [-0.3, 0.0, -2.1 - bob]),
                            ("right_leg", [0.35, 0.0, 0.45]),
                            ("left_leg", [0.35, 0.0, -0.45]),
                        ],
                        [-1.15, 0.0],
                        false,
                    )
                }
            }
        };
        let k = if snap { 1.0 } else { EASE };
        let mut out = Vec::with_capacity(parts.len());
        for (name, target) in parts {
            let last = self
                .shown
                .iter()
                .find(|(n, _)| n == name)
                .map_or([0.0; 3], |(_, a)| *a);
            let a = [0, 1, 2].map(|i| angle_step(last[i], target[i], k));
            out.push((name.to_string(), a));
        }
        self.body = if snap {
            body
        } else {
            [angle_step(self.body[0], body[0], k), angle_step(self.body[1], body[1], k)]
        };
        self.shown = out;
        let parts: Vec<PartPose> = self
            .shown
            .iter()
            .map(|(name, a)| PartPose {
                part: name.clone(),
                pitch: a[0],
                yaw: a[1],
                roll: a[2],
                pivot_y: None,
                pivot_z: None,
            })
            .collect();
        // On a wall the body is set by the wall's plane, not by the view.
        // Lying back off the wall, the feet would reach 0.9 into it from the
        // middle: move the body out so the soles touch the wall.
        let twist = if let Phase::WallRun(_) = self.phase { self.run_twist } else { 0.0 };
        let (facing, shift) = match self.phase {
            Phase::Slap(w, _) => (Some((-w.x).atan2(w.z).to_degrees() as f32), None),
            Phase::WallRun(w) => (
                Some((-w.x).atan2(w.z).to_degrees() as f32),
                Some({
                    // Feet on the wall: the body turned about its middle (0.9
                    // up) by the lean puts the soles 0.9·sin into the wall.
                    let s = f64::from(self.wall_lean.sin());
                    (w * -(0.9 * s - WALL_GAP) + UP * (0.9 * s)).wit()
                }),
            ),
            _ => (None, None),
        };
        motion::set_pose(&parts, self.body[0], self.body[1], twist, facing, shift);
    }
}
