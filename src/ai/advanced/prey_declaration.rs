//! `prey-is-declared-on`: a Domination seat whose military power is at
//! least 1 / [`PREY_DECLARE_RATIO`] times a major rival's declares on it
//! without waiting on a short home Defend row (`war_policy`) or a staged
//! ring (`opens` in the declaration step). The war writes the Siege row that
//! brings the army to the ring; the declaration range, another major war,
//! the edge, the treasury and the production-parity checks still apply.
//!
//! **Why (live Emperor, 2026-10-19).** The one domination win of the loop
//! (civvis-20261018T090059Z) took America, at under a third of our power, from
//! turn 139. In the games since, a weak neighbour at war's reach was held for
//! 15-30 turns: Korea at 65 against our 500-650 from turn 88 to 117
//! (civvis-20261019T054930Z), America at 19-95 against 274-523 from 89 past
//! 120 (023252Z), Poland at 74-117 against 220-373 from 95 to 110 (002331Z),
//! alternating "the Defend row at … is short N unit(s)" with "the Siege row
//! for … asks N strength and M is staged on its ring" — a ring inside a
//! border our army may not cross at peace. `overwhelming-power-declares`
//! waives the ring only at four times the rival's steady power with the army's
//! median within ten tiles. On the siege replay harness (five such boards x 4
//! seeds x 40 turns) the waiver at 0.4 took 52 cities against 46, and left the
//! twelve stuck-siege boards unchanged (126 against 126).
//!
//! Off, nothing changes.

use super::*;

/// A major rival at or under this share of our military power is prey.
pub(super) const PREY_DECLARE_RATIO: f64 = 0.4;

impl AdvancedAi {
    /// Whether `target` is prey this seat declares on now: see the module.
    /// Always false with the gene off.
    pub(super) fn prey_is_declared_on_now(&self, g: &Game, pid: usize, target: usize) -> bool {
        self.prey_is_declared_on
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && g.players
                .get(target)
                .is_some_and(|rival| rival.alive && !rival.is_minor && !rival.is_barbarian)
            && g.military_power(target) <= PREY_DECLARE_RATIO * g.military_power(pid)
    }
}

#[cfg(test)]
mod tests;
