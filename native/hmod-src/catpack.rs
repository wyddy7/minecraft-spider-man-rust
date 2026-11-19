//! The cat backpack: a backpack on the player's back, its front half leaning open, with a small cat
//! sitting in it, head and crossed paws over the rim, looking away from the player's back.
//! Drawn by an entity layer on both player models through the mod API (no mixins). The bag is the
//! mod's own model and texture. The cat, grown or a kitten, is the mod's own small model laid out
//! on the game's texture of it (`entity/cat/cat_<variant>`, 64×32, or `…_baby`, 32×32): the
//! game's own head, a short body and two-part front legs cut from the same places of that
//! texture, so every cat variant fits without a texture of ours.
//!
//! Worn as an item: any item in the chest slot whose `custom_data` has `catpack` (`kitten` or
//! `cat`) and, optionally, `catpack_cat` (a variant). `/catpack give` gives the two test bags.
//! Only the local player's bag is seen for now (other players' equipment isn't read yet).
//! `/catpack` alone forces the bag onto every player in view, for looking at it.
//!
//! While `SPIDEY_CATPACK_TUNE` names a file, the pose is read from it twice a second
//! (`key = numbers`, see `Tune`), so it can be tuned in a running game without a rebuild.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use mod_api::{
    BlockEntityDrawContext, CubeListBuilder, EntityRenderLayer, EntityRenderState,
    HumanoidPart, LayerDefinition, LayerTarget, MeshDefinition, ModelPart, ModelPolygon, PartPose,
    PlayerModel, PosedParts,
};

/// The bag's texture: `assets/spider_man/textures/entity/catpack.png` (made by
/// `native/tools/catpack_texture.py`).
const BAG_SPRITE: &str = "spider_man:entity/catpack";

/// The cat variants: the texture `entity/cat/cat_<name>`.
pub const VARIANTS: [&str; 11] = [
    "tabby", "black", "red", "siamese", "british_shorthair", "calico", "persian", "ragdoll",
    "white", "jellie", "all_black",
];

/// A kitten or a grown cat.
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
    fn texture(self, variant: &str) -> String {
        match self {
            Self::Kitten => format!("entity/cat/cat_{variant}_baby"),
            Self::Cat => format!("entity/cat/cat_{variant}"),
        }
    }
    fn model(self) -> &'static Mutex<Option<ModelPart>> {
        match self {
            Self::Kitten => &KITTEN,
            Self::Cat => &CAT,
        }
    }
}

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
/// The two cats and the bag as built once at client start; posed again each frame.
static KITTEN: Mutex<Option<ModelPart>> = Mutex::new(None);
static CAT: Mutex<Option<ModelPart>> = Mutex::new(None);
static BAG: Mutex<Option<ModelPart>> = Mutex::new(None);

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

/// Builds both cats (each one's head is the game's own, from `minecraft:cat#main` and
/// `minecraft:cat_baby#main`) and the bag. Called once at client start.
pub fn bake(host: &dyn mod_api::ClientHost) {
    let head = |layer: &str| host.bake_layer(layer).and_then(|cat| cat.get_child("head").cloned());
    *lock(&CAT) = head("minecraft:cat#main").map(cat);
    *lock(&KITTEN) = head("minecraft:cat_baby#main").map(kitten);
    *lock(&BAG) = Some(bag());
}

/// One box's faces, baked on a `texture` of that many texels at the box-UV place (`u`, `v`).
fn cat_box(texture: [u32; 2], u: i32, v: i32, origin: [f32; 3], size: [f32; 3]) -> Vec<ModelPolygon> {
    let mut mesh = MeshDefinition::new();
    let root = mesh.root();
    let cubes = CubeListBuilder::create().tex_offs(u, v).add_box(origin, size);
    mesh.add_or_replace_child(root, "box", cubes, PartPose::ZERO);
    let baked = LayerDefinition::create(mesh, texture[0], texture[1]).bake_root();
    baked.get_child("box").map(|b| b.polygons.clone()).unwrap_or_default()
}

