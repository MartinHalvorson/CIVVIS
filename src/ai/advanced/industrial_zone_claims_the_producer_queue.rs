//! `industrial-zone-claims-the-producer-queue`: once Apprenticeship is known,
//! the most productive city that `industrial-zone-in-the-producers` names for
//! an Industrial Zone (or for the Workshop of the zone it holds) puts that
//! build at the head of its queue ahead of the delegated governor's
//! rescoring, over an idle queue or a deferrable head. One claim a turn. A
//! Settler, a Builder or Trader, Walls, a wonder, a project, the faith
//! defence's Holy Site, Shrine or Temple, the science and culture items
//! (Campus, Theater Square, Government Plaza, Monument, Library, University,
//! Amphitheater), a claimed Granary, Aqueduct, deterrent, Trader or backlog
//! Builder, the early Settler floor's city, a threatened or freshly attacked
//! city, a city due its Granary, and anything finishing this turn keep their
//! queue; at war with a major a military unit keeps its queue too. The
//! displaced item keeps its progress.
//!
//! **Why (2026-10-11 census, 38 live Immortal and 120 Emperor GC runs).** We
//! learn Apprenticeship at a median t82 at Immortal (first rival t48), but the
//! first Industrial Zone is founded a median 22 turns later (t104; Emperor 27
//! turns, t106) and stands at t112: a median 0 zones at t100, 1 at t125 and
//! 1.5 at t150 (Emperor 0 / 1 / 2). Over the turns between the technology and
//! the first zone, 58% of our producer cities' turns (two or more worked tiles
//! of 2+ Production) had a legal zone in the host's buildable list, and those
//! turns went to a military unit 39%, an ordinary building 21%, another
//! district 19% and an idle frame 20%. `industrial-zone-in-the-producers`
//! already names the zone city, but it asks the delegated governor's
//! `pick_item`, the strategic governor's idle queue and the Theater
//! reservation -- all of which read an idle queue the controller's claims and
//! the scorer take first -- so the census logged 142 of its journal lines
//! across 38 Immortal games while the first zone still waited 22 turns. A zone
//! city with its Workshop gained a median 2.8 (mean 4.0) Production over the
//! empire's zone-less cities in the same turns (the earlier 42-game census in
//! `ai/industrial_zones.rs`), and the top third of Emperor games by
//! production held 3 zones at t150 to the bottom third's 0.
//!
//! The claim reuses the producer step's own choice
//! (`BasicAi::producer_workshop_item`, then `BasicAi::producer_zone_item`:
//! among the most productive unthreatened cities with a free specialty slot,
//! not due their first Campus, on the best Production-adjacency site, done
//! within 20 turns). It runs after the Aqueduct claim and before the Prophet
//! race and the routine claims; the deterrent and the governor's rescoring
//! then leave the claimed build alone
//! ([`AdvancedAi::producer_zone_claim_holds`]). Off, nothing runs.

use super::*;

/// The fewest cities from which the claim opens a zone: the producer step's
/// own floor.
const PRODUCER_CLAIM_MIN_CITIES: usize = 3;

impl AdvancedAi {
    /// The Industrial Zone or Workshop `cid` would claim and how long it
    /// takes: the producer step's pick for an unthreatened city not due its
    /// Granary. `None` with the gene off.
    pub(super) fn producer_zone_claim_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64)> {
        if !self.industrial_zone_claims_the_producer_queue {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
        {
            return None;
        }
        let n_cities = g.player_city_ids(pid).len();
        if n_cities < PRODUCER_CLAIM_MIN_CITIES {
            return None;
        }
        // A city within one of its housing with a Granary still to build
        // leaves its queue to the Granary claim.
        if (city.pop as f64) + 1.0 >= g.city_housing(city)
            && BasicAi::civ_building(g, pid, cid, "granary").is_some()
        {
            return None;
        }
        let threatened = plan.threatened_city;
        let item = BasicAi::producer_workshop_item(g, pid, cid, n_cities, threatened)
            .or_else(|| BasicAi::producer_zone_item(g, pid, cid, n_cities, threatened))?;
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

    /// Whether `item` at the head of `cid` is an Industrial Zone or Workshop
    /// this gene would keep: still legal, in an unthreatened city of ours.
    /// The producer step stops naming a city once its zone is queued, so the
    /// hold reads the queued build itself; the deterrent and the delegated
    /// governor's rescoring leave it alone. Always false with the gene off.
    pub(super) fn producer_zone_claim_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        if !self.industrial_zone_claims_the_producer_queue {
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
        let is_zone_build = match item {
            Item::District { district, .. } => g.district_family(*district) == "industrial_zone",
            Item::Building { building } => {
                *building == "workshop"
                    || g.rules
                        .buildings
                        .get(building)
                        .is_some_and(|spec| spec.replaces == Some(crate::name!("workshop")))
            }
            _ => false,
        };
        is_zone_build && g.can_produce(pid, cid, item)
    }

    /// See the module: the most productive city the producer step names puts
    /// its Industrial Zone (or Workshop) at the head of its queue, idle or
    /// over a deferrable item. One claim a turn. Exact no-op with the gene
    /// off.
    pub(super) fn claim_producer_zone(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.industrial_zone_claims_the_producer_queue {
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
            let Some((zone, turns)) = self.producer_zone_claim_item(g, pid, cid, plan) else {
                continue;
            };
            if self.early_settler_floor_holds(g, pid, cid, plan) {
                continue;
            }
            match g.cities[&cid].queue.first() {
                Some(item) => {
                    if *item == zone
                        || self.deterrence_unit_holds(g, pid, item)
                        || self.bound_granary_holds(g, pid, cid, plan, item)
                        || self.idle_route_trader_holds(g, pid, cid, plan, item)
                        || self.backlog_builder_holds(g, pid, cid, plan, item)
                        || self.capped_aqueduct_holds(g, pid, cid, plan, item)
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
                best = Some((production, turns, cid, zone));
            }
        }
        let Some((production, turns, cid, zone)) = best else {
            return;
        };
        let head = g.cities[&cid].queue.first().cloned();
        let name = g.cities[&cid].name.clone();
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: zone.clone(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Cities, Decision,
                   "{name} claims its {} for its industry", Self::plain_item(&zone);
                   "industrial-zone-claims-the-producer-queue: {production:.0} Production, \
                    {turns:.1} turns, ahead of {}",
                   head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                       "{} (it keeps its progress)", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
