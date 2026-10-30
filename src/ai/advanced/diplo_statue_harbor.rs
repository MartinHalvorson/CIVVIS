//! `diplo-seat-statue-harbor`: on a seat aiming at a Diplomatic victory, the
//! strongest coastal city builds its Harbor once Celestial Navigation allows
//! it, so the Statue of Liberty (+4 Diplomatic Victory Points) is producible
//! the turn Civil Engineering lands.
//!
//! **Why (live Emperor Pericles, 2026-10-30).**
//! - The diplomatic racer stalls short of twenty: civvis-20261030T111154Z sat
//!   at 18 points from t183 to its t213 loss, every rival voting B on our
//!   claim, so the last points had to come from outside the Congress. The
//!   Statue of Liberty is the largest such source, four points.
//! - The Statue must stand on a coast tile next to a Harbor
//!   (`wonders.json`: `adjacent_district: harbor`). In 101932Z it was never
//!   once on our buildable list: our only Harbor stood in a 15-19 Production
//!   city, from t160. `diplo-seat-wonder-claim` can only claim what the menu
//!   offers.
//!
//! **What the gene does.** For a Diplomacy seat that has not seen the Statue
//! built or ruled out by the host, when no city of ours with at least
//! [`STATUE_HARBOR_PRODUCTION_SHARE`] of the best candidate's Production holds
//! or queues a Harbor that leaves the Statue a site, the unthreatened city with
//! the most Production that can build its first Harbor in time on such a site
//! ([`AdvancedAi::statue_room`]) claims it:
//! an idle queue, or one over a deferrable head that keeps its progress (the
//! Theater claim's rule), with the other claims' heads kept. The delegated
//! governor's rescoring leaves it alone ([`AdvancedAi::statue_harbor_holds`]).
//!
//! Off, or for a seat not aiming at Diplomacy, nothing changes.

use super::*;

/// A Harbor in a city with at least this share of the best candidate's
/// Production already serves the Statue.
pub(super) const STATUE_HARBOR_PRODUCTION_SHARE: f64 = 0.75;

/// Build turns past which a city does not start the Statue's Harbor: the
/// first-district horizon `BasicAi::first_harbor_item` uses.
pub(super) const STATUE_HARBOR_MAX_TURNS: f64 = 15.0;

impl AdvancedAi {
    /// Whether the Statue of Liberty is still open to this seat: no city on
    /// the board holds it and the host has not ruled it out.
    fn statue_still_open(g: &Game) -> bool {
        let statue = crate::name!("statue_of_liberty");
        g.rules.wonders.contains_key(&statue)
            && !g.host_unavailable_wonders.contains(&statue)
            && !g
                .cities
                .values()
                .any(|city| city.wonders.contains_key(&statue))
    }

    /// Whether a Harbor at `harbor` leaves the Statue of Liberty a site: a free
    /// coast tile of `city`'s, within three tiles of the centre, beside land
    /// and beside the Harbor (`wonder_sites`' rule for a water, coast wonder
    /// with `adjacent_district: harbor`). Live Immortal
    /// civvis-20261030T173341Z held Harbors in Corinth (24 Production) and
    /// Ephesus (39) whose coast offered the Statue nothing, and the only offer
    /// was Megara's, at 10 Production and 62 turns.
    pub(super) fn statue_room(g: &Game, city: &crate::game::City, harbor: Pos) -> bool {
        g.nbrs(harbor).into_iter().any(|pos| {
            pos != city.pos
                && city.owned_tiles.contains(&pos)
                && g.wdist(pos, city.pos) <= 3
                && g.map.tiles.get(&pos).is_some_and(|tile| {
                    tile.terrain == "coast"
                        && !tile.flooded
                        && tile.district.is_none()
                        && tile.district_foundation.is_none()
                        && tile.wonder.is_none()
                        && tile.resource.as_ref().is_none_or(|resource| {
                            matches!(
                                g.rules.resources[resource].class.as_str(),
                                "bonus" | "artifact"
                            )
                        })
                })
                && g.nbrs(pos)
                    .into_iter()
                    .any(|next| g.map.get(next).is_some_and(|tile| !g.rules.is_water(tile)))
        })
    }

    /// Whether `city` holds or queues a Harbor that leaves the Statue a site.
    fn has_statue_harbor(g: &Game, city: &crate::game::City) -> bool {
        city.districts.iter().any(|(district, pos)| {
            g.district_family(*district) == "harbor" && Self::statue_room(g, city, *pos)
        }) || city.queue.iter().any(|item| {
            matches!(item, Item::District { district, pos }
                    if g.district_family(*district) == "harbor"
                        && Self::statue_room(g, city, *pos))
        })
    }

