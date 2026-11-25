//! Spider-Man as a native Horizon mod. Most of the mod runs inside the game's own functions (the
//! `[[native.mixin]]` rows and the `spider_man` module in `src/game/`). This crate holds what the
//! mod API offers on its own (the keys in Controls, the chat command, the Options page) and the
//! state those game-side copies share: they can call this crate, this crate can't call them, so
//! what the API hands the crate waits here until the game-side tick takes it.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use mod_api::{ClientHost, Flow, Hooks, KeyRegistration, Registrar, RegistrationContext, SettingSpec};

pub mod catpack;
pub mod catpack_net;
pub mod catpack_rules;
pub mod catpack_ui;

/// The mod's keys: id, the label Controls shows, the default key.
pub const KEYS: [(&str, &str, &str); 2] = [
    ("web", "Web swing", "key.keyboard.r"),
    ("zip", "Web zip", "key.keyboard.z"),
];
/// The mod's chat commands, without the slash.
pub const COMMANDS: [&str; 1] = ["swingmode"];
/// The web delay row on the mod's page in Options > Mods.
const WEB_DELAY: &str = "web_delay";
/// The camera distance row: the web mode's chase camera on the ground, in blocks.
const CAMERA: &str = "camera_distance";
/// The long-flight speed bonus row: how far past the usual top speed a long flight may go, in percent.
const SPEED: &str = "long_flight_speed";
const DEFAULT_SPEED: f64 = 30.0;
/// The cat backpack's cartoon eyes row: off (the game's own face, the default) or on.
const EYES: &str = "cat_eyes";
/// The row that puts the others back to their defaults, then turns itself off.
const RESET: &str = "reset";
/// The defaults: half the reference web flight (the mod's own default since 0.1), the camera 6 blocks back.
const DEFAULT_WEB_DELAY: f64 = 0.5;
const DEFAULT_CAMERA: f64 = 6.0;
/// The mod's id, as its settings are kept under.
const MOD_ID: &str = "spider_man";

/// Whether the player is in their own world (single player or a LAN world they host), as of the
/// game-side tick: only there does the mod take its commands.
static OWN_WORLD: AtomicBool = AtomicBool::new(false);

pub fn set_own_world(own: bool) {
    OWN_WORLD.store(own, Ordering::Relaxed);
}

/// Whether this game plays its own world now (client side): the cat backpack's client hooks do
/// nothing anywhere else.
pub fn own_world() -> bool {
    OWN_WORLD.load(Ordering::Relaxed)
}

/// The world's owner as this game's client knows them: the local player's UUID while they play
/// their own world, else none. The cat backpack is single player only: on the world's own
/// server its rules act for this one player and no one else (a guest of a LAN world, any
/// player of a dedicated server).
static OWNER: Mutex<Option<u128>> = Mutex::new(None);
/// Whether the server running in this process is the game's own (integrated) one, as of its
/// `server_starting`; false for a dedicated server and once it has stopped.
static OWN_SERVER: AtomicBool = AtomicBool::new(false);

