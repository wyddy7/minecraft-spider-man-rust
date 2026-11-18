// Before the client's level tick moves the player: the plug's tick.
fn before_level_tick(game: &mut crate::game::Game<'_>) {
    crate::game::spider_man::tick(game);
}
