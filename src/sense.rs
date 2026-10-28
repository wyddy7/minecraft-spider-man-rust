//! What the mod knows of the world around the player each tick: the rays it
//! asks the game to cast (`motion.set-rays`) and what they found.
//!
//! The game casts the rays asked for in one tick at the start of the next,
//! from where the player then is, so `read` interprets this tick's hits with
//! the layout `ask` sent the tick before.

use crate::{P, UP};
use horizon_mod_sdk::motion::{self, Ray, State, Vec3 as V};

/// How far the long rays reach, in blocks: Danny's AoT's hook reach
/// (`maxHookDistance`), picked for webs too.
pub const RANGE: f64 = 250.0;
/// The anchor fan: yaw offsets from the view and elevations above the
/// horizon, in degrees.
const FAN_YAW: [f64; 5] = [-40.0, -20.0, 0.0, 20.0, 40.0];
const FAN_ELEVATION: [f64; 4] = [30.0, 45.0, 60.0, 75.0];
/// The hooks' spread from the crosshair, degrees.
const SPREAD: f64 = 3.0;
/// A wall beside the body: rays this long at these heights over the feet
/// (WallClimbManager.findAdjacentWall).
const WALL_REACH: f64 = 0.8;
const WALL_HEIGHTS: [f64; 2] = [0.2, 1.4];
const WALLS: [P; 4] = [
    P::new(1.0, 0.0, 0.0),
    P::new(-1.0, 0.0, 0.0),
    P::new(0.0, 0.0, 1.0),
    P::new(0.0, 0.0, -1.0),
];
/// How far a wall goes on above the feet: a ray into it every 2 blocks.
const WALL_ABOVE: usize = 16;
/// Rays straight ahead along the flight from these heights off the feet.
const AHEAD_HEIGHTS: [f64; 5] = [-1.0, 0.0, 1.0, 2.0, 3.0];
/// The ground ahead: every half block out to 10, a ray down from this
/// height over the feet to this depth below them (a wall that goes on reads
/// as the top).
const PROFILE_STEPS: usize = 20;
const PROFILE_TOP: f64 = 6.5;
const PROFILE_DEPTH: f64 = 3.0;
const PROFILE_MAX: f64 = 6.0;

/// The point a ray from the eyes hit.
#[derive(Clone, Debug)]
pub struct Probe {
    pub hit: Option<V>,
}

/// One tick's view of the world, as the mod reads it.
#[derive(Clone)]
pub struct Sense {
    /// Blocks per tick.
    pub velocity: V,
    pub on_ground: bool,
    pub eye: V,
    /// Unit view vector, turned with the wall camera.
    pub look: V,
    /// The crosshair's ray, and it turned left and right for a pair of hooks.
    pub aim: Probe,
    pub aim_left: Probe,
    pub aim_right: Probe,
    /// Rays above and ahead of the player for picking a swing anchor.
    pub fan: Vec<Probe>,
    /// A wall within reach of the body: the unit direction toward it.
    pub wall: Option<V>,
    /// How many blocks the wall goes on above the feet.
    pub wall_above: f64,
    /// The height of the first block straight below the feet.
    pub floor: Option<f64>,
    /// How far each ray ahead along the flight met a block.
    pub ahead: Vec<Option<f64>>,
    /// The ground ahead along the move, every half block from 0.5 to 10:
    /// its top as a height over the feet (6: a wall that goes on); none for
    /// a drop deeper than 3. Empty when there is no direction to look in.
    pub profile: Vec<Option<f64>>,
}

/// What the rays sent last tick were, to read their hits.
#[derive(Default)]
pub struct Rays {
    /// The rays in the order sent; the eyes and feet they left from are
    /// this tick's, as the game casts them.
    sent: Vec<Ray>,
    profile: bool,
    wall: Option<P>,
}

