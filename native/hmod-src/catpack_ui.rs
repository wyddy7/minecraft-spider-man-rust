//! The cat backpack on a client: what its server told it (who wears which bag, the open storage
//! window), the window itself, and what the player asks of the server.
//!
//! The window is a screen of the mod's own, laid out as a chest's: the bag's 27 slots over the
//! player's inventory and hotbar. A click on a stack asks the server to move the whole stack to
//! the other side; the server answers with the window as it then is. Nothing moves here.

use std::sync::{Mutex, PoisonError};
use std::time::Instant;

use mod_api::{ClientHost, Compound, DrawContext, ItemStack, Tag};

use crate::catpack_net::{kind, message};

const MOD: &str = "spider_man";

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Who wears a bag, as the server last said: the entity, what the bag shows, and when it was
/// said (a wearer the server stops naming is forgotten).
static BACKS: Mutex<Vec<(u32, String, String, Instant)>> = Mutex::new(Vec::new());
/// The open window: the bag's stacks by slot, and the inventory's by slot.
static WINDOW: Mutex<Option<(Vec<(usize, ItemStack)>, Vec<(usize, ItemStack)>)>> = Mutex::new(None);
/// The bag on this player's own back, whole, for the inventory's back slot.
static MINE: Mutex<Option<ItemStack>> = Mutex::new(None);
/// The key that opens the worn bag, as Controls lists it.
pub const KEY: &str = "catpack";
/// Inventory slots the server said to pick up onto the cursor, each with the ticks left to wait
/// (the slot's new stack arrives beside the message, not before it).
static PICKS: Mutex<Vec<(u32, u8)>> = Mutex::new(Vec::new());
/// The player clicked the back slot with a stack on the cursor; the next tick sees to it.
static CURSOR_TO_BACK: Mutex<bool> = Mutex::new(false);
/// What the player asked since the last tick, sent with the next one.
static ASKS: Mutex<Vec<Compound>> = Mutex::new(Vec::new());

/// The bags on backs: the wearer's entity id, the bag's kind and its cat's variant.
pub fn backs() -> Vec<(u32, String, String)> {
    let mut all = lock(&BACKS);
    all.retain(|(.., at)| at.elapsed().as_secs() < 8);
    all.iter().map(|(id, kind, variant, _)| (*id, kind.clone(), variant.clone())).collect()
}

/// Whether the entity wears a bag on the back.
pub fn wears(entity: u32) -> bool {
    lock(&BACKS).iter().any(|(id, ..)| *id == entity)
}

fn text(message: &Compound, key: &str) -> Option<String> {
    match message.get(&key.into()) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    }
}

fn stacks(host: &dyn ClientHost, message: &Compound, key: &str) -> Vec<(usize, ItemStack)> {
    let Some(Tag::List(list)) = message.get(&key.into()) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in list {
        let Tag::Compound(entry) = entry else { continue };
        let (Some(Tag::Int(slot)), Some(Tag::Compound(saved))) = (entry.get(&"s".into()), entry.get(&"i".into())) else { continue };
        if let (Ok(slot), Some(stack)) = (usize::try_from(*slot), ItemStack::from_nbt(host, saved)) {
            out.push((slot, stack));
        }
    }
    out
}

/// The server tells this client (`catpack_net`).
pub fn heard(told: Compound, context: &mut dyn mod_api::IPayloadContext) {
    match kind(&told).as_str() {
        "back" => {
            let Some(Tag::Int(entity)) = told.get(&"e".into()) else { return };
            let entity = *entity as u32;
            // The wearer's own game is also sent the bag itself.
            if let Ok(host) = context.client_host()
                && host.local_player().is_some_and(|p| p.entity().id() == entity)
            {
                *lock(&MINE) = match told.get(&"i".into()) {
                    Some(Tag::Compound(saved)) => ItemStack::from_nbt(host, saved),
                    _ => None,
                };
            }
            let mut all = lock(&BACKS);
            all.retain(|(id, ..)| *id != entity);
            if let Some(kind) = text(&told, "kind") {
                all.push((entity, kind, text(&told, "variant").unwrap_or_else(|| "red".into()), Instant::now()));
            }
        }
        "pick" => {
            if let Some(Tag::Int(slot)) = told.get(&"s".into())
                && let Ok(slot) = u32::try_from(*slot)
            {
                lock(&PICKS).push((slot, 2));
            }
        }
        "window" => {
            let Ok(host) = context.client_host() else { return };
            if matches!(told.get(&"open".into()), Some(Tag::Byte(0))) {
                *lock(&WINDOW) = None;
                host.mod_screen_close(MOD);
                return;
            }
            let window = (stacks(host, &told, "items"), stacks(host, &told, "inv"));
            let first = lock(&WINDOW).replace(window).is_none();
            if first && !host.mod_screen_open(MOD) {
                // Another screen is in the way: the server is told the window is not shown.
                *lock(&WINDOW) = None;
                lock(&ASKS).push(message("close", []));
            }
        }
        _ => {}
    }
}

