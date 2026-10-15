//! `neighborhood-houses-the-capped-city`: after Urbanization, a city of
//! [`CAPPED_NEIGHBORHOOD_MIN_POP`] or more that grows at half its rate or
//! worse for want of housing (headroom under 2) and has no Aqueduct left to
//! claim puts a Neighborhood at the head of its queue, on its legal site that
//! houses the most (2 to 6 by the tile's Appeal), over an idle queue or a
//! deferrable head, when it finishes within
//! [`CAPPED_NEIGHBORHOOD_MAX_STANDARD`] standard turns. One claim a turn, for
//! the most capped city, and no more than one Neighborhood in flight per
//! [`NEIGHBORHOOD_CITIES_PER_CLAIM`] cities, so a war's production is not
//! spent on housing all at once. A threatened or freshly attacked city keeps
//! its queue (`city-under-fire-ranks-first` holds it). The displaced item
//! keeps its progress.
//!
//! **Why (2026-10-15 census, Emperor GC runs from 10-13).** -34's
//! decomposition of the t140 production gap (0.72 of the top AI's per city)
//! puts about 70% of it on city size and the size on housing: 42% of our
//! cities are housing-limited at t140 and 85 of them can build no Aqueduct
//! (no fresh water). Urbanization itself is late for want of Culture (a
//! perfect beeline reaches it at a median t133 against the top AI's t101;
//! we adopt it at t149), so this gene is for the passers' late game, where a
//! Domination win has to convert. Of the 33 games that reached Urbanization,
//! 3 ever built a Neighborhood, while 3 to 10 of their cities stood
//! housing-capped at the end: the district chooser never asks for one.
//!
//! The claim runs right after `aqueduct-claims-the-capped-queue` (a city with
//! a legal Aqueduct waits for that cheaper fix), so the Settler, the Granary,
//! the deterrent, the Trader, the backlog Builder and the Aqueduct keep their
//! order; the routine claims and the governor's rescoring then leave the
//! claimed Neighborhood alone ([`AdvancedAi::capped_neighborhood_holds`]).
//! Off, nothing runs.

use super::*;

/// The longest the claimed Neighborhood may take, in standard turns.
pub(crate) const CAPPED_NEIGHBORHOOD_MAX_STANDARD: u32 = 40;

/// The smallest city the claim builds for.
pub(crate) const CAPPED_NEIGHBORHOOD_MIN_POP: i32 = 4;

/// Cities per Neighborhood in flight at once (at least one).
pub(crate) const NEIGHBORHOOD_CITIES_PER_CLAIM: usize = 5;

/// Housing headroom under which the host halves growth or worse
/// (`Game::housing_growth_mult`: 2 or more grows at full rate).
const CAPPED_HEADROOM: f64 = 2.0;

/// The host's growth factor at a housing headroom (`Game::housing_growth_mult`).
fn growth_factor(headroom: f64) -> f64 {
    if headroom >= 2.0 {
        1.0
    } else if headroom >= 1.0 {
        0.5
    } else if headroom > -4.0 {
        0.25
    } else {
        0.0
    }
}

/// The Neighborhood a city would claim: the item, its turns, the city's
/// housing headroom, and the housing the site adds.
pub(crate) struct CappedNeighborhood {
    pub item: Item,
    pub turns: f64,
    pub headroom: f64,
    pub housing: f64,
}

impl AdvancedAi {
    /// Whether `item` is a Neighborhood (or a civilization's own).
    fn is_neighborhood(g: &Game, item: &Item) -> bool {
        matches!(item, Item::District { district, .. }
            if g.district_family(*district) == crate::name!("neighborhood"))
    }

