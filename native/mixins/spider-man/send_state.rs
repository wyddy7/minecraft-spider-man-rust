// Before the player's state packet: the own world's server hears the player
// as grounded while the plug forgets the fall.
fn before_send_state(game: &mut crate::game::Game<'_>) {
    crate::game::spider_man::before_send_state(game);
}
