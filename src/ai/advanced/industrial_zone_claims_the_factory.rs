//! `industrial-zone-claims-the-factory`: once Industrialization is known, the
//! Industrial Zone city whose Factory would newly reach the most of our cities
//! (a Factory pays every city centre within six tiles of its zone) puts that
//! Factory at the head of its queue; once the empire has Coal, the Factory
//! city whose Coal Power Plant would power the most of our unpowered Factories
//! puts the plant there. One claim a turn, over an idle queue or a deferrable
//! head, with `industrial-zone-claims-the-producer-queue`'s rules exactly: a
//! Settler, a Builder or Trader, Walls, a wonder, a project, the faith
//! defence's Holy Site, Shrine or Temple, the science and culture items
//! (Campus, Theater Square, Government Plaza, Monument, Library, University,
//! Amphitheater), a claimed Granary, Aqueduct, Industrial Zone, Workshop,
//! deterrent, Trader or backlog Builder, the early Settler floor's city, a
//! city due its Settler (whatever its head holds), a threatened or freshly
//! attacked city, a city due its Granary, a queued wall
//! breaker ([`AdvancedAi::queued_wall_breaker`]), and anything finishing this
//! turn keep their queue; at war with a major a military unit keeps its queue
//! too. The displaced item keeps its progress.
//!
//! **Why (2026-10-13 census, 17 live Immortal GC runs on 9582d5d3c).** After
//! turn 100 the rivals' Production per citizen climbs from about 2.8 to
//! 4.5-6.8 by turn 125-150 while ours stays at 2.0-2.5, and our share of the
//! second-best rival's Production falls (median x0.83 from t100 to t150 over
//! the lane's ~60 Immortal games that reached t150). The visible Industrial
//! Zone counts are level (ours 2-4 at t125-150, after the zone claims), but
//! what stands in them is not: across the 11 games at turn 150 we held 25
//! Workshops, 9 Factories and 1 Coal Power Plant, with Industrialization
//! known in 9 of them (median t118). Our cities spent 679 city-turns with a
//! Workshop and no Factory after Industrialization, only 53 of them building
//! one (University 54, Builder 48, Campus projects 36, Theater 32, Spies 23,
//! Bombards 22 ...), and 119 city-turns with a Factory and no power, while
//! Coal income was positive in 74% of samples (median +3, stockpile at its
//! cap of 25). A Factory pays +3 to every city within six tiles of its zone
//! and +3 more when powered, so on a compact 8-10 city empire two Factories
//! and a plant are worth roughly 25-50 Production, against our 165-235 at
//! turn 150. The Industrial Zone claim stops at the Workshop.
//!
//! The Factory goes where it reaches the most cities no standing Factory
//! reaches; the plant goes where it powers the most unpowered Factories. Each
//! must finish within the producer step's `PRODUCER_ZONE_MAX_TURNS`. The
//! claim runs after the Industrial Zone claim and before the Campus building
//! claim; the deterrent, the Campus building claim and the governor's
//! rescoring then leave the claimed build alone
//! ([`AdvancedAi::factory_claim_holds`]). Off, nothing runs.

use super::*;
use crate::ai::industrial_zones::PRODUCER_ZONE_MAX_TURNS;

/// The fewest cities from which the claim builds: the producer step's floor.
const FACTORY_CLAIM_MIN_CITIES: usize = 3;
/// Coal in the stockpile that runs a plant without income: a plant burns one
/// a turn.
const FACTORY_CLAIM_COAL_STOCK: f64 = 4.0;

/// A Factory or the building a civilization replaces it with.
fn is_factory(g: &Game, building: &Name) -> bool {
    *building == "factory"
        || g.rules
            .buildings
            .get(building)
            .is_some_and(|spec| spec.replaces == Some(crate::name!("factory")))
}

/// A Coal Power Plant or its replacement.
fn is_coal_plant(g: &Game, building: &Name) -> bool {
    *building == "coal_power_plant"
        || g.rules
            .buildings
            .get(building)
            .is_some_and(|spec| spec.replaces == Some(crate::name!("coal_power_plant")))
}

/// Where `city`'s regional buildings reach from: its Industrial Zone, else
/// its centre.
fn industry_origin(g: &Game, city: &crate::game::City) -> Pos {
    g.city_district_family_position(city, crate::name!("industrial_zone"))
        .unwrap_or(city.pos)
}

