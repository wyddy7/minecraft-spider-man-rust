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
/// The open window is the worn bag's: it has "Take off".
static WINDOW_OF: Mutex<bool> = Mutex::new(false);
/// The key that opens the worn bag, as Controls lists it.
pub const KEY: &str = "catpack";
/// The player clicked the back slot; the next tick sees to it.
static SLOT_CLICK: Mutex<bool> = Mutex::new(false);
/// The click being made is the back slot's: this game's own answer to it swaps the cursor with
/// the back, as the server's will.
static PREDICT: Mutex<bool> = Mutex::new(false);
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
    // Single player only: taken only on the integrated connection this game owns, as it is at
    // this moment; a message from another server, or one arriving after the connection ended, is
    // dropped.
    match context.client_host() {
        Ok(host) if crate::integrated_connection(&*host) => {}
        _ => return,
    }
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
        "window" => {
            let Ok(host) = context.client_host() else { return };
            if matches!(told.get(&"open".into()), Some(Tag::Byte(0))) {
                *lock(&WINDOW) = None;
                host.mod_screen_close(MOD);
                return;
            }
            *lock(&WINDOW_OF) = matches!(told.get(&"back".into()), Some(Tag::Byte(1)));
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

/// Leaving a world or a connection: what the server said, the open window and what was waiting
/// to be asked are all forgotten.
pub fn forget() {
    lock(&BACKS).clear();
    *lock(&WINDOW) = None;
    *lock(&MINE) = None;
    *lock(&WINDOW_OF) = false;
    *lock(&SLOT_CLICK) = false;
    *lock(&PREDICT) = false;
    lock(&ASKS).clear();
}

/// Each client tick: what was asked goes to the server, and a window the player closed is
/// reported closed.
pub fn tick(host: &mut dyn ClientHost) {
    if lock(&WINDOW).is_some() && host.mod_screen() != Some(MOD) {
        *lock(&WINDOW) = None;
        lock(&ASKS).push(message("close", []));
    }
    if std::mem::take(&mut *lock(&CLOSE)) && host.mod_screen() == Some(MOD) {
        host.mod_screen_close(MOD);
    }
    // The bag's key opens the bag on the back; with shift it takes the bag off, into the hand.
    while host.consume_click(MOD, KEY) {
        if lock(&MINE).is_none() {
            continue;
        }
        if host.modifiers().shift() {
            lock(&ASKS).push(message("hand", []));
        } else {
            lock(&ASKS).push(message("open", []));
        }
    }
    if !crate::catpack::learnt() {
        if let Some(stack) = lock(&MINE).as_ref() {
            crate::catpack::learn(&*host, stack);
        }
        if let Some(stack) = host.local_player().and_then(|player| player.container_menu(&*host)).and_then(|menu| menu.carried(&*host)).flatten() {
            crate::catpack::learn(&*host, &stack);
        }
    }
    // What was asked so far goes first: a click on the back slot must reach the server after them.
    for ask in std::mem::take(&mut *lock(&ASKS)) {
        crate::catpack_net::ask(host, ask);
    }
    if std::mem::take(&mut *lock(&SLOT_CLICK))
        && let Some(menu) = host.local_player().and_then(|player| player.container_menu(&*host))
        && let Some(index) = menu.slot_kinds(&*host).and_then(|kinds| kinds.iter().position(|kind| matches!(kind, mod_api::MenuSlotKind::Player(_))))
    {
        // The swap is a click of the game's own, on any slot of the player's: the server is
        // told the next click is the back slot's, and both sides then answer that click
        // (`stacked_on_other`) by swapping the cursor with the back instead of the slot.
        crate::catpack_net::ask(host, message("slot", []));
        *lock(&PREDICT) = true;
        let sent = menu.click(host, &[mod_api::MenuClick::Pickup { slot: Some(index), button: mod_api::MouseButton::Left }]);
        if std::mem::take(&mut *lock(&PREDICT)) || !sent {
            crate::catpack_net::ask(host, message("noslot", []));
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
    if let Some((label, _)) = window_button() {
        let [bx, by, bw, bh] = take_off_button(size);
        let over_button = mouse[0] >= bx && mouse[0] < bx + bw && mouse[1] >= by && mouse[1] < by + bh;
        draw.fill([bx, by, bw, bh], 0xFF37_3737);
        draw.fill([bx + 1.0, by + 1.0, bw - 2.0, bh - 2.0], if over_button { 0xFFA0_A0A0 } else { 0xFF8B_8B8B });
        draw.text(label, [bx + (bw - draw.text_width(label)) / 2.0, by + 2.0], 0xFFFF_FFFF, true);
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
    if let Some((_, ask)) = window_button()
        && mouse[0] >= bx
        && mouse[0] < bx + bw
        && mouse[1] >= by
        && mouse[1] < by + bh
    {
        lock(&ASKS).push(message(ask, []));
        return true;
    }
    match hovered(size, mouse) {
        Some(Cell::Bag(slot)) => lock(&ASKS).push(message("take", [("s", Tag::Int(slot as i32))])),
        Some(Cell::Inventory(slot)) => lock(&ASKS).push(message("put", [("s", Tag::Int(slot as i32))])),
        None => {}
    }
    true
}

/// A hotbar key pressed with the mouse over the back slot swaps the back with that hotbar slot,
/// as over the off hand's slot. True when the key was the slot's.
pub fn key_on_slot(screen: &mod_api::ScreenView<'_>, key: &str) -> bool {
    if !over_slot(screen) {
        return false;
    }
    let Some(n) = key.strip_prefix("key.keyboard.").and_then(|n| n.parse::<i32>().ok()).filter(|n| (1..=9).contains(n)) else { return false };
    lock(&ASKS).push(message("swap", [("s", Tag::Int(n - 1))]));
    true
}

/// The bag's own key, pressed while its window is open, closes the window, as it opened it.
/// True when the key was the window's.
pub fn key_on_window(host: &dyn ClientHost, key: &str) -> bool {
    if host.mod_screen() != Some(MOD) || lock(&WINDOW).is_none() || host.mod_key(MOD, KEY).is_none_or(|(bound, _)| bound != key) {
        return false;
    }
    *lock(&CLOSE) = true;
    true
}

/// The window is to be closed with the next tick (a key hook can't close it itself).
static CLOSE: Mutex<bool> = Mutex::new(false);

/// The open window's one button, with what it asks of the server: "Take off" on the worn bag's
/// window, none on the window of a bag in the hand.
fn window_button() -> Option<(&'static str, &'static str)> {
    lock(&WINDOW_OF).then_some(("Take off", "off"))
}

/// The window's button: `[x, y, w, h]`.
fn take_off_button(size: [f32; 2]) -> [f32; 4] {
    let [ox, oy] = origin(size);
    [ox + WIDTH - 62.0, oy + 4.0, 54.0, 12.0]
}

/// The back slot beside the player's own inventory (survival's and creative's): `[x, y]` of its
/// cell, left of the panel, level with the armour.
fn back_slot(screen: &mod_api::ScreenView<'_>) -> Option<[f32; 2]> {
    (matches!(screen.id, "inventory" | "creative") && screen.menu.is_none()).then(|| [screen.rect[0] - CELL - 4.0, screen.rect[1] + 7.0])
}

/// Draws the back slot on the inventory: the bag worn, or an empty cell. Drawn with the screen's
/// background, so the stack on the cursor passes over it as over the game's own slots.
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
    }
}

/// A slot's well with what it holds, as the game draws its own.
fn draw_cell(draw: &mut dyn DrawContext, [x, y]: [f32; 2], stack: Option<&ItemStack>) {
    draw.fill([x, y, CELL, CELL], 0xFF37_3737);
    draw.fill([x + 1.0, y + 1.0, CELL - 1.0, CELL - 1.0], 0xFFFF_FFFF);
    draw.fill([x + 1.0, y + 1.0, CELL - 2.0, CELL - 2.0], 0xFF8B_8B8B);
    if let Some(stack) = stack {
        draw.gui_draw_item_with(stack, [x + 1.0, y + 1.0], Some(""));
        if stack.count() > 1 {
            let count = stack.count().to_string();
            draw.text(&count, [x + 17.0 - draw.text_width(&count), y + 9.0], 0xFFFF_FFFF, true);
        }
    }
}

/// How many rows of nine show all of what a bag holds: none for an empty one.
fn rows(held: &[(usize, ItemStack)]) -> usize {
    held.iter().map(|(slot, _)| slot / 9 + 1).max().unwrap_or(0)
}

/// What a bag holds, as its cells: the rows in use, nine to a row, from `at`.
fn draw_held(draw: &mut dyn DrawContext, held: &[(usize, ItemStack)], at: [f32; 2]) {
    for i in 0..rows(held) * 9 {
        let stack = held.iter().find(|(slot, _)| *slot == i).map(|(_, stack)| stack);
        draw_cell(draw, [at[0] + (i % 9) as f32 * CELL, at[1] + (i / 9) as f32 * CELL], stack);
    }
}

/// The bag's key as Controls names it, short: `G`.
fn key_name(host: &dyn ClientHost) -> String {
    let bound = host.mod_key(MOD, KEY).map_or_else(|| "key.keyboard.g".to_string(), |(key, _)| key);
    bound.rsplit('.').next().unwrap_or("g").to_uppercase()
}

/// The bag as an item: its tooltip says how to put it on and shows what it holds as cells.
pub struct BagPart;

#[mod_api::horizon_part]
impl mod_api::ItemPart for BagPart {
    fn tooltip(&self, _host: &dyn ClientHost, event: &mut mod_api::TooltipEvent<'_>) {
        use mod_api::Text;
        use mod_api::text::{TextContent, TextStyle};
        let mut gray = TextStyle::default();
        gray.color = Some("gray".into());
        let line = Text::literal("Press ")
            .append(Text::new(TextContent::Keybind("key.sneak".into())))
            .append(Text::literal("+"))
            .append(Text::new(TextContent::Keybind("key.use".into())))
            .append(Text::literal(" to wear"))
            .with_style(gray);
        event.lines.push((line, u32::MAX));
    }

    fn tooltip_component(&self, host: &dyn ClientHost, event: &mut mod_api::TooltipComponentEvent<'_>) -> Option<[f32; 2]> {
        let rows = rows(&crate::catpack_rules::stored(host, event.stack));
        (rows > 0).then(|| [9.0 * CELL, rows as f32 * CELL + 2.0])
    }

    fn tooltip_component_draw(&self, host: &dyn ClientHost, event: &mut mod_api::TooltipComponentDrawEvent<'_>) {
        draw_held(event.draw, &crate::catpack_rules::stored(host, event.stack), event.origin);
    }
}

/// The back slot's tooltip, over everything, while the mouse is on it: the worn bag's name, how
/// to take it off, and what it holds; an empty slot says what it is for.
pub fn draw_slot_label(host: &dyn ClientHost, draw: &mut dyn DrawContext, screen: &mod_api::ScreenView<'_>) {
    let Some([x, y]) = back_slot(screen) else { return };
    if !over([x, y, CELL, CELL], screen.mouse) {
        return;
    }
    let Some(stack) = lock(&MINE).clone() else {
        draw.text("Back", [x, y - 12.0], 0xFFFF_FFFF, true);
        return;
    };
    let held = crate::catpack_rules::stored(host, &stack);
    let hint = format!("Press Shift+{} to unwear", key_name(host));
    let grid = rows(&held) as f32 * CELL;
    let width = draw.text_width("Back catpack").max(draw.text_width(&hint)).max(if grid > 0.0 { 9.0 * CELL } else { 0.0 });
    let height = 22.0 + if grid > 0.0 { grid + 3.0 } else { 0.0 };
    // Beside the mouse, kept on the screen; the game's own tooltip colours.
    let size = draw.size();
    let left = (screen.mouse[0] + 12.0).min(size[0] - width - 6.0).max(4.0);
    let top = (screen.mouse[1] - 12.0).min(size[1] - height - 6.0).max(4.0);
    draw.fill([left - 4.0, top - 4.0, width + 8.0, height + 8.0], 0xF010_0010);
    draw.fill([left - 4.0, top - 4.0, width + 8.0, 1.0], 0x5050_00FF);
    draw.fill([left - 4.0, top + height + 3.0, width + 8.0, 1.0], 0x5028_007F);
    draw.fill([left - 4.0, top - 4.0, 1.0, height + 8.0], 0x5050_00FF);
    draw.fill([left + width + 3.0, top - 4.0, 1.0, height + 8.0], 0x5028_007F);
    draw.text("Back catpack", [left, top], 0xFFFF_FFFF, true);
    draw.text(&hint, [left, top + 12.0], 0xFFAA_AAAA, true);
    draw_held(draw, &held, [left, top + 25.0]);
}

/// This game's own answer to a pickup click (`item_stacked_on_other` on the client): the click
/// made for the back slot swaps the cursor with the worn bag, as the server answers it.
pub fn stacked_on_other(event: &mut mod_api::ItemStackedOnOtherEvent<'_, '_>) {
    if !std::mem::take(&mut *lock(&PREDICT)) {
        return;
    }
    event.cancelled = true;
    event.event.cancellation_result = true;
    let held = event.event.carried.get().cloned();
    if held.as_ref().is_some_and(|held| held.count() != 1 || !crate::catpack_rules::is_bag(held)) {
        return;
    }
    let worn = std::mem::replace(&mut *lock(&MINE), held);
    event.event.carried.set(worn);
}

/// Whether letting go of a mouse button here is the back slot's to swallow: over the slot, but
/// not on the creative screen, where the throw is the point.
pub fn swallows_release(screen: &mod_api::ScreenView<'_>) -> bool {
    screen.id != "creative" && over_slot(screen)
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
        let carried = host.local_player().and_then(|player| player.container_menu(host)).and_then(|menu| menu.carried(host)).flatten();
        if screen.id == "creative" {
            // A creative inventory keeps its cursor to itself and takes no click made for it.
            // The slot lies outside its panel, where a click throws what the cursor holds: a bag
            // is let go that way, and the server wears the bag thrown instead of dropping it.
            // With nothing on the cursor the worn bag goes into the inventory.
            return match carried {
                Some(stack) => {
                    if stack.count() == 1 && crate::catpack_rules::is_bag(&stack) {
                        lock(&ASKS).push(message("thrown", []));
                    }
                    false
                }
                None => {
                    lock(&ASKS).push(message("off", []));
                    true
                }
            };
        }
        if shift && carried.is_none() {
            lock(&ASKS).push(message("off", []));
        } else {
            *lock(&SLOT_CLICK) = true;
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
