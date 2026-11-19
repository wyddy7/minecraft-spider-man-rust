//! The cat backpack, prototype: a backpack on the player's back with a cat peeking out of it.
//! Drawn by an entity layer on both player models through the mod API (no mixins): the bag is the
//! mod's own model and texture, the cat is the game's own model (`minecraft:cat_baby#main` for a
//! kitten, `minecraft:cat#main` for a grown cat) with a vanilla cat texture, posed here in code.
//!
//! Worn as an item: any item in the chest slot whose `custom_data` has `catpack` (`kitten` or
//! `cat`) and, optionally, `catpack_cat` (a variant). `/catpack give` gives the two test bags.
//! Only the local player's bag is seen for now (other players' equipment isn't read yet).
//! `/catpack` alone forces the bag onto every player in view, for looking at it.
//!
//! While `SPIDEY_CATPACK_TUNE` names a file, the pose is read from it twice a second
//! (`key = numbers`, see `Tune::read`), so it can be tuned in a running game without a rebuild.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Instant;

use mod_api::{
    BlockEntityDrawContext, CubeListBuilder, EntityRenderLayer, EntityRenderState, HumanoidPart,
    LayerDefinition, LayerTarget, MeshDefinition, ModelPart, PartPose, PlayerModel, PosedParts,
};

/// The bag's texture: `assets/spider_man/textures/entity/catpack.png` (made by
/// `native/tools/catpack_texture.py`).
const BAG_SPRITE: &str = "spider_man:entity/catpack";

/// A kitten or a grown cat: the game's model of it and what that model's numbers are.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Kitten,
    Cat,
}

impl Kind {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "kitten" => Some(Self::Kitten),
            "cat" => Some(Self::Cat),
            _ => None,
        }
    }
    fn layer(self) -> &'static str {
        match self {
            Self::Kitten => "minecraft:cat_baby#main",
            Self::Cat => "minecraft:cat#main",
        }
    }
    fn texture(self, variant: &str) -> String {
        match self {
            Self::Kitten => format!("entity/cat/cat_{variant}_baby"),
            Self::Cat => format!("entity/cat/cat_{variant}"),
        }
    }
    /// The scale the game's own layer gives the whole model (the grown cat is drawn at 0.8).
    fn base_scale(self) -> f32 {
        match self {
            Self::Kitten => 1.0,
            Self::Cat => 0.8,
        }
    }
    /// The head's centre in the model's own frame, in texels.
    fn head_centre(self) -> [f32; 3] {
        match self {
            Self::Kitten => [0.0, 18.5, -4.5],
            Self::Cat => [0.0, 14.5, -10.0],
        }
    }
    /// How long a front leg is, in texels.
    fn leg_length(self) -> f32 {
        match self {
            Self::Kitten => 2.0,
            Self::Cat => 10.0,
        }
    }
}

/// The cat variants: the texture `entity/cat/cat_<name>` (`_baby` for a kitten).
pub const VARIANTS: [&str; 11] = [
    "tabby", "black", "red", "siamese", "british_shorthair", "calico", "persian", "ragdoll",
    "white", "jellie", "all_black",
];

/// What a bag on a back shows.
#[derive(Clone, PartialEq, Debug)]
struct Worn {
    kind: Kind,
    variant: String,
}

/// `/catpack` alone: this bag on every player in view.
static FORCED: AtomicBool = AtomicBool::new(false);
static FORCED_WORN: Mutex<Option<Worn>> = Mutex::new(None);
/// The local player's entity id and the bag in their chest slot, as of the last client tick.
static LOCAL: Mutex<Option<(u32, Worn)>> = Mutex::new(None);
/// The two models as baked once at client start; posed again each frame.
static KITTEN: Mutex<Option<ModelPart>> = Mutex::new(None);
static CAT: Mutex<Option<ModelPart>> = Mutex::new(None);
static BAG: OnceLock<ModelPart> = OnceLock::new();

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The two test bags `/catpack give` hands out: black bundles worn in the chest slot.
pub const GIVE: [&str; 2] = [
    r#"/give @s minecraft:black_bundle[minecraft:equippable={slot:"chest"},minecraft:custom_data={catpack:"kitten",catpack_cat:"red"},minecraft:item_name="Cat backpack: kitten"]"#,
    r#"/give @s minecraft:black_bundle[minecraft:equippable={slot:"chest"},minecraft:custom_data={catpack:"cat",catpack_cat:"red"},minecraft:item_name="Cat backpack: cat"]"#,
];

/// What `/catpack …` asks for.
pub enum Command {
    /// A line to show in chat.
    Say(String),
    /// Chat lines to send to the server (the `/give`s), then a line to show.
    Send(Vec<String>, String),
}

