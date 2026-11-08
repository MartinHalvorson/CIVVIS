//! The research turn: which technology and civic `AdvancedAi` studies next.
//!
//! `advanced_research` picks the tech and civic each turn; `tech_value` and
//! `civic_value` price the candidates, and the helpers here supply the
//! targets it routes toward — the government ladder, unreachable science and
//! housing buildings, unconnected luxuries, boosted bargains, wartime
//! modernization and the barbarian emergency.
//!
//! Moved verbatim out of the one `impl AdvancedAi` block in `advanced.rs`
//! (2026-11-04). Methods the rest of the controller calls are `pub(super)`;
//! the code is otherwise unchanged.

use super::*;

/// `spies-steal-the-lead`: the civic value Diplomatic Service (Spy capacity)
/// gains on a Science aim under the gene, Guilds (its prerequisite) half: the
/// scale of the lanes' own civic bonuses below (Diplomacy +60 for Political
/// Philosophy, Civil Service and Guilds).
pub(crate) const STEAL_CIVIC_VALUE: f64 = 60.0;

impl AdvancedAi {
    /// The technology behind the best Campus building this empire is already
    /// equipped to use and cannot research its way to on merit.
    ///
    /// ## Why a goal is needed and a weight is not enough
    ///
    /// `advanced_research`'s ordinary path is an argmax of `tech_value` over
    /// `available_techs` — **the nodes whose prerequisites are already met.** A
    /// node one step further out is not scored low, it is *not present*. So a
    /// valuable technology sitting behind a worthless one is unreachable no
    /// matter what it is worth.
    ///
    /// That is exactly what the live games do, and #958 is the control that
    /// proves it. Run `civvis-20260803T082856Z` finished turn 251 with **seven
    /// cities, six Campuses, six Universities and zero Research Labs**:
    ///
    /// | tech | held | unlocks |
    /// |---|---|---|
    /// | `scientific_theory` | yes | — |
    /// | **`sanitation`** | **no** | Sewer, Medic — nothing this planner wants |
    /// | **`chemistry`** | **no, and never *available*** | **`research_lab`** |
    /// | `replaceable_parts` | yes | — |
    ///
    /// The empire took Replaceable Parts — one of Chemistry's two prerequisites
    /// — and went on to Composites, Plastics and Combined Arms, while Chemistry
    /// stayed out of reach behind a Sewer. #958 raised what a Research Lab is
    /// worth to `tech_value` by 2.1-2.5x and changed nothing, because the node
    /// was never in the list being scored.
    ///
    /// ## What this returns
    ///
    /// The most valuable science building whose **district and prerequisite
    /// building the empire already owns** and whose technology it does not.
    /// Owning the Campus and the University is the whole qualification: it means
    /// the empire has already paid for everything except the node, so the node
    /// is worth a detour. An empire with no Campus is not sent shopping for
    /// Chemistry.
    ///
    /// Data-driven rather than naming Chemistry, so the same rule reaches
    /// Education for a Library-only empire and keeps working if the ruleset
    /// moves a building to a different node.
    pub(super) fn unreachable_science_building_tech(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<&'static str> {
        if !self.research_economy {
            return None;
        }
        let campus = crate::name!("campus");
        let equipped: Vec<&crate::game::City> = g
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .filter(|city| {
                city.districts
                    .keys()
                    .any(|district| g.district_family(*district) == campus)
            })
            .collect();
        if equipped.is_empty() {
            return None;
        }
        let mut best: Option<(f64, &str)> = None;
        for spec in g.rules.buildings.values() {
            if spec.wonder || spec.yields.science <= 0.0 {
                continue;
            }
            if spec.district.map(|d| g.district_family(d)) != Some(campus) {
                continue;
            }
            let Some(tech) = spec.tech.as_deref() else {
                continue;
            };
            if g.players[pid].techs.contains(&Name::new(tech)) {
                continue;
            }
            // Every prerequisite building must already stand somewhere the
            // district does, or this is a goal the empire cannot cash in.
            let ready = equipped.iter().any(|city| {
                spec.requires
                    .iter()
                    .all(|needed| city.buildings.iter().any(|built| built == needed))
            });
            if !ready {
                continue;
            }
            if best.is_none_or(|(science, _)| spec.yields.science > science) {
                best = Some((spec.yields.science, tech));
            }
        }
        best.map(|(_, tech)| Box::leak(tech.to_string().into_boxed_str()) as &'static str)
    }

    pub(super) fn unreachable_housing_tech(&self, g: &Game, pid: usize) -> Option<&'static str> {
        if self.housing_research_2 {
            return self.usable_housing_tech(g, pid).map(Name::as_str);
        }
        if !self.housing_research {
            return None;
        }
        // Only when the ceiling is genuinely being paid. `housing_growth_mult`
        // pays 1.0 at headroom 2 and halves below it, so 2 is the break-even.
        //
        // ★★★ REPAIRED 2026-08-19 after the 4,000-pair native screen priced
        // the old gate at −2.2 pp wins / −0.67 pp score share (z −3.0 / −4.6,
        // seeds 40000000..): "any one capped city" handed the empire's
        // research slot to the Aqueduct line — the exact hijack the
        // `first_campus_tech` doc records live (Wheel-toward-Engineering at
        // t34, Writing at t83). Two conditions replace it, each restoring a
        // premise the goal always claimed: the ceiling must bind the EMPIRE
        // (at least two capped cities, and at least half of them), and tech
        // must be the binding constraint — a capped city that can already
        // produce a housing-raising building or district is the production
        // lane's job (`housing_buildings`), not a
        // research detour.
        let capped: Vec<u32> = g
            .cities
            .values()
            .filter(|city| {
                city.owner == pid && g.city_housing_headroom(city) < HOUSING_CARD_HEADROOM
            })
            .map(|city| city.id)
            .collect();
        let cities = g.player_city_ids(pid).len();
        if capped.len() < 2 || capped.len() * 2 < cities {
            return None;
        }
        let raises_housing = |item: &Item| match item {
            Item::Building { building } => g
                .rules
                .buildings
                .get(building)
                .is_some_and(|spec| spec.housing > 0.0),
            Item::District { district, .. } => g
                .rules
                .districts
                .get(district)
                .is_some_and(|spec| spec.housing > 0.0),
            _ => false,
        };
        let tech_bound = capped
            .iter()
            .any(|cid| !g.producible_items(pid, *cid).iter().any(raises_housing));
        if !tech_bound {
            return None;
        }
        // The cheapest un-researched tech that unlocks a district this empire
        // could actually place. Asking the ruleset rather than naming
        // `engineering` keeps this correct if Firaxis moves the Aqueduct, and
        // picks up the Sewer's tech on the rare game that gets that far.
        let mut best: Option<(f64, &str)> = None;
        for (family, spec) in g.rules.districts.iter() {
            let raises_housing = matches!(family.as_str(), "aqueduct" | "neighborhood");
            if !raises_housing {
                continue;
            }
            let Some(tech) = spec.tech.as_deref() else {
                continue;
            };
            if g.players[pid].techs.contains(&Name::new(tech)) {
                continue;
            }
            let cost = g
                .rules
                .techs
                .get(&Name::new(tech))
                .map(|t| t.cost)
                .unwrap_or(0.0);
            if best.is_none_or(|(cheapest, _)| cost < cheapest) {
                best = Some((cost, tech));
            }
        }
        best.map(|(_, tech)| Box::leak(tech.to_string().into_boxed_str()) as &'static str)
    }

    /// A quality gap alone does not make a barbarian tech an emergency for a
    /// Science seat.  The production governor already raises a defender for a
    /// nearby ring; research should interrupt the victory chain only when the
    /// simulator says a visible barbarian can actually strike one of our City
    /// Centers on its next fresh turn.  This keeps the frozen controller's
    /// historical behavior intact while making the live gate use the same
    /// movement- and terrain-accurate envelope as the defensive alarm.
    fn barbarian_research_is_urgent(&self, g: &Game, pid: usize) -> bool {
        if !self.battlefront_observation {
            return true;
        }
        if self.barbarian_research_prices_the_field {
            return Self::barbarian_field_attack_is_imminent(g, pid);
        }
        let visible = g.player_vision_frame(pid);
        g.player_city_ids(pid)
            .into_iter()
            .any(|cid| Self::imminent_city_attack(g, pid, cid, &visible))
    }

    /// An active major war must be able to retire its standing land army.
    /// Score bonuses cannot reach a missing prerequisite when a victory lane
    /// owns the research slot. Route one immediate successor explicitly; once
    /// its technology is known, let production/resources/Gold catch up before
    /// requesting another generation. Do not require stockpiled resources here:
    /// the missing prerequisite may be the technology that reveals them.
    pub(super) fn wartime_modernization_tech(&self, g: &Game, pid: usize) -> Option<Name> {
        if !self.victory_planning
            || !g.players.iter().any(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && g.is_at_war(pid, other.id)
            })
        {
            return None;
        }
        let mut goals: BTreeMap<Name, (usize, f64)> = BTreeMap::new();
        for unit in g.units.values().filter(|unit| unit.owner == pid) {
            let held = &g.rules.units[unit.kind];
            if held.class != "military" || matches!(held.domain.as_deref(), Some("sea" | "air")) {
                continue;
            }
            let Some(successor) = held.upgrade_to else {
                continue;
            };
            let successor = &g.rules.units[g.player_unit_replacement(pid, successor)];
            let Some(tech) = successor.tech else { continue };
            if !successor.buildable || g.players[pid].techs.contains(&tech) {
                continue;
            }
            let gain = successor.strength.max(successor.ranged_attack_strength())
                - held.strength.max(held.ranged_attack_strength());
            if gain > 0.0 {
                let entry = goals.entry(tech).or_default();
                entry.0 += 1;
                entry.1 += gain;
            }
        }
        goals
            .into_iter()
            // One obsolete scout must not displace the empire's victory lane.
            .filter(|(_, (count, _))| *count >= 2)
            .max_by(|(a, (_, av)), (b, (_, bv))| {
                self.modernization_research_value(g, pid, *a, *av)
                    .total_cmp(&self.modernization_research_value(g, pid, *b, *bv))
                    .then_with(|| b.cmp(a))
            })
            .map(|(tech, _)| tech)
    }

