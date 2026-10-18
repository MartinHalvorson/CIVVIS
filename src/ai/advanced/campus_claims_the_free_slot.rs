//! `campus-claims-the-free-slot`: a city of three or more citizens with a
//! free district slot, no Campus standing or queued, and a Campus it can
//! finish within `BasicAi::FIRST_CAMPUS_MAX_TURNS` (the pick of
//! `BasicAi::first_campus_item`) puts that Campus at the head of its queue,
//! over an idle queue or a deferrable head
//! ([`AdvancedAi::capped_aqueduct_may_defer`]: a military unit while no major
//! war is open, a Scout, an ordinary building, or a district with nothing
//! invested). One claim a turn, the quickest Campus first. A Settler, a
//! Builder or Trader, Walls, a wonder, a project, the faith defence's Holy
//! Site, Shrine or Temple, the science and culture items, every earlier
//! claim's item, the early Settler floor's city, the Prophet race's site city
//! while the race is open without a site, a threatened or freshly attacked
//! city, and anything finishing this turn keep their queue. The displaced item
//! keeps its progress, and the delegated governor's rescoring leaves the
//! claimed Campus alone ([`AdvancedAi::free_slot_campus_holds`]).
//!
//! **Why (2026-10-18 census, 164 live Emperor GC runs of 10-16/17).** Our
//! Science per citizen was 0.78 / 0.81 / 0.93 at t50 / t60 / t75 against the
//! rivals' 1.73 / 1.84 / 1.93, and Science at t100 43.8 against 102.7; the
//! citizen count and the +16% handicap explain about 1.6 times of that, the
//! rest is districts. Campuses per city stood at 0.06 / 0.15 / 0.22 / 0.30 /
//! 0.49 at t40 / t50 / t60 / t75 / t100, behind the Holy Sites until t100,
//! while slots were not the limit: 1.57 available against 0.59 used per city
//! at t50, and 69% of our cities at t50 (62% at t60) held a free slot and no
//! Campus. On those city-turns after Writing the Campus was not among the top
//! 22 items built: Granary 14%, Settler 13%, Builder 12%, Archer 9%, Walls 9%.
//! By Campuses at t60 (0 / 1 / 2+, n = 38 / 61 / 40) the games reached 22.0 /
//! 24.6 / 26.6 techs and 31 / 45 / 57 Science at t100 and an r150 of 0.30 /
//! 0.45 / 0.62, and the gap held within each city count at t60. The only
//! Campus a non-capital city got came from `campus-before-the-army-2`, the
//! delegated governor's step, which reads the idle queue the claims and the
//! routine military floor take first.
//!
//! The claim runs after `campus-building-claims-the-queue` and before the
//! Prophet race's site and the routine claims. Off, nothing runs.
//!
//! **Version 2** (`campus-claims-the-free-slot-2`) claims from
//! [`FREE_SLOT_CAMPUS_MIN_POP_2`] citizens and in every eligible city each
//! turn rather than one. Live, version one's claims began only around turn
//! 80: across its first five Emperor games 23 of the t45-65 city samples with
//! a free slot and a buildable Campus were held back by the three-citizen
//! floor alone, and the one-a-turn pace left others on a routine unit.

use super::*;

/// The fewest citizens a city holds before its free slot goes to a Campus:
/// a smaller city's slot is its only one and its Campus its slowest.
pub(super) const FREE_SLOT_CAMPUS_MIN_POP: i32 = 3;

/// Version 2's floor: a two-citizen city's first slot goes to its Campus.
pub(super) const FREE_SLOT_CAMPUS_MIN_POP_2: i32 = 2;

