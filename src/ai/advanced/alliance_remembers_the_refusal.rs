//! `alliance-remembers-the-refusal`: a partner asked for an alliance that did
//! not ally is not asked again for [`ALLIANCE_REFUSAL_TURNS`] turns.
//!
//! Every live King Culture seat on 2026-11-01 froze on a turn a rival
//! refused our alliance ask: the rival's counter to the refusal sat "Adding
//! to pending" in DiplomacyManager.csv and was never processed (102116Z t120,
//! 063953Z t186, 091422Z t120, 081602Z t149, 143722Z t153). Accepted asks
//! never froze. Most refusals were the same rival asked again on the desks'
//! ten-turn retry: 081602Z asked its partner six times (t144-t222) and was
//! refused six times, 102116Z four times, 091422Z three. A refusal answered
//! the next turn is answered the same ten turns later; thirty turns is long
//! enough for the partner's standing to change (062425Z's third ask of the
//! same rival, 31 turns after its second, was accepted and ran t204-t223).
//!
//! The live board does not carry the refusal itself, so an ask that is not
//! followed by an alliance is read as one: the cool-down starts at the ask
//! and lifts once the partner is allied with us. Both desks that ask
//! (`propose_strategic_alliance` and the coalition) read and write it.

use super::AdvancedAi;
use crate::game::Game;

/// Turns a partner that did not ally is left unasked.
pub(super) const ALLIANCE_REFUSAL_TURNS: u32 = 30;

impl AdvancedAi {
    /// `alliance-remembers-the-refusal`: whether `partner` was asked within
    /// [`ALLIANCE_REFUSAL_TURNS`] and is not allied with us. Never off.
    pub(super) fn alliance_refusal_barred(&self, g: &Game, pid: usize, partner: usize) -> bool {
        self.alliance_remembers_the_refusal
            && g.alliance_with(pid, partner).is_none()
            && self
                .alliance_ask_turns
                .get(&partner)
                .is_some_and(|asked| g.turn < asked.saturating_add(ALLIANCE_REFUSAL_TURNS))
    }

    /// `alliance-remembers-the-refusal`: an alliance was just asked of
    /// `partner`. A no-op off.
    pub(super) fn alliance_note_asked(&mut self, g: &Game, partner: usize) {
        if self.alliance_remembers_the_refusal {
            self.alliance_ask_turns.insert(partner, g.turn);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::opt_in_off_in_both_controllers;
    use super::*;
    use crate::game::AllianceState;

    fn game() -> Game {
        Game::new_full(3, 24, 16, 7_923, 300, 0, false)
    }

    /// A partner asked at turn 10 that did not ally is barred through turn
    /// 39 and asked again from 40; an ally is never barred, and nothing is
    /// remembered or barred off the gene.
    #[test]
    fn a_partner_that_did_not_ally_waits_thirty_turns() {
        let mut g = game();
        let mut ai = AdvancedAi::new();
        ai.enable_alliance_remembers_the_refusal();
        g.turn = 10;
        ai.alliance_note_asked(&g, 1);
        assert!(
            ai.alliance_refusal_barred(&g, 0, 1),
            "barred the turn it was asked"
        );
        assert!(
            !ai.alliance_refusal_barred(&g, 0, 2),
            "another partner is not"
        );
        g.turn = 39;
        assert!(ai.alliance_refusal_barred(&g, 0, 1), "still barred at 39");
        g.turn = 40;
        assert!(!ai.alliance_refusal_barred(&g, 0, 1), "asked again from 40");
        // An ally is never barred.
        g.turn = 20;
        let alliance = AllianceState {
            kind: "research".to_string(),
            points: 0.0,
            level: 1,
            ends: 60,
        };
        g.players[0].alliances.insert(1, alliance.clone());
        g.players[1].alliances.insert(0, alliance);
        assert!(
            !ai.alliance_refusal_barred(&g, 0, 1),
            "allied: not a refusal"
        );
        // Off: nothing is remembered, nothing is barred.
        let mut off = AdvancedAi::new();
        off.alliance_note_asked(&g, 2);
        assert!(off.alliance_ask_turns.is_empty());
        assert!(!off.alliance_refusal_barred(&g, 0, 2));
    }

    #[test]
    fn alliance_remembers_the_refusal_is_opt_in() {
        opt_in_off_in_both_controllers("alliance-remembers-the-refusal", |ai| {
            ai.alliance_remembers_the_refusal
        });
    }
}
