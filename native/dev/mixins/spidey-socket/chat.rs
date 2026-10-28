// The plug's chat commands, before the server sees them.
fn plug_command(game: &mut crate::game::Game<'_>, message: &str) -> std::ops::ControlFlow<()> {
    crate::game::spidey_socket::chat_input(game, message)
}
