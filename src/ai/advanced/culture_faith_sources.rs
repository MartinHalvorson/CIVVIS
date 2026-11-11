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
//!
//! `culture-faith-site-per-theater-3` carries versions one and two and lifts
//! their caps: one Holy Site for every Theater city with no cap of six, and
//! sources open until Faith reaches forty a city with no empire ceiling
//! (`CULTURE_FAITH_PER_THEATER_INCOME_3`). It also keeps a Theater city's last
//! free specialty district slot for its Holy Site
//! (`AdvancedAi::culture_faith_hold_the_slots`): live Deity Pericles
//! 20261110T232642Z (12 cities) placed a Diplomatic Quarter in Athens at t66,
//! a Commercial Hub in Eretria at t88 and Industrial Zones in Argos at t91 and
//! Athens at t102, each in the city's only free slot beside a standing
//! Acropolis with no Holy Site; their Holy Sites came 16-68 turns later, at
//! the next population step. Cold War came at t124 with 1,869 Faith banked
//! and the game still needed about 2.5 times its concert Tourism. A finished
//! Holy Site paid 22-35 Faith a turn there. The claim itself keeps every
//! hold and the safety rule, and under version three gives every Theater city
//! its source in one pass. Version three also researches
//! Astrology ahead of the production technologies
//! (`AdvancedAi::culture_faith_astrology_first`): 20261111T000134Z stood three
//! Acropolises at t36 and researched Horseback Riding, Construction and the
//! war's upgrades until Astrology at t66.

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

/// `culture-faith-site-per-theater-3`: Faith a turn per city below which the
/// claim keeps opening sources, with no empire ceiling. Live Deity Pericles
/// 20261110T232642Z crossed version one's 160 at t152 with 10 cities and
/// spent about 9,750 Faith on 15 Rock Bands, yet needed about 2.5 times its
/// concert Tourism to win; a complete Holy Site there paid 22-35 a turn, so
/// forty a city is about one complete site in every city.
const CULTURE_FAITH_PER_THEATER_INCOME_3: f64 = 40.0;

/// `culture-gold-buys-the-shrines`: Gold the Shrine or Temple buy leaves in
/// the treasury on top of the army's upkeep floor
/// (`purchases-keep-the-upkeep-reserve`). Live Deity Online Kristina
/// 2026-11-10 (six games with a Holy Site) held 120-290 Gold and earned +8 to
/// +81 a turn when each Holy Site finished, yet waited 2-38 turns for the
/// Shrine and 5-53 for the Temple (Online prices 140 and 240); a reserve that
/// grew with the empire (25 a city) would have kept most Temples unaffordable.
const CULTURE_SHRINE_GOLD_RESERVE: f64 = 50.0;

