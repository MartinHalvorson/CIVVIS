//! `delegations-reach-every-rival`: the diplomatic-mission step in
//! `advanced_diplomacy` asks every [`MISSION_CADENCE`] turns instead of every
//! eighth, skips a rival that already hosts our Delegation (unless the
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
//! Off, nothing changes.

use super::*;

/// Turns between asks while the gene is on.
pub(super) const MISSION_CADENCE: u32 = 2;

/// Standard turns before the same rival is asked again.
pub(super) const MISSION_RETRY_TURNS: u32 = 16;

impl AdvancedAi {
    /// Whether this turn is an extra mission turn: every
    /// [`MISSION_CADENCE`] turns with the gene on, never with it off.
    pub(super) fn mission_cadence_due(&self, g: &Game, pid: usize) -> bool {
        self.delegations_reach_every_rival
            && g.turn % MISSION_CADENCE == pid as u32 % MISSION_CADENCE
    }

    /// Whether `other` may be asked: always with the gene off; with it on,
    /// only a rival without our Embassy, holding our Delegation only when
    /// the Embassy is ready to supersede it, and not asked within
    /// [`MISSION_RETRY_TURNS`].
    pub(super) fn mission_partner_open(&self, g: &Game, pid: usize, other: usize) -> bool {
        if !self.delegations_reach_every_rival {
            return true;
        }
        let retry = g.standard_duration(MISSION_RETRY_TURNS);
        if self
            .mission_asked
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
            self.mission_asked.insert(*player, g.turn);
        }
    }
}

#[cfg(test)]
mod tests;
