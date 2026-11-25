//! The `web` mode, built to the reference gameplay (`docs/video-spec.md`):
//! the web flies before it holds, the body hangs along the rope, letting go
//! in the air turns into one forward flip and then a spread-eagle fall, and
//! Z zips to the aimed point with the body along the web.
use super::{P, UP, axis, key, pick_anchor, web};
use crate::sense::Sense;
use crate::swing_anim as anim;
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
/// Ticks the drop with the flip should take (measured,
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
/// Below the start by this much after the drop's flip, the vault lets go
/// (a drop off a roof, not off a car).
const VAULT_DROP_HANDOFF: f64 = 2.0;
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
/// A change of the kind of pose in flight (hanging, a measured swing, what follows its release) is eased across
/// for this many ticks, this much of the way each.
const SEAM_TICKS: u32 = 6;
/// Where a pose starts or ends, the body turns no more than this a tick (radians), and a part's angle no more
/// than this: a pose that starts far from the last one is caught up with over a few ticks, never in one. A take
/// in full play is not held to it: its timing is its author's.
const MAX_TURN: f64 = 0.7;
const MAX_PART_STEP: f32 = 1.1;
/// How far the body on a web leans from the line of the swing to where the web is stuck (0..1).
const WEB_LEAN: f64 = 0.25;
/// The body on a web tips fore or aft from the rope by no more than this (radians).
const BODY_LEAN: f64 = 0.6;

/// How much of the trunk on the web is the measured one (1) rather than the rule «the body stands along the rope»
/// (0). In the footage the body lies along the flight under the web hand, or is already head down at the release.
const TRUNK_ON_WEB: f64 = 0.0;
/// A web shot out of the air: for this many ticks the web arm goes to the anchor by `SHOT_REACH` a tick (it eased
/// there over the cross-fade, which read as a drift, not a shot).
const SHOT_SNAP: usize = 3;
const SHOT_REACH: f32 = 0.6;
/// Free flight with no take in play: the body lies along the flight path (a dive when falling forward, level when
/// flying level) instead of the one spread-eagle. Level speed below `GLIDE_SPEED` (blocks a tick) keeps the
/// spread-eagle: a slow or near-vertical fall has no path to lie along.
const GLIDE_SPEED: f64 = 0.25;
const GLIDE_HOLD: u32 = 14;
/// The trunk follows the path's pitch within these bounds (head down, head up; radians): the measured dive holds
/// at 45 to 55 degrees, never vertical.
const GLIDE_PITCH: (f64, f64) = (0.96, 0.26);
/// How fast the body comes round to the path, a tick: the measured tip-over takes about 0.8 s.
const GLIDE_EASE: f32 = 0.14;
/// The measured flight's rows the limbs drift between while gliding (its dive, before the flare at the end).
const GLIDE_ROWS: (f32, f32) = (9.0, 17.0);
/// Takes whose release, when it throws the player upward, plays as the launch the footage shows: feet first and
/// head down while rising, a slow arch onto the belly, then a prone glide.
const AFTER_LAUNCH: [bool; 9] = [false, false, false, true, false, false, true, false, false];
/// The take the launch is played from. Take 3 measured badly (its body leans 27-64 degrees sideways), so both
/// launches play take 6: its limbs and its body's turn since the release, then the glide before the next web of
/// the same flight (`swing_anim::AIR`).
const LAUNCH_TAKE: usize = 6;
/// Upward speed at the release (blocks a tick) from which a release counts as a launch.
const LAUNCH_RISE: f64 = 0.55;
/// Takes whose own after-block turns the body head down. They follow a release only when it rises at least
/// `INVERT_RISE` (blocks a tick); a flatter or falling release goes on with one of `LEVEL_TAKES` instead.
const INVERTING: [bool; 9] = [false, false, true, true, false, true, true, false, true];
const INVERT_RISE: f64 = 0.2;
const LEVEL_TAKES: [usize; 4] = [0, 1, 4, 7];
/// The head's angle at the release, in the plane of the flight: straight down is a quarter turn below ahead;
/// the measured trunk leans 20 degrees past it, head back.
const LAUNCH_START: f64 = -1.92;
/// The measured limbs are averaged over this many ticks either side.
const LAUNCH_SMOOTH: usize = 1;
const LEAN_EASE: f64 = 0.3;
/// A web stuck further than this to a side (blocks) is taken with that side's hand.
const WEB_SIDE: f64 = 0.5;
/// The head turns no faster than this share of the way a tick, and no further than this (radians).
const HEAD_EASE: f32 = 0.4;
const HEAD_PITCH: f64 = 1.1;
const HEAD_TURN: f64 = 1.0;
/// A swing's flat direction follows the flight this much a tick, while the flat speed is over this.
const HEADING_EASE: f64 = 0.15;
const HEADING_SPEED: f64 = 0.15;
/// Game ticks a second.
const TICKS_PER_SEC: f32 = 20.0;
/// A pose left on the ground eases out for at most this many ticks.
const EASE_OUT_TICKS: u32 = 10;

struct Web {
    /// The tick the web was shot, and whether it was shot out of the air: the shot's arm thrust plays from it.
    shot_at: u32,
    shot_in_air: bool,
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
    /// The measured swing played on this web (`swing_anim.rs`): which take, the rope's angle from the vertical
    /// (towards the direction of flight) when it bit and on the last tick, and the tick the kick started.
    variant: usize,
    theta0: f32,
    theta_prev: f32,
    kick_at: Option<u32>,
    /// The downswing's row when the kick began: the kick is blended in from it.
    kick_from: f32,
    /// The body's fore / aft tip from the rope, followed a part of the way each tick.
    lean: f64,
    /// The flat direction the swing goes, followed slowly: the body's front and the arc's angle are read against
    /// it, not against the speed of the moment (which fades and turns at the top of the arc).
    heading: Option<P>,
    /// The take is played mirrored, the web on the left hand: decided as the web bites, by the side it is stuck on.
    mirror: Option<bool>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    Ground,
    Air,
    Flip(u32),
    Fall,
    /// Running up a wall, toward the unit direction of the wall.
    WallRun(P),
    /// Smacked into a wall too fast: stuck flat for a
    /// moment, then sliding off; ticks so far.
    Slap(P, u32),
    /// Over an obstacle on the ground: the move, past the rise yet, ticks.
    Vault(Move, bool, u32),
}

