//! The cat backpack's rules, on the world's own server: a player's own tamed cat goes into the
//! bag, rides there, grows there, and comes back out as the same cat.
//!
//! Single player only: every rule here acts for the world's own player alone (`crate::own_player`:
//! the server is this game's own and the player is the one playing it here). A LAN guest or a
//! player of a dedicated server is never answered, told or changed.
//!
//! A bag is the mod's own registered item (`spider_man:cat_backpack`), never another item that
//! carries the same data. The cat in a bag is a short typed record (`Cat`), kept in the stack's
//! `custom_data` under `catpack_data`, beside what the bag shows (`catpack`: `kitten` or `cat`;
//! `catpack_cat`: the variant): its variant, voice, name, collar, age, whether its age is locked,
//! whether it sits, its health, its owner, and the UUID it had (only to refuse letting out a cat
//! that is already in the world). Nothing else of the cat is kept, and nothing else is ever put
//! back: a cat comes out of a bag as the mod builds it from those checked fields, by the mod
//! API's own spawn, for the player who owns it. So the bag can be dropped, put in a chest or
//! lost with its cat, as a bundle with its items, and no stack, whatever its data says, can put
//! anything else into the world.
//!
//! - **In**: use a bag in the hand on your own tamed cat, or, wearing the bag, sneak and use the
//!   cat. A bag that holds a cat, someone else's cat and an untamed one are refused with a line
//!   over the hotbar.
//! - **Out**: sneak and use the bag on the top of a block with two clear blocks above it. Only the
//!   cat's owner lets it out.
//! - **Growing**: a kitten in a bag in a player's inventory grows as it would outside (not one
//!   whose age is locked); grown, the bag shows a grown cat.
//!
//! A cat is never lost to a failure. Going in, the stack is written first and the cat removed
//! only after; coming out, the cat is put into the world first and the stack emptied after, and
//! a stack that can't be emptied takes the cat back out of the world.
//!
//! Known gap: the cat comes out with a new UUID (the mod API puts a mob into the world with one
//! of its own); everything the player sees of it (name, owner, variant, collar, age, health) is
//! the cat's own.
//!
//! The bag is worn in a back slot of the mod's own and holds 27 stacks (both below).
//!
//! Not here yet: growing in chests and as a dropped item.

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