impl Rays {
    /// This tick's view: the hits of the rays sent last tick.
    /// `feet` is the player's height, for the floor.
    pub fn read(&self, st: &State, feet: f64) -> Sense {
        let hit = |i: usize| st.hits.get(i).copied().flatten();
        let eye = P::of(&st.eye);
        let probe = |i: usize| {
            let dir = self.sent.get(i).map_or(P::default(), |r| P::of(&r.dir));
            Probe {
                hit: hit(i).map(|d| (eye + dir * d).wit()),
            }
        };
        let mut i = 0;
        let mut next = |n: usize| {
            let at = i;
            i += n;
            at
        };
        let (aim, aim_left, aim_right) = (probe(next(1)), probe(next(1)), probe(next(1)));
        let fan_at = next(FAN_YAW.len() * FAN_ELEVATION.len());
        let fan = (0..FAN_YAW.len() * FAN_ELEVATION.len())
            .map(|k| probe(fan_at + k))
            .collect();
        let walls_at = next(WALLS.len() * WALL_HEIGHTS.len());
        let wall = (0..WALLS.len())
            .find(|w| (0..WALL_HEIGHTS.len()).any(|h| hit(walls_at + w * WALL_HEIGHTS.len() + h).is_some()))
            .map(|w| WALLS[w].wit());
        let above_at = next(WALL_ABOVE);
        // Rays into last tick's wall; this tick's wall must be the same one.
        let same_wall = wall.as_ref().map(P::of).is_some_and(|w| self.wall == Some(w));
        let wall_above = if same_wall {
            (0..WALL_ABOVE).take_while(|k| hit(above_at + k).is_some()).count() as f64 * 2.0
        } else {
            0.0
        };
        let floor_at = next(1);
        let floor = hit(floor_at).map(|d| feet + 0.1 - d);
        let ahead_at = next(AHEAD_HEIGHTS.len());
        let ahead = (0..AHEAD_HEIGHTS.len()).map(|k| hit(ahead_at + k)).collect();
        let profile_at = next(PROFILE_STEPS);
        let profile = if self.profile && st.hits.len() >= profile_at + PROFILE_STEPS {
            (0..PROFILE_STEPS)
                .map(|k| hit(profile_at + k).map(|d| (PROFILE_TOP - d).min(PROFILE_MAX)))
                .collect()
        } else {
            Vec::new()
        };
        Sense {
            velocity: st.velocity,
            on_ground: st.on_ground,
            eye: st.eye,
            look: st.look,
            aim,
            aim_left,
            aim_right,
            fan,
            wall,
            wall_above,
            floor,
            ahead,
            profile,
        }
    }

    /// Sends next tick's rays, laid out as `read` expects them. `yaw` is
    /// the player's, in degrees as F3 shows it.
    pub fn ask(&mut self, sense: &Sense, yaw: f32) {
        let ray = |from_eyes: bool, offset: P, dir: P, distance: f64, solid: bool| Ray {
            from_eyes,
            offset: offset.wit(),
            dir: dir.norm().wit(),
            distance,
            solid,
        };
        let eye_ray = |dir: P| ray(true, P::default(), dir, RANGE, false);
        let look = P::of(&sense.look);
        let yaw = f64::from(yaw);
        let mut rays = vec![
            eye_ray(look),
            eye_ray(turn_about_up(look, -SPREAD)),
            eye_ray(turn_about_up(look, SPREAD)),
        ];
        for off in FAN_YAW {
            for elevation in FAN_ELEVATION {
                rays.push(eye_ray(view_vector(yaw + off, -elevation)));
            }
        }
        for w in WALLS {
            for h in WALL_HEIGHTS {
                rays.push(ray(false, UP * h, w, WALL_REACH, false));
            }
        }
        // How far up this tick's wall goes, read next tick.
        self.wall = sense.wall.as_ref().map(P::of);
        let into = self.wall.unwrap_or_default();
        for k in 1..=WALL_ABOVE {
            rays.push(ray(false, UP * (k as f64 * 2.0), into, 1.0, false));
        }
        rays.push(ray(false, UP * 0.1, P::new(0.0, -1.0, 0.0), RANGE, false));
        let flat = P::of(&sense.velocity).flat();
        let reach = (flat.len() * 14.0).clamp(3.0, 40.0);
        let ahead = if flat.len() > 0.05 { flat.norm() } else { P::default() };
        for h in AHEAD_HEIGHTS {
            rays.push(ray(false, UP * h, ahead, reach, false));
        }
        // Along the move, or the flat view when standing.
        let along = if flat.len() > 0.05 { flat.norm() } else { look.flat().norm() };
        self.profile = along.len() > 0.5;
        for k in 1..=PROFILE_STEPS {
            let at = along * (k as f64 * 0.5) + UP * PROFILE_TOP;
            rays.push(ray(false, at, P::new(0.0, -1.0, 0.0), PROFILE_TOP + PROFILE_DEPTH, true));
        }
        motion::set_rays(&rays);
        self.sent = rays;
    }
}

/// `Entity.calculateViewVector`: degrees as F3 shows them.
fn view_vector(yaw: f64, pitch: f64) -> P {
    let (y, p) = (yaw.to_radians(), pitch.to_radians());
    P::new(-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos())
}

/// `v` turned about the world's up as `degrees` more yaw turns the view.
fn turn_about_up(v: P, degrees: f64) -> P {
    let (s, c) = degrees.to_radians().sin_cos();
    P::new(v.x * c - v.z * s, v.y, v.x * s + v.z * c)
}
