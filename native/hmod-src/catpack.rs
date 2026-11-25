//! The cat backpack: a backpack on the player's back, its front half leaning open, with a small cat
//! sitting in it, head and crossed paws over the rim, looking away from the player's back.
//! Drawn by an entity layer on both player models through the mod API (no mixins). The bag is the
//! mod's own model and texture. The cat, grown or a kitten, is the mod's own small model laid out
//! on the game's texture of it (`entity/cat/cat_<variant>`, 64×32, or `…_baby`, 32×32): the
//! game's own head, a short body and two-part front legs cut from the same places of that
//! texture, so every cat variant fits without a texture of ours.
//!
//! Worn as an item: the mod's own bag item in the chest slot, whose `custom_data` has `catpack`
//! (`kitten` or `cat`) and, optionally, `catpack_cat` (a variant), or worn in the back slot the
//! server keeps (`catpack_rules`). Only in the player's own world (`crate::own_world`).

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, PoisonError};

use mod_api::{
    BlockEntityDrawContext, CubeListBuilder, EntityRenderLayer, EntityRenderState,
    HumanoidPart, LayerDefinition, LayerTarget, MeshDefinition, ModelPart, ModelPolygon, ModelVertex, PartPose,
    PlayerModel, PosedParts,
};

/// The bag's texture: `assets/spider_man/textures/entity/catpack.png` (made by
/// `native/tools/catpack_texture.py`).
const BAG_SPRITE: &str = "spider_man:entity/catpack";

/// The grown cat's eyes, `assets/spider_man/textures/entity/cat_eyes.png` (made by
/// `native/tools/cat_eyes_texture.py`): a yellow iris standing out of the game's head, a dark pupil
/// that moves freely in it, and lids in the cat's own fur. Proportions after the measured reference
/// (an 8-wide head: eyes 3 wide and 3.6 tall, flush with its sides, 2.1 apart, the pupil 1.85 by
/// 1.9), scaled to the cat's 5-wide face.
const EYE_SPRITE: &str = "spider_man:entity/cat_eyes";
/// In the grown cat's centred head frame (the head 5×4×5 from (-2.5, -2, -2.5), +y down, the face
/// at z -2.5 towards -z, the nose 3×2 a pixel out at the bottom): each eye's left edge, its top,
/// its size, its front (a quarter texel out of the face) and the face behind it.
const EYE_X: [f32; 2] = [-2.5, 0.5];
const EYE_TOP: f32 = -1.95;
const EYE_W: f32 = 2.0;
const EYE_H: f32 = 2.25;
const EYE_FRONT: f32 = -2.75;
const FACE: f32 = -2.5;
/// The pupil's size at rest.
const PUPIL: [f32; 2] = [1.16, 1.19];
/// The forehead texel of the cat texture the lids are coloured from (the head's front face starts at 5, 5).
const LID_UV: [f32; 2] = [7.0, 5.0];
/// Each eye's lids (top, bottom, angry brow, worried brow) and pupil, by name.
const LIDS: [[&str; 4]; 2] = [["lid0_top", "lid0_bottom", "lid0_angry", "lid0_worried"], ["lid1_top", "lid1_bottom", "lid1_angry", "lid1_worried"]];
const PUPILS: [&str; 2] = ["pupil0", "pupil1"];

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
    /// A bag with no cat in it: worn shut.
    Empty,
}

impl Kind {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "kitten" => Some(Self::Kitten),
            "cat" => Some(Self::Cat),
            "empty" => Some(Self::Empty),
            _ => None,
        }
    }
    fn texture(self, variant: &str) -> String {
        match self {
            Self::Kitten => format!("entity/cat/cat_{variant}_baby"),
            Self::Cat | Self::Empty => format!("entity/cat/cat_{variant}"),
        }
    }
    fn model(self) -> &'static Mutex<Option<ModelPart>> {
        match self {
            Self::Kitten => &KITTEN,
            Self::Cat | Self::Empty => &CAT,
        }
    }
}

/// What a bag on a back shows.
#[derive(Clone, PartialEq, Debug)]
struct Worn {
    kind: Kind,
    variant: String,
}

/// Each drawn player's entity id and the bag in their chest slot, as of the last client tick: a
/// worn stack is sent to everyone who sees its wearer, so other players' bags show too.
static WORN: Mutex<Vec<(u32, Worn)>> = Mutex::new(Vec::new());
/// Each drawn player's bags in hand, as of the last client tick: the entity, whether the off
/// hand, and what the bag shows.
static HELD: Mutex<Vec<(u32, bool, Worn)>> = Mutex::new(Vec::new());
/// Client ticks since the game began: a held bag's clock.
static CLOCK: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// This window's own player: their health at the last tick, and the tick they last lost some.
static OWN_HEALTH: Mutex<(f32, u32)> = Mutex::new((0.0, 0));
/// The players whose bag was asked for since the last client tick.
static SEEN: Mutex<Vec<u32>> = Mutex::new(Vec::new());
/// The two cats and the bag as built once at client start; posed again each frame.
static KITTEN: Mutex<Option<ModelPart>> = Mutex::new(None);
static CAT: Mutex<Option<ModelPart>> = Mutex::new(None);
static BAG: Mutex<Option<ModelPart>> = Mutex::new(None);
static EYES: Mutex<Option<ModelPart>> = Mutex::new(None);

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The bag's item id. It is listed on the mod's own creative tab
/// (`data/spider_man/horizon/creative_tab/spider_man.json`).
pub const ITEM: &str = "spider_man:cat_backpack";

/// The bag's item as this run of the game knows it: learnt from the first stack of it seen with
/// a host at hand (an item's id is the host's to say), so that a stack can be told a bag
/// without one.
static BAG_ITEM: std::sync::OnceLock<mod_api::ItemKey> = std::sync::OnceLock::new();

/// The bag's item as the mod registered it (`register`): known before any stack of it is seen.
pub fn set_item_key(key: mod_api::ItemKey) {
    let _ = BAG_ITEM.set(key);
}

/// Looks at `stack` for the bag's item.
pub fn learn(host: &dyn mod_api::Host, stack: &mod_api::ItemStack) {
    if BAG_ITEM.get().is_none() && stack.item().id(host) == ITEM {
        let _ = BAG_ITEM.set(stack.item());
    }
}

/// Whether the bag's item is known yet.
pub fn learnt() -> bool {
    BAG_ITEM.get().is_some()
}

/// Whether the stack is the mod's own bag item.
pub fn is_bag_item(stack: &mod_api::ItemStack) -> bool {
    BAG_ITEM.get() == Some(&stack.item())
}

/// The bag as an item of the mod's own: one to a stack, fire-proof (a cat is not lost to one
/// slip into lava), born empty (a bag with no `custom_data` of its own is an empty one). It is its own item so that nothing of the game's takes it for
/// something else: no recipe, no trade, no use of a vanilla item it might have been made of.
pub fn item() -> mod_api::Item {
    use mod_api::Tag;
    let mut item = mod_api::Item::new(ITEM, "minecraft:clock");
    item.max_stack_size = Some(1);
    let mut fire = mod_api::Compound::default();
    fire.insert("types".into(), Tag::String("#minecraft:is_fire".into()));
    item.components = vec![
        ("minecraft:item_model".into(), Some(Tag::String("spider_man:catpack".into()))),
        ("minecraft:item_name".into(), Some(Tag::String("Back catpack".into()))),
        ("minecraft:damage_resistant".into(), Some(Tag::Compound(fire))),
    ];
    item
}

