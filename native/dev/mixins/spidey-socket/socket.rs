// The game's side of the mod: Horizon's former `motion` capability as native
// hooks. Each tick it hands the mod (in development, a library reloaded while
// the game runs) the player, its keys and the hits of the rays it asked for, then
// applies what comes back: velocity, the fall reset (and the own world's
// server hearing the player as grounded), ropes, the pose, the camera and its
// shake, chat lines. Acts only in the player's own world (single player or a
// LAN world they host); on another server the mod only sees keys. Values from
// the mod that are not finite are dropped, and a panic in it stops the mod for
// that tick, not the game.

use horizon_dev_reload::abi::{Camera, Input, Player, Pose, Ray, State, Vec3};
use std::ops::ControlFlow;
use std::sync::Mutex;
use std::time::Instant;

/// Rays cast a tick; the rest find nothing.
const RAY_BUDGET: usize = 256;
/// How far from the player a fixed camera may stand, in blocks.
const FIXED_CAMERA_REACH: f64 = 16.0;
/// The plug's chat commands, taken before the server sees them.
const COMMANDS: [&str; 1] = ["swingmode"];
/// The strongest shake: yaw and pitch, degrees.
const SHAKE_DEGREES: (f32, f32) = (3.0, 2.0);
/// The camera's advisor on a turned wall (Web of Shadows): a mouse left alone
/// this long (s) lets the view drift behind the run at this rate (1/s, ~1 s
/// to settle), looking this far down onto the wall (radians), once the run
/// moves at least this much a tick (blocks).
const STEER_IDLE: f64 = 0.4;
const STEER_RATE: f64 = 2.5;
const STEER_DOWN: f64 = 0.35;
const STEER_MOVE: f64 = 0.02;

struct Shared {
    commands: Vec<(String, String)>,
    /// Last tick's pose and this tick's, blended per frame.
    poses: [Option<Pose>; 2],
    camera: Option<Camera>,
    /// The plug forgot the fall this tick; the on-ground value to restore
    /// after the state packet.
    safe_fall: bool,
    saved_on_ground: Option<bool>,
    cam_ease: Option<(f64, f64, Instant)>,
    wall_ease: (f64, Option<Instant>),
    /// Jolts (start, strength, seconds) and the noise's own clock.
    jolts: Vec<(Instant, f32, f32)>,
    shake_epoch: Option<Instant>,
    /// The playing wind loop, if one was started.
    wind: Option<u64>,
    /// The wall turn last frame, the view it left, when the mouse last moved
    /// and the last frame's time (see `steer`).
    steer_turn: Option<glam::DQuat>,
    steer_seen: Option<(f32, f32)>,
    steer_idle: Option<Instant>,
    steer_then: Option<Instant>,
}

static SHARED: Mutex<Shared> = Mutex::new(Shared {
    commands: Vec::new(),
    poses: [None, None],
    camera: None,
    safe_fall: false,
    saved_on_ground: None,
    cam_ease: None,
    wall_ease: (0.0, None),
    jolts: Vec::new(),
    shake_epoch: None,
    wind: None,
    steer_turn: None,
    steer_seen: None,
    steer_idle: None,
    steer_then: None,
});

fn shared<T>(f: impl FnOnce(&mut Shared) -> T) -> T {
    let mut s = SHARED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    f(&mut s)
}

fn vec(v: [f64; 3]) -> Vec3 {
    Vec3 {
        x: v[0],
        y: v[1],
        z: v[2],
    }
}

fn dvec(v: &Vec3) -> glam::DVec3 {
    glam::DVec3::new(v.x, v.y, v.z)
}

/// The player's own world: the world runs in this game (in-frame), or this
/// game's own server thread is running and the connection is to it. An address
/// alone is not enough: a remote server can be reached by a host named `local`.
fn local(game: &super::Game<'_>) -> bool {
    match game.remote.as_ref() {
        None => true,
        Some(r) => game.integrated.is_some() && r.session.address == "local",
    }
}

fn finite(v: &Vec3) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}

/// The camera if every number in it is finite.
fn sane_camera(c: Camera) -> Option<Camera> {
    let ok = match &c {
        Camera::Chase(c) => {
            [c.distance, c.height, c.side, c.wall_blend].iter().all(|v| v.is_finite())
                && c.wall.as_ref().is_none_or(finite)
        }
        Camera::Fixed(f) => finite(&f.at) && finite(&f.look_at),
    };
    ok.then_some(c)
}

