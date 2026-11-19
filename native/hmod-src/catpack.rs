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
//! Every player's bag is seen: a worn stack is sent to all who see its wearer.
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

/// `/catpack` alone: this bag on every player in view.
static FORCED: AtomicBool = AtomicBool::new(false);
static FORCED_WORN: Mutex<Option<Worn>> = Mutex::new(None);
/// Each drawn player's entity id and the bag in their chest slot, as of the last client tick: a
/// worn stack is sent to everyone who sees its wearer, so other players' bags show too.
static WORN: Mutex<Vec<(u32, Worn)>> = Mutex::new(Vec::new());
/// The players whose bag was asked for since the last client tick.
static SEEN: Mutex<Vec<u32>> = Mutex::new(Vec::new());
/// The two cats and the bag as built once at client start; posed again each frame.
static KITTEN: Mutex<Option<ModelPart>> = Mutex::new(None);
static CAT: Mutex<Option<ModelPart>> = Mutex::new(None);
static BAG: Mutex<Option<ModelPart>> = Mutex::new(None);

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The bag's item id.
pub const ITEM: &str = "spider_man:cat_backpack";

/// The bag's item as this run of the game knows it: learnt from the first stack of it seen with
/// a host at hand (an item's id is the host's to say), so that a stack can be told a bag
/// without one.
static BAG_ITEM: std::sync::OnceLock<mod_api::ItemKey> = std::sync::OnceLock::new();

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

