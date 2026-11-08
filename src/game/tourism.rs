//! Tourism and Great Works: where a civilization's tourism comes from, the
//! tourists it earns, and the works that produce it.
//!
//! Tourism per turn and its components and sources (`tourism_per_turn`,
//! `tourism_components_per_turn_with_sources`, religious tourism,
//! `tourism_by_tile`), the multipliers and pressure against a rival (open
//! borders, trade routes, government intolerance), domestic, foreign and
//! visiting tourists, and `check_culture_victory`. Great Works:
//! slots, housed works and pieces, granting a work, local multipliers and
//! theming. `legacy_tourism_pressure` and `add_passive_tourism` stay in
//! `game.rs` for now because an open PR edits them.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    pub(super) fn starting_major_count(&self) -> usize {
        self.players
            .iter()
            .filter(|player| !player.is_minor && !player.is_barbarian)
            .count()
    }

    pub(super) fn visiting_tourists_from(&self, source: usize, target: usize) -> i64 {
        if source == target
            || self.same_team(source, target)
            || !self.players[source].alive
            || !self.players[target].alive
            || self.players[source].is_minor
            || self.players[source].is_barbarian
            || self.players[target].is_minor
            || self.players[target].is_barbarian
        {
            return 0;
        }
        // The host's own per-pair count where the mirror recorded one
        // (`GetTouristsFrom`), exactly as `foreign_tourists` prefers the
        // observed aggregate; a native game has no entries.
        if let Some(observed) = self.observed_visiting_tourists.get(&(source, target)) {
            return (*observed).max(0);
        }
        let divisor = self.starting_major_count() as f64 * TOURISM_PER_VISITOR;
        if divisor <= 0.0 {
            return 0;
        }
        (self.tourism_pressure_against(source, target) / divisor).floor() as i64
    }

    pub fn domestic_tourists(&self, pid: usize) -> i64 {
        // See `foreign_tourists`: the host's own counter wins when observed.
        if let Some(observed) = self
            .observed_public_empire_stats
            .get(&pid)
            .and_then(|stats| stats.domestic_tourists)
        {
            return observed as i64;
        }
        let generated = (self.players[pid].culture_lifetime / 100.0).floor() as i64;
        let visiting_elsewhere = self
            .players
            .iter()
            .filter(|source| source.id != pid)
            .map(|source| self.visiting_tourists_from(source.id, pid))
            .sum::<i64>();
        // A civilization's tourists abroad come out of its own pool, which
        // cannot go below empty: the host's counter never reads negative (0
        // of 174 live readings, 2026-10-31), and a negative native pool let
        // any rival clear the Culture Victory bar against it.
        generated.saturating_sub(visiting_elsewhere).max(0)
    }

    fn government_tourism_intolerance(&self, pid: usize) -> f64 {
        match self.players[pid].government.as_deref() {
            Some(
                "communism"
                | "democracy"
                | "fascism"
                | "corporate_libertarianism"
                | "digital_democracy"
                | "synthetic_technocracy",
            ) => 20.0,
            _ => 0.0,
        }
    }

    pub(super) fn tourism_open_borders(&self, source: usize, target: usize) -> bool {
        self.are_allied(source, target)
            || self.players[target]
                .open_borders_until
                .get(&source)
                .is_some_and(|until| *until > self.turn)
            || self
                .active_trade_deals
                .iter()
                .filter(|deal| deal.ends > self.turn)
                .any(|deal| {
                    (deal.from == target && deal.to == source && deal.offer.open_borders)
                        || (deal.to == target && deal.from == source && deal.request.open_borders)
                })
    }

    pub(crate) fn has_tourism_trade_route(&self, source: usize, target: usize) -> bool {
        self.routes.iter().any(|route| {
            route.owner == source
                && route.ends > self.turn
                && self
                    .cities
                    .get(&route.dest)
                    .is_some_and(|city| city.owner == target)
        })
    }

    /// Gathering Storm's international modifiers are summed and then applied
    /// independently to each civilization. Religious penalties affect only
    /// Holy City and Relic Tourism; Cristo cancels Enlightenment, not the
    /// separate different-religion penalty.
    pub(crate) fn international_tourism_multiplier(
        &self,
        source: usize,
        target: usize,
        religious: bool,
    ) -> f64 {
        let mut percent = self.policy_effect(target, "incoming_tourism_pct");
        if self.tourism_open_borders(source, target) {
            percent += 25.0;
        }
        if self.has_tourism_trade_route(source, target) {
            percent += 25.0 + self.policy_effect(source, "trade_partner_tourism_pct");
        }
        if self.players[source].government != self.players[target].government {
            percent -= self.government_tourism_intolerance(source)
                + self.government_tourism_intolerance(target);
        }
        if religious {
            if self.players[source].religion.is_some()
                && self.players[target].religion.is_some()
                && self.players[source].religion != self.players[target].religion
            {
                percent -= 50.0;
            }
            let cristo = self.empire_wonder_effect(source, "religious_tourism_unreduced") > 0.0;
            if !cristo {
                percent += self.tree_effect(target, "incoming_religious_tourism_pct");
            }
        }
        (1.0 + percent / 100.0).max(0.0)
    }

    pub fn tourism_pressure_against(&self, source: usize, target: usize) -> f64 {
        self.players[source]
            .tourism_pressure
            .get(&target)
            .copied()
            .unwrap_or_else(|| self.legacy_tourism_pressure(source, target))
    }

    pub fn foreign_tourists(&self, pid: usize) -> i64 {
        // A live mirror cannot reconstruct tourism from culture history, but
        // the host's World Rankings screen publishes both tourist counters
        // for every major and the mirror records them. Prefer the observation
        // exactly as `score` prefers `observed_score`; a native game has no
        // observed entries and keeps its own arithmetic.
        if let Some(observed) = self
            .observed_public_empire_stats
            .get(&pid)
            .and_then(|stats| stats.foreign_tourists)
        {
            return observed as i64;
        }
        if self.starting_major_count() == 0 || !self.players[pid].alive {
            return 0;
        }
        // Each rival owns a separate integer tourist pool. Fractions do not
        // combine across civilizations, and eliminating a civilization
        // removes both its contributed visitors and future Tourism market.
        self.players
            .iter()
            .filter(|rival| rival.id != pid && !self.same_team(pid, rival.id))
            .map(|rival| self.visiting_tourists_from(pid, rival.id))
            .sum()
    }

    fn great_work_local_multiplier(&self, city: &City, kind: &str) -> f64 {
        let mut multiplier = if self.world_era >= 5 {
            1.0 + self.city_building_effect(city, "modern_civ_tourism_pct") / 100.0
        } else {
            1.0
        };
        if kind == "relic" {
            // Reliquaries triples a Relic's Tourism as it triples its Faith.
            multiplier *= 1.0 + self.city_religion_belief_effect(city, "relic_tourism_pct") / 100.0;
        }
        if kind == "relic"
            && city
                .wonders
                .contains_key(&crate::name!("st_basils_cathedral"))
        {
            multiplier *= 1.0
                + self.rules.wonders["st_basils_cathedral"]
                    .effects
                    .get("city_religious_tourism_pct")
                    .copied()
                    .unwrap_or(0.0)
                    / 100.0;
        }
        multiplier
    }

    pub(super) fn great_work_slots(&self, pid: usize) -> Vec<(u32, String)> {
        if let Some(memo) = self.query_memo.gw_slots.borrow().as_ref() {
            if let Some(slots) = memo.get(&pid) {
                return slots.clone();
            }
        }
        let slots = self.great_work_slots_uncached(pid);
        if let Some(memo) = self.query_memo.gw_slots.borrow_mut().as_mut() {
            memo.insert(pid, slots.clone());
        }
        slots
    }

    fn great_work_slots_uncached(&self, pid: usize) -> Vec<(u32, String)> {
        let mut slots = Vec::new();
        for city in self.cities.values().filter(|city| city.owner == pid) {
            if self.city_has_palace(city) {
                for (kind, count) in &self.rules.buildings["palace"].great_work_slots {
                    slots.extend(std::iter::repeat_n(
                        (city.id, kind.clone()),
                        (*count).max(0) as usize,
                    ));
                }
            }
            for building in city.buildings.iter().filter(|building| {
                !city.pillaged_buildings.contains(*building)
                    && self.building_district_is_active(city, building)
            }) {
                for (kind, count) in &self.rules.buildings[building].great_work_slots {
                    slots.extend(std::iter::repeat_n(
                        (city.id, kind.clone()),
                        (*count).max(0) as usize,
                    ));
                }
            }
            for wonder in city.wonders.keys() {
                for (kind, count) in &self.rules.wonders[wonder].great_work_slots {
                    slots.extend(std::iter::repeat_n(
                        (city.id, kind.clone()),
                        (*count).max(0) as usize,
                    ));
                }
            }
        }
        slots
    }

    pub(super) fn housed_great_works_with_extra(
        &self,
        pid: usize,
        extra: Option<(&str, usize)>,
    ) -> BTreeMap<u32, BTreeMap<String, usize>> {
        let slots = self.great_work_slots(pid);
        let mut housed = BTreeMap::<u32, BTreeMap<String, usize>>::new();
        let mut available = BTreeMap::<String, usize>::new();
        for (kind, counter) in GREAT_WORK_COUNTERS {
            let count = self.players[pid]
                .counters
                .get(counter)
                .copied()
                .unwrap_or(0)
                .max(0) as usize;
            available.insert(kind.to_string(), count);
        }
        let named_total = available.values().sum::<usize>();
        if named_total == 0 {
            // Pre-named-Great-Person saves represented each generic Artist as
            // three works that matched the slots available at the time.
            let legacy = self.players[pid]
                .gp_claimed
                .get("artist")
                .copied()
                .unwrap_or(0)
                .max(0) as usize
                * 3;
            if legacy > 0 {
                let mut ranked_slots: Vec<(usize, &(u32, String))> = slots
                    .iter()
                    .enumerate()
                    // Legacy generic Artists predate the universal Palace
                    // slot model and filled typed cultural-building slots.
                    .filter(|(_, (_, kind))| kind != "any")
                    .collect();
                ranked_slots.sort_by(
                    |(left_index, (left_city, left_kind)),
                     (right_index, (right_city, right_kind))| {
                        let left_value = self.great_work_tourism(pid, left_kind)
                            * self.great_work_local_multiplier(&self.cities[left_city], left_kind);
                        let right_value = self.great_work_tourism(pid, right_kind)
                            * self
                                .great_work_local_multiplier(&self.cities[right_city], right_kind);
                        right_value
                            .partial_cmp(&left_value)
                            .unwrap_or(std::cmp::Ordering::Equal)
                            .then_with(|| left_city.cmp(right_city))
                            .then_with(|| left_index.cmp(right_index))
                    },
                );
                let mut occupied: BTreeSet<usize> = ranked_slots
                    .iter()
                    .take(legacy)
                    .map(|(index, _)| *index)
                    .collect();
                for (_, (city, slot_kind)) in ranked_slots.into_iter().take(legacy) {
                    *housed
                        .entry(*city)
                        .or_default()
                        .entry(slot_kind.clone())
                        .or_insert(0) += 1;
                }
                if let Some((work, count)) = extra {
                    let compatible = |slot: &str| {
                        work == slot
                            || (matches!(work, "art" | "religious_art")
                                && matches!(slot, "art" | "religious_art"))
                    };
                    for _ in 0..count {
                        let destination = slots
                            .iter()
                            .enumerate()
                            .find(|(index, (_, slot))| {
                                !occupied.contains(index) && compatible(slot)
                            })
                            .or_else(|| {
                                slots.iter().enumerate().find(|(index, (_, slot))| {
                                    !occupied.contains(index) && slot == "any"
                                })
                            });
                        let Some((index, (city, _))) = destination else {
                            break;
                        };
                        occupied.insert(index);
                        *housed
                            .entry(*city)
                            .or_default()
                            .entry(work.to_string())
                            .or_insert(0) += 1;
                    }
                }
                return housed;
            }
        }
        if let Some((kind, count)) = extra {
            *available.entry(kind.to_string()).or_insert(0) += count;
        }

        let mut used = BTreeSet::<usize>::new();
        let compatible = |work: &str, slot: &str| {
            work == slot
                || (matches!(work, "art" | "religious_art")
                    && matches!(slot, "art" | "religious_art"))
        };
        for work in [
            "relic",
            "music",
            "religious_art",
            "art",
            "artifact",
            "writing",
        ] {
            let mut candidates: Vec<usize> = slots
                .iter()
                .enumerate()
                .filter(|(index, (_, slot))| !used.contains(index) && compatible(work, slot))
                .map(|(index, _)| index)
                .collect();
            candidates.sort_by(|left, right| {
                let left_city = &self.cities[&slots[*left].0];
                let right_city = &self.cities[&slots[*right].0];
                self.great_work_local_multiplier(right_city, work)
                    .partial_cmp(&self.great_work_local_multiplier(left_city, work))
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| slots[*left].0.cmp(&slots[*right].0))
                    .then_with(|| left.cmp(right))
            });
            let count = available.get(work).copied().unwrap_or(0);
            for index in candidates.into_iter().take(count) {
                used.insert(index);
                *housed
                    .entry(slots[index].0)
                    .or_default()
                    .entry(work.to_string())
                    .or_insert(0) += 1;
                *available.get_mut(work).unwrap() -= 1;
            }
        }

        let mut any_slots: Vec<usize> = slots
            .iter()
            .enumerate()
            .filter(|(index, (_, slot))| !used.contains(index) && slot == "any")
            .map(|(index, _)| index)
            .collect();
        any_slots.sort_by_key(|index| (slots[*index].0, *index));
        for index in any_slots {
            let city = &self.cities[&slots[index].0];
            let best = available
                .iter()
                .filter(|(_, count)| **count > 0)
                .max_by(|(left, _), (right, _)| {
                    let left_value = self.great_work_tourism(pid, left)
                        * self.great_work_local_multiplier(city, left);
                    let right_value = self.great_work_tourism(pid, right)
                        * self.great_work_local_multiplier(city, right);
                    left_value
                        .partial_cmp(&right_value)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| right.cmp(left))
                })
                .map(|(kind, _)| kind.clone());
            let Some(kind) = best else { break };
            *available.get_mut(&kind).unwrap() -= 1;
            *housed.entry(city.id).or_default().entry(kind).or_insert(0) += 1;
        }
        housed
    }

    /// Housing Great Works assigns them across every city the player owns, and
    /// `city_yields` runs the whole assignment to read one city's entry.
    /// Share the ordered allocation within a query instead of copying it for
    /// each city. An escaped snapshot stays immutable after the query ends.
    pub(crate) fn housed_great_works(&self, pid: usize) -> Arc<GreatWorksByCity> {
        if let Some(memo) = self.query_memo.housed_works.borrow().as_ref() {
            if let Some(value) = memo.get(&pid) {
                return Arc::clone(value);
            }
        }
        let allocation = self.housed_great_works_uncached(pid);
        let value = if allocation.is_empty() {
            static EMPTY: std::sync::OnceLock<Arc<GreatWorksByCity>> = std::sync::OnceLock::new();
            Arc::clone(EMPTY.get_or_init(|| Arc::new(BTreeMap::new())))
        } else {
            Arc::new(allocation)
        };
        if let Some(memo) = self.query_memo.housed_works.borrow_mut().as_mut() {
            memo.insert(pid, Arc::clone(&value));
        }
        value
    }

    pub(super) fn housed_great_works_uncached(
        &self,
        pid: usize,
    ) -> BTreeMap<u32, BTreeMap<String, usize>> {
        // The host's placement, where the mirror has read it (seat 0 only:
        // the export names our own cities' works).
        if pid == 0 {
            if let Some(observed) = &self.observed_great_work_housing {
                return observed
                    .iter()
                    .filter(|(city, _)| self.cities.get(*city).is_some_and(|c| c.owner == pid))
                    .map(|(city, kinds)| (*city, kinds.clone()))
                    .collect();
            }
        }
        let player = &self.players[pid];
        if player.gp_claimed.get("artist").copied().unwrap_or(0) <= 0
            && GREAT_WORK_COUNTERS
                .iter()
                .all(|(_, counter)| player.counters.get(*counter).copied().unwrap_or(0) <= 0)
        {
            // With no named or legacy works, slots cannot change the answer.
            return BTreeMap::new();
        }
        self.housed_great_works_with_extra(pid, None)
    }

    /// Tourism generated specifically by Relics and Holy Cities. Keeping this
    /// source separate is necessary because Enlightenment reduces only these
    /// sources and Cristo Redentor cancels exactly that reduction.
    pub(super) fn religious_tourism_components_per_turn(&self, pid: usize) -> (f64, f64) {
        let global_multiplier = 1.0
            + (self.tree_effect(pid, "tourism_pct")
                + self.monopoly_bonuses(pid).1
                + self.gov_effects(pid).tourism_pct)
                / 100.0;
        let mut holy_city_tourism = 0.0;
        let mut film_studio_bonus = 0.0;
        for city_id in self
            .players
            .iter()
            .filter_map(|founder| founder.holy_city)
            .filter(|city| self.cities.get(city).is_some_and(|city| city.owner == pid))
        {
            holy_city_tourism += 8.0;
            film_studio_bonus += 8.0
                * self.city_building_effect(&self.cities[&city_id], "modern_civ_tourism_pct")
                / 100.0;
        }

        let mut relic_tourism = 0.0;
        for (city_id, works) in self.housed_great_works(pid).iter() {
            let city = &self.cities[city_id];
            let mut value = works.get("relic").copied().unwrap_or(0) as f64
                * self.great_work_tourism(pid, "relic");
            if city
                .wonders
                .contains_key(&crate::name!("st_basils_cathedral"))
            {
                value *= 1.0
                    + self.rules.wonders["st_basils_cathedral"]
                        .effects
                        .get("city_religious_tourism_pct")
                        .copied()
                        .unwrap_or(0.0)
                        / 100.0;
            }
            relic_tourism += value;
            film_studio_bonus +=
                value * self.city_building_effect(city, "modern_civ_tourism_pct") / 100.0;
        }
        (
            (holy_city_tourism + relic_tourism) * global_multiplier,
            film_studio_bonus * global_multiplier,
        )
    }

    pub fn religious_tourism_per_turn(&self, pid: usize) -> f64 {
        self.religious_tourism_components_per_turn(pid).0
    }

    /// Current tourism generated by this civilization each turn.
    ///
    /// Named Great Works occupy compatible active building/wonder slots;
    /// legacy generic-Artist saves retain their former three-work fallback.
    fn add_tourism_source(
        total: &mut f64,
        by_tile: &mut Option<&mut BTreeMap<Pos, f64>>,
        position: Pos,
        amount: f64,
    ) {
        *total += amount;
        Self::record_tourism_source(by_tile, position, amount);
    }

    fn record_tourism_source(
        by_tile: &mut Option<&mut BTreeMap<Pos, f64>>,
        position: Pos,
        amount: f64,
    ) {
        if amount != 0.0 {
            if let Some(sources) = by_tile.as_deref_mut() {
                *sources.entry(position).or_default() += amount;
            }
        }
    }

    fn tourism_components_per_turn_with_sources(
        &self,
        pid: usize,
        mut by_tile: Option<&mut BTreeMap<Pos, f64>>,
    ) -> (f64, f64) {
        // A sweep of every city the empire owns, each of which is asked for
        // its full yields to read one figure out of them.
        let _memo = self.query_memo();
        let mut tourism = 0.0;
        let mut film_studio_bonus = 0.0;
        let housed_works = self.housed_great_works(pid);
        let housed_pieces = self.housed_great_work_pieces(pid);
        for city in self.cities.values().filter(|city| city.owner == pid) {
            let city_tourism_start = tourism;
            for position in city.wonders.values() {
                Self::add_tourism_source(&mut tourism, &mut by_tile, *position, 2.0);
            }
            Self::add_tourism_source(
                &mut tourism,
                &mut by_tile,
                city.pos,
                city.products.len().min(self.product_capacity(city)) as f64,
            );
            let (_, _, theming_tourism) = self.city_theming(
                pid,
                city.id,
                housed_pieces
                    .get(&city.id)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
            );
            Self::add_tourism_source(&mut tourism, &mut by_tile, city.pos, theming_tourism);
            for (kind, count) in housed_works.get(&city.id).into_iter().flatten() {
                let mut value = self.great_work_tourism(pid, kind) * *count as f64;
                if matches!(
                    kind.as_str(),
                    "writing" | "art" | "religious_art" | "music" | "any"
                ) {
                    value *=
                        1.0 + self.governor_effect(pid, city.id, "great_work_tourism_pct") / 100.0;
                }
                if kind == "relic" {
                    value *=
                        1.0 + self.city_religion_belief_effect(city, "relic_tourism_pct") / 100.0;
                }
                if kind == "relic"
                    && city
                        .wonders
                        .contains_key(&crate::name!("st_basils_cathedral"))
                {
                    value *= 1.0
                        + self.rules.wonders["st_basils_cathedral"]
                            .effects
                            .get("city_religious_tourism_pct")
                            .copied()
                            .unwrap_or(0.0)
                            / 100.0;
                }
                Self::add_tourism_source(&mut tourism, &mut by_tile, city.pos, value);
            }
            if self.tree_effect(pid, "improvement_culture_tourism") > 0.0 {
                for (district, position) in &city.districts {
                    if !self.district_is_active(city, district, *position) {
                        continue;
                    }
                    let multiplier = self.rules.districts[district]
                        .effects
                        .get("tourism_after_flight")
                        .copied()
                        .unwrap_or(0.0);
                    if multiplier > 0.0 {
                        Self::add_tourism_source(
                            &mut tourism,
                            &mut by_tile,
                            *position,
                            self.district_yields(district, *position).culture * multiplier,
                        );
                    }
                }
            }
            for (wonder, position) in &city.wonders {
                let spec = &self.rules.wonders[wonder];
                Self::add_tourism_source(
                    &mut tourism,
                    &mut by_tile,
                    *position,
                    spec.effects.get("tourism").copied().unwrap_or(0.0),
                );
            }
            for building in city.buildings.iter().filter(|building| {
                !city.pillaged_buildings.contains(*building)
                    && self.building_district_is_active(city, building)
            }) {
                let spec = &self.rules.buildings[building];
                let mut building_tourism = spec.effects.get("tourism").copied().unwrap_or(0.0);
                if self.tree_effect(pid, "wall_tourism") > 0.0 {
                    building_tourism += spec
                        .effects
                        .get("tourism_after_conservation")
                        .copied()
                        .unwrap_or(0.0);
                    building_tourism += spec
                        .effects
                        .get("faith_tourism_after_conservation")
                        .copied()
                        .unwrap_or(0.0)
                        * spec.yields.faith;
                }
                if self.players[pid].techs.contains(&crate::name!("flight")) {
                    let per_feature = spec
                        .effects
                        .get("feature_tourism_after_flight")
                        .copied()
                        .unwrap_or(0.0);
                    if per_feature != 0.0 {
                        for position in city.owned_tiles.iter().filter(|position| {
                            let tile = &self.map.tiles[position];
                            tile.feature.is_some() && self.rules.is_passable(tile)
                        }) {
                            Self::add_tourism_source(
                                &mut tourism,
                                &mut by_tile,
                                *position,
                                per_feature,
                            );
                        }
                    }
                    building_tourism += if city.pop >= 20 {
                        spec.effects
                            .get("population_20_tourism_after_flight")
                            .copied()
                            .unwrap_or(0.0)
                    } else if city.pop >= 10 {
                        spec.effects
                            .get("population_10_tourism_after_flight")
                            .copied()
                            .unwrap_or(0.0)
                    } else {
                        0.0
                    };
                }
                building_tourism += if city.pop >= 20 {
                    spec.effects
                        .get("population_20_tourism")
                        .copied()
                        .unwrap_or(0.0)
                } else if city.pop >= 10 {
                    spec.effects
                        .get("population_10_tourism")
                        .copied()
                        .unwrap_or(0.0)
                } else {
                    0.0
                };
                let coastal_wonder_tourism = spec
                    .effects
                    .get("coastal_wonder_tourism")
                    .copied()
                    .unwrap_or(0.0);
                if coastal_wonder_tourism != 0.0 {
                    building_tourism += coastal_wonder_tourism
                        * city
                            .wonders
                            .values()
                            .filter(|position| {
                                let tile = &self.map.tiles[position];
                                matches!(tile.terrain.as_str(), "coast" | "ocean")
                                    || self.nbrs(**position).iter().any(|neighbor| {
                                        matches!(
                                            self.map.tiles[neighbor].terrain.as_str(),
                                            "coast" | "ocean"
                                        )
                                    })
                            })
                            .count() as f64;
                }
                let geothermal = spec
                    .effects
                    .get("geothermal_fissure_tourism")
                    .copied()
                    .unwrap_or(0.0);
                if geothermal != 0.0 {
                    for position in city.owned_tiles.iter().filter(|position| {
                        self.map.tiles[position].feature.as_deref() == Some("geothermal_fissure")
                    }) {
                        Self::add_tourism_source(&mut tourism, &mut by_tile, *position, geothermal);
                    }
                }
                if matches!(self.players[pid].age.as_str(), "golden" | "heroic") {
                    building_tourism *= 1.0
                        + spec
                            .effects
                            .get("golden_age_faith_tourism_pct")
                            .copied()
                            .unwrap_or(0.0)
                            / 100.0;
                }
                Self::add_tourism_source(&mut tourism, &mut by_tile, city.pos, building_tourism);
            }
            for pos in &city.owned_tiles {
                let tile = &self.map.tiles[pos];
                let Some(improvement) = tile.improvement.as_deref() else {
                    continue;
                };
                if tile.pillaged {
                    continue;
                }
                let effects = &self.rules.improvements[improvement].effects;
                let appeal = self.tile_appeal(*pos);
                // National Park Tourism follows its tiles' live Appeal even
                // after later surroundings drag that value below zero. Other
                // appeal-based improvements are only legal at positive Appeal.
                let tourism_appeal = if improvement == "national_park" {
                    appeal
                } else {
                    appeal.max(0)
                };
                let mut improvement_tourism = effects.get("tourism").copied().unwrap_or(0.0)
                    + effects.get("appeal_tourism").copied().unwrap_or(0.0) * tourism_appeal as f64;
                if self.players[pid].techs.contains(&crate::name!("flight")) {
                    improvement_tourism +=
                        effects.get("tourism_after_flight").copied().unwrap_or(0.0);
                    let faith_factor = effects
                        .get("faith_tourism_after_flight")
                        .copied()
                        .unwrap_or(0.0);
                    if faith_factor != 0.0 {
                        let improved = self.player_tile_yields(pid, *pos, tile);
                        let mut bare = tile.clone();
                        bare.improvement = None;
                        let unimproved = self.player_tile_yields(pid, *pos, &bare);
                        improvement_tourism +=
                            (improved.faith - unimproved.faith).max(0.0) * faith_factor;
                    }
                }
                if improvement == "seastead" {
                    improvement_tourism += self
                        .nbrs(*pos)
                        .iter()
                        .filter(|neighbor| {
                            self.map.tiles[neighbor].feature.as_deref() == Some("reef")
                        })
                        .count() as f64
                        * effects.get("adjacent_reef_tourism").copied().unwrap_or(0.0);
                }
                if improvement == "seaside_resort" {
                    improvement_tourism *=
                        1.0 + self.empire_wonder_effect(pid, "seaside_resort_tourism_pct") / 100.0;
                }
                if city
                    .wonders
                    .contains_key(&crate::name!("golden_gate_bridge"))
                    && (improvement_tourism > 0.0 || improvement == "national_park")
                {
                    improvement_tourism *= 1.0
                        + self.rules.wonders["golden_gate_bridge"]
                            .effects
                            .get("city_improvement_park_tourism_pct")
                            .copied()
                            .unwrap_or(0.0)
                            / 100.0;
                }
                Self::add_tourism_source(&mut tourism, &mut by_tile, *pos, improvement_tourism);
                let mut renewable_power = self.rules.improvements[improvement]
                    .effects
                    .get("power")
                    .copied()
                    .unwrap_or(0.0);
                if matches!(
                    improvement,
                    "wind_farm" | "solar_farm" | "offshore_wind_farm" | "geothermal_plant"
                ) {
                    renewable_power += self.governor_effect(pid, city.id, "renewable_power_bonus");
                }
                Self::add_tourism_source(
                    &mut tourism,
                    &mut by_tile,
                    *pos,
                    renewable_power
                        * (1.0 + self.empire_wonder_effect(pid, "renewable_power_pct") / 100.0)
                        * self.empire_wonder_effect(pid, "renewable_power_tourism"),
                );
                if self.tree_effect(pid, "improvement_culture_tourism") > 0.0 {
                    let improved = self.player_tile_yields(pid, *pos, &self.map.tiles[pos]);
                    let mut bare = self.map.tiles[pos].clone();
                    bare.improvement = None;
                    let unimproved = self.player_tile_yields(pid, *pos, &bare);
                    Self::add_tourism_source(
                        &mut tourism,
                        &mut by_tile,
                        *pos,
                        (improved.culture - unimproved.culture).max(0.0),
                    );
                }
            }
            let mut building_renewable_power = city
                .buildings
                .iter()
                .filter(|building| {
                    !city.pillaged_buildings.contains(*building)
                        && self.building_district_is_active(city, building)
                })
                .map(|building| {
                    self.rules.buildings[building]
                        .effects
                        .get("renewable_power_generated")
                        .copied()
                        .unwrap_or(0.0)
                })
                .sum::<f64>();
            if self.city_has_active_building_family(city, crate::name!("hydroelectric_dam")) {
                building_renewable_power +=
                    self.governor_effect(pid, city.id, "renewable_power_bonus");
            }
            Self::add_tourism_source(
                &mut tourism,
                &mut by_tile,
                city.pos,
                building_renewable_power
                    * (1.0 + self.empire_wonder_effect(pid, "renewable_power_pct") / 100.0)
                    * self.empire_wonder_effect(pid, "renewable_power_tourism"),
            );
            film_studio_bonus += (tourism - city_tourism_start)
                * self.city_building_effect(city, "modern_civ_tourism_pct")
                / 100.0;
        }

        // Preserve the total's original arithmetic order — a few gameplay
        // callers compare exact floating-point deltas — while allocating its
        // Culture component to the cities that produced it for the map ledger.
        let culture = self
            .player_city_ids(pid)
            .into_iter()
            .map(|city_id| self.city_yields(city_id).culture)
            .sum::<f64>();
        for city in self.cities.values().filter(|city| city.owner == pid) {
            Self::record_tourism_source(
                &mut by_tile,
                city.pos,
                0.15 * self.city_yields(city.id).culture,
            );
        }
        let mut holy_city_tourism = 0.0;
        for city_id in self
            .players
            .iter()
            .filter_map(|founder| founder.holy_city)
            .filter(|city| self.cities.get(city).is_some_and(|city| city.owner == pid))
        {
            holy_city_tourism += 8.0;
            Self::record_tourism_source(&mut by_tile, self.cities[&city_id].pos, 8.0);
            film_studio_bonus += 8.0
                * self.city_building_effect(&self.cities[&city_id], "modern_civ_tourism_pct")
                / 100.0;
        }
        let global_multiplier = 1.0
            + (self.tree_effect(pid, "tourism_pct")
                + self.monopoly_bonuses(pid).1
                + self.gov_effects(pid).tourism_pct)
                / 100.0;
        if let Some(sources) = by_tile.as_mut() {
            for amount in sources.values_mut() {
                *amount *= global_multiplier;
            }
        }
        (
            (tourism + holy_city_tourism + 0.15 * culture) * global_multiplier,
            film_studio_bonus * global_multiplier,
        )
    }

    pub(super) fn tourism_components_per_turn(&self, pid: usize) -> (f64, f64) {
        if let Some(memo) = self.query_memo.tourism.borrow().as_ref() {
            if let Some(components) = memo.get(&pid) {
                return *components;
            }
        }
        let components = self.tourism_components_per_turn_with_sources(pid, None);
        if let Some(memo) = self.query_memo.tourism.borrow_mut().as_mut() {
            memo.insert(pid, components);
        }
        components
    }

    /// Exact current Tourism source value at every map tile. Values sum to
    /// [`Game::tourism_per_turn`]; the map lens can therefore explain the
    /// culture race without reimplementing its rules in JavaScript.
    pub fn tourism_by_tile(&self, pid: usize) -> BTreeMap<Pos, f64> {
        let mut sources = BTreeMap::new();
        self.tourism_components_per_turn_with_sources(pid, Some(&mut sources));
        sources
    }

    pub fn tourism_per_turn(&self, pid: usize) -> f64 {
        if let Some(&observed) = self.observed_tourism_per_turn.get(&pid) {
            return observed;
        }
        self.tourism_components_per_turn(pid).0
    }

    /// The engine's own tourism per turn, ignoring any host reading — what
    /// `live_divergence` scores against the next export.
    pub fn tourism_per_turn_model(&self, pid: usize) -> f64 {
        self.tourism_components_per_turn(pid).0
    }

    pub(super) fn great_work_tourism(&self, pid: usize, kind: &str) -> f64 {
        // Shipped per-work Tourism: writings 2, works of art 2, artifacts 3,
        // music 4, relics 8.
        let base = match kind {
            "writing" => 2.0 * (1.0 + self.tree_effect(pid, "writing_tourism_pct") / 100.0),
            "art" | "religious_art" => {
                2.0 * (1.0 + self.policy_effect(pid, "art_artifact_tourism_pct") / 100.0)
            }
            "artifact" => 3.0 * (1.0 + self.policy_effect(pid, "art_artifact_tourism_pct") / 100.0),
            "music" => 4.0 * (1.0 + self.policy_effect(pid, "music_tourism_pct") / 100.0),
            "any" => 4.0,
            "relic" => 8.0,
            _ => 0.0,
        };
        let target = if kind == "religious_art" { "art" } else { kind };
        if self.congress_effect_active("heritage_organization", "A", target) {
            base * 2.0
        } else if self.congress_effect_active("heritage_organization", "B", target) {
            0.0
        } else {
            base
        }
    }

    /// Culture victory: visiting tourists must exceed the largest rival
    /// domestic-tourist count.
    pub(super) fn check_culture_victory(&mut self) {
        if self.is_finished() {
            return;
        }
        let majors: Vec<usize> = self
            .players
            .iter()
            .filter(|p| self.victory_eligible(p.id))
            .map(|p| p.id)
            .collect();
        if majors.len() < 2 {
            return;
        }
        for pid in &majors {
            let foreign = self.foreign_tourists(*pid);
            let target = majors
                .iter()
                .filter(|oid| *oid != pid && !self.same_team(*pid, **oid))
                .map(|oid| self.domestic_tourists(*oid))
                .max();
            if target.is_some_and(|target| foreign > target) && self.set_winner(*pid, "culture") {
                return;
            }
        }
    }

    /// Create a great work: the counter the yield model reads and the piece
    /// object theming reads move together.
    pub(crate) fn grant_great_work(&mut self, pid: usize, kind: &str, era: usize, creator: &str) {
        *self.players[pid]
            .counters
            .entry(format!("great_work:{kind}"))
            .or_insert(0) += 1;
        self.players[pid].great_work_pieces.push(GreatWorkPiece {
            kind: kind.to_string(),
            era,
            creator: creator.to_string(),
        });
    }

    /// The pieces each city houses: the counted distribution decides how
    /// many works of each kind a city holds, and the player's pieces fill
    /// those holdings in creation order.
    pub(super) fn housed_great_work_pieces(&self, pid: usize) -> Arc<GreatWorkPiecesByCity> {
        if let Some(memo) = self.query_memo.housed_pieces.borrow().as_ref() {
            if let Some(value) = memo.get(&pid) {
                return Arc::clone(value);
            }
        }
        let allocation = self.housed_great_work_pieces_uncached(pid);
        let value = if allocation.is_empty() {
            static EMPTY: std::sync::OnceLock<Arc<GreatWorkPiecesByCity>> =
                std::sync::OnceLock::new();
            Arc::clone(EMPTY.get_or_init(|| Arc::new(BTreeMap::new())))
        } else {
            Arc::new(allocation)
        };
        if let Some(memo) = self.query_memo.housed_pieces.borrow_mut().as_mut() {
            memo.insert(pid, Arc::clone(&value));
        }
        value
    }

    pub(super) fn housed_great_work_pieces_uncached(&self, pid: usize) -> GreatWorkPiecesByCity {
        let housed = self.housed_great_works(pid);
        if housed.is_empty() {
            return BTreeMap::new();
        }
        let mut by_kind: BTreeMap<&str, Vec<&GreatWorkPiece>> = BTreeMap::new();
        for piece in &self.players[pid].great_work_pieces {
            by_kind.entry(piece.kind.as_str()).or_default().push(piece);
        }
        let mut cursors: BTreeMap<String, usize> = BTreeMap::new();
        let mut out: BTreeMap<u32, Vec<GreatWorkPiece>> = BTreeMap::new();
        for (city, kinds) in housed.iter() {
            let entry = out.entry(*city).or_default();
            for (kind, count) in kinds {
                let cursor = cursors.entry(kind.clone()).or_insert(0);
                let pool = by_kind.get(kind.as_str());
                for offset in 0..*count {
                    if let Some(piece) = pool.and_then(|pool| pool.get(*cursor + offset)) {
                        entry.push((*piece).clone());
                    }
                }
                *cursor += count;
            }
        }
        out
    }

    /// Themed buildings in one city under the shipped predicates - an Art
    /// Museum wants three works of art from the same era by three different
    /// artists, an Archaeological Museum three artifacts from one era - and
    /// the bonus Culture and Tourism the themed trio earns (+100%).
    pub(super) fn city_theming(
        &self,
        pid: usize,
        cid: u32,
        pieces: &[GreatWorkPiece],
    ) -> (usize, f64, f64) {
        let city = &self.cities[&cid];
        let mut themed = 0;
        let mut culture = 0.0;
        let mut tourism = 0.0;
        if city.buildings.iter().any(|b| b == "art_museum") {
            let art: Vec<&GreatWorkPiece> = pieces
                .iter()
                .filter(|piece| matches!(piece.kind.as_str(), "art" | "religious_art"))
                .collect();
            let eras: BTreeSet<usize> = art.iter().map(|piece| piece.era).collect();
            for era in eras {
                let of_era: Vec<&&GreatWorkPiece> =
                    art.iter().filter(|piece| piece.era == era).collect();
                let creators: BTreeSet<&str> =
                    of_era.iter().map(|piece| piece.creator.as_str()).collect();
                if of_era.len() >= 3 && creators.len() >= 3 {
                    themed += 1;
                    culture += 9.0; // three works of art at 3 Culture each
                    tourism += 3.0 * self.great_work_tourism(pid, "art");
                    break;
                }
            }
        }
        if city.buildings.iter().any(|b| b == "archaeological_museum") {
            let artifacts: Vec<&GreatWorkPiece> = pieces
                .iter()
                .filter(|piece| piece.kind == "artifact")
                .collect();
            let eras: BTreeSet<usize> = artifacts.iter().map(|piece| piece.era).collect();
            for era in eras {
                let of_era: Vec<&&GreatWorkPiece> =
                    artifacts.iter().filter(|piece| piece.era == era).collect();
                let origins: BTreeSet<&str> =
                    of_era.iter().map(|piece| piece.creator.as_str()).collect();
                if of_era.len() >= 3 && origins.len() >= 3 {
                    themed += 1;
                    culture += 9.0;
                    tourism += 3.0 * self.great_work_tourism(pid, "artifact");
                    break;
                }
            }
        }
        (themed, culture, tourism)
    }
}