    /// The Neighborhood `cid` would claim: an unthreatened city of
    /// [`CAPPED_NEIGHBORHOOD_MIN_POP`] or more with housing headroom under 2,
    /// no legal Aqueduct left to build, and a legal Neighborhood site that
    /// finishes in time, the one that houses most. `None` with the gene off.
    pub(super) fn capped_neighborhood_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<CappedNeighborhood> {
        if !self.neighborhood_houses_the_capped_city {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || city.pop < CAPPED_NEIGHBORHOOD_MIN_POP
        {
            return None;
        }
        let headroom = g.city_housing_headroom(city);
        if headroom >= CAPPED_HEADROOM {
            return None;
        }
        // `capital-walls-before-the-surprise` takes the capital's queue after
        // every ordinary writer while its Walls are armed; claiming there too
        // would alternate the two.
        let walls = Item::Building {
            building: crate::name!("walls"),
        };
        if self.capital_walls_hold(g, pid, cid, &walls) {
            return None;
        }
        // The Aqueduct is the cheaper housing: a city that can still build
        // one leaves it to `aqueduct-claims-the-capped-queue`.
        let aqueduct = BasicAi::civ_district(g, pid, "aqueduct");
        if !g.city_has_district_family(city, crate::name!("aqueduct"))
            && g.district_sites(cid, aqueduct).into_iter().any(|pos| {
                g.can_produce(
                    pid,
                    cid,
                    &Item::District {
                        district: aqueduct,
                        pos,
                    },
                )
            })
        {
            return None;
        }
        let neighborhood = BasicAi::civ_district(g, pid, "neighborhood");
        let (pos, housing) = g
            .district_sites(cid, neighborhood)
            .into_iter()
            .filter(|pos| {
                g.can_produce(
                    pid,
                    cid,
                    &Item::District {
                        district: neighborhood,
                        pos: *pos,
                    },
                )
            })
            .map(|pos| (pos, g.district_housing(neighborhood.as_str(), pos)))
            .max_by(|(left_pos, left), (right_pos, right)| {
                left.total_cmp(right).then_with(|| right_pos.cmp(left_pos))
            })?;
        let item = Item::District {
            district: neighborhood,
            pos,
        };
        let production = g.city_yields(cid).production;
        let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
            let remaining =
                g.item_cost_for_city(pid, cid, &item) - g.item_invested_production(cid, &item);
            remaining.max(0.0) / production.max(0.5)
        });
        (turns <= g.standard_duration(CAPPED_NEIGHBORHOOD_MAX_STANDARD) as f64).then_some(
            CappedNeighborhood {
                item,
                turns,
                headroom,
                housing,
            },
        )
    }

    /// Whether `item` at the head of `cid` is a Neighborhood this gene would
    /// still claim there: the routine claims and the delegated governor's
    /// rescoring leave it alone. Always false with the gene off.
    pub(super) fn capped_neighborhood_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        Self::is_neighborhood(g, item) && self.capped_neighborhood_item(g, pid, cid, plan).is_some()
    }

    /// See the module: the most capped city puts its best-housing
    /// Neighborhood at the head of its queue, idle or over a deferrable item,
    /// while fewer than one per [`NEIGHBORHOOD_CITIES_PER_CLAIM`] cities is
    /// in flight. One claim a turn. Exact no-op with the gene off.
    pub(super) fn claim_capped_neighborhood(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.neighborhood_houses_the_capped_city {
            return;
        }
        let city_ids = g.player_city_ids(pid);
        let n_cities = city_ids.len();
        let in_flight = city_ids
            .iter()
            .filter(|cid| {
                g.cities[cid]
                    .queue
                    .iter()
                    .any(|item| Self::is_neighborhood(g, item))
            })
            .count();
        if in_flight >= (n_cities / NEIGHBORHOOD_CITIES_PER_CLAIM).max(1) {
            return;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_barbarian
                && !other.is_minor
                && g.is_at_war(pid, other.id)
        });
        let settlers = self.counts(g, pid).settlers;
        let mut best: Option<(f64, f64, f64, u32, Item)> = None;
        for cid in city_ids {
            let Some(claim) = self.capped_neighborhood_item(g, pid, cid, plan) else {
                continue;
            };
            if self.early_settler_floor_holds(g, pid, cid, plan) {
                continue;
            }
            match g.cities[&cid].queue.first() {
                Some(item) => {
                    if Self::is_neighborhood(g, item)
                        || self.deterrence_unit_holds(g, pid, item)
                        || self.bound_granary_holds(g, pid, cid, plan, item)
                        || self.idle_route_trader_holds(g, pid, cid, plan, item)
                        || self.backlog_builder_holds(g, pid, cid, plan, item)
                        || self.capped_aqueduct_holds(g, pid, cid, plan, item)
                        || Self::queued_wall_breaker(g, item)
                        || !Self::capped_aqueduct_may_defer(g, pid, cid, item, at_major_war)
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
            // The most capped city first; then the site that houses most;
            // then the quickest; then the lowest id.
            let key = (claim.headroom, -claim.housing, claim.turns, cid);
            if best.as_ref().is_none_or(|(headroom, housing, turns, city, _)| {
                key < (*headroom, -*housing, *turns, *city)
            }) {
                best = Some((claim.headroom, claim.housing, claim.turns, cid, claim.item));
            }
        }
        let Some((headroom, housing, turns, cid, item)) = best else {
            return;
        };
        let head = g.cities[&cid].queue.first().cloned();
        let (name, pop, cap) = {
            let city = &g.cities[&cid];
            (city.name.clone(), city.pop, g.city_housing(city))
        };
        let Item::District { pos, .. } = item else {
            return;
        };
        if g.apply(pid, &Action::Produce { city: cid, item }).is_ok() {
            think!(self.journal(), Cities, Decision,
                   "{name} builds a Neighborhood";
                   "neighborhood-houses-the-capped-city: population {pop} against housing {cap:.0}, \
                    growth x{}; the site at {pos:?} houses {housing:.0} by its Appeal, {turns:.1} turns, \
                    ahead of {}",
                   growth_factor(headroom),
                   head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                       "{} (it keeps its progress)", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