/// The pose if every number in it is finite.
fn sane_pose(p: Pose) -> Option<Pose> {
    let ok = [p.pitch, p.roll, p.twist].iter().all(|v| v.is_finite())
        && p.facing.is_none_or(f32::is_finite)
        && p.shift.as_ref().is_none_or(finite)
        && p.parts.iter().all(|q| {
            [q.pitch, q.yaw, q.roll].iter().all(|v| v.is_finite())
                && q.pivot_y.is_none_or(f32::is_finite)
                && q.pivot_z.is_none_or(f32::is_finite)
        });
    ok.then_some(p)
}

/// Forgets everything the mod asked for (no mod output, or not our world).
fn clear(s: &mut Shared) {
    s.poses = [None, None];
    s.camera = None;
    s.safe_fall = false;
    s.cam_ease = None;
    s.wall_ease = (0.0, None);
    s.jolts.clear();
    s.shake_epoch = None;
    s.steer_turn = None;
    s.steer_seen = None;
}

/// The first collision box along the unit `dir` from `from` within `reach`:
/// how far along it is (voxel walk, then the slab method per box).
fn clip_solid(game: &super::Game<'_>, from: [f64; 3], dir: [f64; 3], reach: f64) -> Option<f64> {
    use horizon_core::geometry::Axis;
    let world = game.mob_world_base(&[]);
    let mut cell = from.map(|v| v.floor() as i32);
    let step = dir.map(|d| if d > 0.0 { 1 } else { -1 });
    let delta = dir.map(|d| if d == 0.0 { f64::INFINITY } else { (1.0 / d).abs() });
    let mut next = [0.0; 3];
    for a in 0..3 {
        next[a] = if dir[a] == 0.0 {
            f64::INFINITY
        } else if dir[a] > 0.0 {
            (f64::from(cell[a]) + 1.0 - from[a]) * delta[a]
        } else {
            (from[a] - f64::from(cell[a])) * delta[a]
        };
    }
    let mut t = 0.0;
    while t <= reach {
        let mut best: Option<f64> = None;
        for b in world.block_collision_boxes(cell[0], cell[1], cell[2]) {
            let (lo, hi) = (
                [b.min(Axis::X), b.min(Axis::Y), b.min(Axis::Z)],
                [b.max(Axis::X), b.max(Axis::Y), b.max(Axis::Z)],
            );
            let (mut t0, mut t1) = (0.0f64, f64::INFINITY);
            let mut miss = false;
            for a in 0..3 {
                if dir[a] == 0.0 {
                    if from[a] < lo[a] || from[a] > hi[a] {
                        miss = true;
                    }
                    continue;
                }
                let (u, v) = ((lo[a] - from[a]) / dir[a], (hi[a] - from[a]) / dir[a]);
                t0 = t0.max(u.min(v));
                t1 = t1.min(u.max(v));
            }
            if !miss && t0 <= t1 && t0 <= reach {
                best = Some(best.map_or(t0, |b: f64| b.min(t0)));
            }
        }
        if best.is_some() {
            return best;
        }
        let a = if next[0] < next[1] && next[0] < next[2] {
            0
        } else if next[1] < next[2] {
            1
        } else {
            2
        };
        t = next[a];
        next[a] += delta[a];
        cell[a] += step[a];
    }
    None
}

fn cast(game: &super::Game<'_>, rays: &[Ray], eye: [f64; 3], feet: [f64; 3]) -> Vec<Option<f64>> {
    rays.iter()
        .take(RAY_BUDGET)
        .map(|r| {
            let base = if r.from_eyes { eye } else { feet };
            if !finite(&r.offset) || [r.offset.x, r.offset.y, r.offset.z].iter().any(|v| v.abs() > 64.0) {
                return None;
            }
            let from = [base[0] + r.offset.x, base[1] + r.offset.y, base[2] + r.offset.z];
            let len = dvec(&r.dir).length();
            if !(len > 1e-9) || !r.distance.is_finite() {
                return None;
            }
            let dir = (dvec(&r.dir) / len).to_array();
            let reach = r.distance.clamp(0.0, 256.0);
            if r.solid {
                clip_solid(game, from, dir, reach)
            } else {
                game.clip_ray(from, dir, reach).map(|h| {
                    glam::DVec3::from_array(h.point).distance(glam::DVec3::from_array(from))
                })
            }
        })
        .collect()
}

