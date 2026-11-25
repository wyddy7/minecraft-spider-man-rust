// The plug's pose for the frame, before the game draws the player's own model.
fn posed_player(game: &crate::game::Game<'_>, alpha: f64, out: &Vec<horizon_client::mesher::Vertex>) {
    crate::game::spidey_socket::pose_frame(game, alpha, out.len())
}
