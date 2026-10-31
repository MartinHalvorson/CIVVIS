//! `naval-crossing-keeps-going`: a Settler that has crossed alone keeps
//! crossing on a quiet sea instead of waiting again at every water tile.
//!
//! `naval-escort-patience` lets an embarked Settler cross without its naval
//! guard once it has held [`super::NAVAL_ESCORT_PATIENCE`] turns and no
//! hostile ship is within [`super::NAVAL_THREAT_RADIUS`]. The wait is counted
//! per water tile (`naval_escort_waits`, keyed by the tile it holds on), so
//! the next water tile starts the count again: the Settler holds two turns,
//! takes one step, and holds two more, all the way across. Live Immortal
//! civvis-20261031T042324Z (6p Continents): one Settler held on the water
//! from turn 52 to 79, crossing about one tile every 2-3 turns, with 110
//! "holds for a naval escort" lines and 21 "crosses without its naval
//! escort" on 48 turns; 024232Z held 22 turns, 011801Z 19, 033836Z 17.
//!
//! With the gene on, a crossing alone toward a site marks that voyage under
//! way for the next turn, and while it is under way and the sea stays quiet
//! the Settler steps on without holding again. The mark is keyed by the
//! target site, not the unit, so it survives a unit-id remap, and it lapses
//! as soon as a turn passes without a crossing step toward that site (the
//! Settler has landed, been retargeted or a ship came near).
use super::*;

impl AdvancedAi {
    /// Whether a crossing toward `target` is under way: the gene is on and a
    /// crossing step toward it was taken this turn or the last.
    pub(super) fn naval_crossing_underway(&self, g: &Game, target: Pos) -> bool {
        self.naval_crossing_keeps_going
            && self
                .naval_crossing_targets
                .get(&target)
                .is_some_and(|last| last.saturating_add(1) >= g.turn)
    }

    /// Record a crossing step toward `target` this turn (gene on only), and
    /// drop voyages that have lapsed.
    pub(super) fn note_naval_crossing(&mut self, g: &Game, target: Pos) {
        if !self.naval_crossing_keeps_going {
            return;
        }
        let turn = g.turn;
        self.naval_crossing_targets
            .retain(|_, last| last.saturating_add(1) >= turn);
        self.naval_crossing_targets.insert(target, turn);
    }
}

#[cfg(test)]
mod tests;