/// Each client tick: what was asked goes to the server, and a window the player closed is
/// reported closed.
pub fn tick(host: &mut dyn ClientHost) {
    if lock(&WINDOW).is_some() && host.mod_screen() != Some(MOD) {
        *lock(&WINDOW) = None;
        lock(&ASKS).push(message("close", []));
    }
    // The bag's key opens the bag on the back.
    while host.consume_click(MOD, KEY) {
        if lock(&MINE).is_some() {
            lock(&ASKS).push(message("open", []));
        }
    }
    // A bag taken off through the back slot is picked up from where the server put it.
    let due: Vec<u32> = {
        let mut picks = lock(&PICKS);
        picks.iter_mut().for_each(|(_, wait)| *wait = wait.saturating_sub(1));
        let due = picks.iter().filter(|(_, wait)| *wait == 0).map(|(slot, _)| *slot).collect();
        picks.retain(|(_, wait)| *wait > 0);
        due
    };
    for slot in due {
        let Some(menu) = host.local_player().and_then(|player| player.container_menu(&*host)) else { continue };
        let Some(index) = menu.slot_kinds(&*host).and_then(|kinds| kinds.iter().position(|kind| *kind == mod_api::MenuSlotKind::Player(slot))) else { continue };
        menu.click(host, &[mod_api::MenuClick::Pickup { slot: Some(index), button: mod_api::MouseButton::Left }]);
    }
    // A bag on the cursor, clicked onto the back slot: it is put down in a free inventory slot,
    // as a click of the player's would, and the server is asked to wear it from there and to
    // send the bag that was worn back to the cursor.
    if std::mem::take(&mut *lock(&CURSOR_TO_BACK))
        && let Some(menu) = host.local_player().and_then(|player| player.container_menu(&*host))
        && menu.carried(&*host).flatten().is_some_and(|stack| stack.count() == 1 && stack.item().id(&*host) == crate::catpack::ITEM)
        && let Some(kinds) = menu.slot_kinds(&*host)
    {
        let free = |range: std::ops::Range<u32>| {
            kinds.iter().enumerate().find_map(|(index, kind)| match kind {
                mod_api::MenuSlotKind::Player(slot) if range.contains(slot) && menu.slot(&*host, index) == Some(None) => Some((index, *slot)),
                _ => None,
            })
        };
        if let Some((index, slot)) = free(9..36).or_else(|| free(0..9))
            && menu.click(host, &[mod_api::MenuClick::Pickup { slot: Some(index), button: mod_api::MouseButton::Left }])
        {
            lock(&ASKS).push(message("wear", [("s", Tag::Int(slot as i32)), ("c", Tag::Byte(1))]));
        }
    }
    for ask in std::mem::take(&mut *lock(&ASKS)) {
        crate::catpack_net::ask(host, ask);
    }
}

// The window's layout, in GUI pixels from its top left: a chest's.
const WIDTH: f32 = 176.0;
const HEIGHT: f32 = 168.0;
const CELL: f32 = 18.0;
const BAG_AT: [f32; 2] = [8.0, 18.0];
const INVENTORY_AT: [f32; 2] = [8.0, 86.0];
const HOTBAR_AT: [f32; 2] = [8.0, 144.0];

/// Where a cell is: a bag slot, or an inventory slot (0 to 8 the hotbar).
#[derive(Clone, Copy, PartialEq)]
enum Cell {
    Bag(usize),
    Inventory(usize),
}

/// Every cell with its top left corner, from the window's.
fn cells() -> impl Iterator<Item = (Cell, [f32; 2])> {
    let grid = |at: [f32; 2], rows: usize| (0..rows * 9).map(move |i| (i, [at[0] + (i % 9) as f32 * CELL, at[1] + (i / 9) as f32 * CELL]));
    grid(BAG_AT, 3)
        .map(|(i, at)| (Cell::Bag(i), at))
        .chain(grid(INVENTORY_AT, 3).map(|(i, at)| (Cell::Inventory(i + 9), at)))
        .chain(grid(HOTBAR_AT, 1).map(|(i, at)| (Cell::Inventory(i), at)))
}

fn origin(size: [f32; 2]) -> [f32; 2] {
    [((size[0] - WIDTH) / 2.0).floor(), ((size[1] - HEIGHT) / 2.0).floor()]
}

fn hovered(size: [f32; 2], mouse: [f32; 2]) -> Option<Cell> {
    let [ox, oy] = origin(size);
    cells().find(|(_, [x, y])| mouse[0] >= ox + x && mouse[0] < ox + x + CELL && mouse[1] >= oy + y && mouse[1] < oy + y + CELL).map(|(cell, _)| cell)
}

