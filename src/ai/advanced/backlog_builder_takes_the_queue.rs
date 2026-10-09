//! `backlog-builder-takes-the-queue`: while the worked backlog runs ahead of
//! the Builder charges (`builders-cover-the-worked-backlog`'s own gate: four
//! or more worked tiles unimproved and the charges in hand and queued below
//! 60% of them, fewer Builders afield and in training than cities), the city
//! that trains the Builder soonest puts one at the head of its queue, over an
//! idle queue or a deferrable head, ahead of the delegated governor's
//! rescoring. One claim a turn. A Settler, a Builder, a Trader, Walls, a
//! wonder, a project, the faith defence's Holy Site, Shrine or Temple, a
//! Granary or Water Mill, an Aqueduct, a claimed Granary or deterrent, the
//! early Settler floor's city, a threatened or freshly attacked city, and
//! anything finishing this turn keep their queue; at war with a major a
//! military unit keeps its queue too (and the backlog gate itself stands
//! down while the army is short). The displaced item keeps its progress.
//!
//! **Why (2026-10-09 eco7 census, 103 live Emperor GC runs since 10-09).**
//! Per citizen our Production is 2.0 against the rivals' 2.7-2.9 at t50-t100,
//! which is the Emperor handicap's +40% to the AI and nothing more; the gap
//! we can close is the tiles. At t40 only a quarter of our worked non-centre
//! tiles are improved (a third at t60), the empire holds a median 1 Builder
//! (0.25-0.4 per city) and 1-3 charges, and the unimproved worked tiles run
//! 6 (t30) → 8 → 11 → 13 → 14 (t70) against 1 → 1 → 2 → 3 → 4 charges: the
//! backlog gate wants a Builder on 93% of turns t30-t90. Its existing claim
//! (`claim_idle_queue_for_worked_backlog`) fires a median 8 times a game, but
//! only on an idle queue, and the Settler floor, the Granary, the deterrent,
//! the Trader and the routine claims take the idle queues first; its Gold
//! purchase fired 0 times (the reserve holds). An improved tile is worth
//! about one Food (Farm) or one to two Production (Mine, Lumber Mill) a turn
//! for the rest of the game.
//!
//! The claim runs after the Trader claim and before the Prophet race and the
//! routine claims, so a due Settler, the Granary, the deterrent and the Trader
//! keep their order; the governor's rescoring then leaves the claimed Builder
//! alone ([`AdvancedAi::backlog_builder_holds`]). Off, nothing runs.

use super::*;

impl AdvancedAi {
    /// Builders afield plus cities with a Builder at the head of the queue,
    /// `except` excluded.
    fn backlog_builders_in_hand(g: &Game, pid: usize, except: Option<u32>) -> usize {
        let afield = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| g.units[uid].kind == "builder")
            .count();
        let training = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| Some(*cid) != except)
            .filter(|cid| {
                matches!(
                    g.cities[cid].queue.first(),
                    Some(Item::Unit { unit }) if *unit == "builder"
                )
            })
            .count();
        afield + training
    }

    /// The Builder `cid` would train for the backlog and how long it takes,
    /// under the backlog gate of `builders-cover-the-worked-backlog`.
    /// `None` with this gene off.
    fn backlog_builder_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64)> {
        if !self.backlog_builder_takes_the_queue {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
        {
            return None;
        }
        let counts = self.counts(g, pid);
        let n_cities = g.player_city_ids(pid).len();
        let builders = Self::backlog_builders_in_hand(g, pid, Some(cid));
        let turns = self.base.worked_backlog_claim_turns(
            g,
            pid,
            cid,
            n_cities,
            counts.settlers,
            builders,
            counts.military,
        )?;
        Some((
            Item::Unit {
                unit: crate::name!("builder"),
            },
            turns,
        ))
    }

    /// Whether `item` at the head of `cid` is the Builder this gene claims
    /// and the backlog still wants it: the governor's rescoring leaves it
    /// alone. Always false with the gene off.
    pub(super) fn backlog_builder_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        if !self.backlog_builder_takes_the_queue
            || !matches!(item, Item::Unit { unit } if *unit == "builder")
        {
            return false;
        }
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
        {
            return false;
        }
        // The gate as `worked_backlog_builder` reads it, with this city's own
        // queued Builder taken back out of the cover and the count.
        let n_cities = g.player_city_ids(pid).len();
        if n_cities < 2 || Self::backlog_builders_in_hand(g, pid, Some(cid)) >= n_cities {
            return false;
        }
        let backlog = BasicAi::unimproved_worked_tiles(g, pid);
        let own = if city.queue.first() == Some(item) {
            g.builder_charges(pid).max(1)
        } else {
            0
        };
        let cover = BasicAi::builder_charge_cover(g, pid) - own;
        backlog >= BasicAi::WORKED_BACKLOG_MIN_TILES
            && f64::from(cover) < backlog as f64 * BasicAi::WORKED_BACKLOG_COVER
    }

    /// See the module: while the worked backlog runs ahead of the charges,
    /// the city that trains a Builder soonest puts one at the head of its
    /// queue, idle or over a deferrable item. One claim a turn. Exact no-op
    /// with the gene off.
    pub(super) fn claim_backlog_builder(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.backlog_builder_takes_the_queue {
            return;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_barbarian
                && !other.is_minor
                && g.is_at_war(pid, other.id)
        });
        let mut best: Option<(f64, u32, Item)> = None;
        for cid in g.player_city_ids(pid) {
            let Some((builder, turns)) = self.backlog_builder_item(g, pid, cid, plan) else {
                continue;
            };
            if self.early_settler_floor_holds(g, pid, cid, plan) {
                continue;
            }
            // An idle city passed the gate (which already keeps a due
            // Settler's queue); a busy one yields only a deferrable head.
            if let Some(item) = g.cities[&cid].queue.first() {
                if self.deterrence_unit_holds(g, pid, item)
                    || self.bound_granary_holds(g, pid, cid, plan, item)
                    || self.idle_route_trader_holds(g, pid, cid, plan, item)
                    || !Self::idle_route_trader_may_defer(g, pid, cid, item, at_major_war)
                {
                    continue;
                }
            }
            if best
                .as_ref()
                .is_none_or(|(old, old_city, _)| (turns, cid) < (*old, *old_city))
            {
                best = Some((turns, cid, builder));
            }
        }
        let Some((turns, cid, builder)) = best else {
            return;
        };
        let head = g.cities[&cid].queue.first().cloned();
        let name = g.cities[&cid].name.clone();
        let backlog = BasicAi::unimproved_worked_tiles(g, pid);
        let cover = BasicAi::builder_charge_cover(g, pid);
        if g.apply(pid, &Action::Produce { city: cid, item: builder }).is_ok() {
            think!(self.journal(), Economy, Decision,
                   "{name} trains a Builder for the worked backlog";
                   "backlog-builder-takes-the-queue: {backlog} worked tiles unimproved against \
                    {cover} charges, {turns:.1} turns, ahead of {}",
                   head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                       "{} (it keeps its progress)", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
