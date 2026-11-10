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
//!
//! `culture-faith-site-per-theater-2` carries version one and claims the
//! sources in parallel, one Faith item per Theater city at a time. Without a
//! host production quote the claim used to keep one item in the whole empire:
//! live Deity Kristina 20261110T183027Z, armed with version one, built its
//! Holy Site at t60, the Shrine at t69, the Temple at t92 and the next Holy
//! Site only at t121, while that city also ran a festival, a Spy and Mont St
//! Michel; Faith stood at 12 a turn at t100 against the 60 the bands need.
//! Version two also goes ahead of a wonder or an Aqueduct at the head
//! of a Theater city's queue (`culture_faith_passes_the_head`): a paired
//! decider read of 22 live boards found serialization never bound, while
//! those heads, a backlog Builder, an early-floor Settler, an Amphitheater
//! and war nearby did. The Settler, Builder, Theater, Amphitheater, Trader
//! and threatened-defence holds still apply, and so does the safety rule.
//! `culture-gold-buys-the-shrines` pays for the buildings behind a finished
//! Holy Site: while the sources are open, the purchase pass buys one Shrine
//! (else Temple) a turn with Gold in a safe Holy Site city, cheapest first,
//! keeping the upkeep floor plus `CULTURE_SHRINE_GOLD_RESERVE`.
//! The production the city would have spent (35 and 60 on Online) goes to its
//! next claim instead.

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

