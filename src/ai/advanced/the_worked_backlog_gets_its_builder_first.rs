//! `the-worked-backlog-gets-its-builder-first`: while the worked tiles that
//! need a Builder run [`BACKLOG_FIRST_UNCOVERED_TILES`] or more ahead of the
//! Builder charges in hand and queued, the backlog wants a Builder whatever
//! the stock quota says, and the backlog Builder's claim is asked once more
//! ahead of the Monument, Granary, breaker, deterrent and Trader claims.
//!
//! **Why (32 live Deity Online Gran Colombia Domination games, 2026-11-11
//! 19:57Z to 11-12 00:46Z).** At t60 the seat held a median 1 Builder and 2
//! charges against 7 worked land tiles a Builder could improve at once; 6 of
//! the 32 had no Builder and 3 had charges enough. At t100 it was 2 Builders
//! and 4 charges against 11 tiles. The seat trained or bought 4.7 Builders a
//! game by t60 and spent every charge (a fifth on chops and harvests), so the
//! shortfall is Builders, not their use. On 1,228 turns t1-60 the improvable
//! worked land ran ahead of the charges; a Builder was in training on 203 of
//! them. On the rest:
//!
//! - 460 turns the stock quota wanted no Builder: fewer than
//!   `WORKED_BACKLOG_MIN_TILES` (4) unimproved worked tiles (346), or charges
//!   already at `WORKED_BACKLOG_COVER` (60%) of them (114);
//! - 188 turns the empire had one city, where the step never asks;
//! - 374 turns the quota was open and no Builder came. The idle queues of
//!   those turns went first to the claims that run ahead of the backlog's:
//!   of 301 idle choices, the early Settler floor took 34, the bound Granary
//!   27, the deterrent 29, the conquest opening's reservation 15, and the
//!   backlog Builder 39.
//!
//! When the backlog claim does fire it holds: 61 of its 84 claims t1-60
//! became Builders.
//!
//! **What it does.** (1) The quota: `BasicAi::worked_backlog_builder` also
//! wants a Builder while the unimproved worked tiles exceed the charges by
//! [`BACKLOG_FIRST_UNCOVERED_TILES`] or more, about one charge a tile. Every
//! other gate of the step is unchanged: two cities, a Builder per city at
//! most, the threat, war, besieged, barbarian and Settler-due gates, and the
//! build-turn cap. The claim's hold (`backlog_builder_holds`) reads the same
//! quota, so the governor keeps the Builder it claimed. (2) The order: the
//! claim chain (`run_idle_queue_claims`) asks the backlog Builder's claim once
//! more just after the early Settler floor and the first capital's
//! reservation, so an idle or deferrable queue goes to the Builder before the
//! Monument, Granary, breaker, deterrent and Trader claims take it. The
//! Settler floor's city, the first capital's reserved soldier, science in
//! progress and every head the claim already keeps still keep their queues,
//! and the stock claim keeps its own place below.
//!
//! It rides on `builders-cover-the-worked-backlog` and
//! `backlog-builder-takes-the-queue` (both armed on the live seat); with
//! either off it does nothing. Off, nothing changes.

use super::*;

/// Unimproved worked tiles beyond the Builder charges in hand and queued
/// from which the gene wants another Builder: about one charge a tile, with
/// one tile of slack so that a single newly worked tile calls no Builder.
pub(crate) const BACKLOG_FIRST_UNCOVERED_TILES: i64 = 2;

impl BasicAi {
    /// `the-worked-backlog-gets-its-builder-first`: whether `backlog`
    /// unimproved worked tiles run [`BACKLOG_FIRST_UNCOVERED_TILES`] or more
    /// ahead of `cover` charges. Always false with the gene off.
    pub(crate) fn worked_backlog_runs_ahead(&self, backlog: usize, cover: i32) -> bool {
        self.the_worked_backlog_gets_its_builder_first
            && backlog as i64 - i64::from(cover) >= BACKLOG_FIRST_UNCOVERED_TILES
    }
}

impl AdvancedAi {
    /// The journal's reading of the gene when a backlog Builder is ordered
    /// while the backlog runs ahead: `None` with the gene off or the backlog
    /// covered.
    pub(super) fn worked_backlog_first_note(&self, backlog: usize, cover: i32) -> Option<String> {
        if !self.base.worked_backlog_runs_ahead(backlog, cover) {
            return None;
        }
        let stock = backlog >= BasicAi::WORKED_BACKLOG_MIN_TILES
            && f64::from(cover) < backlog as f64 * BasicAi::WORKED_BACKLOG_COVER;
        Some(format!(
            "the-worked-backlog-gets-its-builder-first: {backlog} worked tiles unimproved \
             against {cover} charges, {} short; the stock quota {}",
            backlog as i64 - i64::from(cover),
            if stock {
                "wants one too"
            } else {
                "would not ask"
            }
        ))
    }

    /// See the module: the backlog Builder's claim, asked ahead of the
    /// Monument, Granary, breaker, deterrent and Trader claims while the
    /// backlog runs ahead of the charges. The first capital's reserved
    /// soldier keeps its queue. Exact no-op with the gene off.
    pub(super) fn claim_worked_backlog_builder_first(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.base.the_worked_backlog_gets_its_builder_first {
            return;
        }
        let backlog = BasicAi::unimproved_worked_tiles(g, pid);
        let cover = BasicAi::builder_charge_cover(g, pid);
        if !self.base.worked_backlog_runs_ahead(backlog, cover) {
            return;
        }
        let reserved = Self::conquest_capital(g, pid).filter(|cid| {
            g.cities
                .get(cid)
                .and_then(|city| city.queue.first())
                .is_some_and(|item| self.first_capital_training_holds(g, pid, *cid, item))
        });
        let Some((turns, cid, builder)) =
            self.backlog_builder_claim_candidate(g, pid, plan, reserved)
        else {
            return;
        };
        let head = g.cities[&cid].queue.first().cloned();
        let name = g.cities[&cid].name.clone();
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: builder,
            },
        )
        .is_ok()
        {
            think!(self.journal(), Economy, Decision,
                   "{name} trains a Builder before the other claims";
                   "the-worked-backlog-gets-its-builder-first: {backlog} worked tiles unimproved \
                    against {cover} charges, {turns:.1} turns, ahead of {}",
                   head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                       "{} (it keeps its progress)", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
