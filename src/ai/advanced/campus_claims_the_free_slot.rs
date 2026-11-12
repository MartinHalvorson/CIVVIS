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
//! Korea pursuing Science can claim its first slot at one citizen when the
//! host quotes a Seowon within the existing Campus budget. Native Jinju at
//! 43/0 offered an eight-turn Seowon but used its only slot for a Plaza.
//! The ordinary picker still checks the actual site, slot and safety holds.
//! With version 2, a peaceful Korean Science city may also put a native-quoted
//! first Seowon of at most five turns before near-rival parity recruitment.
//! The first expansion, other earlier claims and active-war military queues
//! keep their existing priority. A committed first Seowon survives a parity
//! replan with zero paid progress; its remaining quote may change on placement.
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
//!
//! Native Korea pursuing Science may also take at most three host-quoted
//! turns for its first Seowon before a longer repeated Settler, during the
//! early floor, with at least two cities and another Settler walking. War,
//! rush and threatened plans retain the Settler. The six-city floor remains;
//! the brief Campus commitment is protected through zero-progress replans.

use super::*;

/// The fewest citizens a city holds before its free slot goes to a Campus:
/// a smaller city's slot is its only one and its Campus its slowest.
pub(super) const FREE_SLOT_CAMPUS_MIN_POP: i32 = 3;

/// Version 2's floor: a two-citizen city's first slot goes to its Campus.
pub(super) const FREE_SLOT_CAMPUS_MIN_POP_2: i32 = 2;