/// Every client tick, before the level ticks and the player moves.
pub fn tick(game: &mut super::Game<'_>) {
    let local = local(game);
    let playing = game.screen == super::Screen::Playing;
    let mut input = Input {
        tick: game.ticks,
        keys: if playing {
            game.keys.iter().map(|k| format!("{k:?}")).collect()
        } else {
            Vec::new()
        },
        commands: shared(|s| std::mem::take(&mut s.commands)),
        ..Input::default()
    };
    if local
        && let (Some(p), Some(eye)) = (game.player.as_ref(), game.eye_position())
    {
        let feet = [p.position.x, p.position.y, p.position.z];
        let hits = cast(game, &horizon_dev_reload::rays(), eye, feet);
        // The view follows the wall camera's turn: a ray along it stays
        // under the crosshair.
        let look = wall_turn(game) * glam::DVec3::from_array(super::view_vector(game.yaw, game.pitch));
        input.state = Some(State {
            velocity: vec([p.velocity.x, p.velocity.y, p.velocity.z]),
            on_ground: p.ground.on_ground,
            eye: vec(eye),
            look: vec(look.to_array()),
            hits,
        });
        input.player = Some(Player {
            x: feet[0],
            y: feet[1],
            z: feet[2],
            yaw: game.yaw,
            pitch: game.pitch,
            ..Player::default()
        });
    }
    let Some(out) = horizon_dev_reload::tick(&input) else {
        shared(clear);
        wind(game, 0.0);
        return;
    };
    for line in &out.chat {
        game.system_message(horizon_client::text::Text::literal(line));
    }
    let current = horizon_dev_reload::current();
    shared(|s| {
        s.safe_fall = local && out.reset_fall;
        if !local {
            clear(s);
            return;
        }
        s.poses = [s.poses[1].take(), current.pose.clone().and_then(sane_pose)];
        s.camera = current.camera.and_then(sane_camera);
        if let Some((strength, seconds)) = out.shake.filter(|(a, b)| a.is_finite() && b.is_finite()) {
            s.shake_epoch.get_or_insert_with(Instant::now);
            s.jolts.push((Instant::now(), strength.clamp(0.0, 2.0), seconds.clamp(0.0, 5.0)));
        }
    });
    // Wind lasts while the plug keeps asking for it, tick by tick.
    let wind_now = match out.wind {
        horizon_dev_reload::abi::Set::To(v) if local => v,
        _ => 0.0,
    };
    wind(game, wind_now);
    if !local {
        return;
    }
    if let Some(p) = game.player.as_mut() {
        if let Some(v) = out.velocity {
            // Clipped like `motion`'s: at most 5 blocks a tick on each axis.
            let c = |x: f64| if x.is_finite() { x.clamp(-5.0, 5.0) } else { 0.0 };
            p.velocity = horizon_core::geometry::Vec3::new(c(v.x), c(v.y), c(v.z));
        }
        if out.reset_fall {
            p.fall_distance = 0.0;
        }
    }
}

/// Wind while the plug asks for it: the elytra's flying loop, heard without
/// distance, its volume (0..0.5) and pitch set every tick; silent at 0. The
/// engine sends a new volume to the sound card only for ticking sounds, so the
/// loop is a biome-style one standing still (`direction` 0): its volume is
/// `fade / 40`, and the hook sets `fade`.
fn wind(game: &mut super::Game<'_>, volume: f32) {
    use horizon_client::sound_engine::{
        Attenuation, NEUTRAL, SoundEngine, SoundInstance, Sounds, Ticking,
    };
    let volume = if volume.is_finite() { volume.clamp(0.0, 0.5) } else { 0.0 };
    let playing = shared(|s| s.wind).filter(|id| game.audio.engine.instance(*id).is_some());
    let id = match playing {
        Some(id) => id,
        None if volume > 0.0 => {
            let mut s = SoundInstance::new(
                "minecraft:item.elytra.flying",
                NEUTRAL,
                0.0,
                1.0,
                SoundEngine::unseeded(),
                true,
                0,
                Attenuation::None,
                [0.0; 3],
                true,
            );
            s.can_start_silent = true;
            s.ticking = Ticking::BiomeLoop { fade: 0, direction: 0 };
            let (id, _) = game.audio.play_instance(s, false);
            id
        }
        None => return,
    };
    // Silent: the loop ends on its next tick and is started again when needed.
    shared(|s| s.wind = (volume > 0.0).then_some(id));
    if let Some(s) = game.audio.engine.instance_mut(id) {
        s.ticking = Ticking::BiomeLoop {
            fade: if volume > 0.0 { (volume * 40.0).round() as i32 } else { -1 },
            direction: 0,
        };
        s.pitch = 0.9 + volume * 0.6;
    }
}

