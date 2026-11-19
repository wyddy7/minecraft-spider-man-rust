//! The cat backpack's rules, on the world's own server: a player's own tamed cat goes into the
//! bag, rides there, grows there, and comes back out as the same cat.
//!
//! The cat in a bag is the cat itself: its whole entity data, as the game saves it, kept in the
//! stack's `custom_data` under `catpack_data`, beside what the bag shows (`catpack`: `kitten` or
//! `cat`; `catpack_cat`: the variant). Nothing of it is kept anywhere else, so the bag can be
//! dropped, put in a chest or lost with its cat, as a bundle with its items.
//!
//! - **In**: use a bag in the hand on your own tamed cat, or, wearing the bag, sneak and use the
//!   cat. A bag that holds a cat, someone else's cat and an untamed one are refused with a line
//!   over the hotbar.
//! - **Out**: sneak and use the bag on the top of a block with two clear blocks above it.
//! - **Growing**: a kitten in a bag in a player's inventory grows as it would outside (not one
//!   whose age is locked); grown, the bag shows a grown cat.
//!
//! A cat is never lost to a failure. Going in, the stack is written first and the cat removed
//! only after; coming out, the cat is put into the world first and the stack emptied only after.
//! Whatever fails in between leaves the cat in at least one place, and a cat that is in both is
//! refused its second coming out (the world already has one of its UUID).
//!
//! Known gap: the cat comes out with a new UUID. The mod API puts a mob into the world and then
//! merges its data, and a mob's UUID can't be merged; everything else (name, owner, variant,
//! collar, age, health) is the cat's own.
//!
//! Storage is the bundle's own: the bag is a bundle, so things go in and out of it in the
//! inventory as with any bundle, and stay in it with the cat.
//!
//! Not here yet: growing in chests and as a dropped item; a cell for the back slot in the
//! inventory window.

use mod_api::{
    Compound, EntityInteractEvent, HostSide, InteractionResult, ItemStack, LivingRef, PlayerRef,
    ServerHost, SpawnReason, Tag, Text, UseItemOnBlockEvent,
};

use crate::catpack::VARIANTS;

/// What a stack is to the rules: not a bag, an empty bag, or a bag holding a cat.
enum Bag {
    Empty(Compound),
    Holding(Compound),
}

/// The stack's `custom_data`, if it is a cat backpack.
fn bag(stack: &ItemStack) -> Option<Bag> {
    let id = mod_api::ComponentId::new("minecraft:custom_data")?;
    let Tag::Compound(custom) = mod_api::ItemResource::of(stack).component(&id)? else { return None };
    custom.get(&"catpack".into())?;
    Some(if custom.contains_key(&"catpack_data".into()) { Bag::Holding(custom) } else { Bag::Empty(custom) })
}

/// `stack` with this `custom_data`, drawn as `model`.
fn rewritten(stack: &ItemStack, custom: Compound, model: &str) -> Option<ItemStack> {
    let mut patch = Compound::default();
    patch.insert("minecraft:custom_data".into(), Tag::Compound(custom));
    patch.insert("minecraft:item_model".into(), Tag::String(model.into()));
    let mut next = stack.clone();
    next.apply_components(&patch).ok()?;
    Some(next)
}

/// What a cat's data shows on a bag: `kitten` or `cat`, and its variant.
fn shown(cat: &Compound) -> (&'static str, &'static str) {
    let kind = match cat.get(&"Age".into()) {
        Some(Tag::Int(age)) if *age < 0 => "kitten",
        _ => "cat",
    };
    let named = match cat.get(&"variant".into()) {
        Some(Tag::String(s)) => s.to_string_lossy(),
        _ => String::new(),
    };
    let named = named.strip_prefix("minecraft:").unwrap_or(&named);
    let variant = VARIANTS.iter().copied().find(|v| *v == named).unwrap_or("tabby");
    (kind, variant)
}