/// Ground parkour by the obstacle's height (video-spec
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
/// 2026-10-28).
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
const OVERHANG_REATTACH: u32 = 15;
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
    /// frames, 0 = it bites on the next tick. Default half: the full flight
    /// itself cuts the wait by about that much.
    pub web_delay: f64,
    /// How much past `MAX_SPEED` a long flight may go, as a share (`/swingmode speed`, percent there).
    pub long_flight_extra: f64,
    /// A jolt of the camera that fades: a slap sets it to 1.
    pub impact: f64,
    cam_wall: Option<P>,
    cam_blend: f32,
    /// This stay on the wall turns the camera (decided as you land on it).
    cam_long: Option<bool>,
    /// A phase whose pose is held for checking (`/swingmode pose`).
    debug: Option<Phase>,
    /// On the line, play a swing from the measured takes instead of the one fixed pose (`/swingmode anim`).
    pub swing_anim: bool,
    /// `/swingmode row V R`: stand in row R of swing variant V, to check the converted angles by eye.
    pub anim_debug: Option<(usize, usize)>,
    /// `/swingmode take N`: every web plays take N, to check the takes one by one.
    pub anim_take: Option<usize>,
    /// Bench view: turn the held row's model by this many degrees from the player's facing (90 = its right side to the camera).
    pub anim_debug_turn: f32,
    /// The body's turn in flight (world from model axes, about the middle), eased as one rotation from pose to
    /// pose; `rot_live` while it is what the model shows, and the facing it gives the renderer.
    rot: M,
    rot_live: bool,
    anim_facing: Option<f32>,
    /// Which kind of pose played last tick, and the ticks left to ease across a change of kind.
    source: u8,
    seam: u32,
    /// The facing and twist the renderer got last tick.
    sent: (Option<f32>, f32),
    /// The web hand this tick (from the feet), where the rope is drawn from.
    anim_hand: Option<P>,
    anim_variant: Option<usize>,
    /// The take on the web, and what follows its release, is mirrored.
    anim_mirror: bool,
    /// What follows a release, in play: the take, the tick it started, the body's turn at the release.
    after: Option<(usize, u32, M)>,
    /// What a take goes on with once its after-block has played out (`swing_anim::CONT`): the take, the tick it
    /// started, the body's turn it started from.
    cont: Option<(usize, u32, M)>,
    /// Free flight: ticks in a row with too little level speed to glide, and the last heading it had. The
    /// spread-eagle waits `GLIDE_HOLD` such ticks: flashed for a moment (off a wall slap, at the top of an arc) it
    /// threw the arms out and back.
    slow: u32,
    glide_ahead: P,
    /// The release in play is a launch (`AFTER_LAUNCH`): the body's turn the measured turn starts from, feet
    /// first along the flight at the release.
    launch: Option<M>,
    /// The level heading the release in play left with: the measured trunk after it is laid in that plane.
    after_ahead: P,
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
    /// Ticks in the air in a row with the mod driving: a long flight raises the speed cap.
    airborne: u32,
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
    /// Ticks after an overhang in which the wall above is caught without R
    /// (a walk under a balcony must not end in a fall).
    reattach: u32,
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
    /// Webs shot so far, for `/swingmode debug`, and zips begun.
    pub webs: u32,
    pub zips: u32,
    /// The last vault picked, for `/swingmode debug`.
    pub vault_note: String,
    /// Ticks a held pose has been easing out on the ground.
    ease_out: u32,
}

