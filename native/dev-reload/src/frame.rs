// Per-frame state the game's draw hooks share across its crates: the
// whole-view turn of the wall camera. Development only: a shipped mod has no
// crate both sides see, so it keeps the game's own view.

use std::sync::Mutex;

/// Whether the camera may turn the whole view (here: yes; a shipped mod: no).
pub const WALL_TURN: bool = true;

static VIEW_TURN: Mutex<Option<[f32; 4]>> = Mutex::new(None);

/// The camera orientation (x, y, z, w quaternion: camera axes to world)
/// replacing yaw and pitch while set.
pub fn set_view_turn(q: Option<[f32; 4]>) {
    if let Ok(mut t) = VIEW_TURN.lock() {
        *t = q;
    }
}

pub fn view_turn() -> Option<[f32; 4]> {
    VIEW_TURN.lock().ok().and_then(|t| *t)
}
