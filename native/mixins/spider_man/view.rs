// The wall camera's whole orientation for the frame's angles, else the game's.
fn turned_view(yaw: f32, pitch: f32) -> std::ops::ControlFlow<glam::Mat4> {
    match ::spider_man::view_turn::get(yaw, pitch) {
        Some(q) => std::ops::ControlFlow::Break(glam::Mat4::from_quat(glam::Quat::from_array(q).conjugate())),
        None => std::ops::ControlFlow::Continue(()),
    }
}
