//! The cat backpack's rules, on the world's own server: a player's own tamed cat goes into the
//! bag, rides there, grows there, and comes back out as the same cat.
//!
//! The cat in a bag is the cat itself: its whole entity data, as the game saves it, kept in the
//! stack's `custom_data` under `catpack_data`, beside what the bag shows (`catpack`: `kitten` or
//! `cat`; `catpack_cat`: the variant). Nothing of it is kept anywhere else, so the bag can be
//! dropped, put in a chest or lost with its cat, as a bundle with its items.
//!
//! - **In**: use a bag in the hand on your own tamed cat. A bag that holds a cat, someone else's
//!   cat and an untamed one are refused with a line over the hotbar.
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
//! Not here yet: a bag worn on the back taking a cat in, a storage window, other players seeing
//! the cat, growing in chests and as a dropped item.

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

/// A player uses what they hold on an entity: a bag on a cat takes it in.
pub fn entity_interact(host: HostSide, event: &mut EntityInteractEvent<'_>) -> Option<InteractionResult> {
    let stack = event.stack?;
    let held = bag(stack)?;
    if event.kind != "minecraft:cat" {
        return None;
    }
    // The client only says the click is taken, so it doesn't also sit the cat down.
    let HostSide::Server(host) = host else { return Some(InteractionResult::Success) };
    let player = host.players().acting_player()?;
    let Bag::Empty(mut custom) = held else {
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
    let Some(full) = rewritten(stack, custom, &format!("spider_man:catpack_{kind}_{variant}")) else {
        return Some(InteractionResult::Fail);
    };
    // The bag first, the cat after: a failure leaves the cat in the world.
    let hand = event.hand;
    if !matches!(LivingRef::new(player.entity()).set_hand_item(host, hand, Some(full)), Ok(true)) {
        say(host, player, "The cat can't be carried right now");
        return Some(InteractionResult::Fail);
    }
    if !cat.discard(host) {
        // The cat stays, so the bag goes back to what it was.
        let _ = LivingRef::new(player.entity()).set_hand_item(host, hand, Some(stack.clone()));
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

/// Once a second for each player: the kittens in the bags they carry grow by that second.
pub fn grow(host: &mut dyn ServerHost, player: PlayerRef) {
    let Some(slots) = player.inventory_slots(host) else { return };
    for (slot, stack) in slots {
        let Some(Bag::Holding(mut custom)) = bag(&stack) else { continue };
        let Some(Tag::Compound(cat)) = custom.get_mut(&"catpack_data".into()) else { continue };
        if matches!(cat.get(&"AgeLocked".into()), Some(Tag::Byte(n)) if *n != 0) {
            continue;
        }
        let Some(Tag::Int(age)) = cat.get_mut(&"Age".into()) else { continue };
        if *age >= 0 {
            continue;
        }
        *age = (*age + 20).min(0);
        let (kind, variant) = shown(cat);
        custom.insert("catpack".into(), Tag::String(kind.into()));
        let Some(next) = rewritten(&stack, custom, &format!("spider_man:catpack_{kind}_{variant}")) else { continue };
        let _ = player.set_inventory_slot(host, slot, Some(next));
    }
}

/// After each level's tick, on the server: once a second, the kittens its players carry grow.
pub fn level_tick(host: HostSide, event: &mut mod_api::LevelTickPostEvent) {
    let HostSide::Server(host) = host else { return };
    let level = event.level;
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
    }
}

/// Each level's ticks since its kittens last grew.
static TICKS: std::sync::Mutex<Vec<(mod_api::LevelRef, u8)>> = std::sync::Mutex::new(Vec::new());
