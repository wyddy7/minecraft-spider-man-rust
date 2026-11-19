//! Spider-Man as a native Horizon mod. Most of the mod runs inside the game's own functions (the
//! `[[native.mixin]]` rows and the `spider_man` module in `src/game/`). This crate holds what the
//! mod API offers on its own (the keys in Controls, the chat command, the Options page) and the
//! state those game-side copies share: they can call this crate, this crate can't call them, so
//! what the API hands the crate waits here until the game-side tick takes it.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use mod_api::{ClientHost, Flow, Hooks, KeyRegistration, Registrar, RegistrationContext, SettingSpec};

pub mod catpack;

/// The mod's keys: id, the label Controls shows, the default key.
pub const KEYS: [(&str, &str, &str); 2] = [
    ("web", "Web swing", "key.keyboard.r"),
    ("zip", "Web zip", "key.keyboard.z"),
];
/// The mod's chat commands, without the slash.
pub const COMMANDS: [&str; 1] = ["swingmode"];
/// The web delay row on the mod's page in Options > Mods.
const WEB_DELAY: &str = "web_delay";

/// Whether the player is in their own world (single player or a LAN world they host), as of the
/// game-side tick: only there does the mod take its commands.
static OWN_WORLD: AtomicBool = AtomicBool::new(false);

pub fn set_own_world(own: bool) {
    OWN_WORLD.store(own, Ordering::Relaxed);
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
        let delay = SettingSpec::range(WEB_DELAY, "Web delay", 0.0, 2.0, 0.25, 1.0)
            .tooltip("How long a web flies before it bites: 1 as in Spider-Man 2, 0 at once.");
        let _ = r.register_setting(delay);
    }

    /// The chat box lists, colours and completes the mod's commands.
    fn register_client_commands(
        &self,
        _host: &mut dyn ClientHost,
        event: &mut mod_api::RegisterClientCommandsEvent<'_>,
    ) {
        event.commands.push(mod_api::Command::new("swingmode"));
        event.commands.push(mod_api::Command::new("swingmode <args:text>"));
        event.commands.push(mod_api::Command::new("catpack"));
        event.commands.push(mod_api::Command::new("catpack <cat:text>"));
    }

    /// The kitten's model is baked once, when the client has started.
    fn client_setup(&self, host: &mut dyn ClientHost, _event: &mut mod_api::ClientSetupEvent) {
        catpack::bake(host);
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
        if let Some(args) = event.command.trim().strip_prefix("catpack")
            && (args.is_empty() || args.starts_with(' '))
        {
            let line = catpack::command(args);
            let _ = host.chat_line(mod_api::Text::literal(line));
            return Flow::Cancel;
        }
        if !is_command(event.command) || !OWN_WORLD.load(Ordering::Relaxed) {
            return Flow::Continue;
        }
        let command = event.command.trim();
        let (name, args) = command.split_once(' ').unwrap_or((command, ""));
        ran().push((name.to_owned(), args.trim().to_owned()));
        Flow::Cancel
    }

    /// A landing on the own world's server while the owner's web holds (`fall_gate`): no fall
    /// damage, as under Slow Falling, so a web shot after a long drop lands softly.
    fn living_fall(&self, _cx: &dyn mod_api::LoopContext, event: &mut mod_api::LivingFallEvent<'_>) -> Flow {
        if fall_gate::forget(event.entity) {
            event.distance = 0.0;
            return Flow::Cancel;
        }
        Flow::Continue
    }

    /// The web delay row changed: the mod takes it as a quiet `/swingmode delay`, which keeps it.
    /// Only the last of several steps (a drag, or changes on the title screen) is taken.
    fn setting_changed(&self, host: &mut dyn ClientHost, event: &mut mod_api::SettingChangedEvent<'_>) {
        if event.key != WEB_DELAY {
            return;
        }
        if let Some(delay) = host.setting_f64(WEB_DELAY).filter(|d| d.is_finite()) {
            let mut ran = ran();
            ran.retain(|(name, args)| !(name == "swingmode" && args.ends_with(" quiet")));
            ran.push(("swingmode".into(), format!("delay {delay:.2} quiet")));
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
