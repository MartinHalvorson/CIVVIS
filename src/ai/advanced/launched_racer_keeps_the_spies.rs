//! `launched-racer-keeps-the-spies`: a rival whose Exoplanet Expedition has
//! launched leads the Spies over any rival that has not.
//!
//! `racer_spy_leader` counts a racer one project behind as level when it
//! makes [`RACER_SCIENCE_EDGE`] times the Science of those ahead, and
//! compares finishes only between two racers that have both launched. So a
//! stage-three rival with the edge reads level with a rival already in
//! flight, the finish comparison is skipped and Production decides. Live
//! Deity Indonesia civvis-20261105T154934Z, our closest loss: we launched at
//! t181 and stood 19 of 25 at t200, when America won on 23. America launched
//! at t186 making about 320 Science, and Byzantium held the Spies on three
//! projects making 500; 6 of our 7 Disrupt Rocketry runs went to Byzantium
//! and none to America in flight.
//!
//! Replayed over the 35 Deity Science losses of 11-05 (Production tiebreak
//! left out), the leader named the eventual winner at 25, 27, 30 and 31 of 35
//! at 8, 6, 4 and 2 turns before the end, and launched-first at 28, 30, 33
//! and 34. The two differ on 40 of 548 flight turns, in 8 games. A horizon on
//! the flyer's finish was no better at any of the four (14 turns: 27, 29, 33,
//! 34). The winner launched first in 30 of the 35.
//!
//! What this buys is not a stopped expedition. A flight cannot be recalled
//! and no flyer's pace fell after a Disrupt (0 of 102 changes): a Disrupt
//! denies the lasers still to come. The Spy already standing on a stage-three
//! rival's pad keeps working it, because a Spy offered Disrupt Rocketry where
//! it stands is not moved. Judge the gene on lasers built after launch, the
//! flyer's arrival turn, and whether a stage-three racer launches sooner, not
//! on naming the winner.
//!
//! Under the gene the leader is read among the launched racers alone, by
//! `racer_spy_leader`'s own order, when any has launched. Off, nothing
//! changes.

use super::*;

impl AdvancedAi {
    /// The racer the Spies go to: `racer_spy_leader` over `racers`, or over
    /// those that have launched the Exoplanet Expedition when any has and
    /// the gene is on.
    pub(crate) fn racer_spy_leader_of(&self, g: &Game, racers: &BTreeSet<usize>) -> Option<usize> {
        if self.launched_racer_keeps_the_spies {
            let launched: BTreeSet<usize> = racers
                .iter()
                .copied()
                .filter(|rival| {
                    g.players[*rival]
                        .science_projects
                        .contains("exoplanet_expedition")
                })
                .collect();
            if !launched.is_empty() {
                return Self::racer_spy_leader(g, &launched);
            }
        }
        Self::racer_spy_leader(g, racers)
    }
}

#[cfg(test)]
mod tests;
