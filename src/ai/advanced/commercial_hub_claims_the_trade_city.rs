//! `commercial-hub-claims-the-trade-city`: from the Commercial Hub step's
//! window (turn 60 live), the city `commercial-hub-and-traders` names for a
//! Commercial Hub (or for the Market of the hub it holds) puts that build at
//! the head of its queue ahead of the delegated governor's rescoring, over an
//! idle queue or a deferrable head. One claim a turn, after the Campus
//! building claim. The same items keep their queue as under the Aqueduct,
//! Industrial Zone and Campus building claims: a Settler, a Builder or
//! Trader, Walls, a wonder, a project, the faith defence, the science and
//! culture items, every other claim's build, the early Settler floor's city,
//! a threatened or freshly attacked city, a city due its Granary, a wall
//! breaker the siege reservation queued
//! ([`AdvancedAi::queued_wall_breaker`]), anything finishing this turn, a
//! military unit while at war with a major, and an idle city with a Settler
//! due. The displaced item keeps its progress. While a walled domination
//! siege is below its gun cap the claim stands down entirely, so the siege
//! reservation (which runs later and reuses only idle or fresh routine
//! queues) still finds its city ([`AdvancedAi::siege_reservation_open`]).
//!
//! **Why (2026-10-11 census, 75 live Immortal GC runs since 08:27Z and 82
//! Emperor since 10-10 18:00Z).** Gold a turn falls behind the best rival
//! from 12 vs 45 at t75 to 15.5 vs 99.5 at t100 and 12.4 vs 171 at t125
//! while unit upkeep rises 9.5 -> 16 -> 25 and buildings and districts add
//! 6 -> 13 -> 25: gross Gold is ~44 at t100 and ~62 at t125 against the
//! rival's net 100-170. We hold a median 0 Commercial Hubs at t100 and 1 at
//! t125 (first hub a median t101; 19 of 75 Immortal games none by t150), no
//! Harbor, and 1-2 trade routes against a capacity of 1-2 -- the hub is the
//! missing route slot, the Market and the Gold adjacency. Over t60-125 a
//! city of ours had a legal Commercial Hub in the host's buildable list but
//! none standing for a median 211 city-turns a game (Emperor 229), while
//! those queues held an idle frame 18%, a Builder 8%, a Campus 6%, a Settler
//! 6%, a Granary 6%, an Aqueduct 5%, a Catapult 5% and Walls 4%.
//! `commercial-hub-and-traders` already names the hub city (its two or three
//! most productive unthreatened cities) and `commercial-hub-in-the-strategic-
//! queue` keeps a queued hub, but both read an idle queue that the
//! controller's claims and the scorer take first -- the Industrial Zone's
//! pattern before its claim.
//!
//! The claim reuses the step's own choice (`BasicAi::commercial_market_item`,
//! then `BasicAi::commercial_hub_item`: at most two hubs, three from six
//! cities, on the best Gold site, done within 20 turns). The deterrent and
//! the governor's rescoring then leave the claimed build alone
//! ([`AdvancedAi::commercial_hub_claim_holds`]). Off, nothing runs.

use super::*;

/// The fewest cities from which the claim opens a hub: the step's own floor.
const COMMERCIAL_CLAIM_MIN_CITIES: usize = 3;

