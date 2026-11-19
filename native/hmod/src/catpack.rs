//! The cat backpack, visual prototype: a backpack on the player's back with a vanilla kitten
//! peeking out of it. Drawn by an entity layer on both player models through the mod API (no
//! mixins): the bag is the mod's own model and texture, the kitten is the game's own
//! `minecraft:cat_baby#main` model with a vanilla cat texture, posed here in code.
//!
//! Prototype limits: `/catpack` turns it on for every player in view (single player only means
//! the local one); no item, no capture, nothing saved. While `SPIDEY_CATPACK_TUNE` names a file,
//! the pose is read from it twice a second (`key = numbers`, see `Tune::read`), so it can be tuned
//! in a running game without a rebuild.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Instant;

use mod_api::{
    BlockEntityDrawContext, CubeListBuilder, EntityRenderLayer, EntityRenderState, HumanoidPart,
    LayerDefinition, LayerTarget, MeshDefinition, ModelPart, PartPose, PlayerModel, PosedParts,
};

/// The bag's texture: `assets/spider_man/textures/entity/catpack.png`.
const BAG_SPRITE: &str = "spider_man:entity/catpack";
/// The kitten's model layer, as `entity-models.json` names it.
const CAT_LAYER: &str = "minecraft:cat_baby#main";

static ON: AtomicBool = AtomicBool::new(false);
static VARIANT: Mutex<String> = Mutex::new(String::new());
/// The kitten as baked once at client start; posed again each frame.
static CAT: Mutex<Option<ModelPart>> = Mutex::new(None);
static BAG: OnceLock<ModelPart> = OnceLock::new();

/// The cat variants: the texture `entity/cat/cat_<name>_baby`.
pub const VARIANTS: [&str; 11] = [
    "tabby", "black", "red", "siamese", "british_shorthair", "calico", "persian", "ragdoll",
    "white", "jellie", "all_black",
];

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `/catpack`: on/off; `/catpack <variant>`: on with that cat. Returns the line for chat.
pub fn command(args: &str) -> String {
    let args = args.trim();
    if args.is_empty() {
        let on = !ON.fetch_xor(true, Ordering::Relaxed);
        return format!("Cat backpack {}", if on { "on" } else { "off" });
    }
    if VARIANTS.contains(&args) {
        *lock(&VARIANT) = args.to_owned();
        ON.store(true, Ordering::Relaxed);
        return format!("Cat backpack on: {args}");
    }
    format!("Cats: {}", VARIANTS.join(", "))
}

/// Bakes the kitten (the game's layer) and the bag (ours). Called once at client start.
pub fn bake(host: &dyn mod_api::ClientHost) {
    *lock(&CAT) = host.bake_layer(CAT_LAYER);
    let _ = BAG.set(bag());
}

/// The bag in the body's frame, in texels: +y down from the shoulders, the back towards +z (the
/// body is 8 wide, 12 tall, 4 deep about its pivot). An open top: the body box ends at the rim's
/// foot, four walls make the rim, and a dark plate is the inside seen from above.
fn bag() -> ModelPart {
    let mut mesh = MeshDefinition::new();
    let root = mesh.root();
    let mut add = |name: &str, u: i32, v: i32, origin: [f32; 3], size: [f32; 3]| {
        let cubes = CubeListBuilder::create().tex_offs(u, v).add_box(origin, size);
        mesh.add_or_replace_child(root, name, cubes, PartPose::ZERO);
    };
    add("body", 0, 0, [-3.5, 3.0, 2.3], [7.0, 7.5, 5.0]);
    add("inside", 64, 0, [-3.0, 2.95, 2.8], [6.0, 0.05, 4.0]);
    add("rim_back", 40, 0, [-3.5, 1.5, 2.3], [7.0, 1.5, 0.5]);
    add("rim_front", 40, 0, [-3.5, 1.5, 6.8], [7.0, 1.5, 0.5]);
    add("rim_left", 40, 0, [-3.5, 1.5, 2.8], [0.5, 1.5, 4.0]);
    add("rim_right", 40, 0, [3.0, 1.5, 2.8], [0.5, 1.5, 4.0]);
    add("pocket", 0, 20, [-2.5, 6.0, 7.3], [5.0, 3.5, 1.0]);
    add("buckle", 96, 0, [-0.6, 6.6, 8.3], [1.2, 1.2, 0.3]);
    add("strap_top_r", 40, 20, [-2.5, -0.5, -2.5], [1.2, 0.4, 4.8]);
    add("strap_top_l", 40, 20, [1.3, -0.5, -2.5], [1.2, 0.4, 4.8]);
    add("strap_front_r", 40, 20, [-2.5, -0.1, -2.55], [1.2, 7.0, 0.3]);
    add("strap_front_l", 40, 20, [1.3, -0.1, -2.55], [1.2, 7.0, 0.3]);
    LayerDefinition::create(mesh, 128, 64).bake_root()
}

