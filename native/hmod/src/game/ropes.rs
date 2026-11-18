// The plug's ropes, drawn with the leads.
fn plug_ropes(
    game: &crate::game::Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) {
    crate::game::spider_man::rope_geometry(game, camera, alpha, out);
}
