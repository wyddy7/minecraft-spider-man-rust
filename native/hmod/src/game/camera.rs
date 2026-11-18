// The plug's chase or fixed camera, else the game's own.
fn plug_camera(
    game: &mut crate::game::Game<'_>,
    partial: f32,
) -> std::ops::ControlFlow<Option<(glam::DVec3, f32, f32)>> {
    crate::game::spider_man::camera_view(game, partial)
}