/// Before the player's state goes to the server: while the plug forgets the
/// fall, the own world's server hears the player as on the ground, so no
/// fall builds up there. Never on another server.
pub fn before_send_state(game: &mut super::Game<'_>) {
    let safe = shared(|s| s.safe_fall) && local(game);
    let Some(p) = game.player.as_mut().filter(|_| safe) else {
        return;
    };
    shared(|s| s.saved_on_ground = Some(p.ground.on_ground));
    p.ground.on_ground = true;
}

/// After the state packet: the player's own on-ground value again.
pub fn after_send_state(game: &mut super::Game<'_>) {
    if let Some(on_ground) = shared(|s| s.saved_on_ground.take())
        && let Some(p) = game.player.as_mut()
    {
        p.ground.on_ground = on_ground;
    }
}

/// The mod's chat commands (`/swingmode …`) go to the mod, not the server,
/// in the player's own world only. `typed`: from the chat box (kept in its
/// history); else a line sent another way (the control socket, a clicked
/// command), which typed chat also passes through on a server.
pub fn chat_input(game: &mut super::Game<'_>, message: &str, typed: bool) -> ControlFlow<()> {
    let Some(command) = message.trim().strip_prefix('/') else {
        return ControlFlow::Continue(());
    };
    let (name, args) = command.split_once(' ').unwrap_or((command, ""));
    if !COMMANDS.contains(&name) || horizon_dev_reload::loads() == 0 || !local(game) {
        return ControlFlow::Continue(());
    }
    if typed {
        game.ui.chat.add_recent(message);
    }
    shared(|s| s.commands.push((name.to_owned(), args.trim().to_owned())));
    ControlFlow::Break(())
}

/// Ropes: a thin strip from the hand (or where the rope says), both faces,
/// full bright; a flying web zigzags.
pub fn rope_geometry(
    game: &super::Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) {
    let ropes = horizon_dev_reload::current().ropes;
    let Some(p) = game.player.as_ref().filter(|_| !ropes.is_empty() && local(game)) else {
        return;
    };
    let white = game.gui.atlas.font.white();
    let uv = [
        [white.u0, white.v1],
        [white.u1, white.v1],
        [white.u1, white.v0],
        [white.u0, white.v0],
    ];
    // The ropes ride the drawn player: its position between the last two
    // ticks, not the tick's own.
    let lag = glam::DVec3::new(
        game.previous_position.x - p.position.x,
        game.previous_position.y - p.position.y,
        game.previous_position.z - p.position.z,
    ) * (1.0 - alpha);
    let feet = glam::DVec3::new(p.position.x, p.position.y, p.position.z) + lag;
    let hand = feet + glam::DVec3::new(0.0, 1.3, 0.0);
    let light = (15u32 * 16) | ((15u32 * 16) << 8);
    for rope in ropes
        .iter()
        .filter(|r| finite(&r.to) && r.from.as_ref().is_none_or(finite) && r.progress.is_finite() && r.wave.is_finite())
        .take(4)
    {
        let start = rope.from.as_ref().map_or(hand, |f| feet + dvec(f));
        let d = (dvec(&rope.to) - start) * f64::from(rope.progress.clamp(0.0, 1.0));
        let side = d.cross(start - camera).normalize_or_zero();
        let across = side * 0.025;
        let color = horizon_client::mobs::render::abgr(rope.color);
        let steps = if rope.wave > 0.0 { 16 } else { 1 };
        let wave = f64::from(rope.wave.clamp(0.0, 2.0));
        let point = |i: i32| {
            let t = f64::from(i) / f64::from(steps);
            let bend = if i % 2 == 0 { 1.0 } else { -1.0 };
            let swing = if i == 0 || i == steps {
                0.0
            } else {
                bend * wave * (t * std::f64::consts::PI).sin()
            };
            start + d * t + side * swing
        };
        for i in 0..steps {
            let (a, b) = (point(i), point(i + 1));
            let quad = [a - across, a + across, b + across, b - across];
            for quad in [quad, [quad[3], quad[2], quad[1], quad[0]]] {
                for (v, uv) in quad.iter().zip(uv) {
                    out.push(horizon_client::mesher::Vertex {
                        normal: [0; 4],
                        pos: (*v - camera).as_vec3().to_array(),
                        uv,
                        layer: 0,
                        color,
                        light,
                        glint_uv: [0.0, 0.0],
                    });
                }
            }
        }
    }
}

