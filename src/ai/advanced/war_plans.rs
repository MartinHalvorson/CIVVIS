//! War plans: choosing a war, keeping it, and what it costs.
//!
//! `choose_war_plan` and `maintain_war_plan`, and what they read: the
//! unit catalog and what the war can field (unlocks, predecessors, the
//! strongest current melee, staging routes, breach requirements, remaining
//! research), affordability and the treasury, war estimates, aborts and
//! captures, the early-rush victim, `campaign_city_value`, and nuclear
//! targets and goals. `may_form_war_plan` stays in `advanced.rs` for now
//! because an open PR edits it.
//!
//! Moved verbatim out of the one `impl AdvancedAi` block in `advanced.rs`
//! (2026-11-05). Methods the rest of the controller calls are `pub(super)`;
//! the code is otherwise unchanged.

use super::*;

impl AdvancedAi {
    /// Actual unit names available to this civilization, with generic units
    /// replaced by their uniques and foreign uniques removed. Keeping this in
    /// one catalog makes breakthrough selection, predecessor discovery,
    /// production, and upgrade reservation agree on the same upgrade graph.
    pub(super) fn player_unit_catalog(g: &Game, pid: usize) -> Vec<Name> {
        let civ = g.players[pid].civ.as_str();
        // `Game::player_unit_replacement` answers one unit by scanning the
        // whole roster, so asking it for every unit made the catalog
        // quadratic. One pass records, for each base unit, the first unique
        // this player owns that replaces it, in the same roster order that
        // function's `find` walks, under its same arena rule: every entry
        // resolves to the unit it would have named.
        let mut replacements: BTreeMap<Name, Name> = BTreeMap::new();
        if !(g.is_arena() && !g.tactics.unique_units) {
            for (name, spec) in &g.rules.units {
                if let (Some(base), Some(owner)) = (spec.replaces, spec.unique_to.as_deref()) {
                    if g.owns_civ_unique(pid, owner) {
                        replacements.entry(base).or_insert(*name);
                    }
                }
            }
        }
        let mut units = BTreeSet::new();
        for (name, spec) in &g.rules.units {
            if !spec.buildable || spec.unique_to.as_deref().is_some_and(|owner| owner != civ) {
                continue;
            }
            let actual = replacements.get(name).copied().unwrap_or(*name);
            let actual_spec = &g.rules.units[actual];
            if actual_spec.buildable
                && actual_spec
                    .unique_to
                    .as_deref()
                    .is_none_or(|owner| owner == civ)
            {
                units.insert(actual);
            }
        }
        units.into_iter().collect()
    }

    pub(super) fn war_unit_unlocked(g: &Game, pid: usize, unit: Name) -> bool {
        let spec = &g.rules.units[unit];
        spec.tech
            .is_none_or(|tech| g.players[pid].techs.contains(&tech))
            && spec
                .civic
                .is_none_or(|civic| g.players[pid].civics.contains(&civic))
            && !g.unit_is_obsolete(pid, unit)
    }

    pub(super) fn war_direct_predecessor(g: &Game, pid: usize, assault: Name) -> Option<Name> {
        Self::player_unit_catalog(g, pid)
            .into_iter()
            .filter(|unit| Self::war_unit_unlocked(g, pid, *unit))
            .filter(|unit| Self::war_unit_successor(g, pid, *unit) == Some(assault))
            .max_by(|left, right| {
                g.rules.units[*left]
                    .strength
                    .total_cmp(&g.rules.units[*right].strength)
                    .then_with(|| right.cmp(left))
            })
    }

    fn war_strongest_current_melee(g: &Game, pid: usize) -> Option<Name> {
        Self::player_unit_catalog(g, pid)
            .into_iter()
            .filter(|unit| {
                let spec = &g.rules.units[*unit];
                spec.class == "military"
                    && spec.is_melee_capable()
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && Self::war_unit_unlocked(g, pid, *unit)
            })
            .max_by(|left, right| {
                g.rules.units[*left]
                    .strength
                    .total_cmp(&g.rules.units[*right].strength)
                    .then_with(|| right.cmp(left))
            })
    }

