//! What a city builds next, for the baseline controller.
//!
//! `pick_item` chooses a city's next production. It works through the
//! opening defence and the Settler book, recon and the front line (combined
//! arms, naval recon, siege support), the economic recovery items, and the
//! district and building items: the first Campus and its tiers, industry,
//! culture defence, the Plaza, housing (Granary, Aqueduct), amenities,
//! luxuries and the Builder backlog. When nothing else wins, it falls back to
//! a wonder. Gold is spent alongside it (`spend_gold`, the `buy_gold_*`
//! helpers), and so is the decision to add a Trader safely.
//!
//! Moved verbatim out of the main `impl BasicAi` block in `ai.rs`
//! (2026-11-05). Methods the rest of the controller calls are `pub(super)`;
//! the code is otherwise unchanged. `cities`, the city turn that calls
//! `pick_item`, stays in `ai.rs` for now because an open PR edits it.

use super::*;

impl BasicAi {
    /// Prefer a Native-quoted non-resort Campus site over a standing resort.
    /// `sites` already carries district legality; unquoted and all-resort
    /// choices retain the ordinary fallback, including repairable resorts.
    pub(super) fn culture_resort_site_is_displaced(
        g: &Game,
        cid: u32,
        district: Name,
        pos: Pos,
        sites: &[Pos],
    ) -> bool {
        if g.district_family(district) != "campus"
            || !sites.contains(&pos)
            || g.map.tiles.get(&pos).and_then(|tile| tile.improvement)
                != Some(crate::name!("seaside_resort"))
        {
            return false;
        }
        let Some(quoted) = g
            .host_district_sites
            .get(&cid)
            .and_then(|items| items.get(&district))
        else {
            return false;
        };
        quoted.contains(&pos)
            && sites.iter().any(|alternative| {
                quoted.contains(alternative)
                    && g.map.tiles.get(alternative).is_some_and(|tile| {
                        !tile.flooded
                            && !tile.submerged
                            && tile.improvement != Some(crate::name!("seaside_resort"))
                    })
            })
    }

    /// The strongest land recon unit this city can build right now.
    ///
    /// Ranked by movement first and strength second, because what this buys is
    /// tiles turned over per turn — a Scout that walks further finds a
    /// civilization sooner than a Ranger that fights better. Land only: the
    /// caller's trigger asks about unexplored *ground*, and a coastal city that
    /// answers a land-recon gap with a galley leaves the continent uncharted.
    pub(super) fn best_recon(&self, g: &Game, pid: usize, cid: u32) -> Option<String> {
        let mut best: Option<((f64, f64), String)> = None;
        for (name, spec) in &g.rules.units {
            // The same test `unit_doctrine` applies, read off the spec because
            // there is no unit yet to ask. `siege` is checked first there and
            // wins over the promotion class, so honour that order.
            if spec.class != "military"
                || matches!(spec.domain.as_deref(), Some("sea" | "air"))
                || spec.siege
                || spec.promotion_class != "recon"
            {
                continue;
            }
            if !g.can_produce(pid, cid, &Item::Unit { unit: *name }) {
                continue;
            }
            let rank = (spec.moves, spec.strength.max(spec.ranged_attack_strength()));
            if best.as_ref().map(|(b, _)| rank > *b).unwrap_or(true) {
                best = Some((rank, name.to_string()));
            }
        }
        best.map(|(_, n)| n)
    }

    pub(super) fn combined_arms_unit(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        melee: usize,
        ranged: usize,
    ) -> Option<String> {
        // Ranged units trade efficiently, but only melee units can take a
        // city. Alternate the strongest available unit in each role so an
        // advanced army never degenerates into an uncapturing firing line.
        // A walled enemy city is a wall problem, and only siege answers it.
        // Ask for that FIRST when one is in reach and the army has none, then
        // fall back to the ordinary melee/ranged alternation.
        let want_ranged = melee > ranged;
        self.best_military(g, pid, cid, Some(want_ranged))
            .or_else(|| self.best_military(g, pid, cid, None))
    }

    /// The sea's `recon_is_the_missing_arm`: no usable explorer, and water the
    /// empire has never seen. A hull trapped in a lake or linked to a Settler
    /// does not answer it. One free ship normally completes the arm; during a
    /// major naval war, retain a second eye so the fighting ship cannot make
    /// the unexplored world disappear from production's priorities.
    /// See `naval_recon`.
    /// Any version of the naval eye. See `naval_recon_2` and `naval_recon_3`.
    pub(crate) fn naval_recon_on(&self) -> bool {
        self.naval_recon || self.naval_recon_2 || self.naval_recon_3
    }

