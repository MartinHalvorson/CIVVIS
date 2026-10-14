//! `embassy-finds-the-capital`: a Resident Embassy reveals the host's
//! capital, so a Domination seat sends one to every met rival whose
//! original capital it has never seen, as soon as that rival will take it.
//!
//! ★★★ THE ONE PEACETIME REVEAL WAS SPENT ON THE WRONG RIVAL. Over 30
//! Immortal games of 10-13/14 (90 rival slots) 38 rival capitals were never
//! seen by the end of the game and the rest took a median 70 turns (strong
//! games) from first contact: 64 of 90 rivals already enforced their borders
//! when we met them. Of the 52 capitals that were seen, 12 appeared the very
//! turn a Resident Embassy landed (ordered at t, on the board at t + 1), in
//! all 12 cases where one landed while the capital was unknown; a Delegation
//! reveals nothing. The stock mission cadence (one mission every 8 turns)
//! wasted that reveal three ways:
//! - it asks the rival our board likes best, but the bridge refuses to ask a
//!   leader whose standing toward us is Unfriendly or Denounced, so 880 of
//!   1,038 asks ended `delegation_unfriendly` and 49 were sent;
//! - it never asks `denied_partner`, the plan's Conquest target: the rival
//!   whose capital the campaign needs most;
//! - once that favourite holds an Embassy the cadence falls through to no
//!   mission at all and never asks the next rival (225843Z: China, Neutral at
//!   Diplomatic Service, was never asked).
//!
//! 58 rival capitals were still unknown when Diplomatic Service arrived; 25
//! of those rivals had a turn at peace reading Neutral or warmer with no
//! Embassy yet. Asked then, 7 capitals never seen would have been revealed
//! and 16 more a median 7 turns (mean 12, up to 38) sooner.
//!
//! Under the gene, on a Domination plan with Diplomatic Service known, one
//! Resident Embassy a turn goes to such a rival: met, at peace, holding no
//! Embassy of ours, read by the host as Neutral or warmer toward us (the
//! bridge's own rule; a native board without that reading asks any rival not
//! denouncing either way), and not asked in the last
//! [`EMBASSY_CAPITAL_ASK_TURNS`] turns (the bridge's own per-target retry).
//! The plan's target goes first: an Embassy is no friendship and no promise,
//! so it leaves the war open. The stock cadence keeps everything else.
use super::{Action, AdvancedAi, Game, VictoryTarget};

/// Turns between two asks of the same rival: the bridge refuses a repeat
/// inside its `PeaceRetryTurns` (5) window anyway.
pub const EMBASSY_CAPITAL_ASK_TURNS: u32 = 5;
/// What the stock cadence holds back above the war treasury floor for an
/// Embassy; the bridge reads the host's own cost before it asks.
const EMBASSY_CAPITAL_GOLD: f64 = 25.0;

impl AdvancedAi {
    /// Whether `rival` would hear an Embassy ask: the host's reading of its
    /// standing toward us is Neutral or warmer, or, on a board without that
    /// reading, neither side has denounced the other.
    fn embassy_capital_hears(g: &Game, pid: usize, rival: usize) -> bool {
        match g.players[rival].observed_diplomatic_state.get(&pid) {
            Some(state) => matches!(
                state.as_str(),
                "NEUTRAL" | "FRIENDLY" | "DECLARED_FRIEND" | "ALLIED"
            ),
            None => {
                let denounced = |by: usize, of: usize| {
                    g.players[by]
                        .denounced_until
                        .get(&of)
                        .is_some_and(|until| *until > g.turn)
                };
                !denounced(pid, rival) && !denounced(rival, pid)
            }
        }
    }

    /// The rivals an Embassy would reveal a needed capital for, in the order
    /// they are asked: `target` (the plan's) first, then the one asked least
    /// recently, then by seat. Empty with the gene off, off Domination, or
    /// before Diplomatic Service.
    pub(super) fn embassy_capital_candidates(
        &self,
        g: &Game,
        pid: usize,
        target: Option<usize>,
    ) -> Vec<usize> {
        if !self.embassy_finds_the_capital
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !g.players[pid]
                .civics
                .contains(&crate::name!("diplomatic_service"))
        {
            return Vec::new();
        }
        let mut rivals: Vec<usize> = g
            .players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && !rival.is_free_city
                    && !g.same_team(pid, rival.id)
                    && g.has_met(pid, rival.id)
                    && !g.is_at_war(pid, rival.id)
            })
            .map(|rival| rival.id)
            .filter(|rival| !Self::original_capital_known(g, pid, *rival))
            .filter(|rival| {
                !g.diplomatic_mission_to(pid, *rival)
                    .is_some_and(|mission| mission.kind == "embassy")
            })
            .filter(|rival| Self::embassy_capital_hears(g, pid, *rival))
            .filter(|rival| {
                self.embassy_capital_asks
                    .get(rival)
                    .is_none_or(|asked| g.turn >= asked + EMBASSY_CAPITAL_ASK_TURNS)
            })
            .collect();
        rivals.sort_by_key(|rival| {
            (
                Some(*rival) != target,
                self.embassy_capital_asks.get(rival).copied(),
                *rival,
            )
        });
        rivals
    }

    /// `embassy-finds-the-capital`: send this turn's Embassy, if any rival
    /// qualifies and the treasury clears the war floor. Returns whether one
    /// was sent, so the stock cadence opens no second session this turn.
    pub(super) fn embassy_finds_the_capital_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        target: Option<usize>,
    ) -> bool {
        let Some(rival) = self
            .embassy_capital_candidates(g, pid, target)
            .into_iter()
            .next()
        else {
            return false;
        };
        let floor = self.war_treasury_floor(g, pid);
        if g.players[pid].gold < floor + EMBASSY_CAPITAL_GOLD {
            crate::think!(self.journal(), Diplomacy, Detail,
                "Embassy to {} waits on the treasury", g.players[rival].civ;
                "embassy-finds-the-capital: {:.0} Gold against a floor of {floor:.0} plus {EMBASSY_CAPITAL_GOLD:.0}",
                g.players[pid].gold);
            return false;
        }
        if g.apply(pid, &Action::SendEmbassy { player: rival })
            .is_err()
        {
            return false;
        }
        self.embassy_capital_asks.insert(rival, g.turn);
        let standing = g.players[rival]
            .observed_diplomatic_state
            .get(&pid)
            .map(|state| state.to_lowercase())
            .unwrap_or_else(|| "not hostile".to_string());
        crate::think!(self.journal(), Diplomacy, Decision,
            "Sending a Resident Embassy to {}", g.players[rival].civ;
            "embassy-finds-the-capital: Domination needs their original capital and we have never seen it; an Embassy reveals it; they read {standing} toward us{}",
            if Some(rival) == target { ", and they are the plan's target" } else { "" });
        true
    }
}

#[cfg(test)]
mod tests;
