//! `trader-fills-the-idle-route`: while a trade route slot stands empty (the
//! capacity above the routes running, the Traders afield and the Traders in
//! training), the safe origin city that trains a Trader soonest puts one at
//! the head of its queue, ahead of the delegated governor's rescoring, over an
//! idle queue or a deferrable head. One claim a turn. A Settler, a Builder,
//! Walls, a wonder, a project, the faith defence's Holy Site, Shrine or
//! Temple, a claimed Granary or deterrent, the early Settler floor's city, a
//! threatened or freshly attacked city, and anything finishing this turn keep
//! their queue; at war with a major a military unit keeps its queue too. The
//! displaced item keeps its progress.
//!
//! **Why (2026-10-09 eco5 census, 134 live Emperor GC runs since 10-07 that
//! reached t100).** Foreign Trade opens the first route slot at a median
//! turn 25, and the first route starts at a median turn 69: a 42-turn lag
//! (passers 41, t150 retirees 49). Empty slot-turns by t100 run a median 63
//! (passers 50, retirees 68), and at t100 we hold 2 slots against 1 route.
//! A Trader costs 40 Production (about 3 turns in a working city) and the
//! route options of the time pay 3-6 Gold, or 2-4 Food and 1-2 Production
//! to the origin, every turn, and lay a road. The Trader already has two
//! claims on paper, `BasicAi::pick_item` after the Settler, housing and
//! Builder steps, and `solvency-first-trade-slot`'s reservation in the
//! delegated governor, but both read an idle queue, and in the expansion
//! window the Settler floor, the Granary, the deterrent and the recon and
//! catch-up claims take every idle queue first (G131304Z: slot at t30, a
//! Trader queued t73 and displaced, the first route at t104).
//!
//! The claim runs after the deterrent and before the Prophet race and the
//! routine claims, so a due Settler, the Granary and the deterrent keep their
//! order; the governor's rescoring then leaves the claimed Trader alone
//! ([`AdvancedAi::idle_route_trader_holds`]). Off, nothing runs.

use super::*;

/// The longest the claimed Trader may take, in standard turns (8 live turns
/// at the seat's speed): a slot is worth claiming in a city that can fill it
/// this decade, not in the one that would take the rest of the era.
pub(crate) const IDLE_ROUTE_TRADER_MAX_STANDARD: u32 = 12;

