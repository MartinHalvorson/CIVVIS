//! `aqueduct-claims-the-capped-queue`: a city within one of its housing,
//! with its Granary built (or none to build) and no Aqueduct, whose legal
//! Aqueduct (Rome's Bath) it can finish within
//! [`CAPPED_AQUEDUCT_MAX_STANDARD`] standard turns, puts that Aqueduct at the
//! head of its queue ahead of the delegated governor's rescoring, over an idle
//! queue or a deferrable head. One claim a turn, for the most capped city. A
//! Settler, a Builder or Trader, Walls, a wonder, a project, the faith
//! defence's Holy Site, Shrine or Temple, the science and culture items
//! (Campus, Theater Square, Government Plaza, Industrial Zone, Monument,
//! Library, University, Amphitheater), a claimed Granary, deterrent, Trader or
//! backlog Builder, the early Settler floor's city, a threatened or freshly
//! attacked city, a wall breaker the siege reservation queued (any siege
//! unit; [`AdvancedAi::queued_wall_breaker`]), and anything finishing this
//! turn keep their queue; at war with a major a military unit keeps its
//! queue too. The displaced item keeps its progress.
//!
//! **Why (2026-10-11 census, 19 live Immortal and 45 Emperor GC runs).**
//! Food per citizen is at parity with the AI (gross food per population, ours
//! 2.67 vs the most populous rival's 2.63 at t100 at Immortal), yet our
//! population runs 0.57-0.67× theirs at equal city counts, and the housing
//! ceiling is the binder: at Immortal t100, 34% of our cities grow at the
//! host's ×0.25 (at or over housing) and 25% at ×0.5 (one under), and only
//! 8% hold an Aqueduct (18% at t125). Of the capped cities without one, 22 of
//! 67 at t100 (Emperor 39 of 139) had a LEGAL Aqueduct in the host's
//! buildable list, 20 of them with the Granary already built. They sat on a
//! Campus, Holy Site, Settler, Builder, Library or an idle frame instead.
//! `housing-cap-builds-the-aqueduct` already puts the Aqueduct first in
//! `BasicAi::pick_item`, but that step reads an idle queue the controller's
//! claims take first, so it journalled 37 Aqueducts across 19 Immortal games.
//!
//! The claim runs after the backlog Builder and before the Prophet race and
//! the routine claims, so a due Settler, the Granary, the deterrent, the
//! Trader and the backlog Builder keep their order; the deterrent and the
//! governor's rescoring then leave the claimed Aqueduct alone
//! ([`AdvancedAi::capped_aqueduct_holds`]). Off, nothing runs.

use super::*;

/// The longest the claimed Aqueduct may take, in standard turns (24 live
/// turns at the seat's speed): a city at its housing grows at a quarter to a
/// half of its rate, so the district pays for its own turns there; the cap
/// keeps a tiny city from spending half the game on it.
pub(crate) const CAPPED_AQUEDUCT_MAX_STANDARD: u32 = 36;

/// The smallest city the claim builds for: a newly founded one reaches its
/// housing on its first improvements.
pub(crate) const CAPPED_AQUEDUCT_MIN_POP: i32 = 3;

