// Whether the fall the own world's server is checking belongs to the world's
// owner while their web holds. Kept where the players' fall is checked (the
// motion module): set at the start of each player's turn on the server thread
// (`server_turn.rs`), read by the fall check (`fall.rs`). A thread-local, so
// the window's own prediction of the fall never sees it.

use std::cell::Cell;

thread_local! {
    static FORGET: Cell<bool> = const { Cell::new(false) };
}

pub fn set(forget: bool) {
    FORGET.with(|f| f.set(forget));
}

pub fn forget() -> bool {
    FORGET.with(Cell::get)
}