/// `/catpack`: the forced bag on/off; `/catpack kitten|cat [variant]`: force that one;
/// `/catpack give`: the two test bags.
pub fn command(args: &str) -> Command {
    let mut words = args.split_whitespace();
    let Some(first) = words.next() else {
        let on = !FORCED.fetch_xor(true, Ordering::Relaxed);
        return Command::Say(format!("Cat backpack on every player: {}", if on { "on" } else { "off" }));
    };
    if first == "give" {
        let lines = GIVE.iter().map(|s| (*s).to_owned()).collect();
        return Command::Send(lines, "Two test bags: put one in the chest slot".into());
    }
    if let Some(kind) = Kind::parse(first) {
        let variant = words.next().filter(|v| VARIANTS.contains(v)).unwrap_or("red").to_owned();
        let line = format!("Cat backpack on every player: {first}, {variant}");
        *lock(&FORCED_WORN) = Some(Worn { kind, variant });
        FORCED.store(true, Ordering::Relaxed);
        return Command::Say(line);
    }
    Command::Say(format!("/catpack [kitten|cat [variant]] | give. Cats: {}", VARIANTS.join(", ")))
}

/// Bakes both cats (the game's layers) and the bag (ours). Called once at client start.
pub fn bake(host: &dyn mod_api::ClientHost) {
    *lock(&KITTEN) = host.bake_layer(Kind::Kitten.layer());
    *lock(&CAT) = host.bake_layer(Kind::Cat.layer());
    let _ = BAG.set(bag());
}

/// Each client tick: which bag the local player wears, read from their chest slot.
pub fn tick(host: &dyn mod_api::ClientHost) {
    let worn = host.local_player().and_then(|player| {
        let living = mod_api::LivingRef::new(player.entity());
        let stack = living.equipment(host, mod_api::EquipmentSlot::Chest)?;
        Some((player.entity().id(), worn_by(&stack)?))
    });
    *lock(&LOCAL) = worn;
}