/// What the stack is to the rules, if it is the mod's own bag item: empty, or holding a cat.
fn bag(stack: &ItemStack) -> Option<Bag> {
    if !crate::catpack::is_bag_item(stack) {
        return None;
    }
    let read = |name: &str| mod_api::ItemResource::of(stack).component(&mod_api::ComponentId::new(name)?);
    let mut custom = match read("minecraft:custom_data") {
        Some(Tag::Compound(custom)) => custom,
        _ => Compound::default(),
    };
    custom.entry("catpack".into()).or_insert_with(|| Tag::String("empty".into()));
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

/// The sixteen dye colours a collar may be, in the game's order (a collar saved as a number is
/// its index here).
const DYES: [&str; 16] = [
    "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray",
    "cyan", "purple", "blue", "brown", "green", "red", "black",
];

/// The voices a cat may have (`sound_variant`), as the game's own cat spawner picks them.
const VOICES: [&str; 2] = ["classic", "royal"];

/// The longest name a cat keeps, as an anvil allows.
const NAME_MAX: usize = 50;

/// A collar as the cat's data saved it: a number or a dye's name. Put back the same way.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Collar {
    Index(u8),
    Named(&'static str),
}

/// The cat in a bag: only these fields, each checked, are kept and ever put back.
#[derive(Clone, PartialEq, Debug)]
struct Cat {
    /// One of `VARIANTS`.
    variant: &'static str,
    /// One of `VOICES`; none keeps the game's own pick.
    voice: Option<&'static str>,
    /// Plain text, at most `NAME_MAX` characters; no styling, no events.
    name: Option<String>,
    collar: Option<Collar>,
    /// Ticks: below 0 a kitten, from -24000 to 0.
    age: i32,
    age_locked: bool,
    sitting: bool,
    /// From just above 0 to a cat's full 10.
    health: Option<f32>,
    /// Who owns the cat; it comes out only for them.
    owner: u128,
    /// The UUID the cat had when it went in.
    uuid: Option<u128>,
}

fn uuid_of(tag: Option<&Tag>) -> Option<u128> {
    match tag {
        Some(Tag::IntArray(parts)) if parts.len() == 4 => Some(parts.iter().fold(0u128, |acc, part| (acc << 32) | u128::from(*part as u32))),
        _ => None,
    }
}

fn uuid_tag(uuid: u128) -> Tag {
    Tag::IntArray((0..4).rev().map(|i| (uuid >> (32 * i)) as u32 as i32).collect())
}

/// A name as plain text: a string, or a text component's own `text` and its `extra` parts'.
fn plain_name(tag: &Tag, depth: u8, out: &mut String) {
    match tag {
        Tag::String(text) => out.push_str(&text.to_string_lossy()),
        Tag::Compound(component) if depth < 4 => {
            if let Some(text @ Tag::String(_)) = component.get(&"text".into()) {
                plain_name(text, depth + 1, out);
            }
            if let Some(Tag::List(extra)) = component.get(&"extra".into()) {
                for part in extra.iter().take(16) {
                    plain_name(part, depth + 1, out);
                }
            }
        }
        _ => {}
    }
}

impl Cat {
    /// The checked fields of `data`: a cat's data as the game saves it, or a bag's record of one
    /// (which uses the same names). `owner`, when given, is the owner (a cat going in, whose
    /// owner the game has just said); else the data's own. None without an owner, which no cat
    /// in a bag lacks.
    fn read(data: &Compound, owner: Option<u128>) -> Option<Self> {
        let owner = owner.or_else(|| uuid_of(data.get(&"Owner".into())))?;
        let variant = match data.get(&"variant".into()) {
            Some(Tag::String(s)) => {
                let named = s.to_string_lossy();
                let named = named.strip_prefix("minecraft:").unwrap_or(&named).to_owned();
                VARIANTS.iter().copied().find(|v| *v == named)
            }
            _ => None,
        }
        .unwrap_or("tabby");
        let voice = match data.get(&"sound_variant".into()) {
            Some(Tag::String(s)) => {
                let named = s.to_string_lossy();
                let named = named.strip_prefix("minecraft:").unwrap_or(&named).to_owned();
                VOICES.iter().copied().find(|v| *v == named)
            }
            _ => None,
        };
        let name = data.get(&"CustomName".into()).and_then(|tag| {
            let mut text = String::new();
            plain_name(tag, 0, &mut text);
            let text: String = text.chars().filter(|c| !c.is_control() && *c != '\u{a7}').take(NAME_MAX).collect();
            (!text.trim().is_empty()).then_some(text)
        });
        let collar = match data.get(&"CollarColor".into()) {
            Some(Tag::Byte(n)) => u8::try_from(*n).ok().filter(|n| *n < 16).map(Collar::Index),
            Some(Tag::Int(n)) => u8::try_from(*n).ok().filter(|n| *n < 16).map(Collar::Index),
            Some(Tag::String(s)) => {
                let named = s.to_string_lossy();
                let named = named.strip_prefix("minecraft:").unwrap_or(&named).to_owned();
                DYES.iter().copied().find(|d| *d == named).map(Collar::Named)
            }
            _ => None,
        };
        let age = match data.get(&"Age".into()) {
            Some(Tag::Int(age)) => (*age).clamp(-24000, 0),
            _ => 0,
        };
        let flag = |key: &str| matches!(data.get(&key.into()), Some(Tag::Byte(n)) if *n != 0);
        let health = match data.get(&"Health".into()) {
            Some(Tag::Float(bits)) => Some(f32::from_bits(*bits)).filter(|h| h.is_finite() && *h > 0.0).map(|h| h.min(10.0)),
            _ => None,
        };
        Some(Self {
            variant,
            voice,
            name,
            collar,
            age,
            age_locked: flag("AgeLocked"),
            sitting: flag("Sitting"),
            health,
            owner,
            uuid: uuid_of(data.get(&"UUID".into())),
        })
    }

    /// The cat's fields as the game's own data names them, for `owner`: what a bag keeps (with
    /// the cat's UUID, `with_uuid`) and what a released cat is given (without).
    fn data(&self, owner: u128, with_uuid: bool) -> Compound {
        let mut data = Compound::default();
        data.insert("variant".into(), Tag::String(format!("minecraft:{}", self.variant).as_str().into()));
        if let Some(voice) = self.voice {
            data.insert("sound_variant".into(), Tag::String(format!("minecraft:{voice}").as_str().into()));
        }
        if let Some(name) = &self.name {
            data.insert("CustomName".into(), Tag::String(name.as_str().into()));
        }
        match self.collar {
            Some(Collar::Index(n)) => _ = data.insert("CollarColor".into(), Tag::Byte(n as i8)),
            Some(Collar::Named(dye)) => _ = data.insert("CollarColor".into(), Tag::String(dye.into())),
            None => {}
        }
        data.insert("Age".into(), Tag::Int(self.age));
        data.insert("AgeLocked".into(), Tag::Byte(i8::from(self.age_locked)));
        data.insert("Sitting".into(), Tag::Byte(i8::from(self.sitting)));
        if let Some(health) = self.health {
            data.insert("Health".into(), Tag::Float(health.to_bits()));
        }
        data.insert("Owner".into(), uuid_tag(owner));
        if with_uuid && let Some(uuid) = self.uuid {
            data.insert("UUID".into(), uuid_tag(uuid));
        }
        data
    }

    /// What the cat shows on a bag: `kitten` or `cat`, and its variant.
    fn shown(&self) -> (&'static str, &'static str) {
        (if self.age < 0 { "kitten" } else { "cat" }, self.variant)
    }
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
            if !crate::own_world() {
                return None;
            }
            let worn = host.local_player().is_some_and(|player| {
                let entity = player.entity();
                entity.is_shift_key_down(host) == Some(true) && crate::catpack_ui::wears(entity.id())
            });
            return (in_hand.is_some() || worn).then_some(InteractionResult::Success);
        }
    };
    let player = host.players().acting_player()?;
    if !crate::own_player(player.uuid()) {
        return None;
    }
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
    // Only the checked fields go into the bag, and the owner is the player taking it in.
    let Some(owner) = owner else { return Some(InteractionResult::Fail) };
    let Some(record) = cat.nbt(host).and_then(|data| Cat::read(&data, Some(owner))) else {
        say(host, player, "The cat can't be carried right now");
        return Some(InteractionResult::Fail);
    };
    let (kind, variant) = record.shown();
    custom.insert("catpack".into(), Tag::String(kind.into()));
    custom.insert("catpack_cat".into(), Tag::String(variant.into()));
    custom.insert("catpack_data".into(), Tag::Compound(record.data(owner, true)));
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

