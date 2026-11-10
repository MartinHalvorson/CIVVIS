//! Espionage: spies, their missions, and the counterspies that stop them.
//!
//! A spy's capacity, agents and promotions (`spy_capacity`, `spy_agents`,
//! `available_spy_promotions`, `level_up_spy`); the missions and their odds
//! (`spy_mission_spec`, `spy_success_chance`, `spy_escape_chance`, the
//! stealable-tech and stealable-work checks); the legal actions; and the
//! orders that assign, send and promote spies (`do_assign_spy`,
//! `do_spy_mission`, `do_promote_spy`), down to resolving a mission and
//! rewarding the defender.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    /// Production, duration, offensive level, defensive level, stolen-boost
    /// bonus, and unrestricted-promotion flag for the espionage layer.
    pub fn spy_modifiers(&self, pid: usize) -> (f64, f64, i64, i64, i64, bool) {
        (
            1.0 + self.policy_effect(pid, "spy_production_pct") / 100.0,
            1.0 + self.policy_effect(pid, "spy_time_pct") / 100.0,
            (self.policy_effect(pid, "offensive_spy_level")
                + self.empire_building_sum(pid, |building| {
                    building
                        .effects
                        .get("offensive_spy_level")
                        .copied()
                        .unwrap_or(0.0)
                })) as i64,
            self.policy_effect(pid, "enemy_spy_level") as i64,
            self.policy_effect(pid, "spy_tech_boost_bonus") as i64,
            self.policy_effect(pid, "spy_any_promotion") > 0.0,
        )
    }

    /// How many espionage agents a player actually has.
    ///
    /// ⚠ TWO REPRESENTATIONS, AND THE LIVE BRIDGE ONLY EVER FILLS THE SECOND.
    /// A native CIVVIS Spy is a `self.spies` entry and NOTHING ELSE — the
    /// production arm inserts it and returns before `place_new_unit`, so
    /// `self.units` never holds one. Civilization VI has no such split: a Spy
    /// there is an ordinary `UNIT_SPY`, which is what the mirror imports, so
    /// `self.spies` stays empty for the whole of a live game.
    ///
    /// Everything that asks "am I at spy capacity" read only `self.spies`, so
    /// in a live game the answer was **always zero**. Measured across the 23
    /// completed Civilization VI runs of 2026-08-07/08: `UNIT_SPY` is the
    /// **second most-requested production item in the fleet**, 550 of 5,618
    /// orders (**9.8% of every production decision**), and **458 of them
    /// (84%) were refused as unplayable** — while the empire visibly held one
    /// to four Spies at the time. Each refusal throws that city's production
    /// back to the control mod's fallback ladder, which is the ladder that
    /// builds warriors.
    ///
    /// Taking the maximum is correct in both worlds and cannot over-count: a
    /// native game contributes 0 from the unit census, a live game
    /// contributes 0 from the agent map.
    pub fn spy_agents(&self, pid: usize) -> usize {
        let agents = self.spies.values().filter(|spy| spy.owner == pid).count();
        let units = self
            .units
            .values()
            .filter(|unit| unit.owner == pid && unit.kind == "spy")
            .count();
        agents.max(units)
    }

    pub fn spy_capacity(&self, pid: usize) -> i64 {
        (self.tree_effect(pid, "spy_capacity")
            + self.empire_building_sum(pid, |building| {
                building.effects.get("spy_capacity").copied().unwrap_or(0.0)
            })) as i64
    }

    /// Defensive level penalty applied to an enemy operation at a specific
    /// city tile. Diplomatic Quarter protection is positional; Consulate and
    /// Chancery protection also reaches every owned city with an Encampment.
    pub fn spy_defense_level(&self, cid: u32, target: Pos) -> i64 {
        let Some(city) = self.cities.get(&cid) else {
            return 0;
        };
        let mut levels = 0.0;
        if let Some(quarter) =
            self.city_district_family_position(city, crate::name!("diplomatic_quarter"))
        {
            if !self.map.tiles[&quarter].pillaged
                && (target == quarter || self.nbrs(quarter).contains(&target))
            {
                levels += self.rules.districts["diplomatic_quarter"]
                    .effects
                    .get("spy_defense_levels")
                    .copied()
                    .unwrap_or(0.0);
            }
        }
        let protected_by_consulate = self
            .cities
            .values()
            .filter(|source| source.owner == city.owner)
            .filter(|source| {
                source.id == city.id
                    || self.city_has_active_district_family(city, crate::name!("encampment"))
            })
            .map(|source| self.city_building_effect(source, "enemy_spy_level_reduction"))
            .sum::<f64>();
        levels += protected_by_consulate;
        levels as i64
    }

    /// Science granted when this civilization captures or kills an enemy
    /// Spy, parameterized by the defeated Spy's level for the espionage
    /// resolver and external agents.
    pub fn spy_capture_science(&self, pid: usize, spy_level: i64) -> f64 {
        self.empire_building_sum(pid, |building| {
            building
                .effects
                .get("spy_capture_or_kill_science_per_level")
                .copied()
                .unwrap_or(0.0)
        }) * spy_level.max(0) as f64
    }

    /// The espionage promotions the live bridge has to spell for the host.
    ///
    /// Public because the live bridge translates these to Civilization VI's own
    /// identifiers and must not keep a second copy: a spy promotion is spelled
    /// `PROMOTION_SPY_<NAME>` on the host, and `civ6_unit_promotion_name` reads
    /// this list to decide which names take that prefix. That function maps a
    /// bare `&str` with no ruleset in hand, so the list stays a constant.
    ///
    /// ⚠ IT IS NO LONGER THE ROSTER. The offer generator reads
    /// [`Self::spy_promotion_roster`] out of `data/promotions.json`, because a
    /// hardcoded roster is how added content becomes unreachable — CIVVIS has
    /// shipped that bug twice. `espionage_promotions_are_the_shipped_roster`
    /// asserts this constant still names exactly the ruleset's espionage class,
    /// so a promotion added to the data cannot silently go out misspelled.
    pub const SPY_PROMOTIONS: [&'static str; 17] = [
        "ace_driver",
        "cat_burglar",
        "con_artist",
        "covert_action",
        "demolitions",
        "disguise",
        "guerrilla_leader",
        "license_to_kill",
        "linguist",
        "polygraph",
        "quartermaster",
        "rocket_scientist",
        "satchel_charges",
        "seduction",
        "smear_campaign",
        "surveillance",
        "technologist",
    ];

    /// Every espionage promotion the ruleset declares, in name order.
    ///
    /// `SpecMap` keeps its keys sorted, so this walks the class in exactly the
    /// order the old constant listed it and the deterministic three-card offer
    /// below is unchanged.
    pub(crate) fn spy_promotion_roster(&self) -> impl Iterator<Item = &str> {
        self.rules
            .promotions
            .iter()
            .filter(|(_, spec)| spec.class == "espionage")
            .map(|(name, _)| name.as_str())
    }

    /// What a Spy's own promotions add up to for one espionage effect.
    ///
    /// The magnitudes live in `data/promotions.json` beside the promotion that
    /// grants them, so a value read here is one the fidelity audit compares
    /// against the shipped game rather than a number buried in this file.
    pub(crate) fn spy_promotion_effect(&self, spy: &Spy, effect: &str) -> f64 {
        spy.promotions
            .iter()
            .filter_map(|name| self.rules.promotions.get(name.as_str()))
            .filter_map(|spec| spec.effects.get(effect))
            .sum()
    }

    pub(super) fn spy_needs_promotion(spy: &Spy) -> bool {
        spy.level.max(0) as usize > spy.promotions.len() && spy.promotions.len() < 3
    }

    /// Promotion offers are stable across saves and action enumeration. Non-
    /// State Actors exposes the complete tree; otherwise each earned level
    /// receives a deterministic three-node offer, mirroring Civ VI's choice.
    pub(crate) fn available_spy_promotions(&self, spy_id: u32) -> Vec<Name> {
        let Some(spy) = self.spies.get(&spy_id) else {
            return Vec::new();
        };
        if !Self::spy_needs_promotion(spy) || spy.captured_by.is_some() {
            return Vec::new();
        }
        // A live Spy's menu is the host's own (`HostUnitFacts::spy_promotions`):
        // exactly what the host offers, less what it holds or refused, so a
        // choice never goes out that the host would turn away.
        if let Some(menu) = self
            .host_unit_facts
            .get(&spy_id)
            .and_then(|facts| facts.spy_promotions.as_ref())
        {
            let refused = self.blocked_promotions.get(&spy_id);
            return menu
                .iter()
                .filter(|name| !spy.promotions.contains(name.as_str()))
                .filter(|name| !refused.is_some_and(|refused| refused.contains(*name)))
                .copied()
                .collect();
        }
        let available: Vec<&str> = self
            .spy_promotion_roster()
            .filter(|promotion| !spy.promotions.contains(*promotion))
            .collect();
        if self.spy_modifiers(spy.owner).5 {
            return available.into_iter().map(Name::new).collect();
        }
        let start = (spy.id as usize + spy.level.max(0) as usize * 5) % available.len().max(1);
        (0..available.len().min(3))
            .map(|offset| Name::new(available[(start + offset) % available.len()]))
            .collect()
    }

    pub(super) fn spy_mission_spec(kind: &str) -> Option<(u32, f64)> {
        match kind {
            "gain_sources" | "listening_post" => Some((8, 1.0)),
            "counterspy" => Some((16, 1.0)),
            "fabricate_scandal" => Some((16, 0.56)),
            "siphon_funds" | "foment_unrest" => Some((8, 0.56)),
            "steal_tech_boost" | "sabotage_production" | "neutralize_governor" => Some((8, 0.35)),
            "great_work_heist" | "disrupt_rocketry" | "breach_dam" => Some((8, 0.20)),
            "recruit_partisans" => Some((8, 0.10)),
            _ => None,
        }
    }

    /// The levels a Spy's promotions add to one named operation.
    ///
    /// Civ VI attaches a `MODIFIER_PLAYER_UNIT_ADJUST_SPY_OPERATION_CHANCE`
    /// naming an `OperationType` to each of the ten operation promotions, all
    /// of them worth `Amount = 2`. CIVVIS spells that pairing as an effect key
    /// `mission_level_<operation>` on the promotion itself, so the mission a
    /// promotion sharpens is data, not a match arm here: a new espionage
    /// promotion needs no edit to this file to take effect.
    ///
    /// ⚠ THE KEY IS BUILT WITH `format!` ON PURPOSE. `tools/civvis_inert.py`
    /// credits an effect key to the engine when it appears as a literal or
    /// when a `format!` template could build it, and nothing else — a
    /// `strip_prefix("mission_level_")` consumer is invisible to it, so all
    /// ten keys were reported as data the engine ignores. The allocation is
    /// on the espionage path, which runs a handful of times a turn.
    fn spy_mission_promotion_level(&self, spy: &Spy, kind: &str) -> i64 {
        if spy.promotions.is_empty() {
            return 0;
        }
        self.spy_promotion_effect(spy, &format!("mission_level_{kind}")) as i64
    }

    fn spy_city_has_stealable_tech(&self, owner: usize, target: usize) -> bool {
        self.players[target]
            .techs
            .iter()
            .any(|tech| !self.players[owner].techs.contains(tech))
    }

    pub(super) fn spy_city_has_stealable_work(&self, owner: usize, cid: u32) -> bool {
        self.housed_great_works(self.cities[&cid].owner)
            .get(&cid)
            .is_some_and(|works| {
                works.iter().any(|(kind, count)| {
                    *count > 0 && self.can_house_additional_great_work(owner, kind)
                })
            })
    }

    fn active_spy_target_position(&self, city: &City, family: impl AsName) -> Option<Pos> {
        city.districts.iter().find_map(|(district, position)| {
            (self.district_is_family(district, family)
                && self.district_is_active(city, district, *position))
            .then_some(*position)
        })
    }

    pub(super) fn spy_mission_already_running(&self, owner: usize, cid: u32, kind: &str) -> bool {
        self.spies.values().any(|spy| {
            spy.owner == owner
                && spy
                    .mission
                    .as_ref()
                    .is_some_and(|mission| mission.city == cid && mission.kind == kind)
        })
    }

    pub(super) fn spy_operation_actions(&self, spy: &Spy, city: &City) -> Vec<Action> {
        let mut actions = Vec::new();
        let offensive = city.owner != spy.owner;
        // The host's own menu for this Spy, when it crossed: the operations
        // `UnitManager.CanStartOperation(spy, op, cityPlot, false, true)`
        // answers yes to from the city it stands in
        // (`EspionageChooser.lua:196-213`), as CIVVIS mission kinds. A mission
        // the host would not let it start is not offered, whatever the
        // board's model of the city says.
        let host_menu = self
            .host_unit_facts
            .get(&spy.id)
            .and_then(|facts| facts.spy_missions.as_ref());
        let mut add = |mission: &str, target: Pos| {
            if !host_menu.is_some_and(|menu| !menu.contains(mission))
                && !self.congress_effect_active("espionage_pact", "B", mission)
                && !self.spy_mission_already_running(spy.owner, city.id, mission)
            {
                actions.push(Action::SpyMission {
                    spy: spy.id,
                    mission: mission.to_string(),
                    target,
                });
            }
        };
        if !offensive {
            add("counterspy", city.pos);
            for (_, position) in &city.districts {
                if !self.map.tiles[position].pillaged {
                    add("counterspy", *position);
                }
            }
            return actions;
        }
        if self.are_allied(spy.owner, city.owner) {
            return actions;
        }
        if self.players[city.owner].is_minor {
            add("fabricate_scandal", city.pos);
            return actions;
        }
        if spy.sources_city != Some(city.id) || spy.sources_until <= self.turn {
            add("gain_sources", city.pos);
        }
        add("listening_post", city.pos);
        add("foment_unrest", city.pos);
        if self.city_governor_active(city.owner, city.id) {
            add("neutralize_governor", city.pos);
        }
        for (family, mission) in [
            ("commercial_hub", "siphon_funds"),
            ("campus", "steal_tech_boost"),
            ("theater_square", "great_work_heist"),
            ("industrial_zone", "sabotage_production"),
            ("neighborhood", "recruit_partisans"),
            ("spaceport", "disrupt_rocketry"),
            ("dam", "breach_dam"),
        ] {
            // The board's own district, else (under `spy_targets_read_the_host`)
            // the plot the host aims this mission at.
            let Some(target) = self
                .active_spy_target_position(city, Name::new(family))
                .or_else(|| {
                    self.spy_targets_read_the_host
                        .then(|| {
                            self.host_unit_facts
                                .get(&spy.id)
                                .and_then(|facts| facts.spy_targets.as_ref())
                                .and_then(|targets| targets.get(mission))
                                .map(|target| target.pos)
                        })
                        .flatten()
                })
            else {
                continue;
            };
            let useful = match mission {
                "steal_tech_boost" => self.spy_city_has_stealable_tech(spy.owner, city.owner),
                "great_work_heist" => {
                    self.spy_city_has_stealable_work(spy.owner, city.id)
                        // See `heist_reads_the_host_menu`.
                        || (self.heist_reads_the_host_menu
                            && host_menu.is_some_and(|menu| menu.contains("great_work_heist")))
                }
                "recruit_partisans" => self.barb_pid.is_some(),
                _ => true,
            };
            if useful {
                add(mission, target);
            }
        }
        actions
    }

    /// Agent-local action generation is also used by both built-in AIs, so
    /// espionage decisions do not require rebuilding the complete empire
    /// action space for each Spy.
    pub(crate) fn legal_spy_actions(&self, pid: usize, spy_id: u32) -> Vec<Action> {
        if self.is_finished() || self.current != pid {
            return Vec::new();
        }
        let Some(spy) = self.spies.get(&spy_id) else {
            return Vec::new();
        };
        if spy.owner != pid || spy.captured_by.is_some() || spy.mission.is_some() {
            return Vec::new();
        }
        if Self::spy_needs_promotion(spy) {
            let offers = self.available_spy_promotions(spy_id);
            // A host menu that leaves nothing to choose (every pick held or
            // refused) owes nothing this turn: the Spy keeps its missions
            // rather than standing idle behind a promotion it cannot take.
            let host_menu = self
                .host_unit_facts
                .get(&spy_id)
                .is_some_and(|facts| facts.spy_promotions.is_some());
            if !offers.is_empty() || !host_menu {
                return offers
                    .into_iter()
                    .map(|promotion| Action::PromoteSpy {
                        spy: spy_id,
                        promotion: Name::new(&promotion),
                    })
                    .collect();
            }
        }
        if spy.ready_turn > self.turn {
            return Vec::new();
        }
        let mut actions = Vec::new();
        for city in self.cities.values().filter(|city| {
            self.players[city.owner].alive
                && !self.players[city.owner].is_barbarian
                && Some(city.id) != spy.city
                && (city.owner == pid || !self.are_allied(pid, city.owner))
                && (city.owner == pid || self.players[pid].explored.contains(&city.pos))
        }) {
            actions.push(Action::AssignSpy {
                spy: spy_id,
                city: city.id,
            });
        }
        if let Some(city) = spy.city.and_then(|city| self.cities.get(&city)) {
            actions.extend(self.spy_operation_actions(spy, city));
        }
        actions
    }

    fn counterspy_protects(&self, counterspy: &Spy, city: u32, target: Pos) -> bool {
        let Some(mission) = counterspy.mission.as_ref() else {
            return false;
        };
        mission.kind == "counterspy"
            && mission.city == city
            && (self.spy_promotion_effect(counterspy, "counterspy_entire_city") > 0.0
                || mission.target == target
                || self.nbrs(mission.target).contains(&target))
    }

    pub(super) fn defending_counterspy(
        &self,
        defender: usize,
        city: u32,
        target: Pos,
    ) -> Option<u32> {
        self.spies
            .values()
            .filter(|spy| {
                spy.owner == defender
                    && spy.captured_by.is_none()
                    && self.counterspy_protects(spy, city, target)
            })
            .max_by_key(|spy| {
                (
                    spy.level
                        + self.spy_promotion_effect(spy, "counterspy_defense_level") as i64
                        + self.spy_promotion_effect(spy, "counterspy_entire_city") as i64,
                    std::cmp::Reverse(spy.id),
                )
            })
            .map(|spy| spy.id)
    }

    /// Net offensive level after sources, promotions, policy/building hooks,
    /// governors, Diplomatic Quarter defenses, and active Counterspies.
    pub(crate) fn spy_effective_level(&self, spy_id: u32, mission: &SpyMission) -> i64 {
        let Some(spy) = self.spies.get(&spy_id) else {
            return 0;
        };
        let Some(city) = self.cities.get(&mission.city) else {
            return spy.level;
        };
        let mut level = spy.level + self.spy_modifiers(spy.owner).2;
        if spy.sources_city == Some(city.id) && spy.sources_until > self.turn {
            level += 2;
        }
        level += self.spy_mission_promotion_level(spy, &mission.kind);
        if self.congress_effect_active("espionage_pact", "A", &mission.kind) {
            level += 2;
        }
        // Quartermaster and Polygraph are Civ VI's two BOOST_ALL_SPIES
        // promotions: one Counterspy carrying either lifts (or drops) every
        // other agent by its own magnitude. They do not stack with themselves,
        // so the strongest one on the board is what applies.
        level += self.counterspy_promotion_effect(spy.owner, "allied_spy_level") as i64;
        if city.owner == spy.owner {
            return level;
        }
        level += self.spy_modifiers(city.owner).3;
        level += self.governor_effect(city.owner, city.id, "enemy_spy_level") as i64;
        level -= self.spy_defense_level(city.id, mission.target);
        level -= self.counterspy_promotion_effect(city.owner, "enemy_spy_level") as i64;
        if let Some(counterspy) = self.defending_counterspy(city.owner, city.id, mission.target) {
            let defender = &self.spies[&counterspy];
            level -= 1 + defender.level;
            level -= self.spy_promotion_effect(defender, "counterspy_defense_level") as i64;
        }
        level
    }

    /// The odds a detected Spy gets out of the city it was caught in.
    ///
    /// Civ VI's Ace Driver is an `ESCAPE_BOOST` worth four, and it enters here
    /// exactly where the agent's own level does.
    pub(crate) fn spy_escape_chance(&self, spy: &Spy) -> f64 {
        let ace = self.spy_promotion_effect(spy, "escape_level");
        (0.35 + 0.10 * (spy.level.max(0) as f64 + ace)).min(0.95)
    }

    /// The strongest value of one effect carried by an active Counterspy of
    /// this civilization. Civ VI's `BOOST_ALL_SPIES` promotions apply from the
    /// Counterspy to every other agent, and two Counterspies with the same
    /// promotion do not stack.
    fn counterspy_promotion_effect(&self, owner: usize, effect: &str) -> f64 {
        self.spies
            .values()
            .filter(|spy| {
                spy.owner == owner
                    && spy.captured_by.is_none()
                    && spy
                        .mission
                        .as_ref()
                        .is_some_and(|active| active.kind == "counterspy")
            })
            .map(|spy| self.spy_promotion_effect(spy, effect))
            .fold(0.0, f64::max)
    }

    pub(crate) fn spy_success_chance(&self, spy_id: u32, mission: &SpyMission) -> f64 {
        let base = Self::spy_mission_spec(&mission.kind)
            .map(|(_, chance)| chance)
            .unwrap_or(0.0);
        if base >= 1.0 {
            return 1.0;
        }
        (base + 0.10 * self.spy_effective_level(spy_id, mission) as f64).clamp(0.05, 0.95)
    }

    fn level_up_spy(&mut self, spy_id: u32) {
        let mut mastered = None;
        if let Some(spy) = self.spies.get_mut(&spy_id) {
            if spy.level < 3 && !Self::spy_needs_promotion(spy) {
                spy.level += 1;
                if spy.level == 3 {
                    mastered = Some(spy.owner);
                }
            }
        }
        if let Some(owner) = mastered {
            let master_spies = self.players[owner]
                .counters
                .entry("master_spies".to_string())
                .or_insert(0);
            let first = *master_spies == 0;
            *master_spies += 1;
            self.add_historic_moment(
                owner,
                if first {
                    "MOMENT_SPY_MAX_LEVEL_FIRST"
                } else {
                    "MOMENT_SPY_MAX_LEVEL"
                },
            );
        }
    }

    pub(super) fn spy_home_city(&self, pid: usize) -> Option<u32> {
        self.cities
            .values()
            .filter(|city| city.owner == pid)
            .min_by_key(|city| (!self.city_capital_flag(city), city.id))
            .map(|city| city.id)
    }

    fn spy_return_home(&mut self, spy_id: u32, delay: u32) {
        let Some(owner) = self.spies.get(&spy_id).map(|spy| spy.owner) else {
            return;
        };
        let home = self.spy_home_city(owner);
        let ready_turn = self.turn + self.standard_duration(delay);
        if let Some(spy) = self.spies.get_mut(&spy_id) {
            spy.city = home;
            spy.ready_turn = ready_turn;
            spy.mission = None;
            spy.sources_city = None;
            spy.sources_until = 0;
            spy.captured_by = None;
        }
    }

    pub(super) fn do_assign_spy(
        &mut self,
        pid: usize,
        spy_id: u32,
        cid: u32,
    ) -> Result<(), String> {
        let action = Action::AssignSpy {
            spy: spy_id,
            city: cid,
        };
        if !self.legal_spy_actions(pid, spy_id).contains(&action) {
            return Err("Spy cannot be assigned to that city".into());
        }
        let spy = self.spies[&spy_id].clone();
        let destination = &self.cities[&cid];
        let distance = spy
            .city
            .and_then(|old| self.cities.get(&old))
            .map(|old| self.wdist(old.pos, destination.pos).max(0) as u32)
            .unwrap_or(0);
        let mut travel = (1 + distance / 6).min(5);
        let disguise = self.spy_promotion_effect(&spy, "travel_turns_max");
        if destination.owner != pid && disguise > 0.0 {
            travel = travel.min(disguise as u32);
        }
        let ready_turn = self.turn + self.standard_duration(travel);
        let spy = self.spies.get_mut(&spy_id).unwrap();
        spy.city = Some(cid);
        spy.ready_turn = ready_turn;
        spy.mission = None;
        spy.sources_city = None;
        spy.sources_until = 0;
        Ok(())
    }

    pub(super) fn do_spy_mission(
        &mut self,
        pid: usize,
        spy_id: u32,
        kind: &str,
        target: Pos,
    ) -> Result<(), String> {
        let action = Action::SpyMission {
            spy: spy_id,
            mission: kind.to_string(),
            target,
        };
        if !self.legal_spy_actions(pid, spy_id).contains(&action) {
            return Err("Spy mission is unavailable at that target".into());
        }
        let (base_duration, _) =
            Self::spy_mission_spec(kind).ok_or_else(|| "unknown Spy mission".to_string())?;
        let spy = self.spies[&spy_id].clone();
        let linguist = 1.0 + self.spy_promotion_effect(&spy, "mission_time_pct") / 100.0;
        let duration = self.standard_duration(
            ((base_duration as f64 * self.spy_modifiers(pid).1 * linguist).ceil() as u32).max(1),
        );
        let city = spy.city.ok_or_else(|| "Spy is not in a city".to_string())?;
        let city_owner = self.cities[&city].owner;
        self.break_promises_on_spying(pid, city_owner);
        self.spies.get_mut(&spy_id).unwrap().mission = Some(SpyMission {
            kind: kind.to_string(),
            city,
            target,
            started: self.turn,
            ends: self.turn + duration,
        });
        Ok(())
    }

    pub(super) fn do_promote_spy(
        &mut self,
        pid: usize,
        spy_id: u32,
        promotion: &str,
    ) -> Result<(), String> {
        if self.spies.get(&spy_id).is_none_or(|spy| spy.owner != pid)
            || !self
                .available_spy_promotions(spy_id)
                .iter()
                .any(|available| available == promotion)
        {
            return Err("Spy promotion is unavailable".into());
        }
        self.spies
            .get_mut(&spy_id)
            .unwrap()
            .promotions
            .insert(promotion.to_string());
        Ok(())
    }

    fn steal_tech_boosts(&mut self, attacker: usize, defender: usize, count: usize) {
        let candidates: Vec<Name> = self.players[defender]
            .techs
            .iter()
            .filter(|tech| {
                !self.players[attacker].techs.contains(*tech)
                    && !self.players[attacker].boosted_techs.contains(*tech)
            })
            .take(count)
            .cloned()
            .collect();
        for tech in candidates {
            let cost = self.tech_cost(tech.as_str());
            let fraction = self.node_boost_frac(attacker, tech.as_str(), true);
            let player = &mut self.players[attacker];
            player.boosted_techs.insert(tech);
            if player.research.as_deref() == Some(tech.as_str()) {
                player.research_progress += cost * fraction;
            }
        }
    }

    fn steal_great_work(&mut self, attacker: usize, defender: usize, cid: u32) {
        let works = self
            .housed_great_works(defender)
            .get(&cid)
            .cloned()
            .unwrap_or_default();
        let kind = ["writing", "art", "religious_art", "artifact", "music"]
            .into_iter()
            .find(|kind| {
                works.get(*kind).copied().unwrap_or(0) > 0
                    && self.can_house_additional_great_work(attacker, kind)
            });
        let Some(kind) = kind else {
            return;
        };
        self.move_great_work(defender, attacker, kind);
    }

    fn spawn_partisans(&mut self, cid: u32) {
        let Some(barbarian) = self.barb_pid else {
            return;
        };
        let era = self.world_era;
        let kind = if era >= 6 {
            "modern_at"
        } else if era >= 5 {
            "at_crew"
        } else if era >= 3 {
            "pike_and_shot"
        } else if era >= 2 {
            "pikeman"
        } else {
            "spearman"
        };
        let center = self.cities[&cid].pos;
        let positions: Vec<Pos> =
            self.nbrs(center)
                .into_iter()
                .filter(|position| {
                    self.map.get(*position).is_some_and(|tile| {
                        self.rules.is_passable(tile) && !self.rules.is_water(tile)
                    }) && !self
                        .unit_ids_at(*position)
                        .iter()
                        .any(|unit| self.rules.units[self.units[unit].kind].class == "military")
                })
                .take(2)
                .collect();
        for position in positions {
            self.spawn_unit(kind, barbarian, position);
        }
    }

    pub(super) fn apply_spy_mission_effect(
        &mut self,
        spy_id: u32,
        mission: &SpyMission,
        undetected: bool,
    ) {
        let Some(spy) = self.spies.get(&spy_id).cloned() else {
            return;
        };
        let Some(city) = self.cities.get(&mission.city).cloned() else {
            return;
        };
        let defender = city.owner;
        match mission.kind.as_str() {
            "gain_sources" => {
                let sources_until = self.turn + self.standard_duration(24);
                let spy = self.spies.get_mut(&spy_id).unwrap();
                spy.sources_city = Some(city.id);
                spy.sources_until = sources_until;
            }
            "siphon_funds" => {
                let amount = (self.city_yields(city.id).gold * 8.0)
                    .max(0.0)
                    .min(self.players[defender].gold.max(0.0));
                self.players[defender].gold -= amount;
                self.players[spy.owner].gold += amount;
            }
            "steal_tech_boost" => {
                let bonus = if undetected {
                    self.spy_modifiers(spy.owner).4.max(0) as usize
                } else {
                    0
                };
                self.steal_tech_boosts(spy.owner, defender, 1 + bonus);
            }
            "great_work_heist" => self.steal_great_work(spy.owner, defender, city.id),
            "sabotage_production" => {
                let buildings: Vec<Name> = city
                    .buildings
                    .iter()
                    .filter(|building| {
                        self.rules.buildings[building]
                            .district
                            .is_some_and(|family| {
                                self.district_is_family(
                                    Name::new(&family),
                                    crate::name!("industrial_zone"),
                                )
                            })
                    })
                    .cloned()
                    .collect();
                self.cities
                    .get_mut(&city.id)
                    .unwrap()
                    .pillaged_buildings
                    .extend(buildings);
            }
            "recruit_partisans" => self.spawn_partisans(city.id),
            "foment_unrest" => {
                // Shipped ESPIONAGE_FOMENT_UNREST_BASE_LOYALTY_CHANGE -15 plus
                // ESPIONAGE_FOMENT_UNREST_LEVEL_LOYALTY_CHANGE -5 per level.
                self.cities.get_mut(&city.id).unwrap().loyalty =
                    (city.loyalty - 15.0 - 5.0 * spy.level.max(0) as f64).max(0.0);
            }
            "neutralize_governor" => {
                // Shipped ESPIONAGE_NEUTRALIZE_GOVERNOR_BASE_TURNS is 6 flat.
                // Every mission whose effect scales with the Spy's level has
                // its own _LEVEL_ parameter beside the base — Foment Unrest and
                // Fabricate Scandal both do; this one has no such row, so the
                // level term was invention.
                let until = self.turn + self.standard_duration(6);
                for state in self.players[defender]
                    .governor_roster
                    .values_mut()
                    .filter(|state| state.city == Some(city.id))
                {
                    state.disabled_until = state.disabled_until.max(until);
                }
            }
            "disrupt_rocketry" => {
                self.map.tiles.get_mut(&mission.target).unwrap().pillaged = true;
                let city = self.cities.get_mut(&city.id).unwrap();
                if city.queue.first().is_some_and(|item| {
                    matches!(item, Item::Project { project } if self.rules.projects.get(project.as_str()).is_some_and(|spec| spec.district == Some(crate::name!("spaceport"))))
                }) {
                    city.production = 0.0;
                }
                city.production_progress.retain(|key, _| {
                    let Some(project) = key.strip_prefix("project:") else {
                        return true;
                    };
                    self.rules
                        .projects
                        .get(project)
                        .is_none_or(|spec| spec.district != Some(crate::name!("spaceport")))
                });
            }
            "breach_dam" => {
                self.map.tiles.get_mut(&mission.target).unwrap().pillaged = true;
                let floodplains: Vec<Pos> = city
                    .owned_tiles
                    .iter()
                    .copied()
                    .filter(|position| {
                        matches!(
                            self.map.tiles[position].feature.as_deref(),
                            Some("floodplains" | "grassland_floodplains" | "plains_floodplains")
                        )
                    })
                    .collect();
                self.resolve_flood(&floodplains);
            }
            "fabricate_scandal" => {
                let victim = self
                    .players
                    .iter()
                    .filter(|player| {
                        player.id != spy.owner
                            && player.alive
                            && !player.is_minor
                            && !player.is_barbarian
                    })
                    .map(|player| (self.envoys_at(player.id, defender), player.id))
                    .filter(|(envoys, _)| *envoys > 0)
                    .max_by_key(|(envoys, player)| (*envoys, std::cmp::Reverse(*player)))
                    .map(|(_, player)| player);
                if let Some(victim) = victim {
                    // Shipped ESPIONAGE_FABRICATE_SCANDAL_BASE_ENVOYS_REMOVED 2
                    // plus _LEVEL_ENVOYS_REMOVED 1 per level — the same
                    // base-plus-level shape Foment Unrest already follows.
                    let remove = 2 + spy.level.max(0);
                    if let Some((_, envoys)) = self.players[victim]
                        .envoys
                        .iter_mut()
                        .find(|(minor, _)| *minor == defender)
                    {
                        *envoys = (*envoys - remove).max(0);
                    }
                }
            }
            _ => {}
        }
    }

    fn reward_defending_spy(
        &mut self,
        defender: usize,
        defender_spy: Option<u32>,
        defeated_level: i64,
    ) {
        if let Some(spy) = defender_spy {
            self.level_up_spy(spy);
        }
        self.players[defender].research_overflow +=
            self.spy_capture_science(defender, defeated_level + 1);
    }

    pub(super) fn resolve_spy_mission(&mut self, spy_id: u32) {
        let Some(spy) = self.spies.get(&spy_id).cloned() else {
            return;
        };
        let Some(mission) = spy.mission.clone() else {
            return;
        };
        let Some(defender) = self.cities.get(&mission.city).map(|city| city.owner) else {
            self.spy_return_home(spy_id, 1);
            return;
        };
        if matches!(mission.kind.as_str(), "counterspy" | "listening_post") {
            self.spies.get_mut(&spy_id).unwrap().mission = None;
            return;
        }
        if mission.kind == "gain_sources" {
            self.apply_spy_mission_effect(spy_id, &mission, true);
            self.spies.get_mut(&spy_id).unwrap().mission = None;
            return;
        }
        // An operation outlives the border it was launched across: the target
        // city can change hands while the operative is still inside it. When
        // it is the spy's own civilization that took the city there is nobody
        // left to steal from, and being "captured" by your own side would pin
        // that capacity slot shut for the rest of the game — the exchange that
        // frees a captive needs a counterparty, and no civilization is its
        // own. Counterspy and listening posts have already returned above;
        // they belong in friendly cities.
        if defender == spy.owner || self.same_team(defender, spy.owner) {
            self.spy_return_home(spy_id, 1);
            return;
        }
        let success = self.rng.chance(self.spy_success_chance(spy_id, &mission));
        let detected = self.rng.chance(if success { 0.25 } else { 0.70 });
        if success {
            self.apply_spy_mission_effect(spy_id, &mission, !detected);
            self.level_up_spy(spy_id);
            // Bodyguard of Lies pays for offensive work only — a counterspy
            // succeeding at home is not a Historic Moment.
            if defender != spy.owner {
                self.dedication_trigger(spy.owner, "spy_success", 1);
            }
        }
        if !detected {
            if let Some(spy) = self.spies.get_mut(&spy_id) {
                spy.mission = None;
            }
            return;
        }
        self.add_grievances(defender, spy.owner, 25.0);
        let counterspy = self.defending_counterspy(defender, mission.city, mission.target);
        let escape = self.spy_escape_chance(&spy);
        if self.rng.chance(escape) {
            self.spy_return_home(spy_id, 2);
            return;
        }
        let captured = counterspy.is_some() || self.rng.chance(0.55);
        self.reward_defending_spy(defender, counterspy, spy.level);
        if captured {
            if let Some(spy) = self.spies.get_mut(&spy_id) {
                spy.mission = None;
                spy.sources_city = None;
                spy.sources_until = 0;
                spy.captured_by = Some(defender);
                spy.city = Some(mission.city);
                spy.ready_turn = u32::MAX;
            }
        } else {
            self.spies.remove(&spy_id);
        }
    }

    pub(crate) fn can_house_additional_great_work(&self, pid: usize, kind: &str) -> bool {
        self.can_house_great_works(pid, kind, 1)
    }

    /// Move one work of a kind between players (spy theft, trade deals),
    /// newest piece first, counters and pieces in lockstep.
    pub(super) fn move_great_work(&mut self, from: usize, to: usize, kind: &str) {
        let key = format!("great_work:{kind}");
        *self.players[from].counters.entry(key.clone()).or_insert(0) -= 1;
        *self.players[to].counters.entry(key).or_insert(0) += 1;
        if let Some(index) = self.players[from]
            .great_work_pieces
            .iter()
            .rposition(|piece| piece.kind == kind)
        {
            let piece = self.players[from].great_work_pieces.remove(index);
            self.players[to].great_work_pieces.push(piece);
        }
    }
}
