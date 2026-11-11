// The server's fall check for a move: while the owner's web holds, the fall so
// far and this move's own drop are forgotten, as Slow Falling forgets the fall
// every tick (so a web shot after a long drop lands softly). The check adds
// this move's drop (as f32, like Entity.checkFallDamage) to the distance next:
// starting from its negative leaves 0.
fn web_forgets_fall(w: &mut crate::motion::MotionWorld<'_, '_, '_>, dy: f64) {
    if crate::motion::spider_man_fall::forget() {
        w.player.fall_distance = if dy < 0.0 { f64::from(dy as f32) } else { 0.0 };
    }
}
