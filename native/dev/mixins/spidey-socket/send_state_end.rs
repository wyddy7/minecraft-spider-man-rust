// After the state packet: the player's own on-ground value again.
fn after_send_state(value: (), game: &mut crate::game::Game<'_>) {
    crate::game::spidey_socket::after_send_state(game);
    value
}