/// The kitten's pose, in the bag's frame (texels, radians unless named `_deg`).
#[derive(Clone)]
struct Tune {
    cat_pos: [f32; 3],
    cat_rot_deg: [f32; 3],
    cat_scale: f32,
    head_rot_deg: [f32; 3],
    head_scale: f32,
    leg_pos: [f32; 3],
    leg_rot_deg: [f32; 3],
    leg_len: f32,
    show_body: bool,
    show_hind: bool,
    show_tail: bool,
    idle_amp_deg: f32,
    idle_speed: f32,
}

impl Default for Tune {
    fn default() -> Self {
        Self {
            // Turned to face away from the player's back; head centre about 6 texels behind the
            // body's pivot, level with the shoulders.
            cat_pos: [0.0, -18.5, 1.5],
            cat_rot_deg: [0.0, 180.0, 0.0],
            cat_scale: 1.0,
            head_rot_deg: [0.0, 0.0, 12.0],
            head_scale: 1.0,
            // The right front paw's shoulder in the kitten's own frame; the left one mirrors x.
            leg_pos: [-1.0, 19.0, -4.5],
            leg_rot_deg: [-60.0, 20.0, 0.0],
            leg_len: 2.0,
            show_body: false,
            show_hind: false,
            show_tail: false,
            idle_amp_deg: 4.0,
            idle_speed: 0.08,
        }
    }
}

impl Tune {
    /// Lines `key = a b c` (`#` starts a comment); unknown keys and bad numbers are skipped.
    fn read(text: &str) -> Self {
        let mut t = Self::default();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((key, value)) = line.split_once('=') else { continue };
            let n: Vec<f32> = value.split_whitespace().filter_map(|v| v.parse().ok()).collect();
            let v3 = |dst: &mut [f32; 3]| {
                if let [a, b, c] = n[..] {
                    *dst = [a, b, c];
                }
            };
            match (key.trim(), n.first().copied()) {
                ("cat_pos", _) => v3(&mut t.cat_pos),
                ("cat_rot", _) => v3(&mut t.cat_rot_deg),
                ("head_rot", _) => v3(&mut t.head_rot_deg),
                ("leg_pos", _) => v3(&mut t.leg_pos),
                ("leg_rot", _) => v3(&mut t.leg_rot_deg),
                ("cat_scale", Some(x)) => t.cat_scale = x,
                ("head_scale", Some(x)) => t.head_scale = x,
                ("leg_len", Some(x)) => t.leg_len = x,
                ("show_body", Some(x)) => t.show_body = x != 0.0,
                ("show_hind", Some(x)) => t.show_hind = x != 0.0,
                ("show_tail", Some(x)) => t.show_tail = x != 0.0,
                ("idle_amp", Some(x)) => t.idle_amp_deg = x,
                ("idle_speed", Some(x)) => t.idle_speed = x,
                _ => {}
            }
        }
        t
    }

    /// The tuning file's pose, read at most twice a second; the defaults without the file.
    fn current() -> Self {
        static CACHE: Mutex<Option<(Instant, Tune)>> = Mutex::new(None);
        let Some(path) = std::env::var_os("SPIDEY_CATPACK_TUNE") else { return Self::default() };
        let mut cache = lock(&CACHE);
        if let Some((at, t)) = cache.as_ref()
            && at.elapsed().as_millis() < 500
        {
            return t.clone();
        }
        let t = std::fs::read_to_string(path).map(|s| Self::read(&s)).unwrap_or_default();
        *cache = Some((Instant::now(), t.clone()));
        t
    }
}

fn rad(deg: [f32; 3]) -> [f32; 3] {
    deg.map(f32::to_radians)
}