    pub(super) fn advanced_research(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        // Explicit evaluator targets and the adaptive live plan must drive the
        // same prerequisite search.  Previously only `victory_target` enabled
        // milestone routing, so a normal spectator AI could correctly assess
        // Science or Culture yet wander through generic unlocks indefinitely.
        let objective = self.decision_objective(plan.strategy);
        let civic_objective = if self.victory_portfolio {
            objective
        } else {
            plan.strategy
        };
        let revealed_launch_switch = self.revealed_launch_research_switch(g, pid);
        if g.players[pid].research.is_none() || revealed_launch_switch.is_some() {
            // Every candidate's value reads the empire's science rate, and a
            // comparator asks for two values per comparison: under one memo
            // each city's yields are derived once for the whole choice.
            let memo = g.query_memo();
            let mut available = BasicAi::era_window_techs(g, pid);
            let science_commitment = objective == GrandStrategy::Science
                || self.diplomatic_science_backup(g, pid, plan)
                || (self.science_drive_active() && plan.strategy != GrandStrategy::Recovery);
            let science_lane_active = self.victory_target == Some(VictoryTarget::Science)
                || self.phase_specialization_active(g)
                || self.science_drive_active();
            let foundation_goal = self.science_foundation_research_goal(g, pid, plan);
            let first_launch_goal = self.first_launch_research_goal(g, pid);
            let science_victory_goal = first_launch_goal.or_else(|| {
                if science_lane_active {
                    Self::science_victory_tech_goal(g, pid, objective)
                } else {
                    None
                }
            });
            if let Some(goal) = self.science_endgame_research_goal(g, pid) {
                for tech in g.available_techs(pid) {
                    if self.tech_leads_to(g, &tech, goal) && !available.contains(&tech) {
                        available.push(tech);
                    }
                }
            }
            let great_person_goal = BasicAi::live_great_person_tech_goal(g, pid);
            // The rolling window protects unattended research from leaving
            // an old branch behind. An explicit Science target (or an
            // adaptive seat that is already driving) is a different contract:
            // when the next victory milestone is already legal, do not make
            // it wait for unrelated backlog just because its era is outside
            // that floor. Prerequisites still come through the normal window;
            // this only admits the exact, already-unlocked milestone.
            let science_milestone = science_victory_goal
                .filter(|_| {
                    great_person_goal.is_none()
                        && (self.victory_target == Some(VictoryTarget::Science)
                            || (self.portfolio_target() == Some(VictoryTarget::Science)
                                && self.portfolio_specializing() == Some(true))
                            || self.science_drive_active())
                })
                .and_then(|goal| {
                    (!available.iter().any(|tech| tech.as_str() == goal))
                        .then(|| {
                            g.available_techs(pid)
                                .into_iter()
                                .find(|tech| tech.as_str() == goal)
                        })
                        .flatten()
                });
            if let Some(milestone) = science_milestone {
                if !available.contains(&milestone) {
                    available.push(milestone);
                }
            }
            let science_harbor_goal = self.science_harbor_research_goal(g, pid, objective);
            let luxury_goal = self.unconnected_luxury_tech(g, pid);
            let bargain_goal = self.boosted_bargain_tech(g, pid, &available);
            // `boosted-bargain-first-3`: the lane goals the bargain would
            // displace, in the order the match below ranks them, so the
            // veto can see whether the beeline is about to land its node.
            let lane_target: Option<&str> = science_harbor_goal
                .or(science_victory_goal)
                .or(great_person_goal.as_deref());
            let bargain_goal_3 =
                self.boosted_bargain_tech_3(g, pid, plan, &available, lane_target, None);
            let barbarian_military_goal = if self.base.barbarian_tactics_enabled()
                && (objective != GrandStrategy::Science
                    || self.barbarian_research_is_urgent(g, pid))
            {
                self.barbarian_research_upgrade_goal(g, pid)
            } else {
                None
            };
            let opening_archery_goal = self.opening_archery_goal(g, pid);
            let defensive_walls_goal = self
                .frontier_walls_research_goal(g, pid)
                .or_else(|| self.defensive_walls_research_goal(g, pid, plan));
            let standing_army_fuel_goal = self.standing_army_fuel_goal(g, pid);
            // `breaker-research-first`: no non-breaker upgrade while a siege
            // is held for its wall-breaker. See `advanced/decisive_window.rs`.
            let wartime_modernization_goal = self
                .wartime_modernization_tech(g, pid)
                .filter(|tech| !self.modernization_yields_to_the_breaker(g, pid, *tech));
            let production_technology_goal = self.named_production_technology_goal(g, pid, plan);
            let domination_siege_goal = self.domination_siege_research_goal(g, pid, plan);
            let domination_campus_goal = self.domination_campus_unlock_goal(g, pid);
            let every_lane_campus_goal = self.every_lane_campus_unlock_goal(g, pid);
            let science_writing_goal = self.science_writing_before_the_luxury_goal(g, pid);
            let endgame_goal = self.science_endgame_research_goal(g, pid);
            let endgame_research_preempts_wartime =
                self.science_endgame_research_preempts_wartime(g, pid, endgame_goal);
            let conversion_upgrade_goal = self
                .conversion_upgrade_research()
                .filter(|goal| !g.players[pid].techs.contains(&Name::new(goal)));
            // `decisive-window`: the cheapest research toward an assault and a
            // breaker that beat the campaign target's defender and walls. See
            // `advanced/decisive_window.rs`.
            let decisive_window = self.decisive_window(g, pid, plan);
            let decisive_window_goal =
                self.decisive_window_research_goal(g, pid, decisive_window.as_ref());
            // Say what the window reads when it does not take the slot, so a
            // live readout can tell "already armed" from "nothing in reach".
            if self.decisive_window
                && decisive_window_goal.is_none()
                && self.journal().wants(crate::reasoning::Level::Detail)
            {
                match &decisive_window {
                    Some(window)
                        if window.tech_goal.is_some_and(|goal| {
                            self.window_yields_to_the_bombers(g, pid, goal)
                        }) =>
                    {
                        think!(self.journal(), Research, Detail, "decisive-window: yields to the bombers";
                               "{} for {} lies off the Advanced Flight path, and the air surge's research owns the slot ({} techs from the Bomber)",
                               plain(window.tech_goal.map_or("", Name::as_str)),
                               plain(window.assault.as_str()),
                               Self::air_surge_missing_techs(g, pid))
                    }
                    Some(window) if window.tech_goal.is_some() => {
                        think!(self.journal(), Research, Detail, "decisive-window: yields to the faith veto";
                               "{} is owed for {}, but a rival faith is at the bar and Astrology comes first",
                               plain(window.tech_goal.map_or("", Name::as_str)),
                               plain(window.assault.as_str()))
                    }
                    Some(window) => {
                        think!(self.journal(), Research, Detail, "decisive-window: open";
                               "{} beats {}'s best defender ({:.0}) by {:.0}{}",
                               plain(window.assault.as_str()),
                               g.players[window.target].civ,
                               window.defender,
                               window.margin,
                               window
                                   .breaker
                                   .map(|breaker| format!(
                                       " and {} opens tier-{} walls",
                                       plain(breaker.as_str()),
                                       window.wall_tier
                                   ))
                                   .unwrap_or_default())
                    }
                    None => {
                        think!(self.journal(), Research, Detail, "decisive-window: no package";
                               "nothing this civilization can train beats the target by {:.0} and opens its walls within {} turns of research",
                               decisive_window::DECISIVE_MARGIN,
                               g.standard_duration(decisive_window::DECISIVE_RESEARCH_HORIZON))
                    }
                }
            }
            // `engineering-for-capped-fresh-water`: read once, only if every arm
            // above it passes. See the arm below.
            let capped_engineering = std::cell::OnceCell::new();
            // `siege-tier-before-the-beeline`: read once, only if every arm
            // above it passes. See the arm below.
            let siege_tier = std::cell::OnceCell::new();
            // `old-techs-before-the-beeline`: read once, only if every arm
            // above it passes. See the arm below.
            let old_techs = std::cell::OnceCell::new();
            let forced_goal = match objective {
                // `capital-walls-before-the-surprise`: Masonry for the capital
                // goes ahead of the opening Archery too. 040735Z's slot opened
                // at turn 11 to eight turns of Archery, which left the walls
                // at about turn 31 against a capture at 29.
                _ if defensive_walls_goal.is_some()
                    && (self.capital_walls_research_threat(g, pid).is_some()
                        || self.frontier_walls_research_goal(g, pid).is_some()) =>
                {
                    defensive_walls_goal.as_deref()
                }
                _ if opening_archery_goal.is_some() => opening_archery_goal.as_deref(),
                _ if defensive_walls_goal.is_some() => defensive_walls_goal.as_deref(),
                // `diplo-match-point-beelines-the-points`: from match point the
                // last points in the tree, whatever the plan reads, ahead of
                // every war and lane arm below. Live Immortal
                // civvis-20261031T011801Z stood on 19 of 20 from t172 while
                // Rocketry (`domination-siege-research`), Guidance Systems (the
                // wartime modernization) and then the counter plan's Satellites
                // and Electricity took the slot; Seasteads started at t201 and
                // Rome won on Science at t205. Behind only the walls and the
                // opening Archery, which are done long before the bar. Exact
                // no-op while the gene is off.
                _ if self.diplo_match_point_tech(g, pid).is_some() => {
                    self.diplo_match_point_tech(g, pid)
                }
                _ if conversion_upgrade_goal.is_some() => conversion_upgrade_goal,
                _ if self.war_plan.as_ref().is_some_and(|plan| {
                    !g.players[pid].techs.contains(&plan.breakthrough_tech)
                }) =>
                {
                    self.war_plan
                        .as_ref()
                        .map(|plan| plan.breakthrough_tech.as_str())
                }
                _ if g.has_ability(pid, "taxis")
                    && g.players[pid].religion.is_none()
                    && !g.players[pid].techs.contains(&crate::name!("astrology")) =>
                {
                    Some("astrology")
                }
                // `found-against-a-rival-faith`: one cheap Ancient node opens
                // the Prophet race a rival faith has made worth running. See
                // `advanced/faith_veto.rs`.
                _ if self.faith_veto_due(g, pid)
                    && self.prophet_race_enterable_for(g, pid, self.victory_target)
                    && !g.players[pid].techs.contains(&crate::name!("astrology")) =>
                {
                    Some("astrology")
                }
                // The air surge's beeline. Behind the appointed timed war's
                // breakthrough (that package is already half-built when it
                // holds the research slot) and behind the Taxis religion
                // opening, ahead of everything else: three technologies is
                // the whole appointment, and a lane goal that displaces one
                // of them turns the surge into a research plan that never
                // launches. See `advanced/air_surge.rs`.
                // `decisive-window`, ahead of the air surge: a package whose
                // assault and breaker already beat the target needs no
                // research and yields the slot, and once the target reaches
                // Urban Defenses the Bomber is itself a priced breaker, so the
                // two goals converge rather than compete. Live King: every
                // breaker arrived one wall tier late while the surge and the
                // one-step modernization owned research.
                _ if decisive_window_goal.is_some() => decisive_window_goal.map(Name::as_str),
                // `engineering-for-capped-fresh-water`: behind the breaker and
                // ahead of the air surge and every lane beeline, a technology a
                // turn or two away that unlocks the Aqueducts two capped
                // cities are waiting on. One-shot: it ends once known. See
                // `advanced/engineering_for_capped_fresh_water.rs`.
                _ if capped_engineering
                    .get_or_init(|| self.capped_fresh_water_engineering_goal(g, pid))
                    .is_some() =>
                {
                    capped_engineering
                        .get()
                        .copied()
                        .flatten()
                        .map(|pick| self.capped_engineering_pick(&pick))
                }
                // `siege-tier-before-the-beeline`: behind Medieval walls the
                // next fed siege design's technology, a few turns away, ahead
                // of the air surge and every lane beeline while the Bomber is
                // still far. See `advanced/siege_tier_before_the_beeline.rs`.
                _ if siege_tier
                    .get_or_init(|| {
                        self.siege_tier_before_the_beeline_goal(g, pid, plan, domination_siege_goal)
                    })
                    .is_some() =>
                {
                    siege_tier
                        .get()
                        .copied()
                        .flatten()
                        .map(|pick| self.siege_tier_pick(&pick))
                }
                // Open labs and the first pad before advanced backlog or air
                // research, retaining emergencies, fuel and named production.
                _ if foundation_goal.is_some()
                    && barbarian_military_goal.is_none()
                    && standing_army_fuel_goal.is_none()
                    && production_technology_goal.is_none() =>
                {
                    foundation_goal
                }
                // `old-techs-before-the-beeline`: behind the siege tier and
                // ahead of the air surge and every lane beeline, the
                // technology our land army waits on to upgrade once the world
                // has left its era, then any technology two eras behind the
                // world. See `advanced/old_techs_before_the_beeline.rs`.
                _ if old_techs
                    .get_or_init(|| {
                        self.old_techs_before_the_beeline_goal(
                            g,
                            pid,
                            endgame_research_preempts_wartime || first_launch_goal.is_some(),
                        )
                    })
                    .is_some() =>
                {
                    old_techs
                        .get()
                        .copied()
                        .flatten()
                        .map(|pick| self.old_tech_pick(g, &pick))
                }
                _ if self.air_surge_research_goal(g, pid).is_some() => {
                    self.air_surge_research_goal(g, pid)
                }
                // An ancient rush rides. `rush_census` measures **0% of
                // empires holding `horseback_riding` at turn 50**, so the
                // stack is warriors — and a warrior attacking the measured
                // capital takes 28 damage a blow and dies on its fourth,
                // having dealt 134 of the 200 needed. That is the whole
                // reason a war declared at turn 30 does not take a city until
                // turn 53: the stack trades itself, and the next one has to be
                // built and marched all over again.
                //
                // A horseman is strength 36 for 80 production against a
                // warrior's 20 for 40 — it takes 15 a blow instead of 28, so
                // it survives the siege rather than paying for it — and it has
                // **4 movement against 2**, which halves the nine-to-twelve
                // turn march that is the lane's other binding cost. At 195
                // science it is also cheaper to reach than iron working's 225.
                _ if plan.rush
                    && !g.players[pid]
                        .techs
                        .contains(&crate::name!("horseback_riding")) =>
                {
                    Some("horseback_riding")
                }
                // A nearby barbarian with a materially stronger unit is an
                // immediate modernization problem, even when the empire is
                // pursuing Science or another peaceful victory. The goal is
                // the strongest upgrade supported by the army already in the
                // field; `goal_pick` below walks its prerequisites.
                _ if barbarian_military_goal.is_some() => barbarian_military_goal.as_deref(),
                // A committed launch chain has already paid for its Spaceport
                // and serial projects. Finish the next science rung before an
                // optional standing-army upgrade, so a Recovery turn cannot
                // strand the expedition without its next launch or laser tech.
                _ if endgame_research_preempts_wartime => endgame_goal,
                _ if standing_army_fuel_goal.is_some() => standing_army_fuel_goal.as_deref(),
                _ if production_technology_goal.is_some() => production_technology_goal
                    .as_ref()
                    .map(|tech| tech.as_str()),
                // An owned Rocketry commits the first launch through Recovery
                // and optional modernization; urgent defenses remain above.
                _ if first_launch_goal.is_some() => first_launch_goal,
                _ if wartime_modernization_goal.is_some() => wartime_modernization_goal.as_deref(),
                _ if domination_siege_goal.is_some() => domination_siege_goal.as_deref(),
                // Once the late launch chain is committed, finish its remaining
                // research before optional economic and bargain detours.
                _ if endgame_goal.is_some() => endgame_goal,
                // `enter-the-prophet-race-2`: Astrology is a dead-end branch no
                // lane goal is an ancestor of, so no beeline ever reaches it.
                // Take it once the opening techs are in, while a Prophet slot
                // is still open for this seat.
                // `astrology-before-the-second-city`: a lone capital takes it
                // at the same point, so the Holy Site can start the turn the
                // second city stands. See
                // `advanced/astrology_before_the_second_city.rs`.
                // `campus-unlock-before-the-luxury`: the Campus unlock ahead
                // of the Astrology and luxury-connection detours. See
                // `advanced/domination_research.rs`.
                // Science can take the same earlier slot without moving any
                // military, barbarian, wall or committed launch goal above it.
                _ if science_writing_goal.is_some() => science_writing_goal,
                _ if self.campus_unlock_before_the_luxury && domination_campus_goal.is_some() => {
                    domination_campus_goal
                }
                _ if (self.prophet_race_enterable_for(g, pid, self.victory_target)
                    || self.astrology_before_the_second_city_due(g, pid))
                    && g.players[pid].techs.len() >= PROPHET_RACE_OPENING_TECHS
                    && !g.players[pid].techs.contains(&crate::name!("astrology")) =>
                {
                    Some("astrology")
                }
                // `connect-the-luxury`: Irrigation and Sailing are dead-end
                // branches no lane goal is an ancestor of, so no beeline ever
                // reaches them — nine live King games researched Irrigation
                // 0 times by t150 while owning a mean 3.1 unimproved
                // Plantation or sea luxuries within three tiles of a city,
                // and ran Displeased on 48 % of city-turns (−10 % on every
                // yield, ≈1,150 yield points a game). Take the cheapest
                // connecting tech once the opening is in; the beeline
                // resumes the turn it lands.
                _ if luxury_goal.is_some()
                    && g.players[pid].techs.len() >= PROPHET_RACE_OPENING_TECHS =>
                {
                    luxury_goal
                }
                // Two founded cities can invest in campuses. Finish their
                // unlock before optional bargains and generic branch scores.
                _ if domination_campus_goal.is_some() => domination_campus_goal,
                // `campus-unlock-every-lane`: the same unlock for a Culture,
                // Religion or adaptive seat, which otherwise has no Writing
                // goal at all. See `advanced/domination_research.rs`.
                _ if every_lane_campus_goal.is_some() => every_lane_campus_goal,
                // `boosted-bargain-first`: a Eureka in hand on a technology
                // two turns from done is a tech for a fifth of its price, and
                // the beeline walks past it every turn. Behind the luxury
                // connection (an amenity for four cities), ahead of the lane.
                _ if bargain_goal.is_some() => bargain_goal,
                // `boosted-bargain-first-3`: the same slot as v1 — behind
                // every war, barbarian, religion and luxury contract — but a
                // one-turn finish only, on a peaceful seat whose lane is not
                // about to land its target. See `boosted_bargain_tech_3`.
                _ if bargain_goal_3.is_some() => bargain_goal_3.as_deref(),
                _ if science_harbor_goal.is_some() => science_harbor_goal,
                _ if science_victory_goal.is_some() => science_victory_goal,
                _ if great_person_goal.is_some() => great_person_goal.as_deref(),
                _ if science_commitment && science_lane_active => {
                    Self::science_victory_tech_goal(g, pid, GrandStrategy::Science)
                }
                GrandStrategy::Culture if self.phase_specialization_active(g) => {
                    ["printing", "radio", "computers"]
                        .into_iter()
                        .find(|tech| !g.players[pid].techs.contains(&Name::new(tech)))
                }
                GrandStrategy::Diplomacy
                    if self.phase_specialization_active(g)
                        && !g.players[pid].techs.contains(&crate::name!("seasteads")) =>
                {
                    Some("seasteads")
                }
                GrandStrategy::Religion
                    if !g.players[pid].techs.contains(&crate::name!("astrology")) =>
                {
                    Some("astrology")
                }
                _ => None,
            };
            // Last, so no lane loses its own opening to it: a Campus building
            // the empire is already equipped for but cannot reach. This is the
            // only goal here that exists because the target is **absent from
            // the candidate list**, not because it ranks below something else —
            // see `unreachable_science_building_tech`. `goal_pick` below walks
            // the cheapest legal step toward it, which is what crosses the
            // worthless prerequisite the argmax will never take on merit.
            let forced_goal = forced_goal.or_else(|| {
                science_lane_active
                    .then(|| self.unreachable_science_building_tech(g, pid))
                    .flatten()
            });
            // And after THAT, the growth ceiling: a Campus the empire can reach
            // still only makes beakers out of citizens it is allowed to have.
            // Behind the science goals on purpose — this never takes a slot the
            // science chain wanted, it takes one the argmax would have spent on
            // whatever happened to be cheapest.
            let forced_goal = forced_goal.or_else(|| self.unreachable_housing_tech(g, pid));
            // An already-legal Science milestone outside the rolling window
            // is the one deliberate exception to the cheapest-prerequisite
            // picker. Keep live Great Person backfill on the original picker:
            // those seats intentionally clear the old branch before resuming
            // their long Science beeline.
            let science_milestone_pick = science_milestone.filter(|_| {
                great_person_goal.is_none()
                    && science_victory_goal.is_some()
                    && forced_goal == science_victory_goal
            });
            // An appointed wing must be allowed to finish its already legal
            // breakthrough. The rolling era window otherwise strands Advanced
            // Flight behind unrelated Renaissance backlog after Radio lands.
            // Only admit the exact milestone when it owns the forced goal;
            // prerequisites and higher-priority military goals keep their rules.
            if let Some(goal) = self.air_surge_research_goal(g, pid) {
                if forced_goal == Some(goal) {
                    // `air-surge-2`, Domination lane: every legal step on the
                    // chain, not only the milestone. With one Medieval tech
                    // unresearched the window ends at the Industrial era, so
                    // Flight and Radio (Modern) never become candidates: live
                    // King 20261001T022028Z researched Economics and Castles
                    // at turns 119-123 between Steam Power and Flight, and
                    // 225143Z researched Economics three techs from the
                    // Bomber at turn 140.
                    let whole_chain = self.air_surge_2
                        && self.active_victory_target(g) == Some(VictoryTarget::Domination);
                    for tech in g.available_techs(pid) {
                        if (tech.as_str() == goal
                            || (whole_chain && self.tech_leads_to(g, &tech, goal)))
                            && !available.contains(&tech)
                        {
                            available.push(tech);
                        }
                    }
                }
            }
            // The window's chain crosses eras the rolling window does not
            // offer yet (Military Science behind a Medieval backlog): admit
            // every legal step toward it while it owns the forced goal.
            if let Some(goal) =
                decisive_window_goal.filter(|goal| forced_goal == Some(goal.as_str()))
            {
                for tech in g.available_techs(pid) {
                    if self.tech_leads_to(g, &tech, goal.as_str()) && !available.contains(&tech) {
                        available.push(tech);
                    }
                }
            }
            // `siege-tier-before-the-beeline`: Metal Casting and Ballistics sit
            // a Renaissance step beyond a Medieval backlog the rolling window
            // still holds; admit every legal step toward the siege technology
            // while it owns the forced goal.
            if let Some(pick) = siege_tier
                .get()
                .copied()
                .flatten()
                .filter(|pick| forced_goal == Some(pick.tech.as_str()))
            {
                for tech in g.available_techs(pid) {
                    if self.tech_leads_to(g, &tech, pick.tech.as_str())
                        && !available.contains(&tech)
                    {
                        available.push(tech);
                    }
                }
            }
            // An army already consuming an unrevealed fuel cannot be supplied
            // by unrelated era backfill. Admit only legal steps on its selected
            // supply path, and only when no earlier commitment owns research.
            if let Some(goal) = standing_army_fuel_goal {
                if forced_goal == Some(goal.as_str()) {
                    for tech in g.available_techs(pid) {
                        if self.tech_leads_to(g, &tech, goal.as_str()) && !available.contains(&tech)
                        {
                            available.push(tech);
                        }
                    }
                }
            }
            // The committed first launch may cross the rolling era window.
            // Admit only its available prerequisites, only while it owns the goal.
            if first_launch_goal.is_some() && forced_goal == first_launch_goal {
                for tech in g.available_techs(pid) {
                    if self.tech_leads_to(g, &tech, "satellites") && !available.contains(&tech) {
                        available.push(tech);
                    }
                }
            }
            if let Some(goal) = foundation_goal.filter(|goal| forced_goal == Some(*goal)) {
                for tech in g.available_techs(pid) {
                    if self.tech_leads_to(g, &tech, goal) && !available.contains(&tech) {
                        available.push(tech);
                    }
                }
            }
            self.admit_science_chain_steps(
                g,
                pid,
                science_victory_goal,
                forced_goal,
                &mut available,
            );
            let production_step_pick = production_technology_goal
                .filter(|goal| forced_goal == Some(goal.as_str()))
                .and_then(|goal| self.named_production_technology_step(g, pid, goal));
            let goal_pick = science_milestone_pick.or(production_step_pick).or_else(|| {
                forced_goal.and_then(|goal| {
                    if self.beeline_orders_by_value {
                        let steps: Vec<Name> = available
                            .iter()
                            .filter(|tech| self.tech_leads_to(g, tech, goal))
                            .cloned()
                            .collect();
                        return self.beeline_step(g, pid, plan.strategy, &steps, true);
                    }
                    available
                        .iter()
                        .filter(|tech| self.tech_leads_to(g, tech, goal))
                        // The cheapest step by what it will actually cost: the
                        // printed price with `chase-every-boost` off, the price
                        // less the boost in hand with it on. See
                        // `advanced/chase_every_boost.rs`.
                        .min_by(|a, b| {
                            self.beeline_step_cost(g, pid, a.as_str(), true)
                                .partial_cmp(&self.beeline_step_cost(g, pid, b.as_str(), true))
                                .unwrap()
                                .then(a.cmp(b))
                        })
                        .cloned()
                })
            });
            let fallback_pick = available
                .iter()
                .max_by(|a, b| {
                    self.tech_value(g, pid, a, plan.strategy)
                        .partial_cmp(&self.tech_value(g, pid, b, plan.strategy))
                        .unwrap()
                        .then_with(|| b.cmp(a))
                })
                .cloned();
            // V1 above can replace a forced goal merely because a side node
            // is cheap. V2 enters only here, after `goal_pick` is absent, and
            // may replace the ordinary fallback only with a one-turn boosted
            // node of nearly equal board value. `boost-first-research-2` sits
            // behind it at the same seam: the boost-in-hand scale as a
            // tie-break among comparable candidates, never over a forced
            // goal. See `advanced/boost_research.rs`.
            let pick = if let Some(goal_pick) = goal_pick {
                Some(goal_pick)
            } else {
                fallback_pick.map(|ordinary| {
                    self.boosted_bargain_tech_2(g, pid, plan.strategy, &available, &ordinary)
                        .or_else(|| {
                            self.boosted_bargain_tech_3(
                                g,
                                pid,
                                plan,
                                &available,
                                None,
                                Some(&ordinary),
                            )
                        })
                        .or_else(|| {
                            self.boost_tiebreak_pick(
                                g,
                                pid,
                                plan.strategy,
                                &available,
                                &ordinary,
                                true,
                            )
                        })
                        .unwrap_or(ordinary)
                })
            };
            // Reconsider an in-flight path only for the observed milestone,
            // and only when the ordinary priority ladder also gives it the slot.
            let pick = pick.filter(|tech| {
                g.players[pid].research.is_none()
                    || (revealed_launch_switch == Some(tech.as_str())
                        && forced_goal == Some(tech.as_str()))
            });
            if let Some(tech) = pick {
                if self.journal().wants(crate::reasoning::Level::Decision) {
                    // `beeline-orders-by-value` takes the best-valued step,
                    // not the cheapest; say which.
                    let step = if self.beeline_orders_by_value {
                        "most valuable"
                    } else {
                        "cheapest"
                    };
                    let why = match (forced_goal, &goal_pick) {
                        (Some(goal), Some(_)) => {
                            if foundation_goal == Some(goal) {
                                format!("science-foundation-before-backlog: the {step} step toward {}, opening usable labs and the first Spaceport before advanced backlog", plain(goal))
                            } else if opening_archery_goal.as_deref() == Some(goal) {
                                format!("the first range-two defender is needed against nearby barbarians; unlock {} before the economic beeline", plain(goal))
                            } else if defensive_walls_goal.as_deref() == Some(goal) {
                                if let Some((city, rival, ratio)) =
                                    self.frontier_walls_research_threat(g, pid)
                                {
                                    format!(
                                        "frontier-walls-before-the-surprise: unlock Ancient Walls for {}; {} fields {ratio:.1}x our power and its soldier was seen near this frontier",
                                        g.cities[&city].name, g.players[rival].civ
                                    )
                                } else {
                                    // `capital-walls-before-the-surprise` names its own reason.
                                    match self.capital_walls_research_threat(g, pid) {
                                        Some((rival, ratio)) => format!(
                                            "capital-walls-before-the-surprise: unlock Ancient Walls for the capital; {} fields {ratio:.1}x our power and its soldiers stood within {} tiles",
                                            g.players[rival].civ,
                                            capital_walls_before_the_surprise::CAPITAL_WATCH_TILES
                                        ),
                                        None => "unlock Ancient Walls for wartime recovery or a stronger hostile approaching an exposed city before the longer army upgrade path".to_string(),
                                    }
                                }
                            } else if barbarian_military_goal.as_deref() == Some(goal) {
                                format!(
                                    "the {step} step toward {}, needed to catch a nearby barbarian army",
                                    plain(goal)
                                )
                            } else if let Some(window) = decisive_window
                                .as_ref()
                                .filter(|window| window.tech_goal.map(Name::as_str) == Some(goal))
                            {
                                format!(
                                    "decisive-window: the {step} step toward {}; {}{} {} {}'s best defender ({:.0}) by {:.0}{} in {:.0} turns",
                                    plain(goal),
                                    plain(window.assault.as_str()),
                                    if window.unique { " (our unique unit)" } else { "" },
                                    if window.margin >= decisive_window::DECISIVE_MARGIN {
                                        "beats".to_string()
                                    } else {
                                        format!("at {:.1}x their power faces", window.power_ratio)
                                    },
                                    g.players[window.target].civ,
                                    window.defender,
                                    window.margin,
                                    window
                                        .breaker
                                        .map(|breaker| format!(
                                            " and {} opens tier-{} walls",
                                            plain(breaker.as_str()),
                                            window.wall_tier
                                        ))
                                        .unwrap_or_default(),
                                    window.turns,
                                )
                            } else if old_techs.get().copied().flatten().is_some_and(
                                |pick: old_techs_before_the_beeline::OldTechPick| {
                                    pick.tech.as_str() == goal
                                },
                            ) {
                                format!(
                                    "old-techs-before-the-beeline: the {step} step toward {}, owed from an era the world has left",
                                    plain(goal)
                                )
                            } else if domination_siege_goal.as_deref() == Some(goal) {
                                format!("domination-siege-research: unlock {} to supply the missing wall-breaking capability for the campaign", plain(goal))
                            } else if standing_army_fuel_goal.as_deref() == Some(goal) {
                                format!("the {step} step toward {}, needed to reveal fuel for the standing army with no reserve", plain(goal))
                            } else if production_technology_goal == Some(Name::new(goal)) {
                                format!(
                                    "the {step} step toward {}, needed to unlock production on currently worked tiles",
                                    plain(goal)
                                )
                            } else if wartime_modernization_goal.as_deref() == Some(goal) {
                                format!(
                                    "the {step} step toward {}, needed to modernize the standing army at war",
                                    plain(goal)
                                )
                            } else if domination_campus_goal == Some(goal) {
                                "unlock campuses for the expanding domination economy".to_string()
                            } else {
                                format!(
                                    "the {step} step toward {}, which {} needs",
                                    plain(goal),
                                    objective.as_str()
                                )
                            }
                        }
                        _ => {
                            let runner_up = available
                                .iter()
                                .filter(|other| **other != tech)
                                .max_by(|a, b| {
                                    self.tech_value(g, pid, a, plan.strategy)
                                        .partial_cmp(&self.tech_value(g, pid, b, plan.strategy))
                                        .unwrap()
                                        .then_with(|| b.cmp(a))
                                })
                                .map(|other| {
                                    format!(
                                        "ahead of {} at {:.0}",
                                        plain(other),
                                        self.tech_value(g, pid, other, plan.strategy)
                                    )
                                })
                                .unwrap_or_else(|| "with nothing else on offer".to_string());
                            format!(
                                "worth {:.0} to the {} plan, {runner_up}",
                                self.tech_value(g, pid, &tech, plan.strategy),
                                plan.strategy.as_str()
                            )
                        }
                    };
                    think!(self.journal(), Research, Decision, "Researching {}", plain(&tech); "{why}");
                }
                drop(memo);
                let _ = g.apply(
                    pid,
                    &Action::Research {
                        tech: Name::new(&tech),
                    },
                );
            }
        }
        if g.players[pid].civic.is_none() {
            let memo = g.query_memo();
            let available = g.available_civics(pid);
            // A victory beeline cannot usefully precede the government's
            // policy capacity. Science formerly aimed at Space Race from turn
            // one, letting Games and Drama delay Political Philosophy while
            // the empire needed Colonization. Secure the first government,
            // then resume the victory lane.
            let first_government = self.wide_map_capacity
                && !g.players[pid]
                    .civics
                    .contains(&crate::name!("political_philosophy"));
            let great_person_goal = BasicAi::live_great_person_civic_goal(g, pid);
            // `decisive-window`: a civic-gated window unit (the Samurai's
            // Feudalism, the Tagma's Divine Right, the Winged Hussar's
            // Mercantilism). See `advanced/decisive_window.rs`.
            let decisive_civic_goal = self.decisive_window_civic_goal(g, pid, plan);
            let forced_goal = match objective {
                _ if g.has_ability(pid, "taxis")
                    && !g.players[pid]
                        .civics
                        .contains(&crate::name!("divine_right")) =>
                {
                    Some("divine_right")
                }
                _ if great_person_goal.is_some() => great_person_goal.as_deref(),
                // See `advanced/culture_drama_first.rs`. No-ops with both
                // genes off.
                _ if self.culture_drama_before_government(g, pid) => Some("drama_poetry"),
                _ if first_government => Some("political_philosophy"),
                _ if self.culture_drama_after_government(g, pid) => Some("drama_poetry"),
                // `diplo-match-point-beelines-the-points`: the civic's point,
                // whatever the plan reads -- ahead of the war civics and the
                // government ladder. Live Immortal civvis-20261030T190332Z
                // stood on 18 of 20 from t172, its civic slot on
                // `government_ladder_goal`'s Corporate Libertarianism, and lost
                // to Technology at t175. Exact no-op while the gene is off.
                _ if self.diplo_match_point_civic(g, pid).is_some() => {
                    self.diplo_match_point_civic(g, pid)
                }
                // Put an already-built museum to work before buying more
                // government capacity. Recovery keeps the normal ladder.
                GrandStrategy::Culture
                    if plan.strategy == GrandStrategy::Culture
                        && !self.lane_lost
                        && self.culture_museum_unlock_goal(g, pid).is_some() =>
                {
                    self.culture_museum_unlock_goal(g, pid)
                }
                // The active Culture plan needs its Faith-to-tourism unlocks.
                // A later government's cheapest path can postpone Conservation
                // and take Professional Sports before Cold War.
                // An assigned Culture lane keeps this priority through a
                // temporary counterplan once it has six policy slots. Native
                // 040449Z reached 121 Faith/turn but lost at 179 without Cold
                // War after researching Class Struggle and then Suffrage.
                GrandStrategy::Culture
                    if self.culture_cold_war_window
                        && (plan.strategy == GrandStrategy::Culture
                            || (self.active_victory_target(g) == Some(VictoryTarget::Culture)
                                && {
                                    let slots = g.gov_slots(pid);
                                    slots.military
                                        + slots.economic
                                        + slots.diplomatic
                                        + slots.wildcard
                                        >= 6
                                }))
                        && !self.lane_lost
                        && matches!(
                            self.culture_civic_goal(g, pid),
                            Some("conservation" | "cold_war")
                        ) =>
                {
                    self.culture_civic_goal(g, pid)
                }
                _ if decisive_civic_goal.is_some() => decisive_civic_goal.map(Name::as_str),
                _ if self.domination_upgrade_civic_goal(g, pid).is_some() => {
                    self.domination_upgrade_civic_goal(g, pid)
                }
                // Filled museums can use this tourism card now; eight slots
                // already provide room for it. Keep emergency and unit-window
                // goals above this, and return to the ladder after the unlock.
                GrandStrategy::Culture if self.culture_heritage_unlock_goal(g, pid).is_some() => {
                    self.culture_heritage_unlock_goal(g, pid)
                }
                // See `government_ladder`: the same sentence one rung up. The
                // tier-1 arm above exists because "a victory beeline cannot
                // usefully precede the government's policy capacity"; tier 2
                // is where two thirds of that capacity actually is, and no
                // lane ever asks for it.
                _ if self.government_ladder_goal(g, pid, objective).is_some() => {
                    self.government_ladder_goal(g, pid, objective)
                }
                // See `lane_release_when_hopeless`: a lane that cannot be won
                // does not get to spend the rest of the game's civics on its
                // own beeline. The live King seat took "the cheapest step
                // toward global warming mitigation" for every civic from turn
                // 107 to turn 229 while its diplomatic race stood at 2 points
                // against 19 -- a path that crosses no government civic at
                // all, and never arrived. Released, `civic_value`'s argmax
                // takes the slot back.
                _ if self.lane_lost => None,
                GrandStrategy::Culture => self.culture_civic_goal(g, pid),
                GrandStrategy::Science
                    if !g.players[pid].civics.contains(&crate::name!("space_race")) =>
                {
                    Some("space_race")
                }
                GrandStrategy::Diplomacy
                    if !g.players[pid]
                        .civics
                        .contains(&crate::name!("global_warming_mitigation")) =>
                {
                    Some("global_warming_mitigation")
                }
                GrandStrategy::Religion
                    if !g.players[pid].civics.contains(&crate::name!("theology")) =>
                {
                    Some("theology")
                }
                _ => None,
            };
            let goal_pick = forced_goal.and_then(|goal| {
                if self.beeline_orders_by_value {
                    let steps: Vec<Name> = available
                        .iter()
                        .filter(|civic| self.civic_leads_to(g, civic, goal))
                        .cloned()
                        .collect();
                    return self.beeline_step(g, pid, civic_objective, &steps, false);
                }
                available
                    .iter()
                    .filter(|civic| self.civic_leads_to(g, civic, goal))
                    // The civic half of the same step cost; see
                    // `advanced/chase_every_boost.rs`.
                    .min_by(|a, b| {
                        self.beeline_step_cost(g, pid, a.as_str(), false)
                            .partial_cmp(&self.beeline_step_cost(g, pid, b.as_str(), false))
                            .unwrap()
                            .then(a.cmp(b))
                    })
                    .cloned()
            });
            // The civic half of the `boost-first-research-2` tie-break, at
            // the same seam: after the forced goal, over the ordinary argmax.
            // See `advanced/boost_research.rs`.
            let pick = goal_pick.or_else(|| {
                available
                    .iter()
                    .max_by(|a, b| {
                        self.civic_value(g, pid, a, civic_objective)
                            .partial_cmp(&self.civic_value(g, pid, b, civic_objective))
                            .unwrap()
                            .then_with(|| b.cmp(a))
                    })
                    .cloned()
                    .map(|ordinary| {
                        self.boost_tiebreak_pick(
                            g,
                            pid,
                            plan.strategy,
                            &available,
                            &ordinary,
                            false,
                        )
                        .unwrap_or(ordinary)
                    })
            });
            // `civic-awaits-its-inspiration`: a civic that would finish before
            // its near inspiration lands gives way. See
            // `advanced/civic_inspiration_wait.rs`.
            let pick = pick.map(|civic| {
                self.civic_awaits_its_inspiration_pick(
                    g,
                    pid,
                    &available,
                    &civic,
                    goal_pick.and(forced_goal),
                    civic_objective,
                )
                .unwrap_or(civic)
            });
            if let Some(civic) = pick {
                if self.journal().wants(crate::reasoning::Level::Decision) {
                    let step = if self.beeline_orders_by_value {
                        "most valuable"
                    } else {
                        "cheapest"
                    };
                    let why = match (forced_goal, &goal_pick) {
                        (Some(goal), Some(_)) => {
                            format!(
                                "the {step} step toward {}, which {} needs",
                                plain(goal),
                                objective.as_str()
                            )
                        }
                        _ => {
                            let runner_up = available
                                .iter()
                                .filter(|other| **other != civic)
                                .max_by(|a, b| {
                                    self.civic_value(g, pid, a, civic_objective)
                                        .partial_cmp(&self.civic_value(g, pid, b, civic_objective))
                                        .unwrap()
                                        .then_with(|| b.cmp(a))
                                })
                                .map(|other| {
                                    format!(
                                        "ahead of {} at {:.0}",
                                        plain(other),
                                        self.civic_value(g, pid, other, plan.strategy)
                                    )
                                })
                                .unwrap_or_else(|| "with nothing else on offer".to_string());
                            format!(
                                "worth {:.0} to the {} plan, {runner_up}",
                                self.civic_value(g, pid, &civic, plan.strategy),
                                plan.strategy.as_str()
                            )
                        }
                    };
                    think!(self.journal(), Research, Decision, "Adopting the {} civic", plain(&civic);
                           "{why}");
                }
                drop(memo);
                let _ = g.apply(
                    pid,
                    &Action::Civic {
                        civic: Name::new(&civic),
                    },
                );
            }
        }
    }

