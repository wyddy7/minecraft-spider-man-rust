//! The cat backpack's messages between a server and its clients.
//!
//! The server owns what a player wears on the back and what a bag holds; a client only shows it
//! and asks. Two channels, each body one NBT compound with a `t` that names the message:
//!
//! - `spider_man:catpack_state`, to a client:
//!   - `back`: the entity `e` wears a bag that shows `kind` and `variant` (or, with no `kind`,
//!     wears none). Sent to everyone in the wearer's level when it changes, and again every
//!     couple of seconds, so a player who joins or comes near catches up.
//!   - `window`: the storage window, `open` or not, with what the bag (`items`) and the
//!     player's inventory (`inv`) hold, each entry a slot `s` and a stack `i`.
//! - `spider_man:catpack_ask`, to the server:
//!   - `off`: take my bag off my back; `open`: open the bag on my back;
//!   - `slot`: I clicked my back slot, and the pickup click I send next is that slot's (the bags
//!     on my cursor and my back change places); `noslot`: that click was not sent after all;
//!   - `wear` (inventory slot `s`): put that bag on my back, the worn one into its place;
//!   - `swap` (hotbar slot `s`): my back and that slot change places (a number key over the slot);
//!   - `thrown`: the bag I just threw, I threw onto my back slot (a creative inventory lets a
//!     stack off its cursor no other way): wear that one;
//!   - `put` (inventory slot `s`) and `take` (bag slot `s`): move a stack in the open window;
//!   - `close`: I closed the window.
//!
//! Both channels are optional: a peer without the mod still connects, and simply has no bag.
//! Nothing here trusts the client: an ask names slots only, and the server checks each against
//! what it holds itself.

use std::sync::OnceLock;

use mod_api::{Compound, PayloadId, Registrar, Tag, ToClient, ToServer};

static TO_CLIENT: OnceLock<ToClient<Vec<u8>>> = OnceLock::new();
static TO_SERVER: OnceLock<ToServer<Vec<u8>>> = OnceLock::new();

/// Declares the two channels. Called from the mod's `register`.
pub fn register(r: &mut dyn Registrar) {
    let mut events = r.payload_handlers();
    let mut channels = events.registrar("1").optional();
    let to_client = channels.play_to_client_typed::<Vec<u8>>(PayloadId("spider_man:catpack_state".into()), |bytes, context| {
        if let Some(message) = decode(&bytes) {
            crate::catpack_ui::heard(message, context);
        }
        Ok(())
    });
    let to_server = channels.play_to_server_typed::<Vec<u8>>(PayloadId("spider_man:catpack_ask".into()), |bytes, context| {
        if let Some(message) = decode(&bytes) {
            crate::catpack_rules::asked(message, context);
        }
        Ok(())
    });
    let _ = TO_CLIENT.set(to_client);
    let _ = TO_SERVER.set(to_server);
}

fn encode(message: Compound) -> Option<Vec<u8>> {
    Tag::Compound(message).to_network_bytes().ok()
}

fn decode(bytes: &[u8]) -> Option<Compound> {
    match Tag::from_network_bytes(bytes, None).ok()?.0 {
        Tag::Compound(message) => Some(message),
        _ => None,
    }
}

/// A message of `kind` with these fields.
pub fn message(kind: &str, fields: impl IntoIterator<Item = (&'static str, Tag)>) -> Compound {
    let mut message = Compound::default();
    message.insert("t".into(), Tag::String(kind.into()));
    for (key, value) in fields {
        message.insert(key.into(), value);
    }
    message
}

/// What kind of message this is.
pub fn kind(message: &Compound) -> String {
    match message.get(&"t".into()) {
        Some(Tag::String(s)) => s.to_string_lossy(),
        _ => String::new(),
    }
}

/// The server tells `player`. False for a player whose game has no such channel.
pub fn tell(host: &mut dyn mod_api::ServerHost, player: mod_api::PlayerRef, message: Compound) -> bool {
    let (Some(channel), Some(bytes)) = (TO_CLIENT.get(), encode(message)) else { return false };
    channel.send(host, player, &bytes).is_ok()
}

/// The client asks its server. False where the server has no such channel.
pub fn ask(host: &mut dyn mod_api::ClientHost, message: Compound) -> bool {
    let (Some(channel), Some(bytes)) = (TO_SERVER.get(), encode(message)) else { return false };
    channel.send(host, &bytes).is_ok()
}
