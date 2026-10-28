// The wall camera's whole orientation, while it is turned.
fn turned_view() -> std::ops::ControlFlow<glam::Mat4> {
    match horizon_dev_reload::frame::view_turn() {
        Some(q) => std::ops::ControlFlow::Break(glam::Mat4::from_quat(
            glam::Quat::from_array(q).conjugate(),
        )),
        None => std::ops::ControlFlow::Continue(()),
    }
}