/// Builds both cats (each one's head is the game's own, from `minecraft:cat#main` and
/// `minecraft:cat_baby#main`) and the bag. Called once at client start.
pub fn bake(host: &dyn mod_api::ClientHost) {
    let head = |layer: &str| host.bake_layer(layer).and_then(|cat| cat.get_child("head").cloned());
    *lock(&CAT) = head("minecraft:cat#main").map(cat);
    *lock(&KITTEN) = head("minecraft:cat_baby#main").map(kitten);
    *lock(&BAG) = Some(bag());
    *lock(&EYES) = Some(eyes());
}

/// A quad facing -z from (`x0`, `y0`) to (`x1`, `y1`) at depth `z`, its texture from (`ua`, `va`)
/// to (`ub`, `vb`), 0 to 1. With `tri`, its bottom outer corner is pulled to the top outer one: a
/// triangle, `tri` saying which side (-1 left, 1 right) keeps its full height.
fn quad(x0: f32, y0: f32, x1: f32, y1: f32, z: f32, [ua, va, ub, vb]: [f32; 4], tri: f32) -> ModelPolygon {
    let (left_bottom, right_bottom) = match tri {
        t if t > 0.0 => (y0, y1),
        t if t < 0.0 => (y1, y0),
        _ => (y1, y1),
    };
    let v = |x: f32, y: f32, u: f32, w: f32| ModelVertex::new([x, y, z], u, w);
    ModelPolygon::new([v(x1, y0, ub, va), v(x0, y0, ua, va), v(x0, left_bottom, ua, vb), v(x1, right_bottom, ub, vb)], [0.0, 0.0, -1.0])
}

/// A face of one flat colour: every corner on the middle of the texel (`u`, `v`) of a `size` texture.
fn solid(u: f32, v: f32, size: [f32; 2]) -> [f32; 4] {
    let (u, v) = ((u + 0.5) / size[0], (v + 0.5) / size[1]);
    [u, v, u, v]
}

/// The grown cat's head with the eyes' lids added, on the cat's own forehead fur so every variant's
/// lids match its coat: per eye a top lid hanging from the eye's top edge, a bottom lid standing
/// on its bottom edge, and two slanted brows (down to the inner corner, down to the outer), each
/// scaled from its edge by how far it is closed.
fn with_lids(head: ModelPart) -> ModelPart {
    let fur = solid(LID_UV[0], LID_UV[1], [64.0, 32.0]);
    let z = EYE_FRONT - 0.02;
    let mut children: Vec<(String, ModelPart)> = head.children().map(|(n, p)| (n.to_owned(), p.clone())).collect();
    for (side, (x0, names)) in EYE_X.iter().zip(LIDS).enumerate() {
        let (x1, inner) = (x0 + EYE_W, if side == 0 { 1.0 } else { -1.0 });
        let shapes = [
            (quad(*x0, 0.0, x1, EYE_H, z, fur, 0.0), EYE_TOP),
            (quad(*x0, -EYE_H, x1, 0.0, z, fur, 0.0), EYE_TOP + EYE_H),
            (quad(*x0, 0.0, x1, EYE_H, z, fur, inner), EYE_TOP),
            (quad(*x0, 0.0, x1, EYE_H, z, fur, -inner), EYE_TOP),
        ];
        for (name, (polygon, y)) in names.into_iter().zip(shapes) {
            children.push((name.to_owned(), ModelPart::new(vec![polygon], Vec::new(), PartPose::offset(0.0, y, 0.0))));
        }
    }
    ModelPart::new(head.polygons.clone(), children, head.initial_pose())
}