/// `culture-gold-buys-the-shrines`: Gold the Shrine or Temple buy leaves in
/// the treasury on top of the army's upkeep floor
/// (`purchases-keep-the-upkeep-reserve`). Live Deity Online Kristina
/// 2026-11-10 (six games with a Holy Site) held 120-290 Gold and earned +8 to
/// +81 a turn when each Holy Site finished, yet waited 2-38 turns for the
/// Shrine and 5-53 for the Temple (Online prices 140 and 240); a reserve that
/// grew with the empire (25 a city) would have kept most Temples unaffordable.
const CULTURE_SHRINE_GOLD_RESERVE: f64 = 50.0;

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
        let per_theater = self.culture_faith_per_theater_on();
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
        self.culture_faith_per_theater_on()
            && (self.culture_seat_theater_first || self.culture_seat_theater_first_2)
            && !self.lane_lost
            && self.active_victory_target(g) == Some(VictoryTarget::Culture)
            && !g.players[pid].techs.contains(&crate::name!("astrology"))
            && g.player_city_ids(pid).into_iter().any(|cid| {
                g.city_has_district_family(&g.cities[&cid], crate::name!("theater_square"))
            })
    }

    /// `culture-faith-site-per-theater`, carried by its version two.
    fn culture_faith_per_theater_on(&self) -> bool {
        self.culture_faith_site_per_theater || self.culture_faith_site_per_theater_2
    }

    /// `culture-faith-site-per-theater-2`: a queue head the Culture Faith
    /// claim goes ahead of: a wonder or an Aqueduct with more than two turns of
    /// work left. A Spy is not passed: `science-denial-trains-spies` takes the
    /// queue back in the same frame. A Holy Site with its Shrine and Temple costs about
    /// 120 Online production and pays 13-29 Faith a turn, two or three more
    /// Rock Bands over t80-t180 (about 22-33 visitors); a 355-460 production
    /// wonder is worth about one visitor. Every other head keeps its holds.
    fn culture_faith_passes_the_head(g: &Game, pid: usize, cid: u32, head: &Item) -> bool {
        let remaining =
            g.item_cost_for_city(pid, cid, head) - g.item_invested_production(cid, head);
        if !(remaining > 2.0 * g.city_yields(cid).production.max(0.5)) {
            return false;
        }
        match head {
            Item::Wonder { .. } => true,
            Item::Building { building } => g
                .rules
                .buildings
                .get(building)
                .is_some_and(|spec| spec.wonder),
            Item::District { district, .. } => g.district_family(*district) == "aqueduct",
            _ => false,
        }
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
        self.culture_faith_source_scan(g, pid, plan, None)
    }

    /// [`Self::culture_faith_source_target`], noting in `skips` why each
    /// Theater city took no source (`culture-faith-site-per-theater-2`'s
    /// journal line).
    fn culture_faith_source_scan(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        mut skips: Option<&mut Vec<String>>,
    ) -> Option<(u32, Item, f64)> {
        let mut skip = |cid: u32, why: String| {
            if let Some(skips) = skips.as_deref_mut() {
                skips.push(format!("{} {why}", g.cities[&cid].name));
            }
        };
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
            if !g.city_has_district_family(city, crate::name!("theater_square")) {
                continue;
            }
            if !Self::culture_faith_city_safe(g, pid, cid, plan) {
                skip(cid, "is not safe".into());
                continue;
            }
            if let Some(head) = city.queue.first() {
                // `culture-faith-site-per-theater-2`: a city already building
                // a source keeps it; the next claim goes to another city.
                if self.culture_faith_site_per_theater_2 && Self::culture_faith_item(g, head) {
                    skip(cid, "is building its source".into());
                    continue;
                }
                let passes = self.culture_faith_site_per_theater_2
                    && !self.early_settler_floor_holds(g, pid, cid, plan)
                    && Self::culture_faith_passes_the_head(g, pid, cid, head);
                if !passes
                    && (self.culture_theater_holds(g, pid, cid, plan, head)
                        || self.early_settler_floor_holds(g, pid, cid, plan)
                        || self.bound_granary_holds(g, pid, cid, plan, head)
                        || self.capital_monument_holds(g, pid, cid, plan, head)
                        || self.deterrent_holds_against_culture(g, pid, head)
                        || self.idle_route_trader_holds(g, pid, cid, plan, head)
                        || self.backlog_builder_holds(g, pid, cid, plan, head)
                        || self.capped_aqueduct_holds(g, pid, cid, plan, head)
                        || !Self::culture_theater_may_defer(g, pid, cid, head, at_war))
                {
                    skip(cid, format!("keeps its {}", Self::plain_item(head)));
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
                skip(cid, "has no legal source within the budget".into());
                continue;
            };
            let quote = g.host_production_turns(cid, &item);
            if held.is_some() && quote.is_none() && !self.culture_faith_site_per_theater_2 {
                continue;
            }
            let turns = quote.unwrap_or_else(|| {
                (g.item_cost_for_city(pid, cid, &item) - g.item_invested_production(cid, &item))
                    .max(0.0)
                    / g.city_yields(cid).production.max(0.5)
            });
            // `culture-faith-site-per-theater-2`: a Holy Site priced by the
            // tree's progress takes 10-20 Online turns from t90, past the
            // twenty-standard-turn cap, so the version-two cap is thirty.
            let cap = g.standard_duration(if self.culture_faith_site_per_theater_2 {
                30
            } else {
                20
            }) as f64;
            if !turns.is_finite()
                || turns > cap
                || g.max_turns.saturating_sub(g.turn) as f64 <= turns + cap * 2.0
            {
                skip(
                    cid,
                    format!(
                        "needs {turns:.0} turns for its {} (cap {cap:.0})",
                        Self::plain_item(&item)
                    ),
                );
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
        let mut skips = Vec::new();
        let target = self.culture_faith_source_scan(
            g,
            pid,
            plan,
            self.culture_faith_site_per_theater_2.then_some(&mut skips),
        );
        let Some((city, item, turns)) = target else {
            if !skips.is_empty() {
                think!(self.journal(), Cities, Detail,
                    "No new Faith source this turn";
                    "culture-faith-site-per-theater-2: {}", skips.join("; "));
            }
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
            let pace = if self.culture_faith_site_per_theater_2 {
                "one source per Theater city at a time"
            } else {
                "one safe source at a time"
            };
            think!(self.journal(), Cities, Decision,
                "{} claims {} for Culture's Faith economy", g.cities[&city].name, Self::plain_item(&item);
                "{pace}, {turns:.1} production turns; Faith supports Naturalists and Rock Bands after the Prophet race");
        }
    }

    /// `culture-gold-buys-the-shrines`: the Shrine or Temple the treasury
    /// buys this turn, as (city, building, Gold price). While the Culture
    /// Faith sources are open (`culture_faith_source_budget`, which already
    /// stops once Faith meets its ceiling or the treasury is draining), a safe
    /// city with a standing Holy Site and no Shrine (or a Shrine and no
    /// Temple) is a candidate; the cheapest wins, lowest city id on a tie.
    /// The buy must leave the upkeep floor plus
    /// [`CULTURE_SHRINE_GOLD_RESERVE`], and income must not be negative.
    /// `None` with the gene off or for another target.
    pub(super) fn culture_shrine_purchase(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Name, f64)> {
        if !self.culture_gold_buys_the_shrines || g.players[pid].gold_per_turn < 0.0 {
            return None;
        }
        self.culture_faith_source_budget(g, pid)?;
        let cities = g.player_city_ids(pid);
        let floor = self.purchase_upkeep_floor(g, pid, 0.0) + CULTURE_SHRINE_GOLD_RESERVE;
        let gold = g.players[pid].gold;
        let mut best: Option<(u32, Name, f64)> = None;
        for cid in cities {
            if !g.city_has_district_family(&g.cities[&cid], crate::name!("holy_site"))
                || !Self::culture_faith_city_safe(g, pid, cid, plan)
            {
                continue;
            }
            let Some(Item::Building { building }) = BasicAi::civ_building(g, pid, cid, "shrine")
                .or_else(|| BasicAi::civ_building(g, pid, cid, "temple"))
            else {
                continue;
            };
            let Some(price) = g.building_gold_purchase_cost(pid, cid, &building) else {
                continue;
            };
            if !price.is_finite() || gold + f64::EPSILON < price + floor {
                continue;
            }
            if best.as_ref().is_none_or(|(old_city, _, old_price)| {
                price.total_cmp(old_price).then(cid.cmp(old_city)).is_lt()
            }) {
                best = Some((cid, building, price));
            }
        }
        best
    }

    /// `culture-gold-buys-the-shrines`: buy [`Self::culture_shrine_purchase`].
    /// True when bought. Exact no-op with the gene off.
    pub(super) fn buy_culture_shrine(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        let Some((city, building, price)) = self.culture_shrine_purchase(g, pid, plan) else {
            return false;
        };
        let bank = g.players[pid].gold;
        if g.apply(
            pid,
            &Action::BuyBuilding {
                city,
                building,
                currency: "gold".to_string(),
            },
        )
        .is_err()
        {
            return false;
        }
        think!(self.journal(), Faith, Decision,
            "{} buys its {} with Gold for Culture's Faith", g.cities[&city].name, building;
            "culture-gold-buys-the-shrines: {price:.0} of {bank:.0} Gold; the Holy Site already stands, and each turn it waits is a turn of Faith the Rock Bands lose");
        true
    }
}

#[cfg(test)]
mod tests;