/// Lets the cat in the bag `stack` (whose data is `custom`) out at `place`, for `player`: the cat
/// as it is now in the world and the bag as it then is, empty, or why not. The cat is put into
/// the world first and the bag emptied by the caller after, who takes the cat back out of the
/// world if that fails, so a failure leaves the cat in the bag.
///
/// The cat comes out only for its owner, and only as the mod builds it from the record's checked
/// fields (`Cat`): the mod API's own spawn of a plain cat, then those fields merged into it.
fn release(host: &mut dyn ServerHost, player: PlayerRef, stack: &ItemStack, mut custom: Compound, place: [f64; 3]) -> Result<(mod_api::EntityRef, ItemStack), &'static str> {
    let Some(Tag::Compound(saved)) = custom.get(&"catpack_data".into()) else { return Err("There is no cat in this backpack") };
    let Some(cat) = Cat::read(saved, None) else { return Err("There is no cat in this backpack") };
    if cat.owner != player.uuid() {
        return Err("Only the cat's owner can let it out");
    }
    let Some(level) = player.entity().level(host) else { return Err("The cat can't come out here") };
    // A cat already in the world (a failure once left it in both places) is not let out twice.
    if cat.uuid.is_some_and(|uuid| host.entity_by_uuid(mod_api::EntityUuid(uuid)).is_some()) {
        return Err("This cat is already out in the world");
    }
    let Some(out) = level.spawn_mob(host, "minecraft:cat", place, SpawnReason::Bucket) else { return Err("The cat can't come out here") };
    if !out.merge_nbt(host, cat.data(player.uuid(), false)) {
        out.discard(host);
        return Err("The cat can't come out here");
    }
    custom.remove(&"catpack_data".into());
    custom.remove(&"catpack_cat".into());
    custom.insert("catpack".into(), Tag::String("empty".into()));
    match rewritten(stack, custom, "spider_man:catpack") {
        Some(empty) => Ok((out, empty)),
        None => {
            out.discard(host);
            Err("The cat can't come out here")
        }
    }
}

