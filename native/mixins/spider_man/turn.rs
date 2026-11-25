// The posed player's whole turn, given to what the game drew for it.
fn turned_player(
    value: (),
    game: &crate::game::Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) {
    crate::game::spider_man::turn_posed(game, camera, alpha, out);
    value
}