fn say(host: &mut dyn ServerHost, player: mod_api::PlayerRef, line: &str) {
    let _ = player.action_bar(host, Text::literal(line));
}

/// Where a bag that takes a cat in is: in the hand that clicked, or worn.
#[derive(Clone, Copy)]
enum Place {
    Hand(mod_api::Hand),
    Worn,
}

fn put(host: &mut dyn ServerHost, player: PlayerRef, place: Place, stack: ItemStack) -> bool {
    let living = LivingRef::new(player.entity());
    match place {
        Place::Hand(hand) => matches!(living.set_hand_item(host, hand, Some(stack)), Ok(true)),
        Place::Worn => set_back(host, player, Some(stack)),
    }
}

/// A player uses what they hold on an entity: a bag on a cat takes it in. So does a worn bag,
/// when its wearer sneaks and uses a cat with a hand that holds no bag.
pub fn entity_interact(host: HostSide, event: &mut EntityInteractEvent<'_>) -> Option<InteractionResult> {
    if event.kind != "minecraft:cat" {
        return None;
    }
    let in_hand = event.stack.filter(|s| bag(s).is_some());
    let host = match host {
        HostSide::Server(host) => host,
        // The client only says the click is taken, so it doesn't also sit the cat down.
        HostSide::Client(host) => {
            let worn = host.local_player().is_some_and(|player| {
                let entity = player.entity();
                entity.is_shift_key_down(host) == Some(true) && worn_on_backs().iter().any(|(id, _)| *id == entity.id())
            });
            return (in_hand.is_some() || worn).then_some(InteractionResult::Success);
        }
    };
    let player = host.players().acting_player()?;
    let (place, stack) = match in_hand {
        Some(stack) => (Place::Hand(event.hand), stack.clone()),
        None => {
            if player.entity().is_shift_key_down(host) != Some(true) {
                return None;
            }
            let worn = back(host, player)?;
            bag(&worn)?;
            (Place::Worn, worn)
        }
    };
    let Some(Bag::Empty(mut custom)) = bag(&stack) else {
        say(host, player, "The backpack already holds a cat");
        return Some(InteractionResult::Fail);
    };
    let cat = host.entity(event.entity as u32)?;
    let living = LivingRef::new(cat);
    let owner = player.entity().uuid(host);
    if living.is_tamed(host) != Some(true) || owner.is_none() || living.owner(host) != owner {
        say(host, player, "Only your own tamed cat goes into the backpack");
        return Some(InteractionResult::Fail);
    }
    let Some(data) = cat.nbt(host) else {
        say(host, player, "The cat can't be carried right now");
        return Some(InteractionResult::Fail);
    };
    let (kind, variant) = shown(&data);
    custom.insert("catpack".into(), Tag::String(kind.into()));
    custom.insert("catpack_cat".into(), Tag::String(variant.into()));
    custom.insert("catpack_data".into(), Tag::Compound(data));
    let Some(full) = rewritten(&stack, custom, &format!("spider_man:catpack_{kind}_{variant}")) else {
        return Some(InteractionResult::Fail);
    };
    // The bag first, the cat after: a failure leaves the cat in the world.
    if !put(host, player, place, full) {
        say(host, player, "The cat can't be carried right now");
        return Some(InteractionResult::Fail);
    }
    if !cat.discard(host) {
        // The cat stays, so the bag goes back to what it was.
        put(host, player, place, stack);
        return Some(InteractionResult::Fail);
    }
    Some(InteractionResult::SuccessServer)
}