fn owner() -> Option<u128> {
    *OWNER.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Whether the connection this client has right now is to the integrated server this game
/// owns (asked of the game at the moment of the call, not remembered from a tick): the cat
/// backpack's messages are sent and taken only then, and only while `own_world` holds.
pub fn integrated_connection(host: &dyn ClientHost) -> bool {
    own_world() && host.client_session().client_server().is_some_and(|server| server.integrated_local)
}

/// The connection ends or the level goes: from this moment the client is in no own world, its
/// player is no one's owner, and what it knew of bags is forgotten. The next tick in an own world
/// sets them again.
fn teardown() {
    set_own_world(false);
    *OWNER.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    catpack::forget();
    catpack_ui::forget();
}

/// Whether the server-side cat backpack may act for `player`: the server is this game's own and
/// `player` is the one playing it here.
pub fn own_player(player: u128) -> bool {
    OWN_SERVER.load(Ordering::Relaxed) && owner() == Some(player)
}

/// The owner's UUID, when the server is this game's own and its owner is known.
pub fn own_server_owner() -> Option<u128> {
    OWN_SERVER.load(Ordering::Relaxed).then(owner).flatten()
}

/// Commands the player ran (name, arguments), for the game-side tick to hand to the mod.
static RAN: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

fn ran() -> std::sync::MutexGuard<'static, Vec<(String, String)>> {
    RAN.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The commands run since the last call, oldest first.
pub fn take_commands() -> Vec<(String, String)> {
    std::mem::take(&mut *ran())
}

/// Whether `command` (without the slash) is one of the mod's.
pub fn is_command(command: &str) -> bool {
    let name = command.trim().split(' ').next().unwrap_or("");
    COMMANDS.contains(&name)
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _cx: &RegistrationContext) {
        for (id, label, key) in KEYS {
            r.register_key_mapping(KeyRegistration::new(id, key).label(label));
        }
        r.register_key_mapping(KeyRegistration::new(catpack_ui::KEY, "key.keyboard.g").label("Back catpack"));
        catpack_net::register(r);
        let bag = r.item_key(catpack::ITEM);
        // The registered item is what makes a stack a bag; no other item's data does.
        catpack::set_item_key(bag);
        r.item_part(bag, catpack_ui::BagPart);
        let delay = SettingSpec::range(WEB_DELAY, "Web delay", 0.0, 2.0, 0.25, DEFAULT_WEB_DELAY)
            .tooltip("How long a web flies before it bites: 1 as in Spider-Man 2, 0 at once.");
        let _ = r.register_setting(delay);
        let camera = SettingSpec::range(CAMERA, "Camera distance", 2.0, 12.0, 0.5, DEFAULT_CAMERA)
            .tooltip("How far behind the player the camera stays in web mode, in blocks; it backs off a little more in flight.");
        let _ = r.register_setting(camera);
        let speed = SettingSpec::range(SPEED, "Long-flight speed bonus %", 0.0, 200.0, 10.0, DEFAULT_SPEED)
            .tooltip("After 3 s in the air the top speed rises over 5 s by this much; landing resets it. 0 = the usual top speed.");
        let _ = r.register_setting(speed);
        let eyes = SettingSpec::choice(EYES, "Cartoon cat eyes", ["off", "on"], 0)
            .tooltip("The cat in the backpack gets big cartoon eyes in its coat's colours, which blink, glance and show how it feels.");
        let _ = r.register_setting(eyes);
        // The page has no button row, so the reset is a row with one thing to click: it reads
        // "click here", does its work on the click and reads "done" until the page is opened again.
        let reset = SettingSpec::choice(RESET, "Reset all to defaults", ["click here", "done"], 0)
            .tooltip("Click to put every setting above back to its default.");
        let _ = r.register_setting(reset);
    }

    /// The mod's own items: the cat backpack.
    fn items(&self, _cx: &RegistrationContext, event: &mut mod_api::ItemsEvent<'_>) {
        event.items.push(catpack::item());
    }

    /// The chat box lists, colours and completes the mod's commands.
    fn register_client_commands(
        &self,
        _host: &mut dyn ClientHost,
        event: &mut mod_api::RegisterClientCommandsEvent<'_>,
    ) {
        event.commands.push(mod_api::Command::new("swingmode"));
        event.commands.push(mod_api::Command::new("swingmode <args:text>"));
    }

    /// The kitten's model is baked once, when the client has started.
    fn client_setup(&self, host: &mut dyn ClientHost, _event: &mut mod_api::ClientSetupEvent) {
        catpack::bake(host);
    }

    /// Which cat backpack the local player wears, from their chest slot.
    fn client_tick_post(&self, host: &mut dyn ClientHost, _event: &mut mod_api::ClientTickPostEvent) {
        let now = if integrated_connection(host) { host.local_player().map(|p| p.uuid()) } else { None };
        let changed = {
            let mut known = OWNER.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::replace(&mut *known, now) != now
        };
        // Another world, another connection or none: what the client knew of the last one goes.
        if changed {
            catpack::forget();
            catpack_ui::forget();
        }
        if now.is_none() {
            return;
        }
        catpack::set_eyes(host.setting_choice(EYES) == Some(1));
        catpack::tick(host);
        catpack_ui::tick(host);
    }

    /// The world's own server starts: the cat backpack acts on it only when it is this game's
    /// own, and starts with nothing remembered from an earlier world.
    fn server_starting(&self, _host: &mut dyn mod_api::ServerHost, event: &mut mod_api::ServerStartingEvent) {
        catpack_rules::forget_all();
        OWN_SERVER.store(!event.dedicated, Ordering::Relaxed);
    }

    /// The server has stopped: nothing of its players is kept for the next world.
    fn server_stopped(&self, _host: &mut dyn mod_api::ServerHost, _event: &mut mod_api::ServerStoppedEvent) {
        OWN_SERVER.store(false, Ordering::Relaxed);
        catpack_rules::forget_all();
    }

    /// A player leaves: a back changed inside a click is saved while they are still listed,
    /// then whatever the server held for them in memory goes.
    fn player_logged_out(&self, host: &mut dyn mod_api::ServerHost, event: &mut mod_api::PlayerLoggedOutEvent<'_>) {
        catpack_rules::logged_out(host, event.player);
    }

    fn client_player_logging_out(&self, _host: &mut dyn ClientHost, _event: &mut mod_api::ClientPlayerLoggingOutEvent) {
        teardown();
    }

    fn client_disconnecting(&self, _host: &dyn ClientHost, _event: &mut mod_api::ClientDisconnectingEvent) {
        teardown();
    }

    fn client_level_unload(&self, _host: &mut dyn ClientHost, _event: &mut mod_api::ClientLevelUnloadEvent<'_>) {
        teardown();
    }

    /// The cat backpack draws over both player models.
    fn register_entity_layers(
        &self,
        _context: &RegistrationContext,
        event: &mut mod_api::RegisterEntityLayersEvent<'_>,
    ) {
        catpack::register_layers(event);
    }

    /// `/swingmode …` goes to the mod, not the server, in the player's own world only; elsewhere
    /// the server gets it as always.
    fn command(&self, host: &mut dyn ClientHost, event: &mut mod_api::CommandEvent<'_>) -> Flow {
        if !is_command(event.command) || !OWN_WORLD.load(Ordering::Relaxed) {
            return Flow::Continue;
        }
        let command = event.command.trim();
        let (name, args) = command.split_once(' ').unwrap_or((command, ""));
        // `/swingmode eyes on|off` sets the row (the setting keeps it); the mod's reply comes from
        // the game-side tick as for every other `/swingmode`.
        match args.trim() {
            "eyes on" => host.set_setting(MOD_ID, EYES, "1"),
            "eyes off" => host.set_setting(MOD_ID, EYES, "0"),
            _ => {}
        }
        ran().push((name.to_owned(), args.trim().to_owned()));
        Flow::Cancel
    }

    /// A landing on the own world's server while the owner's web holds (`fall_gate`): no fall
    /// damage, as under Slow Falling, so a web shot after a long drop lands softly.
    fn entity_interact(&self, host: mod_api::HostSide, event: &mut mod_api::EntityInteractEvent<'_>) -> Option<mod_api::InteractionResult> {
        catpack_rules::entity_interact(host, event)
    }

    fn use_item_on_block(&self, host: mod_api::HostSide, event: &mut mod_api::UseItemOnBlockEvent<'_>) -> Flow {
        if let Some(result) = catpack_rules::use_item_on_block(host, event) {
            *event.cancellation_result = result;
            event.cancelled = true;
        }
        Flow::Continue
    }

    fn level_tick_post(&self, host: mod_api::HostSide, event: &mut mod_api::LevelTickPostEvent) {
        catpack_rules::level_tick(host, event);
    }

    fn right_click_item(&self, host: mod_api::HostSide, event: &mut mod_api::RightClickItemEvent<'_>) -> Flow {
        if catpack_rules::right_click_item(host, event) {
            *event.cancellation_result = mod_api::InteractionResult::Consume;
            event.cancelled = true;
        }
        Flow::Continue
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut mod_api::ScreenRenderPostEvent<'_>) {
        if !own_world() {
            return;
        }
        catpack_ui::draw(host, event.draw, event.screen.mouse);
        catpack_ui::draw_slot_label(host, event.draw, event.screen);
    }

    fn screen_render_background(&self, _host: &dyn ClientHost, event: &mut mod_api::ScreenRenderBackgroundEvent<'_>) {
        if !own_world() {
            return;
        }
        catpack_ui::draw_slot(event.draw, event.screen);
    }

    fn item_toss(&self, host: &mut dyn mod_api::ServerHost, event: &mut mod_api::ItemTossEvent) -> Flow {
        if catpack_rules::item_toss(host, event) { Flow::Cancel } else { Flow::Continue }
    }

    fn screen_key_pressed_pre(&self, host: &dyn ClientHost, event: &mut mod_api::ScreenKeyPressedPreEvent<'_>) -> Flow {
        if own_world() && !event.repeat && (catpack_ui::key_on_window(host, event.key) || catpack_ui::key_on_slot(event.screen, event.key)) { Flow::Cancel } else { Flow::Continue }
    }

    fn render_hand(&self, host: &dyn ClientHost, event: &mut mod_api::RenderHandEvent<'_, '_>) -> Flow {
        if own_world() && catpack::render_hand(host, event) { Flow::Cancel } else { Flow::Continue }
    }

    fn item_stacked_on_other(&self, context: &dyn mod_api::LoopContext, event: &mut mod_api::ItemStackedOnOtherEvent<'_, '_>) -> Flow {
        match context.side() {
            mod_api::LoopSide::Server => catpack_rules::stacked_on_other(event),
            mod_api::LoopSide::Client if own_world() => catpack_ui::stacked_on_other(event),
            _ => {}
        }
        Flow::Continue
    }

    fn screen_mouse_button_released_pre(&self, _host: &dyn ClientHost, event: &mut mod_api::ScreenMouseButtonReleasedPreEvent<'_>) -> Flow {
        // The back slot lies outside the inventory's panel, where letting go of a button throws
        // what the cursor holds.
        if own_world() && catpack_ui::swallows_release(event.screen) {
            event.cancelled = true;
        }
        Flow::Continue
    }

    fn screen_mouse_button_pressed_pre(&self, host: &dyn ClientHost, event: &mut mod_api::ScreenMouseButtonPressedPreEvent<'_>) -> Flow {
        if own_world() && (catpack_ui::click(host, event.screen.mouse, event.button) || catpack_ui::click_slot(host, event.screen, event.button)) {
            event.cancelled = true;
        }
        Flow::Continue
    }

    /// A dropped cat backpack never despawns.
    fn item_expire(&self, _host: &mut dyn mod_api::ServerHost, event: &mut mod_api::ItemExpireEvent<'_>) {
        catpack_rules::item_expire(event);
    }

    fn living_death(&self, host: &mut dyn mod_api::ServerHost, event: &mut mod_api::LivingDeathEvent<'_>) -> Flow {
        catpack_rules::living_death(host, event);
        Flow::Continue
    }

    fn living_fall(&self, _cx: &dyn mod_api::LoopContext, event: &mut mod_api::LivingFallEvent<'_>) -> Flow {
        if fall_gate::forget(event.entity) {
            event.distance = 0.0;
            return Flow::Cancel;
        }
        Flow::Continue
    }

    /// A row changed (web delay, camera distance): the mod takes it as a quiet `/swingmode delay` or
    /// `/swingmode camera`, which keeps it. The reset row, clicked, writes both defaults back (the rows
    /// show them the next time the page opens) and is ready to be clicked again.
    /// Only the last of several steps (a drag, or changes on the title screen) is taken.
    fn setting_changed(&self, host: &mut dyn ClientHost, event: &mut mod_api::SettingChangedEvent<'_>) {
        if event.key == RESET {
            if host.setting_choice(RESET) == Some(1) {
                host.set_setting(MOD_ID, WEB_DELAY, &DEFAULT_WEB_DELAY.to_string());
                host.set_setting(MOD_ID, CAMERA, &DEFAULT_CAMERA.to_string());
                host.set_setting(MOD_ID, SPEED, &DEFAULT_SPEED.to_string());
                host.set_setting(MOD_ID, EYES, "0");
                host.set_setting(MOD_ID, RESET, "0");
                let mut ran = ran();
                ran.retain(|(name, args)| !(name == "swingmode" && args.ends_with(" quiet")));
                ran.push(("swingmode".into(), format!("delay {DEFAULT_WEB_DELAY:.2} quiet")));
                ran.push(("swingmode".into(), format!("camera {DEFAULT_CAMERA:.2} quiet")));
                ran.push(("swingmode".into(), format!("speed {DEFAULT_SPEED:.0} quiet")));
            }
            return;
        }
        let command = match event.key {
            WEB_DELAY => "delay",
            CAMERA => "camera",
            SPEED => "speed",
            _ => return,
        };
        if let Some(value) = host.setting_f64(event.key).filter(|v| v.is_finite()) {
            let mut ran = ran();
            ran.retain(|(name, args)| !(name == "swingmode" && args.starts_with(command) && args.ends_with(" quiet")));
            ran.push(("swingmode".into(), format!("{command} {value:.2} quiet")));
        }
    }
}

