// The game's side: load the plug, reload it when its file changes, call it.
//
// Each build is copied to a fresh name before loading (macOS may hand back a
// cached image for a reused path) and old builds are never unloaded, so
// nothing a previous build handed out can dangle. A plug's own state starts
// over on reload; the mod keeps what must survive in its storage.

use crate::abi::{Current, Input, Output, PLUG_SYMBOL, PlugCall, Ray};
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Instant, SystemTime};

unsafe extern "C" {
    fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlerror() -> *const c_char;
}

const RTLD_NOW: c_int = 2;
/// How often the plug's file is looked at.
const CHECK_EVERY_SECONDS: f32 = 0.25;

struct Host {
    call: Option<(SystemTime, PlugCall)>,
    checked: Option<Instant>,
    loads: u32,
    current: Current,
}

static HOST: Mutex<Host> = Mutex::new(Host {
    call: None,
    checked: None,
    loads: 0,
    current: Current {
        rays: Vec::new(),
        ropes: Vec::new(),
        pose: None,
        camera: None,
        wind: 0.0,
        panel: None,
    },
});

fn last_error() -> String {
    // SAFETY: dlerror returns null or a NUL-terminated string owned by libc.
    let message = unsafe { dlerror() };
    if message.is_null() {
        return "unknown error".into();
    }
    // SAFETY: checked non-null above.
    unsafe { CStr::from_ptr(message) }.to_string_lossy().into_owned()
}

impl Host {
    /// Loads the plug if its file is new; keeps the old one on any failure
    /// (a build half written) and tries again on the next check.
    fn refresh(&mut self) {
        if self.checked.is_some_and(|t| t.elapsed().as_secs_f32() < CHECK_EVERY_SECONDS) {
            return;
        }
        self.checked = Some(Instant::now());
        let Some(path) = std::env::var_os("HORIZON_DEV_PLUG").map(PathBuf::from) else {
            return;
        };
        let Some(modified) = std::fs::metadata(&path).and_then(|m| m.modified()).ok() else {
            return;
        };
        if self.call.is_some_and(|(seen, _)| seen == modified) {
            return;
        }
        let attempt = self.loads + 1;
        let copy = std::env::temp_dir().join(format!(
            "horizon-dev-plug-{}-{}.{}",
            std::process::id(),
            attempt,
            path.extension().and_then(|e| e.to_str()).unwrap_or("so")
        ));
        if let Err(error) = std::fs::copy(&path, &copy) {
            eprintln!("[dev-reload] copy failed: {error}");
            return;
        }
        let Ok(c_path) = CString::new(copy.to_string_lossy().as_bytes()) else {
            return;
        };
        // SAFETY: a NUL-terminated path to a plug built for this game.
        let handle = unsafe { dlopen(c_path.as_ptr(), RTLD_NOW) };
        if handle.is_null() {
            eprintln!("[dev-reload] load failed: {}", last_error());
            return;
        }
        // SAFETY: a live library and a NUL-terminated symbol name.
        let symbol = unsafe { dlsym(handle, PLUG_SYMBOL.as_ptr()) };
        if symbol.is_null() {
            eprintln!("[dev-reload] the plug has no {PLUG_SYMBOL:?}");
            return;
        }
        // SAFETY: plugs export this symbol with exactly the PlugCall signature.
        let call = unsafe { std::mem::transmute::<*mut c_void, PlugCall>(symbol) };
        self.call = Some((modified, call));
        self.loads = attempt;
        // A new build starts with nothing asked: the old one's ropes, pose and
        // camera don't linger.
        self.current = Current::default();
        eprintln!("[dev-reload] loaded plug build {}", self.loads);
    }
}

extern "C" fn collect(ctx: *mut c_void, bytes: *const u8, len: usize) {
    // SAFETY: ctx is the Vec passed to the plug just below, alive for the
    // call; bytes is valid for len bytes for the duration of this call.
    let out = unsafe { &mut *ctx.cast::<Vec<u8>>() };
    out.extend_from_slice(unsafe { std::slice::from_raw_parts(bytes, len) });
}

/// One tick of the plug: None while no plug is set up or loaded.
pub fn tick(input: &Input) -> Option<Output> {
    let mut host = HOST.lock().ok()?;
    host.refresh();
    let (_, call) = host.call?;
    let bytes = serde_json::to_vec(input).ok()?;
    let mut answer: Vec<u8> = Vec::new();
    // SAFETY: the input is valid for its length; `collect` only appends to
    // `answer`, which outlives the call.
    unsafe {
        call(
            bytes.as_ptr(),
            bytes.len(),
            collect,
            (&raw mut answer).cast::<c_void>(),
        )
    };
    let out: Output = match serde_json::from_slice(&answer) {
        Ok(out) => out,
        Err(error) => {
            // A panic in the plug (it starts over) or a broken build: forget
            // what it asked for, so no pose or rope stays frozen.
            eprintln!("[dev-reload] unreadable output: {error}");
            host.current = Current::default();
            return None;
        }
    };
    host.current.apply(&out);
    Some(out)
}

/// The rays the plug asked for, to cast before its next tick.
pub fn rays() -> Vec<Ray> {
    HOST.lock().map(|h| h.current.rays.clone()).unwrap_or_default()
}

/// The sticky requests so far, for per-frame hooks (pose, camera, ropes).
pub fn current() -> Current {
    HOST.lock().map(|h| h.current.clone()).unwrap_or_default()
}

/// How many plug builds were loaded.
pub fn loads() -> u32 {
    HOST.lock().map(|h| h.loads).unwrap_or(0)
}