/// A player uses what they hold on a block: sneaking, a bag with a cat lets it out there.
pub fn use_item_on_block(host: HostSide, event: &mut UseItemOnBlockEvent<'_>) -> Option<InteractionResult> {
    let click = event.click;
    let stack = click.stack?;
    let Bag::Holding(mut custom) = bag(stack)? else { return None };
    let HostSide::Server(host) = host else { return None };
    // Once a click: before the block has its say.
    if event.phase != mod_api::UsePhase::ItemBeforeBlock {
        return None;
    }
    let player = match host.players().acting_player() {
        Some(player) => player,
        None => host.players().server_players().into_iter().find(|p| p.uuid() == click.player)?,
    };
    if player.entity().is_shift_key_down(host) != Some(true) {
        return None;
    }
    let Some(Tag::Compound(mut cat)) = custom.get(&"catpack_data".into()).cloned() else { return None };
    let level = player.entity().level(host)?;
    let face = click.hit.face;
    let floor = click.pos;
    let at = [floor[0] + face[0], floor[1] + face[1], floor[2] + face[2]];
    let above = [at[0], at[1] + 1, at[2]];
    let clear = |host: &dyn ServerHost, pos: [i32; 3]| level.block(host, pos).is_some_and(|b| b.id(host) == "minecraft:air");
    if face != [0, 1, 0] || clear(host, floor) || !clear(host, at) || !clear(host, above) {
        say(host, player, "Let the cat out on top of a block with room above it");
        return Some(InteractionResult::Fail);
    }
    // A cat already in the world (a failure once left it in both places) is not let out twice.
    if let Some(Tag::IntArray(uuid)) = cat.get(&"UUID".into())
        && let [a, b, c, d] = uuid[..]
    {
        let uuid = [a, b, c, d].iter().fold(0u128, |acc, part| (acc << 32) | u128::from(*part as u32));
        if host.entity_by_uuid(mod_api::EntityUuid(uuid)).is_some() {
            say(host, player, "This cat is already out in the world");
            return Some(InteractionResult::Fail);
        }
    }
    let place = [f64::from(at[0]) + 0.5, f64::from(at[1]), f64::from(at[2]) + 0.5];
    let Some(spawned) = level.spawn_mob(host, "minecraft:cat", place, SpawnReason::Bucket) else {
        say(host, player, "The cat can't come out here");
        return Some(InteractionResult::Fail);
    };
    // The same cat, where it was let out.
    for gone in ["Pos", "Motion", "Rotation"] {
        cat.remove(&gone.into());
    }
    if !spawned.merge_nbt(host, cat) {
        spawned.discard(host);
        say(host, player, "The cat can't come out here");
        return Some(InteractionResult::Fail);
    }
    // The cat first, the bag after: a failure leaves the cat in the bag as well, and it is
    // refused a second coming out above.
    custom.remove(&"catpack_data".into());
    custom.remove(&"catpack_cat".into());
    custom.insert("catpack".into(), Tag::String("empty".into()));
    if let Some(empty) = rewritten(stack, custom, "spider_man:catpack") {
        let _ = LivingRef::new(player.entity()).set_hand_item(host, click.hand, Some(empty));
    }
    Some(InteractionResult::SuccessServer)
}

/// `stack` a second later, if it is a bag whose kitten grew in that second.
fn grown(stack: &ItemStack) -> Option<ItemStack> {
    let Bag::Holding(mut custom) = bag(stack)? else { return None };
    let Some(Tag::Compound(cat)) = custom.get_mut(&"catpack_data".into()) else { return None };
    if matches!(cat.get(&"AgeLocked".into()), Some(Tag::Byte(n)) if *n != 0) {
        return None;
    }
    let Some(Tag::Int(age)) = cat.get_mut(&"Age".into()) else { return None };
    if *age >= 0 {
        return None;
    }
    *age = (*age + 20).min(0);
    let (kind, variant) = shown(cat);
    custom.insert("catpack".into(), Tag::String(kind.into()));
    rewritten(stack, custom, &format!("spider_man:catpack_{kind}_{variant}"))
}

/// Once a second for each player: the kittens in the bags they carry grow by that second.
pub fn grow(host: &mut dyn ServerHost, player: PlayerRef) {
    let Some(slots) = player.inventory_slots(host) else { return };
    for (slot, stack) in slots {
        if let Some(next) = grown(&stack) {
            let _ = player.set_inventory_slot(host, slot, Some(next));
        }
    }
}

