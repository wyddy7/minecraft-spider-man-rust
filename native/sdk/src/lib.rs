// The Horizon mod SDK's client surface, as far as our mods use it, served
// from the tick's `Input` and gathered into its `Output` instead of a
// WebAssembly host. A mod written for the SDK builds unchanged as a plug.
// Reads see this tick's snapshot; sets apply after the tick, as in the SDK.

use horizon_dev_reload::abi::{Input, Output, Set};
use std::cell::RefCell;

thread_local! {
    static INPUT: RefCell<Input> = RefCell::new(Input::default());
    static OUTPUT: RefCell<Output> = RefCell::new(Output::default());
    /// Physical keys held last tick, to turn into key events.
    static HELD: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// The mod's registered keys: name and default physical key.
    static KEYS: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
    static INIT_FAILED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn input<T>(f: impl FnOnce(&Input) -> T) -> T {
    INPUT.with_borrow(f)
}

fn output(f: impl FnOnce(&mut Output)) {
    OUTPUT.with_borrow_mut(f)
}

/// `key.keyboard.r` → `KeyR`, `key.keyboard.space` → `Space`.
fn physical(key: &str) -> String {
    let name = key.strip_prefix("key.keyboard.").unwrap_or(key);
    match name {
        "space" => "Space".into(),
        "left.shift" => "ShiftLeft".into(),
        "left.control" => "ControlLeft".into(),
        n if n.len() == 1 && n.chars().all(|c| c.is_ascii_alphabetic()) => {
            format!("Key{}", n.to_ascii_uppercase())
        }
        n if n.len() == 1 && n.chars().all(|c| c.is_ascii_digit()) => format!("Digit{n}"),
        n => n.into(),
    }
}

pub trait Mod: Sized + 'static {
    fn init() -> Result<(Self, Registration), String>;
    fn tick(&mut self, _tick: u64) {}
    fn command(&mut self, _name: &str, _args: &str) -> Result<Option<String>, String> {
        Ok(None)
    }
    fn ui_select(&mut self, _id: u32, _row: u32) {}
    fn ui_closed(&mut self) {}
    fn ui_settings_opened(&mut self) {}
}

#[derive(Clone, Debug, Default)]
pub struct Registration {
    pub commands: Vec<String>,
    pub tick: bool,
    pub frame: bool,
    pub chat: bool,
}

impl Registration {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn command(mut self, name: &str) -> Self {
        self.commands.push(name.into());
        self
    }
    pub fn on_tick(mut self) -> Self {
        self.tick = true;
        self
    }
    pub fn on_frame(mut self) -> Self {
        self.frame = true;
        self
    }
    pub fn on_chat(mut self) -> Self {
        self.chat = true;
        self
    }
}

pub fn info(message: &str) {
    log::write(log::Level::Info, message);
}

pub fn warn(message: &str) {
    log::write(log::Level::Warn, message);
}

pub mod log {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Level {
        Debug,
        Info,
        Warn,
        Error,
    }
    pub fn write(level: Level, message: &str) {
        eprintln!("[plug {level:?}] {message}");
    }
}

pub mod chat {
    pub fn show(text: &str) {
        super::output(|o| o.chat.push(text.into()));
    }
}

pub mod game {
    pub use horizon_dev_reload::abi::Player;
    pub fn get_player() -> Option<Player> {
        super::input(|i| i.player.clone())
    }
}