/// The last frame's GUI size, for the click that follows it (a click has no draw context).
static SIZE: Mutex<[f32; 2]> = Mutex::new([0.0; 2]);

/// Draws the window over the mod's own screen.
pub fn draw(host: &dyn ClientHost, draw: &mut dyn DrawContext, mouse: [f32; 2]) {
    if host.mod_screen() != Some(MOD) {
        return;
    }
    let Some((bag, inventory)) = lock(&WINDOW).clone() else { return };
    let size = draw.size();
    *lock(&SIZE) = size;
    let [ox, oy] = origin(size);
    // The panel, as the game's containers: light grey with a bright top left and a dark bottom
    // right edge.
    draw.fill([ox - 1.0, oy - 1.0, WIDTH + 2.0, HEIGHT + 2.0], 0xFF00_0000);
    draw.fill([ox, oy, WIDTH, HEIGHT], 0xFFC6_C6C6);
    draw.fill([ox, oy, WIDTH, 2.0], 0xFFFF_FFFF);
    draw.fill([ox, oy, 2.0, HEIGHT], 0xFFFF_FFFF);
    draw.fill([ox, oy + HEIGHT - 2.0, WIDTH, 2.0], 0xFF55_5555);
    draw.fill([ox + WIDTH - 2.0, oy, 2.0, HEIGHT], 0xFF55_5555);
    draw.text("Back catpack", [ox + 8.0, oy + 6.0], 0xFF40_4040, false);
    draw.text("Inventory", [ox + 8.0, oy + 75.0], 0xFF40_4040, false);
    if lock(&MINE).is_some() {
        let [bx, by, bw, bh] = take_off_button(size);
        let over_button = mouse[0] >= bx && mouse[0] < bx + bw && mouse[1] >= by && mouse[1] < by + bh;
        draw.fill([bx, by, bw, bh], 0xFF37_3737);
        draw.fill([bx + 1.0, by + 1.0, bw - 2.0, bh - 2.0], if over_button { 0xFFA0_A0A0 } else { 0xFF8B_8B8B });
        draw.text("Take off", [bx + (bw - draw.text_width("Take off")) / 2.0, by + 2.0], 0xFFFF_FFFF, true);
    }
    let over = hovered(size, mouse);
    for (cell, [x, y]) in cells() {
        let (x, y) = (ox + x, oy + y);
        // A slot: dark top left, bright bottom right, a grey well.
        draw.fill([x, y, CELL, CELL], 0xFF37_3737);
        draw.fill([x + 1.0, y + 1.0, CELL - 1.0, CELL - 1.0], 0xFFFF_FFFF);
        draw.fill([x + 1.0, y + 1.0, CELL - 2.0, CELL - 2.0], 0xFF8B_8B8B);
        let held = match cell {
            Cell::Bag(slot) => bag.iter().find(|(s, _)| *s == slot),
            Cell::Inventory(slot) => inventory.iter().find(|(s, _)| *s == slot),
        };
        if let Some((_, stack)) = held {
            // The count is drawn here, inside the slot's bottom right corner as the game's own.
            draw.gui_draw_item_with(stack, [x + 1.0, y + 1.0], Some(""));
            if stack.count() > 1 {
                let count = stack.count().to_string();
                draw.text(&count, [x + 17.0 - draw.text_width(&count), y + 9.0], 0xFFFF_FFFF, true);
            }
        }
        if over == Some(cell) {
            draw.fill([x + 1.0, y + 1.0, CELL - 2.0, CELL - 2.0], 0x80FF_FFFF);
        }
    }
}

/// A mouse button goes down over the mod's own screen: a left click on a stack asks the server
/// to move it across. True when the click was the window's.
pub fn click(host: &dyn ClientHost, mouse: [f32; 2], button: i32) -> bool {
    if host.mod_screen() != Some(MOD) || lock(&WINDOW).is_none() {
        return false;
    }
    if button != 0 {
        return true;
    }
    let size = *lock(&SIZE);
    let [bx, by, bw, bh] = take_off_button(size);
    if lock(&MINE).is_some() && mouse[0] >= bx && mouse[0] < bx + bw && mouse[1] >= by && mouse[1] < by + bh {
        lock(&ASKS).push(message("off", []));
        return true;
    }
    match hovered(size, mouse) {
        Some(Cell::Bag(slot)) => lock(&ASKS).push(message("take", [("s", Tag::Int(slot as i32))])),
        Some(Cell::Inventory(slot)) => lock(&ASKS).push(message("put", [("s", Tag::Int(slot as i32))])),
        None => {}
    }
    true
}

/// The window's "Take off" button, shown while a bag is on the back: `[x, y, w, h]`.
fn take_off_button(size: [f32; 2]) -> [f32; 4] {
    let [ox, oy] = origin(size);
    [ox + WIDTH - 62.0, oy + 4.0, 54.0, 12.0]
}

