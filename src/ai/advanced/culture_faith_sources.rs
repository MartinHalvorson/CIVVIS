//! Faith for Culture's Naturalists and Rock Bands is useful after the Prophet
//! race closes. Native Pericles 20261107T023905Z had Theology at turn 56 but
//! no Holy Sites across nine cities at 170. The religion-oriented governor
//! cannot supply this economy. Once the first writing slots stand, reserve
//! safe faith items within the existing source budget, developing at most one Holy Site per three
//! cities (three total), and stop once observed Faith income meets the budget.
//!
//! `culture-faith-site-per-theater` lifts both limits: one Holy Site for every
//! city with a standing Theater Square, and Faith claims until income reaches
//! twenty a city. See `CULTURE_FAITH_SITES_PER_THEATER_CAP`. It also researches
//! Astrology once the first Theater Square stands
//! (`AdvancedAi::culture_faith_astrology_due`), so the sites are unlocked
//! when the sources open.

use super::*;

/// `culture-faith-opens-at-the-theaters`: standing Theater Squares that open
/// the Faith sources without waiting for an Amphitheater. Live Deity Pericles
/// 2026-11-08 (45 games) earned a median of 15 Faith a turn at t120, and the
/// lower half averaged 0.6 Holy Sites against 2.0. Amphitheaters reach only
/// 29% of cities by t100, so the original gate opens too late to fund the
/// Rock Bands that unlock at Cold War.
const CULTURE_FAITH_OPENING_THEATERS: usize = 2;

/// `culture-faith-site-per-theater`: the most Holy Sites the Culture Faith
/// claim develops, one per city with a standing Theater Square. Live Deity
/// Online Kristina 2026-11-10 (six games, 3-8 cities at t100) held 0-2 Holy
/// Sites and 0-15 Faith a turn at t100: the one-per-three-cities budget
/// allowed one site in every three- to five-city game, and three of the six
/// games started none before t130. The religion seat on the same lane
/// made 147 Faith a turn at t100 from Shrines and Temples in six of seven
/// cities. A finished Holy Site with its Shrine, Temple, the faith-bought
/// worship building of the faith its city follows and three Holy Site
/// specialists paid 13-29 Faith a turn (most above 20) in every live
/// Kristina game that built one (140731Z, 144009Z, 152217Z, 153619Z).
const CULTURE_FAITH_SITES_PER_THEATER_CAP: usize = 6;

/// `culture-faith-site-per-theater`: Faith a turn per city, and the empire
/// ceiling, below which the claim keeps opening sources. Three complete
/// sources reach the 60 a turn the Rock Bands need by t100; the old ceiling
/// of eight a city stopped a four-city empire at 32.
const CULTURE_FAITH_PER_THEATER_INCOME: (f64, f64) = (20.0, 160.0);

impl AdvancedAi {
    fn culture_faith_source_budget(&self, g: &Game, pid: usize) -> Option<usize> {
        if !(self.culture_seat_theater_first || self.culture_seat_theater_first_2)
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
            || self.lane_lost
        {
            return None;
        }
        let city_ids = g.player_city_ids(pid);
        let cities = city_ids.len();
        let theater_cities = city_ids
            .iter()
            .filter(|cid| {
                g.city_has_district_family(&g.cities[*cid], crate::name!("theater_square"))
            })
            .count();
        let opened_by_theaters = self.culture_faith_opens_at_the_theaters
            && theater_cities >= CULTURE_FAITH_OPENING_THEATERS;
        let per_theater = self.culture_faith_site_per_theater;
        if cities < if per_theater { 2 } else { 3 }
            || !(opened_by_theaters
                || city_ids.iter().any(|cid| {
                    g.cities[cid].buildings.iter().any(|building| {
                        g.building_is_family(*building, crate::name!("amphitheater"))
                    })
                }))
        {
            return None;
        }
        let income = city_ids
            .into_iter()
            .map(|cid| g.city_yields(cid).faith)
            .sum::<f64>()
            + g.player_yield_extras(pid).faith
            + g.arena_side_yields(pid).faith
            + g.observed_yield_adjustments
                .get(&pid)
                .map_or(0.0, |yield_| yield_.faith);
        let (per_city, ceiling) = if per_theater {
            CULTURE_FAITH_PER_THEATER_INCOME
        } else {
            (8.0, 80.0)
        };
        if !income.is_finite()
            || income >= (cities as f64 * per_city).min(ceiling)
            || (g.players[pid].gold_per_turn < 0.0
                && g.players[pid].gold < 100.0 + 25.0 * cities as f64)
        {
            return None;
        }
        Some(if per_theater {
            theater_cities.clamp(1, CULTURE_FAITH_SITES_PER_THEATER_CAP)
        } else {
            (cities / 3).min(3)
        })
    }