/// The test bags `/catpack give` hands out, drawn as the bag
/// (`native/tools/catpack_item_models.py` makes the item models): with a kitten, with a grown
/// cat, empty. A cat given this way is only a picture: it has no cat's data to let out.
pub const GIVE: [&str; 3] = [
    r#"/give @s spider_man:cat_backpack[custom_data={catpack:"kitten",catpack_cat:"red"},item_model="spider_man:catpack_kitten_red"]"#,
    r#"/give @s spider_man:cat_backpack[custom_data={catpack:"cat",catpack_cat:"red"},item_model="spider_man:catpack_cat_red"]"#,
    r#"/give @s spider_man:cat_backpack"#,
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
        return Command::Say(format!("Back catpack on every player: {}", if on { "on" } else { "off" }));
    };
    if first == "give" {
        let lines = GIVE.iter().map(|s| (*s).to_owned()).collect();
        return Command::Send(lines, "Three test bags: use one to put it on your back".into());
    }
    if let Some(kind) = Kind::parse(first) {
        let variant = words.next().filter(|v| VARIANTS.contains(v)).unwrap_or("red").to_owned();
        let line = format!("Back catpack on every player: {first}, {variant}");
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
    let kind = match text("catpack") {
        Some(named) => Kind::parse(&named)?,
        // The mod's own item, as it is made: no data of its own yet.
        None if is_bag_item(stack) => Kind::Empty,
        None => return None,
    };
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
    // How lively the cat is at rest: 1 as made, 0 still (for shots).
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
    [1.0, 0.0, 0.0],
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
    [1.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
    [-1.0, 0.0, 0.0],
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

/// The lid's turn, in degrees, as it goes from hanging out in front (`shut` 0, the pose's angle)
/// up and over to lying on the well (`shut` 1).
fn pose_bag(bag: &mut ModelPart, t: &Tune, shut: f32) {
    if let Some(lid) = bag.get_child_mut("lid") {
        let open = t.get("open")[0];
        lid.x_rot = (open + (270.0 - open) * shut).to_radians();
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
fn motion(entity: &EntityRenderState, t: &Tune) -> Frame {
    let [amount, _, _] = t.get("idle");
    let age = entity.age;
    let living = entity.living.as_ref();
    let hurt = living.is_some_and(|l| l.has_red_overlay);
    let body_rot = living.map_or(0.0, |l| l.body_rot);
    let mut all = lock(&MOTION);
    // Carriers not drawn for a while are forgotten (their age stops while they aren't).
    if all.len() > 64 {
        all.clear();
    }
    let i = all.iter().position(|m| m.entity == entity.id).unwrap_or_else(|| {
        all.push(Motion {
            entity: entity.id,
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
    let (walk, stride) = living.map_or((0.0, 0.0), |l| (l.walk_animation_speed.clamp(0.0, 1.5), l.walk_animation_pos));
    let velocity = lock(&VELOCITY).iter().find(|(id, _)| *id == entity.id).map_or([0.0; 3], |(_, v)| *v);
    let rise = velocity[1].clamp(-1.5, 1.0);
    let speed = velocity[0].hypot(velocity[2]).min(2.0);
    let rush = ((speed - 0.35) / 0.6).clamp(0.0, 1.0);

    // Where the hiding wants the cat and the lid.
    let (duck_to, shut_to, hiding) = if since < FLINCH {
        (-0.18, 0.0, true)
    } else if since < PEEK_AT {
        (1.0, if m.duck.x > 0.7 { 1.0 } else { 0.0 }, true)
    } else if since < OUT_AT {
        (0.42, 0.74, true)
    } else {
        (0.25 * rush, 0.0, false)
    };

    // Where the head wants to look: every three seconds a new place, held; under the lid, out
    // one way and then the other.
    let slot = ((age + (entity.id % 97) as f32 * 7.0) / 60.0).floor() as i32;
    let mut look = [(chance(entity.id, slot, 1) - 0.5) * 70.0, (chance(entity.id, slot, 2) - 0.6) * 22.0];
    if chance(entity.id, slot, 3) < 0.25 {
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
        let into = (age + (entity.id % 97) as f32 * 7.0).rem_euclid(60.0);
        if into < dt && chance(entity.id, slot, 4) < 0.4 && !hiding {
            m.roll.v += 4.0 * amount * if chance(entity.id, slot, 5) < 0.5 { 1.0 } else { -1.0 };
        }
    }
    // Kneading: a few seconds now and then, the paws pressing in turn.
    let kneading = if !hiding && chance(entity.id, slot, 6) < 0.3 { amount * (1.0 - rush) } else { 0.0 };
    let press = (age * 0.45).sin();
    Frame {
        duck: m.duck.x,
        paws: [m.paws[0].x, m.paws[1].x],
        shut: m.lid.x.max(-0.08),
        yaw: m.yaw.x.clamp(-75.0, 75.0),
        pitch: m.pitch.x.clamp(-35.0, 35.0),
        roll: m.roll.x.clamp(-25.0, 25.0),
        lean: m.lean.x.clamp(-15.0, 15.0),
        bob: m.bob.x.clamp(-1.2, 1.6),
        knead: [9.0 * kneading * press.max(0.0), 9.0 * kneading * (-press).max(0.0)],
        hidden: m.duck.x > 0.9 && m.lid.x > 0.93,
    }
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
    // Ducking, a part also draws back from the front wall, so nothing of it shows through.
    let place = |part: &mut ModelPart, key: &str, duck: f32, bob: f32| {
        let [x, y, z] = t.get(key);
        let back = duck.clamp(0.0, 1.0);
        part.set_pos(-x / scale, (y + DUCK * duck + bob) / scale, -(z - DRAW_BACK * back) / scale);
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
        let Some(worn) = shown(entity.id) else { return Ok(()) };
        let Some(body) = parts.and_then(|p| p.get(HumanoidPart::Body)) else { return Ok(()) };
        let tune = Tune::current(worn.kind);
        draw.push();
        // The model's frame, as `LivingEntityRenderer` sets it up, then the body part's.
        draw.scale([1.0, -1.0, -1.0]);
        draw.translate([0.0, -1.501, 0.0]);
        apply(draw, &body.pose);
        // The tuning file's `hide` holds the cat that far ducked, for looking at it.
        let frame = match tune.get("hide")[0] {
            _ if worn.kind == Kind::Empty => Frame { duck: 1.0, paws: [1.0; 2], shut: 1.0, hidden: true, ..Frame::default() },
            h if h >= 0.0 => Frame { duck: h, paws: [h; 2], shut: ((h - 0.5) / 0.5).clamp(0.0, 1.0), hidden: h > 0.98, ..Frame::default() },
            _ => motion(entity, &tune),
        };
        if let Some(bag) = lock(&BAG).as_mut() {
            pose_bag(bag, &tune, frame.shut);
            draw.model_part(bag, BAG_SPRITE, 0xFF_FFFF);
        }
        if let Some(cat) = lock(worn.kind.model()).as_mut() {
            pose(cat, &tune, &frame);
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
