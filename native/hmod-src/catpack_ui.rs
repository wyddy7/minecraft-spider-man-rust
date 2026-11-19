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
            let mut all = lock(&BACKS);
            all.retain(|(id, ..)| *id != entity);
            if let Some(kind) = text(&told, "kind") {
                all.push((entity, kind, text(&told, "variant").unwrap_or_else(|| "red".into()), Instant::now()));
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

/// The client's own player uses an empty hand on nothing: sneaking, they ask for their bag off
/// their back; else they ask to open it.
pub fn right_click_empty(host: &dyn ClientHost) {
    let Some(player) = host.local_player() else { return };
    let entity = player.entity();
    if !wears(entity.id()) {
        return;
    }
    let sneaking = entity.is_shift_key_down(host) == Some(true);
    lock(&ASKS).push(message(if sneaking { "off" } else { "open" }, []));
}

/// Each client tick: what was asked goes to the server, and a window the player closed is
/// reported closed.
pub fn tick(host: &mut dyn ClientHost) {
    if lock(&WINDOW).is_some() && host.mod_screen() != Some(MOD) {
        *lock(&WINDOW) = None;
        lock(&ASKS).push(message("close", []));
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
    draw.text("Cat backpack", [ox + 8.0, oy + 6.0], 0xFF40_4040, false);
    draw.text("Inventory", [ox + 8.0, oy + 75.0], 0xFF40_4040, false);
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
    match hovered(size, mouse) {
        Some(Cell::Bag(slot)) => lock(&ASKS).push(message("take", [("s", Tag::Int(slot as i32))])),
        Some(Cell::Inventory(slot)) => lock(&ASKS).push(message("put", [("s", Tag::Int(slot as i32))])),
        None => {}
    }
    true
}
