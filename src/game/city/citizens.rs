//! The citizen plan: which tiles and specialist slots a city's citizens
//! work.
//!
//! The strategy a city's governor weights yields by
//! (`citizen_strategy`), the value of one citizen's job under it
//! (`citizen_value`), the specialist jobs a city's districts and
//! buildings offer (`city_specialist_jobs`), and the plan itself
//! (`city_citizen_plan`, `city_citizen_plan_weighted`). `docs/OPENINGS.md`
//! discusses the weights.
//!
//! Moved verbatim out of the `impl Game` block in `game/city.rs`
//! (2026-11-05). Methods the rest of `city` calls are `pub(super)`;
//! methods that were `pub(super)` (all of `game`) are
//! `pub(in crate::game)`, the same reach; the code is otherwise unchanged.

use super::*;

impl Game {
    /// Worked plots that need a Builder, taken once per immutable query scope.
    /// The complete citizen assignment and legal improvement tests remain the
    /// original calculation. A policy trace repeats them to retain its reads.
    pub(crate) fn unimproved_worked_tiles(&self, pid: usize) -> usize {
        let _memo = self.query_memo();
        let reuse = !self.query_memo.policy_reads_on.get();
        if reuse {
            if let Some(count) = self
                .query_memo
                .worked_backlog
                .borrow()
                .as_ref()
                .and_then(|memo| memo.get(&pid))
                .copied()
            {
                return count;
            }
        }
        let count = self
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| self.city_unimproved_worked_tiles(pid, cid))
            .sum();
        if reuse {
            if let Some(memo) = self.query_memo.worked_backlog.borrow_mut().as_mut() {
                memo.insert(pid, count);
            }
        }
        count
    }

    /// One city's contribution, including authoritative host assignments.
    pub(crate) fn city_unimproved_worked_tiles(&self, pid: usize, cid: u32) -> usize {
        self.city_citizen_plan(cid)
            .worked_tiles
            .into_iter()
            .filter(|pos| {
                self.map.get(*pos).is_some_and(|tile| {
                    tile.district.is_none() && (tile.improvement.is_none() || tile.pillaged)
                }) && self
                    .valid_improvements(pid, *pos)
                    .iter()
                    .any(|improvement| self.rules.improvements[improvement].builder_buildable)
            })
            .count()
    }

    /// Build the automatic citizen governor's priorities from three layers:
    /// survival/growth, this city's current role and production, and the
    /// civilization's strengths.  Re-evaluating it from current state means a
    /// city changes jobs immediately when it starts a wonder, goes to war,
    /// reaches its housing cap, or develops a specialty district.
    pub fn citizen_strategy(&self, cid: u32) -> CitizenStrategy {
        let city = &self.cities[&cid];
        let player = &self.players[city.owner];
        let population_settler = matches!(
            city.queue.first(),
            Some(Item::Unit { unit }) if unit == "settler"
        ) && self.settler_consumes_population(city.owner, cid);
        let mut weights = Yields {
            food: 1.25,
            production: 1.55,
            gold: 0.85,
            science: 1.30,
            culture: 1.20,
            faith: 0.90,
        };
        weights.food += player.citizen_food_bias;
        let mut focus = "balanced".to_string();

        // Existing districts make cities lean into their established role.
        // This is intentionally based on the district's actual ruleset yields
        // so modded specialty districts inherit sensible behavior.
        let mut specialty = Yields::default();
        for (name, pos) in &city.districts {
            specialty.add(self.district_yields(name, *pos));
        }
        for name in &city.buildings {
            specialty.add(self.rules.buildings[name].yields);
        }
        weights.production += specialty.production * 0.12;
        weights.gold += specialty.gold * 0.12;
        weights.science += specialty.science * 0.18;
        weights.culture += specialty.culture * 0.18;
        weights.faith += specialty.faith * 0.18;
        let specialties = [
            (specialty.production, "production"),
            (specialty.gold, "commerce"),
            (specialty.science, "science"),
            (specialty.culture, "culture"),
            (specialty.faith, "faith"),
        ];
        if let Some((amount, name)) = specialties
            .into_iter()
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then_with(|| a.1.cmp(b.1)))
        {
            if amount > 0.0 {
                focus = name.to_string();
            }
        }

        // Production is the immediate city-level plan. Yield-bearing items
        // also reinforce the specialization they are being built to support.
        if let Some(item) = city.queue.first() {
            match item {
                Item::Unit { unit } if unit == "settler" => {
                    focus = "expansion".to_string();
                    // A population-consuming Settler gets an exact food budget
                    // below. Provision Settlers do not shrink their city and
                    // retain the ordinary growth appetite.
                    if !population_settler {
                        weights.food += 0.55;
                    }
                    weights.production += 1.15;
                }
                Item::Unit { unit } | Item::Formation { unit, .. } => {
                    focus = if self.rules.units[unit].class == "military" {
                        "military".to_string()
                    } else {
                        "development".to_string()
                    };
                    weights.production += if focus == "military" { 1.35 } else { 0.85 };
                }
                Item::Building { building } => {
                    let spec = &self.rules.buildings[building];
                    focus = if spec.wonder {
                        "wonder"
                    } else {
                        "infrastructure"
                    }
                    .to_string();
                    weights.production += if spec.wonder { 1.75 } else { 0.80 };
                    weights.food += spec.yields.food * 0.10;
                    weights.gold += spec.yields.gold * 0.15;
                    weights.science += spec.yields.science * 0.22;
                    weights.culture += spec.yields.culture * 0.22;
                    weights.faith += spec.yields.faith * 0.22;
                }
                Item::District { district, pos } => {
                    focus = district.replace('_', " ");
                    weights.production += 1.00;
                    let dy = self.district_yields(district, *pos);
                    weights.gold += dy.gold * 0.16;
                    weights.science += dy.science * 0.22;
                    weights.culture += dy.culture * 0.22;
                    weights.faith += dy.faith * 0.22;
                }
                Item::Wonder { wonder, .. } => {
                    let spec = &self.rules.wonders[wonder];
                    focus = "wonder".to_string();
                    weights.production += 1.75;
                    weights.gold += spec.yields.gold * 0.15;
                    weights.science += spec.yields.science * 0.22;
                    weights.culture += spec.yields.culture * 0.22;
                    weights.faith += spec.yields.faith * 0.22;
                }
                Item::Repair { .. } => {
                    focus = "repair".to_string();
                    weights.production += 1.25;
                }
                Item::Project { .. } => {
                    focus = "space race".to_string();
                    weights.production += 1.75;
                    weights.science += 0.60;
                }
                Item::Product { product } => {
                    focus = format!("{product} product");
                    weights.production += 1.25;
                    weights.gold += 0.40;
                    weights.culture += 0.25;
                }
            }
        }

        // Civilization plans use ability keys rather than seat numbers, so a
        // custom ruleset may reorder civilizations without changing behavior.
        // The roster boundary matters here too: historical identities keep a
        // neutral plan even where a legacy rules row shares their name.
        if self.uses_civ6_content(city.owner) {
            match self.rules.civs.get(&player.civ).map(|c| c.ability.as_str()) {
                Some("trajans_column") => {
                    weights.production += 0.30;
                    weights.culture += 0.55;
                }
                Some("iteru") => {
                    weights.production += if self.map.tiles[&city.pos].has_river() {
                        1.00
                    } else {
                        0.55
                    };
                    weights.gold += 0.35;
                }
                Some("platos_republic") => weights.culture += 1.35,
                Some("dynastic_cycle") => {
                    weights.production += 0.30;
                    weights.science += 0.75;
                    weights.culture += 0.75;
                }
                Some("epic_quest") => {
                    weights.production += 0.65;
                    weights.gold += 0.45;
                }
                Some("gifts_for_the_tlatoani") => {
                    weights.food += 0.20;
                    weights.production += 0.85;
                    weights.gold += 0.40;
                }
                Some("ta_seti") => {
                    weights.food += 0.20;
                    weights.production += 1.25;
                }
                Some("killer_of_cyrus") => {
                    weights.food += 0.35;
                    weights.production += 1.05;
                }
                _ => {}
            }
        }

        let at_war = self.players.iter().any(|other| {
            other.id != city.owner
                && other.alive
                && !other.is_barbarian
                && self.is_at_war(city.owner, other.id)
        });
        if at_war {
            weights.production += 1.00;
            if city.queue.is_empty() {
                focus = "wartime".to_string();
            }
        }

        // Everything above is local: the districts standing here, the item in
        // the queue, the civilization, and one empire-wide `at_war` boolean.
        // None of it can see what the empire is trying to *win*, so a seat
        // racing to Mars and a seat feeding a war work the same tiles. A
        // directive is the channel that carries the plan down to the tile, and
        // it is additive on purpose — the local evidence keeps its full weight
        // and the plan tilts the margin.
        let mut growth_ceiling = f64::INFINITY;
        if let Some(directive) = player.city_directives.get(&cid) {
            weights.food += directive.emphasis.food;
            weights.production += directive.emphasis.production;
            weights.gold += directive.emphasis.gold;
            weights.science += directive.emphasis.science;
            weights.culture += directive.emphasis.culture;
            weights.faith += directive.emphasis.faith;

            match directive.role {
                CityRole::Balanced => {}
                CityRole::Forge => weights.production += 0.70,
                CityRole::Breadbasket => weights.food += 0.70,
                // Double down on whatever this city already earns most of,
                // reusing the specialty vector measured above rather than
                // guessing a second time.
                CityRole::Specialist => match focus.as_str() {
                    "production" => weights.production += 0.55,
                    "commerce" => weights.gold += 0.55,
                    "science" => weights.science += 0.55,
                    "culture" => weights.culture += 0.55,
                    "faith" => weights.faith += 0.55,
                    _ => {}
                },
                CityRole::Bastion => {
                    weights.production += 0.90;
                    if city.queue.is_empty() {
                        focus = "besieged".to_string();
                    }
                }
            }

            // Military awareness, on its own axis from the role: how hard the
            // city is being pressed scales the hammer appetite whatever it is
            // otherwise for, so a Forge two tiles from an enemy army starts
            // paying for its own defense before the alarm names it a Bastion.
            // Capped so a hopeless ratio cannot drive the appetite arbitrarily
            // high.
            if directive.pressure > 0.0 {
                weights.production += directive.pressure.min(2.0) * 0.80;
            }
            if directive.halt_growth {
                weights.food *= 0.70;
                growth_ceiling = 0.0;
            }
            if directive.role != CityRole::Balanced && focus == "balanced" {
                focus = directive.role.as_str().to_string();
            }
        }

        let housing_headroom = self.city_housing(city) - city.pop as f64;
        let amenities = self.city_amenity_surplus(city);
        let growth_surplus = if housing_headroom > 1.0 && amenities >= -2 {
            (0.75 + housing_headroom * 0.25).min(2.0)
        } else {
            // Do not sacrifice useful production/science to grow into a hard
            // housing cap; the food constraint below still prevents starvation.
            weights.food *= 0.55;
            0.0
        };
        if population_settler {
            // Production completes after this turn's growth step, then removes
            // one citizen. Bank at most the smaller city's next growth cost so
            // it can recover that citizen immediately afterward. Food above
            // that threshold has no marginal value while the Settler is in the
            // queue: nutrition remains a hard constraint, while discretionary
            // citizens optimize production and the empire's other yields.
            let post_settler_growth = self.growth_cost((city.pop - 1).max(1));
            growth_ceiling = growth_ceiling.min((post_settler_growth - city.food).max(0.0));
            weights.food = 0.0;
        }
        let food_target = 2.0 * city.pop as f64 + growth_surplus.min(growth_ceiling);
        CitizenStrategy {
            focus,
            weights,
            food_target,
        }
    }

    pub(in crate::game) fn citizen_value(ys: Yields, weights: Yields) -> f64 {
        ys.food * weights.food
            + ys.production * weights.production
            + ys.gold * weights.gold
            + ys.science * weights.science
            + ys.culture * weights.culture
            + ys.faith * weights.faith
    }

    /// Return one entry for every specialist slot in an active specialty
    /// district. The district supplies the base specialist yield; tier-three
    /// buildings improve every citizen in that district, and every worship
    /// building adds the standard +1 Faith specialist bonus.
    pub(in crate::game) fn city_specialist_jobs(&self, city: &City) -> Vec<(String, Yields)> {
        let mut districts = std::collections::BTreeMap::<String, Yields>::new();
        for (district, position) in &city.districts {
            if self.map.tiles[position].pillaged
                || (self.district_is_family(district, crate::name!("encampment"))
                    && city.encampment_pillaged)
            {
                continue;
            }
            let family = self.district_family(*district).to_string();
            let base = self
                .rules
                .districts
                .get(&family)
                .or_else(|| self.rules.districts.get(district.as_str()))
                .map(|spec| spec.citizen_yields)
                .unwrap_or_default();
            if base != Yields::default() {
                districts.entry(family).or_insert(base);
            }
        }

        let mut jobs = Vec::new();
        for (family, mut yields) in districts {
            let mut slots = 0;
            for building_name in &city.buildings {
                if city.pillaged_buildings.contains(building_name)
                    || (city.encampment_pillaged
                        && self.rules.buildings[building_name]
                            .district
                            .is_some_and(|district| self.district_family(district) == "encampment"))
                {
                    continue;
                }
                let building = &self.rules.buildings[building_name];
                if building
                    .district
                    .is_none_or(|district| self.district_family(district) != family)
                {
                    continue;
                }
                slots += building.citizen_slots.max(0) as usize;
                yields.food += building.effects.get("citizen_food").copied().unwrap_or(0.0);
                yields.production += building
                    .effects
                    .get("citizen_production")
                    .copied()
                    .unwrap_or(0.0);
                yields.gold += building.effects.get("citizen_gold").copied().unwrap_or(0.0);
                yields.science += building
                    .effects
                    .get("citizen_science")
                    .copied()
                    .unwrap_or(0.0);
                yields.culture += building
                    .effects
                    .get("citizen_culture")
                    .copied()
                    .unwrap_or(0.0);
                yields.faith += building
                    .effects
                    .get("citizen_faith")
                    .copied()
                    .unwrap_or(0.0);
                if building.worship_belief.is_some() {
                    yields.faith += 1.0;
                }
            }
            jobs.extend((0..slots).map(|_| (family.clone(), yields)));
        }
        jobs
    }

    /// Choose the actual population-worked tiles. It exhausts food-bearing and
    /// other usable tiles before falling back to specialists, while choosing
    /// by strategic value within each tier. It then performs the least-cost
    /// swaps needed to hit the food target. A final local improvement pass
    /// recovers strategic value without violating nutrition. This keeps the
    /// hot turn loop fast while preventing a production-focused governor from
    /// starving a city.
    pub fn city_citizen_plan(&self, cid: u32) -> CitizenPlan {
        self.city_citizen_plan_weighted(cid, None)
    }

    /// The same citizen assignment under a substituted weight vector.
    ///
    /// `None` is exactly [`Self::city_citizen_plan`] — this is additive and
    /// changes no behaviour. It exists so an instrument can ask what a city
    /// *would* work under different appetites without the engine adopting
    /// them, which is how `docs/OPENINGS.md` bounds the food ceiling on the
    /// capital: expansion is gated by capital growth, and the shipped weights
    /// value production at 1.55 against food at 1.25.
    pub fn city_citizen_plan_weighted(&self, cid: u32, weights: Option<Yields>) -> CitizenPlan {
        let city = &self.cities[&cid];
        let mut strategy = self.citizen_strategy(cid);
        if let Some(weights) = weights {
            strategy.weights = weights;
        }
        // A live Firaxis mirror has already made its citizen assignment. Use that
        // exact baseline for ordinary planning and yields; weighted probes remain
        // counterfactual and deliberately run CIVVIS's governor below.
        if weights.is_none() {
            if let Some(worked_tiles) = self.observed_city_worked_tiles.get(&cid) {
                return CitizenPlan {
                    strategy,
                    worked_tiles: worked_tiles.clone(),
                    specialists: self
                        .observed_city_specialists
                        .get(&cid)
                        .cloned()
                        .unwrap_or_default(),
                };
            }
        }
        let mut center = self.workable_tile_yields(city.pos);
        center.food = center.food.max(2.0);
        center.production = center.production.max(1.0);

        const FOOD_TILE_TIER: u8 = 0;
        const USABLE_TILE_TIER: u8 = 1;
        const SPECIALIST_TIER: u8 = 2;
        const BARREN_TILE_TIER: u8 = 3;

        #[derive(Clone)]
        struct Job {
            key: CitizenJobKey,
            pos: Option<Pos>,
            specialist: Option<String>,
            yields: Yields,
            value: f64,
            // Lower tiers are exhausted first. A specialist should not pull
            // an early citizen off usable ground just because its district
            // yield matches the city's current focus; barren plots remain a
            // lower fallback so a city with no usable tiles can still employ
            // its available specialist slots.
            fallback_tier: u8,
        }

        // A zero food weight means food is a constraint, not an objective.
        // The Settler budget uses that state: rank every usable job by its
        // other yields first, then let the nutrition swaps below buy only the
        // food still required. Putting food-bearing tiles in the preferred
        // tier here would optimize food beyond that explicit target.
        let growth_supported = strategy.weights.food > 1e-9
            && strategy.food_target > 2.0 * city.pop.max(0) as f64 + 1e-9;

        let mut cands: Vec<Job> = city
            .owned_tiles
            .iter()
            .filter(|pos| **pos != city.pos)
            .filter_map(|pos| {
                let tile = &self.map.tiles[pos];
                if tile.district.is_some()
                    || tile.district_foundation.is_some()
                    || tile.wonder.is_some()
                {
                    return None;
                }
                if tile.improvement.as_deref().is_some_and(|improvement| {
                    self.rules.improvements[improvement]
                        .effects
                        .get("unworkable")
                        .copied()
                        .unwrap_or(0.0)
                        > 0.0
                }) {
                    return None;
                }
                let ys = self.workable_tile_yields(*pos);
                let fallback_tier = if growth_supported && ys.food > 0.0 {
                    FOOD_TILE_TIER
                } else if ys.total() > 0.0 {
                    USABLE_TILE_TIER
                } else {
                    BARREN_TILE_TIER
                };
                Some(Job {
                    key: CitizenJobKey::tile(*pos),
                    pos: Some(*pos),
                    specialist: None,
                    yields: ys,
                    value: Self::citizen_value(ys, strategy.weights),
                    fallback_tier,
                })
            })
            .collect();
        for (index, (district, yields)) in self.city_specialist_jobs(city).into_iter().enumerate() {
            cands.push(Job {
                key: CitizenJobKey::Specialist(format!("specialist:{district}:{index:03}")),
                pos: None,
                specialist: Some(district),
                yields,
                value: Self::citizen_value(yields, strategy.weights),
                fallback_tier: SPECIALIST_TIER,
            });
        }
        cands.sort_by(|a, b| {
            a.fallback_tier
                .cmp(&b.fallback_tier)
                .then_with(|| b.value.partial_cmp(&a.value).unwrap())
                .then(a.key.cmp(&b.key))
        });
        let workers = (city.pop.max(0) as usize).min(cands.len());
        let mut selected = vec![false; cands.len()];
        for slot in selected.iter_mut().take(workers) {
            *slot = true;
        }
        // Fixed food (buildings, districts, routes, envoys, beliefs) satisfies
        // nutrition before citizens are pulled off more valuable jobs. This is
        // important for granary/harbor cities: food infrastructure should let
        // their population work production or specialty yields.
        let mut food = center.food;
        for (name, pos) in &city.districts {
            food += self.district_yields(name, *pos).food;
        }
        for name in &city.buildings {
            food += self.rules.buildings[name].yields.food;
        }
        for route in self.routes.iter().filter(|r| r.origin == cid) {
            if let Some(dest) = self.cities.get(&route.dest) {
                food += self.route_yields(route.dest, dest.owner == city.owner).food;
            }
        }
        if !self.players[city.owner].is_minor {
            food += self.envoy_yields(city.owner, city).food;
        }
        if self.city_religion(city).is_some() {
            if self.city_has_active_building_family(city, crate::name!("shrine")) {
                food += self.city_religion_belief_effect(city, "shrine_food");
            }
            if self.city_has_active_building_family(city, crate::name!("temple")) {
                food += self.city_religion_belief_effect(city, "temple_food");
            }
        }
        food += cands
            .iter()
            .enumerate()
            .filter(|(i, _)| selected[*i])
            .map(|(_, c)| c.yields.food)
            .sum::<f64>();

        // Meet nutrition through the smallest strategic-value sacrifice per
        // useful food. The loop is bounded by the candidate count because
        // every accepted swap strictly raises collected food.
        for _ in 0..cands.len() {
            if food + 1e-9 >= strategy.food_target {
                break;
            }
            let need = strategy.food_target - food;
            let mut best: Option<(f64, f64, &CitizenJobKey, &CitizenJobKey, usize, usize)> = None;
            for (out, a) in cands.iter().enumerate().filter(|(i, _)| selected[*i]) {
                for (inside, b) in cands.iter().enumerate().filter(|(i, _)| !selected[*i]) {
                    let food_gain = b.yields.food - a.yields.food;
                    if food_gain <= 1e-9 {
                        continue;
                    }
                    let value_gain = b.value - a.value;
                    let useful_food = food_gain.min(need);
                    let efficiency = value_gain / useful_food;
                    let candidate = (efficiency, value_gain, &a.key, &b.key, out, inside);
                    if best
                        .as_ref()
                        .map(|old| {
                            candidate.0 > old.0 + 1e-9
                                || ((candidate.0 - old.0).abs() < 1e-9
                                    && (candidate.1 > old.1 + 1e-9
                                        || ((candidate.1 - old.1).abs() < 1e-9
                                            && (candidate.2, candidate.3) < (old.2, old.3))))
                        })
                        .unwrap_or(true)
                    {
                        best = Some(candidate);
                    }
                }
            }
            match best {
                Some((_, _, _, _, out, inside)) => {
                    selected[out] = false;
                    selected[inside] = true;
                    food += cands[inside].yields.food - cands[out].yields.food;
                }
                None => break,
            }
        }

        // One-swap local optimum under the nutrition constraint.
        for _ in 0..cands.len() {
            let mut best: Option<(f64, &CitizenJobKey, &CitizenJobKey, usize, usize)> = None;
            for (out, a) in cands.iter().enumerate().filter(|(i, _)| selected[*i]) {
                for (inside, b) in cands.iter().enumerate().filter(|(i, _)| !selected[*i]) {
                    if b.fallback_tier > a.fallback_tier {
                        continue;
                    }
                    let value_gain = b.value - a.value;
                    let next_food = food + b.yields.food - a.yields.food;
                    if value_gain <= 1e-9 || next_food + 1e-9 < strategy.food_target {
                        continue;
                    }
                    let candidate = (value_gain, &a.key, &b.key, out, inside);
                    if best
                        .as_ref()
                        .map(|old| {
                            candidate.0 > old.0 + 1e-9
                                || ((candidate.0 - old.0).abs() < 1e-9
                                    && (candidate.1, candidate.2) < (old.1, old.2))
                        })
                        .unwrap_or(true)
                    {
                        best = Some(candidate);
                    }
                }
            }
            match best {
                Some((_, _, _, out, inside)) => {
                    selected[out] = false;
                    selected[inside] = true;
                    food += cands[inside].yields.food - cands[out].yields.food;
                }
                None => break,
            }
        }

        let mut worked_tiles: Vec<Pos> = cands
            .iter()
            .enumerate()
            .filter(|(i, _)| selected[*i])
            .filter_map(|(_, c)| c.pos)
            .collect();
        worked_tiles.sort();
        let mut specialists: Vec<String> = cands
            .iter()
            .enumerate()
            .filter(|(i, _)| selected[*i])
            .filter_map(|(_, c)| c.specialist.clone())
            .collect();
        specialists.sort();
        CitizenPlan {
            strategy,
            worked_tiles,
            specialists,
        }
    }
}

#[cfg(test)]
mod worked_backlog_memo_tests;
