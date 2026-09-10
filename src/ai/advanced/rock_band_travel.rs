use super::{AdvancedAi, VictoryTarget};
use crate::game::{Action, Game};
use crate::Pos;

impl AdvancedAi {
    pub(super) fn advanced_rock_band_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        if self.active_victory_target(g) != Some(VictoryTarget::Culture) {
            return self.base.rock_band_step(g, pid, uid);
        }
        if g.rock_concert_tourism(pid, uid).is_some() {
            return g.apply(pid, &Action::PerformConcert { unit: uid }).is_ok();
        }
        let Some(target) = self.culture_concert_destination(g, pid, uid) else {
            return false;
        };
        let Some(next) = g.route_step(uid, target, 0) else {
            return false;
        };
        self.base.path_move(g, pid, uid, next)
    }

    fn culture_concert_destination(&self, g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        let moves = g.unit_max_moves(uid).max(1.0);
        g.map
            .tiles
            .keys()
            .copied()
            .filter_map(|position| {
                let value = g.rock_concert_ai_value(pid, uid, position)?;
                let distance = g.route_distance(uid, position, 0)?;
                // Route length is an optimistic travel estimate: terrain and
                // native movement may take longer. Include the performance
                // turn so a tiny adjacency advantage cannot justify a long tour.
                let score = value / (1.0 + distance as f64 / moves);
                Some((score, distance, position))
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.cmp(&left.1))
                    .then_with(|| right.2.cmp(&left.2))
            })
            .map(|(_, _, position)| position)
    }
}

#[cfg(test)]
mod tests;