impl AdvancedAi {
    /// The Campus `cid` would claim and how long it takes: the first Campus
    /// (`BasicAi::first_campus_item`) of an unthreatened city of ours with
    /// [`FREE_SLOT_CAMPUS_MIN_POP`] citizens, none standing or queued, and
    /// not the Prophet race's site city. `None` with the gene off.
    pub(super) fn free_slot_campus_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64)> {
        if !self.campus_claims_the_free_slot && !self.campus_claims_the_free_slot_2 {
            return None;
        }
        let min_pop = if self.campus_claims_the_free_slot_2 {
            FREE_SLOT_CAMPUS_MIN_POP_2
        } else {
            FREE_SLOT_CAMPUS_MIN_POP
        };
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || city.pop < min_pop
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
        {
            return None;
        }
        let queued = city.queue.iter().any(|item| {
            matches!(item, Item::District { district, .. } if g.district_family(*district) == "campus")
        });
        if queued {
            return None;
        }
        // The Prophet race's Holy Site takes this city's slot first.
        if self
            .base
            .race_site_city(g, pid)
            .is_some_and(|(site_city, _)| site_city == cid)
        {
            return None;
        }
        let item = BasicAi::first_campus_item(g, pid, cid)?;
        let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
            g.item_cost_for(pid, &item) / g.city_yields(cid).production.max(0.5)
        });
        Some((item, turns))
    }

    /// Whether `item` at the head of `cid` is a first Campus this gene would
    /// claim there: the delegated governor's rescoring leaves it alone.
    /// Always false with the gene off.
    pub(super) fn free_slot_campus_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        if !self.campus_claims_the_free_slot && !self.campus_claims_the_free_slot_2 {
            return false;
        }
        let Item::District { district, .. } = item else {
            return false;
        };
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        g.district_family(*district) == "campus"
            && city.owner == pid
            && plan.threatened_city != Some(cid)
            && !g.city_has_district_family(city, crate::name!("campus"))
            && g.can_produce(pid, cid, item)
    }

    /// See the module: the city that finishes its first Campus soonest puts
    /// it at the head of its queue, idle or over a deferrable item. One claim
    /// a turn. Exact no-op with the gene off.
    pub(super) fn claim_free_slot_campus(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.campus_claims_the_free_slot && !self.campus_claims_the_free_slot_2 {
            return;
        }
        // Version 2: every eligible city, each the quickest of those left.
        let claims = if self.campus_claims_the_free_slot_2 {
            g.player_city_ids(pid).len()
        } else {
            1
        };
        for _ in 0..claims {
            if !self.claim_one_free_slot_campus(g, pid, plan) {
                return;
            }
        }
    }

    /// One claim: the city that finishes its first Campus soonest. True when
    /// a Campus was put at the head of a queue.
    fn claim_one_free_slot_campus(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) -> bool {
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
            let Some((campus, turns)) = self.free_slot_campus_item(g, pid, cid, plan) else {
                continue;
            };
            if self.early_settler_floor_holds(g, pid, cid, plan) {
                continue;
            }
            match g.cities[&cid].queue.first() {
                Some(item) => {
                    if self.deterrence_unit_holds(g, pid, item)
                        || self.bound_granary_holds(g, pid, cid, plan, item)
                        || self.idle_route_trader_holds(g, pid, cid, plan, item)
                        || self.backlog_builder_holds(g, pid, cid, plan, item)
                        || self.capped_aqueduct_holds(g, pid, cid, plan, item)
                        || self.capped_neighborhood_holds(g, pid, cid, plan, item)
                        || self.producer_zone_claim_holds(g, pid, cid, plan, item)
                        || self.factory_claim_holds(g, pid, cid, plan, item)
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
            // The quickest Campus first; then the lowest id, so the claim is
            // deterministic.
            if best
                .as_ref()
                .is_none_or(|(old_turns, old_city, _)| (turns, cid) < (*old_turns, *old_city))
            {
                best = Some((turns, cid, campus));
            }
        }
        let Some((turns, cid, campus)) = best else {
            return false;
        };
        let head = g.cities[&cid].queue.first().cloned();
        let name = g.cities[&cid].name.clone();
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: campus.clone(),
            },
        )
        .is_err()
        {
            return false;
        }
        think!(self.journal(), Cities, Decision,
               "{name} claims its free district slot for a Campus";
               "campus-claims-the-free-slot: {turns:.1} turns, ahead of {}",
               head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                   "{} (it keeps its progress)", Self::plain_item(&item))));
        true
    }
}

#[cfg(test)]
mod tests;