/// After each level's tick, on the server: once a second, the kittens its players carry grow.
pub fn level_tick(host: HostSide, event: &mut mod_api::LevelTickPostEvent) {
    let HostSide::Server(host) = host else { return };
    let level = event.level;
    backs_tick(host, level);
    let mut ticks = TICKS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if ticks.len() > 64 {
        ticks.clear();
    }
    let i = ticks.iter().position(|(l, _)| *l == level).unwrap_or_else(|| {
        ticks.push((level, 0));
        ticks.len() - 1
    });
    ticks[i].1 += 1;
    if ticks[i].1 < 20 {
        return;
    }
    ticks[i].1 = 0;
    drop(ticks);
    for player in level.online_players(host).unwrap_or_default() {
        grow(host, player);
        if let Some(stack) = back(host, player)
            && let Some(next) = grown(&stack)
        {
            set_back(host, player, Some(next));
        }
    }
}

/// Each level's ticks since its kittens last grew.
static TICKS: std::sync::Mutex<Vec<(mod_api::LevelRef, u8)>> = std::sync::Mutex::new(Vec::new());

// ---- The back slot ----
//
// A bag is worn on the back, in a slot of the mod's own: the chest slot stays free for a
// chestplate or an elytra. The game has no such slot, so the mod keeps it: the stack lives in its
// wearer's persistent data (`spider_man_back`), which the game saves with the player.
//
// - **On**: use a bag in the hand (not on a cat, not sneaking). A bag already on the back comes
//   into the hand in its place.
// - **Off**: sneak and use an empty hand on nothing. The bag goes into the inventory; with no
//   room there it stays on the back.
// - **Death**: the bag drops where its wearer died, with all that is in it, unless the world
//   keeps inventories.
//
// Known gap: the slot is the world owner's process's own. A guest of a LAN world wears a bag and
// the host sees it, but the guest's own game is not told (nothing is sent), and a dedicated
// server would need the mod on it.

/// Where a player's persistent data keeps the bag on their back.
const BACK: &str = "spider_man_back";

/// Each player's back, as last read or written: their UUID, their entity id, the bag.
static BACKS: std::sync::Mutex<Vec<(u128, u32, Option<ItemStack>)>> = std::sync::Mutex::new(Vec::new());
/// Players (by UUID) who asked to take their bag off, from their own client: handled on the
/// server's next tick. Reaches the server only in the asker's own world.
static TAKE_OFF: std::sync::Mutex<Vec<u128>> = std::sync::Mutex::new(Vec::new());

fn backs() -> std::sync::MutexGuard<'static, Vec<(u128, u32, Option<ItemStack>)>> {
    BACKS.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The bags on backs, by wearer's entity id, for the client of the same process to draw.
pub fn worn_on_backs() -> Vec<(u32, ItemStack)> {
    backs().iter().filter_map(|(_, id, stack)| Some((*id, stack.clone()?))).collect()
}

/// The bag on `player`'s back.
fn back(host: &mut dyn ServerHost, player: PlayerRef) -> Option<ItemStack> {
    let (uuid, id) = (player.uuid(), player.entity().id());
    if let Some(known) = backs().iter_mut().find(|(u, ..)| *u == uuid) {
        known.1 = id;
        return known.2.clone();
    }
    let stack = match player.persistent_data(host).and_then(|data| data.get(&BACK.into()).cloned()) {
        Some(Tag::Compound(saved)) => ItemStack::from_nbt(host, &saved),
        _ => None,
    };
    backs().push((uuid, id, stack.clone()));
    stack
}

