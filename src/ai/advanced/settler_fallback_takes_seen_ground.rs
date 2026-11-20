//! `settler-fallback-takes-seen-ground`: when the preferred site search
//! finds nothing, the Settler's wider fallback takes seen ground before it
//! walks toward a plot the seat has never seen.
//!
//! ## What the live runs say (110 Deity runs, 2026-11-19 12:00Z to 11-20)
//!
//! - `settler-never-idles`' exhaustion fallback ("Settler takes a site the
//!   preferred search refused") named 626 sites in 94 of the 110 runs. 217 of
//!   them (35%) ended at the Settler's next change of destination as "the old
//!   site is water" (187) or "impassable" (30): plots the board marked
//!   `unknown` when they were named, which no seen plot can be. The walks
//!   toward them lasted 643 turns in all, about three each.
//! - Its ranked list is the advanced scan, which values an unknown plot from
//!   its seen neighbours, so a fog plot on a coast can outrank every seen
//!   site. `settler-keeps-its-named-site` already refuses unknown plots in the
//!   ranked pick (of 141 such plots later revealed, 79 were Coast or Ocean and
//!   5 Mountains, and 15 of 153 were ever founded), but its header says the
//!   exhaustion fallbacks still look into the fog, as before.
//! - Run civvis-20261120T071206Z (Mali, Lv 8 Deity): the Settler built on t10
//!   was sent by this fallback to three plots across the map seam that it had
//!   never seen, (-13, 27), (-15, 30) and (-15, 31), worth 15 to 21; each was
//!   set aside the turn after as water, water, impassable, then (-11, 24) as
//!   water on t15. Only then did the legal-site tier name the seen (53, 25),
//!   worth 160.8, seven tiles the other way.
//!
//! ## What the gene does
//!
//! Inside `settler_exhaustion_target` (`advanced/settler_never_idles.rs`):
//!
//! 1. The ranked tier (tier 2) asks its seen sites first, with the same
//!    filters, reachability and Loyalty forecast as before.
//! 2. If none passes, the legal-site tier (tier 3) is asked over seen plots
//!    only, with its own safety, Loyalty and route checks unchanged.
//! 3. Only then are the ranked tier's unseen plots asked, in its order, and
//!    after them tier 3 over every plot, exactly as before.
//!
//! A seen site taken while an unseen plot ranked above it is journalled as
//! "Settler takes seen ground before the fog", naming both plots.
//!
//! ## What is untouched
//!
//! The preferred search, the step-risk drop and the capture and
//! hostile-reach holds; the stranded Settler's frontier step toward unseen
//! land (`settler_stranded`, `settler_frontier_step`), which runs only when
//! both tiers return nothing; every filter of both tiers. With no unknown
//! plot in the ranked list (every native board, and most live frames) the
//! order is the historical one. Off (the default), nothing changes.

use super::{AdvancedAi, Game};
use crate::think;
use crate::Pos;

impl AdvancedAi {
    /// Whether the board marks `pos` unknown: ground this seat has never seen.
    pub(super) fn plot_is_unseen(g: &Game, pos: Pos) -> bool {
        g.map.get(pos).is_some_and(|tile| g.rules.is_unknown(tile))
    }

    /// Move the unseen plots out of the exhaustion fallback's ranked list and
    /// return them; both keep their ranked order. Off, `ranked` is untouched
    /// and the result is empty.
    pub(super) fn exhaustion_fog_set_aside(
        &self,
        g: &Game,
        ranked: &mut Vec<(Pos, f64)>,
    ) -> Vec<(Pos, f64)> {
        if !self.settler_fallback_takes_seen_ground {
            return Vec::new();
        }
        let (seen, fog): (Vec<_>, Vec<_>) = std::mem::take(ranked)
            .into_iter()
            .partition(|(pos, _)| !Self::plot_is_unseen(g, *pos));
        *ranked = seen;
        fog
    }

    /// Journal a seen site taken while an unseen plot ranked above it.
    /// `worth` is the ranked value of a tier-2 site; `None` is the tier-3
    /// legal-site search over seen plots, asked when no seen ranked site
    /// passed. Nothing is written when the ranked list held no unseen plot.
    pub(super) fn journal_fog_passed_over(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        site: Pos,
        worth: Option<f64>,
        fog: &[(Pos, f64)],
    ) {
        let Some(&(plot, plot_worth)) = fog.first() else {
            return;
        };
        if worth.is_some_and(|worth| worth >= plot_worth) {
            // The seen site ranked first anyway: nothing was passed over.
            return;
        }
        let from = g.units[&uid].pos;
        let tier = if worth.is_some() {
            "the wider ranking"
        } else {
            "the legal-site search"
        };
        let visible = self.battlefront_visibility(g, pid);
        think!(self.journal(), Expansion, Detail,
               "Settler takes seen ground before the fog";
               "settler-fallback-takes-seen-ground: {} unseen plot(s) ranked first, the best \
                {plot:?} (worth {plot_worth:.1}, {} tiles away); {tier} names {site:?}, seen, {} \
                tiles away, worth {:.1} ({:.1} after visible threats)",
               fog.len(), g.wdist(from, plot), g.wdist(from, site),
               self.settle_value(g, pid, site),
               self.settle_value_visible(g, pid, site, &visible); site);
    }
}

#[cfg(test)]
mod tests;