    pub(super) fn tech_leads_to(&self, g: &Game, candidate: &str, target: &str) -> bool {
        candidate == target
            || g.rules
                .tech_ancestors
                .get(target)
                .is_some_and(|ancestors| ancestors.contains(candidate))
    }

    pub(super) fn civic_leads_to(&self, g: &Game, candidate: &str, target: &str) -> bool {
        candidate == target
            || g.rules
                .civic_ancestors
                .get(target)
                .is_some_and(|ancestors| ancestors.contains(candidate))
    }

    /// The civic that unlocks the tier-2 government this lane would adopt, or
    /// `None` when the ladder has nothing to climb. See `government_ladder`.
    ///
    /// The pairs mirror `strategic_government`'s own priority lists, so the
    /// civic chased is the government that will actually be adopted when it
    /// lands rather than a second opinion that disagrees with the adopter.
    ///
    /// Three guards keep it a rung and not a beeline: rung one has to be owned
    /// first (every tier-2 chain runs through Political Philosophy anyway);
    /// once ANY tier-2 gate is owned the empire has left the four-slot
    /// governments, and a lateral move between two six-slot ones does not
    /// justify a second 340-to-440-cost detour; and the climb stops after the
    /// halfway turn, because the reason the shipped horizon exists at all is
    /// that a policy slot bought at turn 200 has almost nothing left to pay
    /// back.
    pub(super) fn government_ladder_goal(
        &self,
        g: &Game,
        pid: usize,
        objective: GrandStrategy,
    ) -> Option<&'static str> {
        if !self.government_ladder && !self.government_ladder_2 && !self.government_ladder_3 {
            return None;
        }
        if !g.players[pid]
            .civics
            .contains(&crate::name!("political_philosophy"))
        {
            return None;
        }
        if self.government_ladder_3 {
            return self.government_ladder_route(g, pid, objective);
        }
        if self.government_ladder_2 {
            return self.government_ladder_rung(g, pid, objective);
        }
        if g.turn as f64 > g.max_turns.max(1) as f64 * GOVERNMENT_LADDER_WINDOW {
            return None;
        }
        let ladder: [&str; 3] = match objective {
            GrandStrategy::Conquest => ["divine_right", "reformed_church", "exploration"],
            GrandStrategy::Religion => ["reformed_church", "divine_right", "exploration"],
            _ => ["exploration", "divine_right", "reformed_church"],
        };
        if ladder
            .iter()
            .any(|civic| g.players[pid].civics.contains(&Name::new(civic)))
        {
            return None;
        }
        ladder.into_iter().next()
    }

    /// `government-ladder-2`: the same climb, but it does not stop at tier two
    /// and it does not stop climbing while the empire is behind the field on
    /// policy slots.
    ///
    /// Version one asks a fixed question — "do I own Exploration, Divine Right
    /// or Reformed Church?" — and retires the moment any one of them lands.
    /// That is a rung, not a ladder: an empire that owns Exploration and plays
    /// a six-slot Merchant Republic against a field on ten never climbs again.
    /// The live King seat never even reached rung one, so the retirement clause
    /// was never the binding constraint; what the run showed is that the
    /// binding constraint is the WINDOW. Half the clock is turn 125, and the
    /// civics that actually gate the large governments sit past it.
    ///
    /// So version two asks the question capacity actually poses: is there a
    /// government whose gate civic I do not own that would carry more policy
    /// slots than the one I am playing, and is the cheapest step toward it
    /// still worth taking? It ranks by the gate civic's own cost, which rises
    /// with era, so the answer early is a tier-2 government and the answer
    /// late is the tier-4 one — the same order version one hard-coded, without
    /// the ceiling.
    ///
    /// The window still exists, because a policy slot bought at turn 240 pays
    /// nothing back. It is simply longer while the empire is measurably behind
    /// on slots: [`GOVERNMENT_LADDER_WINDOW`] normally, and
    /// [`GOVERNMENT_LADDER_BEHIND_WINDOW`] while some living major plays a
    /// government with more capacity than ours.
    pub(super) fn government_ladder_rung(
        &self,
        g: &Game,
        pid: usize,
        objective: GrandStrategy,
    ) -> Option<&'static str> {
        let capacity = |name: &str| {
            g.rules.governments.get(name).map_or(0, |spec| {
                spec.slots.military
                    + spec.slots.economic
                    + spec.slots.diplomatic
                    + spec.slots.wildcard
            })
        };
        let ours = g.players[pid].government.as_deref().map_or(0, capacity);
        let behind = g
            .players
            .iter()
            .filter(|rival| {
                rival.id != pid && rival.alive && !rival.is_minor && !rival.is_barbarian
            })
            .any(|rival| rival.government.as_deref().map_or(0, capacity) > ours);
        let window = if behind {
            GOVERNMENT_LADDER_BEHIND_WINDOW
        } else {
            GOVERNMENT_LADDER_WINDOW
        };
        if g.turn as f64 > g.max_turns.max(1) as f64 * window {
            return None;
        }
        let priorities = Self::government_priorities(objective, false);
        let culture_match = Self::culture_government_match(g, pid, objective);
        g.rules
            .governments
            .iter()
            .filter(|(name, _)| {
                self.government_capacity_fallback
                    || priorities.contains(&name.as_str())
                    || culture_match.as_deref() == Some(name.as_str())
            })
            .filter(|(name, _)| capacity(name.as_str()) > ours)
            .filter_map(|(_, spec)| spec.civic.as_ref())
            .filter(|civic| !g.players[pid].civics.contains(civic))
            .filter(|civic| g.rules.civics.get(civic.as_str()).is_some())
            // Cheapest gate first: civic cost rises with era, so this walks the
            // ladder a rung at a time instead of beelining the last government
            // in the game from the Classical era.
            .min_by(|a, b| {
                g.rules.civics[a.as_str()]
                    .cost
                    .partial_cmp(&g.rules.civics[b.as_str()].cost)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.as_str().cmp(b.as_str()))
            })
            .map(|civic| civic.as_str())
    }

    /// Whether this seat can still found a religion: none of its own yet, a
    /// Prophet already claimed or a slot still open once the Prophets other
    /// majors hold are counted, and the first half of the clock. See
    /// `enter_the_prophet_race_2`.
    /// `connect-the-luxury`: the cheapest technology that would let some
    /// improvement take an owned, unimproved luxury no unlocked improvement
    /// can — Irrigation for a Plantation luxury, Sailing for a sea one.
    /// `None` with the gene off, or when every owned luxury is connectable
    /// already. The builder side already prices a luxury connection; this is
    /// the research side it was waiting on.
    pub(super) fn unconnected_luxury_tech(&self, g: &Game, pid: usize) -> Option<&'static str> {
        if self.connect_the_luxury_2 {
            return self.useful_luxury_tech(g, pid);
        }
        if !self.connect_the_luxury {
            return None;
        }
        let player = &g.players[pid];
        let unlocked = |tech: &Option<Name>, civic: &Option<Name>| {
            tech.as_ref().is_none_or(|tech| player.techs.contains(tech))
                && civic
                    .as_ref()
                    .is_none_or(|civic| player.civics.contains(civic))
        };
        let mut best: Option<(f64, Name)> = None;
        for tile in g.map.tiles.values() {
            let owned = tile
                .owner_city
                .and_then(|city| g.cities.get(&city))
                .is_some_and(|city| city.owner == pid);
            if !owned || tile.improvement.is_some() {
                continue;
            }
            let Some(resource) = tile.resource else {
                continue;
            };
            if !g
                .rules
                .resources
                .get(&resource)
                .is_some_and(|spec| spec.class == "luxury")
            {
                continue;
            }
            let takers: Vec<_> = g
                .rules
                .improvements
                .values()
                .filter(|spec| spec.resources.contains(&resource))
                .collect();
            if takers.iter().any(|spec| unlocked(&spec.tech, &spec.civic)) {
                continue;
            }
            for spec in takers {
                let Some(tech) = spec.tech else {
                    continue;
                };
                if player.techs.contains(&tech)
                    || spec
                        .civic
                        .as_ref()
                        .is_some_and(|civic| !player.civics.contains(civic))
                {
                    continue;
                }
                let cost = g
                    .rules
                    .techs
                    .get(&tech)
                    .map_or(f64::INFINITY, |spec| spec.cost);
                if best.is_none_or(|(best_cost, _)| cost < best_cost) {
                    best = Some((cost, tech));
                }
            }
        }
        best.map(|(_, tech)| tech.as_str())
    }

    /// `boosted-bargain-first`: the cheapest available technology whose
    /// Eureka is in hand and whose remaining cost — the speed-scaled cost
    /// less the boost's share — is at most `BOOSTED_BARGAIN_TURNS` of the
    /// cities' science. `None` with the gene off or without such a bargain.
    /// The science read here is the cities' own; empire-wide bonuses only
    /// make the bargain cheaper than it reads.
    pub(super) fn boosted_bargain_tech(
        &self,
        g: &Game,
        pid: usize,
        available: &[Name],
    ) -> Option<&'static str> {
        if !self.boosted_bargain_first {
            return None;
        }
        let player = &g.players[pid];
        let science: f64 = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.city_yields(cid).science)
            .sum::<f64>()
            .max(1.0);
        available
            .iter()
            .copied()
            .filter(|tech| player.boosted_techs.contains(tech))
            .map(|tech| {
                let percent = g.rules.techs[tech.as_str()]
                    .boost
                    .as_ref()
                    .and_then(|boost| boost.percent)
                    .unwrap_or(40.0);
                let remaining = g.tech_cost(tech.as_str()) * (1.0 - percent / 100.0);
                (remaining, tech)
            })
            .filter(|(remaining, _)| *remaining <= science * BOOSTED_BARGAIN_TURNS)
            .min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)))
            .map(|(_, tech)| tech.as_str())
    }

    /// `boosted-bargain-first-2`: the best already-boosted available node
    /// that finishes within one turn and retains at least
    /// `BOOSTED_BARGAIN_V2_VALUE_FLOOR` of the ordinary fallback winner's
    /// strategic value. `advanced_research` asks only when no forced lane goal
    /// produced a pick, so this can break a close fallback decision but cannot
    /// interrupt a victory, war, defence, religion, luxury, or infrastructure
    /// beeline.
    pub(super) fn boosted_bargain_tech_2(
        &self,
        g: &Game,
        pid: usize,
        strategy: GrandStrategy,
        available: &[Name],
        ordinary: &Name,
    ) -> Option<Name> {
        if !self.boosted_bargain_first_2 {
            return None;
        }
        let player = &g.players[pid];
        let science = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.city_yields(cid).science)
            .sum::<f64>()
            .max(1.0);
        let value_floor =
            self.tech_value(g, pid, ordinary, strategy) * BOOSTED_BARGAIN_V2_VALUE_FLOOR;
        available
            .iter()
            .cloned()
            .filter(|tech| player.boosted_techs.contains(tech))
            .filter_map(|tech| {
                let remaining =
                    g.tech_cost(tech.as_str()) * (1.0 - Self::boost_frac(g, &tech, true));
                (remaining <= science * BOOSTED_BARGAIN_V2_TURNS
                    && self.tech_value(g, pid, &tech, strategy) + f64::EPSILON >= value_floor)
                    .then(|| (self.tech_value(g, pid, &tech, strategy), remaining, tech))
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.total_cmp(&left.1))
                    .then_with(|| right.2.cmp(&left.2))
            })
            .map(|(_, _, tech)| tech)
    }

    /// `boosted-bargain-first-3`: the highest-value already-boosted available
    /// technology that finishes within `BOOSTED_BARGAIN_V3_TURNS` of the
    /// cities' science, or `None` — with the gene off, without such a
    /// bargain, or under any of the vetoes below.
    ///
    /// Two callers. `advanced_research` asks first in v1's slot of the forced
    /// goal ladder with `lane_target` set to the lane goal the bargain would
    /// displace (`ordinary` unset): that is v1's source of research pace
    /// (+0.33 techs at standard t150 on the 2026-09-08 screen), bought here
    /// for exactly one turn of delay instead of v1's two. It asks again on
    /// the fallback path with `ordinary` set to the plain `tech_value` winner
    /// (`lane_target` unset), where the bargain must keep
    /// `BOOSTED_BARGAIN_V3_VALUE_FLOOR` of that winner's value — v2's rule.
    ///
    /// The vetoes are v2's discipline (+0.47 pp wins where v1 lost −0.80 pp)
    /// written as conditions instead of as a position: no interruption while
    /// the seat is at war with a living major, holds a war plan, plans
    /// `Recovery`, or names a threatened city; and none while `lane_target`
    /// is itself legal and within `BOOSTED_BARGAIN_V3_LANE_NEAR_TURNS` of
    /// done, because then the beeline is on its final step toward a
    /// victory-chain, Harbor or Great Person node and the turn is the lane's.
    /// Among bargains the pick is the highest `tech_value`, then the cheaper,
    /// then the earlier name — not v1's cheapest, which bought whatever was
    /// on sale.
    pub(super) fn boosted_bargain_tech_3(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        available: &[Name],
        lane_target: Option<&str>,
        ordinary: Option<&Name>,
    ) -> Option<Name> {
        if !self.boosted_bargain_first_3 {
            return None;
        }
        let player = &g.players[pid];
        let at_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && g.is_at_war(pid, other.id)
        });
        if at_war
            || self.war_plan.is_some()
            || plan.strategy == GrandStrategy::Recovery
            || plan.threatened_city.is_some()
        {
            return None;
        }
        let science = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.city_yields(cid).science)
            .sum::<f64>()
            .max(1.0);
        if let Some(target) = lane_target {
            let landing = available.iter().any(|tech| tech.as_str() == target)
                && self.beeline_step_cost(g, pid, target, true)
                    <= science * BOOSTED_BARGAIN_V3_LANE_NEAR_TURNS;
            if landing {
                return None;
            }
        }
        let value_floor = ordinary
            .map(|ordinary| {
                self.tech_value(g, pid, ordinary, plan.strategy) * BOOSTED_BARGAIN_V3_VALUE_FLOOR
            })
            .unwrap_or(f64::NEG_INFINITY);
        available
            .iter()
            .cloned()
            .filter(|tech| player.boosted_techs.contains(tech))
            .filter_map(|tech| {
                let remaining =
                    g.tech_cost(tech.as_str()) * (1.0 - Self::boost_frac(g, &tech, true));
                let value = self.tech_value(g, pid, &tech, plan.strategy);
                (remaining <= science * BOOSTED_BARGAIN_V3_TURNS
                    && value + f64::EPSILON >= value_floor)
                    .then_some((value, remaining, tech))
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.total_cmp(&left.1))
                    .then_with(|| right.2.cmp(&left.2))
            })
            .map(|(_, _, tech)| tech)
    }

    /// `beeline-orders-by-value`: the step a forced goal takes next among
    /// `steps`, its remaining prerequisites the era window offers. Every one
    /// of them is researched before the goal whatever the order, so the goal
    /// lands on the same turn either way; the order only decides which unlock
    /// arrives first. The stock picker takes the cheapest printed price with
    /// ties broken by name, which put Sailing ahead of Writing (both 50) and
    /// Military Tactics ahead of Education on a Science seat; this takes the
    /// best `tech_value` / `civic_value` — the same score the unforced argmax
    /// ranks by, boost in hand included — with ties to the earlier name.
    pub(super) fn beeline_step(
        &self,
        g: &Game,
        pid: usize,
        strategy: GrandStrategy,
        steps: &[Name],
        techs: bool,
    ) -> Option<Name> {
        let value = |node: &Name| {
            if techs {
                self.tech_value(g, pid, node.as_str(), strategy)
            } else {
                self.civic_value(g, pid, node.as_str(), strategy)
            }
        };
        steps
            .iter()
            .map(|node| (value(node), *node))
            .max_by(|a, b| a.0.total_cmp(&b.0).then_with(|| b.1.cmp(&a.1)))
            .map(|(_, node)| node)
    }

    pub(super) fn tech_value(
        &self,
        g: &Game,
        pid: usize,
        tech: &str,
        strategy: GrandStrategy,
    ) -> f64 {
        let spec = &g.rules.techs[tech];
        // The whole opinion about boosts: the credit for one in hand, the wait
        // for one nearly earned, and the credit for the boosts this node makes
        // chaseable. With the three boost genes off this is the flat 28 for a
        // boost in hand and nothing otherwise, exactly as before. See
        // `advanced/boost_research.rs`.
        let mut value = self.boost_research_value(g, pid, tech, true);
        for (name, unit) in &g.rules.units {
            if unit.tech.as_deref() == Some(tech)
                && unit
                    .unique_to
                    .as_ref()
                    .is_none_or(|c| c == &g.players[pid].civ)
            {
                let power = unit.strength.max(unit.ranged_attack_strength());
                value += if strategy == GrandStrategy::Conquest {
                    power * 3.2
                } else {
                    power * 1.1
                };
                // `unique-unit-preference`: the credit reads the rules' own
                // `unique_to`, which every unique unit carries. The civs.json
                // `unique_unit` field names one for 14 of 105 civilizations
                // (Gran Colombia's Llanero is not among them), so the credit
                // never reached most of the roster.
                let ours = if self.base.unique_unit_preference {
                    unit.unique_to.as_deref() == Some(g.players[pid].civ.as_str())
                } else {
                    g.rules.civs[&g.players[pid].civ].unique_unit.as_deref() == Some(name)
                };
                if !self.civ_blind && ours {
                    value += 55.0;
                }
            }
        }
        // A node that unlocks the successor of units already on the map buys
        // an upgrade for every one of them, not merely the option to train
        // something new. Counted over a whole game, the commonest reason an
        // upgrade was unavailable was a node its owner had simply never taken
        // — Archery, Iron Working, Machinery — while empires holding ten to
        // thirty technologies still fielded the Slingers and Warriors those
        // nodes would have retired.
        let stranded: f64 = g
            .units
            .values()
            .filter(|unit| unit.owner == pid)
            .filter_map(|unit| {
                let held = &g.rules.units[unit.kind];
                let successor = g.rules.units.get(held.upgrade_to.as_deref()?)?;
                (successor.tech.as_deref() == Some(tech)).then(|| {
                    (successor.strength.max(successor.ranged_attack_strength())
                        - held.strength.max(held.ranged_attack_strength()))
                    .max(0.0)
                })
            })
            .sum();
        // Capped so a large stale army nudges the order rather than dictating
        // it: at the ceiling this is worth about as much as an embarkation
        // prerequisite, which is the largest bonus already in this function.
        value += (stranded
            * if strategy == GrandStrategy::Conquest {
                2.4
            } else {
                1.4
            })
        .min(150.0);
        for building in g
            .rules
            .buildings
            .values()
            .filter(|b| b.tech.as_deref() == Some(tech))
        {
            value += self.yield_value(building.yields, strategy) * 14.0
                + building.housing * 12.0
                + building.amenity * 18.0;
        }
        for district in g
            .rules
            .districts
            .values()
            .filter(|d| d.tech.as_deref() == Some(tech))
        {
            value += self.yield_value(district.yields, strategy) * 18.0
                + district.defense * 1.5
                + district.amenity * 18.0;
        }
        for project in g
            .rules
            .projects
            .values()
            .filter(|p| p.tech.as_deref() == Some(tech))
        {
            value += if strategy == GrandStrategy::Science {
                if project.repeatable {
                    120.0
                } else {
                    260.0
                }
            } else if project.repeatable {
                25.0
            } else {
                65.0
            };
        }
        for improvement in g
            .rules
            .improvements
            .values()
            .filter(|i| i.tech.as_deref() == Some(tech))
        {
            value += self.yield_value(improvement.yields, strategy) * 10.0 + 18.0;
        }
        if strategy == GrandStrategy::Religion && tech == "astrology" {
            value += 95.0;
        }
        // See `advanced/early_archers.rs`: the node that unlocks the
        // shooter, and every node on the way, while the empire lacks it.
        // Zero with the gene off.
        value += self.early_archers_research_value(g, pid, tech);
        // `early-conquest-opening`: the node that upgrades the strike force's
        // Slinger into a real shooter, while the reservation is open. Zero
        // while off. See `advanced/early_conquest.rs`.
        value += self.conquest_research_value(g, pid, tech);
        if let Some(goal) = BasicAi::water_research_goal(g, pid) {
            if self.tech_leads_to(g, tech, goal) {
                // Embarkation and ocean access change which parts of the map
                // are strategically reachable, so their prerequisites must
                // compete with ordinary yield unlocks rather than wait for a
                // naval unit to happen to win a generic score comparison.
                value += match goal {
                    "sailing" => 190.0,
                    "shipbuilding" => 230.0,
                    "celestial_navigation" => 150.0,
                    "cartography" => 210.0,
                    "square_rigging" | "steam_power" | "refining" | "electricity"
                    | "combined_arms" | "lasers" | "telecommunications" => 185.0,
                    _ => 0.0,
                };
            }
        }
        if (strategy == GrandStrategy::Science && self.phase_specialization_active(g))
            || self.science_drive_active()
        {
            // `science_victory_drive`: a driving seat keys the beeline on
            // the next UNKNOWN chain tech, whatever its plan, so research
            // holds the chain while a project is being built. Stock research
            // uses the same next-unknown chain target as forced-goal routing;
            // while the Earth Satellite is building, nothing should send the
            // seat back toward the already-known Rocketry node.
            let milestone = if self.science_drive_active() {
                Self::science_drive_milestone(g, pid)
            } else {
                Self::science_victory_tech_goal(g, pid, GrandStrategy::Science)
            };
            if milestone.is_some_and(|milestone| self.tech_leads_to(g, tech, milestone)) {
                value += if self.raced_target() == Some(VictoryTarget::Science) {
                    900.0
                } else {
                    260.0
                };
            }
        }
        value += self.science_drive_tech_bonus(g, pid, tech);
        value += self.portfolio_tech_bonus(g, pid, tech);
        // A device is only a victory beeline when somebody we can actually
        // reach has infrastructure worth breaking.  The old path fired for
        // every Industrial-era Conquest plan, even across a quiet border or
        // after the only useful target had already been neutralised.  Price
        // the prerequisites aggressively once the shared nuclear-goal reader
        // names a reachable victory threat (or a Domination objective), then
        // stand down at its bounded stockpile.  Gated on victory planning so
        // the frozen advanced_v1 anchor's research remains untouched.
        let nuclear_path_candidate = self.tech_leads_to(g, tech, "nuclear_fission")
            || self.tech_leads_to(g, tech, "nuclear_fusion");
        if self.victory_planning
            && matches!(strategy, GrandStrategy::Conquest | GrandStrategy::Recovery)
            && g.world_era >= 4
            && nuclear_path_candidate
        {
            if let Some(goal) = self.nuclear_war_goal(g, pid, strategy, None) {
                let completed = &g.players[pid].science_projects;
                let milestone = if !completed.contains("manhattan_project") {
                    Some("nuclear_fission")
                } else if goal.needs_thermonuclear
                    && g.players[pid]
                        .counters
                        .get("project_effect:thermonuclear_devices")
                        .copied()
                        .unwrap_or(0)
                        <= 0
                    && !completed.contains("operation_ivy")
                {
                    Some("nuclear_fusion")
                } else {
                    None
                };
                if milestone.is_some_and(|milestone| self.tech_leads_to(g, tech, milestone)) {
                    let urgency = f64::from(goal.pressure.progress.max(0));
                    value += 340.0 + urgency * 4.0 + goal.priority.min(1_000.0) * 0.25;
                    if self.victory_target == Some(VictoryTarget::Domination) {
                        value += 300.0;
                    }
                }
            }
        }
        // One-step lookahead prevents cheap prerequisites from being ignored.
        // Only units this civilization can field count: another
        // civilization's unique unit is no unlock here, and with the whole
        // shipped roster Military Tactics alone carries the Impi, the
        // Berserker and the Khevsureti — +24 on Mathematics for everyone.
        let civ = g.players[pid].civ.as_str();
        for (future, child) in &g.rules.techs {
            if child.requires.iter().any(|r| r == tech) {
                let unlocks = g
                    .rules
                    .units
                    .values()
                    .filter(|u| {
                        u.tech.as_deref() == Some(future)
                            && u.unique_to.as_deref().is_none_or(|owner| owner == civ)
                    })
                    .count()
                    + g.rules
                        .buildings
                        .values()
                        .filter(|b| b.tech.as_deref() == Some(future))
                        .count()
                    + g.rules
                        .districts
                        .values()
                        .filter(|d| d.tech.as_deref() == Some(future))
                        .count()
                    + g.rules
                        .projects
                        .values()
                        .filter(|p| p.tech.as_deref() == Some(future))
                        .count();
                value += unlocks as f64 * 8.0;
            }
        }
        // Discount by opportunity cost so a flashy late-era unlock does not
        // stall several cheaper advances. Square root still lets a genuinely
        // transformative breakthrough win the comparison.
        //
        // A boost in hand is a discount on THIS divisor, not a term above it:
        // the node costs `1 - frac` of its printed price, so the score it buys
        // is this one times `1 / (1 - frac).sqrt()`. One with
        // `boost_first_research` off. See `advanced/boost_research.rs`.
        (value + 35.0) / spec.cost.max(10.0).sqrt() * self.boost_in_hand_scale(g, pid, tech, true)
    }

    pub(super) fn civic_value(
        &self,
        g: &Game,
        pid: usize,
        civic: &str,
        strategy: GrandStrategy,
    ) -> f64 {
        let spec = &g.rules.civics[civic];
        // The civic half of the same term; see `advanced/boost_research.rs`.
        let mut value = self.boost_research_value(g, pid, civic, false);
        for building in g
            .rules
            .buildings
            .values()
            .filter(|b| b.civic.as_deref() == Some(civic))
        {
            value += self.yield_value(building.yields, strategy) * 15.0
                + building.housing * 12.0
                + building.amenity * 18.0;
        }
        for district in g
            .rules
            .districts
            .values()
            .filter(|d| d.civic.as_deref() == Some(civic))
        {
            value += self.yield_value(district.yields, strategy) * 18.0 + district.amenity * 18.0;
        }
        value += g
            .rules
            .governments
            .values()
            .filter(|gov| gov.civic.as_deref() == Some(civic))
            .map(|gov| {
                let slots = gov.slots.military
                    + gov.slots.economic
                    + gov.slots.diplomatic
                    + gov.slots.wildcard;
                45.0 + slots as f64 * 18.0
            })
            .sum::<f64>();
        value += g
            .rules
            .policies
            .values()
            .filter(|p| p.civic.as_deref() == Some(civic))
            .count() as f64
            * 13.0;
        if strategy == GrandStrategy::Expansion && matches!(civic, "early_empire" | "foreign_trade")
        {
            value += 45.0;
        }
        if strategy == GrandStrategy::Culture && civic == "drama_poetry" {
            value += 60.0;
        }
        if strategy == GrandStrategy::Diplomacy
            && matches!(civic, "political_philosophy" | "civil_service" | "guilds")
        {
            value += 60.0;
        }
        if strategy == GrandStrategy::Religion && civic == "theology" {
            value += 120.0;
        }
        // `spies-steal-the-lead`: Spy capacity opens at Diplomatic Service
        // (through Guilds). Live Deity Korea carriers took it at t108-151 and
        // a stealing Spy is worth ~+50 Science a turn, so a Science aim that
        // steals wants the civic sooner. Zero with the gene off.
        if self.spies_steal_the_lead
            && (strategy == GrandStrategy::Science
                || self.victory_target == Some(VictoryTarget::Science))
        {
            value += match civic {
                "diplomatic_service" => STEAL_CIVIC_VALUE,
                "guilds" => STEAL_CIVIC_VALUE / 2.0,
                _ => 0.0,
            };
        }
        // `spy-civics-come-first`: Spy capacity past Diplomatic Service
        // (which `spies-steal-the-lead` already brings forward). Zero with
        // the gene off. See `advanced/spy_civics_come_first.rs`.
        value += self.spy_civic_value(strategy, civic);
        value += match civic {
            "foreign_trade" | "craftsmanship" => 25.0,
            "early_empire" | "state_workforce" => 38.0,
            "political_philosophy" => 70.0,
            // Culture infrastructure is a prerequisite for every strategy,
            // not only a culture-victory plan.
            "drama_poetry" => 55.0,
            _ => 0.0,
        };
        // The civic half of the same discount; see `advanced/boost_research.rs`.
        (value + 32.0) / spec.cost.max(10.0).sqrt() * self.boost_in_hand_scale(g, pid, civic, false)
    }
}

#[cfg(test)]
mod steal_civic_tests;