/// The bag a stack is, by its `custom_data`: `catpack` names the kind, `catpack_cat` the variant.
fn worn_by(stack: &mod_api::ItemStack) -> Option<Worn> {
    let id = mod_api::ComponentId::new("minecraft:custom_data")?;
    let data = mod_api::ItemResource::of(stack).component(&id)?;
    let text = |key: &str| match data.get(key) {
        Some(mod_api::Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    };
    let kind = Kind::parse(&text("catpack")?)?;
    let variant = text("catpack_cat").filter(|v| VARIANTS.contains(&v.as_str())).unwrap_or_else(|| "red".into());
    Some(Worn { kind, variant })
}

/// The bag in the body's frame, in whole texels (the style's rule: one texture pixel is one model
/// pixel): +y down from the shoulders, the back towards +z (the body is 8 wide, 12 tall, 4 deep
/// about its pivot). An open top: the body box ends at the rim's foot, four 1-texel walls make the
/// rim, and a dark plate is the inside seen from above. The straps are thin planes, as vanilla
/// draws flat parts. UV places match `native/tools/catpack_texture.py`.
fn bag() -> ModelPart {
    let mut mesh = MeshDefinition::new();
    let root = mesh.root();
    let mut add = |name: &str, u: i32, v: i32, origin: [f32; 3], size: [f32; 3]| {
        let cubes = CubeListBuilder::create().tex_offs(u, v).add_box(origin, size);
        mesh.add_or_replace_child(root, name, cubes, PartPose::ZERO);
    };
    add("body", 0, 0, [-3.5, 3.0, 2.3], [7.0, 7.0, 5.0]);
    add("inside", 32, 16, [-2.5, 2.9, 3.3], [5.0, 0.0, 3.0]);
    add("rim_back", 0, 16, [-3.5, 1.0, 2.3], [7.0, 2.0, 1.0]);
    add("rim_front", 0, 16, [-3.5, 1.0, 6.3], [7.0, 2.0, 1.0]);
    add("rim_left", 20, 16, [-3.5, 1.0, 3.3], [1.0, 2.0, 3.0]);
    add("rim_right", 20, 16, [2.5, 1.0, 3.3], [1.0, 2.0, 3.0]);
    add("pocket", 0, 24, [-2.5, 6.0, 7.3], [5.0, 3.0, 1.0]);
    add("buckle", 16, 24, [-0.5, 6.0, 8.3], [1.0, 1.0, 0.25]);
    add("strap_top_r", 32, 24, [-3.0, -0.5, -2.5], [1.0, 0.5, 5.0]);
    add("strap_top_l", 32, 24, [2.0, -0.5, -2.5], [1.0, 0.5, 5.0]);
    add("strap_front_r", 48, 24, [-3.0, 0.0, -2.75], [1.0, 7.0, 0.5]);
    add("strap_front_l", 48, 24, [2.0, 0.0, -2.75], [1.0, 7.0, 0.5]);
    LayerDefinition::create(mesh, 64, 32).bake_root()
}

/// The cat's pose, in the bag's frame (texels; angles in degrees). Places are given where they
/// end up on the bag, so the same numbers fit the kitten and the grown cat at any scale.
#[derive(Clone)]
struct Tune {
    /// The cat's size against the game's own (1 = as the game draws it).
    cat_scale: f32,
    /// Where the head's centre sits.
    head_at: [f32; 3],
    head_rot: [f32; 3],
    head_scale: f32,
    /// Where the right front paw's shoulder sits (the left one mirrors x), its turn, and how far
    /// the paw reaches, in the bag's texels.
    paw_at: [f32; 3],
    paw_rot: [f32; 3],
    paw_reach: f32,
    show_body: bool,
    show_hind: bool,
    show_tail: bool,
    idle_amp: f32,
    idle_speed: f32,
}

impl Default for Tune {
    fn default() -> Self {
        Self {
            cat_scale: 0.8,
            head_at: [0.0, -1.5, 5.5],
            head_rot: [0.0, 0.0, 10.0],
            head_scale: 1.0,
            paw_at: [-0.8, 0.5, 6.8],
            paw_rot: [-35.0, 28.0, 0.0],
            paw_reach: 2.5,
            show_body: false,
            show_hind: false,
            show_tail: false,
            idle_amp: 4.0,
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
                ("head_at", _) => v3(&mut t.head_at),
                ("head_rot", _) => v3(&mut t.head_rot),
                ("paw_at", _) => v3(&mut t.paw_at),
                ("paw_rot", _) => v3(&mut t.paw_rot),
                ("cat_scale", Some(x)) => t.cat_scale = x,
                ("head_scale", Some(x)) => t.head_scale = x,
                ("paw_reach", Some(x)) => t.paw_reach = x,
                ("show_body", Some(x)) => t.show_body = x != 0.0,
                ("show_hind", Some(x)) => t.show_hind = x != 0.0,
                ("show_tail", Some(x)) => t.show_tail = x != 0.0,
                ("idle_amp", Some(x)) => t.idle_amp = x,
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

/// Poses the cat for this frame: turned to face away from the player's back, scaled, placed so
/// its head lands on `head_at`; `age` in ticks drives the idle sway.
fn pose(cat: &mut ModelPart, kind: Kind, t: &Tune, age: f32) {
    cat.reset_all_poses();
    let s = kind.base_scale() * t.cat_scale;
    // Turned half round about y, a point (x, y, z) of the model lands at root + s·(-x, y, -z).
    let [hx, hy, hz] = kind.head_centre();
    let root = [t.head_at[0] + s * hx, t.head_at[1] - s * hy, t.head_at[2] + s * hz];
    cat.load_pose(PartPose::offset_and_rotation(root[0], root[1], root[2], 0.0, std::f32::consts::PI, 0.0).with_scale(s));
    let to_model = |p: [f32; 3]| [-(p[0] - root[0]) / s, (p[1] - root[1]) / s, -(p[2] - root[2]) / s];
    let sway = t.idle_amp.to_radians();
    if let Some(head) = cat.get_child_mut("head") {
        let [rx, ry, rz] = rad(t.head_rot);
        head.set_rotation(
            rx + 0.5 * sway * (age * t.idle_speed * 0.7).sin(),
            ry + 0.6 * sway * (age * t.idle_speed * 0.45).sin(),
            rz + sway * (age * t.idle_speed).sin(),
        );
        head.x_scale = t.head_scale;
        head.y_scale = t.head_scale;
        head.z_scale = t.head_scale;
    }
    for (name, side) in [("right_front_leg", 1.0), ("left_front_leg", -1.0)] {
        if let Some(leg) = cat.get_child_mut(name) {
            let [px, py, pz] = to_model([t.paw_at[0] * side, t.paw_at[1], t.paw_at[2]]);
            let [lx, ly, lz] = rad(t.paw_rot);
            leg.set_pos(px, py, pz);
            leg.set_rotation(lx, ly * side, lz * side);
            leg.y_scale = t.paw_reach / (kind.leg_length() * s);
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

/// The bag `entity` shows this frame: the forced one, else the local player's own.
fn shown(entity: u32) -> Option<Worn> {
    if FORCED.load(Ordering::Relaxed) {
        return Some(lock(&FORCED_WORN).clone().unwrap_or(Worn { kind: Kind::Kitten, variant: "red".into() }));
    }
    lock(&LOCAL).as_ref().filter(|(id, _)| *id == entity).map(|(_, w)| w.clone())
}

struct Layer;

impl EntityRenderLayer for Layer {
    fn submit(
        &self,
        entity: &EntityRenderState,
        parts: Option<&PosedParts>,
        draw: &mut dyn BlockEntityDrawContext,
    ) -> Result<(), String> {
        let Some(worn) = shown(entity.id) else { return Ok(()) };
        let (Some(body), Some(bag)) = (parts.and_then(|p| p.get(HumanoidPart::Body)), BAG.get()) else {
            return Ok(());
        };
        let tune = Tune::current();
        draw.push();
        // The model's frame, as `LivingEntityRenderer` sets it up, then the body part's.
        draw.scale([1.0, -1.0, -1.0]);
        draw.translate([0.0, -1.501, 0.0]);
        apply(draw, &body.pose);
        draw.model_part(bag, BAG_SPRITE, 0xFF_FFFF);
        let model = match worn.kind {
            Kind::Kitten => &KITTEN,
            Kind::Cat => &CAT,
        };
        if let Some(cat) = lock(model).as_mut() {
            pose(cat, worn.kind, &tune, entity.age);
            draw.model_part(cat, &worn.kind.texture(&worn.variant), 0xFF_FFFF);
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