/// The wall camera's turn of the whole view. Set by the game's camera each frame for the yaw and
/// pitch it renders from, read where Horizon's render crate builds the view matrix. It lives here
/// because a module can only go into the game crate, and both sides can call this crate.
pub mod view_turn {
    use std::sync::Mutex;

    /// The yaw and pitch the frame renders from, and the turned orientation (x, y, z, w: camera
    /// axes to world) to show for them.
    static TURN: Mutex<Option<(f32, f32, [f32; 4])>> = Mutex::new(None);

    pub fn set(turn: Option<(f32, f32, [f32; 4])>) {
        *TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = turn;
    }

    /// The turned orientation for exactly the frame's angles; any other view the game builds
    /// (previews, other cameras) stays its own.
    pub fn get(yaw: f32, pitch: f32) -> Option<[f32; 4]> {
        let (y, p, q) = (*TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner))?;
        (y.to_bits() == yaw.to_bits() && p.to_bits() == pitch.to_bits()).then_some(q)
    }
}

/// The own world's owner, while their web holds: their player's entity id on the server, set at
/// the start of each player's turn on the server thread (None on any other player's turn), read
/// by `living_fall`. Only that entity's fall is forgotten: a mob landing in the same tick is
/// hurt as always. A thread-local, so the window's own prediction of the fall never sees it.
pub mod fall_gate {
    use std::cell::Cell;

    thread_local! {
        static OWNER: Cell<Option<u32>> = const { Cell::new(None) };
    }

    pub fn set(owner: Option<u32>) {
        OWNER.with(|o| o.set(owner));
    }

    /// Whether `entity`'s fall is forgotten now.
    pub fn forget(entity: u32) -> bool {
        OWNER.with(Cell::get) == Some(entity)
    }
}
