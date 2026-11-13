//! `faithless-seat-holds-its-faith-strikes` (opt-in): a seat that has founded
//! no religion holds `faith-strike-wipes-the-spreaders` unless the rival's
//! faith stands at match point.
//!
//! **Why.** A faith strike declares war to condemn a rival's spreaders. What
//! it protects is the seat's own faith, and a seat that founded none has
//! none to protect. The live Rome lane (Deity, 6 players, Continents) shows
//! what the strikes cost such a seat (52 Score games of 2026-11-12/13, the
//! seat founded no faith in any of them):
//!
//! - 30 strikes in 26 games took no city. Within 30 turns they lost 3 cities
//!   to the target and 8 to others, and 78 units to the target against 37 of
//!   its killed; 21 of the 30 had no peace within 30 turns.
//! - No rival's faith came near a Religious Victory: at most 3 of 6 living
//!   majors at a strike, and no rival won one in the 52 games.
//!
//! `score-seat-holds-its-faith-strikes` answered that for a Score seat, and
//! since its arming it has held a strike in 31 of 32 live games (442 journal
//! lines, 2026-11-13 10:00Z-16:00Z): left alone, the seat opens one of these
//! wars in nearly every game. On 2026-11-13 15:57Z the Rome lane moved to a
//! Science target, where the Score hold is inert and the strikes come back.
//! The reason for the hold never depended on the target; it depended on the
//! seat having no faith of its own.
//!
//! **The rule.** On a seat that has founded no religion and whose victory
//! target is not Religion, a faith strike that is otherwise ready is held
//! unless the rival's faith is at match point by either of the two readings
//! `faith-war-at-match-point` uses:
//! - the religion lane reads its match point (`faith_at_match_point`: every
//!   living major but one);
//! - or we are its last holdout (`we_are_the_last_holdout`: every other
//!   living major follows it and our majority does not).
//!
//! The hold is journalled "Holding the faith strike on X" with the reading,
//! at most once a rival every [`HELD_NOTE_TURNS`] standard turns. A seat
//! that founded a faith, and a Religion seat, strike as before; so does the
//! match-point faith war (`faith-war-at-match-point`), which opens only at
//! match point anyway. `score-seat-holds-its-faith-strikes` is unchanged and
//! is read first.
//!
//! Off: unchanged.

use super::score_seat_holds_its_faith_strikes::HELD_NOTE_TURNS;
use super::*;

impl AdvancedAi {
    /// `faithless-seat-holds-its-faith-strikes`: whether a seat with no
    /// religion of its own holds the faith strike on `rival`. `true` when the
    /// gene is on, the seat founded no religion, its victory target is not
    /// Religion, and `rival`'s faith is at match point by neither reading.
    /// Journals the hold. `false` with the gene off.
    pub(super) fn faithless_seat_holds_the_strike(
        &mut self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> bool {
        if !self.faithless_seat_holds_its_faith_strikes
            || g.players[pid].religion.is_some()
            || self.active_victory_target(g) == Some(VictoryTarget::Religion)
        {
            return false;
        }
        if self.faith_at_match_point(g, rival) || Self::we_are_the_last_holdout(g, pid, rival) {
            return false;
        }
        self.journal_faithless_strike_held(g, rival);
        true
    }

    fn journal_faithless_strike_held(&mut self, g: &Game, rival: usize) {
        if !self.journal().wants(crate::reasoning::Level::Detail) {
            return;
        }
        let window = g.standard_duration(HELD_NOTE_TURNS).max(1);
        if self
            .faith_war_held_noted
            .get(&rival)
            .is_some_and(|(turn, noted)| {
                *noted == "faithless_seat" && g.turn.saturating_sub(*turn) < window
            })
        {
            return;
        }
        self.faith_war_held_noted
            .insert(rival, (g.turn, "faithless_seat"));
        let (converted, living) = self.religious_conversion_tally(g, rival);
        think!(self.journal(), Military, Detail,
               "Holding the faith strike on {}", g.players[rival].civ;
               "faithless-seat-holds-its-faith-strikes: its faith holds {converted} of {} living majors, short of match point ({} of them, or every one but us); we founded no religion, so no faith of ours is at stake, and the war costs units, grievances and friendships",
               living, living.saturating_sub(1));
    }
}

#[cfg(test)]
mod tests;