/// The plug's pose this frame: the last two ticks' blended by `alpha`.
fn frame_pose(alpha: f32) -> Option<Pose> {
    let [prev, cur] = shared(|s| s.poses.clone());
    let mut pose = cur?;
    let Some(prev) = prev else {
        return Some(pose);
    };
    let lerp = |a: f32, b: f32| {
        // A jump of more than half a turn (a flip ending) is not blended.
        if (b - a).abs() > std::f32::consts::PI {
            b
        } else {
            a + (b - a) * alpha
        }
    };
    pose.pitch = lerp(prev.pitch, pose.pitch);
    pose.roll = lerp(prev.roll, pose.roll);
    pose.twist = lerp(prev.twist, pose.twist);
    for part in &mut pose.parts {
        if let Some(p) = prev.parts.iter().find(|p| p.part == part.part) {
            part.pitch = lerp(p.pitch, part.pitch);
            part.yaw = lerp(p.yaw, part.yaw);
            part.roll = lerp(p.roll, part.roll);
            if let (Some(a), Some(b)) = (p.pivot_y, part.pivot_y) {
                part.pivot_y = Some(lerp(a, b));
            }
            if let (Some(a), Some(b)) = (p.pivot_z, part.pivot_z) {
                part.pivot_z = Some(lerp(a, b));
            }
        }
    }
    Some(pose)
}

/// The player's own model in third person, posed by the plug: drawn here
/// (body and the parts the pose names over the vanilla animation) instead of
/// the game's draw. Without a pose, or while sleeping, swimming, gliding or
/// dying, the game draws it as always. While posed, armour, the held item
/// and elytra are not drawn (their draws are the game's private helpers).
pub fn player_model(
    game: &super::Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) -> ControlFlow<()> {
    use horizon_client::mobs::{anim, render};
    // Where the game itself would not draw the model (first person, spectator,
    // invisible), or not our world: the game decides as always.
    // (The player's effects are kept under bare ids; both spellings checked.)
    if game.camera_type.first_person()
        || game.spectator()
        || ["invisibility", "minecraft:invisibility"]
            .iter()
            .any(|id| game.potions.effects.get(*id).is_some())
        || game.player.as_ref().is_none_or(|p| p.health.dead_or_dying())
        || !local(game)
    {
        return ControlFlow::Continue(());
    }
    let (Some(pose), Some(m)) = (frame_pose(alpha as f32), game.player_model.as_ref()) else {
        return ControlFlow::Continue(());
    };
    if m.extra.fall_flying || m.death_time > 0 || game.sleeping.is_some() || m.extra.swim.amount(alpha as f32) > 0.0 {
        return ControlFlow::Continue(());
    }
    let a = alpha as f32;
    // A posed body faces where the plug says (a wall it runs on), else
    // where the player looks.
    let facing = pose.facing.unwrap_or(game.yaw);
    let mut m = m.clone();
    m.body_yaw = facing;
    m.previous_body_yaw = facing;
    let at = glam::DVec3::new(
        m.previous.x + (m.position.x - m.previous.x) * alpha,
        m.previous.y + (m.position.y - m.previous.y) * alpha,
        m.previous.z + (m.position.z - m.previous.z) * alpha,
    ) - camera;
    // The game turns the body by its yaw, then flips and scales the model:
    // the shift goes before that turn, pitch, roll and twist (about the
    // body's middle) right after it, so they are undone and redone around it.
    let turn = glam::Mat4::from_rotation_y((180.0 - facing).to_radians());
    let middle = glam::Vec3::new(0.0, 0.9, 0.0);
    let shift = pose.shift.as_ref().map_or(glam::Vec3::ZERO, |s| {
        glam::Vec3::new(s.x as f32, s.y as f32, s.z as f32).clamp(glam::Vec3::splat(-2.0), glam::Vec3::splat(2.0))
    });
    let origin = glam::Mat4::from_translation(at.as_vec3() + shift)
        * turn
        * glam::Mat4::from_translation(middle)
        * glam::Mat4::from_rotation_x(pose.pitch)
        * glam::Mat4::from_rotation_z(pose.roll)
        * glam::Mat4::from_rotation_y(pose.twist)
        * glam::Mat4::from_translation(-middle)
        * turn.inverse();
    let base = render::base_transform_at(&m, a, origin);
    let light = game.world.light(
        m.position.x.floor() as i32,
        (m.position.y + 1.62).floor() as i32,
        m.position.z.floor() as i32,
    );
    let packed = (u32::from(light >> 4) * 16) | ((u32::from(light & 15) * 16) << 8);
    let (model_key, texture) = render::body(&m);
    let model = m
        .kind
        .layer(&model_key)
        .cloned()
        .unwrap_or_else(|| m.kind.model.clone());
    let Some(uv) = game
        .gui
        .atlas
        .sprites
        .get(texture.as_str())
        .map(|r| [r.u0, r.v0, r.u1, r.v1])
    else {
        return ControlFlow::Continue(());
    };
    let mut poses = anim::Poses::new(&model);
    let state = anim::state(&m, a);
    anim::setup(&m, &state, &mut poses, &game.res.mob_data().animations);
    for part in &pose.parts {
        if !poses.has(&part.part) {
            continue;
        }
        let p = poses.get(&part.part);
        p[3] = part.pitch;
        p[4] = part.yaw;
        p[5] = part.roll;
        if let Some(y) = part.pivot_y {
            p[1] = y;
        }
        if let Some(z) = part.pivot_z {
            p[2] = z;
        }
    }
    render::draw(&model, base, &poses, uv, render::body_color(&m, a), packed, 0, out);
    ControlFlow::Break(())
}

