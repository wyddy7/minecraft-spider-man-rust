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
//! The bag is worn in a back slot of the mod's own and holds 27 stacks (both below).
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

/// What the stack is to the rules, if it is a cat backpack: one that says so in its
/// `custom_data`, or the mod's own item as it is made (no data yet: empty).
fn bag(stack: &ItemStack) -> Option<Bag> {
    let read = |name: &str| mod_api::ItemResource::of(stack).component(&mod_api::ComponentId::new(name)?);
    let custom = match read("minecraft:custom_data") {
        Some(Tag::Compound(custom)) if custom.contains_key(&"catpack".into()) => custom,
        other => {
            if !crate::catpack::is_bag_item(stack) {
                return None;
            }
            let mut custom = match other {
                Some(Tag::Compound(custom)) => custom,
                _ => Compound::default(),
            };
            custom.insert("catpack".into(), Tag::String("empty".into()));
            custom
        }
    };
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
                entity.is_shift_key_down(host) == Some(true) && crate::catpack_ui::wears(entity.id())
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
    // Once a click: before the block has its say.
    if event.phase != mod_api::UsePhase::ItemBeforeBlock {
        return None;
    }
    let host = match host {
        HostSide::Server(host) => host,
        // The client says the click is taken where the server will let the cat out, so it
        // doesn't go on to use the item as well (which would put the emptied bag on the back).
        HostSide::Client(host) => {
            let sneaking = host.local_player().is_some_and(|p| p.entity().is_shift_key_down(host) == Some(true));
            return (sneaking && click.hit.face == [0, 1, 0]).then_some(InteractionResult::Success);
        }
    };
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
    // Any other face is not asking for the cat: the click goes on (and puts the bag on).
    if face != [0, 1, 0] {
        return None;
    }
    if clear(host, floor) || !clear(host, at) || !clear(host, above) {
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
    if !crate::catpack::learnt() {
        for player in level.online_players(host).unwrap_or_default() {
            for (_, stack) in player.inventory_slots(host).unwrap_or_default() {
                crate::catpack::learn(host, &stack);
            }
        }
    }
    let mut ticks = TICKS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if ticks.len() > 64 {
        ticks.clear();
    }
    let i = ticks.iter().position(|(l, _)| *l == level).unwrap_or_else(|| {
        ticks.push((level, 0));
        ticks.len() - 1
    });
    ticks[i].1 += 1;
    let second = ticks[i].1 >= 20;
    let refresh = ticks[i].1 % 5 == 0;
    if second {
        ticks[i].1 = 0;
    }
    drop(ticks);
    if refresh {
        backs_tick(host, level, second);
    }
    if !second {
        return;
    }
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
// chestplate or an elytra. The game has no such slot, so the server keeps it: the stack lives in
// its wearer's persistent data (`spider_man_back`), which the game saves with the player, and
// everyone in the wearer's level is told what it shows (`catpack_net`).
//
// - **On**: sneak and use a bag in the hand (anywhere but on the top of a block when the bag
//   holds a cat: that lets the cat out). A bag already on the back comes into the hand.
// - **Off**: sneak and use an empty hand on nothing. The bag goes into the inventory; with no
//   room there it stays on the back.
// - **Death**: the bag drops where its wearer died, with all that is in it, unless the world
//   keeps inventories.

/// Where a player's persistent data keeps the bag on their back.
const BACK: &str = "spider_man_back";

/// Each player's back, as last read or written: their UUID and the bag.
static BACKS: std::sync::Mutex<Vec<(u128, Option<ItemStack>)>> = std::sync::Mutex::new(Vec::new());

fn backs() -> std::sync::MutexGuard<'static, Vec<(u128, Option<ItemStack>)>> {
    BACKS.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The bag on `player`'s back.
fn back(host: &mut dyn ServerHost, player: PlayerRef) -> Option<ItemStack> {
    let uuid = player.uuid();
    if let Some((_, stack)) = backs().iter().find(|(u, _)| *u == uuid) {
        return stack.clone();
    }
    let stack = match player.persistent_data(host).and_then(|data| data.get(&BACK.into()).cloned()) {
        Some(Tag::Compound(saved)) => ItemStack::from_nbt(host, &saved),
        _ => None,
    };
    backs().push((uuid, stack.clone()));
    stack
}

/// Puts `stack` (or nothing) on `player`'s back: saved first, then told to all.
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
    let uuid = player.uuid();
    {
        let mut all = backs();
        match all.iter_mut().find(|(u, _)| *u == uuid) {
            Some(known) => known.1 = stack,
            None => all.push((uuid, stack)),
        }
    }
    announce(host, player);
    true
}

/// Tells everyone in `wearer`'s level what is on their back.
fn announce(host: &mut dyn ServerHost, wearer: PlayerRef) {
    let Some(level) = wearer.entity().level(host) else { return };
    let news = back_news(host, wearer);
    for player in level.online_players(host).unwrap_or_default() {
        let news = if player == wearer { own_news(host, wearer, news.clone()) } else { news.clone() };
        crate::catpack_net::tell(host, player, news);
    }
}

/// `news` of `wearer`'s back for the wearer's own game: with the bag itself, for their
/// inventory's back slot.
fn own_news(host: &mut dyn ServerHost, wearer: PlayerRef, mut news: Compound) -> Compound {
    if let Some(saved) = back(host, wearer).and_then(|stack| stack.to_nbt(host)) {
        news.insert("i".into(), Tag::Compound(saved));
    }
    news
}

/// The `back` message for `wearer`.
fn back_news(host: &mut dyn ServerHost, wearer: PlayerRef) -> Compound {
    let mut fields = vec![("e", Tag::Int(wearer.entity().id() as i32))];
    if let Some(custom) = back(host, wearer).as_ref().and_then(bag) {
        let custom = match custom {
            Bag::Empty(c) | Bag::Holding(c) => c,
        };
        for (field, key) in [("kind", "catpack"), ("variant", "catpack_cat")] {
            if let Some(value) = custom.get(&key.into()) {
                fields.push((field, value.clone()));
            }
        }
    }
    crate::catpack_net::message("back", fields)
}

/// A player uses the item in their hand on nothing. A bag: sneaking, it goes onto the back;
/// else its storage opens.
pub fn right_click_item(host: HostSide, event: &mut mod_api::RightClickItemEvent<'_>) -> bool {
    let click = event.click;
    if bag(click.stack).is_none() {
        return false;
    }
    let HostSide::Server(host) = host else { return true };
    let Some(player) = host.players().server_players().into_iter().find(|p| p.uuid() == click.player) else { return true };
    if player.entity().is_shift_key_down(host) != Some(true) {
        open(host, player, Source::Hand(click.hand));
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

/// The bag comes off `player`'s back into their inventory.
fn take_off(host: &mut dyn ServerHost, player: PlayerRef) {
    let Some(stack) = back(host, player) else { return };
    close(host, player);
    // Into the inventory first, off the back after.
    if player.give_stack(host, stack).is_err() {
        say(host, player, "No room to take the backpack off");
        return;
    }
    set_back(host, player, None);
}

/// Each server tick of a level: every couple of seconds everyone is told every back again (a
/// player who joined or came near catches up), and open windows are kept true.
fn backs_tick(host: &mut dyn ServerHost, level: mod_api::LevelRef, second: bool) {
    let players = level.online_players(host).unwrap_or_default();
    for player in &players {
        if windows().iter().any(|w| w.player == player.uuid()) {
            show(host, *player);
        }
    }
    if !second {
        return;
    }
    let news: Vec<(PlayerRef, Compound)> = players.iter().map(|p| (*p, back_news(host, *p))).collect();
    for player in &players {
        for (wearer, one) in &news {
            let one = if wearer == player { own_news(host, *wearer, one.clone()) } else { one.clone() };
            crate::catpack_net::tell(host, *player, one);
        }
    }
}

/// A player dies: the bag on their back drops there, unless the world keeps inventories.
pub fn living_death(host: &mut dyn ServerHost, event: &mut mod_api::LivingDeathEvent<'_>) {
    if event.kind != "minecraft:player" {
        return;
    }
    let Some(entity) = host.entity(event.entity) else { return };
    let Some(player) = host.players().server_players().into_iter().find(|p| p.entity().id() == event.entity) else { return };
    close(host, player);
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

// ---- Storage ----
//
// A bag holds 27 stacks, as a chest does, in its own `custom_data` (`catpack_items`: a list of
// a slot `s` and a stack `i`), so they travel with the bag and its cat. Its window is the mod's
// own (`catpack_ui`): the server sends what the bag and the player's inventory hold, the client
// asks to move a whole stack one way or the other, and the server does it and sends again.

/// How many stacks a bag holds.
pub const SLOTS: usize = 27;

/// Where the bag of an open window is.
#[derive(Clone, Copy, PartialEq)]
enum Source {
    Hand(mod_api::Hand),
    Back,
}

/// A player's open window: whose, on which bag, and that bag as the server last wrote it (a bag
/// that is no longer that one closes the window).
struct Window {
    player: u128,
    source: Source,
    bag: ItemStack,
}

static WINDOWS: std::sync::Mutex<Vec<Window>> = std::sync::Mutex::new(Vec::new());

fn windows() -> std::sync::MutexGuard<'static, Vec<Window>> {
    WINDOWS.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn source_stack(host: &mut dyn ServerHost, player: PlayerRef, source: Source) -> Option<ItemStack> {
    match source {
        Source::Hand(hand) => {
            let slot = if hand == mod_api::Hand::Main { mod_api::EquipmentSlot::MainHand } else { mod_api::EquipmentSlot::OffHand };
            LivingRef::new(player.entity()).equipment(host, slot)
        }
        Source::Back => back(host, player),
    }
}

fn write_source(host: &mut dyn ServerHost, player: PlayerRef, source: Source, stack: ItemStack) -> bool {
    match source {
        Source::Hand(hand) => matches!(LivingRef::new(player.entity()).set_hand_item(host, hand, Some(stack)), Ok(true)),
        Source::Back => set_back(host, player, Some(stack)),
    }
}

/// What a bag holds, slot by slot.
fn items(host: &dyn ServerHost, stack: &ItemStack) -> Vec<Option<ItemStack>> {
    let mut slots = vec![None; SLOTS];
    let custom = match bag(stack) {
        Some(Bag::Empty(c) | Bag::Holding(c)) => c,
        None => return slots,
    };
    let Some(Tag::List(list)) = custom.get(&"catpack_items".into()) else { return slots };
    for entry in list {
        let Tag::Compound(entry) = entry else { continue };
        let (Some(Tag::Byte(slot)), Some(Tag::Compound(saved))) = (entry.get(&"s".into()), entry.get(&"i".into())) else { continue };
        if let Some(place) = slots.get_mut(*slot as usize) {
            *place = ItemStack::from_nbt(host, saved);
        }
    }
    slots
}

/// `stack` holding `slots`.
fn with_items(host: &dyn ServerHost, stack: &ItemStack, slots: &[Option<ItemStack>]) -> Option<ItemStack> {
    let mut custom = match bag(stack)? {
        Bag::Empty(c) | Bag::Holding(c) => c,
    };
    let mut list = Vec::new();
    for (slot, held) in slots.iter().enumerate() {
        let Some(held) = held else { continue };
        let mut entry = Compound::default();
        entry.insert("s".into(), Tag::Byte(slot as i8));
        entry.insert("i".into(), Tag::Compound(held.to_nbt(host)?));
        list.push(Tag::Compound(entry));
    }
    if list.is_empty() {
        custom.remove(&"catpack_items".into());
    } else {
        custom.insert("catpack_items".into(), Tag::List(list));
    }
    let mut patch = Compound::default();
    patch.insert("minecraft:custom_data".into(), Tag::Compound(custom));
    let mut next = stack.clone();
    next.apply_components(&patch).ok()?;
    Some(next)
}

/// Opens `player`'s window on the bag at `source`.
fn open(host: &mut dyn ServerHost, player: PlayerRef, source: Source) {
    let Some(stack) = source_stack(host, player, source).filter(|s| bag(s).is_some()) else { return };
    let uuid = player.uuid();
    {
        let mut all = windows();
        all.retain(|w| w.player != uuid);
        all.push(Window { player: uuid, source, bag: stack });
    }
    show(host, player);
}

/// Closes `player`'s window, if one is open, and tells their game.
fn close(host: &mut dyn ServerHost, player: PlayerRef) {
    let uuid = player.uuid();
    let had = {
        let mut all = windows();
        let before = all.len();
        all.retain(|w| w.player != uuid);
        all.len() != before
    };
    if had {
        crate::catpack_net::tell(host, player, crate::catpack_net::message("window", [("open", Tag::Byte(0))]));
    }
}

/// `player`'s open window and its bag as it is now: none (and the window closed) when the bag is
/// no longer where it was or no longer the same bag.
fn open_bag(host: &mut dyn ServerHost, player: PlayerRef) -> Option<(Source, ItemStack)> {
    let uuid = player.uuid();
    let (source, known) = {
        let all = windows();
        let w = all.iter().find(|w| w.player == uuid)?;
        (w.source, w.bag.clone())
    };
    match source_stack(host, player, source) {
        Some(now) if now == known => Some((source, now)),
        _ => {
            close(host, player);
            None
        }
    }
}

fn stacks(host: &dyn ServerHost, list: impl IntoIterator<Item = (usize, ItemStack)>) -> Tag {
    let mut out = Vec::new();
    for (slot, stack) in list {
        let Some(saved) = stack.to_nbt(host) else { continue };
        let mut entry = Compound::default();
        entry.insert("s".into(), Tag::Int(slot as i32));
        entry.insert("i".into(), Tag::Compound(saved));
        out.push(Tag::Compound(entry));
    }
    Tag::List(out)
}

/// Sends `player` their open window: what the bag and their inventory hold.
fn show(host: &mut dyn ServerHost, player: PlayerRef) {
    let Some((_, stack)) = open_bag(host, player) else { return };
    let held = items(host, &stack).into_iter().enumerate().filter_map(|(slot, s)| Some((slot, s?)));
    let held = stacks(host, held);
    let carried = player.inventory_slots(host).unwrap_or_default().into_iter().filter(|(slot, _)| *slot < 36).map(|(slot, s)| (slot as usize, s));
    let carried = stacks(host, carried);
    let window = crate::catpack_net::message("window", [("open", Tag::Byte(1)), ("items", held), ("inv", carried)]);
    crate::catpack_net::tell(host, player, window);
}

/// Writes `player`'s open bag as `next` and remembers it as the window's.
fn rewrite(host: &mut dyn ServerHost, player: PlayerRef, source: Source, next: ItemStack) -> bool {
    if !write_source(host, player, source, next.clone()) {
        return false;
    }
    let uuid = player.uuid();
    if let Some(w) = windows().iter_mut().find(|w| w.player == uuid) {
        w.bag = next;
    }
    true
}

/// Moves the stack in `player`'s inventory slot `slot` into their open bag.
fn stow(host: &mut dyn ServerHost, player: PlayerRef, slot: u32) {
    let Some((source, stack)) = open_bag(host, player) else { return };
    if slot >= 36 {
        return;
    }
    let Some((_, moved)) = player.inventory_slots(host).unwrap_or_default().into_iter().find(|(s, _)| *s == slot) else { return };
    // A bag doesn't go into a bag, and the bag in the hand doesn't go into itself.
    if bag(&moved).is_some() {
        say(host, player, "A backpack doesn't go into a backpack");
        return;
    }
    let mut slots = items(host, &stack);
    let Some(free) = slots.iter().position(Option::is_none) else {
        say(host, player, "The backpack is full");
        return;
    };
    slots[free] = Some(moved.clone());
    let Some(next) = with_items(host, &stack, &slots) else { return };
    // Into the bag first, out of the inventory after: a failure leaves the stack in both places
    // at worst, and the bag is put back as it was.
    if !rewrite(host, player, source, next) {
        return;
    }
    if player.set_inventory_slot(host, slot, None).is_err() {
        rewrite(host, player, source, stack);
    }
}

/// Moves the stack in slot `slot` of `player`'s open bag into their inventory.
fn take(host: &mut dyn ServerHost, player: PlayerRef, slot: usize) {
    let Some((source, stack)) = open_bag(host, player) else { return };
    let mut slots = items(host, &stack);
    let Some(moved) = slots.get_mut(slot).and_then(Option::take) else { return };
    let Some(next) = with_items(host, &stack, &slots) else { return };
    // Into the inventory first, out of the bag after.
    if player.give_stack(host, moved).is_err() {
        say(host, player, "No room in the inventory");
        return;
    }
    rewrite(host, player, source, next);
}

/// A client asks (`catpack_net`): the sender is the connection's own player, never one named in
/// the message.
pub fn asked(message: Compound, context: &mut dyn mod_api::IPayloadContext) {
    let Ok(player) = context.player() else { return };
    let Ok(host) = context.server_host() else { return };
    let slot = match message.get(&"s".into()) {
        Some(Tag::Int(s)) => u32::try_from(*s).ok(),
        _ => None,
    };
    match (crate::catpack_net::kind(&message).as_str(), slot) {
        ("off", _) => take_off(host, player),
        ("open", _) => open(host, player, Source::Back),
        ("close", _) => close(host, player),
        ("put", Some(slot)) => {
            stow(host, player, slot);
            show(host, player);
        }
        ("take", Some(slot)) => {
            take(host, player, slot as usize);
            show(host, player);
        }
        _ => {}
    }
}
