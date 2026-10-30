//! `settler-founds-under-raiders`: a stalled Settler whose only threats are
//! barbarians founds where it stands.
//!
//! When a Settler's stall counter expires (`SETTLER_STALL_LIMIT` turns
//! without progress), `founds_where_it_stands` asks whether the ground it is
//! on would take a city, and refuses when `settlement_tile_risk` passes
//! `SETTLER_STEP_RISK_LIMIT`. A passing barbarian raider is exactly what
//! stalls a Settler, through the safe-step guard, and also what fails that
//! check, so the Settler retargets and stalls again. But a barbarian never
//! captures a city ([`crate::game::Game`] leaves the city at 1 health), while
//! it does capture a Settler. A city on that tile is strictly safer than the
//! Settler standing on it.
//!
//! Live Immortal Pericles seats of 2026-10-30, 21 games, first 100 turns: a
//! Settler lived a median 8 turns (mean 11.4, 51% at 8 or more, tails of
//! 20-58) before founding. Of about 630 "Settler HELD" journal lines, 571 read
//! "the safe-step guard rejected every neighbour". civvis-20261030T175606Z
//! held one Settler from turn 22 to 55 inside a five-by-three box around
//! (44-49, 9-11), retreating from raiders and keeping a blocked site, while
//! the early city floor wanted four cities. At turn 80 the seats the
//! production-share rule kept held about seven cities; those it retired held
//! about five.
//!
//! With the gene on, the tile-risk refusal in `founds_where_it_stands` is
//! waived when every visible hostile military unit within
//! [`RAIDER_REACH`] tiles is a barbarian's and there is at least one. The
//! Loyalty verdict, the unsupported-frontier guard and the hidden-border
//! bookkeeping around the founding are unchanged.
use super::*;

/// Tiles within which a visible hostile counts toward the threat.
pub(crate) const RAIDER_REACH: i32 = 3;

impl AdvancedAi {
    /// Whether the threat at `here` is barbarians alone: the gene is on, a
    /// visible hostile military unit (not air) stands within [`RAIDER_REACH`],
    /// and every such unit belongs to a barbarian player.
    pub(super) fn settler_threat_is_raiders_only(
        &self,
        g: &Game,
        pid: usize,
        here: Pos,
        visible: &TileBits,
    ) -> bool {
        if !self.settler_founds_under_raiders {
            return false;
        }
        let mut raiders = 0;
        for unit in g.units.values() {
            if unit.owner == pid
                || !g.is_at_war(pid, unit.owner)
                || !g.sees(visible, unit.pos)
                || !g.unit_visible_to(unit.id, pid)
                || g.wdist(unit.pos, here) > RAIDER_REACH
            {
                continue;
            }
            let spec = &g.rules.units[unit.kind];
            if spec.class != "military" || spec.domain.as_deref() == Some("air") {
                continue;
            }
            if !g
                .players
                .get(unit.owner)
                .is_some_and(|owner| owner.is_barbarian)
            {
                return false;
            }
            raiders += 1;
        }
        raiders > 0
    }
}

#[cfg(test)]
mod tests;
