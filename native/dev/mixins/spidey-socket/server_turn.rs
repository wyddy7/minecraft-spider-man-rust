// A player's turn on the own world's server: is it the world's owner, with
// their web holding?
fn server_turn(game: &crate::game::Game<'_>, conn: u32) {
    if let Some(host) = game.lan.as_ref() {
        crate::game::spidey_socket::server_turn(host, conn);
    }
}