/// A player uses what they hold on a block: sneaking, a bag with a cat lets it out on top of
/// the block, as a bucket empties.
pub fn use_item_on_block(host: HostSide, event: &mut UseItemOnBlockEvent<'_>) -> Option<InteractionResult> {
    let click = event.click;
    let stack = click.stack?;
    let Bag::Holding(custom) = bag(stack)? else { return None };
    // Once a click: before the block has its say.
    if event.phase != mod_api::UsePhase::ItemBeforeBlock {
        return None;
    }
    let host = match host {
        HostSide::Server(host) => host,
        // The client says the click is taken where the server will let the cat out, so it
        // doesn't go on to use the item as well (which would put the emptied bag on the back).
        HostSide::Client(host) => {
            if !crate::own_world() {
                return None;
            }
            let sneaking = host.local_player().is_some_and(|p| p.entity().is_shift_key_down(host) == Some(true));
            return (sneaking && click.hit.face == [0, 1, 0]).then_some(InteractionResult::Success);
        }
    };
    let player = match host.players().acting_player() {
        Some(player) => player,
        None => host.players().server_players().into_iter().find(|p| p.uuid() == click.player)?,
    };
    if !crate::own_player(player.uuid()) {
        return None;
    }
    if player.entity().is_shift_key_down(host) != Some(true) {
        return None;
    }
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
    let place = [f64::from(at[0]) + 0.5, f64::from(at[1]), f64::from(at[2]) + 0.5];
    match release(host, player, stack, custom, place) {
        Ok((cat, empty)) => {
            // The bag emptied, or the cat back out of the world: never in both places.
            if !matches!(LivingRef::new(player.entity()).set_hand_item(host, click.hand, Some(empty)), Ok(true)) {
                cat.discard(host);
                say(host, player, "The cat can't come out here");
                return Some(InteractionResult::Fail);
            }
            Some(InteractionResult::SuccessServer)
        }
        Err(why) => {
            say(host, player, why);
            Some(InteractionResult::Fail)
        }
    }
}