/// `culture-gold-buys-the-shrines`: turns of Gold income within which the
/// general purchase pass starts saving for a wanted Shrine or Temple. Live
/// Kristina earned +8 to +81 a turn when its Holy Sites finished, so a
/// 140-Gold Shrine is in reach within a few turns in most games.
const CULTURE_SHRINE_FUND_TURNS: f64 = 6.0;

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
        let (per_city, ceiling) = if self.culture_faith_site_per_theater_3 {
            (CULTURE_FAITH_PER_THEATER_INCOME_3, f64::INFINITY)
        } else if per_theater {
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
        Some(if self.culture_faith_site_per_theater_3 {
            theater_cities.max(1)
        } else if per_theater {
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

    /// `culture-faith-site-per-theater-3`: [`Self::culture_faith_astrology_due`]
    /// ahead of the production technologies, the wonders' and the economy's
    /// research goals: the research slot's own arm sits below them, and
    /// 20261111T000134Z researched Horseback Riding, Construction and five
    /// war upgrades between its three Acropolises at t36 and Astrology at t66
    /// (232642Z: Masonry, Horseback Riding and Construction, t41-t49). Never
    /// ahead of a barbarian, launch, wartime-upgrade or siege goal; the caller
    /// keeps those above it. False with version three off.
    pub(super) fn culture_faith_astrology_first(&self, g: &Game, pid: usize) -> bool {
        self.culture_faith_site_per_theater_3 && self.culture_faith_astrology_due(g, pid)
    }

    /// `culture-faith-site-per-theater`, carried by its versions two and three.
    fn culture_faith_per_theater_on(&self) -> bool {
        self.culture_faith_site_per_theater
            || self.culture_faith_site_per_theater_2
            || self.culture_faith_site_per_theater_3
    }

    /// `culture-faith-site-per-theater-2`, carried by its version three: the
    /// parallel claim, its head pass, its thirty-turn cap and its journal.
    fn culture_faith_parallel_on(&self) -> bool {
        self.culture_faith_site_per_theater_2 || self.culture_faith_site_per_theater_3
    }

    /// The Holy Site the claim would place in `cid`. Versions one and two take
    /// the fifteen-turn first-district pick; version three leaves the turns to
    /// the claim's own cap, so its journal can say a site is too slow rather
    /// than that there is none.
    fn culture_faith_site_item(&self, g: &Game, pid: usize, cid: u32) -> Option<Item> {
        if self.culture_faith_site_per_theater_3 {
            BasicAi::first_district_item_with_budget(g, pid, cid, "holy_site", f64::INFINITY)
        } else {
            BasicAi::first_district_item(g, pid, cid, "holy_site")
        }
    }

    /// Turns `cid` needs to finish `item`: the host's quote, else the cost
    /// left over the city's Production.
    fn culture_faith_source_turns(g: &Game, pid: usize, cid: u32, item: &Item) -> f64 {
        g.host_production_turns(cid, item).unwrap_or_else(|| {
            (g.item_cost_for_city(pid, cid, item) - g.item_invested_production(cid, item)).max(0.0)
                / g.city_yields(cid).production.max(0.5)
        })
    }

    /// The most turns a source may take. `culture-faith-site-per-theater-2`:
    /// a Holy Site priced by the tree's progress takes 10-20 Online turns from
    /// t90, past the twenty-standard-turn cap, so the version-two cap is
    /// thirty.
    fn culture_faith_turn_cap(&self, g: &Game) -> f64 {
        g.standard_duration(if self.culture_faith_parallel_on() {
            30
        } else {
            20
        }) as f64
    }

    /// Specialty district slots `city` has free: its capacity less the
    /// specialty districts standing and the foundations laid
    /// (`Game::city_accepts_new_district_site`'s count).
    fn culture_faith_free_slots(g: &Game, city: &crate::game::City) -> usize {
        let used = city
            .districts
            .keys()
            .filter(|name| g.rules.districts[*name].specialty)
            .count()
            + city
                .owned_tiles
                .iter()
                .filter_map(|pos| g.map.get(*pos)?.district_foundation.as_ref())
                .filter(|foundation| g.rules.districts[foundation.district].specialty)
                .count();
        g.city_specialty_district_capacity(city)
            .saturating_sub(used)
    }

    /// `culture-faith-site-per-theater-3`: whether `cid` keeps its last free
    /// specialty district slot for its Holy Site. A city of ours with a
    /// standing Theater Square (or Acropolis), no Holy Site standing, queued
    /// or laid, and exactly one free slot, where the Holy Site can go: on the
    /// menu within the claim's turn cap once Astrology is known, or on some
    /// plot before. The caller checks that the Faith sources are open. Safety
    /// is the claim's rule, not the slot's: a city under threat or beside an
    /// enemy keeps the slot and claims the Holy Site once the danger passes.
    /// Live 232642Z placed Eretria's Commercial Hub at t88 while the plan
    /// named a threatened home city; its Holy Site waited until t104.
    fn culture_faith_keeps_the_slot(&self, g: &Game, pid: usize, cid: u32) -> bool {
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        if city.owner != pid
            || !g.city_has_district_family(city, crate::name!("theater_square"))
            || Self::culture_faith_site_held(g, cid)
            || Self::culture_faith_free_slots(g, city) != 1
        {
            return false;
        }
        if !g.players[pid].techs.contains(&crate::name!("astrology")) {
            let site = BasicAi::civ_district(g, pid, "holy_site");
            return !g.district_sites(cid, site).is_empty();
        }
        self.culture_faith_site_item(g, pid, cid)
            .is_some_and(|item| {
                let turns = Self::culture_faith_source_turns(g, pid, cid, &item);
                turns.is_finite() && turns <= self.culture_faith_turn_cap(g)
            })
    }

    /// `culture-faith-site-per-theater-3`: refuse every other specialty
    /// district in each city that [keeps its slot](Self::culture_faith_keeps_the_slot)
    /// for the production pass, through the board's own refusals (the district
    /// menu reads `Game::blocked_districts`, `Game::can_produce` reads
    /// `Game::blocked_production`), so every claim, step and governor stands
    /// down at once. The Holy Site, the Theater Square, the Government Plaza,
    /// non-specialty districts (Aqueduct, Neighborhood, Dam, Canal, Spaceport)
    /// and a district whose foundation the city already laid stay open. Returns
    /// the board's refusals for [`Self::culture_faith_release_the_slots`];
    /// `None` with the gene off or nothing kept.
    pub(super) fn culture_faith_hold_the_slots(
        &self,
        g: &mut Game,
        pid: usize,
    ) -> Option<super::culture_theater_first::CultureSiteReservation> {
        if !self.culture_faith_site_per_theater_3 {
            return None;
        }
        self.culture_faith_source_budget(g, pid)?;
        let kept: Vec<u32> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| self.culture_faith_keeps_the_slot(g, pid, *cid))
            .collect();
        if kept.is_empty() {
            return None;
        }
        let held = (
            Arc::clone(&g.blocked_production),
            Arc::clone(&g.blocked_districts),
        );
        let mut production = (*held.0).clone();
        let mut districts = (*held.1).clone();
        for &cid in &kept {
            let city = &g.cities[&cid];
            let laid: Vec<Name> = city
                .owned_tiles
                .iter()
                .filter_map(|pos| g.map.get(*pos)?.district_foundation.as_ref())
                .map(|foundation| g.district_family(foundation.district))
                .collect();
            let mut refused = 0;
            for (district, spec) in &g.rules.districts {
                let family = g.district_family(*district);
                if !spec.specialty
                    || ["holy_site", "theater_square", "government_plaza"]
                        .contains(&family.as_str())
                    || laid.contains(&family)
                {
                    continue;
                }
                districts.entry(cid).or_default().insert(*district);
                production
                    .entry(cid)
                    .or_default()
                    .insert(Game::production_block_key(&Item::District {
                        district: *district,
                        pos: city.pos,
                    }));
                refused += 1;
            }
            think!(self.journal(), Cities, Detail,
                "{} keeps its last district slot for the Holy Site", city.name;
                "culture-faith-site-per-theater-3: {refused} other specialty districts stand down this turn; Theater Square standing, no Holy Site, one slot free");
        }
        g.blocked_districts = Arc::new(districts);
        // Also retires every menu derived before the refusals.
        g.replace_blocked_production(production);
        Some(held)
    }

    /// Restore the board's own refusals after the production pass.
    pub(super) fn culture_faith_release_the_slots(
        g: &mut Game,
        held: Option<super::culture_theater_first::CultureSiteReservation>,
    ) {
        Self::release_culture_acropolis_site(g, held);
    }

    /// `culture-faith-site-per-theater-3`: what held a city with no source, in
    /// the skip journal; versions one and two keep their one line.
    fn culture_faith_no_source_reason(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        sites: usize,
        limit: usize,
    ) -> String {
        let city = &g.cities[&cid];
        if !self.culture_faith_site_per_theater_3 {
            "has no legal source within the budget".into()
        } else if g.city_has_district_family(city, crate::name!("holy_site")) {
            "has its Shrine and Temple".into()
        } else if Self::culture_faith_site_held(g, cid) {
            "has its Holy Site under way".into()
        } else if sites >= limit {
            format!("has no Holy Site within the budget ({sites} of {limit})")
        } else if !g.players[pid].techs.contains(&crate::name!("astrology")) {
            "does not know Astrology".into()
        } else if Self::culture_faith_free_slots(g, city) == 0 {
            "has no free district slot".into()
        } else {
            "has no Holy Site plot on its menu".into()
        }
    }

    /// `culture-faith-site-per-theater-3`: why
    /// [`Self::culture_faith_city_safe`] refused `cid`, for the skip journal.
    fn culture_faith_unsafe_reason(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> String {
        if !self.culture_faith_site_per_theater_3 {
            return "is not safe".into();
        }
        let city = &g.cities[&cid];
        if plan.threatened_city == Some(cid) {
            return "is not safe: the plan's threatened city".into();
        }
        if city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4 {
            return format!("is not safe: attacked on t{}", city.last_attacked);
        }
        let braves = self.culture_faith_braves_the_barbarians_on();
        g.units
            .values()
            .filter(|unit| {
                unit.owner != pid
                    && g.is_at_war(pid, unit.owner)
                    && !(braves
                        && g.players
                            .get(unit.owner)
                            .is_some_and(|owner| owner.is_barbarian))
                    && g.rules.units[&unit.kind].class == "military"
                    && g.wdist(city.pos, unit.pos) <= 6
            })
            .min_by_key(|unit| g.wdist(city.pos, unit.pos))
            .map_or_else(
                || "is not safe".into(),
                |unit| {
                    let owner = &g.players[unit.owner];
                    let who = if owner.is_barbarian {
                        "Barbarian".to_string()
                    } else if owner.is_minor {
                        format!("city-state {}", owner.civ)
                    } else {
                        owner.civ.to_string()
                    };
                    format!(
                        "is not safe: a {who} {} {} tiles away",
                        unit.kind,
                        g.wdist(city.pos, unit.pos)
                    )
                },
            )
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
        let room = 2.0 * g.city_yields(cid).production.max(0.5);
        if remaining.is_nan() || remaining <= room {
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

    /// `culture-faith-braves-the-barbarians`: whether the six-tile soldier
    /// rule skips Barbarian units. Live Deity Kristina 20261110T212836Z, at
    /// peace, had a Barbarian Line Infantry and Musketman 1-5 tiles from
    /// Stockholm and Linköping for 50+ turns; the claim journalled "is not
    /// safe" for both cities from t80 to t130 and Faith stood at 1 a turn
    /// with 4-5 Theater Squares. Over the seven games armed with version two,
    /// "is not safe" was the most common reason a Theater city took no
    /// source (264 city-turns). A city attacked in the last four turns, the
    /// plan's threatened city and a major rival's soldier within six tiles
    /// still hold the claim.
    fn culture_faith_braves_the_barbarians_on(&self) -> bool {
        self.culture_faith_braves_the_barbarians
    }

    fn culture_faith_city_safe(
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        braves_barbarians: bool,
    ) -> bool {
        let city = &g.cities[&cid];
        city.owner == pid
            && plan.threatened_city != Some(cid)
            && (city.last_attacked == 0 || g.turn.saturating_sub(city.last_attacked) > 4)
            && !g.units.values().any(|unit| {
                unit.owner != pid
                    && g.is_at_war(pid, unit.owner)
                    && !(braves_barbarians
                        && g.players
                            .get(unit.owner)
                            .is_some_and(|owner| owner.is_barbarian))
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
        if !Self::culture_faith_city_safe(
            g,
            pid,
            cid,
            plan,
            self.culture_faith_braves_the_barbarians_on(),
        ) || g.cities[&cid].queue.first() != Some(item)
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
            if !Self::culture_faith_city_safe(
                g,
                pid,
                cid,
                plan,
                self.culture_faith_braves_the_barbarians_on(),
            ) {
                skip(cid, self.culture_faith_unsafe_reason(g, pid, cid, plan));
                continue;
            }
            if let Some(head) = city.queue.first() {
                // `culture-faith-site-per-theater-2`: a city already building
                // a source keeps it; the next claim goes to another city.
                if self.culture_faith_parallel_on() && Self::culture_faith_item(g, head) {
                    skip(cid, "is building its source".into());
                    continue;
                }
                let passes = self.culture_faith_parallel_on()
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
                self.culture_faith_site_item(g, pid, cid)
            } else {
                None
            };
            let Some(item) = item else {
                skip(
                    cid,
                    self.culture_faith_no_source_reason(g, pid, cid, sites, limit),
                );
                continue;
            };
            let quote = g.host_production_turns(cid, &item);
            if held.is_some() && quote.is_none() && !self.culture_faith_parallel_on() {
                continue;
            }
            let turns = Self::culture_faith_source_turns(g, pid, cid, &item);
            let cap = self.culture_faith_turn_cap(g);
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
        if !self.culture_faith_site_per_theater_3 {
            self.claim_one_culture_faith_source(g, pid, plan);
            return;
        }
        // `culture-faith-site-per-theater-3`: every Theater city its source in
        // one pass, one item each. Native replays of 232642Z t91, 173750Z t88
        // and t106 and 000134Z t89 kept a city's slot while the one claim of
        // the frame went to another city's quicker Shrine or Holy Site, and the
        // kept city queued a Spy, a Caravel, a Skirmisher or a Library, which
        // then held the claim as its head.
        for _ in 0..g.player_city_ids(pid).len() {
            if !self.claim_one_culture_faith_source(g, pid, plan) {
                break;
            }
        }
    }

    /// One claim of [`Self::claim_culture_faith_source`]: true when it put a
    /// new source at the head of a queue.
    fn claim_one_culture_faith_source(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        let mut skips = Vec::new();
        let target = self.culture_faith_source_scan(
            g,
            pid,
            plan,
            self.culture_faith_parallel_on().then_some(&mut skips),
        );
        let version = if self.culture_faith_site_per_theater_3 {
            "culture-faith-site-per-theater-3"
        } else {
            "culture-faith-site-per-theater-2"
        };
        let Some((city, item, turns)) = target else {
            if !skips.is_empty() {
                think!(self.journal(), Cities, Detail,
                    "No new Faith source this turn";
                    "{version}: {}", skips.join("; "));
            }
            return false;
        };
        if g.cities[&city].queue.first() == Some(&item) {
            return false;
        }
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .is_err()
        {
            return false;
        }
        let pace = if self.culture_faith_parallel_on() {
            "one source per Theater city at a time"
        } else {
            "one safe source at a time"
        };
        think!(self.journal(), Cities, Decision,
            "{} claims {} for Culture's Faith economy", g.cities[&city].name, Self::plain_item(&item);
            "{pace}, {turns:.1} production turns; Faith supports Naturalists and Rock Bands after the Prophet race");
        true
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
        let floor = self.purchase_upkeep_floor(g, pid, 0.0) + CULTURE_SHRINE_GOLD_RESERVE;
        let gold = g.players[pid].gold;
        let mut best: Option<(u32, Name, f64)> = None;
        for (cid, building) in
            Self::culture_shrine_wants(g, pid, plan, self.culture_faith_braves_the_barbarians_on())
        {
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

    /// `culture-gold-buys-the-shrines`: each safe city of ours with a standing
    /// Holy Site and its next Faith building, the Shrine or, once the Shrine
    /// stands, the Temple.
    fn culture_shrine_wants(
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        braves_barbarians: bool,
    ) -> Vec<(u32, Name)> {
        g.player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                g.city_has_district_family(&g.cities[cid], crate::name!("holy_site"))
                    && Self::culture_faith_city_safe(g, pid, *cid, plan, braves_barbarians)
            })
            .filter_map(|cid| {
                match BasicAi::civ_building(g, pid, cid, "shrine")
                    .or_else(|| BasicAi::civ_building(g, pid, cid, "temple"))
                {
                    Some(Item::Building { building }) => Some((cid, building)),
                    _ => None,
                }
            })
            .collect()
    }

    /// `culture-gold-buys-the-shrines`: the treasury the general purchase pass
    /// (`advanced_gold_spending`) leaves behind while a Shrine or Temple is
    /// wanted: the cheapest one's price plus the buy's own floor, once that
    /// price is within [`CULTURE_SHRINE_FUND_TURNS`] turns of income. The
    /// host lists a Shrine for sale only on frames where the city is producing
    /// something, and the general pass ran first on the others: live
    /// civvis-20261110T195741Z, the first armed game, finished Göteborg's
    /// Holy Site at t115 with 251 Gold, the general pass spent 120 that frame
    /// and again at t117 and t119, and the Shrine and Temple were built, not
    /// bought (t120, t124). Unlisted, the price is four times the production
    /// cost (`building_purchase_cost`'s Gold rate). 0.0 with the gene off or
    /// while nothing is wanted.
    pub(super) fn culture_shrine_gold_fund(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> f64 {
        let income = g.players[pid].gold_per_turn;
        if !self.culture_gold_buys_the_shrines
            || income < 0.0
            || self.culture_faith_source_budget(g, pid).is_none()
        {
            return 0.0;
        }
        let cheapest =
            Self::culture_shrine_wants(g, pid, plan, self.culture_faith_braves_the_barbarians_on())
                .into_iter()
                .filter_map(|(cid, building)| {
                    g.building_gold_purchase_cost(pid, cid, &building)
                        .or_else(|| {
                            let item = Item::Building { building };
                            Some(4.0 * g.item_cost_for_city(pid, cid, &item))
                        })
                        .filter(|price| price.is_finite() && *price > 0.0)
                })
                .min_by(f64::total_cmp);
        let Some(price) = cheapest else {
            return 0.0;
        };
        let fund = price + self.purchase_upkeep_floor(g, pid, 0.0) + CULTURE_SHRINE_GOLD_RESERVE;
        if fund > g.players[pid].gold + income * CULTURE_SHRINE_FUND_TURNS {
            return 0.0;
        }
        fund
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