impl AdvancedAi {
    /// Our cities a Factory in `cid` would newly reach: city centres within
    /// the Factory's regional range of `cid`'s zone that no standing Factory
    /// of ours already reaches.
    pub(super) fn factory_new_reach(g: &Game, pid: usize, cid: u32) -> usize {
        let range = g
            .rules
            .buildings
            .get(&crate::name!("factory"))
            .map_or(6, |spec| spec.regional_range);
        let Some(city) = g.cities.get(&cid) else {
            return 0;
        };
        let origin = industry_origin(g, city);
        let standing: Vec<Pos> = g
            .cities
            .values()
            .filter(|other| other.owner == pid && other.buildings.iter().any(|b| is_factory(g, b)))
            .map(|other| industry_origin(g, other))
            .collect();
        g.cities
            .values()
            .filter(|other| {
                other.owner == pid
                    && g.wdist(origin, other.pos) <= range
                    && standing
                        .iter()
                        .all(|from| g.wdist(*from, other.pos) > range)
            })
            .count()
    }

    /// Our unpowered Factory cities a Coal Power Plant in `cid` would power:
    /// within the plant's regional range of `cid`'s zone.
    pub(super) fn plant_new_power(g: &Game, pid: usize, cid: u32) -> usize {
        let range = g
            .rules
            .buildings
            .get(&crate::name!("coal_power_plant"))
            .map_or(6, |spec| spec.regional_range);
        let Some(city) = g.cities.get(&cid) else {
            return 0;
        };
        let origin = industry_origin(g, city);
        g.cities
            .values()
            .filter(|other| {
                other.owner == pid
                    && other.buildings.iter().any(|b| is_factory(g, b))
                    && !g.city_is_powered(other)
                    && g.wdist(origin, other.pos) <= range
            })
            .count()
    }

