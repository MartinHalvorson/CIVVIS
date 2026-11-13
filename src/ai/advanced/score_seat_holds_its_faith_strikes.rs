//! `score-seat-holds-its-faith-strikes` (opt-in): a seat whose victory target
//! is Score holds `faith-strike-wipes-the-spreaders` unless the rival's faith
//! stands at match point.
//!
//! **Why (live Rome Score, Deity, 6 players Continents, turn limit 170-160,
//! 52 games of 2026-11-12/13).** The faith strike opened 30 wars in 26 games.
//! It is the only faith opener that fires on a Score seat: the
//! `faith-war-before-the-flip` family needs a Domination (or, under its gene,
//! a Diplomacy) seat, and the religious interception a Domination seat.
//!
//! - **What the strikes cost.** They took no city. Within 30 turns they lost
//!   3 cities to the target and 8 to others, and 78 units to the target
//!   against 37 of its killed. 21 of the 30 had no peace within 30 turns.
//! - **What they protected.** Our cities on the rival's faith fell by 1.07 in
//!   the 15 turns after a strike (29 strikes), against a rise of 1.47 after
//!   one the seat held (114). But the seat founded no faith in any of the 52
//!   games, and on a Score seat a foreign faith in our cities costs no points
//!   of ours.
//! - **No faith came near a Religious Victory.** The rival's faith held at
//!   most 3 of 6 living majors at a strike, never its match point, and no
//!   rival won a Religious Victory in the 52 games. Games with a strike ended
//!   at a mean score rank of 5.8 against 4.9 without one. That comparison is
//!   confounded; it is not a measured cost.
//!
//! **The rule.** On a Score seat a faith strike that is otherwise ready is
//! held unless the rival's faith is at match point by either of the two
//! readings `faith-war-at-match-point` uses:
//! - the religion lane reads its match point (`faith_at_match_point`: every
//!   living major but one);
//! - or we are its last holdout (`we_are_the_last_holdout`: every other
//!   living major follows it and our majority does not).
//!
//! The hold is journalled "Holding the faith strike on X" with the reading,
//! at most once a rival every [`HELD_NOTE_TURNS`] standard turns. Other
//! victory targets are unchanged, and so is the match-point faith war
//! (`faith-war-at-match-point`), which opens only at match point anyway.
//!
//! Off: unchanged.

use super::*;

/// The hold's journal line repeats at most this often per rival.
pub(crate) const HELD_NOTE_TURNS: u32 = 5;

impl AdvancedAi {
    /// `score-seat-holds-its-faith-strikes`: whether a Score seat holds the
    /// faith strike on `rival`. `true` when the gene is on, the seat's victory
    /// target is Score, and `rival`'s faith is at match point by neither
    /// reading. Journals the hold. `false` with the gene off.
    pub(super) fn score_seat_holds_the_strike(
        &mut self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> bool {
        if !self.score_seat_holds_its_faith_strikes
            || self.active_victory_target(g) != Some(VictoryTarget::Score)
        {
            return false;
        }
        if self.faith_at_match_point(g, rival) || Self::we_are_the_last_holdout(g, pid, rival) {
            return false;
        }
        self.journal_score_strike_held(g, rival);
        true
    }

    fn journal_score_strike_held(&mut self, g: &Game, rival: usize) {
        if !self.journal().wants(crate::reasoning::Level::Detail) {
            return;
        }
        let window = g.standard_duration(HELD_NOTE_TURNS).max(1);
        if self
            .faith_war_held_noted
            .get(&rival)
            .is_some_and(|(turn, noted)| {
                *noted == "score_seat" && g.turn.saturating_sub(*turn) < window
            })
        {
            return;
        }
        self.faith_war_held_noted
            .insert(rival, (g.turn, "score_seat"));
        let (converted, living) = self.religious_conversion_tally(g, rival);
        think!(self.journal(), Military, Detail,
               "Holding the faith strike on {}", g.players[rival].civ;
               "score-seat-holds-its-faith-strikes: its faith holds {converted} of {} living majors, short of match point ({} of them, or every one but us); a Score seat's own faith is not at stake, and the war costs units, grievances and friendships",
               living, living.saturating_sub(1));
    }
}

#[cfg(test)]
mod tests;