impl AdvancedAi {
    /// The Aqueduct `cid` would claim, how long it takes, and how far over
    /// its housing the city stands: an unthreatened city within one of its
    /// housing, with its Granary built or unbuildable and no Aqueduct, whose
    /// legal Aqueduct finishes in time. `None` with the gene off.
    pub(super) fn capped_aqueduct_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64, f64)> {
        if !self.aqueduct_claims_the_capped_queue {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || city.pop < CAPPED_AQUEDUCT_MIN_POP
        {
            return None;
        }
        let housing = g.city_housing(city);
        let over = city.pop as f64 + 1.0 - housing;
        if over < 0.0
            || BasicAi::civ_building(g, pid, cid, "granary").is_some()
            || g.city_has_district_family(city, crate::name!("aqueduct"))
        {
            return None;
        }
        let aqueduct = BasicAi::civ_district(g, pid, "aqueduct");
        let pos = g
            .district_sites(cid, aqueduct)
            .into_iter()
            .filter(|pos| {
                g.can_produce(
                    pid,
                    cid,
                    &Item::District {
                        district: aqueduct,
                        pos: *pos,
                    },
                )
            })
            .min()?;
        let item = Item::District {
            district: aqueduct,
            pos,
        };
        let production = g.city_yields(cid).production;
        let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
            let remaining =
                g.item_cost_for_city(pid, cid, &item) - g.item_invested_production(cid, &item);
            remaining.max(0.0) / production.max(0.5)
        });
        (turns <= g.standard_duration(CAPPED_AQUEDUCT_MAX_STANDARD) as f64)
            .then_some((item, turns, over))
    }

    /// Whether `item` at the head of `cid` is the Aqueduct this gene claims
    /// and the city still wants it: the deterrent and the delegated
    /// governor's rescoring leave it alone. Always false with the gene off.
    pub(super) fn capped_aqueduct_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        matches!(item, Item::District { district, .. }
            if g.district_family(*district) == crate::name!("aqueduct"))
            && self
                .capped_aqueduct_item(g, pid, cid, plan)
                .is_some_and(|(aqueduct, _, _)| aqueduct == *item)
    }

    /// Whether the queue head `item` may wait for the Aqueduct: a routine
    /// military unit at peace or a scout, a non-wonder building outside the
    /// defence, faith, housing, science and culture lists, or an uninvested
    /// district outside them; never anything finishing this turn.
    pub(super) fn capped_aqueduct_may_defer(
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
            Item::Unit { unit } => match g.rules.units.get(unit) {
                Some(spec) if spec.class == "military" => !at_major_war,
                Some(_) => *unit == "scout",
                None => false,
            },
            Item::Building { building } => g.rules.buildings.get(building).is_some_and(|spec| {
                !spec.wonder
                    && !matches!(
                        building.as_str(),
                        "walls"
                            | "medieval_walls"
                            | "renaissance_walls"
                            | "shrine"
                            | "temple"
                            | "granary"
                            | "water_mill"
                            | "monument"
                            | "library"
                            | "university"
                            | "amphitheater"
                    )
            }),
            Item::District { district, .. } => {
                let family = g.district_family(*district);
                g.item_invested_production(cid, item) <= 0.0
                    && ![
                        crate::name!("aqueduct"),
                        crate::name!("holy_site"),
                        crate::name!("campus"),
                        crate::name!("theater_square"),
                        crate::name!("government_plaza"),
                        crate::name!("industrial_zone"),
                    ]
                    .contains(&family)
            }
            _ => false,
        }
    }

    /// See the module: the most capped city with a legal, timely Aqueduct
    /// puts it at the head of its queue, idle or over a deferrable item. One
    /// claim a turn. Exact no-op with the gene off.
    pub(super) fn claim_capped_aqueduct(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.aqueduct_claims_the_capped_queue {
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
            let Some((aqueduct, turns, over)) = self.capped_aqueduct_item(g, pid, cid, plan) else {
                continue;
            };
            if self.early_settler_floor_holds(g, pid, cid, plan) {
                continue;
            }
            match g.cities[&cid].queue.first() {
                Some(item) => {
                    if *item == aqueduct
                        || self.deterrence_unit_holds(g, pid, item)
                        || self.bound_granary_holds(g, pid, cid, plan, item)
                        || self.idle_route_trader_holds(g, pid, cid, plan, item)
                        || self.backlog_builder_holds(g, pid, cid, plan, item)
                        || Self::queued_wall_breaker(g, item)
                        || !Self::capped_aqueduct_may_defer(g, pid, cid, item, at_major_war)
                    {
                        continue;
                    }
                }
                // An idle city with a Settler due leaves its queue to the
                // Settler step: the Settler relieves the housing too, and the
                // city count is what the gate reads.
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
                best = Some((over, turns, cid, aqueduct));
            }
        }
        let Some((_, turns, cid, aqueduct)) = best else {
            return;
        };
        let head = g.cities[&cid].queue.first().cloned();
        let (name, pop, housing) = {
            let city = &g.cities[&cid];
            (city.name.clone(), city.pop, g.city_housing(city))
        };
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: aqueduct,
            },
        )
        .is_ok()
        {
            think!(self.journal(), Cities, Decision,
                   "{name} claims its Aqueduct at its housing";
                   "aqueduct-claims-the-capped-queue: population {pop} against housing \
                    {housing:.0} with its Granary built, {turns:.1} turns, ahead of {}",
                   head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                       "{} (it keeps its progress)", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