    pub(super) fn war_staging_route(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        objective: Pos,
    ) -> Option<(u32, usize)> {
        g.player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let spec = &g.rules.units[g.units[uid].kind];
                spec.class == "military"
                    && spec.is_melee_capable()
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
            })
            .filter_map(|uid| {
                self.war_staging_route_for_unit(g, pid, target, uid, objective)
                    .map(|distance| (uid, distance))
            })
            .min_by_key(|(uid, distance)| (*distance, *uid))
    }

    fn war_can_field_unit(g: &Game, pid: usize, unit: Name) -> bool {
        g.player_unit_ids(pid)
            .into_iter()
            .any(|uid| g.units[&uid].kind == unit)
            || g.player_city_ids(pid).into_iter().any(|cid| {
                g.cities[&cid]
                    .queue
                    .first()
                    .and_then(Self::war_item_unit)
                    .is_some_and(|(queued, _)| queued == unit)
                    || g.can_produce(pid, cid, &Item::Unit { unit })
            })
    }

    /// `Some(None)` means the objective is currently unwalled. `None` means
    /// it is walled and this empire cannot field a compatible breach element,
    /// so the target/unlock pair is not a real attack plan.
    pub(super) fn war_breach_requirement(
        &self,
        g: &Game,
        pid: usize,
        city: &crate::game::City,
    ) -> Option<Option<Name>> {
        let wall_hp = if self.battlefront_observation {
            self.remembered_city(city.id)
                .map(|sighting| sighting.wall_hp)
                .unwrap_or(city.wall_hp)
        } else {
            city.wall_hp
        };
        if wall_hp <= 0 {
            return Some(None);
        }
        if super::super::siege_support::has_attackers(g, pid) {
            for unit in [crate::name!("battering_ram"), crate::name!("siege_tower")] {
                if g.city_allows_siege_support(city.id, unit.as_str())
                    && Self::war_can_field_unit(g, pid, unit)
                {
                    return Some(Some(unit));
                }
            }
        }

        let city_strength = self
            .remembered_city(city.id)
            .map(|sighting| sighting.strength)
            .unwrap_or_else(|| g.city_strength(city.id));
        Self::player_unit_catalog(g, pid)
            .into_iter()
            .filter(|unit| {
                let spec = &g.rules.units[*unit];
                spec.class == "military"
                    && spec.siege
                    && spec.bombard_strength + f64::EPSILON >= city_strength
                    && Self::war_unit_unlocked(g, pid, *unit)
                    && Self::war_can_field_unit(g, pid, *unit)
            })
            .min_by(|left, right| {
                g.rules.units[*left]
                    .cost
                    .total_cmp(&g.rules.units[*right].cost)
                    .then(left.cmp(right))
            })
            .map(Some)
    }

    pub(super) fn war_remaining_research_cost(g: &Game, pid: usize, goal: Name) -> f64 {
        let mut path = g
            .rules
            .tech_ancestors
            .get(goal.as_str())
            .cloned()
            .unwrap_or_default();
        path.insert(goal.as_str().to_string());
        let mut cost: f64 = path
            .iter()
            .filter(|tech| !g.players[pid].techs.contains(&Name::new(tech)))
            .map(|tech| {
                g.host_remaining_research_cost(pid, Name::new(tech))
                    .unwrap_or_else(|| g.tech_cost(tech.as_str()))
            })
            .sum();
        if let Some(current) = g.players[pid].research.as_deref() {
            if path.contains(current)
                && g.host_remaining_research_cost(pid, Name::new(current))
                    .is_none()
            {
                cost -= g.players[pid].research_progress.min(g.tech_cost(current));
            }
        }
        cost.max(0.0)
    }

    pub(super) fn war_science_per_turn(g: &Game, pid: usize) -> f64 {
        let _memo = g.query_memo();
        g.player_city_ids(pid)
            .into_iter()
            .map(|city| g.city_yields(city).science)
            .sum::<f64>()
            .max(1.0)
    }

    pub(super) fn war_production_per_turn(g: &Game, pid: usize) -> f64 {
        let _memo = g.query_memo();
        g.player_city_ids(pid)
            .into_iter()
            .map(|city| g.city_yields(city).production)
            .sum::<f64>()
            .max(1.0)
    }

    fn war_candidate_defense(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        city: &crate::game::City,
    ) -> f64 {
        let city_strength = self
            .remembered_city(city.id)
            .map(|sighting| sighting.strength)
            .unwrap_or_else(|| g.city_strength(city.id));
        let visible = self.battlefront_visibility(g, pid);
        let field = g
            .units
            .values()
            .filter(|unit| unit.owner == target && g.wdist(unit.pos, city.pos) <= 6)
            .filter(|unit| {
                !self.battlefront_observation
                    || (g.sees(&visible, unit.pos)
                        && self.battlefront_unit_visible(g, pid, unit.id))
            })
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            .map(|unit| g.unit_strength(unit, true))
            .fold(0.0_f64, f64::max);
        city_strength.max(field)
    }

    pub(super) fn war_resource_feasible(
        g: &Game,
        pid: usize,
        unit: Name,
        launch_turns: u32,
    ) -> bool {
        let spec = &g.rules.units[unit];
        let Some(resource) = spec.requires_resource else {
            return true;
        };
        if g.connected_resource_count(pid, resource.as_str()) > 0 {
            return true;
        }
        let available = g.strategic_stockpile(pid, resource)
            + g.strategic_resource_rate(pid, resource.as_str()) * launch_turns as f64;
        let (_, required) = g.upgrade_costs(pid, 0.0, spec.resource_cost * TIMED_WAR_BODIES as f64);
        available + f64::EPSILON >= required
    }

    pub(super) fn war_existing_or_queued_count(
        &self,
        g: &Game,
        pid: usize,
        assault: Name,
        predecessor: Option<Name>,
    ) -> usize {
        let counts = |unit: Name| {
            Self::war_unit_is_at_least(g, pid, unit, assault) || predecessor == Some(unit)
        };
        g.player_unit_ids(pid)
            .into_iter()
            .filter(|uid| counts(g.units[uid].kind))
            .count()
            + g.player_city_ids(pid)
                .into_iter()
                .filter_map(|cid| g.cities[&cid].queue.first())
                .filter_map(Self::war_item_unit)
                .filter(|(unit, _)| counts(*unit))
                .count()
    }

    fn war_breach_present(g: &Game, pid: usize, breach: Name) -> bool {
        g.player_unit_ids(pid)
            .into_iter()
            .any(|uid| g.units[&uid].kind == breach)
            || g.player_city_ids(pid).into_iter().any(|cid| {
                g.cities[&cid]
                    .queue
                    .first()
                    .and_then(Self::war_item_unit)
                    .is_some_and(|(unit, _)| unit == breach)
            })
    }

    pub(super) fn choose_war_plan(&self, g: &Game, pid: usize) -> Option<WarPlan> {
        let selective_target = if self.selective_timed_war {
            self.selective_timed_war_target()
        } else {
            None
        };
        if self.selective_timed_war && selective_target.is_none() {
            return None;
        }
        let science = Self::war_science_per_turn(g, pid);
        let production = Self::war_production_per_turn(g, pid);
        let catalog = Self::player_unit_catalog(g, pid);
        let current_body = Self::war_strongest_current_melee(g, pid)?;
        let horizon = g
            .max_turns
            .saturating_sub(g.standard_duration(TIMED_WAR_ENDGAME_RESERVE));
        let mut target_choices = Vec::new();

        for target in g.players.iter().filter(|player| {
            player.id != pid
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && selective_target.is_none_or(|selected| player.id == selected)
                && self.campaign_target_legal(g, pid, player.id)
        }) {
            let mut objectives: Vec<_> = g
                .cities
                .values()
                .filter(|city| city.owner == target.id)
                .collect();
            objectives.sort_by(|left, right| {
                self.campaign_city_value(g, pid, left, GrandStrategy::Conquest)
                    .total_cmp(&self.campaign_city_value(g, pid, right, GrandStrategy::Conquest))
                    .then(left.id.cmp(&right.id))
            });
            for city in objectives {
                let Some((_, route_steps)) = self.war_staging_route(g, pid, target.id, city.pos)
                else {
                    continue;
                };
                let Some(breach_unit) = self.war_breach_requirement(g, pid, city) else {
                    continue;
                };
                let defense = self.war_candidate_defense(g, pid, target.id, city);
                let mut unlocks = Vec::new();
                for assault in catalog.iter().copied() {
                    let spec = &g.rules.units[assault];
                    let Some(tech) = spec.tech else {
                        continue;
                    };
                    if g.players[pid].techs.contains(&tech)
                        || spec.class != "military"
                        || !spec.is_melee_capable()
                        || matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    {
                        continue;
                    }
                    let predecessor = Self::war_direct_predecessor(g, pid, assault);
                    let baseline = predecessor.unwrap_or(current_body);
                    if spec.strength - g.rules.units[baseline].strength
                        < TIMED_WAR_MIN_STRENGTH_GAIN
                    {
                        continue;
                    }
                    let expected_damage = 30.0 * ((spec.strength - defense) / 25.0).exp();
                    if expected_damage + f64::EPSILON < TIMED_WAR_MIN_EXPECTED_DAMAGE {
                        continue;
                    }
                    let research_cost = Self::war_remaining_research_cost(g, pid, tech);
                    let research_turns = (research_cost / science).ceil() as u32;
                    let body_count = self
                        .war_existing_or_queued_count(g, pid, assault, predecessor)
                        .min(TIMED_WAR_BODIES);
                    let prebuilt_floor = if self.rapid_timed_war {
                        TIMED_WAR_BODIES
                    } else {
                        SELECTIVE_TIMED_WAR_PREBUILT_BODIES
                    };
                    if self.selective_timed_war && body_count < prebuilt_floor {
                        continue;
                    }
                    let missing = TIMED_WAR_BODIES.saturating_sub(body_count);
                    let build_body = predecessor.unwrap_or(assault);
                    let mut production_cost = g.rules.units[build_body].cost * missing as f64;
                    if let Some(breach) = breach_unit {
                        let breach_present = Self::war_breach_present(g, pid, breach);
                        if self.rapid_timed_war && !breach_present {
                            continue;
                        }
                        if !breach_present {
                            production_cost += g.rules.units[breach].cost;
                        }
                    }
                    let production_turns = (production_cost / production).ceil() as u32;
                    let slowest_moves = breach_unit
                        .map(|breach| spec.moves.min(g.rules.units[breach].moves))
                        .unwrap_or(spec.moves)
                        .max(1.0);
                    let march_turns = (route_steps as f64 / slowest_moves).ceil() as u32;
                    let launch_turns = research_turns
                        .saturating_add(production_turns)
                        .saturating_add(march_turns);
                    if (self.rapid_timed_war
                        && launch_turns > g.standard_duration(RAPID_TIMED_WAR_LAUNCH_WINDOW))
                        || g.turn.saturating_add(launch_turns) >= horizon
                        || !Self::war_resource_feasible(g, pid, assault, launch_turns)
                    {
                        continue;
                    }
                    unlocks.push((
                        research_cost,
                        launch_turns,
                        std::cmp::Reverse((expected_damage * 1000.0).round() as i64),
                        assault,
                        tech,
                        predecessor,
                        research_turns,
                        production_turns,
                        march_turns,
                    ));
                }
                unlocks.sort_by(|left, right| {
                    left.0
                        .total_cmp(&right.0)
                        .then(left.1.cmp(&right.1))
                        .then(left.2.cmp(&right.2))
                        .then(left.3.cmp(&right.3))
                });
                let Some((
                    _,
                    launch_turns,
                    _,
                    assault_unit,
                    breakthrough_tech,
                    predecessor,
                    estimated_research_turns,
                    estimated_production_turns,
                    estimated_march_turns,
                )) = unlocks.into_iter().next()
                else {
                    continue;
                };
                let mut plan = WarPlan {
                    target_player: target.id,
                    objective_city: city.id,
                    breakthrough_tech,
                    assault_unit,
                    predecessor,
                    breach_unit,
                    estimated_research_turns,
                    estimated_production_turns,
                    estimated_upgrade_gold: 0.0,
                    estimated_march_turns,
                    phase: WarPhase::Research,
                    appointed_turn: g.turn,
                    tech_turn: None,
                    declared_turn: None,
                    last_reviewed_turn: g.turn,
                    recovery_assessments: 0,
                };
                plan.estimated_upgrade_gold = self.war_upgrade_bill(g, pid, &plan);
                let score = self.campaign_city_value(g, pid, city, GrandStrategy::Conquest)
                    + launch_turns as f64;
                target_choices.push((score, target.id, city.id, assault_unit, plan));
                // The objective list is already campaign-ranked. The first
                // routeable, breach-feasible city is this target's attack.
                break;
            }
        }
        target_choices
            .into_iter()
            .min_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then(left.1.cmp(&right.1))
                    .then(left.2.cmp(&right.2))
                    .then(left.3.cmp(&right.3))
            })
            .map(|(_, _, _, _, plan)| plan)
    }

    /// One staging predicate is shared by lifecycle phase selection and the
    /// declaration site. That keeps the UI's `strike` phase from promising a
    /// launch the diplomatic executor would refuse (or vice versa).
    pub(super) fn timed_war_staging_ready(&self, status: &WarPackageStatus) -> bool {
        let bodies_staged = if self.selective_timed_war {
            status.staged_bodies >= TIMED_WAR_BODIES
        } else {
            status.staged_bodies >= TIMED_WAR_BODIES - 1 && status.fourth_one_turn_away
        };
        let strength_floor = if self.selective_timed_war {
            SELECTIVE_TIMED_WAR_STRENGTH_FLOOR
        } else {
            1.0
        };
        bodies_staged
            && status.breach_staged
            && status.local_strength_ratio + f64::EPSILON >= strength_floor
    }

    pub(super) fn record_war_abort(&mut self, reason: &'static str) {
        *self.war_census.aborts.entry(reason).or_default() += 1;
        if self.journal().wants(crate::reasoning::Level::Strategy) {
            think!(self.journal(), Military, Strategy, "Standing down the appointed attack";
                   "{reason}");
        }
    }

    fn record_war_capture(&mut self, g: &Game, plan: &WarPlan) {
        self.war_census.objectives_captured += 1;
        if let Some(declared) = plan.declared_turn {
            let elapsed = g.turn.saturating_sub(declared);
            self.war_census.declaration_to_capture_turns += elapsed;
            self.war_census.declaration_to_capture_samples.push(elapsed);
            if elapsed <= TIMED_WAR_CAPTURE_WINDOW {
                self.war_census.objectives_captured_within_ten += 1;
            }
        }
        think!(self.journal(), Military, Strategy, "The appointed city has fallen";
               "{} turns from appointment and {} from declaration",
               g.turn.saturating_sub(plan.appointed_turn),
               plan.declared_turn.map_or(0, |turn| g.turn.saturating_sub(turn)));
    }

    fn refresh_war_estimates(&self, g: &Game, pid: usize, plan: &mut WarPlan) {
        plan.estimated_research_turns =
            (Self::war_remaining_research_cost(g, pid, plan.breakthrough_tech)
                / Self::war_science_per_turn(g, pid))
            .ceil() as u32;
        let status = self.war_package_status(g, pid, plan);
        let missing = TIMED_WAR_BODIES
            .saturating_sub(status.planned_bodies.saturating_add(status.queued_bodies));
        let build = if g.players[pid].techs.contains(&plan.breakthrough_tech) {
            plan.assault_unit
        } else {
            plan.predecessor.unwrap_or(plan.assault_unit)
        };
        let mut cost = g.rules.units[build].cost * missing as f64;
        if let Some(breach) = plan.breach_unit {
            let queued = g.player_city_ids(pid).into_iter().any(|cid| {
                g.cities[&cid]
                    .queue
                    .first()
                    .and_then(Self::war_item_unit)
                    .is_some_and(|(unit, _)| unit == breach)
            });
            if !status.breach_ready && !queued {
                cost += g.rules.units[breach].cost;
            }
        }
        plan.estimated_production_turns =
            (cost / Self::war_production_per_turn(g, pid)).ceil() as u32;
        plan.estimated_upgrade_gold = status.upgrade_gold;
        if let Some(city) = g.cities.get(&plan.objective_city) {
            if let Some((_, steps)) = self.war_staging_route(g, pid, plan.target_player, city.pos) {
                let slowest = plan
                    .breach_unit
                    .map(|breach| {
                        g.rules.units[plan.assault_unit]
                            .moves
                            .min(g.rules.units[breach].moves)
                    })
                    .unwrap_or(g.rules.units[plan.assault_unit].moves)
                    .max(1.0);
                plan.estimated_march_turns = (steps as f64 / slowest).ceil() as u32;
            }
        }
    }

    /// Validate and advance the one appointed campaign before any subsystem
    /// acts. This is the lifecycle authority: research, production, money,
    /// diplomacy, and movement only consume the resulting phase.
    pub(super) fn maintain_war_plan(&mut self, g: &Game, pid: usize) {
        if !self.timed_war {
            self.war_plan = None;
            self.war_status = WarPackageStatus::default();
            return;
        }

        if let Some(mut plan) = self.war_plan.take() {
            let target_alive = g
                .players
                .get(plan.target_player)
                .is_some_and(|target| target.alive);
            let objective_owner = g.cities.get(&plan.objective_city).map(|city| city.owner);
            let at_war = target_alive && g.is_at_war(pid, plan.target_player);
            let mut end = None;
            if objective_owner != Some(plan.target_player) {
                if plan.declared_turn.is_some() && objective_owner == Some(pid) {
                    self.record_war_capture(g, &plan);
                } else {
                    self.record_war_abort("objective changed owner");
                }
                end = Some(());
            } else if !target_alive {
                self.record_war_abort("target no longer alive");
                end = Some(());
            } else if !at_war && !self.campaign_target_legal(g, pid, plan.target_player) {
                self.record_war_abort("diplomacy made war illegal");
                end = Some(());
            } else if at_war && plan.declared_turn.is_none() {
                self.record_war_abort("target opened the war first");
                end = Some(());
            } else if plan.declared_turn.is_some() && !at_war {
                self.record_war_abort("peace closed the war");
                end = Some(());
            }
            if end.is_some() {
                self.plan = None;
            } else {
                self.refresh_war_estimates(g, pid, &mut plan);
                let launch_turns = plan
                    .estimated_research_turns
                    .saturating_add(plan.estimated_production_turns)
                    .saturating_add(plan.estimated_march_turns);
                let horizon = g
                    .max_turns
                    .saturating_sub(g.standard_duration(TIMED_WAR_ENDGAME_RESERVE));
                if g.turn.saturating_add(launch_turns) >= horizon {
                    self.record_war_abort("launch slipped beyond the attack horizon");
                    self.plan = None;
                } else if !Self::war_resource_feasible(g, pid, plan.assault_unit, launch_turns) {
                    self.record_war_abort("strategic resource package became impossible");
                    self.plan = None;
                } else {
                    let review_cadence = g.standard_duration(5).max(1);
                    if g.turn.saturating_sub(plan.last_reviewed_turn) >= review_cadence {
                        plan.last_reviewed_turn = g.turn;
                        if self.threatened_city(g, pid).is_some() {
                            plan.recovery_assessments = plan.recovery_assessments.saturating_add(1);
                        } else {
                            plan.recovery_assessments = 0;
                        }
                    }
                    if plan.recovery_assessments >= 2 {
                        self.record_war_abort("home Recovery persisted for two assessments");
                        self.plan = None;
                    } else if at_war {
                        plan.phase = WarPhase::Exploit;
                        self.war_plan = Some(plan);
                    } else {
                        let previous = plan.phase;
                        let tech_owned = g.players[pid].techs.contains(&plan.breakthrough_tech);
                        if tech_owned && plan.tech_turn.is_none() {
                            plan.tech_turn = Some(g.turn);
                            self.war_census.breakthroughs += 1;
                            let elapsed = g.turn.saturating_sub(plan.appointed_turn);
                            self.war_census.appointment_to_tech_turns += elapsed;
                            self.war_census.appointment_to_tech_samples.push(elapsed);
                        }
                        let status = self.war_package_status(g, pid, &plan);
                        let package_ready =
                            status.ready_bodies >= TIMED_WAR_BODIES && status.breach_ready;
                        let staged = self.timed_war_staging_ready(&status);
                        plan.phase = if !tech_owned {
                            WarPhase::Research
                        } else if !package_ready {
                            WarPhase::Mobilize
                        } else if !staged {
                            WarPhase::Stage
                        } else {
                            WarPhase::Strike
                        };
                        if package_ready
                            && matches!(previous, WarPhase::Research | WarPhase::Mobilize)
                        {
                            self.war_census.mobilizations += 1;
                        }
                        self.war_plan = Some(plan);
                    }
                }
            }
        }

        if self.war_plan.is_none() && self.may_form_war_plan(g, pid) {
            if let Some(plan) = self.choose_war_plan(g, pid) {
                self.war_census.appointments += 1;
                if let Some(city) = g.cities.get(&plan.objective_city) {
                    think!(self.journal(), Military, Strategy,
                           "Appointing a power-spike attack on {}", city.name;
                           "research {}, prebuild {}, reserve {:.0} gold, then stage four {} bodies{}; estimated launch in {} turns",
                           plain(&plan.breakthrough_tech),
                           plan.predecessor.map(|unit| plain(&unit)).unwrap_or_else(|| plain(&plan.assault_unit)),
                           plan.estimated_upgrade_gold,
                           plain(&plan.assault_unit),
                           plan.breach_unit.map(|unit| format!(" with {}", plain(&unit))).unwrap_or_default(),
                           plan.estimated_research_turns
                               .saturating_add(plan.estimated_production_turns)
                               .saturating_add(plan.estimated_march_turns));
                }
                self.war_plan = Some(plan);
            }
        }
        let status = self
            .war_plan
            .as_ref()
            .map(|plan| self.war_package_status(g, pid, plan))
            .unwrap_or_default();
        self.war_status = status;
    }

    /// The normal denial alarm starts at 78, which is right for an army that
    /// can move now but too late for a technology and two production projects.
    /// A Space Race gets the earlier warning because an Expedition is already
    /// an irreversible countdown; religious victories advance in coarse jumps
    /// and need the same lead time.
    fn nuclear_victory_threat(pressure: VictoryFocus) -> bool {
        match pressure.strategy {
            GrandStrategy::Science => pressure.progress >= 65,
            GrandStrategy::Religion => pressure.progress >= 67,
            GrandStrategy::Culture
            | GrandStrategy::Diplomacy
            | GrandStrategy::Conquest
            | GrandStrategy::Expansion => pressure.progress >= 78,
            GrandStrategy::Recovery => false,
        }
    }

    /// How much a functioning district matters to one particular victory.
    /// These are disruption values, not construction values: WMD damage
    /// pillages the district and every building attached to it, which makes a
    /// Spaceport running a launch much more valuable than a generic Campus.
    fn nuclear_victory_district_value(strategy: GrandStrategy, family: &str) -> f64 {
        let general = match family {
            "spaceport" => 320.0,
            "campus" => 150.0,
            "theater_square" | "holy_site" => 160.0,
            "diplomatic_quarter" => 140.0,
            "commercial_hub" | "harbor" => 100.0,
            "industrial_zone" | "encampment" => 90.0,
            "aerodrome" => 70.0,
            _ => 25.0,
        };
        general
            + match (strategy, family) {
                (GrandStrategy::Science, "spaceport") => 850.0,
                (GrandStrategy::Science, "campus") => 260.0,
                (GrandStrategy::Science, "industrial_zone") => 180.0,
                (GrandStrategy::Culture, "theater_square") => 760.0,
                (GrandStrategy::Culture, "commercial_hub" | "harbor") => 160.0,
                (GrandStrategy::Religion, "holy_site") => 820.0,
                (GrandStrategy::Diplomacy, "diplomatic_quarter") => 720.0,
                (GrandStrategy::Diplomacy, "commercial_hub" | "harbor") => 240.0,
                (GrandStrategy::Conquest, "encampment") => 440.0,
                _ => 0.0,
            }
    }

    /// Value the exact disruption a blast can cause at a city centre.  WMD
    /// resolution destroys deployed forces, strips population and walls, and
    /// pillages each district/building in its disk.  Scoring those effects is
    /// what makes a threatened Spaceport, Holy Site, or Diplomatic Quarter
    /// beat a merely walled city when it is the rival's actual win engine.
    pub(super) fn nuclear_target_value(
        &self,
        g: &Game,
        pid: usize,
        target: Pos,
        thermonuclear: bool,
    ) -> Option<f64> {
        let target_city = g.city_at(target)?;
        let target_ref = g.cities.get(&target_city)?;
        if target_ref.owner == pid {
            return None;
        }
        let radius = g.rules.wmds[if thermonuclear {
            "thermonuclear_device"
        } else {
            "nuclear_device"
        }]
        .blast_radius;
        let blast = g.wdisk(target, radius);
        let garrison = blast
            .iter()
            .flat_map(|position| g.unit_ids_at(*position))
            .filter(|unit| g.is_at_war(pid, g.units[unit].owner))
            .count();
        let hardness = g.city_strength(target_city) + target_ref.wall_hp as f64 / 10.0;
        let mut strategic = 0.0;
        for position in &blast {
            let Some(tile) = g.map.get(*position) else {
                continue;
            };
            let Some(hit_city_id) = tile.owner_city else {
                continue;
            };
            let Some(hit_city) = g.cities.get(&hit_city_id) else {
                continue;
            };
            // A target that overlaps our own or a neutral city's assets is
            // never a legal shot.  The live strike pass performs the same
            // safety check for units and every owned tile; doing the
            // permanent-city half here prevents research from chasing a
            // strategic asset it could not actually disrupt.
            if hit_city.owner == pid
                || (hit_city.owner != target_ref.owner && !g.is_at_war(pid, hit_city.owner))
            {
                return None;
            }
            let pressure = if g
                .players
                .get(hit_city.owner)
                .is_some_and(|player| !player.is_minor && !player.is_barbarian)
            {
                self.rival_victory_pressure(g, hit_city.owner)
            } else {
                VictoryFocus {
                    strategy: GrandStrategy::Expansion,
                    progress: 0,
                }
            };
            if g.city_at(*position) == Some(hit_city_id) {
                // A blast halves population, so dense and capital cities still
                // matter even when the opponent's victory asset is next door.
                strategic += hit_city.pop.max(1) as f64 * 3.0;
                if hit_city.is_capital {
                    strategic += if pressure.strategy == GrandStrategy::Conquest {
                        300.0
                    } else {
                        130.0
                    };
                }
            }
            let Some(district) = tile.district else {
                continue;
            };
            if tile.pillaged {
                continue;
            }
            let family = g.district_family(district);
            let urgency = 0.5 + f64::from(pressure.progress.clamp(0, 100)) / 100.0;
            strategic +=
                Self::nuclear_victory_district_value(pressure.strategy, family.as_str()) * urgency;
            // `damage_tile_area` also pillages every building that belongs to
            // this district.  Construction cost is a conservative proxy for
            // the repair delay; the victory-specific district bonus above is
            // deliberately the larger term.
            strategic += hit_city
                .buildings
                .iter()
                .filter(|building| !hit_city.pillaged_buildings.contains(*building))
                .filter_map(|building| g.rules.buildings.get(building.as_str()))
                .filter(|building| {
                    building
                        .district
                        .is_some_and(|required| g.district_family(required) == family)
                })
                .map(|building| building.cost * 0.18)
                .sum::<f64>();
            let queued_project = match hit_city.queue.first() {
                Some(Item::Project { project }) => match (pressure.strategy, family.as_str()) {
                    (GrandStrategy::Science, "spaceport")
                        if matches!(
                            project.as_str(),
                            "launch_earth_satellite"
                                | "launch_moon_landing"
                                | "launch_mars_colony"
                                | "exoplanet_expedition"
                                | "lagrange_laser_station"
                                | "terrestrial_laser_station"
                        ) =>
                    {
                        700.0
                    }
                    (GrandStrategy::Religion, "holy_site") => 180.0,
                    (GrandStrategy::Culture, "theater_square") => 140.0,
                    (GrandStrategy::Diplomacy, "diplomatic_quarter") => 180.0,
                    _ => 0.0,
                },
                _ => 0.0,
            };
            strategic += queued_project * urgency;
        }
        // Preserve the old doctrine's hard-target floor, but let an exposed
        // win-condition asset clear it even when the city has not yet built
        // walls or a garrison.
        if hardness < 50.0 && garrison < 3 && strategic < 125.0 {
            return None;
        }
        Some(hardness + garrison as f64 * 12.0 + strategic)
    }

    /// Find a late-game nuclear response that can affect the board now.  A
    /// target must be revealed and inside the reach of a platform this seat
    /// holds today: this is a quick counter-program, not a hope that a future
    /// silo, bomber or submarine might eventually reach an unseen opponent.
    pub(super) fn nuclear_war_goal(
        &self,
        g: &Game,
        pid: usize,
        strategy: GrandStrategy,
        target_player: Option<usize>,
    ) -> Option<NuclearWarGoal> {
        // `nuclear-goal-needs-a-launcher`: see `advanced/nuclear_launcher.rs`.
        if self.nuclear_goal_has_no_launcher() {
            return None;
        }
        if !self.victory_planning
            || g.world_era < 4
            || !matches!(strategy, GrandStrategy::Conquest | GrandStrategy::Recovery)
        {
            return None;
        }
        // Cities that can run the programme. Delivery is a separate question,
        // answered per target by `Game::wmd_can_reach`.
        let builders = g.player_city_ids(pid);
        if builders.is_empty()
            || !builders.iter().any(|city| {
                g.cities[city].districts.iter().any(|(district, position)| {
                    g.district_family(*district) == crate::name!("industrial_zone")
                        && !g.map.tiles[position].pillaged
                })
            })
        {
            return None;
        }
        let mut best: Option<(f64, NuclearWarGoal)> = None;
        for rival in g.players.iter().filter(|rival| {
            rival.id != pid
                && rival.alive
                && !rival.is_minor
                && !rival.is_barbarian
                && !g.same_team(pid, rival.id)
        }) {
            let at_war = g.is_at_war(pid, rival.id);
            if strategy == GrandStrategy::Recovery && !at_war {
                continue;
            }
            let pressure = self.rival_victory_pressure(g, rival.id);
            let denial = Self::nuclear_victory_threat(pressure);
            let planned_target = self.plan.as_ref().is_some_and(|plan| {
                plan.strategy == GrandStrategy::Conquest && plan.target_player == Some(rival.id)
            });
            let committed = strategy == GrandStrategy::Conquest
                && (target_player == Some(rival.id)
                    || planned_target
                    || (target_player.is_none()
                        && self.victory_target == Some(VictoryTarget::Domination)));
            if !denial && !committed && !at_war {
                continue;
            }
            let target_values: Vec<(f64, bool)> = g
                .cities
                .values()
                .filter(|city| city.owner == rival.id)
                .filter(|city| g.players[pid].explored.contains(&city.pos))
                // Plan from the best device the program can unlock.  A
                // Spaceport two tiles from its centre is a real
                // thermonuclear target even if a standard device's smaller
                // blast cannot quite reach it; rejecting that board state
                // would strand the Fusion half of a useful counter-program.
                .filter_map(|city| {
                    let standard = g
                        .wmd_can_reach(pid, city.pos, false)
                        .then(|| self.nuclear_target_value(g, pid, city.pos, false))
                        .flatten();
                    let thermonuclear = g
                        .wmd_can_reach(pid, city.pos, true)
                        .then(|| self.nuclear_target_value(g, pid, city.pos, true))
                        .flatten();
                    let priority = standard
                        .into_iter()
                        .chain(thermonuclear)
                        .max_by(f64::total_cmp)?;
                    let needs_thermonuclear = thermonuclear.is_some_and(|value| {
                        standard.is_none_or(|ordinary| {
                            value >= ordinary + NUCLEAR_STRATEGIC_TARGET_FLOOR
                        })
                    });
                    Some((priority, needs_thermonuclear))
                })
                .collect();
            let Some((priority, needs_thermonuclear)) = target_values
                .iter()
                .copied()
                .max_by(|left, right| left.0.total_cmp(&right.0))
            else {
                continue;
            };
            let strategic_targets = target_values
                .iter()
                .filter(|(value, _)| *value >= NUCLEAR_STRATEGIC_TARGET_FLOOR)
                .count();
            let desired_stockpile = if pressure.progress >= 90 {
                NUCLEAR_ARSENAL_CAP
            } else if denial || strategic_targets >= 2 {
                2
            } else {
                1
            };
            let score = priority
                + f64::from(pressure.progress.max(0)) * 7.0
                + if committed { 190.0 } else { 0.0 }
                + if at_war { 55.0 } else { 0.0 };
            if best.as_ref().is_none_or(|(current, _)| score > *current) {
                best = Some((
                    score,
                    NuclearWarGoal {
                        pressure,
                        desired_stockpile,
                        priority,
                        needs_thermonuclear,
                    },
                ));
            }
        }
        best.map(|(_, goal)| goal)
    }

    pub(super) fn campaign_city_value(
        &self,
        g: &Game,
        pid: usize,
        city: &crate::game::City,
        strategy: GrandStrategy,
    ) -> f64 {
        // A campaign can remember a city it has seen, but cannot know that a
        // hidden city healed, lost its walls, or received a garrison this
        // turn.  Use the sighting refreshed at the start of the acting turn;
        // an unseen city with no report receives a conservative generic
        // defense estimate rather than its live private combat state.
        let sighting = self
            .battlefront_observation
            .then(|| self.remembered_city(city.id))
            .flatten();
        let conservative_unknown_city =
            self.battlefront_observation && self.blind_objective_strength;
        let observed_hp = sighting.map(|report| report.hp).unwrap_or_else(|| {
            if self.battlefront_observation {
                200
            } else {
                city.hp
            }
        });
        let observed_wall_hp = sighting.map(|report| report.wall_hp).unwrap_or_else(|| {
            if conservative_unknown_city {
                UNKNOWN_OBJECTIVE_WALL_HP
            } else if self.battlefront_observation {
                0
            } else {
                city.wall_hp
            }
        });
        let observed_strength = sighting.map(|report| report.strength).unwrap_or_else(|| {
            if conservative_unknown_city {
                UNKNOWN_OBJECTIVE_STRENGTH
            } else if self.battlefront_observation {
                20.0
            } else {
                g.city_strength(city.id)
            }
        });
        let observed_owner = sighting.map(|report| report.owner).unwrap_or(city.owner);
        let visible = self.battlefront_visibility(g, pid);
        let core_distance = g
            .player_city_ids(pid)
            .into_iter()
            .map(|mine| g.wdist(g.cities[&mine].pos, city.pos))
            .min()
            .unwrap_or(40);
        let military_units = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|unit| g.rules.units[g.units[unit].kind].class == "military")
            .collect::<Vec<_>>();
        let city_is_coastal = g.nbrs(city.pos).into_iter().any(|position| {
            g.map
                .get(position)
                .is_some_and(|tile| g.rules.is_water(tile))
        });
        let military_distance = military_units
            .iter()
            .filter(|unit| {
                let domain = g.rules.units[g.units[unit].kind].domain.as_deref();
                domain != Some("sea") || city_is_coastal
            })
            .map(|unit| g.wdist(g.units[unit].pos, city.pos))
            .min()
            .unwrap_or(core_distance);
        let has_land_force = military_units.iter().any(|unit| {
            !matches!(
                g.rules.units[g.units[unit].kind].domain.as_deref(),
                Some("sea" | "air")
            )
        });
        let has_naval_force = military_units
            .iter()
            .any(|unit| g.rules.units[g.units[unit].kind].domain.as_deref() == Some("sea"));
        // With no army yet, value prospective land staging rather than
        // declaring every objective sealed. Once forces exist, only count
        // adjacent tiles that the relevant land or naval arm can exploit.
        let plan_land_approach = has_land_force || !has_naval_force;
        let approaches = g
            .nbrs(city.pos)
            .into_iter()
            .filter(|position| {
                g.map.get(*position).is_some_and(|tile| {
                    g.rules.is_passable(tile)
                        && if g.rules.is_water(tile) {
                            has_naval_force
                        } else {
                            plan_land_approach
                        }
                })
            })
            .count();
        let friendly_local: f64 = g
            .units
            .values()
            .filter(|unit| unit.owner == pid && g.wdist(unit.pos, city.pos) <= 7)
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            .map(|unit| crate::game::effective_strength(g.unit_strength(unit, true), unit.hp))
            .sum();
        let hostile_local: f64 = g
            .units
            .values()
            .filter(|unit| unit.owner == observed_owner && g.wdist(unit.pos, city.pos) <= 7)
            .filter(|unit| {
                !self.battlefront_observation
                    || (g.sees(&visible, unit.pos)
                        && self.battlefront_unit_visible(g, pid, unit.id))
            })
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            .map(|unit| crate::game::effective_strength(g.unit_strength(unit, true), unit.hp))
            .sum::<f64>()
            + if self.battlefront_observation {
                self.belief.remembered_hidden_military_threat_in_view(
                    g,
                    pid,
                    &visible,
                    city.pos,
                    7,
                    BELIEF_PRESSURE_HORIZON,
                )
            } else {
                0.0
            };

        let friendly_pressure: f64 = g
            .cities
            .values()
            .filter(|source| source.owner == pid)
            .filter_map(|source| {
                let distance = g.wdist(source.pos, city.pos);
                (distance <= 9).then_some(source.pop.max(1) as f64 * (10 - distance) as f64)
            })
            .sum();
        let hostile_pressure: f64 = g
            .cities
            .values()
            .filter(|source| source.owner == city.owner && source.id != city.id)
            .filter_map(|source| {
                let distance = g.wdist(source.pos, city.pos);
                (distance <= 9).then_some(source.pop.max(1) as f64 * (10 - distance) as f64)
            })
            .sum();
        let occupation_risk = (hostile_pressure - friendly_pressure).max(0.0)
            * if strategy == GrandStrategy::Conquest {
                0.7
            } else {
                1.2
            };
        // An Original Capital cannot be razed, so taking it before the
        // surrounding population is controlled creates a forced keep/flip/
        // recapture cycle. Make the rest of that population the campaign
        // objective first; the cost disappears as soon as the capital can be
        // held, or when taking it would end the war or game outright.
        let unsupported_capture = if Self::should_defer_city_capture(g, pid, city.id) {
            10_000.0
        } else {
            0.0
        };

        let defenses = observed_strength * 1.8
            + observed_hp.max(0) as f64 * 0.12
            + observed_wall_hp.max(0) as f64 * 0.16;
        // The first city of a Domination campaign is a foothold, not just a
        // trophy. On the live King Brazil front we selected a 100-wall border
        // city over a nearby unwalled one; it upgraded to 400 walls during a
        // 25-turn assault that captured nothing. The generic defensive score
        // charges only 16 for those first 100 wall HP, less than two tiles of
        // march. Price the setup and bombardment time when ordering conquest
        // objectives; a lone walled city is still a valid target.
        let wall_breach_delay = if strategy == GrandStrategy::Conquest
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && observed_wall_hp > 0
        {
            55.0 + observed_wall_hp as f64 * 0.4
        } else {
            0.0
        };
        // See [`AdvancedAi::siege_commitment`]. `defenses` above already prefers
        // a weakened city, but only by ~37 points across the whole health bar —
        // less than the distance terms swing over five tiles. That is why the
        // campaign walks off a city at 25 hp with its walls down, and why Civ 6
        // then heals it back to full at 20 hp a turn. Price the health already
        // stripped from a *breached* city as the investment it is.
        //
        // `military_distance` is the reach test rather than `core_distance`
        // because the question is whether the army that broke this city can
        // still finish it, not whether the empire is nearby.
        let siege_commitment = if self.siege_commitment
            && observed_wall_hp <= 0
            && military_distance <= SIEGE_COMMITMENT_REACH
        {
            (CITY_MAX_HP - observed_hp).clamp(0, CITY_MAX_HP) as f64 * SIEGE_COMMITMENT_PER_HP
        } else {
            0.0
        };
        let local_balance = (hostile_local - friendly_local).clamp(-250.0, 250.0) * 0.45;
        let approach_cost = (6usize.saturating_sub(approaches)) as f64 * 11.0;
        let development = city.pop.max(1) as f64 * 7.0
            + city.buildings.len() as f64 * 5.0
            + city.districts.len() as f64 * 10.0
            + city.wonders.len() as f64 * 24.0;
        let capital_value = if city.is_capital {
            if strategy == GrandStrategy::Conquest {
                180.0
            } else {
                75.0
            }
        } else {
            0.0
        };
        let science_denial = if city.districts.contains_key(crate::name!("spaceport"))
            && self.rival_victory_pressure(g, city.owner).strategy == GrandStrategy::Science
        {
            110.0
        } else {
            0.0
        };
        let recapture_value = if city.original_owner == pid {
            135.0
        } else {
            0.0
        };
        let liberation_value = if city.original_owner != city.owner
            && city.original_owner != pid
            && g.players
                .get(city.original_owner)
                .is_some_and(|founder| !founder.is_barbarian)
            && strategy == GrandStrategy::Diplomacy
        {
            120.0
        } else {
            0.0
        };

        core_distance as f64 * 7.0
            + military_distance as f64 * 5.0
            + defenses
            + wall_breach_delay
            + local_balance
            + approach_cost
            + occupation_risk
            + unsupported_capture
            + self.conversion_hold_cost(g, pid, city.id)
            - development
            - capital_value
            - science_denial
            - recapture_value
            - liberation_value
            - siege_commitment
            - self.strategic_deposit_prey_value(g, pid, city, strategy)
            - self.foothold_value(g, pid, city, strategy)
    }
}