impl AdvancedAi {
    /// The Commercial Hub or Market `cid` would claim and how long it takes:
    /// the step's pick for an unthreatened city not due its Granary, while
    /// the step's window is open. `None` with the gene off.
    pub(super) fn commercial_hub_claim_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64)> {
        if !self.commercial_hub_claims_the_trade_city || !self.base.commercial_hub_window_open(g) {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || self.base.commercial_city_threatened(g, cid)
        {
            return None;
        }
        let n_cities = g.player_city_ids(pid).len();
        if n_cities < COMMERCIAL_CLAIM_MIN_CITIES {
            return None;
        }
        // A city within one of its housing with a Granary still to build
        // leaves its queue to the Granary claim.
        if (city.pop as f64) + 1.0 >= g.city_housing(city)
            && BasicAi::civ_building(g, pid, cid, "granary").is_some()
        {
            return None;
        }
        let item = BasicAi::commercial_market_item(g, pid, cid)
            .or_else(|| self.base.commercial_hub_item(g, pid, cid, n_cities))?;
        if !g.can_produce(pid, cid, &item) {
            return None;
        }
        let production = g.city_yields(cid).production;
        let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
            let remaining =
                g.item_cost_for_city(pid, cid, &item) - g.item_invested_production(cid, &item);
            remaining.max(0.0) / production.max(0.5)
        });
        Some((item, turns))
    }

    /// Whether `item` at the head of `cid` is a Commercial Hub, or the Market
    /// of the hub it holds, that this gene keeps: the step's own hold
    /// (`BasicAi::commercial_hub_build_holds`), still legal. The step stops
    /// naming a city once its hub is queued, so the hold reads the queued
    /// build itself; the deterrent, the other claims and the delegated
    /// governor's rescoring leave it alone. Always false with the gene off.
    pub(super) fn commercial_hub_claim_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        if !self.commercial_hub_claims_the_trade_city {
            return false;
        }
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        city.owner == pid
            && self
                .base
                .commercial_hub_build_holds(g, cid, item, plan.threatened_city)
            && g.can_produce(pid, cid, item)
    }

    /// See the module: the most productive city the Commercial Hub step
    /// names puts its hub (or the hub's Market) at the head of its queue,
    /// idle or over a deferrable item. One claim a turn. Exact no-op with
    /// the gene off.
    pub(super) fn claim_commercial_hub(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.commercial_hub_claims_the_trade_city {
            return;
        }
        // A live walled siege below its gun cap keeps the idle queues for the
        // siege reservation, which runs after the claims and reuses only an
        // idle or fresh routine queue (see `siege_reservation_open`).
        if self.siege_reservation_open(g, pid, plan) {
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
        let mut best: Option<(f64, f64, u32, Item)> = None;
        for cid in city_ids {
            let Some((build, turns)) = self.commercial_hub_claim_item(g, pid, cid, plan) else {
                continue;
            };
            if self.early_settler_floor_holds(g, pid, cid, plan) {
                continue;
            }
            match g.cities[&cid].queue.first() {
                Some(item) => {
                    if *item == build
                        || self.deterrence_unit_holds(g, pid, item)
                        || self.bound_granary_holds(g, pid, cid, plan, item)
                        || self.idle_route_trader_holds(g, pid, cid, plan, item)
                        || self.backlog_builder_holds(g, pid, cid, plan, item)
                        || self.capped_aqueduct_holds(g, pid, cid, plan, item)
                        || self.producer_zone_claim_holds(g, pid, cid, plan, item)
                        || self.campus_building_claim_holds(g, pid, cid, plan, item)
                        || Self::queued_wall_breaker(g, item)
                        || !Self::capped_aqueduct_may_defer(g, pid, cid, item, at_major_war)
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
            // The most productive city first; then the quickest; then the
            // lowest id, so the claim is deterministic.
            let production = g.city_yields(cid).production;
            if best
                .as_ref()
                .is_none_or(|(old_production, old_turns, old_city, _)| {
                    (-production, turns, cid) < (-*old_production, *old_turns, *old_city)
                })
            {
                best = Some((production, turns, cid, build));
            }
        }
        let Some((production, turns, cid, build)) = best else {
            return;
        };
        let head = g.cities[&cid].queue.first().cloned();
        let name = g.cities[&cid].name.clone();
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: build.clone(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Cities, Decision,
                   "{name} claims its {} for trade", Self::plain_item(&build);
                   "commercial-hub-claims-the-trade-city: {production:.0} Production, \
                    {turns:.1} turns, ahead of {}",
                   head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                       "{} (it keeps its progress)", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