    /// `culture-faith-site-per-theater`: whether the research slot takes
    /// Astrology, the Holy Site's unlock, for a Culture seat that has a
    /// standing Theater Square. Astrology is a dead-end branch no Culture
    /// goal walks through: live Deity Kristina 2026-11-10 learned it at t74-79
    /// in four of six games (t43 and t51 in the others), after the Faith
    /// sources had opened, and its first Holy Site followed at t86-t130.
    /// False with the gene off, for another target, and once it is known.
    pub(super) fn culture_faith_astrology_due(&self, g: &Game, pid: usize) -> bool {
        self.culture_faith_site_per_theater
            && (self.culture_seat_theater_first || self.culture_seat_theater_first_2)
            && !self.lane_lost
            && self.active_victory_target(g) == Some(VictoryTarget::Culture)
            && !g.players[pid].techs.contains(&crate::name!("astrology"))
            && g.player_city_ids(pid).into_iter().any(|cid| {
                g.city_has_district_family(&g.cities[&cid], crate::name!("theater_square"))
            })
    }

    fn culture_faith_item(g: &Game, item: &Item) -> bool {
        match item {
            Item::District { district, .. } => g.district_family(*district) == "holy_site",
            Item::Building { building } => [crate::name!("shrine"), crate::name!("temple")]
                .into_iter()
                .any(|family| g.building_is_family(*building, family)),
            _ => false,
        }
    }

    fn culture_faith_site_held(g: &Game, cid: u32) -> bool {
        let city = &g.cities[&cid];
        g.city_has_district_family(city, crate::name!("holy_site"))
            || city.queue.iter().any(|item| {
                matches!(item, Item::District { district, .. }
                    if g.district_family(*district) == "holy_site")
            })
            || city.owned_tiles.iter().any(|pos| {
                g.map
                    .get(*pos)
                    .and_then(|tile| tile.district_foundation.as_ref())
                    .is_some_and(|foundation| g.district_family(foundation.district) == "holy_site")
            })
    }

    fn culture_faith_city_safe(g: &Game, pid: usize, cid: u32, plan: &StrategicPlan) -> bool {
        let city = &g.cities[&cid];
        city.owner == pid
            && plan.threatened_city != Some(cid)
            && (city.last_attacked == 0 || g.turn.saturating_sub(city.last_attacked) > 4)
            && !g.units.values().any(|unit| {
                unit.owner != pid
                    && g.is_at_war(pid, unit.owner)
                    && g.rules.units[&unit.kind].class == "military"
                    && g.wdist(city.pos, unit.pos) <= 6
            })
    }

