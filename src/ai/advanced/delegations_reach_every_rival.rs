//! `delegations-reach-every-rival`: the diplomatic-mission step in
//! `advanced_diplomacy` can ask on every turn instead of every eighth, skips
//! a rival the host reads as Unfriendly, Denounced or at War, and skips a
//! rival that already hosts our Delegation (unless the
//! Embassy that supersedes it is ready) or our Embassy, and does not ask the
//! same rival again within [`MISSION_RETRY_TURNS`] standard turns.
//!
//! **Why (live Emperor, the pin era of 2026-10-18).** Of 97 rivals met by
//! turn 80, 19 held our Delegation or Embassy by then. The step picks the
//! rival with the best opinion of us and, when that rival already holds our
//! Delegation and no Embassy is ready, sends nothing: one mission a game,
//! to the neighbour that needed it least. A Delegation is the host's "We
//! sent them a delegation" opinion modifier for pocket change, and the
//! neighbour that declares on us early is the one still Unfriendly:
//! civvis-20261018T093720Z, the Ottomans at 107 power against our 47 at
//! turn 30, took Popayán at 42 and the capital at 111; `befriend-the-strongest`
//! asks only a rival the host reads as Friendly.
//!
//! Seven of 25 first contacts in six recorded Deity Pericles Culture games
//! were Neutral on an odd turn and Unfriendly the next. The former two-turn
//! cadence cannot use those windows. The host's mission guard refuses the
//! colder state, so preserve the one mission slot for an eligible rival.
//!
//! Off, nothing changes.

use super::*;

/// Standard turns before the same rival is asked again.
pub(super) const MISSION_RETRY_TURNS: u32 = 16;

impl AdvancedAi {
    /// Every turn can contain the only Neutral first-contact window. The
    /// partner's retry clock still prevents repeated asks.
    pub(super) fn mission_cadence_due(&self, _g: &Game, _pid: usize) -> bool {
        self.delegations_reach_every_rival
    }

    /// Whether `other` may be asked: always with the gene off; with it on,
    /// only a rival without our Embassy, holding our Delegation only when
    /// the Embassy is ready to supersede it, and not asked within
    /// [`MISSION_RETRY_TURNS`].
    pub(super) fn mission_partner_open(&self, g: &Game, pid: usize, other: usize) -> bool {
        if !self.delegations_reach_every_rival {
            return true;
        }
        // Match the live bridge's delegation_unfriendly guard before choosing
        // the one partner to ask. Boards without a host reading keep their
        // ordinary mission eligibility.
        if g.players[other]
            .observed_diplomatic_state
            .get(&pid)
            .is_some_and(|state| matches!(state.as_str(), "UNFRIENDLY" | "DENOUNCED" | "WAR"))
        {
            return false;
        }
        let retry = g.standard_duration(MISSION_RETRY_TURNS);
        if self
            .delegations_asked
            .get(&other)
            .is_some_and(|asked| g.turn.saturating_sub(*asked) < retry)
        {
            return false;
        }
        match g.diplomatic_mission_to(pid, other) {
            None => true,
            Some(mission) => {
                mission.kind == "delegation"
                    && g.players[pid]
                        .civics
                        .contains(&crate::name!("diplomatic_service"))
            }
        }
    }

    /// Remember the rival a mission `action` asked. No-op with the gene off.
    pub(super) fn record_mission_ask(&mut self, g: &Game, action: &Action) {
        if !self.delegations_reach_every_rival {
            return;
        }
        if let Action::SendDelegation { player } | Action::SendEmbassy { player } = action {
            self.delegations_asked.insert(*player, g.turn);
        }
    }
}

#[cfg(test)]
mod tests;