/// The wall camera's turn of the whole view (identity off the wall); the
/// same turn applies to the aim, so the crosshair stays true.
fn wall_turn(game: &super::Game<'_>) -> glam::DQuat {
    // Without the whole-view turn (a shipped mod can't set it), the view is
    // not turned either: half a turn would unhinge the mouse and the aim.
    if !horizon_dev_reload::frame::WALL_TURN {
        return glam::DQuat::IDENTITY;
    }
    let camera = shared(|s| s.camera);
    match camera {
        Some(Camera::Chase(c)) if !game.camera_type.first_person() => {
            let now = Instant::now();
            shared(|s| {
                let (cur, then) = s.wall_ease;
                let dt = then.map_or(0.0, |t| now.duration_since(t).as_secs_f64().min(0.1));
                let want = f64::from(c.wall_blend.clamp(0.0, 1.0));
                let k = cur + (want - cur) * (1.0 - (-8.0_f64 * dt).exp());
                let k = if k.is_finite() { k } else { 0.0 };
                s.wall_ease = (k, Some(now));
                match &c.wall {
                    Some(w) if k > 1e-3 => {
                        let w = dvec(w).normalize_or_zero();
                        glam::DQuat::IDENTITY
                            .slerp(glam::DQuat::from_rotation_arc(glam::DVec3::Y, -w), k)
                    }
                    _ => glam::DQuat::IDENTITY,
                }
            })
        }
        _ => glam::DQuat::IDENTITY,
    }
}

/// A view direction as the game's yaw and pitch, degrees (pitch kept off the
/// poles, where yaw is lost).
fn view_angles(v: glam::DVec3) -> (f32, f32) {
    let v = v.normalize_or_zero();
    let yaw = (-v.x).atan2(v.z).to_degrees() as f32;
    let pitch = (-v.y.clamp(-1.0, 1.0)).asin().to_degrees().clamp(-89.0, 89.0) as f32;
    (yaw, pitch)
}