impl AdvancedAi {
    /// Trade route slots no route, Trader afield or Trader in training
    /// answers. A Trader in `except`'s queue is not counted, so the city that
    /// trains the slot's Trader still sees the slot it is filling.
    pub(super) fn empty_trade_slots(g: &Game, pid: usize, except: Option<u32>) -> i64 {
        let capacity = g.trade_capacity(pid);
        if capacity <= 0 {
            return 0;
        }
        let afield = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| g.units[uid].kind == "trader")
            .count() as i64;
        let training = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| Some(*cid) != except)
            .filter(|cid| {
                g.cities[cid]
                    .queue
                    .iter()
                    .any(|item| matches!(item, Item::Unit { unit } if *unit == "trader"))
            })
            .count() as i64;
        capacity - g.active_routes(pid) - afield - training
    }

    /// The Trader `cid` would train for an empty slot, and how long it
    /// takes: a safe, unthreatened origin that can lay a route now and finish
    /// the Trader in time. `None` with the gene off.
    fn idle_route_trader_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64)> {
        if !self.trader_fills_the_idle_route {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || !self.base.safe_trade_origin_for_controller(g, pid, cid)
        {
            return None;
        }
        let trader = Item::Unit {
            unit: crate::name!("trader"),
        };
        if !g.can_produce(pid, cid, &trader) {
            return None;
        }
        let production = g.city_yields(cid).production;
        let turns = g.host_production_turns(cid, &trader).unwrap_or_else(|| {
            let remaining =
                g.item_cost_for_city(pid, cid, &trader) - g.item_invested_production(cid, &trader);
            remaining.max(0.0) / production.max(0.5)
        });
        (turns <= g.standard_duration(IDLE_ROUTE_TRADER_MAX_STANDARD) as f64)
            .then_some((trader, turns))
    }

    /// Whether `item` at the head of `cid` is the Trader this gene claims
    /// and its slot still stands empty: the governor's rescoring leaves it
    /// alone. Always false with the gene off.
    pub(super) fn idle_route_trader_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        matches!(item, Item::Unit { unit } if *unit == "trader")
            && Self::empty_trade_slots(g, pid, Some(cid)) > 0
            && self
                .idle_route_trader_item(g, pid, cid, plan)
                .is_some_and(|(trader, _)| trader == *item)
    }

    /// Whether the queue head `item` may wait for the Trader: not a Settler
    /// or a Builder, Walls, a wonder, a project or a repair, not the faith
    /// defence's Holy Site, Shrine or Temple, not an Aqueduct, not finishing
    /// this turn, and not a military unit while a major is at war with us.
    fn idle_route_trader_may_defer(
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        at_major_war: bool,
    ) -> bool {
        let remaining =
            g.item_cost_for_city(pid, cid, item) - g.item_invested_production(cid, item);
        if remaining <= g.city_yields(cid).production {
            return false;
        }
        match item {
            Item::Unit { unit } => match g.rules.units.get(unit) {
                Some(spec) if spec.class == "military" => !at_major_war,
                Some(_) => *unit == "scout",
                None => false,
            },
            Item::Building { building } => g.rules.buildings.get(building).is_some_and(|spec| {
                !spec.wonder
                    && !matches!(
                        building.as_str(),
                        "walls"
                            | "medieval_walls"
                            | "renaissance_walls"
                            | "shrine"
                            | "temple"
                            | "granary"
                            | "water_mill"
                    )
            }),
            Item::District { district, .. } => {
                let family = g.district_family(*district);
                family != crate::name!("aqueduct")
                    && family != crate::name!("holy_site")
                    && g.item_invested_production(cid, item) <= 0.0
            }
            _ => false,
        }
    }

    /// See the module: while a trade slot stands empty, the safe origin that
    /// trains a Trader soonest puts one at the head of its queue, idle or
    /// over a deferrable item. One claim a turn. Exact no-op with the gene
    /// off.
    pub(super) fn claim_trader_for_idle_route(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.trader_fills_the_idle_route || Self::empty_trade_slots(g, pid, None) <= 0 {
            return;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_barbarian
                && !other.is_minor
                && g.is_at_war(pid, other.id)
        });
        let city_ids = g.player_city_ids(pid);
        let n_cities = city_ids.len();
        let settlers = self.counts(g, pid).settlers;
        let mut best: Option<(f64, u32, Item)> = None;
        for cid in city_ids {
            let Some((trader, turns)) = self.idle_route_trader_item(g, pid, cid, plan) else {
                continue;
            };
            if self.early_settler_floor_holds(g, pid, cid, plan) {
                continue;
            }
            match g.cities[&cid].queue.first() {
                Some(item) => {
                    if self.deterrence_unit_holds(g, pid, item)
                        || self.bound_granary_holds(g, pid, cid, plan, item)
                        || !Self::idle_route_trader_may_defer(g, pid, cid, item, at_major_war)
                    {
                        continue;
                    }
                }
                // An idle city with a Settler due leaves its queue to the
                // Settler step: the city count is what the gate reads.
                None => {
                    if self.base.settler_due(g, pid, cid, n_cities, settlers) {
                        continue;
                    }
                }
            }
            if best
                .as_ref()
                .is_none_or(|(old, old_city, _)| (turns, cid) < (*old, *old_city))
            {
                best = Some((turns, cid, trader));
            }
        }
        let Some((turns, cid, trader)) = best else {
            return;
        };
        let head = g.cities[&cid].queue.first().cloned();
        let name = g.cities[&cid].name.clone();
        let slots = Self::empty_trade_slots(g, pid, None);
        if g.apply(pid, &Action::Produce { city: cid, item: trader }).is_ok() {
            think!(self.journal(), Economy, Decision,
                   "{name} trains a Trader for an empty trade route";
                   "trader-fills-the-idle-route: {slots} slot(s) of {} empty, {turns:.1} turns, \
                    ahead of {}",
                   g.trade_capacity(pid),
                   head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                       "{} (it keeps its progress)", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
