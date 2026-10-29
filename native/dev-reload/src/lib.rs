// Development only. A native mod's game-side hooks hand the plug one `Input`
// a tick and apply the `Output` it returns; the plug is a library named by
// HORIZON_DEV_PLUG, loaded again whenever that file changes, so logic edits
// show up in a running game. Shipped mods compile their logic in instead.
//
// The data crosses as JSON: game and plug may be built minutes apart, and a
// field one side lacks then defaults instead of misreading memory. Changing
// these types means rebuilding the game.

pub mod abi;
/// The development game turns the whole view onto a wall (`frame`); the
/// plug reads this to know the view it gets is turned. A shipped mod's
/// stand-in says false.
pub const WALL_TURN: bool = true;
#[cfg(feature = "host")]
pub mod frame;

#[cfg(feature = "host")]
mod host;
#[cfg(feature = "host")]
pub use host::{current, loads, rays, tick};
