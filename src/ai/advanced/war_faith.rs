//! `war-faith-buys-the-army`: at war with a major, a Domination seat on a
//! Conquest or Recovery plan spends its Faith on the army first. The turn's
//! ancillary pass runs `military_faith_spending` ahead of Great Person
//! patronage, the religious spenders and the Faith buildings, holding back
//! only the inquisition's reserve ([`AdvancedAi::war_faith_reserve`]) instead
//! of the ordinary 180 Faith plus the counterweight's. The ordinary pass still
//! runs after them on what is left, and the item list is the host's own
//! purchase menu (`legal_purchase_actions` reads `host_purchase_price`), so a
//! city-state's Nihang is offered where the host sells one.
//!
//! **Why (live Emperor, 59 retained games of 2026-10-25).**
//! - Faith income is 15 / 22 / 29 / 34 / 38 a turn at turns 75 / 100 / 125 /
//!   150 / 175 (medians), above Gold's 16-20, with 96-243 banked.
//! - The host sells land combat units for Faith in our cities: in
//!   civvis-20261025T112746Z at turn 150 (Monarchy, no Plaza building) a
//!   Knight for 220, a Crossbowman 180, a Musketman 240, a Bombard 280,
//!   against 75 Faith a turn.
//! - The verified Faith purchases were 1,010 Missionaries, 109 Inquisitors,
//!   88 Apostles and the Great Person buyouts, against about 90 combat units
//!   (~1.5 a game): `military_faith_spending` ran last in the pass, after the
//!   bank was spent, and kept 180 Faith back.
//! - The siege it would feed is short of bodies: during the mustered train's
//!   "holds" with the strength near the city under its bill, our army was
//!   10-12 combat units, 4.4-4.8 of them within ten tiles of the target, the
//!   Siege force 4-5.
//!
//! Off, nothing changes.

use super::*;

impl AdvancedAi {
    /// Whether this turn's Faith goes to the army first: the gene is on, the
    /// seat plays for Domination on a Conquest or Recovery plan, and it is at
    /// war with a living major.
    pub(super) fn war_faith_buys_the_army_now(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        self.war_faith_buys_the_army
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && matches!(
                plan.strategy,
                GrandStrategy::Conquest | GrandStrategy::Recovery
            )
            && g.players.iter().any(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && g.is_at_war(pid, other.id)
            })
    }

    /// The Faith a war purchase leaves in the bank: the inquisition's reserve
    /// (`founder-funds-the-inquisition`), so a founder whose cities are
    /// converting can still answer.
    pub(super) fn war_faith_reserve(&self, g: &Game, pid: usize) -> f64 {
        self.inquisition_faith_reserve(g, pid)
    }
}

#[cfg(test)]
mod tests;