    /// The Factory (else the Coal Power Plant) `cid` would claim, the cities
    /// it would newly reach or power, and how long it takes: in an
    /// unthreatened city of an empire of at least three cities, not due its
    /// Granary, legal now and done within `PRODUCER_ZONE_MAX_TURNS`. The plant
    /// needs Coal income or a stockpile that runs it. `None` with the gene off.
    pub(super) fn factory_claim_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, usize, f64)> {
        if !self.industrial_zone_claims_the_factory {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || g.player_city_ids(pid).len() < FACTORY_CLAIM_MIN_CITIES
            || !g.city_has_district_family(city, crate::name!("industrial_zone"))
        {
            return None;
        }
        // A city within one of its housing with a Granary still to build
        // leaves its queue to the Granary claim.
        if (city.pop as f64) + 1.0 >= g.city_housing(city)
            && BasicAi::civ_building(g, pid, cid, "granary").is_some()
        {
            return None;
        }
        let (item, reach) = if let Some(factory) = BasicAi::civ_building(g, pid, cid, "factory") {
            (factory, Self::factory_new_reach(g, pid, cid))
        } else {
            let coal = g.strategic_resource_rate(pid, "coal") > 0.0
                || g.strategic_stockpile(pid, crate::name!("coal")) >= FACTORY_CLAIM_COAL_STOCK;
            if !coal {
                return None;
            }
            let plant = BasicAi::civ_building(g, pid, cid, "coal_power_plant")?;
            (plant, Self::plant_new_power(g, pid, cid))
        };
        if reach == 0 {
            return None;
        }
        let production = g.city_yields(cid).production;
        let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
            let remaining =
                g.item_cost_for_city(pid, cid, &item) - g.item_invested_production(cid, &item);
            remaining.max(0.0) / production.max(0.5)
        });
        (turns <= PRODUCER_ZONE_MAX_TURNS).then_some((item, reach, turns))
    }

    /// Whether `item` at the head of `cid` is a Factory or Coal Power Plant
    /// this gene would keep: still legal, in an unthreatened city of ours.
    /// The deterrent, the Campus building claim and the delegated governor's
    /// rescoring leave it alone. Always false with the gene off.
    pub(super) fn factory_claim_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        if !self.industrial_zone_claims_the_factory {
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
        let industry = matches!(item, Item::Building { building }
            if is_factory(g, building) || is_coal_plant(g, building));
        industry && g.can_produce(pid, cid, item)
    }

    /// Whether the strategic governor would start a Settler in `cid` now: room
    /// under the settlement target, a free pipeline slot, two citizens, the
    /// expansion window, the factory coordination's pick and a practical
    /// site -- the admission it applies to every post-opening Settler. The
    /// claim runs before that governor, so it asks the same question rather
    /// than waiting to see a queued Settler. (The same test as fork-34's
    /// unpinned `settler_due_in`, 878ed3b04.)
    fn factory_claim_settler_due(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        counts: &EmpireCounts,
    ) -> bool {
        let settler = Item::Unit {
            unit: crate::name!("settler"),
        };
        let city_count = g.player_city_ids(pid).len();
        let desired = self.settlement_target(plan);
        if city_count + counts.settlers >= desired
            || counts.settlers
                >= self.settler_pipeline_width(g, pid, desired, city_count, counts.settlers)
        {
            return false;
        }
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        if city.pop < 2
            || !g.can_produce(pid, cid, &settler)
            || !self.settler_expansion_window_open(g, pid, cid)
        {
            return false;
        }
        if self.settler_factory_coordination
            && !self.settler_factory_is_competitive(g, pid, cid, plan, counts)
        {
            return false;
        }
        self.base.has_practical_settle_site(g, pid)
    }

    /// See the module: the city whose Factory reaches the most new cities (or
    /// whose plant powers the most Factories) puts it at the head of its
    /// queue, idle or over a deferrable item. One claim a turn. Exact no-op
    /// with the gene off.
    pub(super) fn claim_factory(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.industrial_zone_claims_the_factory {
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
        let counts = self.counts(g, pid);
        let settlers = counts.settlers;
        let mut best: Option<(usize, f64, f64, u32, Item)> = None;
        for cid in city_ids {
            let Some((item, reach, turns)) = self.factory_claim_item(g, pid, cid, plan) else {
                continue;
            };
            // A city due its Settler, by the basic step's test or by the
            // strategic governor's own admission, leaves its queue to the
            // Settler whatever stands at its head: the city count is what the
            // gate reads. Replays of 12 games of 10-13 without the governor's
            // test lost 3 of 17 Settler starts at t100-150, each in a claim
            // city on the claim's turn.
            if self.early_settler_floor_holds(g, pid, cid, plan)
                || self.base.settler_due(g, pid, cid, n_cities, settlers)
                || self.factory_claim_settler_due(g, pid, cid, plan, &counts)
            {
                continue;
            }
            if let Some(head) = g.cities[&cid].queue.first() {
                if *head == item
                    || self.deterrence_unit_holds(g, pid, head)
                    || self.bound_granary_holds(g, pid, cid, plan, head)
                    || self.idle_route_trader_holds(g, pid, cid, plan, head)
                    || self.backlog_builder_holds(g, pid, cid, plan, head)
                    || self.capped_aqueduct_holds(g, pid, cid, plan, head)
                    || self.capped_neighborhood_holds(g, pid, cid, plan, head)
                    || self.producer_zone_claim_holds(g, pid, cid, plan, head)
                    || Self::queued_wall_breaker(g, head)
                    || !Self::capped_aqueduct_may_defer(g, pid, cid, head, at_major_war)
                {
                    continue;
                }
            }
            // The most cities reached first; then the most productive; then
            // the quickest; then the lowest id, so the claim is deterministic.
            let production = g.city_yields(cid).production;
            if best
                .as_ref()
                .is_none_or(|(old_reach, old_production, old_turns, old_city, _)| {
                    (std::cmp::Reverse(reach), -production, turns, cid)
                        < (
                            std::cmp::Reverse(*old_reach),
                            -*old_production,
                            *old_turns,
                            *old_city,
                        )
                })
            {
                best = Some((reach, production, turns, cid, item));
            }
        }
        let Some((reach, production, turns, cid, item)) = best else {
            return;
        };
        let head = g.cities[&cid].queue.first().cloned();
        let name = g.cities[&cid].name.clone();
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
                   "{name} claims its {} for the region's industry", Self::plain_item(&item);
                   "industrial-zone-claims-the-factory: reaches {reach} more of our cities, \
                    {production:.0} Production, {turns:.1} turns, ahead of {}",
                   head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                       "{} (it keeps its progress)", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