    pub(crate) fn naval_recon_is_the_missing_arm(&self, g: &Game, pid: usize) -> bool {
        if !(self.naval_recon_on() || self.island_exploration_active(g, pid))
            || self.minor
            || self.barb
        {
            return false;
        }
        if !Self::unseen_water_remains(g, pid) {
            return false;
        }
        let naval_eyes = g
            .units
            .values()
            .filter(|unit| {
                unit.owner == pid
                    && g.rules.units[unit.kind].class == "military"
                    && g.rules.units[unit.kind].domain.as_deref() == Some("sea")
                    && unit.linked_to.is_none()
                    && Self::naval_recon_ship_can_chart(g, pid, unit.id)
            })
            .count();
        // A ship already in a viable coastal queue is the arm being rebuilt.
        // Without this, every city inspected on the same production pass can
        // read the one-hull wartime gap and start another Galley.
        let queued_naval_eyes = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| Self::city_has_naval_recon_launch(g, pid, *cid))
            .flat_map(|cid| g.cities[&cid].queue.iter())
            .filter(|item| {
                matches!(item,
                    Item::Unit { unit } | Item::Formation { unit, .. }
                        if g.rules.units.get(unit.as_str()).is_some_and(|spec| {
                            spec.class == "military" && spec.domain.as_deref() == Some("sea")
                        })
                )
            })
            .count();
        // A barbarian raider counts when the flag asks it to: `desired_navy`'s
        // own `naval_war` test has always included barbarians, but this arm
        // excluded them — so the second wartime eye never arrived during
        // exactly the sea raid that sinks a lone explorer. See `sea_answers`.
        let major_naval_war = self.naval_recon_major_war(g, pid);
        let arm_target = if major_naval_war {
            NAVAL_RECON_WARTIME_ARM_MAX
        } else if self.naval_recon_2 {
            // See `naval_recon_2`: a second peacetime eye.
            2
        } else {
            1
        };
        if naval_eyes + queued_naval_eyes >= arm_target {
            return false;
        }
        g.player_city_ids(pid)
            .into_iter()
            .any(|cid| Self::city_has_naval_recon_launch(g, pid, cid))
    }

    /// The cheapest ship this city can lay down. Exploration wants a hull
    /// that moves, not a broadside; the cost is the whole price of an eye.
    pub(crate) fn best_naval_recon(&self, g: &Game, pid: usize, cid: u32) -> Option<String> {
        if !Self::city_has_naval_recon_launch(g, pid, cid) {
            return None;
        }
        // `contact-fleet`: the faster hull, when it is in reach. See
        // `advanced/contact_fleet.rs`.
        if let Some(fast) = self.contact_fleet_fast_hull(g, pid, cid) {
            return Some(fast);
        }
        let mut best: Option<((i64, i64), String)> = None;
        for (name, spec) in &g.rules.units {
            if spec.class != "military" || spec.domain.as_deref() != Some("sea") {
                continue;
            }
            if !g.can_produce(pid, cid, &Item::Unit { unit: *name }) {
                continue;
            }
            let rank = (spec.cost as i64, -(spec.moves as i64));
            if best.as_ref().map(|(b, _)| rank < *b).unwrap_or(true) {
                best = Some((rank, name.to_string()));
            }
        }
        best.map(|(_, n)| n)
    }

    pub(super) fn siege_support_unit(&self, g: &Game, pid: usize, cid: u32) -> Option<String> {
        if !siege_support::has_attackers(g, pid) {
            return None;
        }
        for unit in ["siege_tower", "battering_ram"] {
            let useful = g.cities.values().any(|city| {
                city.wall_hp > 0
                    && g.is_at_war(pid, city.owner)
                    && g.city_allows_siege_support(city.id, unit)
            });
            if useful
                && g.can_produce(
                    pid,
                    cid,
                    &Item::Unit {
                        unit: Name::new(unit),
                    },
                )
            {
                return Some(unit.to_string());
            }
        }
        None
    }

    /// City-local counterpart used by production. With the gene off this is
    /// the old empire-wide decision; with it on, a threatened frontier cannot
    /// veto a safe origin or cause an unsafe city to build a Trader that then
    /// has to walk elsewhere.
    pub(crate) fn should_add_trader_in_city_for_controller(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        traders: usize,
    ) -> bool {
        if self.barbarian_tactics && self.solvency_first_trade_slot {
            Self::should_add_trader(g, pid, traders)
                && self.safe_trade_origin_for_controller(g, pid, cid)
        } else {
            self.should_add_trader_for_controller(g, pid, traders)
        }
    }

    pub(super) fn economic_recovery_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        traders: usize,
    ) -> Option<Item> {
        if let Some(defence) = self.barbarian_defense_item(g, pid, cid) {
            return Some(defence);
        }
        if self.should_add_trader_in_city_for_controller(g, pid, cid, traders) {
            let trader = Item::Unit {
                unit: crate::name!("trader"),
            };
            if g.can_produce(pid, cid, &trader) {
                return Some(trader);
            }
        }

        let profitable_building = g
            .rules
            .buildings
            .iter()
            .filter(|(_, spec)| !spec.wonder && spec.yields.gold > spec.maintenance)
            .filter(|(building, _)| {
                g.can_produce(
                    pid,
                    cid,
                    &Item::Building {
                        building: **building,
                    },
                )
            })
            .map(|(building, spec)| {
                let net_gold = spec.yields.gold - spec.maintenance;
                (
                    net_gold / spec.cost.max(1.0),
                    net_gold,
                    std::cmp::Reverse(spec.cost as i64),
                    std::cmp::Reverse(*building),
                    *building,
                )
            })
            .max_by(|left, right| {
                left.0
                    .partial_cmp(&right.0)
                    .unwrap()
                    .then_with(|| left.1.partial_cmp(&right.1).unwrap())
                    .then(left.2.cmp(&right.2))
                    .then(left.3.cmp(&right.3))
            })
            .map(|(_, _, _, _, building)| Item::Building { building });
        if profitable_building.is_some() {
            return profitable_building;
        }

        ["commercial_hub", "harbor"]
            .into_iter()
            .flat_map(|district| {
                g.district_sites(cid, Name::new(district))
                    .into_iter()
                    .map(move |pos| (district, pos))
            })
            .filter_map(|(district, pos)| {
                let item = Item::District {
                    district: Name::new(district),
                    pos,
                };
                g.can_produce(pid, cid, &item).then_some((
                    g.district_site_yields(Name::new(district), pos).gold,
                    std::cmp::Reverse(district),
                    std::cmp::Reverse(pos),
                    item,
                ))
            })
            .max_by(|left, right| {
                left.0
                    .partial_cmp(&right.0)
                    .unwrap()
                    .then(left.1.cmp(&right.1))
                    .then(left.2.cmp(&right.2))
            })
            .map(|(_, _, _, item)| item)
    }

    /// A deficit city that cannot build direct Gold infrastructure must still
    /// use its Production without making the deficit worse. The recovery pass
    /// used to return `None` in that case and bypass every ordinary fallback,
    /// leaving otherwise productive cities idle for dozens of turns. Prefer a
    /// useful zero-maintenance building, then a repeatable district project;
    /// both convert the turn into value without adding unit/building upkeep.
    pub(super) fn upkeep_free_recovery_item(&self, g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let building = g
            .rules
            .buildings
            .iter()
            .filter(|(_, spec)| !spec.wonder && spec.maintenance <= f64::EPSILON)
            .filter_map(|(name, spec)| {
                let item = Item::Building { building: *name };
                if !g.can_produce(pid, cid, &item) {
                    return None;
                }
                let value = spec.yields.production * 5.0
                    + spec.yields.food * 3.0
                    + spec.yields.gold * 3.0
                    + spec.yields.science * 2.0
                    + spec.yields.culture * 2.0
                    + spec.yields.faith
                    + spec.housing * 3.0
                    + spec.amenity.max(0.0) * 4.0;
                (value > 0.0).then_some((
                    value / spec.cost.max(1.0),
                    value,
                    std::cmp::Reverse(*name),
                    item,
                ))
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| left.1.total_cmp(&right.1))
                    .then(left.2.cmp(&right.2))
            })
            .map(|(_, _, _, item)| item);
        if building.is_some() {
            return building;
        }

        g.rules
            .projects
            .iter()
            .filter(|(project, spec)| {
                spec.repeatable
                    && !matches!(
                        project.as_str(),
                        "lagrange_laser_station" | "terrestrial_laser_station"
                    )
            })
            .filter(|(project, _)| !self.nuclear_project_barred(project.as_str()))
            .filter_map(|(project, spec)| {
                let item = Item::Project { project: *project };
                if !g.can_produce(pid, cid, &item) {
                    return None;
                }
                // These yields accrue each turn as a percentage of Production,
                // not only on completion. A cheap loyalty project cannot repair
                // a deficit, and must not win an alphabetical tie over research.
                let gold = spec.ongoing_yields.get("gold").copied().unwrap_or(0.0);
                let output = spec
                    .ongoing_yields
                    .iter()
                    .map(|(yield_type, value)| {
                        let weight = match yield_type.as_str() {
                            "production" => 5.0,
                            "food" | "gold" => 3.0,
                            "science" | "culture" => 2.0,
                            _ => 1.0,
                        };
                        value.max(0.0) * weight
                    })
                    .sum::<f64>();
                let cost = g.item_cost_for_city(pid, cid, &item);
                let points = spec.completion_gpp.values().sum::<f64>() / cost.max(1.0);
                Some((gold, output, points, cost, *project, item))
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| left.1.total_cmp(&right.1))
                    .then_with(|| left.2.total_cmp(&right.2))
                    .then_with(|| right.3.total_cmp(&left.3))
                    .then_with(|| right.4.cmp(&left.4))
            })
            .map(|(_, _, _, _, _, item)| item)
    }

    fn minor_district_item(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let family = Self::minor_district_family(g, pid);
        if g.city_has_district_family(&g.cities[&cid], Name::new(family)) {
            return None;
        }
        let district = Self::civ_district(g, pid, family);
        g.district_sites(cid, district)
            .into_iter()
            .filter_map(|pos| {
                let item = Item::District { district, pos };
                g.can_produce(pid, cid, &item).then_some((
                    g.district_site_yields(district, pos).total(),
                    std::cmp::Reverse(pos),
                    item,
                ))
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| left.1.cmp(&right.1))
            })
            .map(|(_, _, item)| item)
    }

    /// `nuclear-goal-needs-a-launcher`: whether `project` is a nuclear
    /// project the base picker leaves alone. Always false with the gene off.
    pub(crate) fn nuclear_project_barred(&self, project: &str) -> bool {
        self.nuclear_goal_needs_a_launcher
            && matches!(
                project,
                "manhattan_project"
                    | "operation_ivy"
                    | "build_nuclear_device"
                    | "build_thermonuclear_device"
            )
    }

    // The counters deliberately remain explicit: the city loop updates them
    // after every selected item, and the diagnostic binary probes this same
    // decision surface. A transient aggregate would only obscure which live
    // count a production rule reads.

    #[allow(clippy::too_many_arguments)]
    pub fn pick_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        n_cities: usize,
        settlers: usize,
        builders: usize,
        traders: usize,
        siege_support: usize,
        military: usize,
        melee: usize,
        ranged: usize,
    ) -> Option<Item> {
        // Repairs restore yields and unlock every building/project in the
        // damaged district, so they outrank adding new infrastructure. Basic
        // and city-state governors previously ignored `Item::Repair`
        // entirely: Mohenjo-Daro left three districts and their buildings
        // pillaged while its queue remained empty for the rest of the game.
        let repair_rank = |item: &Item| match item {
            Item::Repair { repair, .. } if repair == "district" => 0,
            Item::Repair { .. } => 1,
            _ => 2,
        };
        if let Some(repair) = g
            .producible_items(pid, cid)
            .into_iter()
            .filter(|item| matches!(item, Item::Repair { .. }))
            .min_by(|left, right| {
                repair_rank(left)
                    .cmp(&repair_rank(right))
                    .then_with(|| {
                        g.item_cost_for_city(pid, cid, left)
                            .total_cmp(&g.item_cost_for_city(pid, cid, right))
                    })
                    .then_with(|| format!("{left:?}").cmp(&format!("{right:?}")))
            })
        {
            return Some(repair);
        }
        // ⚠ Ahead of the economic-recovery short circuit and the whole ordinary
        // build order. A city with a catapult in range does not have a problem
        // it can solve by finishing a Commercial Hub, and the empire-wide
        // military floor below cannot see that this particular city is the one
        // being taken. Repairs still outrank it: they restore the yields and
        // the defenses this branch is trying to buy.
        if let Some(defence) = self.besieged_city_item(g, pid, cid) {
            return Some(defence);
        }
        if let Some(defence) = self.barbarian_defense_item(g, pid, cid) {
            return Some(defence);
        }
        if self.minor {
            for project in ["repair_outer_defenses", "repair_encampment"] {
                let repair = Item::Project {
                    project: Name::new(project),
                };
                if g.can_produce(pid, cid, &repair) {
                    return Some(repair);
                }
            }
        }
        let city_pop = g.cities[&cid].pop;
        let at_major_war = g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && !player.is_barbarian
                && !player.is_minor
                && g.is_at_war(pid, player.id)
        });
        let recovery_reserve = 100.0 + 25.0 * n_cities as f64;
        let economic_recovery = !self.minor
            && !self.barb
            && g.players[pid].gold_per_turn < -0.5
            && g.players[pid].gold < recovery_reserve;
        let emergency_defense = (at_major_war
            || (self.barbarian_tactics && self.barbarian_local_alarm_for_controller(g, pid, cid)))
            && military < n_cities.max(1);
        if self.minor && !emergency_defense {
            for building in ["walls", "medieval_walls", "renaissance_walls"] {
                let wall = Item::Building {
                    building: Name::new(building),
                };
                if g.can_produce(pid, cid, &wall) {
                    return Some(wall);
                }
            }
        }
        if economic_recovery && !emergency_defense {
            return self
                .economic_recovery_item(g, pid, cid, traders)
                .or_else(|| self.upkeep_free_recovery_item(g, pid, cid));
        }
        // `launch-pad-keeps-its-rung`: the city's own placed Spaceport, until
        // the empire's first pad stands, ahead of every economy step. Local
        // defence and economic recovery above still win.
        if let Some(pad) = self.launch_pad_resume_item(g, pid, cid) {
            return Some(pad);
        }
        // `housing-bound-city-builds-its-granary`: a housing-bound city's
        // Granary, then a short city's Amenity, ahead of every economy step
        // and the military floor below. Local defence above still wins.
        if self.housing_bound_city_builds_its_granary
            && !self.minor
            && !self.barb
            && !emergency_defense
        {
            if let Some(item) = self.housing_bound_granary_step(g, pid, cid, n_cities, builders) {
                return Some(item);
            }
        }
        // `housing-cap-builds-the-aqueduct`: a housing-bound city's Aqueduct
        // once its Granary stands, ahead of every economy step and the
        // military floor below. Local defence above still wins.
        if self.housing_cap_builds_the_aqueduct && !self.minor && !self.barb && !emergency_defense {
            if let Some(item) = self.housing_cap_aqueduct_step(g, pid, cid) {
                return Some(item);
            }
        }
        // `builders-cover-the-worked-backlog`: Builders while the charges in
        // hand cover too little of the ground the cities already work
        // unimproved, ahead of the Monument, the Settler step and the military
        // floor below. A due Settler and local defence above still win.
        if self.builders_cover_the_worked_backlog
            && !self.minor
            && !self.barb
            && !emergency_defense
            && !self.settler_due(g, pid, cid, n_cities, settlers)
        {
            if let Some(builder) = self.worked_backlog_builder_step(g, pid, cid, n_cities, builders)
            {
                return Some(builder);
            }
        }
        // `monument-first`: the cheapest culture in the game before the
        // military floor and the Settler step, which otherwise kept a city
        // from ever reaching it. The capital sends the land grab's first two
        // Settlers out first, and an early rush still assembles its stack.
        if (self.builder_before_the_army
            || self.builder_before_the_army_2
            || self.builder_before_the_army_3)
            && !self.minor
            && !self.barb
            && !emergency_defense
        {
            if let Some(builder) =
                self.builder_before_the_army_item(g, pid, cid, n_cities, builders)
            {
                return Some(builder);
            }
        }
        // `prophet-race-takes-a-district-slot-2`: the race's Holy Site in the
        // city that builds it soonest. See `race_site_city`.
        if self.prophet_race_takes_a_district_slot_2
            && !self.minor
            && !self.barb
            && !emergency_defense
        {
            if let Some((site_city, holy_site)) = self.race_site_city(g, pid) {
                if site_city == cid {
                    think!(self.journal, Cities, Decision,
                           "{} opens the Holy Site for the Great Prophet race", g.cities[&cid].name;
                           "it builds the site soonest of the empire's cities; {} of {} religions founded",
                           g.religions_founded(), g.max_religions());
                    return Some(holy_site);
                }
            }
        }
        if self.campus_before_the_army && !self.minor && !self.barb && !emergency_defense {
            if let Some(item) = Self::campus_before_the_army_item(g, pid, cid, n_cities, false) {
                return Some(self.race_takes_the_district_slot(g, pid, cid, item));
            }
        }
        if self.monument_first
            && !self.minor
            && !self.barb
            && !emergency_defense
            && (!g.cities[&cid].is_capital
                || n_cities + settlers >= Self::MONUMENT_CAPITAL_MIN_CITIES)
            && (self.rush_military_floor == 0 || military >= self.rush_military_floor)
        {
            if let Some(monument) = Self::civ_building(g, pid, cid, "monument") {
                return Some(monument);
            }
        }
        // `capital-settler-after-completion` opens one specific expansion
        // window: when the capital's whole queue has drained and it has
        // reached the engine's population-two floor, a Settler gets the next
        // build. This deliberately comes after repairs, local defense, and
        // economic recovery, and it declines during a major war. It also keeps
        // the normal city target, time window, and practical-site gate, so it
        // is an opening heuristic rather than an unbounded Settler order.
        //
        // The ordinary Settler arm below serializes the pipeline unless a
        // different gene widens it. That is the ranking the live seat was
        // observed to lose to an Archer despite a legal second settlement
        // slot; this gene only changes that queue choice after the previous
        // item has completed.
        if (self.capital_settler_after_completion || self.rapid_city_expansion_2)
            && !self.minor
            && !self.barb
            && !at_major_war
            && !emergency_defense
            && g.cities[&cid].is_capital
            && g.cities[&cid].queue.is_empty()
            && g.cities[&cid].pop >= 2
            && ((n_cities + settlers) as f64) < self.w.city_target
            && (g.turn as f64) < self.w.settler_stop_turn
            && self.has_practical_settle_site(g, pid)
        {
            let settler = Item::Unit {
                unit: crate::name!("settler"),
            };
            if g.can_produce(pid, cid, &settler) {
                return Some(settler);
            }
        }
        let can_add_military = !self.minor || military < Self::minor_military_budget(g, pid);
        // An ancient rush needs a stack, not a garrison. While one is planned
        // the floor is the stack size rather than `mil_per_city * n_cities`,
        // and the units must be melee: only melee can land the capturing blow,
        // only melee exerts the zone of control that seals a siege ring, and a
        // land ranged unit attacking a city takes a flat -17 strength.
        let rushing = !self.minor && !self.barb && self.rush_military_floor > 0;
        // A besieged city raises the floor whether or not a rush is planned:
        // the raiding party is already here, and a stack mustered for a distant
        // rival does nothing about a horseman standing on the city's doorstep.
        let military_floor = if rushing {
            (self.w.mil_per_city * n_cities as f64).max(self.rush_military_floor as f64)
        } else {
            self.w.mil_per_city * n_cities as f64
        };
        if self.front_weighted_floor_2
            && !self.minor
            && !self.barb
            && can_add_military
            && (military as f64) < military_floor
        {
            if let Some(objective) = self.front_objective {
                if let Some(unit) = self
                    .combined_arms_unit(g, pid, cid, melee, ranged)
                    .filter(|unit| Self::is_front_city_for(g, pid, cid, unit, objective))
                {
                    think!(self.journal, Cities, Detail,
                           "A city beside the front takes the floor's {unit}";
                           "{} is {} tiles from the campaign's target and the army holds {military} \
                            against a floor of {military_floor:.1}", g.cities[&cid].name,
                           g.wdist(g.cities[&cid].pos, objective));
                    return Some(Item::Unit {
                        unit: Name::new(&unit),
                    });
                }
            }
        }
        if self.granary_before_the_army
            && !self.minor
            && !self.barb
            && !emergency_defense
            && !self.settler_due(g, pid, cid, n_cities, settlers)
        {
            if let Some(item) = Self::housing_reserve_item(g, pid, cid).filter(|item| {
                !matches!(item, Item::District { .. })
                    || g.host_production_turns(cid, item).unwrap_or_else(|| {
                        g.item_cost_for(pid, item) / g.city_yields(cid).production.max(0.5)
                    }) <= Self::FIRST_CAMPUS_MAX_TURNS
            }) {
                return Some(item);
            }
        }
        if (self.campus_before_the_army_2 || self.campus_before_the_army_3)
            && !self.minor
            && !self.barb
            && !emergency_defense
            && !self.settler_due(g, pid, cid, n_cities, settlers)
        {
            if let Some(item) = Self::campus_before_the_army_item(
                g,
                pid,
                cid,
                n_cities,
                self.campus_before_the_army_3,
            ) {
                return Some(self.race_takes_the_district_slot(g, pid, cid, item));
            }
        }
        // `builder-before-the-army-3`: Builders for the unimproved ground the
        // cities already work, behind the Campus step and a due Settler.
        if self.builder_before_the_army_3
            && !self.minor
            && !self.barb
            && !emergency_defense
            && !self.settler_due(g, pid, cid, n_cities, settlers)
        {
            if let Some(builder) = Self::builder_backlog_item(g, pid, cid, n_cities, builders) {
                return Some(builder);
            }
        }
        // `granary-before-the-army-2`: version 1's housing reserve, behind
        // the Campus step and the Builder backlog.
        if self.granary_before_the_army_2
            && !self.minor
            && !self.barb
            && !emergency_defense
            && !self.settler_due(g, pid, cid, n_cities, settlers)
        {
            if let Some(item) = Self::housing_reserve_item(g, pid, cid).filter(|item| {
                !matches!(item, Item::District { .. })
                    || g.host_production_turns(cid, item).unwrap_or_else(|| {
                        g.item_cost_for(pid, item) / g.city_yields(cid).production.max(0.5)
                    }) <= Self::FIRST_CAMPUS_MAX_TURNS
            }) {
                return Some(item);
            }
        }
        // `industrial-hub`: one zone where its Factory reaches the most
        // cities, and its chain, behind the Campus step and a due Settler.
        if self.industrial_hub
            && !self.minor
            && !self.barb
            && !emergency_defense
            && !self.settler_due(g, pid, cid, n_cities, settlers)
        {
            if let Some(item) = Self::industrial_hub_item(g, pid, cid)
                .filter(|item| !self.fossil_power_excluded(item))
            {
                think!(self.journal, Cities, Detail,
                       "The industrial hub takes the build";
                       "{} in {}", crate::reasoning::plain(&format!("{item:?}")),
                       g.cities[&cid].name);
                return Some(item);
            }
        }
        // `industrial-zone-in-the-producers`: the most productive cities'
        // Industrial Zones and Workshops, behind the industrial hub and a due
        // Settler, ahead of the Commercial Hub step and the floor.
        if self.industrial_zone_in_the_producers
            && !self.minor
            && !self.barb
            && !emergency_defense
            && !self.settler_due(g, pid, cid, n_cities, settlers)
        {
            if let Some(item) =
                self.industrial_zone_producer_item(g, pid, cid, n_cities, self.plan_threatened_city)
            {
                think!(self.journal, Cities, Detail,
                       "{} builds {} for its industry", g.cities[&cid].name,
                       Self::item_label(&item);
                       "industrial-zone-in-the-producers: one of the empire's most productive \
                        cities raises its Industrial Zone and Workshop ahead of the army");
                return Some(item);
            }
        }
        // `industry-before-the-army`: the production chain the district list
        // below never names, behind the Campus step and a due Settler.
        if (self.industry_before_the_army
            || self.industry_before_the_army_2
            || (self.industry_before_the_army_3 && military as f64 >= military_floor))
            && !self.minor
            && !self.barb
            && !emergency_defense
            && !self.settler_due(g, pid, cid, n_cities, settlers)
        {
            if let Some(item) = Self::industry_before_the_army_item(
                g,
                pid,
                cid,
                n_cities,
                self.industry_before_the_army_2 || self.industry_before_the_army_3,
            ) {
                think!(self.journal, Cities, Detail,
                       "Industry before the army takes the build";
                       "{} in {} ahead of a floor of {military_floor:.1} with {military} held",
                       crate::reasoning::plain(&format!("{item:?}")), g.cities[&cid].name);
                return Some(item);
            }
        }
        // `commercial-hub-and-traders`: a hub's Market, a Trader for an open
        // route slot, and a Commercial Hub in the best unthreatened cities,
        // behind the Campus step, the industry steps and a due Settler.
        if !emergency_defense
            && self.commercial_hub_window_open(g)
            && !self.settler_due(g, pid, cid, n_cities, settlers)
        {
            if let Some(item) = self.commercial_hub_step(g, pid, cid, n_cities, traders) {
                // `campus-buildings-first`: a second or third hub gives its
                // slot to this city's next Campus building.
                return Some(self.campus_building_takes_the_hub_slot(g, pid, cid, item));
            }
        }
        // `builders-before-the-lent-floor`: a Domination war lends this
        // governor an army target above the genome's own floor (see
        // `lent_military_floor_base`). Above that own floor, a unit for the
        // lent margin waits while the empire is short of Builders for the
        // ground its cities already work: the Builder backlog without its
        // Settler gate, since the floor ranks ahead of the Settler step anyway.
        // Over 40 live King runs (10-05, turns 1-80) the floor built 157 units
        // in the lent margin while the empire held fewer than one Builder per
        // two cities; a third of worked tiles were improved at turn 60 and
        // 71% of cities grew at a half or a quarter of their rate.
        if self.builders_before_the_lent_floor
            && !self.minor
            && !self.barb
            && !emergency_defense
            && can_add_military
            && (military as f64) < military_floor
            && self
                .lent_military_floor_base
                .is_some_and(|base| (military as f64) >= base * n_cities as f64)
        {
            if let Some(builder) = Self::builder_backlog_item(g, pid, cid, n_cities, builders) {
                think!(self.journal, Cities, Detail,
                       "A Builder before the lent floor's unit";
                       "{} trains a Builder: the empire holds {builders} for {n_cities} cities and \
                        {military} military against a lent floor of {military_floor:.1}",
                       g.cities[&cid].name);
                return Some(builder);
            }
        }
        // ★★★★★ THE FLOOR IS A HEADCOUNT AND CANNOT SEE A MISSING ARM.
        //
        // `military_floor` is `mil_per_city * n_cities`. It counts bodies and
        // asks nothing about what they can do, so an empire holding nineteen
        // units and NO SIEGE reports its army finished while it is physically
        // unable to take a city.
        //
        // Measured on run `civvis-20260803T082856Z`, a game CIVVIS was WINNING
        // (turn 226, 7 cities, score 645, ~3x the corpus mean):
        //
        //   19 military units against a floor of 7  -> this branch never ran
        //   151 turns at war with England, 594 military against their 56
        //   ZERO cities taken; all 7 of ours came from `found`, none captured
        //   England's cities: 400 wall, full health, the entire war
        //   siege units built in 251 turns: ZERO, with engineering,
        //   military_engineering, metal_casting AND steel all researched
        //
        // ⚠ NEITHER EXISTING REPAIR COULD REACH THIS. `siege_units_wanted` and
        // its `+95` production bonus are consulted only through
        // `redirect_repeatable_projects_for_force_gap`, which needs Conquest or
        // Recovery AND an army below `2 * cities` AND a repeatable project at
        // the head of the queue — instrumented over 251 turns, it was entered
        // ONCE. That is why #963 measured parity: it tuned an appetite nothing
        // reads. And a siege ROLE in the chooser is dead too, because the
        // chooser is never called while the headcount is satisfied.
        //
        // So the floor itself has to know an arm is missing. It stays a
        // headcount for everything else; this only adds "and we own nothing
        // that breaks a wall we are actually besieging".
        // ★★★★★ AND THE SAME HEADCOUNT CANNOT SEE THAT THE EMPIRE HAS GONE
        // BLIND. See [`BasicAi::recon_is_the_missing_arm`]: a floor of bodies
        // reads a 22-unit army as finished while not one of them explores, and
        // the run that measured it charted 23% of the world and met the
        // eventual winner on turn 215. Recon is an arm in exactly the sense
        // siege is, and it goes missing the same silent way.
        let missing_recon_arm = self.recon_is_the_missing_arm(g, pid);
        // And the sea has its own eye. See `naval_recon`.
        let missing_naval_recon_arm = self.naval_recon_is_the_missing_arm(g, pid);
        if can_add_military
            && ((military as f64) < military_floor || missing_recon_arm || missing_naval_recon_arm)
        {
            // A capability gap is an invitation to build that capability, not
            // a permanent waiver of the standing-army ceiling. A city without
            // oil cannot answer a missing artillery arm; falling through to a
            // Machine Gun there made every idle city repeat the same unrelated
            // defender while the wall-breaker remained impossible. Try the
            // concrete gaps in order, then use ordinary force production only
            // while the actual headcount is below its floor.
            let recon_pick = missing_recon_arm
                .then(|| self.best_recon(g, pid, cid))
                .flatten();
            let naval_recon_pick = missing_naval_recon_arm
                .then(|| self.best_naval_recon(g, pid, cid))
                .flatten();
            let force_pick = if (military as f64) < military_floor {
                if rushing && melee < self.rush_military_floor {
                    self.best_military(g, pid, cid, Some(false))
                        .or_else(|| self.combined_arms_unit(g, pid, cid, melee, ranged))
                } else {
                    self.combined_arms_unit(g, pid, cid, melee, ranged)
                }
            } else {
                None
            };
            // See `lent_military_floor_base`: a slow city leaves the lent
            // margin above the genome's own floor to the fast cities.
            let force_pick = force_pick.filter(|unit| {
                self.lent_military_floor_base.is_none_or(|base| {
                    (military as f64) < base * n_cities as f64
                        || Self::unit_build_turns(g, pid, cid, unit)
                            <= g.standard_duration(LENT_FLOOR_MAX_BUILD_TURNS) as f64
                })
            });
            // `front-weighted-floor`: leave the unit to a city that puts it at
            // the front sooner.
            let force_pick = force_pick.filter(|unit| {
                let keeps = !self.front_weighted_floor
                    || (military as f64) < FRONT_FLOOR_SHARE * military_floor
                    || self.front_objective.is_none_or(|objective| {
                        !Self::another_city_arrives_sooner(g, pid, cid, unit, objective)
                    });
                if !keeps {
                    think!(self.journal, Cities, Detail,
                           "Military floor leaves the {unit} to a city nearer the front";
                           "{} is {} tiles from the campaign's target; another city puts the unit \
                            there sooner", g.cities[&cid].name,
                           self.front_objective.map_or(0, |objective| g.wdist(g.cities[&cid].pos, objective)));
                }
                keeps
            });
            let picked = recon_pick.or(naval_recon_pick).or(force_pick);
            if let Some(m) = picked {
                // ⚠ THE BRANCH THAT WINS MUST SAY SO.
                //
                // A one-city run built 18 heavy chariots across 90 turns while its
                // own plan asked for seven cities, and NOTHING in the journal said
                // which branch of `pick_item` took the choice. An offline probe with
                // the deployment genome returned a settler for the same board, so
                // the two disagreed with no way to tell which was wrong.
                think!(self.journal, Cities, Detail,
                       "Military floor takes the build";
                       "holding {military} against a floor of {military_floor:.1}{}{}",
                       if missing_recon_arm { ", and the empire has no eyes" } else { "" },
                       if missing_naval_recon_arm { ", and no ship to chart the sea" } else { "" });
                return Some(Item::Unit {
                    unit: Name::new(&m),
                });
            }
        }
        if !self.minor && can_add_military && siege_support == 0 && melee >= 2 {
            if let Some(unit) = self.siege_support_unit(g, pid, cid) {
                return Some(Item::Unit {
                    unit: Name::new(&unit),
                });
            }
        }
        // Survival and the force floor above still win. Once they are met,
        // production already paid for as a physical Great Person outranks a
        // new Spy, Product, district project, Settler, or ordinary building.
        if let Some(item) = self.live_great_person_activation_item(g, pid, cid) {
            return Some(item);
        }
        if !self.minor && !self.barb {
            let has_spaceport = if g.players[pid].live_great_person_activation_needs.is_empty() {
                g.cities.values().any(|city| {
                    city.owner == pid
                        && (g.city_has_district_family(city, crate::name!("spaceport"))
                            || matches!(
                                city.queue.first(),
                                Some(Item::District { district, .. })
                                    if g.district_family(*district) == "spaceport"
                            ))
                })
            } else {
                // The live-only activation signal also means that an
                // unfinished host district is visible as a foundation rather
                // than this simulator's queue entry.
                Self::empire_district_family_ready_or_queued(g, pid, "spaceport")
            };
            // `first-pad-in-the-forge`: the empire's first pad only in a
            // producer. This step hands it to whichever city is being filled.
            // See `ai/first_pad_in_the_forge.rs`.
            if !self.exclude_space_race
                && !has_spaceport
                && g.players[pid].techs.contains(&crate::name!("rocketry"))
                && self.first_pad_city_allowed(g, pid, cid)
            {
                if let Some(pos) = g
                    .district_sites(cid, crate::name!("spaceport"))
                    .into_iter()
                    .next()
                {
                    let item = Item::District {
                        district: crate::name!("spaceport"),
                        pos,
                    };
                    if g.can_produce(pid, cid, &item) {
                        return Some(item);
                    }
                }
            }
            // The Culture lane lends this governor its Museum policy. The
            // advanced scorer values a digger, but delegated production never
            // reaches that scorer. Fill the empty Museum before another Spy.
            if let Some(digger) = self.culture_museum_digger(g, pid, cid) {
                return Some(digger);
            }
            let spy = Item::Unit {
                unit: crate::name!("spy"),
            };
            // `denial-spies-count-their-losses`: no Spy while recent ones keep
            // dying. See `ai/advanced/denial_spies_count_their_losses.rs`.
            if !self.spy_training_paused && g.can_produce(pid, cid, &spy) {
                return Some(spy);
            }
            if let Some(product) = g
                .producible_items(pid, cid)
                .into_iter()
                .find(|item| matches!(item, Item::Product { .. }))
            {
                return Some(product);
            }
            let projects: Vec<Item> = g
                .rules
                .projects
                .iter()
                .filter(|(project, spec)| {
                    (!spec.repeatable
                        || matches!(
                            project.as_str(),
                            "lagrange_laser_station" | "terrestrial_laser_station"
                        ))
                        && (!self.exclude_space_race
                            || !matches!(
                                project.as_str(),
                                "launch_earth_satellite"
                                    | "launch_moon_landing"
                                    | "launch_mars_colony"
                                    | "exoplanet_expedition"
                                    | "lagrange_laser_station"
                                    | "terrestrial_laser_station"
                            ))
                })
                .filter(|(project, _)| !self.nuclear_project_barred(project.as_str()))
                .map(|(project, _)| Item::Project { project: *project })
                .filter(|item| g.can_produce(pid, cid, item))
                .collect();
            // Cost and label taken once per candidate: the comparator used to
            // re-derive both, and its tiebreak built two Debug strings for
            // every comparison the sort made.
            let mut ranked: Vec<(f64, String, Item)> = projects
                .into_iter()
                .map(|item| (g.item_cost_for(pid, &item), format!("{item:?}"), item))
                .collect();
            ranked.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then_with(|| a.1.cmp(&b.1)));
            if let Some((_, _, project)) = ranked.into_iter().next() {
                return Some(project);
            }
        }
        let naval = Self::naval_counts(g, pid).0;
        if can_add_military
            && naval < self.desired_navy(g, pid)
            && !(self.settler_before_the_navy
                && !self.minor
                && !self.barb
                && self.settler_due(g, pid, cid, n_cities, settlers))
        {
            if let Some(unit) = self.best_naval_unit(g, pid, cid) {
                return Some(Item::Unit {
                    unit: Name::new(&unit),
                });
            }
        }
        // `campus-before-harbor-2`: the capital's first Campus before its
        // next Settler once the land grab has three cities to build them.
        if self.capital_campus_first
            && !self.minor
            && !self.barb
            && n_cities >= Self::CAPITAL_CAMPUS_MIN_CITIES
            && g.cities[&cid].is_capital
        {
            if let Some(item) = Self::first_campus_item(g, pid, cid) {
                return Some(self.race_takes_the_district_slot(g, pid, cid, item));
            }
        }
        // `district-buildings-first-2`: and the Campus's Library in the same
        // slot once the Campus stands.
        if self.capital_library_first
            && !self.minor
            && !self.barb
            && n_cities >= Self::CAPITAL_CAMPUS_MIN_CITIES
            && g.cities[&cid].is_capital
        {
            if let Some(item) = Self::campus_library_item(g, pid, cid) {
                return Some(item);
            }
        }
        // ⚠ Five conditions in one `&&` chain, and an empire that ends the game
        // with one city cannot say which of them refused. Named individually so
        // "the site search found nothing" is distinguishable from "the window
        // closed" without attaching a debugger to a finished run.
        if !self.minor && !self.barb {
            // ⚠ `room` deliberately counts EVERY settler, including stranded
            // ones. The cap on how many cities the empire is still trying to
            // reach must not move; only the "one at a time" serialisation is
            // discounted. See `stranded_settlers`.
            let SettlerGates {
                room,
                stranded,
                pipeline,
                parked_brake,
                none_in_flight,
                grown,
                in_window,
            } = self.settler_gates(g, pid, city_pop, n_cities, settlers);
            if room && none_in_flight && grown && in_window {
                if self.has_practical_settle_site(g, pid) {
                    return Some(Item::Unit {
                        unit: crate::name!("settler"),
                    });
                }
                think!(self.journal, Cities, Detail,
                       "No settler: every reachable site is refused";
                       "{n_cities} cities against a target of {:.1}, and the site \
                        search found nothing within reach",
                       self.w.city_target);
            } else if self.journal.wants(crate::reasoning::Level::Detail) {
                let mut why: Vec<&str> = Vec::new();
                if !room {
                    why.push("already at the city target");
                }
                if !none_in_flight {
                    why.push(if parked_brake {
                        "a parked settler holds the pipeline"
                    } else if pipeline > 1 {
                        "the settler pipeline is full"
                    } else {
                        "a settler is already in flight"
                    });
                }
                if !grown {
                    why.push("the city is below settler_min_pop");
                }
                if !in_window {
                    why.push("past settler_stop_turn");
                }
                think!(self.journal, Cities, Detail,
                       "No settler: {}", why.join(", ");
                       "{n_cities} cities and {settlers} settlers ({stranded} \
                        stranded) against a target of {:.1}, turn {} of {:.0}",
                       self.w.city_target, g.turn, self.w.settler_stop_turn);
            }
        }
        // `first-granary-reserve-3`: a housing-bound city grows before it
        // trains another Builder or opens a specialty district.
        if self.housing_reserve && !self.minor && !self.barb {
            if let Some(item) = Self::housing_reserve_item(g, pid, cid) {
                return Some(item);
            }
        }
        if (builders as f64) < self.w.builder_per_city * n_cities as f64
            && Self::has_builder_work(g, pid)
        {
            return Some(Item::Unit {
                unit: crate::name!("builder"),
            });
        }
        if !self.minor
            && self.should_add_trader_in_city_for_controller(g, pid, cid, traders)
            && g.can_produce(
                pid,
                cid,
                &Item::Unit {
                    unit: crate::name!("trader"),
                },
            )
        {
            return Some(Item::Unit {
                unit: crate::name!("trader"),
            });
        }
        if self.minor {
            if let Some(district) = Self::minor_district_item(g, pid, cid) {
                return Some(district);
            }
        }
        if let Some(monument) = Self::civ_building(g, pid, cid, "monument") {
            return Some(monument);
        }
        // `plaza-in-the-district-list`: the standing Plaza's Warlord's Throne,
        // then its Grand Master's Chapel, and the empire's first Plaza in the
        // capital -- ahead of `district-buildings-first`, whose Library,
        // Shrine and Temple otherwise hold the capital's queue every turn
        // (live King civvis-20261005T161132Z, game 142: armed, buildable from
        // turn 38, no Plaza by 81), and which would take any tier-one Plaza
        // building. A Prophet race still open goes first. See
        // `plaza_building_item` and `plaza_item`.
        if self.plaza_in_the_district_list && !self.minor && !self.barb {
            if let Some(item) =
                Self::plaza_building_item_for(g, pid, cid, self.plaza_serves_science)
            {
                return Some(item);
            }
            if !self.plaza_prophet_race_open(g, pid) {
                if let Some(item) = Self::plaza_item(g, pid, cid) {
                    return Some(item);
                }
            }
        }
        // `campus-buildings-first`: the standing Campus's Library, University
        // or Research Lab before the city opens another district or takes an
        // ordinary building. See `campus_buildings.rs`.
        if let Some(item) = self.campus_buildings_first_item(g, pid, cid) {
            return Some(item);
        }
        // `district-buildings-first`: the building that makes a standing
        // district pay before the city opens another district.
        if self.district_buildings_first && !self.minor && !self.barb {
            if let Some(item) = Self::district_building_item(g, pid, cid) {
                return Some(item);
            }
        }

        // `campus-before-harbor`: the city's first Campus before its Harbor.
        if self.campus_before_harbor && !self.minor && !self.barb {
            if let Some(item) = Self::first_campus_item(g, pid, cid) {
                return Some(self.race_takes_the_district_slot(g, pid, cid, item));
            }
        }
        // `culture-defense-theater`: a Theater Square while the empire's
        // Culture trails, ahead of the Harbor and the bred district order.
        if self.culture_defense_theater && !self.minor && !self.barb {
            if let Some(item) = Self::culture_defense_theater_item(g, pid, cid, n_cities) {
                return Some(self.race_takes_the_district_slot(g, pid, cid, item));
            }
        }
        // Coastal infrastructure is part of the water strategy, not an
        // accidental fallback after every land district. A harbor also gives
        // later naval production somewhere sensible to concentrate.
        if !self.minor
            && Self::city_is_coastal(g, cid)
            && !g.city_has_district_family(&g.cities[&cid], crate::name!("harbor"))
        {
            let harbor = Self::civ_district(g, pid, "harbor");
            let sites = g.district_sites(cid, harbor);
            if let Some(pos) = sites.into_iter().max_by(|a, b| {
                g.district_site_yields(harbor, *a)
                    .total()
                    .partial_cmp(&g.district_site_yields(harbor, *b).total())
                    .unwrap()
                    .then(a.cmp(b))
            }) {
                let item = Item::District {
                    district: Name::new(&harbor),
                    pos,
                };
                if g.can_produce(pid, cid, &item) {
                    return Some(item);
                }
            }
        }
        // `industry-in-the-district-list`: a standing zone's Workshop or
        // Factory before the city opens another district.
        if self.industry_in_the_district_list && !self.minor && !self.barb {
            if let Some(item) = Self::industry_building_item(g, pid, cid) {
                return Some(item);
            }
        }
        let mut dpri: Vec<(&str, f64)> = DISTRICT_PRIORITY
            .iter()
            .cloned()
            .zip([
                self.w.d_campus,
                self.w.d_commercial,
                self.w.d_holy,
                self.w.d_theater,
            ])
            .collect();
        // `industry-in-the-district-list`: and the zone itself, between the
        // Campus and the Commercial Hub.
        if self.industry_in_the_district_list && !self.minor && !self.barb {
            dpri.push((
                "industrial_zone",
                0.5 * (self.w.d_campus + self.w.d_commercial),
            ));
        }
        // A city paying the Amenity band asks for the district that repairs it
        // before it asks for another specialty district, because the band
        // multiplies what every one of those districts produces. Weighted by
        // the actual deficit, so this outranks the lane's own families only
        // while the multiplier is genuinely being paid and disappears the
        // moment the city is neutral.
        // ⚠ Taken BEFORE either repair is pushed, so the two repairs are each
        // ranked against the LANE and then against EACH OTHER by their own
        // magnitudes. Reading it after the Amenity push would stack the housing
        // repair on top of the Amenity one and make housing win by construction
        // — wrong in exactly the case that matters most, because
        // `amenity_growth_mult` is **0.0** below −4 and an Aqueduct handed to a
        // city that is not growing at all buys nothing.
        let lane_top = dpri.iter().map(|(_, w)| *w).fold(0.0_f64, f64::max);
        if self.amenity_districts && !self.minor {
            let deficit = (-g.city_amenity_surplus(&g.cities[&cid])).max(0) as f64;
            // ★★★★ NOT AS THE FIRST DISTRICT OF A CITY ONE SHORT. The band at
            // −1/−2 is ×0.90 (`amenity_yield_mult_for`); the deeper bands the
            // measurement above was made in start at −3. A brand-new city is
            // one short as soon as it reaches population 3 with no luxury of
            // its own, and this push then made an Entertainment Complex its
            // FIRST specialty district — 58 production in a 2.7-production
            // city (22 turns) plus a 150 Arena, to recover ten percent of
            // three science — ahead of the Campus the science chain hangs off.
            // Live runs civvis-20260816T054344Z (Puteoli/Rome/Aquileia t50–62),
            // T045316Z (Ravenna t48, Lugdunum) and T021044Z (t51) all opened
            // that way, and the best game (T030249Z) was the one whose new
            // cities were not short and opened with four Campuses. So the push
            // needs either the deeper band (deficit ≥ 2) or a city that already
            // holds two specialty districts, where the Amenity multiplier has
            // something to multiply and the lane has been served.
            let specialty = g.cities[&cid]
                .districts
                .keys()
                .filter(|district| {
                    !matches!(
                        g.district_family(**district).as_str(),
                        "city_center" | "aqueduct" | "neighborhood" | "canal" | "dam"
                    )
                })
                .count();
            if Self::amenity_repair_outranks_lane(deficit, specialty) {
                dpri.push(("entertainment_complex", lane_top + deficit));
            }
        }
        if self.minor {
            dpri.clear();
        }
        // ⚠⚠ THE RANKING IS A CONSTANT, AND ONE FAMILY IS ALWAYS LAST. `d_theater` is
        // the LOWEST of these four in **all 51 genomes** in `data/league/league.json`
        // — typically 1.0 against a Campus's 4.0 — and the loop below only skips a
        // family THIS CITY already has. So every city independently works down the
        // same list, and the fourth entry is reached only by a city that already
        // holds the other three.
        //
        // Measured on live run `civvis-20260803T090911Z` (5 cities, 242 turns):
        // CIVVIS ordered `DISTRICT_CAMPUS` **28 times** and `DISTRICT_THEATER`
        // **zero** times. The empire finished with 4 Campuses and no Theatre Square
        // anywhere, so the whole culture chain — Amphitheatre, Museum, Broadcast
        // Centre, and every Great Work slot — was unreachable by construction.
        // Culture ended at 22.1 against a science of comparable size.
        //
        // So scale each family by how much of the EMPIRE still lacks it. A fifth
        // Campus and a first Theatre Square are not the same decision, and a static
        // weight cannot tell them apart. This is deliberately a coverage term, not a
        // re-tuning: the genome's ordering still decides between two families the
        // empire is equally short of, so a bred preference is preserved wherever it
        // is actually expressing a preference.
        if !self.minor && (self.district_coverage || self.district_coverage_2) {
            let mine: Vec<&crate::game::City> =
                g.cities.values().filter(|city| city.owner == pid).collect();
            let total = mine.len().max(1) as f64;
            for (family, weight) in dpri.iter_mut() {
                let have = mine
                    .iter()
                    .filter(|city| g.city_has_district_family(city, Name::new(family)))
                    .count() as f64;
                // 1.0 when no city has it, falling toward 0.5 when every city
                // does — a quarter under `district_coverage_2`.
                let depth = if self.district_coverage_2 { 0.75 } else { 0.5 };
                *weight *= 1.0 - depth * (have / total);
            }
        }
        // `plaza-in-the-district-list`: the empire's first Government Plaza
        // at the front of the capital's list. A live Prophet race below still
        // goes ahead of it. See `plaza_wanted_here`.
        if self.plaza_in_the_district_list
            && !self.minor
            && !self.barb
            && Self::plaza_wanted_here(g, pid, cid)
        {
            let top = dpri.iter().map(|(_, w)| *w).fold(0.0_f64, f64::max);
            dpri.push(("government_plaza", top + 1.0));
        }
        // `enter-the-prophet-race-2`: the empire's first Holy Site goes to the
        // front while the race is open. The reservation below still limits
        // the empire to one site before a religion exists, and the unlock
        // check still waits for Astrology.
        if self.enter_prophet_race
            && !self.skip_prophet_race
            && g.players[pid].religion.is_none()
            && !g.cities.values().any(|other| {
                other.owner == pid
                    && (other
                        .districts
                        .keys()
                        .any(|district| g.district_family(*district) == "holy_site")
                        || matches!(
                            other.queue.first(),
                            Some(Item::District { district, .. })
                                if g.district_family(*district) == "holy_site"
                        ))
            })
        {
            let top = dpri.iter().map(|(_, w)| *w).fold(0.0_f64, f64::max);
            if let Some(entry) = dpri.iter_mut().find(|(family, _)| *family == "holy_site") {
                entry.1 = top + 1.0;
            }
        }
        dpri.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        for (family, _) in dpri {
            if family == "holy_site" && g.players[pid].religion.is_none() {
                // `skip-the-prophet-race-2`: never open the tab. See the flag's
                // doc comment — gating the prize without gating this costs
                // strictly more than either pure choice.
                if self.skip_prophet_race {
                    continue;
                }
                let prophet_race_closed = g.religions_founded() >= g.max_religions();
                let site_reserved = g.cities.values().any(|other| {
                    other.owner == pid
                        && (other
                            .districts
                            .keys()
                            .any(|district| g.district_family(*district) == "holy_site")
                            || matches!(
                                other.queue.first(),
                                Some(Item::District { district, .. })
                                    if g.district_family(*district) == "holy_site"
                            ))
                });
                // One active Holy Site is enough to contest the finite
                // Prophet race. Before a religion exists, duplicating it in
                // every newly founded city sacrifices settlers, campuses, and
                // basic infrastructure while adding points too late to change
                // the current recruitment. Once every religion is founded,
                // even the first site can no longer win a Prophet slot.
                // Founders may expand their faith network normally.
                if prophet_race_closed || site_reserved {
                    continue;
                }
            }
            if g.city_has_district_family(&g.cities[&cid], Name::new(family)) {
                continue;
            }
            // Ask for the district this civilization actually builds. Greece
            // builds an Acropolis, never a Theater Square, and naming the base
            // district produced an item the engine refused - which stalled the
            // city outright, because a rejected choice ends its turn.
            let dname = Self::civ_district(g, pid, family);
            let spec = &g.rules.districts[&dname];
            let unlocked = spec
                .tech
                .as_ref()
                .map(|t| g.players[pid].techs.contains(t))
                .unwrap_or(true)
                && spec
                    .civic
                    .as_ref()
                    .map(|c| g.players[pid].civics.contains(c))
                    .unwrap_or(true);
            if !unlocked {
                continue;
            }
            let mut sites = g.district_sites(cid, Name::new(dname.as_str()));
            if self.preserve_culture_resort_sites {
                let original = sites.clone();
                sites.retain(|pos| {
                    !Self::culture_resort_site_is_displaced(g, cid, dname, *pos, &original)
                });
            }
            if !sites.is_empty() {
                let best = *sites
                    .iter()
                    .max_by(|a, b| {
                        let ya = g.district_site_yields(dname, **a).total();
                        let yb = g.district_site_yields(dname, **b).total();
                        ya.partial_cmp(&yb).unwrap().then(a.cmp(b))
                    })
                    .unwrap();
                let item = Item::District {
                    district: dname,
                    pos: best,
                };
                // Never hand back something the engine will reject: a refused
                // item costs the city its whole turn.
                if g.can_produce(pid, cid, &item) {
                    return Some(item);
                }
            }
        }
        let mut buildable: Vec<(i64, Name)> = g
            .rules
            .buildings
            .iter()
            .filter(|(b, s)| {
                !s.wonder
                    && !self.fossil_power_excluded(&Item::Building { building: **b })
                    && g.can_produce(pid, cid, &Item::Building { building: **b })
            })
            .map(|(b, s)| (s.cost as i64, *b))
            .collect();
        if !buildable.is_empty() {
            // ⚠⚠ A COST TIE WAS BEING BROKEN ALPHABETICALLY, AND IT DECIDED WHICH
            // MUSEUM EVERY EMPIRE BUILDS. `sort()` on `(cost, Name)` falls through to
            // `Name::cmp`, which compares text — so with Art and Archaeological
            // Museum both at 290, `archaeological_museum` wins on the letter 'c'.
            //
            // They are otherwise IDENTICAL in `data/buildings.json`: same cost, same
            // +2 culture, same maintenance, same great-person points. Only the slot
            // kind differs, `art: 3` against `artifact: 3` — and those are not equally
            // fillable. Writing, Art and Music slots are filled by a Great Person who
            // arrives on their own points. An ARTIFACT slot needs an Archaeologist: a
            // **400-production civilian** that itself requires the Archaeological
            // Museum to exist first, then has to walk to a dig site and spend a charge.
            // No live run has ever built one.
            //
            // Measured on `civvis-20260803T082856Z`, the first run ever to build a
            // Museum: it built `BUILDING_MUSEUM_ARTIFACT` at t181 and finished with
            // **0 great works in 6 slots**. Across 50 earlier runs, Artist 9 activated
            // against 67 idle, Musician 0 against 31, and **0 artifact works ever
            // held**.
            //
            // ⚠ This changes ONLY tied pairs. Cheapest-first is untouched as a policy
            // — that argument belongs elsewhere — and a building with no slots keeps
            // its exact position. What it stops is the alphabet deciding a real
            // question.
            // `science-building-first`: one partition ahead of cost, nothing
            // else touched. See the flag's doc comment.
            let science_first = self.science_building_first;
            let makes_science =
                |b: &Name| -> bool { science_first && g.rules.buildings[b].yields.science > 0.0 };
            let tiebreak = self.slot_kind_tiebreak;
            // `culture-digs-one-museum`: a Culture seat's first Museum takes the
            // Artifact slot. See `advanced/culture_dig_museum.rs`.
            let dig_first = self.digs_one_museum && !Self::owns_dig_museum(g, pid);
            let slot_worth = |b: &Name| -> f64 {
                if !tiebreak && !dig_first {
                    return 0.0;
                }
                g.rules.buildings[b]
                    .great_work_slots
                    .iter()
                    .map(|(kind, count)| {
                        let count = (*count).max(0) as f64;
                        if kind == "artifact" {
                            count * if dig_first { 2.0 } else { 0.5 }
                        } else {
                            count
                        }
                    })
                    .sum()
            };
            buildable.sort_by(|a, b| {
                makes_science(&b.1)
                    .cmp(&makes_science(&a.1))
                    .then_with(|| a.0.cmp(&b.0))
                    .then_with(|| {
                        // ⚠⚠ ONLY WHEN BOTH CANDIDATES HAVE SLOTS. My first version
                        // compared slot worth across every cost tie, which is a far
                        // bigger change than the one being argued for: it reordered
                        // SIX tie groups, putting `old_god_obelisk` over `monument`,
                        // `temple` over `market`, `cathedral` over every other worship
                        // building, and `broadcast_center` over `research_lab`. Priced
                        // at 50 maps it came back **-14 Elo (p=0.6875) with culture
                        // LOWER, 151.7 against 153.8** — those are bad trades and the
                        // measurement said so.
                        //
                        // Restricting it to pairs that both carry slots leaves every
                        // one of those groups exactly as it was and decides only the
                        // question actually at issue: Art Museum against
                        // Archaeological Museum, identical in cost, culture,
                        // maintenance and great-person points, separated in the old
                        // sort by the letter 'c'.
                        let (wa, wb) = (slot_worth(&a.1), slot_worth(&b.1));
                        if wa <= 0.0 || wb <= 0.0 {
                            return std::cmp::Ordering::Equal;
                        }
                        wb.partial_cmp(&wa).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .then_with(|| a.1.cmp(&b.1))
            });
            return Some(Item::Building {
                building: Name::new(&buildable[0].1),
            });
        }
        // developed cities turn to wonders
        if !self.minor && g.cities[&cid].buildings.len() as f64 >= self.w.wonder_min_bld {
            if let Some(wonder) = Self::fallback_wonder(g, pid, cid) {
                return Some(wonder);
            }
        }
        // Repeatable district projects are a developed major-city fallback. If
        // considered with mandatory projects above, their low early base cost
        // makes a basic AI loop them forever before building Monuments,
        // districts, or district buildings. A completely developed city-state
        // may instead leave its production queue empty.
        if !self.barb && !self.minor {
            let projects: Vec<Item> = g
                .rules
                .projects
                .iter()
                .filter(|(project, spec)| {
                    spec.repeatable
                        && !matches!(
                            project.as_str(),
                            "lagrange_laser_station" | "terrestrial_laser_station"
                        )
                })
                .filter(|(project, _)| !self.nuclear_project_barred(project.as_str()))
                .map(|(project, _)| Item::Project { project: *project })
                .filter(|item| g.can_produce(pid, cid, item))
                .collect();
            // Cost and label taken once per candidate: the comparator used to
            // re-derive both, and its tiebreak built two Debug strings for
            // every comparison the sort made.
            let mut ranked: Vec<(f64, String, Item)> = projects
                .into_iter()
                .map(|item| (g.item_cost_for(pid, &item), format!("{item:?}"), item))
                .collect();
            ranked.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then_with(|| a.1.cmp(&b.1)));
            if let Some((_, _, project)) = ranked.into_iter().next() {
                return Some(project);
            }
        }
        can_add_military
            .then(|| self.combined_arms_unit(g, pid, cid, melee, ranged))
            .flatten()
            .map(|m| Item::Unit {
                unit: Name::new(&m),
            })
    }

    fn culture_museum_digger(&self, g: &Game, pid: usize, cid: u32) -> Option<Item> {
        if !self.digs_one_museum
            || self.minor
            || self.barb
            || self.plan_threatened_city == Some(cid)
            || g.players.iter().any(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && g.is_at_war(pid, other.id)
            })
        {
            return None;
        }
        // Reuse the physical-person planner's empty-slot, one-digger,
        // nearby-owned-land-site, local safety and production-budget checks.
        self.live_artifact_activation_digger(g, pid, cid)
    }

    /// A generic fallback has no strategic reason to walk the whole Wonder
    /// catalogue after Firaxis has already refused one in this city. The
    /// mirror's `blocked_wonders` feedback is live-only, so ordinary games
    /// retain their existing cheapest-legal-Wonder behavior.
    pub(super) fn fallback_wonder(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let host_refused_a_wonder_here = g
            .blocked_wonders
            .get(&cid)
            .is_some_and(|wonders| !wonders.is_empty());
        if host_refused_a_wonder_here {
            None
        } else {
            Self::cheapest_available_wonder(g, pid, cid)
        }
    }

    /// Whether `island-exploration` should spend a naval turn finding a new
    /// landmass now, rather than while the capital still has ordinary room.
    pub(crate) fn island_exploration_active(&self, g: &Game, pid: usize) -> bool {
        if !self.island_exploration {
            return false;
        }
        let landmass = Self::capital_landmass(g, pid);
        !landmass.is_empty()
            && self.main_landmass_settlement_room(
                g,
                pid,
                &landmass,
                ISLAND_EXPLORATION_HOME_SITE_LIMIT + 1,
            ) <= ISLAND_EXPLORATION_HOME_SITE_LIMIT
    }

    /// See `campus_before_harbor`: this city's first Campus at its best site,
    /// when the empire can build one here, the city holds none and none is
    /// already queued in it, and the city would finish it within
    /// [`Self::FIRST_CAMPUS_MAX_TURNS`]. `None` otherwise.
    pub(crate) fn first_campus_item(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        Self::first_campus_item_with_budget(g, pid, cid, Self::FIRST_CAMPUS_MAX_TURNS)
    }

    /// First Campus with an explicit build-time budget. Other district
    /// callers retain their existing fifteen-turn limit.
    pub(crate) fn first_campus_item_with_budget(
        g: &Game,
        pid: usize,
        cid: u32,
        max_turns: f64,
    ) -> Option<Item> {
        Self::first_district_item_with_budget(g, pid, cid, "campus", max_turns)
    }

    /// See `prophet_race_takes_a_district_slot`: `item` unless it is a Campus
    /// or a Theater Square while the race is open, a slot is left, and no
    /// city of the empire holds or is building a Holy Site; then this city's
    /// Holy Site, where one is buildable within `FIRST_CAMPUS_MAX_TURNS`.
    pub(crate) fn race_takes_the_district_slot(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: Item,
    ) -> Item {
        let specialty = matches!(
            &item,
            Item::District { district, .. }
                if matches!(g.district_family(*district).as_str(), "campus" | "theater_square")
        );
        if !specialty
            || !(self.prophet_race_takes_a_district_slot
                || self.prophet_race_takes_a_district_slot_2)
            || !self.race_open_without_a_site(g, pid)
        {
            return item;
        }
        // Version 2: only the city that builds the site soonest gives up
        // its slot. See `race_site_city`.
        if self.prophet_race_takes_a_district_slot_2
            && self.race_site_city(g, pid).map(|(city, _)| city) != Some(cid)
        {
            return item;
        }
        match Self::first_district_item(g, pid, cid, "holy_site") {
            Some(holy_site) => {
                think!(self.journal, Cities, Decision,
                       "{} opens a Holy Site in its {} slot", g.cities[&cid].name,
                       match &item {
                           Item::District { district, .. } => plain(district.as_str()),
                           _ => String::new(),
                       };
                       "the Great Prophet race is open with {} of {} religions founded, \
                        and the empire holds no Holy Site",
                       g.religions_founded(), g.max_religions());
                holy_site
            }
            None => item,
        }
    }

    /// Whether the Great Prophet race is open to this seat
    /// (`enter-the-prophet-race-2`), a slot is left, and no city of the
    /// empire holds or is building a Holy Site.
    fn race_open_without_a_site(&self, g: &Game, pid: usize) -> bool {
        self.enter_prophet_race
            && !self.skip_prophet_race
            && g.players[pid].religion.is_none()
            && g.religions_founded() < g.max_religions()
            && !g.cities.values().any(|other| {
                other.owner == pid
                    && (g.city_has_district_family(other, crate::name!("holy_site"))
                        || matches!(
                            other.queue.first(),
                            Some(Item::District { district, .. })
                                if g.district_family(*district) == "holy_site"
                        ))
            })
    }

    /// `prophet-race-takes-a-district-slot-2`: while the race is open
    /// without a site, the city of the empire that builds its Holy Site in
    /// the fewest turns, with that site; ties go to the lower city id.
    /// `None` when the race is closed or no city can build one within
    /// `FIRST_CAMPUS_MAX_TURNS`.
    pub(super) fn race_site_city(&self, g: &Game, pid: usize) -> Option<(u32, Item)> {
        if !self.race_open_without_a_site(g, pid) {
            return None;
        }
        g.player_city_ids(pid)
            .into_iter()
            .filter_map(|city| {
                let item = Self::first_district_item(g, pid, city, "holy_site")?;
                let turns = g.host_production_turns(city, &item).unwrap_or_else(|| {
                    g.item_cost_for(pid, &item) / g.city_yields(city).production.max(0.5)
                });
                Some((turns, city, item))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .map(|(_, city, item)| (city, item))
    }

    /// See `front_weighted_floor`: turns for `cid` to build `unit` and walk it
    /// to `objective` in a straight line at the unit's speed.
    fn front_arrival_turns(g: &Game, pid: usize, cid: u32, unit: &str, objective: Pos) -> f64 {
        let moves = g
            .rules
            .units
            .get(&Name::new(unit))
            .map_or(2.0, |spec| spec.moves)
            + Self::land_movement_bonus(g, pid);
        Self::unit_build_turns(g, pid, cid, unit)
            + g.wdist(g.cities[&cid].pos, objective) as f64 / moves.max(1.0)
    }

    /// The movement our land soldiers carry beyond their spec, read from the
    /// standing army (`Game::unit_max_moves`, the host's own figure on the
    /// live board): the least extra any land military unit on land shows,
    /// so a road or a promotion on one unit does not inflate it. Gran
    /// Colombia's Ejército Patriota reads +1. Zero with no such unit.
    fn land_movement_bonus(g: &Game, pid: usize) -> f64 {
        g.player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| {
                let unit = &g.units[&uid];
                let spec = g.rules.units.get(&unit.kind)?;
                (spec.class == "military"
                    && spec.domain.as_deref().is_none_or(|domain| domain == "land")
                    && !g
                        .map
                        .get(unit.pos)
                        .is_some_and(|tile| g.rules.is_water(tile)))
                .then(|| (g.unit_max_moves(uid) - spec.moves).max(0.0))
            })
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0)
            .min(2.0)
    }

    /// See `front_weighted_floor_2`: whether `cid` stands within
    /// `FRONT_NEAR_TILES` of `objective` and puts `unit` there within
    /// `FRONT_ARRIVAL_MARGIN` of the best city of ours that can build it.
    pub(crate) fn is_front_city_for(
        g: &Game,
        pid: usize,
        cid: u32,
        unit: &str,
        objective: Pos,
    ) -> bool {
        g.wdist(g.cities[&cid].pos, objective) <= FRONT_NEAR_TILES
            && !Self::another_city_arrives_sooner(g, pid, cid, unit, objective)
    }

    /// See `front_weighted_floor`: whether another city of ours that can build
    /// `unit` would put it at `objective` at least `FRONT_ARRIVAL_MARGIN`
    /// standard turns before `cid` would.
    pub(crate) fn another_city_arrives_sooner(
        g: &Game,
        pid: usize,
        cid: u32,
        unit: &str,
        objective: Pos,
    ) -> bool {
        let item = Item::Unit {
            unit: Name::new(unit),
        };
        let ours = Self::front_arrival_turns(g, pid, cid, unit, objective);
        let margin = g.standard_duration(FRONT_ARRIVAL_MARGIN) as f64;
        g.player_city_ids(pid).into_iter().any(|other| {
            other != cid
                && g.can_produce(pid, other, &item)
                && Self::front_arrival_turns(g, pid, other, unit, objective) + margin < ours
        })
    }

    /// See `campus_before_the_army`: this city's first Campus, else the
    /// Library of the Campus it holds, from three cities on and while fewer
    /// than one city in two has a Campus or a Library first in its queue, so
    /// the floor keeps the other half.
    pub(crate) fn campus_before_the_army_item(
        g: &Game,
        pid: usize,
        cid: u32,
        n_cities: usize,
        chain: bool,
    ) -> Option<Item> {
        if n_cities < Self::CAPITAL_CAMPUS_MIN_CITIES {
            return None;
        }
        let in_flight = g
            .player_city_ids(pid)
            .iter()
            .filter(|city| match g.cities[city].queue.first() {
                Some(Item::District { district, .. }) => g.district_family(*district) == "campus",
                Some(Item::Building { building }) => {
                    g.rules.buildings.get(building).is_some_and(|spec| {
                        (chain || spec.requires.is_empty())
                            && spec
                                .district
                                .is_some_and(|district| g.district_family(district) == "campus")
                    })
                }
                _ => false,
            })
            .count();
        if in_flight >= n_cities.div_ceil(2) {
            return None;
        }
        Self::first_campus_item(g, pid, cid)
            .or_else(|| Self::campus_library_item(g, pid, cid))
            .or_else(|| chain.then(|| Self::campus_tier_item(g, pid, cid)).flatten())
    }

    /// The slowest a city may build a Workshop or Factory under
    /// `industry_before_the_army`: a Workshop is 195 production, three times
    /// a first Campus's price on the live board.
    const INDUSTRY_BUILDING_MAX_TURNS: f64 = 20.0;

    /// Net Gold a turn below which `industry_before_the_army_2` leaves the
    /// chain alone: a zone and its Workshop cost two Gold a turn in upkeep,
    /// a Factory two more.
    pub(super) const INDUSTRY_MIN_GOLD_PER_TURN: f64 = 5.0;

    /// The next building of the Industrial Zone this city holds: its
    /// Workshop, else its Factory where no standing own Factory already
    /// reaches the city, each within `INDUSTRY_BUILDING_MAX_TURNS`. `None`
    /// for a city without a zone.
    pub(crate) fn industry_building_item(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let city = g.cities.get(&cid)?;
        if !g.city_has_district_family(city, crate::name!("industrial_zone")) {
            return None;
        }
        let within = |item: &Item| {
            g.host_production_turns(cid, item).unwrap_or_else(|| {
                g.item_cost_for(pid, item) / g.city_yields(cid).production.max(0.5)
            }) <= Self::INDUSTRY_BUILDING_MAX_TURNS
        };
        if let Some(workshop) = Self::civ_building(g, pid, cid, "workshop") {
            return within(&workshop).then_some(workshop);
        }
        let factory = Self::civ_building(g, pid, cid, "factory")?;
        let reached = g.cities.values().any(|other| {
            other.owner == pid
                && other.buildings.iter().any(|building| {
                    g.rules.buildings.get(building).is_some_and(|spec| {
                        spec.regional_range > 0
                            && spec.replaces.unwrap_or(*building) == "factory"
                            && g.wdist(other.pos, city.pos) <= spec.regional_range
                    })
                })
        });
        (!reached && within(&factory)).then_some(factory)
    }

    /// Whether `exclude_fossil_power` refuses this item: a Coal or Oil Power
    /// Plant while the flag stands. The Nuclear plant emits nothing.
    pub(crate) fn fossil_power_excluded(&self, item: &Item) -> bool {
        self.exclude_fossil_power
            && matches!(item, Item::Building { building }
                if matches!(building.as_str(), "coal_power_plant" | "oil_power_plant"))
    }

    /// The fewest own cities an `industrial_hub` zone must reach.
    pub(super) const INDUSTRIAL_HUB_MIN_REACH: usize = 3;

    /// The slowest the hub city may build its zone or a hub building.
    const INDUSTRIAL_HUB_MAX_TURNS: f64 = 25.0;

    /// Fuel a power plant burns, in the order `industrial_hub` tries them,
    /// and the stockpile that must stand before one is built.
    const INDUSTRIAL_HUB_PLANTS: [(&'static str, &'static str); 3] = [
        ("coal_power_plant", "coal"),
        ("oil_power_plant", "oil"),
        ("nuclear_power_plant", "uranium"),
    ];
    const INDUSTRIAL_HUB_MIN_FUEL: f64 = 5.0;

    /// See `industrial_hub`: the hub city's next item. The hub is the own
    /// city whose standing zone reaches the most own cities; with none
    /// reaching `INDUSTRIAL_HUB_MIN_REACH`, the city whose best zone site
    /// would reach the most (then the more productive) opens one there.
    pub(crate) fn industrial_hub_item(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let range = g
            .rules
            .buildings
            .get(&crate::name!("factory"))
            .map_or(6, |spec| spec.regional_range);
        let own: Vec<&crate::game::City> = g.cities.values().filter(|c| c.owner == pid).collect();
        if own.len() < Self::INDUSTRIAL_HUB_MIN_REACH {
            return None;
        }
        let reach = |pos: Pos| own.iter().filter(|c| g.wdist(pos, c.pos) <= range).count();
        let zone_of = |city: &crate::game::City| {
            city.districts
                .iter()
                .find(|(district, _)| g.district_family(**district) == "industrial_zone")
                .map(|(_, pos)| *pos)
        };
        let production = |city: &crate::game::City| g.city_yields(city.id).production;
        let within = |item: &Item| {
            g.host_production_turns(cid, item).unwrap_or_else(|| {
                g.item_cost_for(pid, item) / g.city_yields(cid).production.max(0.5)
            }) <= Self::INDUSTRIAL_HUB_MAX_TURNS
        };
        let hub = own
            .iter()
            .filter_map(|city| zone_of(city).map(|pos| (*city, reach(pos))))
            .filter(|(_, reached)| *reached >= Self::INDUSTRIAL_HUB_MIN_REACH)
            .max_by(|a, b| {
                a.1.cmp(&b.1)
                    .then(production(a.0).total_cmp(&production(b.0)))
                    .then(b.0.id.cmp(&a.0.id))
            })
            .map(|(city, _)| city.id);
        if let Some(hub) = hub {
            if hub != cid {
                return None;
            }
            if let Some(workshop) = Self::civ_building(g, pid, cid, "workshop") {
                return within(&workshop).then_some(workshop);
            }
            if g.players[pid].gold_per_turn < 0.0 {
                return None;
            }
            if let Some(factory) = Self::civ_building(g, pid, cid, "factory") {
                return within(&factory).then_some(factory);
            }
            return Self::INDUSTRIAL_HUB_PLANTS
                .iter()
                .find_map(|(plant, fuel)| {
                    if g.strategic_stockpile(pid, Name::new(fuel)) < Self::INDUSTRIAL_HUB_MIN_FUEL {
                        return None;
                    }
                    Self::civ_building(g, pid, cid, plant).filter(within)
                });
        }
        // No hub yet: the city whose best site reaches the most opens one.
        let zone = Self::civ_district(g, pid, "industrial_zone");
        let spec = g.rules.districts.get(&zone)?;
        let unlocked = spec
            .tech
            .as_ref()
            .is_none_or(|tech| g.players[pid].techs.contains(tech))
            && spec
                .civic
                .as_ref()
                .is_none_or(|civic| g.players[pid].civics.contains(civic));
        if !unlocked {
            return None;
        }
        let best_site = |city: &crate::game::City| {
            g.district_sites(city.id, Name::new(zone.as_str()))
                .into_iter()
                .map(|pos| {
                    (
                        pos,
                        reach(pos),
                        g.district_site_yields(zone, pos).production,
                    )
                })
                .max_by(|a, b| a.1.cmp(&b.1).then(a.2.total_cmp(&b.2)).then(b.0.cmp(&a.0)))
        };
        let (designated, pos, reached) = own
            .iter()
            .filter_map(|city| best_site(city).map(|(pos, reached, _)| (*city, pos, reached)))
            .max_by(|a, b| {
                a.2.cmp(&b.2)
                    .then(production(a.0).total_cmp(&production(b.0)))
                    .then(b.0.id.cmp(&a.0.id))
            })?;
        if designated.id != cid || reached < Self::INDUSTRIAL_HUB_MIN_REACH {
            return None;
        }
        let item = Item::District {
            district: zone,
            pos,
        };
        (g.can_produce(pid, cid, &item) && within(&item)).then_some(item)
    }

    /// See `industry_before_the_army`: this city's Workshop, else its Factory
    /// where no standing Factory reaches it, else its first Industrial Zone,
    /// from three cities on and while fewer than one city in three has the
    /// zone's family first in its queue.
    pub(crate) fn industry_before_the_army_item(
        g: &Game,
        pid: usize,
        cid: u32,
        n_cities: usize,
        reserve: bool,
    ) -> Option<Item> {
        if n_cities < Self::CAPITAL_CAMPUS_MIN_CITIES
            || (reserve && g.players[pid].gold_per_turn < Self::INDUSTRY_MIN_GOLD_PER_TURN)
        {
            return None;
        }
        let industrial = |district: Name| g.district_family(district) == "industrial_zone";
        let in_flight = g
            .player_city_ids(pid)
            .iter()
            .filter(|city| match g.cities[city].queue.first() {
                Some(Item::District { district, .. }) => industrial(*district),
                Some(Item::Building { building }) => g
                    .rules
                    .buildings
                    .get(building)
                    .is_some_and(|spec| spec.district.is_some_and(industrial)),
                _ => false,
            })
            .count();
        if in_flight >= n_cities.div_ceil(3) {
            return None;
        }
        let city = g.cities.get(&cid)?;
        let within = |item: &Item, cap: f64| {
            g.host_production_turns(cid, item).unwrap_or_else(|| {
                g.item_cost_for(pid, item) / g.city_yields(cid).production.max(0.5)
            }) <= cap
        };
        if g.city_has_district_family(city, crate::name!("industrial_zone")) {
            return Self::industry_building_item(g, pid, cid);
        }
        // The Campus comes first: a city that can open one and has not
        // leaves its next district to `campus_before_the_army_2`.
        if Self::first_campus_item(g, pid, cid).is_some() {
            return None;
        }
        // Version 2: and a city without its Theater keeps a slot for one.
        if reserve && !g.city_has_district_family(city, crate::name!("theater_square")) {
            let built = city
                .districts
                .keys()
                .filter(|district| g.rules.districts[district].specialty)
                .count()
                + city
                    .owned_tiles
                    .iter()
                    .filter_map(|position| g.map.tiles[position].district_foundation.as_ref())
                    .filter(|foundation| g.rules.districts[foundation.district].specialty)
                    .count();
            if g.city_specialty_district_capacity(city) < built + 2 {
                return None;
            }
        }
        let dname = Self::civ_district(g, pid, "industrial_zone");
        let spec = g.rules.districts.get(&dname)?;
        let unlocked = spec
            .tech
            .as_ref()
            .is_none_or(|tech| g.players[pid].techs.contains(tech))
            && spec
                .civic
                .as_ref()
                .is_none_or(|civic| g.players[pid].civics.contains(civic));
        if !unlocked {
            return None;
        }
        let (pos, adjacency) = g
            .district_sites(cid, Name::new(dname.as_str()))
            .into_iter()
            .map(|pos| (pos, g.district_site_yields(dname, pos).production))
            .max_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))?;
        if adjacency < 1.0 {
            return None;
        }
        let item = Item::District {
            district: dname,
            pos,
        };
        (g.can_produce(pid, cid, &item) && within(&item, Self::FIRST_CAMPUS_MAX_TURNS))
            .then_some(item)
    }

    /// See `campus_before_the_army_3`: the University, else the Research Lab,
    /// of the Campus this city holds (its civilization's replacement where it
    /// has one), when the city would finish it within
    /// `FIRST_CAMPUS_MAX_TURNS`.
    pub(crate) fn campus_tier_item(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        if !g.city_has_district_family(g.cities.get(&cid)?, crate::name!("campus")) {
            return None;
        }
        ["university", "research_lab"]
            .into_iter()
            .find_map(|family| {
                let item = Self::civ_building(g, pid, cid, family)?;
                let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
                    g.item_cost_for(pid, &item) / g.city_yields(cid).production.max(0.5)
                });
                (turns <= Self::FIRST_CAMPUS_MAX_TURNS).then_some(item)
            })
    }

    /// See `capital_library_first`: the Library of a Campus this city holds,
    /// when the city would finish it within `FIRST_CAMPUS_MAX_TURNS`.
    pub(crate) fn campus_library_item(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        if !g.city_has_district_family(g.cities.get(&cid)?, crate::name!("campus")) {
            return None;
        }
        let item = Self::civ_building(g, pid, cid, "library")?;
        let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
            g.item_cost_for(pid, &item) / g.city_yields(cid).production.max(0.5)
        });
        (turns <= Self::FIRST_CAMPUS_MAX_TURNS).then_some(item)
    }

    /// See `housing_reserve`: the Granary, then the Aqueduct, for a city whose
    /// population is within one of its housing. Neither while the city has
    /// room to grow, and the Aqueduct only once the Granary stands or cannot
    /// be built here.
    /// Cities founded or on the road before `monument_first` lets the capital
    /// stop for its Monument: the capital is the land grab's settler pump,
    /// and cities held at turn 60 drive the rest of the game.
    const MONUMENT_CAPITAL_MIN_CITIES: usize = 3;

    /// `culture_defense_theater` answers while the empire's Culture a turn is
    /// under this share of the strongest living rival major's — the bar the
    /// Advanced governor's `culture-floor` uses (`CULTURE_FLOOR_RATIO`).
    const CULTURE_DEFENSE_RATIO: f64 = 0.7;

    /// A seat's Culture a turn: what its cities make, plus the host's
    /// correction for that seat. On the live board a rival's cities are
    /// rebuilt from what the seat has seen and its public Culture figure
    /// lands in `observed_yield_adjustments`, so the cities alone would read
    /// every rival at about zero. Empty on a native board.
    pub(crate) fn seat_culture_per_turn(g: &Game, seat: usize) -> f64 {
        g.player_city_ids(seat)
            .into_iter()
            .map(|city| g.city_yields(city).culture)
            .sum::<f64>()
            + g.observed_yield_adjustments
                .get(&seat)
                .map_or(0.0, |adjustment| adjustment.culture)
    }

    /// Whether the empire's Culture trails `CULTURE_DEFENSE_RATIO` of the
    /// strongest living rival major's.
    pub(crate) fn culture_trails_the_field(g: &Game, pid: usize) -> bool {
        let best = g
            .players
            .iter()
            .filter(|player| {
                player.id != pid && player.alive && !player.is_minor && !player.is_barbarian
            })
            .map(|player| Self::seat_culture_per_turn(g, player.id))
            .fold(0.0_f64, f64::max);
        best > 0.0 && Self::seat_culture_per_turn(g, pid) < Self::CULTURE_DEFENSE_RATIO * best
    }

    /// See `culture_defense_theater`: this city's Theater Square at its best
    /// site while the empire's Culture trails, the city holds none, and fewer
    /// than half of the empire's cities hold or have queued one.
    /// See `builder_before_the_army`: a Builder while the empire holds (or
    /// has queued) fewer than its quota, there is land to improve, and this
    /// city finishes one within `LENT_FLOOR_MAX_BUILD_TURNS`. The quota is
    /// the genome's `builder_per_city`, at most one per two cities, and at
    /// least one once the empire holds two cities.
    pub(crate) fn builder_before_the_army_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        n_cities: usize,
        builders: usize,
    ) -> Option<Item> {
        if n_cities < 2 {
            return None;
        }
        let quota = if self.builder_before_the_army_2 || self.builder_before_the_army_3 {
            1
        } else {
            ((self.w.builder_per_city * n_cities as f64).ceil() as usize)
                .min(n_cities.div_ceil(2))
                .max(1)
        };
        if builders >= quota || !Self::has_builder_work(g, pid) {
            return None;
        }
        let builder = Item::Unit {
            unit: crate::name!("builder"),
        };
        if !g.can_produce(pid, cid, &builder)
            || Self::unit_build_turns(g, pid, cid, "builder")
                > g.standard_duration(LENT_FLOOR_MAX_BUILD_TURNS) as f64
        {
            return None;
        }
        Some(builder)
    }

    /// See `builder_before_the_army_3`: a Builder while the empire holds (or
    /// has queued) fewer than one per three unimproved worked tiles, at most
    /// one per two cities, and this city finishes one within
    /// `LENT_FLOOR_MAX_BUILD_TURNS`.
    pub(crate) fn builder_backlog_item(
        g: &Game,
        pid: usize,
        cid: u32,
        n_cities: usize,
        builders: usize,
    ) -> Option<Item> {
        if n_cities < 2 || builders >= n_cities.div_ceil(2) {
            return None;
        }
        let quota = Self::unimproved_worked_tiles(g, pid)
            .div_ceil(3)
            .min(n_cities.div_ceil(2));
        if builders >= quota {
            return None;
        }
        let builder = Item::Unit {
            unit: crate::name!("builder"),
        };
        (g.can_produce(pid, cid, &builder)
            && Self::unit_build_turns(g, pid, cid, "builder")
                <= g.standard_duration(LENT_FLOOR_MAX_BUILD_TURNS) as f64)
            .then_some(builder)
    }

    pub(crate) fn culture_defense_theater_item(
        g: &Game,
        pid: usize,
        cid: u32,
        n_cities: usize,
    ) -> Option<Item> {
        if g.city_has_district_family(&g.cities[&cid], crate::name!("theater_square")) {
            return None;
        }
        let held = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|other| {
                let city = &g.cities[other];
                g.city_has_district_family(city, crate::name!("theater_square"))
                    || matches!(
                        city.queue.first(),
                        Some(Item::District { district, .. })
                            if g.district_family(*district) == "theater_square"
                    )
            })
            .count();
        if 2 * held >= n_cities.max(1) || !Self::culture_trails_the_field(g, pid) {
            return None;
        }
        let theater = Self::civ_district(g, pid, "theater_square");
        let pos = g.district_sites(cid, theater).into_iter().max_by(|a, b| {
            g.district_site_yields(theater, *a)
                .total()
                .partial_cmp(&g.district_site_yields(theater, *b).total())
                .unwrap_or(Ordering::Equal)
                .then(a.cmp(b))
        })?;
        let item = Item::District {
            district: theater,
            pos,
        };
        g.can_produce(pid, cid, &item).then_some(item)
    }

    /// District families whose first building `district_buildings_first`
    /// raises ahead of a new district, in the order it takes them. Science
    /// first; the Arena before the Theater because a city short of Amenities
    /// loses a tenth of every yield; the Encampment last, as a soldier's
    /// building rather than an economy's.
    const DISTRICT_BUILDING_ORDER: [&'static str; 8] = [
        "campus",
        "commercial_hub",
        "industrial_zone",
        "entertainment_complex",
        "theater_square",
        "harbor",
        "holy_site",
        "encampment",
    ];

    /// See `district_buildings_first`: the first building of a district the
    /// city already holds — one that requires no other building, so a
    /// Library and never a University — in `DISTRICT_BUILDING_ORDER`.
    pub(crate) fn district_building_item(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let city = &g.cities[&cid];
        Self::DISTRICT_BUILDING_ORDER
            .iter()
            .filter(|family| g.city_has_district_family(city, Name::new(family)))
            .find_map(|family| {
                let mut tier_one: Vec<(i64, Name)> = g
                    .rules
                    .buildings
                    .iter()
                    .filter(|(_, spec)| {
                        !spec.wonder
                            && spec.requires.is_empty()
                            && spec.requires_any.is_empty()
                            && spec
                                .district
                                .is_some_and(|district| g.district_family(district) == *family)
                    })
                    .map(|(name, spec)| (spec.cost as i64, *name))
                    .filter(|(_, name)| {
                        g.can_produce(pid, cid, &Item::Building { building: *name })
                    })
                    .collect();
                tier_one.sort();
                tier_one
                    .into_iter()
                    .next()
                    .map(|(_, building)| Item::Building { building })
            })
    }

    pub(crate) fn housing_reserve_item(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let city = &g.cities[&cid];
        if (city.pop as f64) + 1.0 < g.city_housing(city) {
            return None;
        }
        if let Some(granary) = Self::civ_building(g, pid, cid, "granary") {
            return Some(granary);
        }
        if g.city_has_district_family(city, crate::name!("aqueduct")) {
            return None;
        }
        let aqueduct = Self::civ_district(g, pid, "aqueduct");
        let pos = g.district_sites(cid, aqueduct).into_iter().min()?;
        let item = Item::District {
            district: aqueduct,
            pos,
        };
        g.can_produce(pid, cid, &item).then_some(item)
    }

    /// `housing-bound-city-builds-its-granary`: the city's next build when it
    /// is housing-bound or short of Amenities. In order: its Granary while its
    /// population is within one of its housing (the board's own figure, the
    /// host's on a live seat); then, while its Amenity surplus is negative, the
    /// quickest Amenity building it finishes within `FIRST_CAMPUS_MAX_TURNS`,
    /// or a Builder for a luxury the empire lacks (see `luxury_builder_item`).
    /// `None` in a city the plan names threatened (`plan_threatened_city`) or
    /// that was attacked within four turns, the test the Advanced Amenity
    /// handoffs use, and in a city holding a Settler more than half built.
    pub(super) fn housing_bound_granary_step(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        n_cities: usize,
        builders: usize,
    ) -> Option<Item> {
        let city = &g.cities[&cid];
        let threatened = self.plan_threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4);
        if threatened || Self::settler_half_built(g, pid, cid) {
            return None;
        }
        let housing = g.city_housing(city);
        if (city.pop as f64) + 1.0 >= housing {
            if let Some(granary) = Self::civ_building(g, pid, cid, "granary") {
                think!(self.journal, Cities, Detail,
                       "{} builds its Granary at its housing", city.name;
                       "population {} against housing {housing:.0}: the city grows before it \
                        builds anything else", city.pop);
                return Some(granary);
            }
        }
        let surplus = g.city_amenity_surplus(city);
        if surplus >= 0 {
            return None;
        }
        if let Some(building) = Self::cheap_amenity_building(g, pid, cid) {
            think!(self.journal, Cities, Detail,
                   "{} builds an Amenity", city.name;
                   "{} short of Amenities: {}", -surplus,
                   crate::reasoning::plain(&format!("{building:?}")));
            return Some(building);
        }
        let builder = Self::luxury_builder_item(g, pid, cid, n_cities, builders)?;
        think!(self.journal, Cities, Detail,
               "{} trains a Builder for a luxury", city.name;
               "{} short of Amenities, and a luxury the empire lacks lies unimproved on our \
                ground with {builders} Builders held", -surplus);
        Some(builder)
    }

    /// The longest an Aqueduct may take under `housing-cap-builds-the-aqueduct`.
    /// A city at its housing grows at a quarter to a half of its rate, so the
    /// district pays for its own turns there; the cap keeps a tiny city from
    /// spending half the game on it.
    pub(super) const HOUSING_CAP_AQUEDUCT_MAX_TURNS: f64 = 25.0;
    /// The smallest city `housing-cap-builds-the-aqueduct` builds for: a
    /// newly founded one reaches its housing on its first improvements.
    const HOUSING_CAP_AQUEDUCT_MIN_POP: i32 = 3;

    /// `housing-cap-builds-the-aqueduct`: the Aqueduct (Rome's Bath) of a city
    /// within one of its housing whose Granary already stands or cannot be
    /// built, on the legal site `housing_reserve_item` uses, when it finishes
    /// within `HOUSING_CAP_AQUEDUCT_MAX_TURNS`. `None` in a city the plan
    /// names threatened, a city attacked within four turns, a city holding a
    /// Settler past half its price, a city under
    /// `HOUSING_CAP_AQUEDUCT_MIN_POP`, and a city that already has one.
    pub(super) fn housing_cap_aqueduct_step(&self, g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let city = &g.cities[&cid];
        let threatened = self.plan_threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4);
        if threatened
            || Self::settler_half_built(g, pid, cid)
            || city.pop < Self::HOUSING_CAP_AQUEDUCT_MIN_POP
        {
            return None;
        }
        let housing = g.city_housing(city);
        if (city.pop as f64) + 1.0 < housing
            || Self::civ_building(g, pid, cid, "granary").is_some()
            || g.city_has_district_family(city, crate::name!("aqueduct"))
        {
            return None;
        }
        let aqueduct = Self::civ_district(g, pid, "aqueduct");
        let pos = g.district_sites(cid, aqueduct).into_iter().min()?;
        let item = Item::District {
            district: aqueduct,
            pos,
        };
        if !g.can_produce(pid, cid, &item) {
            return None;
        }
        let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
            g.item_cost_for(pid, &item) / g.city_yields(cid).production.max(0.5)
        });
        if turns > Self::HOUSING_CAP_AQUEDUCT_MAX_TURNS {
            return None;
        }
        think!(self.journal, Cities, Detail,
               "{} builds its Aqueduct at its housing", city.name;
               "population {} against housing {housing:.0} with its Granary built: the Aqueduct \
                takes no district slot and lifts the water housing, about {turns:.0} turns",
               city.pop);
        Some(item)
    }

    /// See `builders_cover_the_worked_backlog`: the share of the unimproved
    /// worked tiles the charges in hand (and the Builders queued) must cover
    /// before no further Builder is wanted.
    pub(super) const WORKED_BACKLOG_COVER: f64 = 0.6;
    /// See `builders_cover_the_worked_backlog`: no Builder for a backlog
    /// smaller than this many unimproved worked tiles.
    pub(super) const WORKED_BACKLOG_MIN_TILES: usize = 4;
    /// See `builders_cover_the_worked_backlog`: the slowest Builder this step
    /// orders, in standard turns, so only a city that delivers one promptly
    /// spends its queue on it.
    pub(super) const WORKED_BACKLOG_BUILDER_MAX_TURNS: u32 = 8;

    /// See `worked_backlog_builder`: the Builder, journalled.
    fn worked_backlog_builder_step(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        n_cities: usize,
        builders: usize,
    ) -> Option<Item> {
        let (backlog, cover, turns) =
            self.worked_backlog_builder(g, pid, cid, n_cities, builders)?;
        think!(self.journal, Cities, Detail,
               "{} trains a Builder for the worked backlog", g.cities[&cid].name;
               "{backlog} worked tiles stand unimproved against {cover} Builder charges in hand \
                and queued, under {:.0}% cover; about {turns:.0} turns",
               Self::WORKED_BACKLOG_COVER * 100.0);
        if let Some(note) = self.production_tile_quota_note(g, pid) {
            think!(self.journal, Cities, Detail,
                   "{}'s Builder answers the production ground", g.cities[&cid].name; "{note}");
        }
        Some(Item::Unit {
            unit: crate::name!("builder"),
        })
    }

    /// Whether `cid` holds production for a Settler past half its price.
    fn settler_half_built(g: &Game, pid: usize, cid: u32) -> bool {
        let settler = Item::Unit {
            unit: crate::name!("settler"),
        };
        let cost = g.item_cost_for_city(pid, cid, &settler);
        cost > 0.0 && g.item_invested_production(cid, &settler) > 0.5 * cost
    }

    /// The quickest building with an Amenity that `cid` can produce within
    /// `FIRST_CAMPUS_MAX_TURNS` (an Arena in a standing Entertainment
    /// Complex, a Stupa, ...); ties to the name.
    fn cheap_amenity_building(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        g.rules
            .buildings
            .iter()
            .filter(|(_, spec)| spec.amenity > 0.0)
            .map(|(name, _)| Item::Building { building: *name })
            .filter(|item| g.can_produce(pid, cid, item))
            .map(|item| {
                let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
                    g.item_cost_for(pid, &item) / g.city_yields(cid).production.max(0.5)
                });
                (turns, format!("{item:?}"), item)
            })
            .filter(|(turns, _, _)| *turns <= Self::FIRST_CAMPUS_MAX_TURNS)
            .min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)))
            .map(|(_, _, item)| item)
    }

    /// A Builder while the empire holds (or has queued) fewer than the
    /// distinct luxuries it lacks that lie unimproved on its own ground with an
    /// improvement it can already build there -- each one an Amenity in four
    /// cities -- capped at one per two cities, from a city that finishes one
    /// within `LENT_FLOOR_MAX_BUILD_TURNS`.
    pub(super) fn luxury_builder_item(
        g: &Game,
        pid: usize,
        cid: u32,
        n_cities: usize,
        builders: usize,
    ) -> Option<Item> {
        let mut missing: BTreeSet<Name> = BTreeSet::new();
        for city in g.player_city_ids(pid) {
            for pos in &g.cities[&city].owned_tiles {
                let Some(tile) = g.map.get(*pos) else {
                    continue;
                };
                let Some(resource) = tile.resource else {
                    continue;
                };
                if tile.improvement.is_some()
                    || missing.contains(&resource)
                    || !g
                        .rules
                        .resources
                        .get(&resource)
                        .is_some_and(|spec| spec.class == "luxury")
                    || g.resource_access_count(pid, resource.as_str()) > 0
                {
                    continue;
                }
                let connectable = g.valid_improvements(pid, *pos).iter().any(|improvement| {
                    let spec = &g.rules.improvements[improvement];
                    spec.builder_buildable && spec.resources.contains(&resource)
                });
                if connectable {
                    missing.insert(resource);
                }
            }
        }
        if builders >= missing.len().min(n_cities.div_ceil(2)) {
            return None;
        }
        let builder = Item::Unit {
            unit: crate::name!("builder"),
        };
        (g.can_produce(pid, cid, &builder)
            && Self::unit_build_turns(g, pid, cid, "builder")
                <= g.standard_duration(LENT_FLOOR_MAX_BUILD_TURNS) as f64)
            .then_some(builder)
    }

    /// `plaza-in-the-district-list`: whether `cid` is our own original
    /// capital and no city of ours holds a Government Plaza or has one first
    /// in its queue.
    pub(crate) fn plaza_wanted_here(g: &Game, pid: usize, cid: u32) -> bool {
        let city = &g.cities[&cid];
        city.owner == pid
            && city.is_capital
            && city.original_owner == pid
            && Self::plaza_absent(g, pid)
    }

    /// `plaza-in-the-district-list`: no city of ours holds a Government
    /// Plaza or has one first in its queue.
    pub(crate) fn plaza_absent(g: &Game, pid: usize) -> bool {
        !g.cities.values().any(|other| {
            other.owner == pid
                && (g.city_has_district_family(other, crate::name!("government_plaza"))
                    || matches!(
                        other.queue.first(),
                        Some(Item::District { district, .. })
                            if g.district_family(*district) == "government_plaza"
                    ))
        })
    }

    /// `plaza-in-the-district-list`: a Prophet race the empire has entered
    /// but not yet started a Holy Site for goes ahead of the Plaza.
    pub(crate) fn plaza_prophet_race_open(&self, g: &Game, pid: usize) -> bool {
        self.enter_prophet_race
            && !self.skip_prophet_race
            && g.players[pid].religion.is_none()
            && !g.cities.values().any(|other| {
                other.owner == pid
                    && (g.city_has_district_family(other, crate::name!("holy_site"))
                        || matches!(
                            other.queue.first(),
                            Some(Item::District { district, .. })
                                if g.district_family(*district) == "holy_site"
                        ))
            })
    }

    /// `plaza-in-the-district-list`: the empire's first Government Plaza for
    /// `cid` when `plaza_wanted_here`, on the site whose district yields are
    /// highest (ties to the lowest tile), if the city can produce it there.
    pub(crate) fn plaza_item(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        if !Self::plaza_wanted_here(g, pid, cid) {
            return None;
        }
        Self::plaza_site_item(g, pid, cid)
    }

    /// `plaza-in-the-district-list`: a Government Plaza for `cid` on its
    /// highest-yield site (ties to the lowest tile), if it can produce one.
    pub(crate) fn plaza_site_item(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let plaza = Self::civ_district(g, pid, "government_plaza");
        g.district_sites(cid, plaza)
            .into_iter()
            .map(|pos| (g.district_site_yields(plaza, pos).total(), pos))
            .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
            .map(|(_, pos)| Item::District {
                district: Name::new(&plaza),
                pos,
            })
            .filter(|item| g.can_produce(pid, cid, item))
    }

    /// `plaza-in-the-district-list`: the next Government Plaza building for a
    /// city that holds the Plaza -- the Warlord's Throne (the conquest tier:
    /// production in captured cities), then the Grand Master's Chapel (Faith
    /// buys land units). A Plaza with another tier-one building goes straight
    /// to the Chapel; one with both, or a tier the host has not unlocked,
    /// offers nothing.
    ///
    /// The stock tier list's entry point, kept for the tests that pin it; the
    /// decider calls [`Self::plaza_building_item_for`].
    #[cfg(test)]
    pub(crate) fn plaza_building_item(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        Self::plaza_building_item_for(g, pid, cid, false)
    }

    /// [`Self::plaza_building_item`], and under `plaza-serves-the-scientist`
    /// (`science`) the Ancestral Hall, then the Intelligence Agency.
    ///
    /// The conquest tiers serve a seat that takes cities: the Warlord's Throne
    /// pays only on a capture (+20% Production for 5 turns) and a Science seat
    /// captured almost none at Deity (live 2026-11-04/05: 9 of 17 games past
    /// turn 130 built the Throne and the Chapel, none ever the Agency). The
    /// Ancestral Hall has no drawback and serves the expansion gap (+50%
    /// Production toward Settlers in the Plaza's city, a free Builder for every
    /// new city); the Intelligence Agency grants a Spy and a Spy slot and makes
    /// every operation likelier (`BUILDING_GOV_SPIES`, `GovernmentTierRequirement`
    /// Tier2 like the Chapel), which `spies-steal-the-lead` turns into stolen
    /// Eurekas. The Audience Chamber was passed over: its -2 Loyalty in every
    /// city without a Governor is a real risk where loyalty flips already cost
    /// cities. A Plaza already holding another tier-one building goes straight
    /// to the Agency, as the stock list goes to the Chapel.
    pub(crate) fn plaza_building_item_for(
        g: &Game,
        pid: usize,
        cid: u32,
        science: bool,
    ) -> Option<Item> {
        if !g.city_has_district_family(&g.cities[&cid], crate::name!("government_plaza")) {
            return None;
        }
        let tiers = if science {
            ["ancestral_hall", "intelligence_agency"]
        } else {
            ["warlords_throne", "grand_masters_chapel"]
        };
        tiers
            .into_iter()
            .find_map(|building| Self::civ_building(g, pid, cid, building))
    }
}

#[cfg(test)]
mod tests;