/// Puts `stack` (or nothing) on `player`'s back: saved first, then shown.
fn set_back(host: &mut dyn ServerHost, player: PlayerRef, stack: Option<ItemStack>) -> bool {
    let mut data = player.persistent_data(host).unwrap_or_default();
    match stack.as_ref().map(|s| s.to_nbt(host)) {
        Some(Some(saved)) => {
            data.insert(BACK.into(), Tag::Compound(saved));
        }
        Some(None) => return false,
        None => {
            data.remove(&BACK.into());
        }
    }
    if player.set_persistent_data(host, data).is_err() {
        return false;
    }
    let (uuid, id) = (player.uuid(), player.entity().id());
    let mut all = backs();
    match all.iter_mut().find(|(u, ..)| *u == uuid) {
        Some(known) => *known = (uuid, id, stack),
        None => all.push((uuid, id, stack)),
    }
    true
}

/// A player uses the item in their hand on nothing: a bag goes onto the back. Its use as the
/// bundle it is made of (throwing out what it holds) is always stopped.
pub fn right_click_item(host: HostSide, event: &mut mod_api::RightClickItemEvent<'_>) -> bool {
    let click = event.click;
    if bag(click.stack).is_none() {
        return false;
    }
    let HostSide::Server(host) = host else { return true };
    let Some(player) = host.players().server_players().into_iter().find(|p| p.uuid() == click.player) else { return true };
    if player.entity().is_shift_key_down(host) == Some(true) {
        return true;
    }
    // The back first, the hand after: a failure leaves the bag in both places at worst, never in
    // neither.
    let before = back(host, player);
    if !set_back(host, player, Some(click.stack.clone())) {
        say(host, player, "The backpack can't be put on right now");
        return true;
    }
    if !matches!(LivingRef::new(player.entity()).set_hand_item(host, click.hand, before.clone()), Ok(true)) {
        set_back(host, player, before);
    }
    true
}

/// The client's own player sneaks and uses an empty hand on nothing: they ask for their bag.
pub fn right_click_empty(host: &dyn mod_api::ClientHost) {
    let Some(player) = host.local_player() else { return };
    if player.entity().is_shift_key_down(host) == Some(true) {
        let mut asked = TAKE_OFF.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if asked.len() < 64 {
            asked.push(player.uuid());
        }
    }
}

/// Each server tick of a level: bags asked off come off, and every player's back is known.
fn backs_tick(host: &mut dyn ServerHost, level: mod_api::LevelRef) {
    let asked = std::mem::take(&mut *TAKE_OFF.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
    for player in level.online_players(host).unwrap_or_default() {
        let worn = back(host, player);
        if !asked.contains(&player.uuid()) {
            continue;
        }
        let Some(stack) = worn else { continue };
        // Into the inventory first, off the back after.
        if player.give_stack(host, stack.clone()).is_err() {
            say(host, player, "No room to take the backpack off");
            continue;
        }
        set_back(host, player, None);
    }
}

/// A player dies: the bag on their back drops there, unless the world keeps inventories.
pub fn living_death(host: &mut dyn ServerHost, event: &mut mod_api::LivingDeathEvent<'_>) {
    if event.kind != "minecraft:player" {
        return;
    }
    let Some(entity) = host.entity(event.entity) else { return };
    let Some(player) = host.players().server_players().into_iter().find(|p| p.entity().id() == event.entity) else { return };
    let Some(stack) = back(host, player) else { return };
    let Some(level) = entity.level(host) else { return };
    if matches!(level.game_rule(host, "minecraft:keep_inventory"), Ok(Some(mod_api::GameRuleValue::Bool(true))))
        || matches!(level.game_rule(host, "keepInventory"), Ok(Some(mod_api::GameRuleValue::Bool(true))))
    {
        return;
    }
    let Some([x, y, z]) = entity.position(host) else { return };
    // Dropped first, off the back after: a failure leaves the bag on the back for the respawn.
    if matches!(level.spawn_item(host, x, y + 0.5, z, stack, 40), Ok(Some(_))) {
        set_back(host, player, None);
    }
}
