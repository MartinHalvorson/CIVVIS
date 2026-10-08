//! `defeated-majors-leave-the-board`: a major the host has eliminated stops
//! counting as a living civilization.
//!
//! The mirror plants a seat for every major at setup and never retires one,
//! so a major we eliminate stays `alive` on the board with no city and no
//! unit. Every "living majors" reading then counts it: the religious match
//! point (`100 * (living - 1) / living`), a faith's conversion share
//! (`religious_conversion_tally`), the early-warning bar, and the counterfaith
//! safety test that wants two living holdouts
//! (`counterfaith_leaves_two_holdouts_at`). Live Emperor G415
//! (civvis-20261008T160451Z) eliminated Poland at turn 104. Ethiopia's
//! Orthodoxy then held Ethiopia and us (6 of 11 cities Orthodox at 108)
//! with only Sumeria left, which is match point (2 of 3), but the board read
//! 2 of 4: "Countering Ethiopia | its religion race reads 50%" from 108 to
//! 127 against a bar of 75, so neither the urgency nor the interception ever
//! opened. Worse, dead Poland still counted as Orthodoxy's second holdout, so
//! the counterweight went on buying ORTHODOX Missionaries against Catholicism
//! in Popayan to turn 107, and Ethiopia won on Religion at 132 with all 11 of
//! our cities. The host's World Congress standing (`congress_dvp`) lists one
//! entry per alive major; it dropped Poland the turn after it fell (and so
//! did it for the other two major eliminations of the October 6-8 runs).
//!
//! Under the gene, at the start of our turn, while the board holds more
//! living majors than the host's count, the excess seats with no city and no
//! unit on the board are retired (`alive = false`), the highest seat first.
//! A seat with a city or a unit is never retired. Off, and headless (no host
//! count): unchanged.

use super::*;

impl AdvancedAi {
    /// Retire the board's dead major seats to the host's living count. See the
    /// module documentation. Exact no-op with the gene off.
    pub(super) fn retire_defeated_majors(&self, g: &mut Game, pid: usize) {
        if !self.defeated_majors_leave_the_board {
            return;
        }
        let Some(living) = g.players[pid].live_living_majors else {
            return;
        };
        let majors: Vec<usize> = g
            .players
            .iter()
            .filter(|p| p.alive && !p.is_minor && !p.is_barbarian)
            .map(|p| p.id)
            .collect();
        if majors.len() <= living {
            return;
        }
        let mut excess = majors.len() - living;
        let mut empty: Vec<usize> = majors
            .into_iter()
            .filter(|seat| {
                *seat != pid
                    && g.player_city_ids(*seat).is_empty()
                    && g.player_unit_ids(*seat).is_empty()
            })
            .collect();
        empty.sort_unstable_by(|a, b| b.cmp(a));
        for seat in empty {
            if excess == 0 {
                break;
            }
            g.players[seat].alive = false;
            excess -= 1;
            think!(self.journal(), Strategy, Detail,
                "Retiring {} from the living", g.players[seat].civ;
                "defeated-majors-leave-the-board: the host counts {} living majors and this seat holds no city and no unit",
                living);
        }
    }
}

#[cfg(test)]
mod tests;
