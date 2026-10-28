// The player's own model, posed by the plug when it sets a pose.
fn posed_player(
    game: &crate::game::Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) -> std::ops::ControlFlow<()> {
    crate::game::spidey_socket::player_model(game, camera, alpha, out)
}