/// Lays the face of `polygons` looking along `normal` on the texture's rectangle from
/// (`rect[0]`, `rect[1]`), the face's top left, to (`rect[2]`, `rect[3]`), in texels, turned by
/// `turns` quarter turns.
fn lay_face(polygons: &mut [ModelPolygon], normal: [f32; 3], rect: [f32; 4], turns: usize, texture: [u32; 2]) {
    let [ua, va, ub, vb] = rect;
    // The corners in the order a baked face holds them: right top, left top, left bottom, right bottom.
    let uv = [(ub, va), (ua, va), (ua, vb), (ub, vb)];
    for polygon in polygons.iter_mut().filter(|p| p.normal == normal) {
        for (k, vertex) in polygon.vertices.iter_mut().enumerate() {
            let (u, v) = uv[(k + turns) % 4];
            vertex.u = u / texture[0] as f32;
            vertex.v = v / texture[1] as f32;
        }
    }
}

/// The game's head with its box's centre moved to the part's origin, so a head is placed by its
/// middle whatever the model.
fn centred(mut head: ModelPart, centre: [f32; 3]) -> ModelPart {
    for polygon in &mut head.polygons {
        for vertex in &mut polygon.vertices {
            for (p, c) in vertex.pos.iter_mut().zip(centre) {
                *p -= c;
            }
        }
    }
    head.set_initial_pose(PartPose::ZERO);
    head
}

/// A cat of these parts, in the game's own model frame (+y down, its front towards -z), every
/// part posed each frame by `pose`. As everywhere in the game, a leg is one straight box turning
/// at one pivot (the shoulder, at the part's origin).
fn assemble(head: ModelPart, body: Vec<ModelPolygon>, legs: Vec<(&str, Vec<ModelPolygon>)>) -> ModelPart {
    let mut parts = vec![
        ("body".to_owned(), ModelPart::new(body, Vec::new(), PartPose::ZERO)),
        ("head".to_owned(), head),
    ];
    for (name, leg) in legs {
        parts.push((name.to_owned(), ModelPart::new(leg, Vec::new(), PartPose::ZERO)));
    }
    // Turned half round about y so the cat looks away from the player's back: a point (x, y, z)
    // of the cat's frame lands at (-x, y, -z) of the bag's.
    ModelPart::new(Vec::new(), parts, PartPose::rotation(0.0, std::f32::consts::PI, 0.0))
}

/// The grown cat, on its 64×32 texture.
///
/// - `head`: the game's grown cat head (5×4×5 with its nose and ears), as the game bakes it.
/// - `body`: an upright trunk, 4×5×3, its foot at the part's origin. It lies on the front end
///   of the texture's body (4×16×6 at 20,0): the chest stripe on its front, the flanks on its
///   sides, the neck's end on top.
/// - `arm_r`, `arm_l`: a paw lying on the flap, 2×3×1: the last rows of the game's front leg
///   (2×10×2 at 40,0), where the light paw is, as wide as the game's and half as thick.
/// - `thick_*` (2×3×2, the game's thickness) and `thin_*` (1×3×1) are shown instead while the
///   pose's `paw` is 1 or 2 (a trial: which of the three stays is not settled).
fn cat(head: ModelPart) -> ModelPart {
    const T: [u32; 2] = [64, 32];
    let body = cat_box(T, 23, 3, [-2.0, -5.0, -1.5], [4.0, 5.0, 3.0]);
    let flat = || {
        let mut leg = cat_box(T, 41, 8, [-1.0, 0.0, -0.5], [2.0, 3.0, 1.0]);
        lay_face(&mut leg, [0.0, 1.0, 0.0], [44.0, 0.0, 46.0, 1.0], 0, T);
        leg
    };
    let thick = || {
        let mut leg = cat_box(T, 40, 7, [-1.0, 0.0, -1.0], [2.0, 3.0, 2.0]);
        lay_face(&mut leg, [0.0, 1.0, 0.0], [44.0, 0.0, 46.0, 2.0], 0, T);
        leg
    };
    let thin = || {
        let mut leg = cat_box(T, 41, 8, [-0.5, 0.0, -0.5], [1.0, 3.0, 1.0]);
        lay_face(&mut leg, [0.0, 1.0, 0.0], [44.0, 0.0, 45.0, 1.0], 0, T);
        leg
    };
    let legs = vec![
        ("arm_r", flat()),
        ("arm_l", flat()),
        ("thick_r", thick()),
        ("thick_l", thick()),
        ("thin_r", thin()),
        ("thin_l", thin()),
    ];
    assemble(centred(head, [0.0, 0.0, -0.5]), body, legs)
}