pub mod motion {
    use super::{Set, output};
    pub use horizon_dev_reload::abi::{
        Camera, ChaseCamera, FixedCamera, PartPose, Pose, Ray, Rope, State, Vec3,
    };
    /// The game can turn the whole view onto a wall (the development
    /// build); a shipped mod keeps the game's own upright view.
    pub const WALL_TURN: bool = horizon_dev_reload::WALL_TURN;
    pub fn get() -> Option<State> {
        super::input(|i| i.state.clone())
    }
    pub fn set_velocity(v: Vec3) {
        output(|o| o.velocity = Some(v));
    }
    pub fn reset_fall() {
        output(|o| o.reset_fall = true);
    }
    pub fn set_ropes(ropes: &[Rope]) {
        output(|o| o.ropes = Set::To(ropes.to_vec()));
    }
    pub fn set_rays(rays: &[Ray]) {
        output(|o| o.rays = Set::To(rays.to_vec()));
    }
    pub fn set_pose(
        parts: &[PartPose],
        pitch: f32,
        roll: f32,
        twist: f32,
        facing: Option<f32>,
        shift: Option<Vec3>,
    ) {
        let pose = Pose {
            parts: parts.to_vec(),
            pitch,
            roll,
            twist,
            facing,
            shift,
        };
        output(|o| o.pose = Set::To(Some(pose)));
    }
    pub fn clear_pose() {
        output(|o| o.pose = Set::To(None));
    }
    pub fn set_wind(volume: f32) {
        output(|o| o.wind = Set::To(volume));
    }
    pub fn set_camera(camera: Option<Camera>) {
        output(|o| o.camera = Set::To(camera));
    }
}

pub mod render {
    pub fn shake(strength: f32, seconds: f32) {
        super::output(|o| o.shake = Some((strength, seconds)));
    }
}

pub mod hud {
    use super::{Set, output};
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Corner {
        TopLeft,
        TopRight,
        BottomLeft,
        BottomRight,
    }
    #[derive(Clone, Debug, PartialEq)]
    pub struct Line {
        pub text: String,
        pub color: u32,
    }
    pub fn set_panel(_corner: Corner, lines: &[Line]) {
        let lines = lines.iter().map(|l| (l.text.clone(), l.color)).collect();
        output(|o| o.panel = Set::To(Some(lines)));
    }
    pub fn clear() {
        output(|o| o.panel = Set::To(None));
    }
}

pub mod input {
    #[derive(Clone, Debug, PartialEq)]
    pub enum Event {
        KeyDown(String),
        KeyUp(String),
        /// Never sent through the socket yet.
        Wheel(f32),
    }
    #[derive(Clone, Debug, PartialEq)]
    pub struct MappingState {
        pub label: String,
        pub down: bool,
    }
    /// Keys use their default binding: the plug has no Controls rows yet.
    pub fn register_key(name: &str, _label: &str, key: &str) -> Result<(), String> {
        super::KEYS.with_borrow_mut(|keys| {
            keys.retain(|k| k.0 != name);
            keys.push((name.into(), super::physical(key)));
        });
        Ok(())
    }
    /// This tick's presses and releases of the registered keys.
    pub fn poll() -> Vec<Event> {
        let now = super::input(|i| i.keys.clone());
        let before = super::HELD.with_borrow_mut(|held| std::mem::replace(held, now.clone()));
        super::KEYS.with_borrow(|keys| {
            let mut events = Vec::new();
            for (name, code) in keys {
                match (before.contains(code), now.contains(code)) {
                    (false, true) => events.push(Event::KeyDown(name.clone())),
                    (true, false) => events.push(Event::KeyUp(name.clone())),
                    _ => {}
                }
            }
            events
        })
    }
    /// The vanilla movement mappings, on their default keys.
    pub fn mapping(name: &str) -> Option<MappingState> {
        let code = match name {
            "key.forward" => "KeyW",
            "key.left" => "KeyA",
            "key.back" => "KeyS",
            "key.right" => "KeyD",
            "key.jump" => "Space",
            "key.sneak" => "ShiftLeft",
            "key.sprint" => "ControlLeft",
            _ => return None,
        };
        Some(MappingState {
            label: name.into(),
            down: super::input(|i| i.keys.iter().any(|k| k == code)),
        })
    }
}

