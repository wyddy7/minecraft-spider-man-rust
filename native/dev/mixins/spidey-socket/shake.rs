// The mod's camera shake, added to the other mods'.
fn plug_shake(game: &crate::game::Game<'_>, shake: &mut mod_api::Shake) {
    crate::game::spidey_socket::camera_shake(game, shake)
}