/// `stack` a second later, if it is a bag whose kitten grew in that second. The record is
/// written back with only its checked fields.
fn grown(stack: &ItemStack) -> Option<ItemStack> {
    let Bag::Holding(mut custom) = bag(stack)? else { return None };
    let Some(Tag::Compound(saved)) = custom.get(&"catpack_data".into()) else { return None };
    let mut cat = Cat::read(saved, None)?;
    if cat.age_locked || cat.age >= 0 {
        return None;
    }
    cat.age = (cat.age + 20).min(0);
    let (kind, variant) = cat.shown();
    custom.insert("catpack".into(), Tag::String(kind.into()));
    custom.insert("catpack_cat".into(), Tag::String(variant.into()));
    custom.insert("catpack_data".into(), Tag::Compound(cat.data(cat.owner, true)));
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
    // Nothing at all on a server that isn't this game's own, or before its player is known.
    if crate::own_server_owner().is_none() {
        return;
    }
    let level = event.level;
    save_backs(host, level);
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
    for player in own_players(host, level) {
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

/// The players of `level` the rules act for: the world's own player, if they are there.
fn own_players(host: &mut dyn ServerHost, level: mod_api::LevelRef) -> Vec<PlayerRef> {
    let mut players = level.online_players(host).unwrap_or_default();
    players.retain(|p| crate::own_player(p.uuid()));
    players
}

/// What the server holds in memory for players, beyond what their save holds: all of it goes
/// when a server starts or stops, so nothing of one world reaches the next.
pub fn forget_all() {
    backs().clear();
    slot_clicks().clear();
    UNSAVED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clear();
    thrown().clear();
    throws_awaited().clear();
    windows().clear();
    TICKS.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clear();
}

/// `player` leaves: a back changed inside a click is saved now, while they are still listed,
/// then what the server held for them in memory goes.
pub fn logged_out(host: &mut dyn ServerHost, player: u128) {
    let pending = {
        let mut unsaved = UNSAVED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let had = unsaved.contains(&player);
        unsaved.retain(|u| *u != player);
        had
    };
    if pending
        && let Some(online) = host.players().server_players().into_iter().find(|p| p.uuid() == player)
    {
        let worn = backs().iter().find(|(u, _)| *u == player).and_then(|(_, stack)| stack.clone());
        set_back(host, online, worn);
    }
    backs().retain(|(u, _)| *u != player);
    slot_clicks().retain(|(u, _)| *u != player);
    thrown().retain(|(u, ..)| *u != player);
    throws_awaited().retain(|(u, _)| *u != player);
    windows().retain(|w| w.player != player);
}

// ---- The back slot ----
//
// A bag is worn on the back, in a slot of the mod's own: the chest slot stays free for a
// chestplate or an elytra. The game has no such slot, so the server keeps it: the stack lives in
// its wearer's persistent data (`PlayerPersisted.spider_man_back`, the part the game carries over
// a death), which the game saves with the player. The save is what counts: it is read every time
// the back is needed, and memory holds it only between a click that changes it and the next tick
// that saves it. Only the wearer's own game is told what it shows (`catpack_net`).
//
// - **On**: sneak and use a bag in the hand (anywhere but on the top of a block when the bag
//   holds a cat: that lets the cat out). A bag already on the back comes into the hand.
// - **The key with shift**: the quick way off. The bag goes into the main hand when it is empty,
//   else into the inventory.
// - **The slot**, beside the inventory: a click swaps the bag on the cursor with the worn one
//   (either may be none), inside a pickup click of the game's own (`stacked_on_other`); shift-click on a bag in the inventory wears it, the worn one taking its
//   place; shift-click on the slot, or the window's Take off, puts the worn bag into the
//   inventory, where with no room it stays on the back.
// - **Death**: the bag drops where its wearer died, with all that is in it, unless the world
//   keeps inventories.
// - **On the ground**: a dropped bag never despawns (`item_expire`), however it got there: a death,
//   a throw, a player's inventory spilled. Lava and the void still take it (it doesn't burn, though).

/// Where a player's persistent data keeps the bag on their back: under `PERSISTED`, which the
/// game carries over to a player's respawn (the rest of the data is lost at a death). 0.2.6 kept
/// it at the top level, which is still read and moved under `PERSISTED` with the next write.
const BACK: &str = "spider_man_back";
const PERSISTED: &str = "PlayerPersisted";

/// Each player's back, as last read from their save or written: their UUID and the bag. Read
/// only where there is no host to read the save with (inside a click), or while a change made
/// there waits for the next tick to be saved.
static BACKS: std::sync::Mutex<Vec<(u128, Option<ItemStack>)>> = std::sync::Mutex::new(Vec::new());

fn backs() -> std::sync::MutexGuard<'static, Vec<(u128, Option<ItemStack>)>> {
    BACKS.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The bag on `player`'s back, as their save holds it (or as a click changed it, until the
/// next tick saves that).
fn back(host: &mut dyn ServerHost, player: PlayerRef) -> Option<ItemStack> {
    let uuid = player.uuid();
    if UNSAVED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).contains(&uuid)
        && let Some((_, stack)) = backs().iter().find(|(u, _)| *u == uuid)
    {
        return stack.clone();
    }
    let data = player.persistent_data(host).unwrap_or_default();
    let saved = match data.get(&PERSISTED.into()) {
        Some(Tag::Compound(persisted)) if persisted.contains_key(&BACK.into()) => persisted.get(&BACK.into()).cloned(),
        _ => data.get(&BACK.into()).cloned(),
    };
    let stack = match saved {
        Some(Tag::Compound(saved)) => ItemStack::from_nbt(host, &saved),
        _ => None,
    };
    let mut all = backs();
    match all.iter_mut().find(|(u, _)| *u == uuid) {
        Some(known) => known.1 = stack.clone(),
        None => all.push((uuid, stack.clone())),
    }
    stack
}

/// Puts `stack` (or nothing) on `player`'s back: saved first, then told to the wearer.
fn set_back(host: &mut dyn ServerHost, player: PlayerRef, stack: Option<ItemStack>) -> bool {
    let mut data = player.persistent_data(host).unwrap_or_default();
    let mut persisted = match data.get(&PERSISTED.into()) {
        Some(Tag::Compound(persisted)) => persisted.clone(),
        _ => Compound::default(),
    };
    match stack.as_ref().map(|s| s.to_nbt(host)) {
        Some(Some(saved)) => {
            persisted.insert(BACK.into(), Tag::Compound(saved));
        }
        Some(None) => return false,
        None => {
            persisted.remove(&BACK.into());
        }
    }
    data.remove(&BACK.into());
    data.insert(PERSISTED.into(), Tag::Compound(persisted));
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

/// Tells the wearer's own game what is on their back (single player: there is no one else).
fn announce(host: &mut dyn ServerHost, wearer: PlayerRef) {
    let news = back_news(host, wearer);
    let news = own_news(host, wearer, news);
    crate::catpack_net::tell(host, wearer, news);
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
    let HostSide::Server(host) = host else { return crate::own_world() };
    let Some(player) = host.players().server_players().into_iter().find(|p| p.uuid() == click.player) else { return false };
    if !crate::own_player(player.uuid()) {
        return false;
    }
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

/// The bag in `player`'s inventory slot `slot` goes onto their back; the one worn, if any, into
/// that slot.
fn wear_from(host: &mut dyn ServerHost, player: PlayerRef, slot: u32) {
    if slot >= 36 {
        return;
    }
    let Some((_, moved)) = player.inventory_slots(host).unwrap_or_default().into_iter().find(|(s, _)| *s == slot) else { return };
    if bag(&moved).is_none() || moved.count() != 1 {
        return;
    }
    close(host, player);
    // The back first, the slot after, as putting on from the hand.
    let before = back(host, player);
    if !set_back(host, player, Some(moved)) {
        return;
    }
    if player.set_inventory_slot(host, slot, before.clone()).is_err() {
        set_back(host, player, before);
    }
}

/// The back and `player`'s hotbar slot `slot` change places, as a number key over the off hand's
/// slot does: a bag there is worn and the worn one lands there; an empty slot takes the worn bag.
fn swap_with(host: &mut dyn ServerHost, player: PlayerRef, slot: u32) {
    if slot >= 9 {
        return;
    }
    let there = player.inventory_slots(host).unwrap_or_default().into_iter().find(|(s, _)| *s == slot).map(|(_, stack)| stack);
    let worn = back(host, player);
    if there.as_ref().is_some_and(|stack| stack.count() != 1 || bag(stack).is_none()) || (there.is_none() && worn.is_none()) {
        return;
    }
    close(host, player);
    // The back first, the slot after.
    if !set_back(host, player, there.clone()) {
        return;
    }
    if player.set_inventory_slot(host, slot, worn.clone()).is_err() {
        set_back(host, player, worn);
    }
}

/// The bag's key with shift: the bag comes off `player`'s back, into their main hand when it is
/// empty, else into their inventory. It puts nothing on.
fn quick_off(host: &mut dyn ServerHost, player: PlayerRef) {
    let Some(worn) = back(host, player) else { return };
    let living = LivingRef::new(player.entity());
    if living.hand_item(host, mod_api::Hand::Main).is_some() {
        take_off(host, player);
        return;
    }
    close(host, player);
    // Into the hand first, off the back after.
    if matches!(living.set_hand_item(host, mod_api::Hand::Main, Some(worn)), Ok(true)) && !set_back(host, player, None) {
        let _ = living.set_hand_item(host, mod_api::Hand::Main, None);
    }
}

/// What the bag `stack` holds, each stack with its slot: for either side to show.
pub fn stored(host: &dyn mod_api::Host, stack: &ItemStack) -> Vec<(usize, ItemStack)> {
    let custom = match bag(stack) {
        Some(Bag::Empty(c) | Bag::Holding(c)) => c,
        None => return Vec::new(),
    };
    let Some(Tag::List(list)) = custom.get(&"catpack_items".into()) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in list {
        let Tag::Compound(entry) = entry else { continue };
        let (Some(Tag::Byte(slot)), Some(Tag::Compound(saved))) = (entry.get(&"s".into()), entry.get(&"i".into())) else { continue };
        if let (Ok(slot), Some(held)) = (usize::try_from(*slot), ItemStack::from_nbt(host, saved))
            && slot < SLOTS
        {
            out.push((slot, held));
        }
    }
    out
}

/// Whether the stack is a cat backpack.
pub fn is_bag(stack: &ItemStack) -> bool {
    bag(stack).is_some()
}

/// Players who clicked their back slot, and when: each one's next pickup click is that slot's.
static SLOT_CLICKS: std::sync::Mutex<Vec<(u128, std::time::Instant)>> = std::sync::Mutex::new(Vec::new());
/// Players whose back changed inside a click, where nothing can be saved: the next tick saves it.
static UNSAVED: std::sync::Mutex<Vec<u128>> = std::sync::Mutex::new(Vec::new());

fn slot_clicks() -> std::sync::MutexGuard<'static, Vec<(u128, std::time::Instant)>> {
    SLOT_CLICKS.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// `player` clicked the back slot of their inventory: the pickup click their game sends next is
/// answered by `stacked_on_other`. The cursor is the game's to move, and it moves it inside a
/// click, so the swap is done there, as the game picks a stack up.
fn slot_click(host: &mut dyn ServerHost, player: PlayerRef) {
    // The back is read here, where there is a host to read it with.
    back(host, player);
    close(host, player);
    let mut clicks = slot_clicks();
    clicks.retain(|(uuid, at)| *uuid != player.uuid() && at.elapsed().as_secs() < 2);
    clicks.push((player.uuid(), std::time::Instant::now()));
}

/// The server's answer to a pickup click (`item_stacked_on_other`): the click a player's game
/// made for their back slot swaps the bag on the cursor with the bag on the back, either of
/// which may be none, and does nothing to the slot it names. The back is saved with the next tick.
pub fn stacked_on_other(event: &mut mod_api::ItemStackedOnOtherEvent<'_, '_>) {
    let Some(uuid) = event.event.player.filter(|uuid| crate::own_player(*uuid)) else { return };
    {
        let mut clicks = slot_clicks();
        let Some(i) = clicks.iter().position(|(u, _)| *u == uuid) else { return };
        if clicks.remove(i).1.elapsed().as_secs() >= 2 {
            return;
        }
    }
    event.cancelled = true;
    event.event.cancellation_result = true;
    let held = event.event.carried.get().cloned();
    if held.as_ref().is_some_and(|held| held.count() != 1 || bag(held).is_none()) {
        return;
    }
    let mut all = backs();
    let Some(known) = all.iter_mut().find(|(u, _)| *u == uuid) else { return };
    let worn = std::mem::replace(&mut known.1, held);
    event.event.carried.set(worn);
    drop(all);
    UNSAVED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(uuid);
}

/// Saves and tells the backs that changed inside a click, for the players in `level`.
fn save_backs(host: &mut dyn ServerHost, level: mod_api::LevelRef) {
    let unsaved = std::mem::take(&mut *UNSAVED.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
    if unsaved.is_empty() {
        return;
    }
    let players = own_players(host, level);
    for uuid in unsaved {
        let saved = players.iter().find(|player| player.uuid() == uuid).is_some_and(|player| {
            let worn = backs().iter().find(|(u, _)| *u == uuid).and_then(|(_, stack)| stack.clone());
            set_back(host, *player, worn)
        });
        if !saved {
            UNSAVED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(uuid);
        }
    }
}

/// Bags just thrown: by whom, the item entity, and when.
static THROWN: std::sync::Mutex<Vec<(u128, mod_api::ItemEntityRef, std::time::Instant)>> = std::sync::Mutex::new(Vec::new());

fn thrown() -> std::sync::MutexGuard<'static, Vec<(u128, mod_api::ItemEntityRef, std::time::Instant)>> {
    THROWN.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Players who said their next throw of a bag is onto their back slot, and when.
static THROWS_AWAITED: std::sync::Mutex<Vec<(u128, std::time::Instant)>> = std::sync::Mutex::new(Vec::new());

fn throws_awaited() -> std::sync::MutexGuard<'static, Vec<(u128, std::time::Instant)>> {
    THROWS_AWAITED.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The bag `stack` goes onto `player`'s back and the bag that was worn into their inventory.
fn wear_thrown(host: &mut dyn ServerHost, player: PlayerRef, stack: ItemStack) -> bool {
    close(host, player);
    let before = back(host, player);
    if !set_back(host, player, Some(stack)) {
        return false;
    }
    if let Some(before) = before {
        let _ = player.give_stack(host, before);
    }
    true
}

/// A player throws an item. A creative inventory lets a stack off its cursor no other way than a
/// throw, so a bag clicked onto the back slot there arrives as one, and the player's game says
/// so (`thrown`) in the tick of the click: before the throw when the button is let go later,
/// after it otherwise. A throw that was awaited is worn and never lands (true: cancel it); any
/// other bag thrown is remembered for a moment, in case the word comes after.
pub fn item_toss(host: &mut dyn ServerHost, event: &mut mod_api::ItemTossEvent) -> bool {
    if !crate::own_player(event.player) {
        return false;
    }
    let Some(stack) = event.item.stack(host) else { return false };
    if stack.count() != 1 || bag(&stack).is_none() {
        return false;
    }
    let awaited = {
        let mut all = throws_awaited();
        all.retain(|(_, at)| at.elapsed().as_secs() < 5);
        all.iter().position(|(uuid, _)| *uuid == event.player).map(|i| all.remove(i)).is_some()
    };
    if awaited
        && let Some(player) = host.players().server_players().into_iter().find(|p| p.uuid() == event.player)
        && wear_thrown(host, player, stack)
    {
        return true;
    }
    let mut all = thrown();
    all.retain(|(.., at)| at.elapsed().as_secs() < 2);
    all.push((event.player, event.item, std::time::Instant::now()));
    false
}

/// `player` says the bag they throw was thrown onto their back slot: one just thrown is worn
/// and its item entity is gone; else the throw to come is awaited.
fn thrown_on_slot(host: &mut dyn ServerHost, player: PlayerRef) {
    let found = {
        let mut all = thrown();
        all.iter().rposition(|(uuid, _, at)| *uuid == player.uuid() && at.elapsed().as_secs() < 2).map(|i| all.remove(i).1)
    };
    let Some(item) = found else {
        throws_awaited().push((player.uuid(), std::time::Instant::now()));
        return;
    };
    let Some(stack) = item.stack(host) else { return };
    if stack.count() != 1 || bag(&stack).is_none() {
        return;
    }
    // Out of the world first: a bag that can't be taken from it stays lying there, unworn.
    if item.entity().discard(host) && !wear_thrown(host, player, stack.clone()) {
        let _ = player.give_stack(host, stack);
    }
}

/// Each server tick of a level: every couple of seconds everyone is told every back again (a
/// player who joined or came near catches up), and open windows are kept true.
fn backs_tick(host: &mut dyn ServerHost, level: mod_api::LevelRef, second: bool) {
    let players = own_players(host, level);
    for player in &players {
        if windows().iter().any(|w| w.player == player.uuid()) {
            show(host, *player);
        }
    }
    if !second {
        return;
    }
    for player in &players {
        announce(host, *player);
    }
}

/// The most life the game lets a mod give a dropped item at a time (`ItemExpireEvent`).
const MAX_EXTRA_LIFE: i32 = 32766;

/// A dropped bag reaches its lifetime: it gets more, every time, so a bag with a cat or things in it
/// is never lost to the five-minute clean-up of items on the ground.
pub fn item_expire(event: &mut mod_api::ItemExpireEvent<'_>) {
    if crate::own_server_owner().is_some() && bag(event.stack).is_some() {
        event.extra_life = event.extra_life.max(MAX_EXTRA_LIFE);
    }
}

/// A player dies: the bag on their back drops there, unless the world keeps inventories.
pub fn living_death(host: &mut dyn ServerHost, event: &mut mod_api::LivingDeathEvent<'_>) {
    if event.kind != "minecraft:player" {
        return;
    }
    let Some(entity) = host.entity(event.entity) else { return };
    let Some(player) = host.players().server_players().into_iter().find(|p| p.entity().id() == event.entity) else { return };
    if !crate::own_player(player.uuid()) {
        return;
    }
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
    let Some((source, stack)) = open_bag(host, player) else { return };
    let held = items(host, &stack).into_iter().enumerate().filter_map(|(slot, s)| Some((slot, s?)));
    let held = stacks(host, held);
    let carried = player.inventory_slots(host).unwrap_or_default().into_iter().filter(|(slot, _)| *slot < 36).map(|(slot, s)| (slot as usize, s));
    let carried = stacks(host, carried);
    let from_back = matches!(source, Source::Back);
    let window = crate::catpack_net::message("window", [("open", Tag::Byte(1)), ("items", held), ("inv", carried), ("back", Tag::Byte(from_back as i8))]);
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
    // Single player only: an ask from anyone but the world's own player is not heard.
    if !crate::own_player(player.uuid()) {
        return;
    }
    let Ok(host) = context.server_host() else { return };
    let slot = match message.get(&"s".into()) {
        Some(Tag::Int(s)) => u32::try_from(*s).ok(),
        _ => None,
    };
    match (crate::catpack_net::kind(&message).as_str(), slot) {
        ("off", _) => take_off(host, player),
        ("slot", _) => slot_click(host, player),
        ("noslot", _) => slot_clicks().retain(|(uuid, _)| *uuid != player.uuid()),
        ("wear", Some(slot)) => wear_from(host, player, slot),
        ("swap", Some(slot)) => swap_with(host, player, slot),
        ("hand", _) => quick_off(host, player),
        ("thrown", _) => thrown_on_slot(host, player),
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
