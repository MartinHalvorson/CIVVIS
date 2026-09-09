//! A refused approach is evidence about a route, not every city beyond it.
use super::{AdvancedAi, Game, Pos};

impl AdvancedAi {
    /// Keep the ranked city and route around the refused tile. The returned
    /// waypoint still goes through the ordinary civilian and escort safety
    /// checks before movement. A missing route can defer this site only until
    /// the host-refusal cooldown ends.
    pub(super) fn settler_refusal_waypoint(&self, g: &Game, uid: u32, target: Pos) -> Option<Pos> {
        if !self.live_move_refusal_break || !self.base.move_refusal_blocked(g, uid) {
            return Some(target);
        }
        if g.units.get(&uid).is_some_and(|unit| unit.pos == target) {
            return Some(target);
        }
        let (blocked, _) = self.base.move_refusal_blocks[&uid];
        g.route_step_avoiding(uid, target, blocked)
    }
}

#[cfg(test)]
mod tests;
