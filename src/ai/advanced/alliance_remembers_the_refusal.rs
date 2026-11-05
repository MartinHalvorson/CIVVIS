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
//!
//! ★★ A FRIENDSHIP STEP IS NOT A REFUSAL (2026-11-04). The host takes an
//! alliance in two asks: to a rival that is only Friendly the agent's
//! `CivvisAllianceStep` declares the friendship and asks nothing else, and
//! the alliance itself waits for the next ask, once the friendship stands
//! (`DIPLO_STATE_DECLARED_FRIEND`; "This relationship state is required to
//! make an Alliance", Expansion1_Civilopedia_Text.xml
//! LOC_PEDIA_CONCEPTS_PAGE_DIPLO_12_CHAPTER_CONTENT_PARA_2). The cool-down
//! read that first ask as a refusal and held the partner thirty turns —
//! longer than the friendship it had just won. Live Deity Korea Science
//! civvis-20261104T213449Z (Online speed): the ask to player 3 at t107 was
//! the friendship step (declared), the bar held it to t137, the 21-turn
//! Online friendship had lapsed by then, and the t137 ask was the friendship
//! step again; the alliance itself was never asked, and player 3 won at
//! t173. So the ask remembers whether the partner was a declared friend: a
//! partner that was not, and is one now, took the friendship and is asked
//! for the alliance after [`ALLIANCE_FRIENDSHIP_STEP_TURNS`]. A partner
//! asked while already a friend, or still not one, keeps the full bar. A
//! native board bundles the friendship into the alliance deal, so there an
//! accepted ask is an alliance (no bar) and a refused one leaves no
//! friendship (full bar), exactly as before.

use super::AdvancedAi;
use crate::game::Game;

/// Turns a partner that did not ally is left unasked.
pub(super) const ALLIANCE_REFUSAL_TURNS: u32 = 30;

/// Turns after a friendship step before the alliance itself is asked: the
/// agent's own retry floor for an alliance ask to one rival
/// (`AllianceRetryTurns`, default 5, in `CivvisAllianceStep`). An ask inside
/// it is refused by the agent as `alliance_cooldown` without reaching the
/// rival, yet would be remembered here as a friend's refusal.
pub(super) const ALLIANCE_FRIENDSHIP_STEP_TURNS: u32 = 5;

impl AdvancedAi {
    /// `alliance-remembers-the-refusal`: whether `partner` was asked within
    /// [`ALLIANCE_REFUSAL_TURNS`] and is not allied with us. Never off.
    /// A partner that was not a declared friend when asked and is one now
    /// took the friendship step: barred only for
    /// [`ALLIANCE_FRIENDSHIP_STEP_TURNS`].
    pub(super) fn alliance_refusal_barred(&self, g: &Game, pid: usize, partner: usize) -> bool {
        if !self.alliance_remembers_the_refusal || g.alliance_with(pid, partner).is_some() {
            return false;
        }
        let Some(&(asked, friend_when_asked)) = self.alliance_ask_turns.get(&partner) else {
            return false;
        };
        let bar = if !friend_when_asked && g.are_friends(pid, partner) {
            ALLIANCE_FRIENDSHIP_STEP_TURNS
        } else {
            ALLIANCE_REFUSAL_TURNS
        };
        g.turn < asked.saturating_add(bar)
    }

    /// `alliance-remembers-the-refusal`: an alliance was just asked of
    /// `partner`, a declared friend of ours or not. A no-op off.
    pub(super) fn alliance_note_asked(&mut self, g: &Game, pid: usize, partner: usize) {
        if self.alliance_remembers_the_refusal {
            self.alliance_ask_turns
                .insert(partner, (g.turn, g.are_friends(pid, partner)));
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
        ai.alliance_note_asked(&g, 0, 1);
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
        off.alliance_note_asked(&g, 0, 2);
        assert!(off.alliance_ask_turns.is_empty());
        assert!(!off.alliance_refusal_barred(&g, 0, 2));
    }

    /// Live civvis-20261104T213449Z: the t107 ask to a Friendly rival only
    /// declared the friendship. Once that friendship stands the alliance is
    /// asked five turns after the step, not thirty; a partner asked while
    /// already a friend, or one that refused the friendship too, keeps the
    /// full bar.
    #[test]
    fn a_friendship_step_waits_five_turns_not_thirty() {
        let mut g = game();
        let mut ai = AdvancedAi::new();
        ai.enable_alliance_remembers_the_refusal();
        g.turn = 107;
        // Asked while not friends: the host can only declare the friendship.
        ai.alliance_note_asked(&g, 0, 1);
        ai.alliance_note_asked(&g, 0, 2);
        // Player 1 took the friendship; player 2 did not.
        g.players[0].friends_until.insert(1, 128);
        g.players[1].friends_until.insert(0, 128);
        assert!(
            ai.alliance_refusal_barred(&g, 0, 1),
            "inside the agent's retry floor"
        );
        g.turn = 111;
        assert!(
            ai.alliance_refusal_barred(&g, 0, 1),
            "still inside it at 111"
        );
        g.turn = 112;
        assert!(
            !ai.alliance_refusal_barred(&g, 0, 1),
            "the new friend is asked for the alliance at 112"
        );
        assert!(
            ai.alliance_refusal_barred(&g, 0, 2),
            "a rival that refused the friendship keeps the thirty-turn bar"
        );
        // Asked again, now as a friend, and not allied: a real refusal.
        ai.alliance_note_asked(&g, 0, 1);
        g.turn = 141;
        assert!(
            ai.alliance_refusal_barred(&g, 0, 1),
            "a friend that refused the alliance waits thirty turns"
        );
        g.turn = 142;
        assert!(!ai.alliance_refusal_barred(&g, 0, 1));
    }

    #[test]
    fn alliance_remembers_the_refusal_is_opt_in() {
        opt_in_off_in_both_controllers("alliance-remembers-the-refusal", |ai| {
            ai.alliance_remembers_the_refusal
        });
    }
}
