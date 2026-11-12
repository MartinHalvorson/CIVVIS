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
//! city due its Granary, a wall breaker the siege reservation queued (any
//! siege unit; [`AdvancedAi::queued_wall_breaker`]), and anything finishing
//! this turn keep their queue; at war with a major a military unit keeps its
//! queue too. The displaced item keeps its progress.
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
//!
//! `campus-and-library-before-the-claims` opens this claim for a Domination
//! seat's Library and quick University even with this gene off, lets them
//! pass the claims' heads (from four cities the Library passes the floor's
//! Settler too), and claims in every such city each turn. See
//! `advanced/campus_and_library_before_the_claims.rs`.

use super::campus_and_library_before_the_claims::ScienceFirstItem;
use super::*;

/// The claim's order among Campus buildings: the Library first (it unlocks the
/// University and costs the least), then the University, then the Research
/// Lab.
pub(super) fn campus_building_rank(g: &Game, item: &Item) -> Option<u8> {
    let Item::Building { building } = item else {
        return None;
    };
    let family = g
        .rules
        .buildings
        .get(building)
        .and_then(|spec| spec.replaces)
        .unwrap_or(*building);
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
        if self.culture_releases_science_claims(g, pid) {
            return None;
        }
        if !self.campus_building_claims_the_queue && !self.campus_and_library_before_the_claims {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
        {
            return None;
        }
        // `campus-and-library-before-the-claims`: whether the gene opens this
        // city. `None` with the gene off.
        let reach = self.campus_and_library_reach(g, pid, cid, plan);
        // A city within one of its housing with a Granary still to build
        // leaves its queue to the Granary claim.
        let granary_first = (city.pop as f64) + 1.0 >= g.city_housing(city)
            && BasicAi::civ_building(g, pid, cid, "granary").is_some();
        if granary_first && reach.is_none() {
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
        // The gene's Library or quick University goes ahead of the Granary
        // too; anything else keeps this claim's own rules.
        let science_first = reach.is_some() && Self::campus_and_library_building(g, rank, turns);
        if !science_first && (granary_first || !self.campus_building_claims_the_queue) {
            return None;
        }
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

    /// Select the original claim without changing queues. `exclude` supports
    /// the parallel-job forecast; `None` preserves the original ranking.
    pub(super) fn campus_building_claim_candidate(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        exclude: Option<u32>,
    ) -> Option<(u8, f64, u32, Item)> {
        self.campus_building_claim_pick(g, pid, plan, exclude, false, false)
            .map(|(rank, turns, cid, item, _)| (rank, turns, cid, item))
    }

    /// The claim's pick, and the head it passes when
    /// `campus-and-library-before-the-claims` let it pass one the claim's own
    /// rules hold. `science_first_only` keeps to the gene's Library and quick
    /// University (its extra claims each turn). `journal_holds` writes the
    /// gene's housing-head line, once a turn.
    fn campus_building_claim_pick(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        exclude: Option<u32>,
        science_first_only: bool,
        journal_holds: bool,
    ) -> Option<(u8, f64, u32, Item, Option<String>)> {
        if !self.campus_building_claims_the_queue && !self.campus_and_library_before_the_claims {
            return None;
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
        let mut best: Option<(u8, f64, u32, Item, Option<String>)> = None;
        for cid in city_ids {
            // `rear-cities-grow-through-the-war-hold`: under the war's hold,
            // only a rear city. Always open while the gene is off.
            if !self.claim_open_behind_the_front(cid) {
                continue;
            }
            if exclude == Some(cid) {
                continue;
            }
            let Some((building, rank, turns)) = self.campus_building_claim_item(g, pid, cid, plan)
            else {
                continue;
            };
            // `campus-and-library-before-the-claims`: how far the gene reaches
            // into this city's queue for its Library or quick University.
            // `None` with the gene off.
            let reach = self
                .campus_and_library_reach(g, pid, cid, plan)
                .filter(|_| Self::campus_and_library_building(g, rank, turns));
            if science_first_only && reach.is_none() {
                continue;
            }
            // `campus-and-library-before-the-claims`: what the gene puts
            // first here.
            let what = if rank == 0 {
                ScienceFirstItem::Library
            } else {
                ScienceFirstItem::University
            };
            // From four cities the Library passes the floor's Settler while
            // at most half of it is paid; the head test below passes it too.
            if self.early_settler_floor_holds(g, pid, cid, plan)
                && !reach.is_some_and(|reach| {
                    g.cities[&cid].queue.first().is_some_and(|item| {
                        self.campus_and_library_passes(g, pid, cid, plan, item, reach, what)
                            .is_some()
                    })
                })
            {
                continue;
            }
            let mut passed = None;
            match g.cities[&cid].queue.first() {
                Some(item) => {
                    if *item == building {
                        continue;
                    }
                    if self.deterrence_unit_holds(g, pid, item)
                        || self.bound_granary_holds(g, pid, cid, plan, item)
                        || self.idle_route_trader_holds(g, pid, cid, plan, item)
                        || self.charting_hull_holds(g, pid, cid, plan, item)
                        || self.backlog_builder_holds(g, pid, cid, plan, item)
                        || self.capped_aqueduct_holds(g, pid, cid, plan, item)
                        || self.capped_neighborhood_holds(g, pid, cid, plan, item)
                        || self.producer_zone_claim_holds(g, pid, cid, plan, item)
                        || self.factory_claim_holds(g, pid, cid, plan, item)
                        || Self::queued_wall_breaker(g, item)
                        || !Self::capped_aqueduct_may_defer(g, pid, cid, item, at_major_war)
                    {
                        passed = reach.and_then(|reach| {
                            // A housing-bound city's Granary and a capped
                            // city's Aqueduct keep the head.
                            if let Some(kept) = self.campus_and_library_housing_keeps(
                                g, pid, cid, plan, item, what, &building,
                            ) {
                                if journal_holds {
                                    self.explain_housing_keeps_the_head(g, cid, kept, what);
                                }
                                return None;
                            }
                            self.campus_and_library_passes(g, pid, cid, plan, item, reach, what)
                        });
                        if passed.is_none() {
                            continue;
                        }
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
                .is_none_or(|(old_rank, old_turns, old_city, _, _)| {
                    (rank, turns, cid) < (*old_rank, *old_turns, *old_city)
                })
            {
                best = Some((rank, turns, cid, building, passed));
            }
        }
        best
    }

    /// See the module. One claim a turn; under
    /// `campus-and-library-before-the-claims` every city's Library or quick
    /// University besides. Exact no-op with both genes off.
    pub(super) fn claim_campus_building(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let rounds = if self.campus_and_library_before_the_claims {
            g.player_city_ids(pid).len().max(1)
        } else {
            1
        };
        for round in 0..rounds {
            // This gene's own claim is the first; the rest are the other gene's.
            let science_first_only = round > 0 || !self.campus_building_claims_the_queue;
            if !self.claim_one_campus_building(g, pid, plan, science_first_only, round == 0) {
                return;
            }
        }
    }

    /// One claim. True when a Campus building was put at the head of a
    /// queue.
    fn claim_one_campus_building(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        science_first_only: bool,
        journal_holds: bool,
    ) -> bool {
        let best =
            self.campus_building_claim_pick(g, pid, plan, None, science_first_only, journal_holds);
        let Some((_, turns, cid, building, passed)) = best else {
            return false;
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
        .is_err()
        {
            return false;
        }
        let head_text = head.map_or_else(
            || "an idle queue".to_string(),
            |item| format!("{} (it keeps its progress)", Self::plain_item(&item)),
        );
        if passed.is_none() && !science_first_only {
            think!(self.journal(), Cities, Decision,
                   "{name} claims its Campus's {}", Self::plain_item(&building);
                   "campus-building-claims-the-queue: {turns:.1} turns, ahead of {head_text}");
        } else {
            let passed = passed.map_or(head_text, |passed| {
                format!("{passed} (it keeps its progress)")
            });
            self.explain_campus_building_passes(g, cid, &building, turns, &passed);
        }
        true
    }
}

#[cfg(test)]
mod tests;