/// The mouse's view under the wall turn. While the turn comes in, the view
/// keeps looking where it looked (no jump onto the wall); while it goes out
/// the view turns with it, so up the wall becomes ahead over the top (the
/// roof, not the sky).
/// On a turned wall, with the mouse left alone, the view drifts behind the
/// run, looking a little down onto the wall (Web of Shadows); any mouse move
/// takes it back at once.
fn steer(game: &mut super::Game<'_>, turn: glam::DQuat, wall: Option<glam::DVec3>) {
    let now = Instant::now();
    let (prev, seen, idle, then) = shared(|s| (s.steer_turn, s.steer_seen, s.steer_idle, s.steer_then));
    let dt = then.map_or(0.0, |t| now.duration_since(t).as_secs_f64().min(0.1));
    // The player's own turn since the last frame (ours was written back).
    let moved = seen.is_none_or(|(y, p)| (game.yaw - y).abs() > 0.01 || (game.pitch - p).abs() > 0.01);
    let idle = if moved { now } else { idle.unwrap_or(now) };
    let id = glam::DQuat::IDENTITY;
    if let Some(prev) = prev.filter(|p| p.angle_between(turn) > 1e-6 && turn.angle_between(id) > p.angle_between(id)) {
        let world = prev * glam::DVec3::from_array(super::view_vector(game.yaw, game.pitch));
        let (yaw, pitch) = view_angles(turn.inverse() * world);
        // By the short way: the game's yaw runs past ±180.
        game.yaw += (yaw - game.yaw + 180.0).rem_euclid(360.0) - 180.0;
        game.pitch = pitch;
    }
    let turned = turn.angle_between(glam::DQuat::IDENTITY) > 1e-4;
    if let (true, Some(w), Some(p)) = (turned, wall, game.player.as_ref()) {
        let step = glam::DVec3::new(
            p.position.x - game.previous_position.x,
            p.position.y - game.previous_position.y,
            p.position.z - game.previous_position.z,
        );
        let along = step - w * step.dot(w);
        if now.duration_since(idle).as_secs_f64() > STEER_IDLE && along.length() > STEER_MOVE {
            let want = along.normalize() * STEER_DOWN.cos() + w * STEER_DOWN.sin();
            let (yaw, pitch) = view_angles(turn.inverse() * want);
            let k = (1.0 - (-STEER_RATE * dt).exp()) as f32;
            let d_yaw = (yaw - game.yaw + 180.0).rem_euclid(360.0) - 180.0;
            game.yaw += d_yaw * k;
            game.pitch += (pitch - game.pitch) * k;
        }
    }
    let view = (game.yaw, game.pitch);
    shared(|s| {
        s.steer_turn = Some(turn);
        s.steer_seen = Some(view);
        s.steer_idle = Some(idle);
        s.steer_then = Some(now);
    });
}

/// The shake's yaw and pitch offsets now, in degrees.
fn shake_offset() -> (f32, f32) {
    let now = Instant::now();
    shared(|s| {
        s.jolts
            .retain(|j| now.duration_since(j.0).as_secs_f32() < j.2);
        if s.jolts.is_empty() {
            s.shake_epoch = None;
            return (0.0, 0.0);
        }
        let mut strength = 0.0f32;
        for j in &s.jolts {
            let left = 1.0 - now.duration_since(j.0).as_secs_f32() / j.2.max(1e-3);
            strength += j.1 * left * left;
        }
        let strength = strength.min(1.0);
        let t = s.shake_epoch.map_or(0.0, |e| now.duration_since(e).as_secs_f32());
        // Smooth noise: sines at unrelated rates, so it never repeats visibly.
        let wave = |a: f32, b: f32, c: f32| {
            ((t * a).sin() * 0.5 + (t * b).sin() * 0.3 + (t * c).sin() * 0.2).clamp(-1.0, 1.0)
        };
        (
            wave(23.0, 37.0, 53.0) * SHAKE_DEGREES.0 * strength,
            wave(29.0, 41.0, 61.0) * SHAKE_DEGREES.1 * strength,
        )
    })
}

