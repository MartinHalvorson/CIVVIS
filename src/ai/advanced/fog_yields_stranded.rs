//! `fog-yields-to-a-stranded-settler`: `frontier-loyalty`'s fogged-frontier
//! veto (no own city within reach while any plot within nine tiles is
//! unexplored) stops refusing sites once one of our Settlers has walked
//! [`FOG_STRANDED_STANDARD`] standard turns without founding.
//!
//! **Why (live 6p Emperor, 2026-11-01; -8e's stall census).**
//! - civvis-20261101T042616Z (Alexander, Religion) held three cities from t40
//!   to t127 while its Production share sat at 0.38-0.49, and was retired at
//!   t177. Its Settler was stranded continuously from t44 to t177 ("no legal
//!   city site was reachable, so it steps toward unseen land", 207 lines):
//!   49 refusals read "it lies beyond the empire's Loyalty reach on ground
//!   the seat has not explored", 39 more the Loyalty forecast. Shipbuilding
//!   was in by t51.
//! - `hidden-border-yields-to-our-pressure` was armed on that seat and never
//!   fired: the fog clause returns its veto first in
//!   `settle_site_frontier_loyalty_verdict`, so the hidden-border yield below
//!   it is never reached for a fogged site.
//! - The fog clause guards a colony from an unseen rival's pressure. A
//!   Settler that has walked fifteen turns is not protecting anything; it is
//!   the empire's next city, idle. Four of about 100 seats since 10-30 had
//!   fifteen or more stranded turns in t40-100, which is rare but fatal.
//!
//! **What the gene does.** While any of our Settlers has walked at least
//! [`FOG_STRANDED_STANDARD`] standard turns (`settler_walk_started`, which
//! the gene stamps too), the fog clause does not veto a site. Every other
//! check stands: the native-pressure revolt forecast above it, and the
//! unresolved-border clause and the modelled forecast below it. A founding
//! the yield allowed is journalled at its site, so the live guard can key a
//! Loyalty loss to it.
//!
//! Off, nothing changes.

use super::*;

/// Standard turns a Settler must have walked before the fog clause yields:
/// about fifteen at Online speed.
pub(super) const FOG_STRANDED_STANDARD: u32 = 22;

impl AdvancedAi {
    /// Whether the fog clause yields this turn: the gene is on and a Settler
    /// of ours has walked [`FOG_STRANDED_STANDARD`] standard turns.
    pub(super) fn fog_veto_yields(&self, g: &Game, pid: usize) -> bool {
        if !self.fog_yields_to_a_stranded_settler {
            return false;
        }
        let patience = g.standard_duration(FOG_STRANDED_STANDARD);
        self.settler_walk_started.iter().any(|(uid, started)| {
            g.units
                .get(uid)
                .is_some_and(|unit| unit.owner == pid && unit.kind == "settler")
                && g.turn.saturating_sub(*started) >= patience
        })
    }

    /// Whether the gene is what lets a Settler found at `site`: the site is
    /// beyond reach on fogged ground and the clause yields. Read BEFORE the
    /// founding (the new city changes the reach).
    pub(super) fn fog_yield_before_founding(&self, g: &Game, pid: usize, site: Pos) -> bool {
        self.frontier_loyalty
            && self.fog_veto_yields(g, pid)
            && Self::beyond_loyalty_reach(g, pid, site)
    }

    /// Journals a founding the fog yield allowed.
    pub(super) fn note_fog_founding(&self, site: Pos, yielded: bool) {
        if yielded {
            think!(self.journal(), Expansion, Decision,
                   "Fog veto yields to a stranded Settler at {site:?}";
                   "fog-yields-to-a-stranded-settler: a Settler of ours has walked \
                    {FOG_STRANDED_STANDARD} standard turns; the site lies beyond our Loyalty \
                    reach on unexplored ground"; site);
        }
    }
}

#[cfg(test)]
mod tests;