impl Default for Spidey {
    fn default() -> Self {
        Spidey {
            web_delay: 0.5,
            long_flight_extra: web::LONG_FLIGHT_EXTRA,
            impact: 0.0,
            cam_wall: None,
            cam_blend: 0.0,
            cam_long: None,
            debug: None,
            swing_anim: true,
            anim_debug: None,
            anim_take: None,
            anim_debug_turn: 0.0,
            rot: IDENTITY,
            rot_live: false,
            source: 0,
            seam: 0,
            sent: (None, 0.0),
            anim_facing: None,
            anim_hand: None,
            anim_variant: None,
            anim_mirror: false,
            after: None,
            launch: None,
            slow: 0,
            glide_ahead: P::new(0.0, 0.0, 1.0),
            cont: None,
            after_ahead: P::new(0.0, 0.0, 1.0),
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
            airborne: 0,
            shown: Vec::new(),
            body: [0.0; 2],
            asked: None,
            climb: 0.0,
            wall_space: false,
            wall_blocked: 0,
            overhang: None,
            reattach: 0,
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
            zips: 0,
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
    /// The renderer turns the model by roll (z), then pitch (x), then the facing (y, 180 - yaw), see `body_point`.
    /// Split `m` into those, taking of the two equal splits the one nearest the last tick's (no 180 jumps).
    fn place(&self, m: [[f64; 3]; 3], yaw: f32) -> ([f32; 2], f32) {
        let p = (-m[1][2]).clamp(-1.0, 1.0).asin();
        let q = m[1][0].atan2(m[1][1]);
        let t = m[0][2].atan2(m[2][2]);
        let near = |a: f64, b: f64| ((a - b + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI).abs();
        let prev = (f64::from(self.body[0]), f64::from(self.body[1]), f64::from(180.0 - self.anim_facing.unwrap_or(yaw)).to_radians());
        let alt = (std::f64::consts::PI - p, q + std::f64::consts::PI, t + std::f64::consts::PI);
        let cost = |c: (f64, f64, f64)| near(c.0, prev.0) + near(c.1, prev.1) + near(c.2, prev.2);
        let (p, q, t) = if cost(alt) < cost((p, q, t)) { alt } else { (p, q, t) };
        let unwrap = |a: f64, b: f64| b + (a - b + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
        ([unwrap(p, prev.0) as f32, unwrap(q, prev.1) as f32], (180.0 - t.to_degrees()) as f32)
    }
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
        // R held is the parkour run, W the direction: walking,
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
            // Off a roof, not a car: once the flip is done and the feet are
            // well below where the run started, the fall is the game's again
            // (WASD steers, a web can be shot), as a plain fall would be.
            if k as f64 >= total && h < -VAULT_DROP_HANDOFF {
                self.vault = None;
                self.asked = None;
                self.phase = Phase::Fall;
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
        let reattach = self.reattach > 0;
        self.reattach = self.reattach.saturating_sub(1);
        // Out from under an overhang, level, then up past its lip; the wall
        // run catches the wall above it on its own.
        if let Some((w, t)) = self.overhang {
            if t < OVERHANG_OUT_TICKS {
                self.overhang = Some((w, t + 1));
                // Level: what gravity takes off is given back.
                return Some(w * -OVERHANG_OUT + UP * web::FALL);
            }
            self.overhang = None;
            self.reattach = OVERHANG_REATTACH;
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
                // On the wall WASD walk it as a floor: W up,
                // S down, A/D along; nothing held clings. R held runs, as on
                // the ground: faster, and up with no key; letting
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
                // On a wall the camera turned onto (a tall one, in a build
                // that turns the view), the keys follow the view
                // laid on the wall (its pitch is kept off the wall's normal,
                // so this never degenerates): look down it and W goes down.
                // Upright on a low wall they go by the wall, as Catlike
                // Coding's climbing: W up, S down, D to the right of someone
                // facing it, A to the left; a view into the wall can't flip
                // them.
                let n = w * -1.0;
                let flat_look = look - n * look.dot(n);
                let turned = self.cam_long == Some(true) && horizon_mod_sdk::motion::WALL_TURN;
                let ahead_w = if turned && flat_look.len() > 0.3 { flat_look.norm() } else { UP };
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
            && (key(state, "KeyR") || reattach)
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
        self.reattach = 0;
        self.wall_blocked = 0;
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

/// A limb's direction in model axes (x right, y up, z back) from its (xRot, zRot), the convention of the measured
/// tables (`swing_anim.rs`): swung forward by -xRot from hanging down, then out to its side by zRot.
fn limb_dir(xrot: f32, zrot: f32, side: f32) -> P {
    let (a, b) = (f64::from(-xrot), f64::from(zrot * side));
    P::new(a.cos() * b.sin() * f64::from(side), -a.cos() * b.cos(), -a.sin()).norm()
}

/// The right arm's (xRot, zRot) that points it along `d` (model axes), the inverse of `limb_dir`: of the two
/// pairs that give the direction, the one nearer the angles it had.
fn limb_angles(d: P, last: [f32; 3]) -> (f32, f32) {
    let a = (-d.z).clamp(-1.0, 1.0).asin();
    let pairs = [(-a, d.x.atan2(-d.y)), (-(std::f64::consts::PI - a), (-d.x).atan2(d.y))];
    let far = |p: &(f64, f64)| {
        f64::from(wrap_angle(p.0 as f32 - last[0]).abs()) + f64::from(wrap_angle(p.1 as f32 - last[2]).abs())
    };
    let p = if far(&pairs[0]) <= far(&pairs[1]) { pairs[0] } else { pairs[1] };
    (p.0 as f32, p.1 as f32)
}

fn lerp_row<const N: usize>(rows: &[[f32; 8]; N], r: f32) -> [f32; 8] {
    let r = r.clamp(0.0, (N - 1) as f32);
    let (i, f) = (r.floor() as usize, r.fract());
    let (a, b) = (rows[i], rows[(i + 1).min(N - 1)]);
    std::array::from_fn(|k| a[k] + (b[k] - a[k]) * f)
}

/// One value read between the rows of a table.
fn lerp1<const N: usize>(rows: &[f32; N], r: f32) -> f32 {
    let r = r.clamp(0.0, (N - 1) as f32);
    let (i, f) = (r.floor() as usize, r.fract());
    rows[i] + (rows[(i + 1).min(N - 1)] - rows[i]) * f
}

/// The body's turn for a trunk at angle `th` in the plane of the flight heading `ahead` (level): 0 = head ahead
/// and belly down, a quarter turn up = upright facing ahead, a quarter turn down = head down, belly back. The
/// measured trunk angles (`swing_anim::TRUNK_*`) are in this frame.
fn plane_turn(ahead: P, th: f64) -> M {
    let (head, back) = (ahead * th.cos() + UP * th.sin(), UP * th.cos() - ahead * th.sin());
    let right = P::new(-ahead.z, 0.0, ahead.x);
    [[right.x, head.x, back.x], [right.y, head.y, back.y], [right.z, head.z, back.z]]
}

fn limb_parts(r: &[f32]) -> Vec<(&'static str, [f32; 3])> {
    vec![
        ("right_arm", [r[0], 0.0, r[1]]),
        ("left_arm", [r[2], 0.0, r[3]]),
        ("right_leg", [r[4], 0.0, r[5]]),
        ("left_leg", [r[6], 0.0, r[7]]),
    ]
}

/// The body turn (world from model axes) that lays the model-space `arm` along the world `rope` and turns the
/// model's front (-z) as close as it can to `travel`.
fn align(arm: P, rope: P, travel: P) -> [[f64; 3]; 3] {
    let frame = |a: P, f: P| {
        let a = a.norm();
        let mut b = f - a * f.dot(a);
        if b.len() < 1e-6 {
            b = P::new(0.0, 1.0, 0.0) - a * a.y;
        }
        let b = b.norm();
        (a, b, a.cross(b))
    };
    let (e1, e2, e3) = frame(arm, P::new(0.0, 0.0, -1.0));
    let (w1, w2, w3) = frame(rope, travel);
    let col = |p: P| [p.x, p.y, p.z];
    let (l, w) = ([col(e1), col(e2), col(e3)], [col(w1), col(w2), col(w3)]);
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| w[k][i] * l[k][j]).sum()))
}

/// Where a pose puts the body: the pitch and roll of the old poses (the facing is the view's or the phase's), or
/// a whole turn.
#[derive(Clone, Copy)]
enum Place {
    Angles([f32; 2]),
    Turn(M),
}

/// The head's (xRot, yRot) that looks along the world direction `d` from a body turned by `m`, within what a
/// neck allows; a direction behind the back only tips the head.
fn look(m: M, d: P) -> [f32; 3] {
    let l = P::new(
        m[0][0] * d.x + m[1][0] * d.y + m[2][0] * d.z,
        m[0][1] * d.x + m[1][1] * d.y + m[2][1] * d.z,
        m[0][2] * d.x + m[1][2] * d.y + m[2][2] * d.z,
    );
    let up = l.y.clamp(-1.0, 1.0).asin().clamp(-HEAD_PITCH, HEAD_PITCH);
    let turn = if l.z < 0.2 { l.x.atan2(-l.z).clamp(-HEAD_TURN, HEAD_TURN) } else { 0.0 };
    [-(up as f32), turn as f32, 0.0]
}

/// The file `SPIDEY_TRACE` names, read once.
fn trace_path() -> Option<&'static str> {
    static PATH: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    PATH.get_or_init(|| std::env::var("SPIDEY_TRACE").ok().filter(|p| !p.is_empty())).as_deref()
}

/// A turn as a matrix: world from model axes (x right, y up, z back).
type M = [[f64; 3]; 3];
const IDENTITY: M = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// The turn the renderer makes of a pitch, a roll and a facing: roll (z), then pitch (x), then 180 - facing (y).
fn euler_mat(body: [f32; 2], facing: f32) -> M {
    let (p, r, t) = (f64::from(body[0]), f64::from(body[1]), f64::from(180.0 - facing).to_radians());
    let rz = [[r.cos(), -r.sin(), 0.0], [r.sin(), r.cos(), 0.0], [0.0, 0.0, 1.0]];
    let rx = [[1.0, 0.0, 0.0], [0.0, p.cos(), -p.sin()], [0.0, p.sin(), p.cos()]];
    let ry = [[t.cos(), 0.0, t.sin()], [0.0, 1.0, 0.0], [-t.sin(), 0.0, t.cos()]];
    mat_mul(ry, mat_mul(rx, rz))
}

fn mat_quat(m: M) -> [f64; 4] {
    let t = m[0][0] + m[1][1] + m[2][2];
    if t > 0.0 {
        let s = (t + 1.0).sqrt() * 2.0;
        return [0.25 * s, (m[2][1] - m[1][2]) / s, (m[0][2] - m[2][0]) / s, (m[1][0] - m[0][1]) / s];
    }
    let i = if m[0][0] >= m[1][1] && m[0][0] >= m[2][2] { 0 } else if m[1][1] >= m[2][2] { 1 } else { 2 };
    let (j, k) = ((i + 1) % 3, (i + 2) % 3);
    let s = (1.0 + m[i][i] - m[j][j] - m[k][k]).max(1e-12).sqrt() * 2.0;
    let mut q = [0.0; 4];
    q[0] = (m[k][j] - m[j][k]) / s;
    q[i + 1] = 0.25 * s;
    q[j + 1] = (m[j][i] + m[i][j]) / s;
    q[k + 1] = (m[k][i] + m[i][k]) / s;
    q
}

/// `k` of the way from turn `a` to turn `b`, the short way round, as one rotation (no axis-by-axis wobble).
fn turn_step(a: M, b: M, k: f64, most: f64) -> M {
    let (qa, mut qb) = (mat_quat(a), mat_quat(b));
    let mut d: f64 = (0..4).map(|i| qa[i] * qb[i]).sum();
    if d < 0.0 {
        qb = qb.map(|v| -v);
        d = -d;
    }
    let th = d.clamp(-1.0, 1.0).acos();
    // `th` is half the angle between the turns: at most `most` of turn a step.
    let k = if th * 2.0 * k > most { most / (th * 2.0) } else { k };
    if k >= 1.0 {
        return b;
    }
    let (wa, wb) = if th < 1e-4 { (1.0 - k, k) } else { (((1.0 - k) * th).sin() / th.sin(), (k * th).sin() / th.sin()) };
    let q: [f64; 4] = std::array::from_fn(|i| qa[i] * wa + qb[i] * wb);
    quat_mat(q.map(|v| v as f32))
}

/// A model-space point (from the feet) turned by the body turn `m` about the middle, like `body_point`.
fn turn_point(m: [[f64; 3]; 3], local: P) -> P {
    let mid = P::new(0.0, 0.9, 0.0);
    let v = local - mid;
    let r: [f64; 3] = std::array::from_fn(|i| m[i][0] * v.x + m[i][1] * v.y + m[i][2] * v.z);
    P::new(r[0], r[1], r[2]) + mid
}

fn mat_mul(a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

fn quat_mat(q: [f32; 4]) -> [[f64; 3]; 3] {
    let n = q.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>().sqrt().max(1e-9);
    let [w, x, y, z] = q.map(|v| f64::from(v) / n);
    [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - w * z), 2.0 * (x * z + w * y)],
        [2.0 * (x * y + w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - w * x)],
        [2.0 * (x * z - w * y), 2.0 * (y * z + w * x), 1.0 - 2.0 * (x * x + y * y)],
    ]
}

fn angle_step(from: f32, to: f32, k: f32) -> f32 {
    let d = (to - from + PI).rem_euclid(TAU) - PI;
    from + d * k
}

impl Spidey {
    /// The wall the camera turns onto during a long run, and how far it has
    /// turned (0..1). Whether it turns is decided once, as you land on the
    /// wall (a tall wall turns it, a low one doesn't), and
    /// held until you leave it: the wall above changing on the way up (a
    /// window ledge, the last floors) never turns it back and forth.
    pub fn wall_camera(&mut self, state: &Sense) -> (Option<P>, f32) {
        // Out from under an overhang is still on the wall.
        let on_wall = matches!(self.phase, Phase::WallRun(_))
            || ((self.overhang.is_some() || self.reattach > 0) && self.cam_long.is_some());
        if !on_wall {
            self.cam_long = None;
        } else if self.cam_long.is_none() {
            self.cam_long = Some(state.wall_above >= LONG_WALL);
        }
        let long = match self.phase {
            _ if self.cam_long != Some(true) => None,
            Phase::WallRun(w) => Some(w),
            _ => self.cam_wall,
        };
        if let Some(w) = long {
            self.cam_wall = Some(w);
        }
        let target = if long.is_some() { 1.0 } else { 0.0 };
        self.cam_blend += (target - self.cam_blend).clamp(-WALL_CAM_STEP, WALL_CAM_STEP);
        if self.cam_blend <= 0.0 {
            self.cam_wall = None;
        }
        // The camera eases the turn itself (at a capped speed); it gets
        // the decision.
        (self.cam_wall, target)
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
        let long_flight_extra = self.long_flight_extra;
        *self = Spidey::default();
        self.web_delay = web_delay;
        self.long_flight_extra = long_flight_extra;
        motion::clear_pose();
    }

    /// Ticks in the air in a row with the mod driving.
    pub fn airborne(&self) -> u32 {
        self.airborne
    }

    /// The speed cap: `MAX_SPEED`, raised by up to `LONG_FLIGHT_EXTRA` in a long flight.
    pub fn speed_cap(&self) -> f64 {
        let ramp = self.airborne.saturating_sub(web::LONG_FLIGHT_START) as f64 / web::LONG_FLIGHT_RAMP as f64;
        web::MAX_SPEED * (1.0 + self.long_flight_extra * ramp.min(1.0))
    }

    /// One tick: the velocity to set, if the mod drives this tick.
    pub fn tick(&mut self, state: &Sense, chest: P, mut v: P) -> Option<P> {
        self.ticks += 1;
        self.airborne = if self.driving && !state.on_ground { self.airborne.saturating_add(1) } else { 0 };
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
        // R: in the air it shoots, let go on release; on the
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
                // Swing down the middle of the street (the arc is bent to
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
                    // A different take on each web, in a fixed shuffle (no RNG in the mod).
                    shot_at: self.ticks,
                    shot_in_air: self.phase != Phase::Ground,
                    variant: self.anim_take.unwrap_or((self.webs as usize * 5) % anim::SWINGS.len()),
                    theta0: 0.0,
                    theta_prev: 0.0,
                    kick_at: None,
                    kick_from: 0.0,
                    lean: 0.0,
                    heading: None,
                    mirror: None,
                });
                self.zip = None;
                self.driving = true;
                // Just off the ground: a low arc that lifts you (the web
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
                self.zips += 1;
                self.zip_flying = flight;
                self.zip_flight = flight;
                self.web = None;
                self.driving = false;
            }
        }
        // Point launch:
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
            return Some(v.clamp(self.speed_cap()));
        }
        // Parkour: running up a wall (the reference run,
        // ~3 s up the facade at ~20 m/s, a vault
        // over the ledge, on up the next tier). Flying into a wall while you
        // face it starts the run; A/D move along it; the wall ending at the
        // top is a vault over the edge; Space kicks off it.
        // Ground parkour by the obstacle's size:
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
                    // from here to the point over an upper arc. The fall is
                    // forgotten from here, as for a web.
                    self.safe = true;
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
                    // From the ground the arc's bottom stays above it (
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
            // Landing fast: a roof run (the reference,
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
        // The fall is forgotten from the moment the web bites, not the shot:
        // a web still flying when you hit the ground does not save you (its
        // flight time follows the web delay).
        if self.web.as_ref().is_some_and(|w| w.flying == 0) {
            self.safe = true;
        }
        if state.on_ground && self.web.is_none() && self.zip.is_none() {
            self.safe = false;
        }
        if let Some(run) = self.ground_run(state, v, ahead, side) {
            return Some(run);
        }
        // Parkour in flight (the reference: low
        // under an overpass, over and through trees): what is ahead along the
        // flight decides a hop over or a duck under.
        if self.driving && !state.on_ground && self.zip.is_none() {
            v = self.dodge(state, v);
        }
        let out = self.driving.then_some(v.clamp(self.speed_cap()));
        self.asked = out;
        out
    }

    pub fn ropes(&self, chest: P, yaw: f32) -> Vec<Rope> {
        let mut out = Vec::new();
        // The raised hand; a flying web leaves the arm pointed at it.
        let hand = |to: P, flying: bool| {
            let point = |local: P| if self.rot_live { turn_point(self.rot, local) } else { body_point(local, self.body, yaw) };
            if let Some(h) = self.anim_hand {
                h
            } else if flying {
                let shoulder = point(SHOULDER);
                let feet = chest - UP * super::CHEST;
                shoulder + (to - (feet + shoulder)).norm() * 0.7
            } else {
                point(HAND_UP)
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
        // In flight the body turns as one rotation, eased from pose to pose; on the ground and on a wall the
        // facing is the phase's own.
        let flight = matches!(self.phase, Phase::Air | Phase::Flip(_) | Phase::Fall);
        let anim_on = flight && self.swing_anim;
        // A web let go this tick: its take goes on with what follows the release.
        if let Some(w) = self.web.as_ref() {
            self.anim_variant = Some(w.variant);
        } else if let Some(vi) = self.anim_variant.take() {
            if anim_on && self.rot_live {
                // What follows is chosen by how the release leaves: thrown well upward = the measured launch, whatever
                // the take; an inverting take on a flat or falling release hands over to a level one.
                let vi = if INVERTING[vi.min(8)] && v.y < INVERT_RISE { LEVEL_TAKES[self.webs as usize % LEVEL_TAKES.len()] } else { vi };
                self.after = Some((vi, self.ticks, self.rot));
                let flat = P::new(v.x, 0.0, v.z);
                let f = f64::from(yaw).to_radians();
                self.after_ahead = if flat.len() > 0.05 { flat.norm() } else { P::new(-f.sin(), 0.0, f.cos()) };
                // The launch is one continuation among the takes' own (takes 3 and 6 only): played on every rising
                // release it was most of what the owner saw (44 of 70 releases, 26.11).
                let launch = AFTER_LAUNCH[vi.min(AFTER_LAUNCH.len() - 1)] && v.y > LAUNCH_RISE && flat.len() > 0.05;
                self.launch = launch.then(|| {
                    // The head's angle in the plane of the flight (0 ahead, a quarter turn up). The footage has
                    // him near vertical, head down, the moment the web goes (the measured trunk is 20 degrees
                    // off the vertical there), whatever way he flies; the belly faces back, against the flight.
                    let (ahead, th) = (flat.norm(), LAUNCH_START);
                    let (head, back) = (ahead * th.cos() + UP * th.sin(), UP * th.cos() - ahead * th.sin());
                    let right = P::new(-ahead.z, 0.0, ahead.x);
                    [[right.x, head.x, back.x], [right.y, head.y, back.y], [right.z, head.z, back.z]]
                });
            }
        }
        if let Some((si, t0, _)) = self.after {
            // A launch lasts until the next web, the ground or a fast fall; any other release as long as its take.
            let over = match self.launch {
                Some(_) => (self.ticks - t0) as usize >= anim::SWINGS[LAUNCH_TAKE].after_ticks + anim::CONT_TICKS[LAUNCH_TAKE],
                None => (self.ticks - t0) as usize >= anim::SWINGS[si].after_ticks,
            };
            if !anim_on || self.web.is_some() || over {
                // Played out to its end in free flight: the take goes on with its own flight, from the turn the
                // body has now, so nothing jumps at the join.
                if anim_on && self.web.is_none() && self.launch.is_none() && anim::CONT_TICKS[si] > 0 {
                    self.cont = Some((si, self.ticks, self.rot));
                }
                self.after = None;
            }
        }
        if let Some((ci, t0, _)) = self.cont {
            if !anim_on || self.web.is_some() || (self.ticks - t0) as usize >= anim::CONT_TICKS[ci] {
                self.cont = None;
            }
        }
        if self.after.is_none() {
            self.launch = None;
        }
        self.anim_hand = None;
        let ticks = self.ticks;
        let held = if self.rot_live { Place::Turn(self.rot) } else { Place::Angles(self.body) };
        // Where the head looks in flight: the way we fly.
        let fly = if v.len() > 0.1 { Some(v.norm()) } else { None };
        // Where the web arm reaches (from the feet), when a pose aims it.
        let mut aim: Option<P> = None;
        // The pose is played mirrored: left for right.
        let mut mirror = false;
        // A web was just shot out of the air: its arm snaps to the anchor instead of easing there.
        let mut thrust = false;
        // Target: parts (xRot, yRot, zRot), the body's turn, whether the pose is itself motion (no easing), and
        // the kind of pose (a change of kind is eased across).
        let (parts, place, snap, source): (Vec<(&str, [f32; 3])>, Place, bool, u8) = if let Some((vi, ri)) = self.anim_debug {
            // Bench: the row under a vertical rope, front towards the player's facing.
            let take = &anim::SWINGS[vi.min(anim::SWINGS.len() - 1)];
            let r = take.line[ri.min(anim::LINE_ROWS - 1)];
            let f = f64::from(yaw + self.anim_debug_turn).to_radians();
            let m = align(limb_dir(r[0], r[1], 1.0), UP, P::new(-f.sin(), 0.0, f.cos()));
            (limb_parts(&r), Place::Turn(m), true, 3)
        } else if let Some(t) = self.zip {
            let dir = (t - chest).norm();
            (
                vec![
                    ("right_arm", [-3.0, 0.0, 0.1]),
                    ("left_arm", [0.2, 0.0, -0.3]),
                    ("right_leg", [0.15, 0.0, 0.05]),
                    ("left_leg", [0.15, 0.0, -0.05]),
                ],
                Place::Angles(tilt(dir, yaw)),
                false,
                0,
            )
        } else if let Some(w) = self.web.as_mut().filter(|_| anim_on) {
            // A swing in three parts: the web shot, then on the line the
            // downswing and the upswing follow the position in the arc and the kick at the bottom plays in real
            // time. The body is placed so that the web arm lies along the rope and the front faces along the arc.
            // The body hangs on the line the swing turns about (over the flight), leaning a little to where the
            // web is stuck; the web arm alone reaches all the way there. Laid whole along the web, the body
            // rolled onto its side whenever the web was stuck off to one side.
            let rope = ((w.pivot - chest).norm() * (1.0 - WEB_LEAN) + (w.anchor - chest).norm() * WEB_LEAN).norm();
            let stuck = w.anchor - (chest - UP * super::CHEST);
            let flat = P::new(v.x, 0.0, v.z);
            let f = f64::from(yaw).to_radians();
            let mut heading = w.heading.unwrap_or(if flat.len() > 0.1 { flat.norm() } else { P::new(-f.sin(), 0.0, f.cos()) });
            if flat.len() > HEADING_SPEED {
                let h = heading * (1.0 - HEADING_EASE) + flat.norm() * HEADING_EASE;
                if h.len() > 1e-3 {
                    heading = h.norm();
                }
            }
            w.heading = Some(heading);
            let take = &anim::SWINGS[w.variant];
            let (tr_intro, tr_line) = (&anim::TRUNK_INTRO[w.variant], &anim::TRUNK_LINE[w.variant]);
            let (row, trunk): ([f32; 8], f32) = if w.flying > 0 {
                let x = 1.0 - w.flying as f32 / w.flight.max(1) as f32;
                let r = x * (anim::INTRO_ROWS - 1) as f32;
                (lerp_row(&take.intro, r), lerp1(tr_intro, r))
            } else {
                // The rope's angle from the vertical: positive while the anchor is still ahead (going down).
                let theta = rope.dot(heading).atan2(rope.y) as f32;
                if w.theta0 == 0.0 {
                    w.theta0 = theta.abs().max(0.3);
                    w.theta_prev = theta;
                }
                let omega = w.theta_prev - theta;
                w.theta_prev = theta;
                // The kick plays at the take's own speed.
                let kick_secs = anim::KICK_SECS;
                let half_kick = kick_secs * 0.5 * TICKS_PER_SEC;
                let down = (1.0 - theta / w.theta0).clamp(0.0, 1.0) * (anim::DOWN_ROWS - 1) as f32;
                if w.kick_at.is_none() && (theta <= 0.0 || (omega > 1e-3 && theta / omega <= half_kick)) {
                    w.kick_at = Some(ticks);
                    w.kick_from = down;
                }
                let up_rows = anim::LINE_ROWS - anim::DOWN_ROWS - anim::KICK_ROWS;
                match w.kick_at {
                    None => (lerp_row(&take.line, down), lerp1(tr_line, down)),
                    Some(t0) => {
                        let kt = (ticks - t0) as f32 / TICKS_PER_SEC;
                        if kt < kick_secs {
                            // The kick starts when the bottom is near, wherever the downswing's rows had got
                            // to: it comes in from that row over its first half, not in one tick.
                            let kr = anim::DOWN_ROWS as f32 + kt / kick_secs * (anim::KICK_ROWS - 1) as f32;
                            let kick = lerp_row(&take.line, kr);
                            let from = lerp_row(&take.line, w.kick_from);
                            let x = (kt / (kick_secs * 0.5)).clamp(0.0, 1.0);
                            let x = x * x * (3.0 - 2.0 * x);
                            let (tk, tf) = (lerp1(tr_line, kr), lerp1(tr_line, w.kick_from));
                            (std::array::from_fn(|i| from[i] + (kick[i] - from[i]) * x), tf + (tk - tf) * x)
                        } else {
                            let r = (anim::DOWN_ROWS + anim::KICK_ROWS) as f32 + (-theta / w.theta0).clamp(0.0, 1.0) * (up_rows - 1) as f32;
                            (lerp_row(&take.line, r), lerp1(tr_line, r))
                        }
                    }
                }
            };
            // The shot's thrust (measured, `swing_anim::SHOT`): from the tick the web leaves, laid over the take's
            // rows and fading out, so the reach belongs to a real shot and never plays without one.
            let since = (ticks - w.shot_at) as usize;
            let row: [f32; 8] = if w.shot_in_air && since < anim::SHOT_TICKS {
                let x = since as f32 / anim::SHOT_TICKS as f32;
                let weight = 1.0 - x * x;
                thrust = since < SHOT_SNAP;
                std::array::from_fn(|i| angle_step(row[i], anim::SHOT[since][i], weight))
            } else {
                row
            };
            // Along the arc: square to the rope in the plane of the swing. It never fades or turns round with
            // the speed, so the body holds still at the top of the arc.
            let right = heading.cross(UP);
            let along = rope.cross(right);
            let along = if along.len() > 1e-3 { along.norm() } else { heading };
            // The web hand is the one on the side the web is stuck on (the legs then sweep the other way); dead
            // ahead, the takes alternate.
            let off = (w.anchor - chest).dot(right);
            let odd = w.variant % 2 == 1;
            mirror = *w.mirror.get_or_insert(if off.abs() > WEB_SIDE { off < 0.0 } else { odd });
            // The body stands along the rope, front along the arc, tipped fore or aft as the take's web arm was
            // from its trunk, within bounds. (Laid by that arm outright, a take whose arm reached sideways put
            // the whole body across the flight.)
            let arm = limb_dir(row[0], row[1], 1.0);
            // (Fading out as the arm leaves the overhead: its tip then says nothing of the trunk.)
            let over = (arm.y / 0.6).clamp(0.0, 1.0);
            let lean = arm.z.atan2(arm.y.max(1e-3)).clamp(-BODY_LEAN, BODY_LEAN) * over * over * (3.0 - 2.0 * over);
            w.lean += (lean - w.lean) * LEAN_EASE;
            let lean = w.lean;
            // The trunk as measured against the vertical, in the plane of the flight; the rope's rule under it
            // by `TRUNK_ON_WEB`.
            let m = turn_step(align(P::new(0.0, lean.cos(), lean.sin()), rope, along), plane_turn(heading, f64::from(trunk)), TRUNK_ON_WEB, f64::INFINITY);
            let mut parts = limb_parts(&row);
            parts.push(("head", look(m, fly.unwrap_or(heading))));
            aim = Some(stuck);
            if w.flying > 0 { (parts, Place::Turn(m), false, 4) } else { (parts, Place::Turn(m), true, 1) }
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
                    held,
                    false,
                    0,
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
                    Place::Angles(tilt(rope, yaw)),
                    false,
                    0,
                )
            }
        } else if let (Some((_, t0, _)), Some(_)) = (self.after, self.launch) {
            // Thrown upward off the web: the measured launch, its turn counted from a body laid feet first along
            // the flight (not from the pose the release happened to have), then the measured glide.
            let take = &anim::SWINGS[LAUNCH_TAKE];
            let (t, n) = ((ticks - t0) as usize, take.after_ticks);
            mirror = self.anim_mirror;
            let side = if mirror { -1.0 } else { 1.0 };
            let turn = |r: &[f32; 12]| quat_mat([r[8], r[9], r[10] * side, r[11] * side]);
            // After the take's own tail, the take's own flight: one unbroken measured piece (the generic flight block
            // stitched on here made the arms jump at the join and thrust at its end).
            let cn = anim::CONT_TICKS[LAUNCH_TAKE];
            let row = |k: usize| if k < n { take.after[k] } else { anim::CONT[LAUNCH_TAKE][(k - n).min(cn - 1)] };
            let trunk = &anim::TRUNK_AFTER[LAUNCH_TAKE];
            let m = if t < n {
                plane_turn(self.after_ahead, f64::from(trunk[t]))
            } else {
                mat_mul(plane_turn(self.after_ahead, f64::from(trunk[n - 1])), turn(&row(t)))
            };
            let now = row(t);
            let mut parts = Vec::with_capacity(5);
            for (l, name) in ["right_arm", "left_arm", "right_leg", "left_leg"].into_iter().enumerate() {
                let mut d = P::new(0.0, 0.0, 0.0);
                // Within one block only.
                let (lo, hi) = if t < n { (0, n - 1) } else { (n, n + cn - 1) };
                for k in t.saturating_sub(LAUNCH_SMOOTH).max(lo)..=(t + LAUNCH_SMOOTH).min(hi) {
                    let r = row(k);
                    d = d + limb_dir(r[2 * l], r[2 * l + 1], 1.0);
                }
                let (x, z) = if d.len() > 1e-3 { limb_angles(d.norm(), [now[2 * l], 0.0, now[2 * l + 1]]) } else { (now[2 * l], now[2 * l + 1]) };
                parts.push((name, [x, 0.0, z]));
            }
            if let Some(d) = fly {
                parts.push(("head", look(m, d)));
            }
            (parts, Place::Turn(m), false, 5)
        } else if let Some((ci, t0, m0)) = self.cont {
            // The take's own flight after its tail, turned on from where the tail left the body.
            let r = anim::CONT[ci][((ticks - t0) as usize).min(anim::CONT_TICKS[ci] - 1)];
            mirror = self.anim_mirror;
            let side = if mirror { -1.0 } else { 1.0 };
            let m = mat_mul(m0, quat_mat([r[8], r[9], r[10] * side, r[11] * side]));
            let mut parts = limb_parts(&r);
            if let Some(d) = fly {
                parts.push(("head", look(m, d)));
            }
            (parts, Place::Turn(m), true, 7)
        } else if anim_on && matches!(self.phase, Phase::Air | Phase::Fall) && self.after.is_none() && {
            let hs = (v.x * v.x + v.z * v.z).sqrt();
            if hs > GLIDE_SPEED {
                self.slow = 0;
                self.glide_ahead = P::new(v.x / hs, 0.0, v.z / hs);
            } else {
                self.slow += 1;
            }
            self.slow < GLIDE_HOLD
        } {
            // Free flight: the trunk along the flight path, limbs from the measured flight, drifting slowly
            // between its rows so the pose is never frozen.
            let hs = (v.x * v.x + v.z * v.z).sqrt();
            let ahead = self.glide_ahead;
            let th = v.y.atan2(hs.max(GLIDE_SPEED)).clamp(-GLIDE_PITCH.0, GLIDE_PITCH.1);
            let m = plane_turn(ahead, th);
            mirror = self.anim_mirror;
            let r = GLIDE_ROWS.0 + (GLIDE_ROWS.1 - GLIDE_ROWS.0) * (0.5 + 0.5 * (ticks as f32 * 0.12).sin());
            let (i, f) = ((r.floor() as usize).min(anim::AIR_TICKS - 2), r.fract());
            let (a, b) = (anim::AIR[i], anim::AIR[i + 1]);
            let row: [f32; 8] = std::array::from_fn(|k| a[k] + (b[k] - a[k]) * f);
            let mut parts = limb_parts(&row);
            parts.push(("head", look(m, if v.len() > 0.1 { v.norm() } else { ahead })));
            (parts, Place::Turn(m), false, 6)
        } else if let Some((si, t0, m0)) = self.after {
            // After letting go: the same take's flip / dive / spread, the body turned as measured since the release.
            let take = &anim::SWINGS[si];
            let r = take.after[((ticks - t0) as usize).min(take.after_ticks - 1)];
            mirror = self.anim_mirror;
            let side = if mirror { -1.0 } else { 1.0 };
            let m = mat_mul(m0, quat_mat([r[8], r[9], r[10] * side, r[11] * side]));
            let mut parts = limb_parts(&r);
            if let Some(d) = fly {
                parts.push(("head", look(m, d)));
            }
            (parts, Place::Turn(m), true, 2)
        } else {
            let (parts, body, snap) = match self.phase {
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
                    // Out of a flight the body comes upright as one rotation, to the view's facing: the facing
                    // alone snapped to it at the landing, half a turn in a tick after a tumble.
                    let upright = if self.rot_live {
                        let (a, b) = (mat_quat(self.rot), mat_quat(euler_mat([0.0, 0.0], yaw)));
                        (0..4).map(|i| a[i] * b[i]).sum::<f64>().abs() > 0.9987
                    } else {
                        self.body.iter().all(|a| a.abs() < 0.08)
                    };
                    let settled = upright && self.shown.iter().all(|(_, a)| a.iter().all(|x| x.abs() < 0.15));
                    if settled || self.ease_out > EASE_OUT_TICKS {
                        self.ease_out = 0;
                        if !self.shown.is_empty() {
                            self.shown.clear();
                            self.body = [0.0; 2];
                            self.rot_live = false;
                            self.anim_facing = None;
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
                    // decision 42).
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
            };
            (parts, Place::Angles(body), snap, 0)
        };
        if source != self.source {
            self.seam = SEAM_TICKS;
        }
        self.source = source;
        // Across a change of kind: an even cross-fade, 1/6 of what is left, then 1/5, … so it starts gently.
        let fade = if self.seam > 0 { 1.0 / self.seam as f32 } else { 1.0 };
        let k = if snap { fade } else { EASE.min(fade) };
        let k = if source == 6 { k.min(GLIDE_EASE) } else { k };
        self.seam = self.seam.saturating_sub(1);
        if matches!(source, 1 | 4) {
            self.anim_mirror = mirror;
        }
        let mut parts = parts;
        if mirror {
            for (name, a) in &mut parts {
                *name = match *name {
                    "right_arm" => "left_arm",
                    "left_arm" => "right_arm",
                    "right_leg" => "left_leg",
                    "left_leg" => "right_leg",
                    other => other,
                };
                *a = [a[0], -a[1], -a[2]];
            }
        }
        // In flight the body's facing is the pose's own, so the game's turn of the head (the view's against the
        // body's) would put the face anywhere, on the back too: every pose in flight names the head.
        let turning = flight || self.anim_debug.is_some() || (self.phase == Phase::Ground && self.rot_live);
        let target = match place {
            Place::Turn(m) => Some(m),
            Place::Angles(b) if turning => Some(euler_mat(b, yaw)),
            Place::Angles(_) => None,
        };
        if let Some(m) = target {
            if !parts.iter().any(|(n, _)| *n == "head") {
                let f = f64::from(yaw).to_radians();
                let ahead = if matches!(self.phase, Phase::Flip(_) | Phase::Ground) { [0.0; 3] } else { look(m, P::new(-f.sin(), 0.0, f.cos())) };
                parts.push(("head", ahead));
            }
        }
        // A take in full play moves as fast as it was made; what starts or ends a pose is held back.
        let held_back = !(matches!(source, 1 | 2 | 7) && snap && fade >= 1.0);
        let mut out = Vec::with_capacity(parts.len());
        for (name, target) in parts {
            let last = self
                .shown
                .iter()
                .find(|(n, _)| n == name)
                .map_or([0.0; 3], |(_, a)| *a);
            let k = if name == "head" { k.min(HEAD_EASE) } else { k };
            let a = [0, 1, 2].map(|i| angle_step(last[i], target[i], k));
            let a = if turning && held_back { [0, 1, 2].map(|i| last[i] + (a[i] - last[i]).clamp(-MAX_PART_STEP, MAX_PART_STEP)) } else { a };
            out.push((name.to_string(), a));
        }
        if let Some(target) = target {
            if !self.rot_live {
                // From what the model showed last: the phase's own facing and twist (a wall), or the view's.
                let (f, t) = self.sent;
                let t = f64::from(t);
                let twist = [[t.cos(), 0.0, t.sin()], [0.0, 1.0, 0.0], [-t.sin(), 0.0, t.cos()]];
                self.rot = mat_mul(euler_mat(self.body, f.unwrap_or(yaw)), twist);
            }
            let most = if held_back { MAX_TURN } else { f64::INFINITY };
            self.rot = turn_step(self.rot, target, f64::from(k), most);
            self.rot_live = true;
            let (body, facing) = self.place(self.rot, yaw);
            self.body = body;
            self.anim_facing = Some(facing);
            if let Some(to) = aim {
                // The web arm points from the shoulder the model shows to where the web is stuck, and the web
                // leaves that hand.
                let (side, name) = if mirror { (-1.0, "left_arm") } else { (1.0, "right_arm") };
                let shoulder = P::new(SHOULDER.x * side, SHOULDER.y, SHOULDER.z);
                let d = (to - turn_point(self.rot, shoulder)).norm();
                let m = self.rot;
                let l = P::new(
                    (m[0][0] * d.x + m[1][0] * d.y + m[2][0] * d.z) * side,
                    m[0][1] * d.x + m[1][1] * d.y + m[2][1] * d.z,
                    m[0][2] * d.x + m[1][2] * d.y + m[2][2] * d.z,
                );
                let last = self.shown.iter().find(|(n, _)| n == name).map_or([0.0; 3], |(_, a)| *a);
                let fs = side as f32;
                let (x, z) = limb_angles(l, [last[0], last[1], last[2] * fs]);
                let k = if thrust { fade.max(SHOT_REACH) } else { fade };
                let a = [angle_step(last[0], x, k), angle_step(last[1], 0.0, k), angle_step(last[2], z * fs, k)];
                if let Some(arm) = out.iter_mut().find(|(n, _)| n == name) {
                    arm.1 = a;
                }
                self.anim_hand = Some(turn_point(self.rot, shoulder + limb_dir(a[0], a[2], fs) * 0.66));
            }
        } else {
            let Place::Angles(body) = place else { unreachable!("only poses in flight turn the body as one rotation") };
            self.body = if snap {
                body
            } else {
                [angle_step(self.body[0], body[0], k), angle_step(self.body[1], body[1], k)]
            };
            self.rot_live = false;
            self.anim_facing = None;
        }
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
            _ if self.anim_facing.is_some() => (self.anim_facing, None),
            _ => (None, None),
        };
        self.sent = (facing, twist);
        // `SPIDEY_TRACE=<file>`: one line a tick with what the model was told, to look for jumps in numbers.
        if let Some(path) = trace_path() {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                let q = mat_quat(if self.rot_live { self.rot } else { euler_mat(self.body, facing.unwrap_or(yaw)) });
                let web = self.web.as_ref().map(|w| (w.flying, w.held, w.variant, w.mirror));
                let _ = writeln!(
                    f,
                    "{}\t{:?}\t{}\t{}\t{}\t{:?}\t{:.4},{:.4},{:.4},{:.4}\t{}\t{:.3},{:.3},{:.3}",
                    self.ticks,
                    self.phase,
                    source,
                    snap,
                    state.on_ground,
                    web,
                    q[0], q[1], q[2], q[3],
                    self.shown.iter().map(|(n, a)| format!("{n}:{:.3}:{:.3}:{:.3}", a[0], a[1], a[2])).collect::<Vec<_>>().join(" "),
                    v.x, v.y, v.z,
                );
            }
        }
        motion::set_pose(&parts, self.body[0], self.body[1], twist, facing, shift);
    }
}
