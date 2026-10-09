//! `granary-claims-the-bound-queue`: a city within one of its housing whose
//! Granary it can finish within [`BOUND_GRANARY_MAX_STANDARD`] standard turns
//! puts that Granary at the head of its queue ahead of the controller's
//! routine claims: the deterrent, the worked backlog's Builder, the recon and
//! catch-up claims, and the delegated governor's rescoring. A Settler (which
//! relieves the housing; an idle city with one due is left to the Settler
//! step), Walls, a wonder, a project, the faith defence's Holy Site, Shrine or
//! Temple, a threatened or freshly attacked city, and anything finishing this
//! turn keep their queue; at war with a major a military unit keeps its queue
//! too. The displaced item keeps its progress.
//!
//! **Why (2026-10-09 eco3 census, 145 live Emperor GC runs, turns 25-90).**
//! 21,729 city-turns were housing-bound (the host's growth multiplier below
//! 1: 0.5 at one under housing, 0.25 at or over it), and 13,409 of them (62%)
//! had no Granary while one was buildable. They came in 1,092 streaks, median
//! 9 turns, p75 18, p90 29, against a median 5.3 turns (83% within 8) to
//! build the Granary at the city's own Production. What held those queues,
//! by city-turns: Walls under a siege or barbarian alarm 891 (kept), the
//! worked backlog's Builder 757, the religious defence's Holy Site 710, the
//! early Settler floor 683 (kept), near-rival-deterrence 595, the routine
//! Archer 534, Settler 470 (kept) and Monument 442, the founder's second
//! source 421, the sea-scout Galley 377, the Plaza 483 and the research
//! catch-up Campus 387. `housing-bound-city-builds-its-granary` already puts
//! the Granary first in `BasicAi::pick_item`, but those claims run before the
//! delegated governor and take the idle queue first; a deterrent then
//! displaces a Granary in progress (Quito, civvis-20261009T061536Z t46: 15 of
//! 32 built, then 25 turns of Archers and Walls at population 1 of 2).
//!
//! The claim runs after the religious defence and the early Settler floor
//! and before the deterrent, so a due Settler and the faith defence keep
//! their order; the deterrent and the delegated governor's rescoring then
//! leave the claimed Granary alone ([`AdvancedAi::bound_granary_holds`]).
//! Off, nothing runs.

use super::*;

/// The longest the claimed Granary may take, in standard turns (10 live turns
/// at the seat's speed). 83% of the census's Granaries were within 8.
pub(crate) const BOUND_GRANARY_MAX_STANDARD: u32 = 15;

impl AdvancedAi {
    /// Whether `cid` is housing-bound, unthreatened and short of a Granary it
    /// can build in time; the Granary item when it is. `None` with the gene
    /// off.
    pub(super) fn bound_granary_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64)> {
        if !self.granary_claims_the_bound_queue {
            return None;
        }
        let city = &g.cities[&cid];
        if plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || (city.pop as f64) + 1.0 < g.city_housing(city)
        {
            return None;
        }
        let granary = BasicAi::civ_building(g, pid, cid, "granary")?;
        let production = g.city_yields(cid).production;
        let turns = g.host_production_turns(cid, &granary).unwrap_or_else(|| {
            let remaining = g.item_cost_for_city(pid, cid, &granary)
                - g.item_invested_production(cid, &granary);
            remaining.max(0.0) / production.max(0.5)
        });
        (turns <= g.standard_duration(BOUND_GRANARY_MAX_STANDARD) as f64).then_some((granary, turns))
    }

    /// Whether `item` at the head of `cid` is the Granary this gene claims and
    /// the city still wants it: the deterrent and the delegated governor's
    /// rescoring leave it alone. Always false with the gene off.
    pub(super) fn bound_granary_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        self.bound_granary_item(g, pid, cid, plan)
            .is_some_and(|(granary, _)| granary == *item)
    }

    /// Whether the queue head `item` may wait for the Granary: not a Settler,
    /// Walls, a wonder, a project or a repair, not the faith defence's Holy
    /// Site, Shrine or Temple, not finishing this turn, and not a military
    /// unit while a major is at war with us.
    fn bound_granary_may_defer(
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
            Item::Unit { unit } => {
                if *unit == "settler" {
                    return false;
                }
                match g.rules.units.get(unit) {
                    Some(spec) if spec.class == "military" => !at_major_war,
                    Some(_) => matches!(unit.as_str(), "builder" | "scout" | "trader"),
                    None => false,
                }
            }
            Item::Building { building } => g.rules.buildings.get(building).is_some_and(|spec| {
                !spec.wonder
                    && !matches!(
                        building.as_str(),
                        "walls" | "medieval_walls" | "renaissance_walls" | "shrine" | "temple"
                    )
            }),
            Item::District { district, .. } => {
                let family = g.district_family(*district);
                family != crate::name!("aqueduct") && family != crate::name!("holy_site")
            }
            _ => false,
        }
    }

    /// See the module: every housing-bound city short of a Granary it can
    /// build in time puts it at the head of its queue, idle or over a
    /// deferrable item. Exact no-op with the gene off.
    pub(super) fn claim_bound_granaries(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.granary_claims_the_bound_queue {
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
        for cid in city_ids {
            let Some((granary, turns)) = self.bound_granary_item(g, pid, cid, plan) else {
                continue;
            };
            let head = g.cities[&cid].queue.first().cloned();
            if head.as_ref() == Some(&granary) {
                continue;
            }
            match &head {
                Some(item) => {
                    if !Self::bound_granary_may_defer(g, pid, cid, item, at_major_war) {
                        continue;
                    }
                }
                // An idle bound city with a Settler due leaves its queue to
                // the Settler step: the Settler relieves the housing too, and
                // the city count is what the gate reads.
                None => {
                    if self.base.settler_due(g, pid, cid, n_cities, settlers) {
                        continue;
                    }
                }
            }
            let (name, pop, housing) = {
                let city = &g.cities[&cid];
                (city.name.clone(), city.pop, g.city_housing(city))
            };
            if g.apply(pid, &Action::Produce { city: cid, item: granary }).is_ok() {
                think!(self.journal(), Cities, Decision,
                       "{name} claims its Granary at its housing";
                       "granary-claims-the-bound-queue: population {pop} against housing \
                        {housing:.0}, {turns:.1} turns, ahead of {}",
                       head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                           "{} (it keeps its progress)", Self::plain_item(&item))));
            }
        }
    }
}

#[cfg(test)]
mod tests;