    pub(super) fn culture_faith_source_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        if !Self::culture_faith_item(g, item) {
            return false;
        }
        let Some(limit) = self.culture_faith_source_budget(g, pid) else {
            return false;
        };
        if !Self::culture_faith_city_safe(g, pid, cid, plan)
            || g.cities[&cid].queue.first() != Some(item)
            || !Self::production_commitment_is_legal(g, pid, cid, item)
        {
            return false;
        }
        !matches!(item, Item::District { .. })
            || g.player_city_ids(pid)
                .into_iter()
                .filter(|city| Self::culture_faith_site_held(g, *city))
                .count()
                <= limit
    }

    pub(super) fn culture_faith_source_target(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Item, f64)> {
        let limit = self.culture_faith_source_budget(g, pid)?;
        let cities = g.player_city_ids(pid);
        // Retain every existing commitment, but a positively quoted Native
        // queue may use the next pipeline inside the existing source budget.
        // Without host quotes the original serialized plan is unchanged.
        let held = cities.iter().find_map(|&cid| {
            let item = g.cities[&cid].queue.first()?;
            self.culture_faith_source_holds(g, pid, cid, plan, item)
                .then(|| (cid, item.clone(), 0.0))
        });
        let active = cities
            .iter()
            .filter(|cid| {
                g.cities[cid]
                    .queue
                    .first()
                    .is_some_and(|item| Self::culture_faith_item(g, item))
            })
            .count();
        if held.is_some() && active >= limit {
            return held;
        }
        let sites = cities
            .iter()
            .filter(|cid| Self::culture_faith_site_held(g, **cid))
            .count();
        let at_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && g.is_at_war(pid, other.id)
        });
        let mut best: Option<(u32, Item, f64)> = None;
        for cid in cities {
            let city = &g.cities[&cid];
            if !Self::culture_faith_city_safe(g, pid, cid, plan)
                || !g.city_has_district_family(city, crate::name!("theater_square"))
            {
                continue;
            }
            if let Some(head) = city.queue.first() {
                if self.culture_theater_holds(g, pid, cid, plan, head)
                    || self.early_settler_floor_holds(g, pid, cid, plan)
                    || self.bound_granary_holds(g, pid, cid, plan, head)
                    || self.capital_monument_holds(g, pid, cid, plan, head)
                    || self.deterrent_holds_against_culture(g, pid, head)
                    || self.idle_route_trader_holds(g, pid, cid, plan, head)
                    || self.backlog_builder_holds(g, pid, cid, plan, head)
                    || self.capped_aqueduct_holds(g, pid, cid, plan, head)
                    || !Self::culture_theater_may_defer(g, pid, cid, head, at_war)
                {
                    continue;
                }
            }
            let item = if g.city_has_district_family(city, crate::name!("holy_site")) {
                BasicAi::civ_building(g, pid, cid, "shrine")
                    .or_else(|| BasicAi::civ_building(g, pid, cid, "temple"))
            } else if sites < limit {
                BasicAi::first_district_item(g, pid, cid, "holy_site")
            } else {
                None
            };
            let Some(item) = item else {
                continue;
            };
            let quote = g.host_production_turns(cid, &item);
            if held.is_some() && quote.is_none() {
                continue;
            }
            let turns = quote.unwrap_or_else(|| {
                (g.item_cost_for_city(pid, cid, &item) - g.item_invested_production(cid, &item))
                    .max(0.0)
                    / g.city_yields(cid).production.max(0.5)
            });
            let cap = g.standard_duration(20) as f64;
            if !turns.is_finite()
                || turns > cap
                || g.max_turns.saturating_sub(g.turn) as f64 <= turns + cap * 2.0
            {
                continue;
            }
            if best.as_ref().is_none_or(|(old_city, _, old_turns)| {
                turns.total_cmp(old_turns).then(cid.cmp(old_city)).is_lt()
            }) {
                best = Some((cid, item, turns));
            }
        }
        best.or(held)
    }

    pub(super) fn claim_culture_faith_source(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        let Some((city, item, turns)) = self.culture_faith_source_target(g, pid, plan) else {
            return;
        };
        if g.cities[&city].queue.first() == Some(&item) {
            return;
        }
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Cities, Decision,
                "{} claims {} for Culture's Faith economy", g.cities[&city].name, Self::plain_item(&item);
                "one safe source at a time, {turns:.1} production turns; Faith supports Naturalists and Rock Bands after the Prophet race");
        }
    }
}

#[cfg(test)]
mod tests;