/// Poses the kitten for this frame: `age` in ticks drives the idle sway.
fn pose(cat: &mut ModelPart, t: &Tune, age: f32) {
    cat.reset_all_poses();
    let [x, y, z] = t.cat_pos;
    let [rx, ry, rz] = rad(t.cat_rot_deg);
    cat.load_pose(PartPose::offset_and_rotation(x, y, z, rx, ry, rz).with_scale(t.cat_scale));
    let sway = t.idle_amp_deg.to_radians();
    if let Some(head) = cat.get_child_mut("head") {
        let [hx, hy, hz] = rad(t.head_rot_deg);
        head.set_rotation(
            hx + 0.5 * sway * (age * t.idle_speed * 0.7).sin(),
            hy + 0.6 * sway * (age * t.idle_speed * 0.45).sin(),
            hz + sway * (age * t.idle_speed).sin(),
        );
        head.x_scale = t.head_scale;
        head.y_scale = t.head_scale;
        head.z_scale = t.head_scale;
    }
    for (name, side) in [("right_front_leg", 1.0), ("left_front_leg", -1.0)] {
        if let Some(leg) = cat.get_child_mut(name) {
            let [px, py, pz] = t.leg_pos;
            let [lx, ly, lz] = rad(t.leg_rot_deg);
            leg.set_pos(px * side, py, pz);
            leg.set_rotation(lx, ly * side, lz * side);
            leg.y_scale = t.leg_len;
        }
    }
    for (name, show) in [
        ("body", t.show_body),
        ("right_hind_leg", t.show_hind),
        ("left_hind_leg", t.show_hind),
        ("tail1", t.show_tail),
        ("tail2", t.show_tail),
    ] {
        if let Some(part) = cat.get_child_mut(name) {
            part.visible = show;
        }
    }
}

/// `ModelPart.translateAndRotate` under the layer's pose: to `p`'s place (texels), turned about
/// z, then y, then x, then scaled.
fn apply(draw: &mut dyn BlockEntityDrawContext, p: &PartPose) {
    draw.translate([p.x / 16.0, p.y / 16.0, p.z / 16.0]);
    for (axis, r) in [([0.0, 0.0, 1.0], p.z_rot), ([0.0, 1.0, 0.0], p.y_rot), ([1.0, 0.0, 0.0], p.x_rot)] {
        if r != 0.0 {
            draw.rotate_about(axis, r.to_degrees(), [0.0; 3]);
        }
    }
    if (p.x_scale, p.y_scale, p.z_scale) != (1.0, 1.0, 1.0) {
        draw.scale([p.x_scale, p.y_scale, p.z_scale]);
    }
}

struct Layer;

impl EntityRenderLayer for Layer {
    fn submit(
        &self,
        entity: &EntityRenderState,
        parts: Option<&PosedParts>,
        draw: &mut dyn BlockEntityDrawContext,
    ) -> Result<(), String> {
        if !ON.load(Ordering::Relaxed) {
            return Ok(());
        }
        let (Some(body), Some(bag)) = (parts.and_then(|p| p.get(HumanoidPart::Body)), BAG.get()) else {
            return Ok(());
        };
        let tune = Tune::current();
        let variant = {
            let v = lock(&VARIANT);
            if v.is_empty() { "tabby".to_owned() } else { v.clone() }
        };
        draw.push();
        // The model's frame, as `LivingEntityRenderer` sets it up, then the body part's.
        draw.scale([1.0, -1.0, -1.0]);
        draw.translate([0.0, -1.501, 0.0]);
        apply(draw, &body.pose);
        draw.model_part(bag, BAG_SPRITE, 0xFF_FFFF);
        if let Some(cat) = lock(&CAT).as_mut() {
            pose(cat, &tune, entity.age);
            draw.model_part(cat, &format!("entity/cat/cat_{variant}_baby"), 0xFF_FFFF);
        }
        draw.pop();
        Ok(())
    }
}

/// Adds the layer to both player models (each resource load asks again).
pub fn register_layers(event: &mut mod_api::RegisterEntityLayersEvent<'_>) {
    let layer: Arc<dyn EntityRenderLayer> = Arc::new(Layer);
    for model in [PlayerModel::Wide, PlayerModel::Slim] {
        let _ = event.layers.add(LayerTarget::Player(model), layer.clone());
    }
}