impl AdvancedAi {
    /// A peaceful Korean Science city can briefly invest in its first Seowon
    /// before chasing a neighbour's military parity. Immediate defence and
    /// the earlier expansion claims still own their queues.
    fn korean_science_parity_pause_safe(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        self.campus_claims_the_free_slot_2
            && g.players[pid].civ == "Korea"
            && g.victory_conditions.science
            && self.active_victory_target(g) == Some(VictoryTarget::Science)
            && g.player_city_ids(pid).len() >= 2
            && matches!(
                plan.strategy,
                GrandStrategy::Science | GrandStrategy::Expansion
            )
            && !plan.rush
            && plan.threatened_city.is_none()
            && g.cities.get(&cid).is_some_and(|city| {
                city.owner == pid
                    && (city.last_attacked == 0 || g.turn.saturating_sub(city.last_attacked) > 4)
            })
            && !g.players.iter().any(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && !other.is_free_city
                    && g.is_at_war(pid, other.id)
            })
    }

    /// Native Jeonju offered a five-turn first Seowon while the parity
    /// claim held its Warrior. No model estimate grants this exception.
    fn short_seowon_precedes_parity(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        campus: &Item,
    ) -> bool {
        self.korean_science_parity_pause_safe(g, pid, cid, plan)
            && g.host_production_turns(cid, campus)
                .is_some_and(|turns| turns.is_finite() && turns > 0.0 && turns <= 5.0)
    }

    /// Preserve a committed first Campus through the zero-progress native
    /// replan, even if losing its worked plot changes the remaining quote.
    pub(super) fn first_seowon_holds_parity(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        self.korean_science_parity_pause_safe(g, pid, cid, plan)
            && g.cities[&cid]
                .queue
                .first()
                .is_some_and(|item| self.free_slot_campus_holds(g, pid, cid, plan, item))
    }

    /// A brief native Seowon build, priced by the host rather than an estimate.
    fn short_korean_science_campus(&self, g: &Game, pid: usize, cid: u32, item: &Item) -> bool {
        self.campus_claims_the_free_slot_2
            && g.players[pid].civ == "Korea"
            && g.victory_conditions.science
            && self.active_victory_target(g) == Some(VictoryTarget::Science)
            && matches!(item, Item::District { district, .. }
                if g.district_family(*district) == "campus")
            && g.host_production_turns(cid, item)
                .is_some_and(|turns| turns.is_finite() && turns > 0.0 && turns <= 3.0)
    }

    /// Keep the brief Campus commitment through a fresh native replan. Its
    /// foundation can already exist while it has no invested production yet.
    pub(super) fn short_seowon_holds_settler_floor(&self, g: &Game, pid: usize, cid: u32) -> bool {
        g.cities.get(&cid).is_some_and(|city| {
            city.owner == pid
                && city
                    .queue
                    .first()
                    .is_some_and(|item| self.short_korean_science_campus(g, pid, cid, item))
        })
    }

    /// A peaceful Culture seat funds two science foundations, then lets
    /// ordinary production price further Campuses against its tourism economy.
    fn culture_limits_campus_foundations(&self, g: &Game, pid: usize) -> bool {
        g.victory_conditions.culture
            && self.active_victory_target(g) == Some(VictoryTarget::Culture)
            && (self.campus_claims_the_free_slot || self.campus_claims_the_free_slot_2)
            && !g.players.iter().any(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && g.is_at_war(pid, other.id)
            })
    }

    fn culture_other_campuses_cover_foundation(&self, g: &Game, pid: usize, cid: u32) -> bool {
        self.culture_limits_campus_foundations(g, pid)
            && g.player_city_ids(pid)
                .into_iter()
                .filter(|other| *other != cid)
                .filter(|other| {
                    let city = &g.cities[other];
                    g.city_has_district_family(city, crate::name!("campus"))
                        || city.queue.iter().any(|item| {
                            matches!(item, Item::District { district, .. }
                                if g.district_family(*district) == "campus")
                        })
                        || city.owned_tiles.iter().any(|pos| {
                            g.map
                                .get(*pos)
                                .and_then(|tile| tile.district_foundation.as_ref())
                                .is_some_and(|foundation| {
                                    g.district_family(foundation.district) == "campus"
                                })
                        })
                })
                .take(2)
                .count()
                >= 2
    }

    /// The AdvancedAi claim owns the first two foundations. Do not let the
    /// delegated governor force a Campus again through a different opening.
    /// Building claims and ordinary scoring remain available.
    pub(super) fn culture_budgeted_base_cities(&mut self, g: &mut Game, pid: usize) {
        let saved_resorts = self.base.preserve_culture_resort_sites;
        self.base.preserve_culture_resort_sites = self.culture_releases_science_claims(g, pid);
        if !self.culture_limits_campus_foundations(g, pid) {
            self.base.cities(g, pid);
            self.base.preserve_culture_resort_sites = saved_resorts;
            return;
        }
        let saved = (
            self.base.capital_campus_first,
            self.base.campus_before_harbor,
            self.base.campus_before_the_army,
            self.base.campus_before_the_army_2,
            self.base.campus_before_the_army_3,
        );
        self.base.capital_campus_first = false;
        self.base.campus_before_harbor = false;
        self.base.campus_before_the_army = false;
        self.base.campus_before_the_army_2 = false;
        self.base.campus_before_the_army_3 = false;
        self.base.cities(g, pid);
        (
            self.base.capital_campus_first,
            self.base.campus_before_harbor,
            self.base.campus_before_the_army,
            self.base.campus_before_the_army_2,
            self.base.campus_before_the_army_3,
        ) = saved;
        self.base.preserve_culture_resort_sites = saved_resorts;
    }

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
        if self.culture_releases_science_claims(g, pid)
            || self.culture_other_campuses_cover_foundation(g, pid, cid)
        {
            return None;
        }
        if !self.campus_claims_the_free_slot
            && !self.campus_claims_the_free_slot_2
            && !self.science_campus_pause_safe(g, pid, cid, plan)
        {
            return None;
        }
        let city = g.cities.get(&cid)?;
        let one_citizen_seowon = self.campus_claims_the_free_slot_2
            && city.pop == 1
            && g.players[pid].civ == "Korea"
            && g.victory_conditions.science
            && self.active_victory_target(g) == Some(VictoryTarget::Science)
            // Native menu entries are keyed by item type, not placement. The picker
            // below still chooses and validates a real legal Seowon site.
            && g.host_production_turns(cid, &Item::District {
                district: crate::name!("seowon"),
                pos: city.pos,
            }).is_some_and(|turns| turns.is_finite() && turns > 0.0
                && turns <= self.free_slot_campus_turn_budget(g));
        let min_pop = if one_citizen_seowon {
            1
        } else if self.campus_claims_the_free_slot_2 {
            FREE_SLOT_CAMPUS_MIN_POP_2
        } else {
            FREE_SLOT_CAMPUS_MIN_POP
        };
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
        // `culture-seat-theater-first`: a Culture seat's Theater Square takes
        // this city's slot first.
        if self.culture_theater_item(g, pid, cid, plan).is_some() {
            return None;
        }
        if self
            .culture_faith_source_target(g, pid, plan)
            .is_some_and(|(source, _, _)| source == cid)
        {
            return None;
        }
        let item = BasicAi::first_campus_item_with_budget(
            g,
            pid,
            cid,
            self.free_slot_campus_turn_budget(g),
        )?;
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
        // The Culture budget limits new reservations. Existing commitments,
        // including ones admitted by ordinary scoring or during a war, keep
        // their protection rather than all losing it when a third appears.
        if self.culture_releases_science_claims(g, pid) {
            return false;
        }
        if !self.campus_claims_the_free_slot
            && !self.campus_claims_the_free_slot_2
            && !(self.science_campus_pause_safe(g, pid, cid, plan)
                && g.host_production_turns(cid, item)
                    .is_some_and(|turns| turns.is_finite() && turns > 0.0))
        {
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
        if !self.campus_claims_the_free_slot
            && !self.campus_claims_the_free_slot_2
            && !self.science_campus_before_repeat_settler
        {
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
            // `rear-cities-grow-through-the-war-hold`: under the war hold only
            // the cities away from the front may claim. Always true otherwise.
            if !self.claim_open_behind_the_front(cid) {
                continue;
            }
            let Some((campus, turns)) = self.free_slot_campus_item(g, pid, cid, plan) else {
                continue;
            };
            let floor_holds = self.early_settler_floor_holds(g, pid, cid, plan);
            // Native Deity Korea kept a three-turn Seowon behind a long
            // Settler at t46-49 while another Settler was already walking.
            // Let this first Campus briefly precede the repeated Settler;
            // keep the first expansion, emergencies and finishing queues.
            let campus_before_settler = (floor_holds
                && self.early_settler_floor(g, plan) > 0
                && n_cities >= 2
                && Self::early_settler_census(g, pid).0 > 0
                && !at_major_war
                && plan.threatened_city.is_none()
                && !plan.rush
                && matches!(
                    plan.strategy,
                    GrandStrategy::Expansion | GrandStrategy::Science
                )
                && self.short_korean_science_campus(g, pid, cid, &campus)
                && matches!(g.cities[&cid].queue.first(), Some(Item::Unit { unit }) if *unit == "settler")
                && g.host_production_turns(cid, &Self::early_settler_item())
                    .is_some_and(|remaining| remaining.is_finite() && remaining > turns))
                || (floor_holds
                    && self.science_campus_precedes_repeat_settler(g, pid, cid, plan, &campus));
            // The standalone Science gene supplies only its narrow exception,
            // not the ordinary idle/deferrable-queue Campus claims.
            if !self.campus_claims_the_free_slot
                && !self.campus_claims_the_free_slot_2
                && !campus_before_settler
            {
                continue;
            }
            if floor_holds && !campus_before_settler {
                self.explain_free_slot_campus_hold(g, cid, turns, "early Settler floor");
                continue;
            }
            match g.cities[&cid].queue.first() {
                Some(item) => {
                    // Preserve the original short-circuit order: report the
                    // first binding hold, rather than guessing from the queue.
                    let hold = if self.deterrence_unit_holds(g, pid, item)
                        && !self.short_seowon_precedes_parity(g, pid, cid, plan, &campus)
                    {
                        Some("near-rival deterrence")
                    } else if self.bound_granary_holds(g, pid, cid, plan, item) {
                        Some("bound Granary claim")
                    } else if self.idle_route_trader_holds(g, pid, cid, plan, item) {
                        Some("idle-route Trader claim")
                    } else if self.charting_hull_holds(g, pid, cid, plan, item) {
                        Some("Religion founder's contact hull")
                    } else if self.first_capital_land_recon_holds(g, pid, cid, plan, item) {
                        Some("first-capital land recon")
                    } else if self.backlog_builder_holds(g, pid, cid, plan, item) {
                        Some("backlog Builder claim")
                    } else if self.capped_aqueduct_holds(g, pid, cid, plan, item) {
                        Some("capped-city Aqueduct claim")
                    } else if self.capped_neighborhood_holds(g, pid, cid, plan, item) {
                        Some("capped-city Neighborhood claim")
                    } else if self.producer_zone_claim_holds(g, pid, cid, plan, item) {
                        Some("producer Industrial Zone claim")
                    } else if self.factory_claim_holds(g, pid, cid, plan, item) {
                        Some("Factory claim")
                    } else if self.campus_building_claim_holds(g, pid, cid, plan, item) {
                        Some("Campus building claim")
                    } else if self.culture_monument_finishes_first(g, pid, cid, plan, item) {
                        Some("near-done Monument")
                    } else if Self::queued_wall_breaker(g, item) {
                        Some("queued wall-breaker")
                    } else if !campus_before_settler
                        && !Self::capped_aqueduct_may_defer(g, pid, cid, item, at_major_war)
                    {
                        let remaining = g.item_cost_for_city(pid, cid, item)
                            - g.item_invested_production(cid, item);
                        if remaining <= g.city_yields(cid).production {
                            Some("queue finishes within one turn")
                        } else if at_major_war
                            && matches!(item, Item::Unit { unit }
                                if g.rules.units.get(unit).is_some_and(|spec| spec.class == "military"))
                        {
                            Some("military queue protected during a major war")
                        } else {
                            Some("queue type or invested progress is not deferrable")
                        }
                    } else {
                        None
                    };
                    if let Some(reason) = hold {
                        self.explain_free_slot_campus_hold(g, cid, turns, reason);
                        continue;
                    }
                }
                // An idle city with a Settler due leaves its queue to the
                // Settler step: the city count is what the gate reads.
                None => {
                    if self.base.settler_due(g, pid, cid, n_cities, settlers) {
                        self.explain_free_slot_campus_hold(
                            g,
                            cid,
                            turns,
                            "Settler due in idle city",
                        );
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

    /// Explain a hold only after the shared picker has found a feasible
    /// first Campus. This records the existing decision; it changes no claim.
    fn explain_free_slot_campus_hold(&self, g: &Game, cid: u32, turns: f64, reason: &str) {
        think!(self.journal(), Cities, Detail,
               "{} postpones its first Campus", g.cities[&cid].name;
               "campus-claims-the-free-slot: {turns:.1} turns; held by {reason}; queue {}",
               g.cities[&cid].queue.first().map_or_else(
                   || "idle".to_string(), Self::plain_item));
    }
}

#[cfg(test)]
mod tests;