/// The eyes' own model, on `EYE_SPRITE`: a root turned half round as the cat's (`assemble`) and a
/// `head` posed each frame as the cat's, holding each eye (the iris standing out of the face, its
/// sides, the top darker under the lid and the bottom amber) and its pupil, a free quad moved and
/// scaled each frame.
fn eyes() -> ModelPart {
    const T: [f32; 2] = [8.0, 16.0];
    let side = solid(4.0, 0.0, T);
    let mut polygons = Vec::new();
    let mut parts = Vec::new();
    for (x0, pupil) in EYE_X.into_iter().zip(PUPILS) {
        let (x1, y0, y1) = (x0 + EYE_W, EYE_TOP, EYE_TOP + EYE_H);
        polygons.push(quad(x0, y0, x1, y1, EYE_FRONT, [0.0, 0.0, 4.0 / T[0], 1.0], 0.0));
        // The sides, from the iris back into the face: top, bottom, left, right.
        let v = |x: f32, y: f32, z: f32| ModelVertex::new([x, y, z], side[0], side[1]);
        let (zf, zb) = (EYE_FRONT, FACE);
        polygons.push(ModelPolygon::new([v(x1, y0, zb), v(x0, y0, zb), v(x0, y0, zf), v(x1, y0, zf)], [0.0, -1.0, 0.0]));
        polygons.push(ModelPolygon::new([v(x1, y1, zf), v(x0, y1, zf), v(x0, y1, zb), v(x1, y1, zb)], [0.0, 1.0, 0.0]));
        polygons.push(ModelPolygon::new([v(x0, y0, zf), v(x0, y0, zb), v(x0, y1, zb), v(x0, y1, zf)], [-1.0, 0.0, 0.0]));
        polygons.push(ModelPolygon::new([v(x1, y0, zb), v(x1, y0, zf), v(x1, y1, zf), v(x1, y1, zb)], [1.0, 0.0, 0.0]));
        let [w, h] = PUPIL.map(|s| s * 0.5);
        let dot = quad(-w, -h, w, h, EYE_FRONT - 0.01, solid(5.0, 0.0, T), 0.0);
        parts.push((pupil.to_owned(), ModelPart::new(vec![dot], Vec::new(), PartPose::ZERO)));
    }
    let head = ModelPart::new(polygons, parts, PartPose::ZERO);
    ModelPart::new(Vec::new(), vec![("head".to_owned(), head)], PartPose::rotation(0.0, std::f32::consts::PI, 0.0))
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
    assemble(with_lids(centred(head, [0.0, 0.0, -0.5])), body, legs)
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

/// Leaving a world or a connection: the bags this client was told of, and saw, are forgotten.
pub fn forget() {
    lock(&WORN).clear();
    lock(&HELD).clear();
    lock(&SEEN).clear();
    lock(&VELOCITY).clear();
    *lock(&OWN_HEALTH) = (0.0, 0);
}

/// Each client tick: which bag the local player wears, read from their chest slot.
pub fn tick(host: &dyn mod_api::ClientHost) {
    if !learnt()
        && let Some(player) = host.local_player()
    {
        for stack in player.inventory(host).unwrap_or_default() {
            learn(host, &stack);
        }
    }
    // The players drawn since the last tick, and this window's own.
    let mut ids = std::mem::take(&mut *lock(&SEEN));
    if let Some(player) = host.local_player() {
        let id = player.entity().id();
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    let mut worn = Vec::new();
    let mut held = Vec::new();
    let mut velocity = Vec::new();
    for id in ids {
        let Some(entity) = host.entity(id) else { continue };
        if let Some(v) = entity.velocity(host) {
            velocity.push((id, v.map(|c| c as f32)));
        }
        let living = mod_api::LivingRef::new(entity);
        if let Some(bag) = living.equipment(host, mod_api::EquipmentSlot::Chest).and_then(|stack| worn_by(&stack)) {
            worn.push((id, bag));
        }
        for (hand, off) in [(mod_api::Hand::Main, false), (mod_api::Hand::Off, true)] {
            if let Some(bag) = living.hand_item(host, hand).and_then(|stack| worn_by(&stack)) {
                held.push((id, off, bag));
            }
        }
    }
    *lock(&HELD) = held;
    let now = CLOCK.fetch_add(1, Ordering::Relaxed) + 1;
    if let Some(health) = host.local_player().and_then(|player| mod_api::LivingRef::new(player.entity()).health(host)) {
        let mut own = lock(&OWN_HEALTH);
        if health < own.0 {
            own.1 = now;
        }
        own.0 = health;
    }
    // The bags on backs, as the server told this client.
    for (id, kind, variant) in crate::catpack_ui::backs() {
        let Some(kind) = Kind::parse(&kind) else { continue };
        let variant = VARIANTS.iter().copied().find(|v| *v == variant).unwrap_or("red").to_owned();
        worn.retain(|(other, _)| *other != id);
        worn.push((id, Worn { kind, variant }));
    }
    *lock(&WORN) = worn;
    *lock(&VELOCITY) = velocity;
}

/// The bag a stack is, by its `custom_data`: `catpack` names the kind, `catpack_cat` the variant.
fn worn_by(stack: &mod_api::ItemStack) -> Option<Worn> {
    let read = |name: &str| mod_api::ItemResource::of(stack).component(&mod_api::ComponentId::new(name)?);
    let data = read("minecraft:custom_data");
    let text = |key: &str| match data.as_ref()?.get(key) {
        Some(mod_api::Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    };
    // Only the mod's own item is a bag, whatever another item's data says.
    if !is_bag_item(stack) {
        return None;
    }
    let kind = match text("catpack") {
        Some(named) => Kind::parse(&named)?,
        // The mod's own item, as it is made: no data of its own yet.
        None => Kind::Empty,
    };
    let variant = text("catpack_cat").filter(|v| VARIANTS.contains(&v.as_str())).unwrap_or_else(|| "red".into());
    Some(Worn { kind, variant })
}

/// Where the lid hinges: the top section's back, its top outer edge, by the wearer's back.
const LID_HINGE: [f32; 3] = [0.0, 1.0, 2.0];

/// The bag in the body's frame, in whole texels (the style's rule: one texture pixel is one model
/// pixel): +y down from the shoulders, the back towards +z (the body is 8 wide, 12 tall, 4 deep
/// about its pivot). After the bag Miles carries his cat in, laid over the game's own pictures
/// of it: narrower at the top than at the foot, deeper than the body.
///
/// - the trunk, as wide as the back, from the chest down to the hips;
/// - on it a narrower top section, unzipped: a back, two cheeks and a front wall, with a well
///   between them where the cat sits on the trunk's top; a grab handle above the back;
/// - the lid, hinged on the back: shut, it lies over the well; open, it stands up behind the
///   cat, lining towards the viewer, so the cat is seen whole from behind (it stands inside the
///   wearer's head: taken, for the look);
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
        .tex_offs(0, 13)
        .add_box([-3.0, 7.0, 7.0], [6.0, 5.0, 2.0])
        .tex_offs(50, 0)
        .add_box([-5.0, 7.0, 3.0], [1.0, 4.0, 3.0])
        .add_box_mirrored([4.0, 7.0, 3.0], [1.0, 4.0, 3.0], true);
    // The narrower section the cat looks out of, with the handle: a part of its own, so the
    // tuning file's `short` can show the bag without it.
    let top = CubeListBuilder::create()
        .tex_offs(26, 0)
        .add_box([-3.0, 1.0, 2.0], [6.0, 3.0, 1.0])
        .tex_offs(26, 4)
        .add_box([-3.0, 1.0, 6.0], [6.0, 3.0, 1.0])
        .tex_offs(40, 0)
        .add_box([-3.0, 1.0, 3.0], [1.0, 3.0, 4.0])
        .add_box_mirrored([2.0, 1.0, 3.0], [1.0, 3.0, 4.0], true)
        .tex_offs(58, 0)
        .add_box([-1.0, 0.0, 2.0], [2.0, 1.0, 1.0]);
    mesh.add_or_replace_child(root, "shell", shell, PartPose::ZERO);
    mesh.add_or_replace_child(root, "top", top, PartPose::ZERO);
    let hinge = PartPose::offset(LID_HINGE[0], LID_HINGE[1], LID_HINGE[2]);
    mesh.add_or_replace_child(root, "lid", one(26, 8, [-3.0, -1.0, 0.0], [6.0, 1.0, 5.0]), hinge);
    mesh.add_or_replace_child(root, "strap_top_r", one(32, 24, [-3.0, -0.5, -2.5], [1.0, 0.5, 5.0]), PartPose::ZERO);
    mesh.add_or_replace_child(root, "strap_top_l", one(32, 24, [2.0, -0.5, -2.5], [1.0, 0.5, 5.0]), PartPose::ZERO);
    mesh.add_or_replace_child(root, "strap_front_r", one(48, 24, [-3.0, 0.0, -2.75], [1.0, 7.0, 0.5]), PartPose::ZERO);
    mesh.add_or_replace_child(root, "strap_front_l", one(48, 24, [2.0, 0.0, -2.75], [1.0, 7.0, 0.5]), PartPose::ZERO);
    // The same straps as the bag carries them off a back: lying flat down its back panel.
    mesh.add_or_replace_child(root, "strap_flat_r", one(48, 24, [-3.0, 4.5, 1.5], [1.0, 7.0, 0.5]), PartPose::ZERO);
    mesh.add_or_replace_child(root, "strap_flat_l", one(48, 24, [2.0, 4.5, 1.5], [1.0, 7.0, 0.5]), PartPose::ZERO);
    LayerDefinition::create(mesh, 64, 32).bake_root()
}

/// What a pose is made of: `key = a b c` lines of the tuning file replace the shown cat's numbers.
/// Places are in the bag's frame (texels, +y down, +z away from the player's back), turns in
/// degrees (the cat's parts turn in the cat's own frame, which looks along -z).
const KEYS: [&str; 20] = [
    // The cat's size against the game's own model of it.
    "scale",
    // How far the lid swings from lying shut, in degrees: three quarters of a turn, over the
    // wearer's shoulders and down flat against their back, out of the cat's way.
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
    // How lively the cat is at rest: 1 as made, 0 still (for shots).
    "idle",
    // The grown cat's paws: 0 flat (2×3×1), 1 the game's thickness (2×3×2), 2 thin (1×3×1).
    "paw",
    // For looking at the hiding: 0 to 1 holds the cat that far ducked; below 0, the game decides.
    "hide",
    // A trial of a shorter bag: above 0, the top section is not drawn and the lid and the cat
    // sit that many texels lower, on the trunk (3 is the section's height).
    "short",
    // The bag held in a hand, seen by its holder: where its middle is in the view (blocks, +x to
    // the hand's side, +y up, -z into the screen), its turn (pitch, yaw, roll) and its size.
    "hand_at",
    "hand_rot",
    "hand_scale",
    // The bag held in a hand, seen from outside: where its middle is on the arm (texels from the
    // shoulder, +x outwards, +y down the arm, +z back), its turn and its size.
    "arm_at",
    "arm_rot",
    "arm_scale",
];

/// The grown cat's pose, in `KEYS`' order: sitting on the trunk's top in the well, by its front
/// wall, its head and neck over it, looking up and aside past the player's shoulder, both paws
/// over the wall, one a little further out (as the game's lying cat holds them).
const CAT_POSE: [[f32; 3]; KEYS.len()] = [
    [0.6, 0.0, 0.0],
    [270.0, 0.0, 0.0],
    [0.0, 4.0, 5.0],
    [6.0, 0.0, 0.0],
    [0.0, -0.5, 5.8],
    [-12.0, -30.0, 6.0],
    [-0.8, 0.5, 6.3],
    [-62.0, 0.0, 0.0],
    [0.8, 0.4, 6.2],
    [-56.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
    [-1.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
    [0.46, -0.36, -0.8],
    [8.0, 160.0, 0.0],
    [0.5, 0.0, 0.0],
    [1.0, 15.0, -1.0],
    [0.0, 180.0, 0.0],
    [0.7, 0.0, 0.0],
];

/// The kitten's pose: the same seat, only its head over the wall.
const KITTEN_POSE: [[f32; 3]; KEYS.len()] = [
    [0.5, 0.0, 0.0],
    [270.0, 0.0, 0.0],
    [0.0, 3.0, 5.0],
    [6.0, 0.0, 0.0],
    [0.0, 0.2, 5.7],
    [-12.0, -30.0, 6.0],
    [0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
    [-1.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
    [0.46, -0.36, -0.8],
    [8.0, 160.0, 0.0],
    [0.5, 0.0, 0.0],
    [1.0, 15.0, -1.0],
    [0.0, 180.0, 0.0],
    [0.7, 0.0, 0.0],
];

#[derive(Clone)]
struct Tune([[f32; 3]; KEYS.len()]);

impl Tune {
    fn of(kind: Kind) -> Self {
        Self(match kind {
            Kind::Kitten => KITTEN_POSE,
            Kind::Cat | Kind::Empty => CAT_POSE,
        })
    }

    fn get(&self, key: &str) -> [f32; 3] {
        KEYS.iter().position(|k| *k == key).map_or([0.0; 3], |i| self.0[i])
    }

    /// `kind`'s pose.
    fn current(kind: Kind) -> Self {
        Self::of(kind)
    }
}

fn rad(deg: [f32; 3]) -> [f32; 3] {
    deg.map(f32::to_radians)
}

/// The lid's turn: from standing open behind the cat (`shut` 0, the pose's angle up from lying)
/// down to lying over the well (`shut` 1).
fn pose_bag(bag: &mut ModelPart, t: &Tune, shut: f32) {
    let short = t.get("short")[0].max(0.0);
    if let Some(lid) = bag.get_child_mut("lid") {
        lid.x_rot = (t.get("open")[0] * (1.0 - shut)).to_radians();
        lid.y = LID_HINGE[1] + short;
    }
    if let Some(top) = bag.get_child_mut("top") {
        top.visible = short == 0.0;
    }
}

/// How deep the cat ducks into the well when it hides, and how far it draws back from the front
/// wall as it does, in the bag's texels.
const DUCK: f32 = 3.4;
const DRAW_BACK: f32 = 1.5;
/// The hiding, in ticks after the carrier is hurt: a start (the cat jerks up), the dive and the
/// lid's slam, then from `PEEK_AT` the lid lifts and the cat looks out from under it, and at
/// `OUT_AT` it comes back out.
const FLINCH: f32 = 2.0;
const PEEK_AT: f32 = 56.0;
const OUT_AT: f32 = 84.0;

/// A damped spring, stepped in ticks: what gives the motion its weight (a turn that overshoots
/// and settles, a lid that bounces).
#[derive(Clone, Copy, Default)]
struct Spring {
    x: f32,
    v: f32,
}

impl Spring {
    fn step(&mut self, target: f32, stiffness: f32, damping: f32, dt: f32) {
        // Small steps keep a stiff spring stable at a low frame rate.
        let steps = (dt / 0.25).ceil().max(1.0);
        let h = dt / steps;
        for _ in 0..steps as usize {
            self.v += ((target - self.x) * stiffness - self.v * damping) * h;
            self.x += self.v * h;
        }
    }
}

/// One carrier's cat in motion.
#[derive(Clone, Copy)]
struct Motion {
    entity: u32,
    /// The carrier's age at the last frame, and at the last hit.
    age: f32,
    hit: f32,
    was_hurt: bool,
    body_rot: f32,
    /// How far the cat has ducked (0 out, 1 down in the well) and each paw after it, the lid (0
    /// hanging open, 1 shut), the head's turn, nod and tilt in degrees on top of the pose's, the
    /// trunk's lean, and the whole cat's bob in texels.
    duck: Spring,
    paws: [Spring; 2],
    lid: Spring,
    yaw: Spring,
    pitch: Spring,
    roll: Spring,
    lean: Spring,
    bob: Spring,
    /// The eyes, eased towards the mood's each tick.
    eyes: EyeFrame,
}

/// The eyes: the pupil's place (-1 to 1 of its reach, x then y in the head's frame) and how much
/// bigger than at rest it is; how far each lid is closed, 0 to 1 (the top, the bottom, a brow
/// slanting down to the inner corner, one slanting down to the outer).
#[derive(Clone, Copy, Default)]
struct EyeFrame {
    pupil: [f32; 2],
    /// How far both pupils are drawn towards the nose (a glare), -1 to 1 of their reach.
    cross: f32,
    dilate: f32,
    top: f32,
    bottom: f32,
    angry: f32,
    worried: f32,
}

/// What `Motion` gives a frame's pose.
#[derive(Clone, Copy, Default)]
struct Frame {
    duck: f32,
    paws: [f32; 2],
    shut: f32,
    yaw: f32,
    pitch: f32,
    roll: f32,
    lean: f32,
    bob: f32,
    /// The paws' kneading, degrees each.
    knead: [f32; 2],
    hidden: bool,
    eyes: EyeFrame,
}

static MOTION: Mutex<Vec<Motion>> = Mutex::new(Vec::new());
/// Each drawn player's entity id and velocity (blocks a tick), as of the last client tick: what
/// the render state doesn't carry.
static VELOCITY: Mutex<Vec<(u32, [f32; 3])>> = Mutex::new(Vec::new());

/// A number from 0 to 1 that stays the same for `entity` through one `slot` of time.
fn chance(entity: u32, slot: i32, salt: u32) -> f32 {
    let mut h = entity.wrapping_mul(0x9E37_79B1) ^ (slot as u32).wrapping_mul(0x85EB_CA6B) ^ salt.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

/// Moves `entity`'s cat on to this frame and says how it is held.
///
/// At rest it lives: it looks around (a new place every few seconds, now and then a quick
/// tilt), breathes, and kneads the lid. It has weight: the head lags behind the carrier's turns
/// and leans into them, bobs with their steps, sinks as they jump and lifts as they fall, and
/// hunkers down at speed while the lid flutters. Hurt, the carrier sends it into the bag: a
/// start, a dive, the lid slamming with a bounce; then the lid lifts and it looks out both ways
/// from under it, and it pops back out, head first, one paw after the other.
/// Who carries a cat, as its motion reads them: a wearer drawn in the world, or a hand.
struct Carrier {
    /// One motion per key: a wearer's entity id; a hand's, its holder's with a high bit set.
    key: u32,
    age: f32,
    hurt: bool,
    body_rot: f32,
    walk: f32,
    stride: f32,
    velocity: [f32; 3],
    /// The hand's swing, 0 to 1: the cat swats along with it.
    swing: f32,
    /// The bag is in a hand, before its holder's eyes.
    held: bool,
}

fn motion(entity: &EntityRenderState, t: &Tune) -> Frame {
    let living = entity.living.as_ref();
    let (walk, stride) = living.map_or((0.0, 0.0), |l| (l.walk_animation_speed.clamp(0.0, 1.5), l.walk_animation_pos));
    motion_of(
        &Carrier {
            key: entity.id,
            age: entity.age,
            hurt: living.is_some_and(|l| l.has_red_overlay),
            body_rot: living.map_or(0.0, |l| l.body_rot),
            walk,
            stride,
            velocity: lock(&VELOCITY).iter().find(|(id, _)| *id == entity.id).map_or([0.0; 3], |(_, v)| *v),
            swing: 0.0,
            held: false,
        },
        t,
    )
}

fn motion_of(carrier: &Carrier, t: &Tune) -> Frame {
    let [amount, _, _] = t.get("idle");
    let Carrier { key, age, hurt, body_rot, walk, stride, velocity, swing, held } = *carrier;
    let mut all = lock(&MOTION);
    // Carriers not drawn for a while are forgotten (their age stops while they aren't).
    if all.len() > 64 {
        all.clear();
    }
    let i = all.iter().position(|m| m.entity == key).unwrap_or_else(|| {
        all.push(Motion {
            entity: key,
            age,
            hit: f32::NEG_INFINITY,
            was_hurt: hurt,
            body_rot,
            duck: Spring::default(),
            paws: [Spring::default(); 2],
            lid: Spring::default(),
            yaw: Spring::default(),
            pitch: Spring::default(),
            roll: Spring::default(),
            lean: Spring::default(),
            bob: Spring::default(),
            eyes: EyeFrame::default(),
        });
        all.len() - 1
    });
    let m = &mut all[i];
    // A step back in age is another world or a respawn: start over from rest.
    let dt = if age < m.age { 0.0 } else { (age - m.age).min(4.0) };
    if age < m.age {
        m.hit = f32::NEG_INFINITY;
    }
    m.age = age;
    if hurt && !m.was_hurt {
        m.hit = age;
    }
    m.was_hurt = hurt;
    let since = age - m.hit;

    // The carrier: how fast they turn (degrees a tick), walk, rise and move.
    let mut turn = (body_rot - m.body_rot).rem_euclid(360.0);
    if turn > 180.0 {
        turn -= 360.0;
    }
    let turn = if dt > 0.0 { (turn / dt).clamp(-30.0, 30.0) } else { 0.0 };
    m.body_rot = body_rot;
    let rise = velocity[1].clamp(-1.5, 1.0);
    let speed = velocity[0].hypot(velocity[2]).min(2.0);
    let rush = ((speed - 0.35) / 0.6).clamp(0.0, 1.0);

    // Where the hiding wants the cat and the lid.
    let (duck_to, shut_to, hiding) = if since < FLINCH {
        (-0.18, 0.0, true)
    } else if since < PEEK_AT {
        (1.0, if m.duck.x > 0.85 { 1.0 } else { 0.0 }, true)
    } else if since < OUT_AT {
        // Peeking: the lid lifted some thirty degrees, the cat's eyes under its edge.
        (0.45, 0.89, true)
    } else {
        (0.25 * rush, 0.0, false)
    };

    // Where the head wants to look: every three seconds a new place, held; under the lid, out
    // one way and then the other.
    let slot = ((age + (key % 97) as f32 * 7.0) / 60.0).floor() as i32;
    let mut look = [(chance(key, slot, 1) - 0.5) * 70.0, (chance(key, slot, 2) - 0.6) * 22.0];
    if held {
        // In a hand it faces its holder: half the time it looks straight up at them.
        if chance(key, slot, 3) < 0.5 {
            look = [0.0, -18.0];
        }
    } else if chance(key, slot, 3) < 0.25 {
        // Now and then it looks round at its carrier.
        look = [48.0, -14.0];
    }
    if since >= PEEK_AT && since < OUT_AT {
        look = [if since < (PEEK_AT + OUT_AT) / 2.0 { -38.0 } else { 30.0 }, 4.0];
    } else if hiding {
        look = [0.0, 10.0];
    }
    let steps = (stride * 0.6662).cos();
    let yaw_to = look[0] * amount - turn * 3.0;
    let pitch_to = look[1] * amount + if since < FLINCH { -24.0 } else { 0.0 } + rise * -22.0 + rush * 14.0 + walk * 3.0 * (stride * 1.3324).sin();
    let roll_to = turn * 1.4 + walk * 4.0 * steps;
    let lean_to = turn * 0.8 + walk * 3.0 * steps;
    let bob_to = rise * 1.4 + walk * 0.3 * (stride * 1.3324).cos().abs() + amount * 0.12 * (age * 0.11).sin();
    // The lid flutters at speed, lifts as the carrier falls and swings with their steps.
    let lid_to = shut_to + if hiding { 0.0 } else { rush * 0.05 * (age * 1.9).sin() + (-rise * 0.25).clamp(0.0, 0.3) + walk * 0.02 * (stride * 1.3324).sin() };

    if dt > 0.0 {
        // Down fast, up with a bounce; the paws a beat behind the head, one after the other.
        let (k, d) = if duck_to > m.duck.x { (0.55, 0.95) } else { (0.26, 0.5) };
        m.duck.step(duck_to, k, d, dt);
        m.paws[0].step(duck_to.max(0.0), k * 0.75, d * 0.95, dt);
        m.paws[1].step(duck_to.max(0.0), k * 0.5, d * 0.85, dt);
        let (k, d) = if lid_to > m.lid.x { (0.7, 0.55) } else { (0.22, 0.38) };
        m.lid.step(lid_to, k, d, dt);
        // The lid can't pass the top of the bag: it bounces off it.
        if m.lid.x > 1.0 {
            m.lid.x = 1.0;
            m.lid.v *= -0.45;
        }
        m.yaw.step(yaw_to, 0.12, 0.48, dt);
        m.pitch.step(pitch_to, 0.16, 0.5, dt);
        m.roll.step(roll_to, 0.2, 0.42, dt);
        m.lean.step(lean_to, 0.18, 0.5, dt);
        m.bob.step(bob_to, 0.3, 0.55, dt);
        // A quick tilt of the head as a new look begins, as a cat's ear flicks.
        let into = (age + (key % 97) as f32 * 7.0).rem_euclid(60.0);
        if into < dt && chance(key, slot, 4) < 0.4 && !hiding {
            m.roll.v += 4.0 * amount * if chance(key, slot, 5) < 0.5 { 1.0 } else { -1.0 };
        }
    }
    // Kneading: a few seconds now and then, the paws pressing in turn.
    let kneading = if !hiding && chance(key, slot, 6) < 0.3 { amount * (1.0 - rush) } else { 0.0 };
    let press = (age * 0.45).sin();
    // The hand swings and the cat swats with it: one paw out and back, the head after it.
    let swat = if hiding { 0.0 } else { (swing.clamp(0.0, 1.0) * std::f32::consts::PI).sin() };
    let eyes = eyes_of(m, key, age, dt, since, hiding, [yaw_to, pitch_to], rise, rush, walk, kneading, swat, held, slot);
    Frame {
        duck: m.duck.x,
        paws: [m.paws[0].x, m.paws[1].x],
        shut: m.lid.x.max(-0.08),
        yaw: m.yaw.x.clamp(-75.0, 75.0),
        pitch: (m.pitch.x + 10.0 * swat).clamp(-35.0, 35.0),
        roll: m.roll.x.clamp(-25.0, 25.0),
        lean: m.lean.x.clamp(-15.0, 15.0),
        bob: m.bob.x.clamp(-1.2, 1.6),
        knead: [9.0 * kneading * press.max(0.0) - 46.0 * swat, 9.0 * kneading * (-press).max(0.0) - 12.0 * swat],
        hidden: m.duck.x > 0.9 && m.lid.x > 0.93,
        eyes,
    }
}

/// A blink `p` ticks in, 0 open to 1 shut, as measured on the reference at 60 fps: down in 4
/// frames, shut 3, up in 6, the lid sliding smoothly.
fn blink(p: f32) -> f32 {
    match p {
        p if !(0.0..4.33).contains(&p) => 0.0,
        p if p < 1.33 => p / 1.33,
        p if p < 2.33 => 1.0,
        p => {
            let t = 1.0 - (p - 2.33) / 2.0;
            t * t * (3.0 - 2.0 * t)
        }
    }
}

/// A cat's slow blink (what a content cat gives the one it trusts): down over 6 ticks, shut 8, up 6.
fn slow_blink(p: f32) -> f32 {
    match p {
        p if !(0.0..20.0).contains(&p) => 0.0,
        p if p < 6.0 => p / 6.0,
        p if p < 14.0 => 1.0,
        p => 1.0 - (p - 14.0) / 6.0,
    }
}

/// The eyes this frame. The pupil darts from place to place across the whole eye (measured at
/// 60 fps: moves of 1 to 6 frames, often a single one; holds of 0.1 to 0.55 s) and leads the head's
/// turns. It grows to twice its size in about 0.17 s. Lids come in from the top and the bottom and as a slanted
/// brow, each snapping in within a tick or two; a blink every few seconds and at most new looks. The mood
/// follows what happens to the carrier.
#[allow(clippy::too_many_arguments)]
fn eyes_of(
    m: &mut Motion,
    key: u32,
    age: f32,
    dt: f32,
    since: f32,
    hiding: bool,
    head_to: [f32; 2],
    rise: f32,
    rush: f32,
    walk: f32,
    kneading: f32,
    swat: f32,
    held: bool,
    slot: i32,
) -> EyeFrame {
    let into = (age + (key % 97) as f32 * 7.0).rem_euclid(60.0);
    let peeking = (PEEK_AT..OUT_AT).contains(&since);
    let started = since < FLINCH + 8.0;
    let grumpy = (OUT_AT..OUT_AT + 70.0).contains(&since);
    let at_carrier = !held && chance(key, slot, 3) < 0.25;
    let drowsy = walk < 0.05 && chance(key, slot, 14) < 0.05;

    // The mood: (top, bottom, angry brow, worried brow, dilation) and where it holds the pupils, if
    // anywhere (towards the nose, down): the eye gives the shape, the pupil the look. Angry, the
    // reference drops the pupils down and in under the brow (a glare); worried, up and in.
    let ((top, bottom, angry, worried, dilate), fixed) = match () {
        _ if started => ((0.0, 0.0, 0.0, 0.0, 1.0), Some([0.0, 0.0])),
        _ if hiding && !peeking => ((1.0, 0.0, 0.0, 0.0, 0.0), None),
        _ if peeking => ((0.3, 0.3, 0.0, 0.55, 0.0), None),
        _ if grumpy || swat > 0.3 => ((0.0, 0.0, 0.65, 0.0, 0.0), Some([0.8, 0.85])),
        _ if rise < -0.55 => ((0.0, 0.0, 0.0, 0.0, 0.8), Some([0.0, -0.6])),
        _ if rush > 0.5 => ((0.38, 0.38, 0.0, 0.0, 0.0), Some([0.0, 0.0])),
        _ if kneading > 0.0 => ((0.0, 0.3, 0.0, 0.0, 0.0), None),
        _ if held => ((0.0, 0.0, 0.0, 0.0, 0.5), None),
        _ if drowsy => ((0.45, 0.0, 0.0, 0.0, 0.0), Some([0.0, 0.5])),
        _ => ((0.0, 0.0, 0.0, 0.0, 0.0), None),
    };
    // Where to look: a new glance every 4 ticks, kept for one to three of them (holds of 0.2 to
    // 0.6 s), plus where the head is still turning to (a positive turn looks to -x, a positive
    // nod down, +y).
    let g = ((age + (key % 31) as f32 * 3.0) / 4.0).floor() as i32;
    let g = g - (chance(key, g, 20) * 3.0) as i32 % 3;
    let glance = if hiding || chance(key, g, 21) < 0.3 { [0.0; 2] } else { [chance(key, g, 22) * 2.0 - 1.0, chance(key, g, 23) * 2.0 - 1.0] };
    let (look, cross) = match fixed {
        Some([cross, down]) => ([0.0, down], cross),
        None => (
            [(glance[0] - (head_to[0] - m.yaw.x) / 14.0).clamp(-1.0, 1.0), (glance[1] + (head_to[1] - m.pitch.x) / 12.0).clamp(-1.0, 1.0)],
            0.0,
        ),
    };
    if dt > 0.0 {
        let e = &mut m.eyes;
        let ease = |x: &mut f32, to: f32, rate: f32| *x += (to - *x) * (1.0 - (-rate * dt).exp());
        ease(&mut e.pupil[0], look[0], 6.0);
        ease(&mut e.pupil[1], look[1], 6.0);
        ease(&mut e.cross, cross, 6.0);
        ease(&mut e.dilate, dilate, 1.2);
        // Moods snap, as a cat's face does: there in a tick or two, then held.
        ease(&mut e.top, top, 2.5);
        ease(&mut e.bottom, bottom, 2.5);
        ease(&mut e.angry, angry, 2.5);
        ease(&mut e.worried, worried, 2.5);
    }

    // Blinks: one at a place in every 70 ticks (now and then a double), and one as most new looks begin.
    let bslot = ((age + (key % 53) as f32 * 5.0) / 70.0).floor() as i32;
    let p = age - (bslot as f32 * 70.0 - (key % 53) as f32 * 5.0 + chance(key, bslot, 11) * 40.0);
    let mut shut = blink(p);
    if chance(key, bslot, 12) < 0.25 {
        shut = shut.max(blink(p - 7.0));
    }
    if chance(key, slot, 13) < 0.7 {
        shut = shut.max(blink(into - 1.0));
    }
    if at_carrier {
        shut = shut.max(slow_blink(into - 12.0));
    } else if drowsy {
        shut = shut.max(slow_blink(into - 20.0));
    }
    let mut out = m.eyes;
    out.top = out.top.max(shut);
    out
}

/// Poses the cat for this frame as `f` holds it.
fn pose(cat: &mut ModelPart, t: &Tune, f: &Frame) {
    cat.visible = !f.hidden;
    // The cat is turned half round about y: a place (x, y, z) of the bag is (-x, y, -z) of the
    // cat. Places are the bag's texels whatever the cat's size.
    let scale = t.get("scale")[0].max(0.1);
    cat.x_scale = scale;
    cat.y_scale = scale;
    cat.z_scale = scale;
    let short = t.get("short")[0].max(0.0);
    // Ducking, a part also draws back from the front wall, so nothing of it shows through.
    let place = |part: &mut ModelPart, key: &str, duck: f32, bob: f32| {
        let [x, y, z] = t.get(key);
        let (back, down) = duck_path(duck);
        part.set_pos(-x / scale, (y + short + DUCK * down + bob) / scale, -(z - DRAW_BACK * back) / scale);
    };
    if let Some(body) = cat.get_child_mut("body") {
        place(body, "body_at", f.duck, f.bob * 0.6);
        let [rx, ry, rz] = rad(t.get("body_rot"));
        body.set_rotation(rx, ry, rz + f.lean.to_radians());
    }
    if let Some(head) = cat.get_child_mut("head") {
        place(head, "head_at", f.duck, f.bob);
        let [rx, ry, rz] = rad(t.get("head_rot"));
        head.set_rotation(rx + f.pitch.to_radians(), ry + f.yaw.to_radians(), rz + f.roll.to_radians());
        // The lids, each scaled down from its edge by how far it is closed.
        for names in LIDS {
            for (name, amount) in names.into_iter().zip([f.eyes.top, f.eyes.bottom, f.eyes.angry, f.eyes.worried]) {
                if let Some(lid) = head.get_child_mut(name) {
                    lid.visible = amount > 0.005;
                    lid.y_scale = amount.max(0.005);
                }
            }
        }
    }
    let kind = t.get("paw")[0] as i32;
    for (name, at, rot, of_kind, side) in [
        ("arm_r", "arm_r_at", "arm_r_rot", 0, 0),
        ("arm_l", "arm_l_at", "arm_l_rot", 0, 1),
        ("thick_r", "arm_r_at", "arm_r_rot", 1, 0),
        ("thick_l", "arm_l_at", "arm_l_rot", 1, 1),
        ("thin_r", "arm_r_at", "arm_r_rot", 2, 0),
        ("thin_l", "arm_l_at", "arm_l_rot", 2, 1),
    ] {
        let Some(arm) = cat.get_child_mut(name) else { continue };
        place(arm, at, f.paws[side], f.bob * 0.6);
        let [rx, ry, rz] = rad(t.get(rot));
        arm.set_rotation(rx + f.knead[side].to_radians(), ry, rz);
        arm.visible = of_kind == kind;
    }
}

/// Poses the eyes' model as the cat's head stands this frame, the pupils where `e` has them.
fn pose_eyes(eyes: &mut ModelPart, cat: &ModelPart, e: &EyeFrame) {
    eyes.visible = cat.visible;
    eyes.load_pose(cat.store_pose());
    let Some(pose) = cat.get_child("head").map(ModelPart::store_pose) else { return };
    let Some(head) = eyes.get_child_mut("head") else { return };
    head.load_pose(pose);
    let size = PUPIL.map(|p| p * (1.0 + e.dilate));
    // How far the pupil's middle may go from the eye's, so the pupil stays inside it.
    let reach = [(EYE_W - size[0]) * 0.5, (EYE_H - size[1]) * 0.5];
    for (side, x0) in EYE_X.iter().enumerate() {
        if let Some(pupil) = head.get_child_mut(PUPILS[side]) {
            // Eye 0's nose side is +x, eye 1's -x.
            let x = (e.pupil[0] + if side == 0 { e.cross } else { -e.cross }).clamp(-1.0, 1.0);
            pupil.set_pos(x0 + EYE_W * 0.5 + x * reach[0], EYE_TOP + EYE_H * 0.5 + e.pupil[1] * reach[1], 0.0);
            pupil.x_scale = 1.0 + e.dilate;
            pupil.y_scale = 1.0 + e.dilate;
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

/// The bag `entity` shows this frame, as of the last client tick.
fn shown(entity: u32) -> Option<Worn> {
    let mut seen = lock(&SEEN);
    if seen.len() < 256 && !seen.contains(&entity) {
        seen.push(entity);
    }
    drop(seen);
    lock(&WORN).iter().find(|(id, _)| *id == entity).map(|(_, w)| w.clone())
}

struct Layer;

impl EntityRenderLayer for Layer {
    fn submit(
        &self,
        entity: &EntityRenderState,
        parts: Option<&PosedParts>,
        draw: &mut dyn BlockEntityDrawContext,
    ) -> Result<(), String> {
        let worn = shown(entity.id);
        // The bags in this player's hands: on the arm, hanging from the hand as the arm swings.
        let held: Vec<(bool, Worn)> = lock(&HELD).iter().filter(|(id, ..)| *id == entity.id).map(|(_, off, bag)| (*off, bag.clone())).collect();
        for (off, bag) in held {
            let Some(arm) = parts.and_then(|p| p.get(if off { HumanoidPart::LeftArm } else { HumanoidPart::RightArm })) else { continue };
            let tune = Tune::current(bag.kind);
            let side = if off { -1.0 } else { 1.0 };
            let frame = if let Some(still) = held_still(bag.kind, &tune) {
                still
            } else {
                let living = entity.living.as_ref();
                let (walk, stride) = living.map_or((0.0, 0.0), |l| (l.walk_animation_speed.clamp(0.0, 1.5), l.walk_animation_pos));
                motion_of(
                    &Carrier {
                        key: entity.id ^ if off { 0xC000_0000 } else { 0x8000_0000 },
                        age: entity.age,
                        hurt: living.is_some_and(|l| l.has_red_overlay),
                        body_rot: living.map_or(0.0, |l| l.body_rot),
                        walk,
                        stride,
                        velocity: lock(&VELOCITY).iter().find(|(id, _)| *id == entity.id).map_or([0.0; 3], |(_, v)| *v),
                        swing: 0.0,
                        held: true,
                    },
                    &tune,
                )
            };
            draw.push();
            draw.scale([1.0, -1.0, -1.0]);
            draw.translate([0.0, -1.501, 0.0]);
            apply(draw, &arm.pose);
            let [x, y, z] = tune.get("arm_at");
            draw.translate([-side * x / 16.0, y / 16.0, z / 16.0]);
            let [pitch, yaw, roll] = tune.get("arm_rot");
            draw.rotate_about([0.0, 1.0, 0.0], side * yaw, [0.0; 3]);
            draw.rotate_about([1.0, 0.0, 0.0], pitch, [0.0; 3]);
            draw.rotate_about([0.0, 0.0, 1.0], side * roll, [0.0; 3]);
            let size = tune.get("arm_scale")[0].max(0.05);
            draw.scale([size; 3]);
            draw.translate(BAG_MIDDLE.map(|c| -c / 16.0));
            draw_bag(draw, &bag, &tune, &frame, false);
            draw.pop();
        }
        let Some(worn) = worn else { return Ok(()) };
        let Some(body) = parts.and_then(|p| p.get(HumanoidPart::Body)) else { return Ok(()) };
        let tune = Tune::current(worn.kind);
        draw.push();
        // The model's frame, as `LivingEntityRenderer` sets it up, then the body part's.
        draw.scale([1.0, -1.0, -1.0]);
        draw.translate([0.0, -1.501, 0.0]);
        apply(draw, &body.pose);
        // The tuning file's `hide` holds the cat that far ducked, for looking at it.
        let frame = held_still(worn.kind, &tune).unwrap_or_else(|| motion(entity, &tune));
        draw_bag(draw, &worn, &tune, &frame, true);
        draw.pop();
        Ok(())
    }
}

/// How far ducked (0 out, 1 down) the cat has drawn back from the front wall, and how far it has
/// gone down: back first, over the wall's top, and down only once clear of it, so neither its
/// head nor its paws pass through the wall. Coming out is the same way round. A cat popping up
/// (below 0) only rises.
fn duck_path(duck: f32) -> (f32, f32) {
    if duck <= 0.0 {
        return (0.0, duck);
    }
    ((duck * 3.0).min(1.0), ((duck - 0.25) / 0.75).max(0.0))
}

/// `frame` with the cat held under its lid: while the lid is over the well, the cat is at least
/// as far down as the lid's slope leaves room for, so the lid never passes through its head (a
/// lid lifted 30 degrees lets it peek; a shut one needs it nearly all the way down).
fn settled(frame: &Frame, tune: &Tune) -> Frame {
    let mut frame = *frame;
    let lift = tune.get("open")[0] * (1.0 - frame.shut.clamp(0.0, 1.0));
    if lift < 90.0 {
        // The head's top against the lid's underside above it, 3.8 texels out from the hinge.
        let down = ((2.7 - 3.8 * lift.to_radians().sin()) / DUCK).clamp(0.0, 1.0);
        if down > 0.0 {
            let duck = 0.25 + 0.75 * down;
            frame.duck = frame.duck.max(duck);
            frame.paws = frame.paws.map(|paw| paw.max(duck));
        }
    }
    frame
}

/// The frame of a bag that isn't moved by its cat: an empty one, shut; or, for looking at the
/// hiding, the tuning file's `hide` holding the cat that far ducked.
fn held_still(kind: Kind, tune: &Tune) -> Option<Frame> {
    match tune.get("hide")[0] {
        _ if kind == Kind::Empty => Some(Frame { duck: 1.0, paws: [1.0; 2], shut: 1.0, hidden: true, ..Frame::default() }),
        h if h >= 0.0 => Some(Frame { duck: h, paws: [h; 2], shut: ((h - 0.5) / 0.5).clamp(0.0, 1.0), hidden: h > 0.98, ..Frame::default() }),
        _ => None,
    }
}

/// The middle of the bag in its own frame, in texels: what a hand holds it by.
const BAG_MIDDLE: [f32; 3] = [0.0, 6.5, 4.5];

/// Draws the bag and its cat, posed for `frame`, in the frame `draw` stands in.
fn draw_bag(draw: &mut dyn BlockEntityDrawContext, worn: &Worn, tune: &Tune, frame: &Frame, on_back: bool) {
    let frame = &settled(frame, tune);
    if let Some(bag) = lock(&BAG).as_mut() {
        pose_bag(bag, tune, frame.shut);
        // Over shoulders the straps hang as worn; off a back they lie flat against the bag.
        for (name, worn_strap) in [
            ("strap_top_r", true),
            ("strap_top_l", true),
            ("strap_front_r", true),
            ("strap_front_l", true),
            ("strap_flat_r", false),
            ("strap_flat_l", false),
        ] {
            if let Some(strap) = bag.get_child_mut(name) {
                strap.visible = worn_strap == on_back;
            }
        }
        draw.model_part(bag, BAG_SPRITE, 0xFF_FFFF);
    }
    if let Some(cat) = lock(worn.kind.model()).as_mut() {
        pose(cat, tune, frame);
        draw.model_part(cat, &worn.kind.texture(&worn.variant), 0xFF_FFFF);
        if worn.kind == Kind::Cat
            && let Some(eyes) = lock(&EYES).as_mut()
        {
            pose_eyes(eyes, cat, &frame.eyes);
            draw.model_part(eyes, EYE_SPRITE, 0xFF_FFFF);
        }
    }
}

/// A hand of this window's player is about to be drawn, seen by the player: a bag in it is drawn
/// here, alive, in place of its still item model. The cat keeps to its rest, hides when its
/// holder is hurt and swats when the hand swings. True when the hand was the bag's.
pub fn render_hand(host: &dyn mod_api::ClientHost, event: &mut mod_api::RenderHandEvent<'_, '_>) -> bool {
    let Some(worn) = event.render.stack.and_then(worn_by) else { return false };
    let tune = Tune::current(worn.kind);
    let off = event.render.hand == mod_api::Hand::Off;
    let side = if off { -1.0 } else { 1.0 };
    let id = event.render.player_entity_id as u32;
    let now = CLOCK.load(Ordering::Relaxed);
    let age = now as f32 + event.render.partial;
    let frame = if let Some(still) = held_still(worn.kind, &tune) {
        still
    } else {
        let velocity = lock(&VELOCITY).iter().find(|(other, _)| *other == id).map_or([0.0; 3], |(_, v)| *v);
        let walk = (velocity[0].hypot(velocity[2]) * 5.0).clamp(0.0, 1.5);
        let hurt_at = lock(&OWN_HEALTH).1;
        motion_of(
            &Carrier {
                key: id ^ if off { 0x6000_0000 } else { 0x4000_0000 },
                age,
                hurt: hurt_at > 0 && now.saturating_sub(hurt_at) < 8,
                body_rot: host.entity(id).and_then(|entity| entity.rotation(host)).map_or(0.0, |r| r[0]),
                walk,
                stride: age * 0.6,
                velocity,
                swing: event.render.swing,
                held: true,
            },
            &tune,
        )
    };
    let draw = &mut *event.draw;
    draw.push();
    let [x, y, z] = tune.get("hand_at");
    draw.translate([side * x, y - event.render.equip * 0.6, z]);
    let [pitch, yaw, roll] = tune.get("hand_rot");
    draw.rotate_about([0.0, 1.0, 0.0], side * yaw, [0.0; 3]);
    draw.rotate_about([1.0, 0.0, 0.0], pitch, [0.0; 3]);
    draw.rotate_about([0.0, 0.0, 1.0], side * roll, [0.0; 3]);
    let size = tune.get("hand_scale")[0].max(0.05);
    draw.scale([size, -size, -size]);
    draw.translate(BAG_MIDDLE.map(|c| -c / 16.0));
    draw_bag(draw, &worn, &tune, &frame, false);
    draw.pop();
    true
}

/// Adds the layer to both player models (each resource load asks again).
pub fn register_layers(event: &mut mod_api::RegisterEntityLayersEvent<'_>) {
    let layer: Arc<dyn EntityRenderLayer> = Arc::new(Layer);
    for model in [PlayerModel::Wide, PlayerModel::Slim] {
        let _ = event.layers.add(LayerTarget::Player(model), layer.clone());
    }
}
