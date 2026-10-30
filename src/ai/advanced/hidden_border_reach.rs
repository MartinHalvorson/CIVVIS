//! `hidden-border-yields-to-our-pressure`: the settle veto for a site within
//! five tiles of a rival border whose city may be hidden
//! (`frontier-loyalty`'s unresolved-border arm) yields where our own cities
//! already press the site with [`HIDDEN_BORDER_OWN_PRESSURE`] or more.
//!
//! **Why (live Immortal Pericles, 2026-10-30).** On crowded Immortal maps the
//! veto set aside most remaining sites, and the Settler walked to fallback
//! sites eight to ten tiles out:
//! - civvis-20261030T123145Z, at peace, held a Settler born at t50 still
//!   walking at t80 after sixteen "rival border whose city may be hidden"
//!   refusals and three "Stranded Settler sets aside N unpriceable sites";
//!   the empire stood at four cities at t78 against the leader's seven and
//!   retired at t88 under the production rule (24% of the leader's).
//! - 120109Z logged 45 such refusals, 131948Z nine.
//!
//! The veto exists because the forecast cannot price a city it cannot see.
//! But a site our own cities already press is not a frontier colony: the
//! Loyalty model sums every city within nine tiles, each by its population
//! times its closeness (`10 - distance`), and a site inside the empire's
//! own pressure keeps that counterweight from the turn it is founded.
//!
//! **What the gene does.** The unresolved-border arm is skipped when the sum
//! over our cities within nine tiles of `population × (10 − distance)`
//! reaches `HIDDEN_BORDER_OWN_PRESSURE`. Every other arm of the verdict —
//! a measured native pressure, ground beyond our Loyalty reach, a visible
//! major city's forecast revolt — still refuses. Off, nothing changes.

use super::*;

/// Our own Loyalty pressure on a site, summed `population × (10 − distance)`
/// over our cities within nine tiles, at which the unresolved-border veto
/// yields: one city of six citizens five tiles away, or a capital of ten
/// seven tiles away.
pub(super) const HIDDEN_BORDER_OWN_PRESSURE: i64 = 30;

impl AdvancedAi {
    /// Our own cities' Loyalty pressure on `site`; see the module.
    pub(super) fn own_loyalty_pressure(g: &Game, pid: usize, site: Pos) -> i64 {
        g.cities
            .values()
            .filter(|city| city.owner == pid)
            .map(|city| {
                let distance = i64::from(g.wdist(city.pos, site));
                if distance > 9 {
                    0
                } else {
                    i64::from(city.pop.max(0)) * (10 - distance)
                }
            })
            .sum()
    }

    /// Whether the unresolved-border veto yields at `site`: the gene is on and
    /// our own pressure there reaches `HIDDEN_BORDER_OWN_PRESSURE`.
    pub(super) fn hidden_border_yields(&self, g: &Game, pid: usize, site: Pos) -> bool {
        self.hidden_border_yields_to_our_pressure
            && Self::own_loyalty_pressure(g, pid, site) >= HIDDEN_BORDER_OWN_PRESSURE
    }
}

#[cfg(test)]
mod tests;