    /// `cid`'s first Harbor on a site that leaves the Statue room, buildable
    /// within `STATUE_HARBOR_MAX_TURNS`, with its build turns.
    fn statue_harbor_site(g: &Game, pid: usize, cid: u32) -> Option<(Item, f64)> {
        let city = g.cities.get(&cid)?;
        if g.city_has_district_family(city, crate::name!("harbor")) {
            return None;
        }
        let district = BasicAi::civ_district(g, pid, "harbor");
        let production = g.city_yields(cid).production.max(0.5);
        g.district_sites(cid, district)
            .into_iter()
            .filter(|pos| Self::statue_room(g, city, *pos))
            .map(|pos| Item::District { district, pos })
            .find(|item| g.can_produce(pid, cid, item))
            .map(|item| {
                let turns = g
                    .host_production_turns(cid, &item)
                    .unwrap_or_else(|| g.item_cost_for(pid, &item) / production);
                (item, turns)
            })
            .filter(|(_, turns)| *turns <= STATUE_HARBOR_MAX_TURNS)
    }

    /// The city that should build the Statue's Harbor and the Harbor item,
    /// with its build turns. `None` with the gene off, for another target,
    /// once a strong enough city of ours holds or queues one, or when no city
    /// can build one in time.
    pub(super) fn statue_harbor_item(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Item, f64)> {
        if !self.diplo_seat_statue_harbor
            || self.active_victory_target(g) != Some(VictoryTarget::Diplomacy)
            || !Self::statue_still_open(g)
        {
            return None;
        }
        let ours = g.player_city_ids(pid);
        let (best_production, cid, item) = ours
            .iter()
            .copied()
            .filter(|cid| {
                let city = &g.cities[cid];
                plan.threatened_city != Some(*cid)
                    && !(city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            })
            .filter_map(|cid| {
                Self::statue_harbor_site(g, pid, cid)
                    .map(|(item, turns)| (g.city_yields(cid).production, cid, item, turns))
            })
            .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
            .map(|(production, cid, item, turns)| (production, cid, (item, turns)))?;
        let served = ours.iter().any(|other| {
            let city = &g.cities[other];
            Self::has_statue_harbor(g, city)
                && g.city_yields(*other).production
                    >= STATUE_HARBOR_PRODUCTION_SHARE * best_production
        });
        if served {
            return None;
        }
        let (item, turns) = item;
        Some((cid, item, turns))
    }

    /// Whether `item` at the head of `cid` is the Statue's Harbor this gene
    /// claims: the delegated governor's rescoring leaves it alone. Always
    /// false with the gene off.
    pub(super) fn statue_harbor_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        if !self.diplo_seat_statue_harbor
            || self.active_victory_target(g) != Some(VictoryTarget::Diplomacy)
            || !Self::statue_still_open(g)
        {
            return false;
        }
        matches!(item, Item::District { district, .. } if g.district_family(*district) == "harbor")
            && g.cities.get(&cid).is_some_and(|city| {
                city.owner == pid
                    && plan.threatened_city != Some(cid)
                    && !g.city_has_district_family(city, crate::name!("harbor"))
            })
    }

    /// See the module. Exact no-op with the gene off or for another target.
    pub(super) fn claim_statue_harbor(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let Some((cid, harbor, turns)) = self.statue_harbor_item(g, pid, plan) else {
            return;
        };
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_barbarian
                && !other.is_minor
                && g.is_at_war(pid, other.id)
        });
        let head = g.cities[&cid].queue.first().cloned();
        if let Some(item) = &head {
            if self.early_settler_floor_holds(g, pid, cid, plan)
                || self.bound_granary_holds(g, pid, cid, plan, item)
                || self.capital_monument_holds(g, pid, cid, plan, item)
                || self.deterrence_unit_holds(g, pid, item)
                || self.idle_route_trader_holds(g, pid, cid, plan, item)
                || self.backlog_builder_holds(g, pid, cid, plan, item)
                || self.capped_aqueduct_holds(g, pid, cid, plan, item)
                || !Self::culture_theater_may_defer(g, pid, cid, item, at_major_war)
            {
                return;
            }
        }
        let name = g.cities[&cid].name.clone();
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: harbor.clone(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Cities, Decision,
                   "{name} claims its {} for the Statue of Liberty", Self::plain_item(&harbor);
                   "diplo-seat-statue-harbor: {turns:.1} turns, the strongest coastal city, \
                    ahead of {}",
                   head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                       "{} (it keeps its progress)", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
