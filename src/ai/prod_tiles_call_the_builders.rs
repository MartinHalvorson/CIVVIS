//! `prod-tiles-call-the-builders`: the worked-backlog Builder quota counts
//! every worked tile that a production improvement (Mine, Lumber Mill, Quarry)
//! fits in full, and the good idle hills and forests at half.
//!
//! **The finding (25 live Deity Online Korea games, 2026-11-04/05).** We work
//! bare production ground for most of the mid-game. Of the plain hills our
//! cities work, 20% carry a Mine at t60, 44% at t80, 77% at t100 and 96% at
//! t140. Of the forests they work, 29% / 60% / 68% / 92% carry a Lumber Mill.
//! The host pays a mined worked Grassland Hills 2/3 against 2/1 bare by t70,
//! and a milled Grassland Forest 2/3 against 2/1. At +2 a tile, the bare
//! worked hills and forests cost 21% / 16% / 13% / 9% / 5% of the empire's
//! Production at t60/70/80/90/100 (a median 8 Production a turn through
//! t60-80). The backlog step (`builders-cover-the-worked-backlog`) fires every
//! two to four turns, yet the logs read 8-15 worked tiles unimproved against
//! 3-7 charges from t50 to t110.
//!
//! **The mechanism.** The step trains a Builder only while the charges in
//! hand and queued cover less than `WORKED_BACKLOG_COVER` (60%) of the
//! unimproved worked tiles. A queued Builder counts its full charges, so one
//! Builder in one queue satisfies the quota and no second city joins it. The
//! empire therefore trains one Builder at a time: about 0.25 a turn, 0.75
//! charges, which is the rate the cities grow onto new ground. A Builder costs
//! 31-57 Production at t40-100 Online, and three charges on worked hills
//! repay it in about six turns.
//!
//! **The gene.** The quota is counted in charges. With the gene off it is
//! `WORKED_BACKLOG_COVER` of the unimproved worked tiles, unchanged. With the
//! gene on, every unimproved worked tile that a production improvement fits
//! wants a whole charge. The rest of the worked backlog keeps the stock 60%.
//! Each owned, unworked, unimproved tile within the work radius that a
//! production improvement fits, and that already yields Food, wants
//! `IDLE_PRODUCTION_TILE_CHARGE`. That last condition is the operator's rule:
//! "ok to skip worse stuff like desert hills if its not something we would be
//! working". A Desert or Snow Hills plot pays no Food, so it enters only once
//! a citizen works it. Every other gate of the step (two cities, one Builder
//! per city at most, the threat and Settler gates, the build-turn cap) is
//! unchanged.

use super::*;

/// The charge one owned, unworked tile with Food asks for when a production
/// improvement fits it: half a worked tile's, so the cities' growth ground
/// waits behind the ground they already work.
pub(crate) const IDLE_PRODUCTION_TILE_CHARGE: f64 = 0.5;

/// A city works its own tiles out to this many rings; tiles its border holds
/// beyond them are no city's growth ground.
const WORK_RADIUS: i32 = 3;

/// See `prod-tiles-call-the-builders`: the production ground that wants a
/// Builder's charge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProductionTileBacklog {
    /// Unimproved (or pillaged) tiles a city works that a production
    /// improvement fits.
    pub(crate) worked: usize,
    /// Owned, unworked, unimproved tiles within the work radius that a
    /// production improvement fits and that already yield Food.
    pub(crate) idle: usize,
}

impl BasicAi {
    /// Whether a Builder improvement that adds Production fits `pos` for
    /// `pid` now.
    pub(crate) fn production_improvement_fits(g: &Game, pid: usize, pos: Pos) -> bool {
        g.valid_improvements(pid, pos).into_iter().any(|improvement| {
            g.rules.improvements[&improvement].builder_buildable
                && g.improvement_yield_change(pid, pos, improvement).production > 0.0
        })
    }

    /// See [`ProductionTileBacklog`].
    pub(crate) fn production_tile_backlog(g: &Game, pid: usize) -> ProductionTileBacklog {
        let _memo = g.query_memo();
        let mut backlog = ProductionTileBacklog::default();
        for cid in g.player_city_ids(pid) {
            let city = &g.cities[&cid];
            let worked: BTreeSet<Pos> = g.city_citizen_plan(cid).worked_tiles.into_iter().collect();
            for pos in &city.owned_tiles {
                if *pos == city.pos || g.wdist(city.pos, *pos) > WORK_RADIUS {
                    continue;
                }
                let Some(tile) = g.map.get(*pos) else {
                    continue;
                };
                if tile.district.is_some()
                    || tile.district_foundation.is_some()
                    || tile.wonder.is_some()
                    || (tile.improvement.is_some() && !tile.pillaged)
                {
                    continue;
                }
                let is_worked = worked.contains(pos);
                if !is_worked && (tile.pillaged || g.workable_tile_yields(*pos).food < 1.0) {
                    continue;
                }
                if !Self::production_improvement_fits(g, pid, *pos) {
                    continue;
                }
                if is_worked {
                    backlog.worked += 1;
                } else {
                    backlog.idle += 1;
                }
            }
        }
        backlog
    }

    /// The Builder charges the worked-backlog step wants in hand and queued,
    /// given `backlog` unimproved worked tiles. Off, this is
    /// `WORKED_BACKLOG_COVER` of the backlog. On, it is one charge per worked
    /// production tile, the stock share of the rest, and
    /// `IDLE_PRODUCTION_TILE_CHARGE` per idle production tile. The gene never
    /// asks for fewer charges than the stock rule.
    pub(crate) fn worked_backlog_charges_wanted(
        &self,
        g: &Game,
        pid: usize,
        backlog: usize,
    ) -> (f64, Option<ProductionTileBacklog>) {
        let stock = backlog as f64 * Self::WORKED_BACKLOG_COVER;
        if !self.prod_tiles_call_the_builders {
            return (stock, None);
        }
        let production = Self::production_tile_backlog(g, pid);
        // The production count is a subset of the backlog's own count, but
        // the two read the board separately. Clamp so an edge case can never
        // count a tile twice.
        let worked = production.worked.min(backlog);
        let wanted = worked as f64
            + (backlog - worked) as f64 * Self::WORKED_BACKLOG_COVER
            + production.idle as f64 * IDLE_PRODUCTION_TILE_CHARGE;
        (wanted.max(stock), Some(production))
    }

    /// The journal's reading of the gene's quota when a backlog Builder is
    /// ordered: `None` with the gene off.
    pub(crate) fn production_tile_quota_note(&self, g: &Game, pid: usize) -> Option<String> {
        let backlog = Self::unimproved_worked_tiles(g, pid);
        let (wanted, production) = self.worked_backlog_charges_wanted(g, pid, backlog);
        let production = production?;
        Some(format!(
            "prod-tiles-call-the-builders: {} worked and {} idle production tiles; the quota \
             asks {wanted:.1} charges against the stock {:.1}",
            production.worked,
            production.idle,
            backlog as f64 * Self::WORKED_BACKLOG_COVER
        ))
    }
}

#[cfg(test)]
#[path = "prod_tiles_call_the_builders_tests.rs"]
mod tests;
