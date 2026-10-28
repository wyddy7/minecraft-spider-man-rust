// The mod's chat commands sent another way (the control socket, a clicked
// command): not added to the chat history.
fn plug_command_sent(game: &mut crate::game::Game<'_>, message: &str) -> std::ops::ControlFlow<()> {
    crate::game::spider_man::chat_input(game, message, false)
}