/// The back slot beside the player's own inventory (survival's and creative's): `[x, y]` of its
/// cell, left of the panel, level with the armour.
fn back_slot(screen: &mod_api::ScreenView<'_>) -> Option<[f32; 2]> {
    (matches!(screen.id, "inventory" | "creative") && screen.menu.is_none()).then(|| [screen.rect[0] - CELL - 4.0, screen.rect[1] + 7.0])
}

/// Draws the back slot on the inventory: the bag worn, or an empty cell.
pub fn draw_slot(draw: &mut dyn DrawContext, screen: &mod_api::ScreenView<'_>) {
    let Some([x, y]) = back_slot(screen) else { return };
    let mine = lock(&MINE).clone();
    draw.fill([x - 3.0, y - 3.0, CELL + 6.0, CELL + 6.0], 0xFF00_0000);
    draw.fill([x - 2.0, y - 2.0, CELL + 4.0, CELL + 4.0], 0xFFC6_C6C6);
    draw.fill([x, y, CELL, CELL], 0xFF37_3737);
    draw.fill([x + 1.0, y + 1.0, CELL - 1.0, CELL - 1.0], 0xFFFF_FFFF);
    draw.fill([x + 1.0, y + 1.0, CELL - 2.0, CELL - 2.0], 0xFF8B_8B8B);
    if let Some(stack) = &mine {
        draw.gui_draw_item_with(stack, [x + 1.0, y + 1.0], Some(""));
    }
    if over([x, y, CELL, CELL], screen.mouse) {
        draw.fill([x + 1.0, y + 1.0, CELL - 2.0, CELL - 2.0], 0x80FF_FFFF);
        draw.text(if mine.is_some() { "Back catpack" } else { "Back" }, [x, y - 12.0], 0xFFFF_FFFF, true);
    }
}

/// Whether the mouse is over the back slot.
pub fn over_slot(screen: &mod_api::ScreenView<'_>) -> bool {
    back_slot(screen).is_some_and(|[x, y]| over([x - 3.0, y - 3.0, CELL + 6.0, CELL + 6.0], screen.mouse))
}

fn over(rect: [f32; 4], mouse: [f32; 2]) -> bool {
    mouse[0] >= rect[0] && mouse[0] < rect[0] + rect[2] && mouse[1] >= rect[1] && mouse[1] < rect[1] + rect[3]
}

/// A left click on the inventory that is the back slot's. On the slot itself: the bag on the
/// cursor goes onto the back and the worn one onto the cursor (with shift and an empty cursor,
/// the worn one goes into the inventory). With shift on a bag in the inventory: that bag goes onto the back, the
/// worn one into its place. The server does the moving; true when the click was taken.
pub fn click_slot(host: &dyn ClientHost, screen: &mod_api::ScreenView<'_>, button: i32) -> bool {
    if back_slot(screen).is_none() || button != 0 {
        return false;
    }
    let shift = host.modifiers().shift();
    if over_slot(screen) {
        let carries = host.local_player().and_then(|player| player.container_menu(host)).and_then(|menu| menu.carried(host)).flatten().is_some();
        match (carries, shift) {
            (true, _) => *lock(&CURSOR_TO_BACK) = true,
            (false, true) => lock(&ASKS).push(message("off", [])),
            (false, false) => lock(&ASKS).push(message("slot", [])),
        }
        return true;
    }
    if !shift {
        return false;
    }
    let Some(menu) = host.local_player().and_then(|player| player.container_menu(host)) else { return false };
    let Some(kinds) = menu.slot_kinds(host) else { return false };
    // The slot under the mouse: by the game's own boxes, or, on the creative screen, which gives
    // none, by where its hotbar row lies (the same on every tab). There a shift-click destroys
    // the stack, so a bag is taken onto the back instead.
    let index = match menu.slot_boxes(host) {
        Some(boxes) => boxes.iter().position(|b| b.is_some_and(|b| over(b, screen.mouse))),
        None if screen.id == "creative" => (0..9u32)
            .find(|i| over([screen.rect[0] + 9.0 + *i as f32 * CELL, screen.rect[1] + 112.0, 16.0, 16.0], screen.mouse))
            .and_then(|i| kinds.iter().position(|kind| *kind == mod_api::MenuSlotKind::Player(i))),
        None => None,
    };
    let Some(index) = index else { return false };
    let Some(mod_api::MenuSlotKind::Player(slot)) = kinds.get(index) else { return false };
    let is_bag = menu.slot(host, index).flatten().is_some_and(|stack| stack.item().id(host) == crate::catpack::ITEM);
    if !is_bag || *slot >= 36 {
        return false;
    }
    lock(&ASKS).push(message("wear", [("s", Tag::Int(*slot as i32))]));
    true
}
