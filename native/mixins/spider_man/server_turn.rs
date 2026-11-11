// A player's turn on the own world's server: is it the world's owner, with
// their web holding?
fn server_turn(host: &crate::game::lan::LanHost, conn: u32) {
    crate::game::spider_man::server_turn(host, conn);
}
