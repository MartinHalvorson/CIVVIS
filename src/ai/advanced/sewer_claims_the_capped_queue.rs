//! `sewer-claims-the-capped-queue`: a city of at least
//! [`CAPPED_SEWER_MIN_POP`] within one of its housing, with Sanitation known
//! and no Sewer, that can finish its Sewer within
//! [`CAPPED_SEWER_MAX_STANDARD`] standard turns puts it at the head of its
//! queue, over an idle queue or a deferrable head. One claim a turn, for the
//! most capped city. It is the last of the housing claims and runs after the
//! Granary, Aqueduct and Neighborhood claims. The head tests are the
//! Aqueduct's ([`AdvancedAi::capped_aqueduct_may_defer`]), and on top of them
//! a Research Lab, a Factory, a power plant, a Spaceport and every project
//! keep their queue: a Sewer never delays the Science race itself.
//!
//! **Why (live Lv 8 Deity Korea and Indonesia Online, 76 games, 2026-11-05).**
//! 41-48% of city-turns from t40 to t150 stand within one of housing, and
//! about 75% of those have more than +2 food: growth runs at a mean ×0.73 of
//! its rate. Granary, Aqueduct and Neighborhood each have a claim; no AI code
//! named the Sewer. Sanitation lands at a median t126, a built Sewer follows a
//! median 25 turns later, and at t160 only 22% of cities hold one while 36% of
//! all cities are capped without one. A capped coastal city usually cannot
//! build an Aqueduct, so this is often its only +2 left. Off, nothing runs.

use super::*;

/// The longest the claimed Sewer may take, in standard turns (12 live turns
/// at Online): it pays through growth, and the Science race ends near t190.
pub(crate) const CAPPED_SEWER_MAX_STANDARD: u32 = 18;

/// The smallest city the claim builds for: it is a late, expensive building.
pub(crate) const CAPPED_SEWER_MIN_POP: i32 = 5;

impl AdvancedAi {
    /// The Sewer `cid` would claim, how long it takes and how far over its
    /// housing the city stands. `None` with the gene off.
    pub(super) fn capped_sewer_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64, f64)> {
        if !self.sewer_claims_the_capped_queue {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || city.pop < CAPPED_SEWER_MIN_POP
        {
            return None;
        }
        let over = city.pop as f64 + 1.0 - g.city_housing(city);
        if over < 0.0 {
            return None;
        }
        // `civ_building` answers only what the city can start, so a Sewer
        // already built or without Sanitation reads `None`.
        let item = BasicAi::civ_building(g, pid, cid, "sewer")?;
        let production = g.city_yields(cid).production;
        let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
            let remaining =
                g.item_cost_for_city(pid, cid, &item) - g.item_invested_production(cid, &item);
            remaining.max(0.0) / production.max(0.5)
        });
        (turns <= g.standard_duration(CAPPED_SEWER_MAX_STANDARD) as f64)
            .then_some((item, turns, over))
    }

    /// Whether `item` is the Sewer this city still wants. Always false with
    /// the gene off.
    pub(super) fn capped_sewer_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        self.capped_sewer_item(g, pid, cid, plan)
            .is_some_and(|(sewer, _, _)| sewer == *item)
    }

    /// The Aqueduct's deferrable test, less what the Science race needs.
    pub(super) fn capped_sewer_may_defer(
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        at_major_war: bool,
    ) -> bool {
        let race_item = match item {
            Item::Building { building } => matches!(
                building.as_str(),
                "research_lab"
                    | "factory"
                    | "coal_power_plant"
                    | "oil_power_plant"
                    | "nuclear_power_plant"
            ),
            Item::District { district, .. } => {
                g.district_family(*district) == crate::name!("spaceport")
            }
            _ => false,
        };
        !race_item && Self::capped_aqueduct_may_defer(g, pid, cid, item, at_major_war)
    }

    /// See the module: the most capped city with a timely Sewer puts it at
    /// the head of its queue. One claim a turn. Exact no-op with the gene off.
    pub(super) fn claim_capped_sewer(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.sewer_claims_the_capped_queue {
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
            let Some((sewer, turns, over)) = self.capped_sewer_item(g, pid, cid, plan) else {
                continue;
            };
            if self.early_settler_floor_holds(g, pid, cid, plan) {
                continue;
            }
            match g.cities[&cid].queue.first() {
                Some(item) => {
                    if *item == sewer
                        || self.deterrence_unit_holds(g, pid, item)
                        || self.bound_granary_holds(g, pid, cid, plan, item)
                        || self.idle_route_trader_holds(g, pid, cid, plan, item)
                        || self.backlog_builder_holds(g, pid, cid, plan, item)
                        || self.capped_aqueduct_holds(g, pid, cid, plan, item)
                        || self.capped_neighborhood_holds(g, pid, cid, plan, item)
                        || Self::queued_wall_breaker(g, item)
                        || !Self::capped_sewer_may_defer(g, pid, cid, item, at_major_war)
                    {
                        continue;
                    }
                }
                None => {
                    if self.base.settler_due(g, pid, cid, n_cities, settlers) {
                        continue;
                    }
                }
            }
            // The most capped city first; then the quickest; then the lowest
            // id, so the claim is deterministic.
            if best
                .as_ref()
                .is_none_or(|(old_over, old_turns, old_city, _)| {
                    (-over, turns, cid) < (-*old_over, *old_turns, *old_city)
                })
            {
                best = Some((over, turns, cid, sewer));
            }
        }
        let Some((over, turns, cid, sewer)) = best else {
            return;
        };
        let name = g.cities[&cid].name.clone();
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: sewer,
            },
        )
        .is_ok()
        {
            think!(self.journal(), Cities, Decision,
                   "{name} claims its Sewer for its housing";
                   "sewer-claims-the-capped-queue: {over:.1} over its housing line, \
                    {turns:.1} turns");
        }
    }
}

#[cfg(test)]
mod tests;
