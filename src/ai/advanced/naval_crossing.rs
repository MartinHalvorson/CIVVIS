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
//! With the gene on, a crossing alone marks that Settler's voyage under way
//! for the next turn, and while it is under way and the sea stays quiet the
//! Settler steps on without holding again. The mark lapses as soon as a turn
//! passes without a crossing step (the Settler has landed or a ship came
//! near).
//!
//! The mark is keyed by the Settler, carried through `remap_unit_memory`.
//! The first version keyed it by the target site, and a Settler afloat is
//! retargeted often: live civvis-20261031T080511Z (armed) crossed alone at
//! t103 toward (37, 33), had its nearby fallbacks refused at t104, and at
//! t105 was marching to (50, 32), so the next water tile waited two turns
//! again (5 lone crossings against 43 escort holds). Afloat is the exposure
//! whichever site the Settler now heads for.
use super::*;

impl AdvancedAi {
    /// Whether Settler `uid`'s crossing is under way: the gene is on and it
    /// took a crossing step alone this turn or the last.
    pub(super) fn naval_crossing_underway(&self, g: &Game, uid: u32) -> bool {
        self.naval_crossing_keeps_going
            && self
                .naval_crossing_units
                .get(&uid)
                .is_some_and(|last| last.saturating_add(1) >= g.turn)
    }

    /// Record a crossing step alone by Settler `uid` this turn (gene on
    /// only), and drop voyages that have lapsed.
    pub(super) fn note_naval_crossing(&mut self, g: &Game, uid: u32) {
        if !self.naval_crossing_keeps_going {
            return;
        }
        let turn = g.turn;
        self.naval_crossing_units
            .retain(|_, last| last.saturating_add(1) >= turn);
        self.naval_crossing_units.insert(uid, turn);
    }
}

#[cfg(test)]
mod tests;