/// The kitten, on its 32×32 texture.
///
/// - `head`: the game's kitten head (5×4×4 with its nose and ears), as the game bakes it.
/// - `body`: a short upright trunk, 4×4×3. The texture's body lies along the kitten (4×3×7), so
///   each face is cut from its front end: the belly on the chest, the back on the back, the
///   flanks turned upright on the sides, the neck's end on top.
///
/// No paws: a kitten is too small to reach the lip, only its head shows over it.
fn kitten(head: ModelPart) -> ModelPart {
    const T: [u32; 2] = [32, 32];
    let mut body = cat_box(T, 0, 8, [-2.0, -4.0, -1.5], [4.0, 4.0, 3.0]);
    lay_face(&mut body, [0.0, 0.0, -1.0], [11.0, 15.0, 15.0, 11.0], 0, T);
    lay_face(&mut body, [0.0, 0.0, 1.0], [7.0, 15.0, 11.0, 11.0], 0, T);
    lay_face(&mut body, [-1.0, 0.0, 0.0], [3.0, 15.0, 7.0, 18.0], 1, T);
    lay_face(&mut body, [1.0, 0.0, 0.0], [11.0, 15.0, 15.0, 18.0], 3, T);
    lay_face(&mut body, [0.0, -1.0, 0.0], [7.0, 15.0, 11.0, 18.0], 0, T);
    assemble(centred(head, [0.0, -1.0, -0.9]), body, Vec::new())
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

/// Where the lid hinges: the top section's front wall, its top outer edge.
const LID_HINGE: [f32; 3] = [0.0, 1.0, 7.0];

/// The bag in the body's frame, in whole texels (the style's rule: one texture pixel is one model
/// pixel): +y down from the shoulders, the back towards +z (the body is 8 wide, 12 tall, 4 deep
/// about its pivot). After the bag Miles carries his cat in, laid over the game's own pictures
/// of it: narrower at the top than at the foot, deeper than the body.
///
/// - the trunk, as wide as the back, from the chest down to the hips;
/// - on it a narrower top section, unzipped: a back, two cheeks and a front wall, with a well
///   between them where the cat sits on the trunk's top; a grab handle above the back;
/// - the lid, hinged on the front wall: open, it hangs out in front, lining up; shut, it lies
///   over the well;
/// - a big pocket low on the front and a mesh pocket low on each side.
///
/// The straps are thin planes, as vanilla draws flat parts. UV places match
/// `native/tools/catpack_texture.py`.
fn bag() -> ModelPart {
    let mut mesh = MeshDefinition::new();
    let root = mesh.root();
    let one = |u: i32, v: i32, origin: [f32; 3], size: [f32; 3]| CubeListBuilder::create().tex_offs(u, v).add_box(origin, size);
    let shell = CubeListBuilder::create()
        .tex_offs(0, 0)
        .add_box([-4.0, 4.0, 2.0], [8.0, 8.0, 5.0])
        .tex_offs(26, 0)
        .add_box([-3.0, 1.0, 2.0], [6.0, 3.0, 1.0])
        .tex_offs(26, 4)
        .add_box([-3.0, 1.0, 6.0], [6.0, 3.0, 1.0])
        .tex_offs(40, 0)
        .add_box([-3.0, 1.0, 3.0], [1.0, 3.0, 4.0])
        .add_box_mirrored([2.0, 1.0, 3.0], [1.0, 3.0, 4.0], true)
        .tex_offs(0, 13)
        .add_box([-3.0, 7.0, 7.0], [6.0, 5.0, 2.0])
        .tex_offs(50, 0)
        .add_box([-5.0, 7.0, 3.0], [1.0, 4.0, 3.0])
        .add_box_mirrored([4.0, 7.0, 3.0], [1.0, 4.0, 3.0], true)
        .tex_offs(58, 0)
        .add_box([-1.0, 0.0, 2.0], [2.0, 1.0, 1.0]);
    mesh.add_or_replace_child(root, "shell", shell, PartPose::ZERO);
    let hinge = PartPose::offset(LID_HINGE[0], LID_HINGE[1], LID_HINGE[2]);
    mesh.add_or_replace_child(root, "lid", one(26, 8, [-3.0, 0.0, -1.0], [6.0, 4.0, 1.0]), hinge);
    mesh.add_or_replace_child(root, "strap_top_r", one(32, 24, [-3.0, -0.5, -2.5], [1.0, 0.5, 5.0]), PartPose::ZERO);
    mesh.add_or_replace_child(root, "strap_top_l", one(32, 24, [2.0, -0.5, -2.5], [1.0, 0.5, 5.0]), PartPose::ZERO);
    mesh.add_or_replace_child(root, "strap_front_r", one(48, 24, [-3.0, 0.0, -2.75], [1.0, 7.0, 0.5]), PartPose::ZERO);
    mesh.add_or_replace_child(root, "strap_front_l", one(48, 24, [2.0, 0.0, -2.75], [1.0, 7.0, 0.5]), PartPose::ZERO);
    LayerDefinition::create(mesh, 64, 32).bake_root()
}

/// What a pose is made of: `key = a b c` lines of the tuning file replace the shown cat's numbers.
/// Places are in the bag's frame (texels, +y down, +z away from the player's back), turns in
/// degrees (the cat's parts turn in the cat's own frame, which looks along -z).
const KEYS: [&str; 13] = [
    // The cat's size against the game's own model of it.
    "scale",
    // How far the flap hangs out from the front wall.
    "open",
    // The trunk's foot, inside the bag, and its lean out over the front wall.
    "body_at",
    "body_rot",
    // The head's centre, on the trunk, and its turn (the cat looks over the player's shoulder).
    "head_at",
    "head_rot",
    // Each front leg's shoulder and its turn out over the front wall, onto the flap.
    "arm_r_at",
    "arm_r_rot",
    "arm_l_at",
    "arm_l_rot",
    // The head's idle sway: degrees, speed.
    "idle",
    // The grown cat's paws: 0 flat (2×3×1), 1 the game's thickness (2×3×2), 2 thin (1×3×1).
    "paw",
    // For looking at the hiding: 0 to 1 holds the cat that far ducked; below 0, the game decides.
    "hide",
];

/// The grown cat's pose, in `KEYS`' order: sitting on the trunk's top in the well, by its front
/// wall, its head and neck over it, looking up and aside past the player's shoulder, both paws
/// over the wall onto the open lid, one a little further out (as the game's lying cat holds
/// them).
const CAT_POSE: [[f32; 3]; KEYS.len()] = [
    [0.6, 0.0, 0.0],
    [35.0, 0.0, 0.0],
    [0.0, 4.0, 5.0],
    [6.0, 0.0, 0.0],
    [0.0, -0.5, 5.8],
    [-12.0, -30.0, 6.0],
    [-0.8, 0.5, 6.3],
    [-62.0, 0.0, 0.0],
    [0.8, 0.4, 6.2],
    [-56.0, 0.0, 0.0],
    [4.0, 0.08, 0.0],
    [0.0, 0.0, 0.0],
    [-1.0, 0.0, 0.0],
];

/// The kitten's pose: the same seat, only its head over the wall.
const KITTEN_POSE: [[f32; 3]; KEYS.len()] = [
    [0.5, 0.0, 0.0],
    [35.0, 0.0, 0.0],
    [0.0, 3.0, 5.0],
    [6.0, 0.0, 0.0],
    [0.0, 0.2, 5.7],
    [-12.0, -30.0, 6.0],
    [0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
    [4.0, 0.08, 0.0],
    [0.0, 0.0, 0.0],
    [-1.0, 0.0, 0.0],
];

#[derive(Clone)]
struct Tune([[f32; 3]; KEYS.len()]);

impl Tune {
    fn of(kind: Kind) -> Self {
        Self(match kind {
            Kind::Kitten => KITTEN_POSE,
            Kind::Cat => CAT_POSE,
        })
    }

    fn get(&self, key: &str) -> [f32; 3] {
        KEYS.iter().position(|k| *k == key).map_or([0.0; 3], |i| self.0[i])
    }

    /// Lines `key = a b c` (`#` starts a comment) over `kind`'s pose; unknown keys and bad numbers
    /// are skipped, and fewer than three numbers set the first ones.
    fn read(kind: Kind, text: &str) -> Self {
        let mut t = Self::of(kind);
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((key, value)) = line.split_once('=') else { continue };
            let Some(i) = KEYS.iter().position(|k| *k == key.trim()) else { continue };
            for (dst, n) in t.0[i].iter_mut().zip(value.split_whitespace().filter_map(|v| v.parse().ok())) {
                *dst = n;
            }
        }
        t
    }

    /// `kind`'s pose under the tuning file, which is read at most twice a second; its own pose
    /// without the file.
    fn current(kind: Kind) -> Self {
        static CACHE: Mutex<Option<(Instant, String)>> = Mutex::new(None);
        let Some(path) = std::env::var_os("SPIDEY_CATPACK_TUNE") else { return Self::of(kind) };
        let mut cache = lock(&CACHE);
        if cache.as_ref().is_none_or(|(at, _)| at.elapsed().as_millis() >= 500) {
            *cache = Some((Instant::now(), std::fs::read_to_string(path).unwrap_or_default()));
        }
        Self::read(kind, cache.as_ref().map_or("", |(_, text)| text))
    }
}

fn rad(deg: [f32; 3]) -> [f32; 3] {
    deg.map(f32::to_radians)
}

/// Sets the lid: hanging out in front by the pose's angle, or, as `hide` goes to 1, swung back
/// to lie over the well.
fn pose_bag(bag: &mut ModelPart, t: &Tune, hide: f32) {
    if let Some(lid) = bag.get_child_mut("lid") {
        let open = t.get("open")[0];
        // The lid waits for the cat to duck, then swings shut.
        let shut = ((hide - 0.5) / 0.5).clamp(0.0, 1.0);
        lid.x_rot = (open + (-90.0 - open) * shut).to_radians();
    }
}

/// How deep the cat ducks into the well when it hides, in the bag's texels.
const DUCK: f32 = 3.4;
/// How far it draws back from the front wall as it ducks.
const DRAW_BACK: f32 = 1.5;
/// How long the cat stays hidden after its carrier is hurt, and how long it takes to duck and
/// to come back out, in seconds.
const HIDE_FOR: f32 = 4.0;
const DUCK_IN: f32 = 0.35;
const COME_OUT: f32 = 0.6;

/// Each carrier's hiding: how far the cat has ducked (0 out, 1 hidden with the lid shut), when
/// that was last moved on, and until when it stays hidden.
static HIDING: Mutex<Vec<(u32, f32, Instant, Instant)>> = Mutex::new(Vec::new());

/// How far `entity`'s cat has ducked this frame; `hurt` sends it into the bag for `HIDE_FOR`.
fn hiding(entity: u32, hurt: bool) -> f32 {
    let now = Instant::now();
    let mut all = lock(&HIDING);
    // Carriers not drawn for a while are forgotten.
    all.retain(|(_, _, at, _)| now.duration_since(*at).as_secs_f32() < 30.0);
    let i = all.iter().position(|(id, ..)| *id == entity).unwrap_or_else(|| {
        all.push((entity, 0.0, now, now));
        all.len() - 1
    });
    let (_, amount, at, until) = &mut all[i];
    if hurt {
        *until = now + std::time::Duration::from_secs_f32(HIDE_FOR);
    }
    let dt = now.duration_since(*at).as_secs_f32().min(0.25);
    *at = now;
    *amount = if now < *until { (*amount + dt / DUCK_IN).min(1.0) } else { (*amount - dt / COME_OUT).max(0.0) };
    *amount
}

/// Poses the cat for this frame; `age` in ticks drives the head's idle sway, `hide` (0 to 1)
/// ducks it into the well, out of sight once the lid is shut.
fn pose(cat: &mut ModelPart, t: &Tune, age: f32, hide: f32) {
    cat.visible = hide < 0.98;
    // The cat is turned half round about y: a place (x, y, z) of the bag is (-x, y, -z) of the cat.
    // Places are the bag's texels whatever the cat's size.
    let scale = t.get("scale")[0].max(0.1);
    cat.x_scale = scale;
    cat.y_scale = scale;
    cat.z_scale = scale;
    let place = |part: &mut ModelPart, key: &str| {
        let [x, y, z] = t.get(key);
        // Ducking, it also draws back from the front wall, so nothing of it shows through.
        let duck = (hide / 0.6).min(1.0);
        part.set_pos(-x / scale, (y + DUCK * duck) / scale, -(z - DRAW_BACK * duck) / scale);
    };
    if let Some(body) = cat.get_child_mut("body") {
        place(body, "body_at");
        let [rx, ry, rz] = rad(t.get("body_rot"));
        body.set_rotation(rx, ry, rz);
    }
    let [amp, speed, _] = t.get("idle");
    let sway = amp.to_radians();
    if let Some(head) = cat.get_child_mut("head") {
        place(head, "head_at");
        let [rx, ry, rz] = rad(t.get("head_rot"));
        head.set_rotation(
            rx + 0.5 * sway * (age * speed * 0.7).sin(),
            ry + 0.6 * sway * (age * speed * 0.45).sin(),
            rz + sway * (age * speed).sin(),
        );
    }
    let kind = t.get("paw")[0] as i32;
    for (name, at, rot, of_kind) in [
        ("arm_r", "arm_r_at", "arm_r_rot", 0),
        ("arm_l", "arm_l_at", "arm_l_rot", 0),
        ("thick_r", "arm_r_at", "arm_r_rot", 1),
        ("thick_l", "arm_l_at", "arm_l_rot", 1),
        ("thin_r", "arm_r_at", "arm_r_rot", 2),
        ("thin_l", "arm_l_at", "arm_l_rot", 2),
    ] {
        let Some(arm) = cat.get_child_mut(name) else { continue };
        place(arm, at);
        let [rx, ry, rz] = rad(t.get(rot));
        arm.set_rotation(rx, ry, rz);
        arm.visible = of_kind == kind;
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
        let Some(body) = parts.and_then(|p| p.get(HumanoidPart::Body)) else { return Ok(()) };
        let tune = Tune::current(worn.kind);
        draw.push();
        // The model's frame, as `LivingEntityRenderer` sets it up, then the body part's.
        draw.scale([1.0, -1.0, -1.0]);
        draw.translate([0.0, -1.501, 0.0]);
        apply(draw, &body.pose);
        let hurt = entity.living.as_ref().is_some_and(|l| l.has_red_overlay);
        let hide = match tune.get("hide")[0] {
            h if h >= 0.0 => h,
            _ => hiding(entity.id, hurt),
        };
        if let Some(bag) = lock(&BAG).as_mut() {
            pose_bag(bag, &tune, hide);
            draw.model_part(bag, BAG_SPRITE, 0xFF_FFFF);
        }
        if let Some(cat) = lock(worn.kind.model()).as_mut() {
            pose(cat, &tune, entity.age, hide);
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