pub mod storage {
    use std::path::PathBuf;
    /// Beside the plug while developing, else in the game's folder: survives
    /// reloads and restarts.
    fn path(key: &str) -> Option<PathBuf> {
        let dir = match std::env::var_os("HORIZON_DEV_PLUG") {
            Some(plug) => PathBuf::from(plug).parent()?.join("plug-storage"),
            None => std::env::current_dir().ok()?.join("mod-storage").join("spider-man"),
        };
        let safe: String = key
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        Some(dir.join(safe))
    }
    pub fn get(key: &str) -> Option<Vec<u8>> {
        std::fs::read(path(key)?).ok()
    }
    pub fn set(key: &str, value: &[u8]) -> Result<(), String> {
        let path = path(key).ok_or("no folder for the settings")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(path, value).map_err(|e| e.to_string())
    }
    pub fn delete(key: &str) {
        if let Some(path) = path(key) {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub mod ui {
    #[derive(Clone, Debug, PartialEq)]
    pub struct TextWidget {
        pub id: u32,
        pub text: String,
        pub color: u32,
    }
    #[derive(Clone, Debug, PartialEq)]
    pub struct ButtonWidget {
        pub id: u32,
        pub label: String,
    }
    #[derive(Clone, Debug, PartialEq)]
    pub enum Widget {
        Text(TextWidget),
        Button(ButtonWidget),
    }
    /// No settings page through the socket yet.
    pub fn set_settings(_widgets: &[Widget]) {}
}

/// Runs one tick of the mod `T`, its first after a (re)load starting with
/// `init`.
#[doc(hidden)]
pub fn run_typed<T: Mod>(slot: &RefCell<Option<T>>, input: &Input) -> Output {
    let tick = input.tick;
    let commands = input.commands.clone();
    INPUT.with_borrow_mut(|i| *i = input.clone());
    OUTPUT.with_borrow_mut(|o| *o = Output::default());
    let mut slot = slot.borrow_mut();
    if slot.is_none() {
        match T::init() {
            Ok((m, _registration)) => *slot = Some(m),
            // Tried again next tick; said once.
            Err(e) if !INIT_FAILED.replace(true) => log::write(log::Level::Error, &e),
            Err(_) => {}
        }
    }
    if let Some(m) = slot.as_mut() {
        for (name, args) in commands {
            match m.command(&name, &args) {
                Ok(Some(reply)) => chat::show(&reply),
                Ok(None) => {}
                Err(e) => chat::show(&e),
            }
        }
        m.tick(tick);
    }
    OUTPUT.with_borrow_mut(std::mem::take)
}

// dev-only:begin (a shipped mod calls run_typed directly; no library boundary)
/// `run_typed` across the plug's boundary: JSON in, JSON out.
#[doc(hidden)]
pub fn run_tick<T: Mod>(slot: &RefCell<Option<T>>, input: &[u8]) -> Vec<u8> {
    let input: Input = serde_json::from_slice(input).unwrap_or_default();
    serde_json::to_vec(&run_typed(slot, &input)).unwrap_or_default()
}

/// Exports the plug's entry for the mod type.
#[macro_export]
macro_rules! export_mod {
    ($ty:ty) => {
        thread_local! {
            static __MOD: ::std::cell::RefCell<Option<$ty>> = const { ::std::cell::RefCell::new(None) };
        }
        /// # Safety
        /// `input` is valid for `input_len` bytes for the call.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn horizon_plug_call(
            input: *const u8,
            input_len: usize,
            write: extern "C" fn(*mut ::core::ffi::c_void, *const u8, usize),
            ctx: *mut ::core::ffi::c_void,
        ) {
            let input = if input.is_null() {
                &[][..]
            } else {
                // SAFETY: the caller guarantees the input range.
                unsafe { ::std::slice::from_raw_parts(input, input_len) }
            };
            // A panic must not cross into the game: the mod starts over and
            // this tick has no output (the game forgets the mod's requests).
            let out = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                __MOD.with(|slot| $crate::run_tick::<$ty>(slot, input))
            }))
            .unwrap_or_else(|_| {
                __MOD.with(|slot| slot.replace(None));
                Vec::new()
            });
            write(ctx, out.as_ptr(), out.len());
        }
    };
}
// dev-only:end
