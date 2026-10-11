//! `campus-building-claims-the-queue`: a city whose standing Campus can build
//! its next building -- the Library, else the University, else the Research
//! Lab, the pick of `campus-buildings-first` (`BasicAi::campus_building_item`,
//! done within 20 turns) -- puts that building at the head of its queue ahead
//! of the delegated governor's rescoring, over an idle queue or a deferrable
//! head. One claim a turn, a Library before a University. A Settler, a Builder
//! or Trader, Walls, a wonder, a project, the faith defence's Holy Site,
//! Shrine or Temple, the science and culture items (Campus, Theater Square,
//! Government Plaza, Monument, Library, University, Amphitheater), a claimed
//! Granary, Aqueduct, Industrial Zone, deterrent, Trader or backlog Builder,
//! the early Settler floor's city, a threatened or freshly attacked city, a
//! city due its Granary, and anything finishing this turn keep their queue; at
//! war with a major a military unit keeps its queue too. The displaced item
//! keeps its progress.
//!
//! **Why (2026-10-11 census, 63 live Immortal and 105 Emperor GC runs since
//! capital-campus-before-the-plaza).** The Campus districts come (Immortal
//! medians 1 / 2 / 4 at t75 / t100 / t125) but their buildings lag: 1.5
//! Libraries and 0 Universities at t100, 3 and 2 at t125. Splitting the
//! Immortal runs by Science per citizen at t125, the top third held 5
//! Campuses, 4.5 Libraries and 3 Universities to the bottom third's 3, 1 and
//! 0 (1.85 against 0.98 Science per citizen); Natural Philosophy or
//! Rationalism was slotted on 95% of the turns we held two Campuses, so the
//! policy slot is not the gap. Over t50-150 a median game spent 45 city-turns
//! with a standing Campus and no Library or Library in production, and 47 with
//! a University buildable but not in production (Emperor 39 and 63); those
//! turns held an idle first frame 26-30%, a military unit 25%, a Builder
//! 9-13%. `campus-buildings-first` already names the building, but only from
//! the delegated governor's `pick_item`, which reads the idle queue the
//! controller's claims and the scorer take first: 204 of its journal lines
//! across 63 Immortal games -- the same shape as the Industrial Zone before
//! `industrial-zone-claims-the-producer-queue`.
//!
//! The claim runs after the Industrial Zone claim and before the Prophet race
//! and the routine claims; the deterrent and the governor's rescoring then
//! leave the claimed building alone ([`AdvancedAi::campus_building_claim_holds`]).
//! Off, nothing runs.

use super::*;

/// The claim's order among Campus buildings: the Library first (it unlocks the
/// University and costs the least), then the University, then the Research
/// Lab.
fn campus_building_rank(g: &Game, item: &Item) -> Option<u8> {
    let Item::Building { building } = item else {
        return None;
    };
    let family = g
        .rules
        .buildings
        .get(building)
        .and_then(|spec| spec.replaces.clone())
        .unwrap_or_else(|| building.clone());
    match family.as_str() {
        "library" => Some(0),
        "university" => Some(1),
        "research_lab" => Some(2),
        _ => None,
    }
}

impl AdvancedAi {
    /// The Campus building `cid` would claim, its rank and how long it takes:
    /// `campus-buildings-first`'s pick for an unthreatened city of ours not
    /// due its Granary. `None` with the gene off.
    pub(super) fn campus_building_claim_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, u8, f64)> {
        if !self.campus_building_claims_the_queue {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
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
        let item = BasicAi::campus_building_item(g, pid, cid)?;
        let rank = campus_building_rank(g, &item)?;
        if !g.can_produce(pid, cid, &item) {
            return None;
        }
        let production = g.city_yields(cid).production;
        let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
            let remaining =
                g.item_cost_for_city(pid, cid, &item) - g.item_invested_production(cid, &item);
            remaining.max(0.0) / production.max(0.5)
        });
        Some((item, rank, turns))
    }

    /// Whether `item` at the head of `cid` is the Campus building this gene
    /// claims and the city still wants it: the deterrent and the delegated
    /// governor's rescoring leave it alone. Always false with the gene off.
    pub(super) fn campus_building_claim_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        campus_building_rank(g, item).is_some()
            && self
                .campus_building_claim_item(g, pid, cid, plan)
                .is_some_and(|(building, _, _)| building == *item)
    }

    /// See the module: the city whose Campus can build its next building puts
    /// it at the head of its queue, idle or over a deferrable item. One claim
    /// a turn. Exact no-op with the gene off.
    pub(super) fn claim_campus_building(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.campus_building_claims_the_queue {
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
        let mut best: Option<(u8, f64, u32, Item)> = None;
        for cid in city_ids {
            let Some((building, rank, turns)) = self.campus_building_claim_item(g, pid, cid, plan)
            else {
                continue;
            };
            if self.early_settler_floor_holds(g, pid, cid, plan) {
                continue;
            }
            match g.cities[&cid].queue.first() {
                Some(item) => {
                    if *item == building
                        || self.deterrence_unit_holds(g, pid, item)
                        || self.bound_granary_holds(g, pid, cid, plan, item)
                        || self.idle_route_trader_holds(g, pid, cid, plan, item)
                        || self.backlog_builder_holds(g, pid, cid, plan, item)
                        || self.capped_aqueduct_holds(g, pid, cid, plan, item)
                        || self.producer_zone_claim_holds(g, pid, cid, plan, item)
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
            // The Library first; then the quickest; then the lowest id, so
            // the claim is deterministic.
            if best
                .as_ref()
                .is_none_or(|(old_rank, old_turns, old_city, _)| {
                    (rank, turns, cid) < (*old_rank, *old_turns, *old_city)
                })
            {
                best = Some((rank, turns, cid, building));
            }
        }
        let Some((_, turns, cid, building)) = best else {
            return;
        };
        let head = g.cities[&cid].queue.first().cloned();
        let name = g.cities[&cid].name.clone();
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: building.clone(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Cities, Decision,
                   "{name} claims its Campus's {}", Self::plain_item(&building);
                   "campus-building-claims-the-queue: {turns:.1} turns, ahead of {}",
                   head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                       "{} (it keeps its progress)", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