/// The plug's camera: the position, yaw and pitch to render from; Continue
/// for the game's own.
pub fn camera_view(
    game: &mut super::Game<'_>,
    partial: f32,
) -> ControlFlow<Option<(glam::DVec3, f32, f32)>> {
    horizon_dev_reload::frame::set_view_turn(None);
    let look_from = |from: glam::DVec3, to: glam::DVec3| {
        let d = (to - from).normalize_or_zero();
        let yaw = (-d.x).atan2(d.z).to_degrees() as f32;
        let pitch = (-d.y.clamp(-1.0, 1.0)).asin().to_degrees() as f32;
        (from, yaw, pitch)
    };
    let Some(camera) = shared(|s| s.camera).filter(|_| local(game)) else {
        return ControlFlow::Continue(());
    };
    let Some(p) = game.player.as_ref() else {
        return ControlFlow::Continue(());
    };
    let (shake_yaw, shake_pitch) = shake_offset();
    match camera {
        Camera::Fixed(f) => {
            // Near the player: a view of the body, not a free camera.
            let body = glam::DVec3::new(p.position.x, p.position.y, p.position.z);
            let near = |v: &Vec3| body + (dvec(v) - body).clamp_length_max(FIXED_CAMERA_REACH);
            ControlFlow::Break(Some(look_from(near(&f.at), near(&f.look_at))))
        }
        Camera::Chase(c) => {
            if game.camera_type.first_person() {
                return ControlFlow::Continue(());
            }
            let a = f64::from(partial);
            let feet = glam::DVec3::new(
                game.previous_position.x + (p.position.x - game.previous_position.x) * a,
                game.previous_position.y + (p.position.y - game.previous_position.y) * a,
                game.previous_position.z + (p.position.z - game.previous_position.z) * a,
            );
            let eye = feet + glam::DVec3::new(0.0, f64::from(p.dimensions.eye_height), 0.0);
            // The mouse turns the camera as always; on a tall wall the whole
            // view is turned so the wall is the floor, and an idle mouse lets
            // it drift behind the run.
            let turn = wall_turn(game);
            steer(game, turn, c.wall.as_ref().map(|w| dvec(w).normalize_or_zero()));
            let Some(p) = game.player.as_ref() else {
                return ControlFlow::Continue(());
            };
            let turned = turn.angle_between(glam::DQuat::IDENTITY) > 1e-4;
            let up = turn * glam::DVec3::Y;
            let look = turn
                * glam::DVec3::from_array(super::view_vector(
                    game.yaw + shake_yaw,
                    game.pitch + shake_pitch,
                ));
            let right = look.cross(up).normalize_or_zero();
            let pivot = eye + up * f64::from(c.height.clamp(-4.0, 4.0));
            let distance = f64::from(c.distance.clamp(0.0, 16.0));
            let reach = |side: f64| {
                let want = pivot - look * distance + right * side;
                let dir = want - pivot;
                let len = dir.length();
                match game.clip_ray(pivot.to_array(), (dir / len.max(1e-6)).to_array(), len) {
                    // Stop short of the wall the ray met.
                    Some(h) => {
                        let at = glam::DVec3::from_array(h.point);
                        let room = (at - pivot).length();
                        (
                            pivot + (at - pivot) * ((room - 0.4).max(0.0) / room.max(1e-6)),
                            room / len.max(1e-6),
                        )
                    }
                    None => (want, 1.0),
                }
            };
            // A wall close behind: try the sides and take the roomier.
            let side = f64::from(c.side.clamp(-4.0, 4.0));
            let (mut at, room) = reach(side);
            let mut want_side = side;
            if room < 0.6 {
                for s in [side + 3.0, side - 3.0] {
                    let (alt, r) = reach(s);
                    if r > room + 0.2 {
                        at = alt;
                        want_side = s;
                        break;
                    }
                }
            }
            // Ease distance and side so the camera never jumps: pulled in
            // fast (no clipping), let out and slid slowly.
            let want_back = (pivot - at).dot(look).max(0.5);
            let now = Instant::now();
            let (back, side) = shared(|s| {
                let eased = match s.cam_ease {
                    Some((back, side, then)) => {
                        let dt = now.duration_since(then).as_secs_f64().min(0.1);
                        let ease =
                            |cur: f64, want: f64, rate: f64| cur + (want - cur) * (1.0 - (-rate * dt).exp());
                        let rate = if want_back < back { 30.0 } else { 3.0 };
                        (ease(back, want_back, rate), ease(side, want_side, 4.0))
                    }
                    None => (want_back, want_side),
                };
                let eased = if eased.0.is_finite() && eased.1.is_finite() {
                    eased
                } else {
                    (want_back, want_side)
                };
                s.cam_ease = Some((eased.0, eased.1, now));
                eased
            });
            let at = pivot - look * back + right * side;
            let view = look_from(at, at + look);
            if turned {
                // The whole orientation goes to the renderer: angles break
                // down when the view runs up a wall.
                let right = look.cross(up).normalize_or_zero();
                let up = right.cross(look);
                let basis = glam::Mat3::from_cols(right.as_vec3(), up.as_vec3(), (-look).as_vec3());
                horizon_dev_reload::frame::set_view_turn(Some(glam::Quat::from_mat3(&basis).to_array()));
            }
            ControlFlow::Break(Some(view))
        }
    }
}
