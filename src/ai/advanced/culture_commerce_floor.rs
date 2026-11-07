//! Fund the first three Culture trade routes before routine claims consume
//! every idle opportunity. Native Pericles 080217Z finished at turn 170 with
//! ten cities, one route slot, no Markets and -14.7 Gold/turn: legal three-turn
//! Hubs repeatedly appeared as hints, then fresh Builder/Campus claims won.
//! This reuses the existing commercial step's legal quotes and safe origins.

use super::*;

const CULTURE_ROUTE_FLOOR: i64 = 3;

impl AdvancedAi {
    /// Snapshot one opportunity after survival/expansion claims, before routine
    /// claims. Originally busy cities are eligible only to finish their Hub or
    /// Market. Once three slots and their Traders are covered, this stops.
    pub(super) fn culture_commerce_candidate(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Item)> {
        if !self.commercial_hub_in_the_strategic_queue
            || !self.base.commercial_hub_window_open(g)
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
            || self.lane_lost
            || !g.victory_conditions.culture
        {
            return None;
        }
        let cities = g.player_city_ids(pid);
        if cities.len() < 3
            || !cities
                .iter()
                .any(|cid| g.city_has_district_family(&g.cities[cid], crate::name!("campus")))
            || !cities.iter().any(|cid| {
                g.cities[cid]
                    .buildings
                    .contains(&crate::name!("amphitheater"))
            })
        {
            return None;
        }
        let capacity = g.trade_capacity(pid);
        let covered = capacity - Self::empty_trade_slots(g, pid, None);
        if capacity >= CULTURE_ROUTE_FLOOR && covered >= CULTURE_ROUTE_FLOOR {
            return None;
        }
        let n_cities = cities.len();
        let traders = self.counts(g, pid).traders;
        let chain_in_flight = cities.iter().any(|cid| {
            g.cities[cid].queue.first().is_some_and(|item| {
                self.base
                    .commercial_hub_build_holds(g, *cid, item, plan.threatened_city)
            })
        });
        cities
            .into_iter()
            .filter_map(|cid| {
                if plan.threatened_city == Some(cid) || self.hint_city_in_reach_of_war(g, pid, cid)
                {
                    return None;
                }
                let head = g.cities[&cid].queue.first();
                let held = head.filter(|item| {
                    self.base
                        .commercial_hub_build_holds(g, cid, item, plan.threatened_city)
                        && Self::production_commitment_is_legal(g, pid, cid, item)
                });
                if held.is_none() && (head.is_some() || chain_in_flight) {
                    return None;
                }
                let item = held.cloned().or_else(|| {
                    self.base
                        .commercial_hub_step(g, pid, cid, n_cities, traders)
                })?;
                let trader = matches!(&item, Item::Unit { unit } if *unit == "trader");
                if capacity >= CULTURE_ROUTE_FLOOR && !trader {
                    return None;
                }
                // A Lighthouse already gives this city's route slot. Do not
                // reserve a duplicate Market just to meet the capacity floor.
                if matches!(&item, Item::Building { building } if *building == "market")
                    && g.cities[&cid]
                        .buildings
                        .contains(&crate::name!("lighthouse"))
                {
                    return None;
                }
                let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
                    (g.item_cost_for_city(pid, cid, &item) - g.item_invested_production(cid, &item))
                        .max(0.0)
                        / g.city_yields(cid).production.max(0.5)
                });
                Some((
                    if matches!(item, Item::District { .. }) {
                        1
                    } else {
                        0
                    },
                    turns,
                    cid,
                    item,
                ))
            })
            .min_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)))
            .map(|(_, _, cid, item)| (cid, item))
    }

    /// Routine claims may replace an originally idle opportunity; survival,
    /// faith and Culture buildings retain their place. Pillaged repairs and
    /// the normal defensive writers run after this reservation.
    pub(super) fn claim_culture_commerce(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        cid: u32,
        item: Item,
    ) {
        if plan.threatened_city == Some(cid) {
            return;
        }
        if let Some(head) = g.cities[&cid].queue.first() {
            if *head == item {
                return;
            }
            let routine_builder = matches!(head, Item::Unit { unit } if *unit == "builder");
            let culture = Self::live_gp_cultural_building_commitment(g, pid, cid, head)
                || matches!(head, Item::Building { building } if matches!(building.as_str(),
                    "monument" | "amphitheater" | "art_museum" | "archaeological_museum" | "broadcast_center"))
                || matches!(head, Item::District { district, .. } if g.district_family(*district) == "theater_square");
            if culture
                || self.deterrence_unit_holds(g, pid, head)
                || (!routine_builder && !Self::idle_route_trader_may_defer(g, pid, cid, head, true))
            {
                return;
            }
        }
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: item.clone(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Cities, Decision,
                "{} funds the Culture trade-route floor", g.cities[&cid].name;
                "commercial-hub-in-the-strategic-queue: {} before routine claims; first three slots and Traders", Self::plain_item(&item));
        }
    }
}

#[cfg(test)]
mod tests;
