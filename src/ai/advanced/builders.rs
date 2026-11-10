//! Builders: what `AdvancedAi`'s Builders improve, and how they get there.
//!
//! `advanced_builder_step` moves each Builder: it ranks the jobs
//! (`builder_jobs_ranked`, `worthwhile_improvements`), walks to the first
//! reachable one, and retreats from a barbarian capture. Improvements are
//! priced by `improvement_value` (tree yields, appeal, and the production
//! foundation a city is short of). `advanced_special_improver_step` places
//! the special improvements other units build, priced by
//! `special_improvement_value`.
//!
//! Moved verbatim out of the one `impl AdvancedAi` block in `advanced.rs`
//! (2026-11-04). Methods the rest of the controller calls are `pub(super)`;
//! the code is otherwise unchanged.

use super::*;

mod culture_ski_resorts;
mod diagnostics;

impl AdvancedAi {
    pub(super) fn improvement_value(
        &self,
        g: &Game,
        pos: Pos,
        improvement: &str,
        strategy: GrandStrategy,
    ) -> f64 {
        let appeal = g.tile_appeal(pos).max(0) as f64;
        self.improvement_value_with_appeal(g, pos, improvement, strategy, appeal)
    }

    /// See `improvement_upgrades_count`: what `pid`'s researched techs and
    /// civics add to `improvement`, the same tree effects
    /// `Game::player_tile_yields` pays on a built one.
    pub(super) fn improvement_tree_yields(g: &Game, pid: usize, improvement: &str) -> Yields {
        let tree = |effect: &str| g.tree_effect(pid, effect);
        let mut yields = Yields::default();
        match improvement {
            "mine" => yields.production += tree("mine_production"),
            "quarry" => yields.production += tree("quarry_production"),
            "lumber_mill" => yields.production += tree("lumber_mill_production"),
            "pasture" => {
                yields.food += tree("pasture_food");
                yields.production += tree("pasture_production");
            }
            "plantation" => {
                yields.food += tree("plantation_food");
                yields.gold += tree("plantation_gold");
            }
            "camp" => {
                yields.food += tree("camp_food");
                yields.production += tree("camp_production");
                yields.gold += tree("camp_gold");
            }
            "fishing_boats" => {
                yields.food += tree("fishing_boats_food");
                yields.production += tree("fishing_boats_production");
                yields.gold += tree("fishing_boats_gold");
            }
            _ => {}
        }
        yields
    }

    pub(super) fn improvement_value_with_appeal(
        &self,
        g: &Game,
        pos: Pos,
        improvement: &str,
        strategy: GrandStrategy,
        appeal: f64,
    ) -> f64 {
        let tile = &g.map.tiles[&pos];
        let spec = &g.rules.improvements[improvement];
        let mut yields = spec.yields;
        yields.gold += spec.effects.get("appeal_gold").copied().unwrap_or(0.0) * appeal;
        if self.improvement_upgrades_count {
            if let Some(owner) = tile
                .owner_city
                .and_then(|city| g.cities.get(&city))
                .map(|city| city.owner)
            {
                yields.add(Self::improvement_tree_yields(g, owner, improvement));
            }
        }
        // `korea-mines-beside-the-seowon`: Three Kingdoms' +1 Science per
        // Mine and +1 Food per Farm beside a Seowon. Zero with the gene off.
        // See `advanced/korea_mines_beside_the_seowon.rs`.
        yields.add(self.three_kingdoms_improvement_yields(g, pos, improvement));
        let mut value = self.yield_value(yields, strategy);
        if strategy == GrandStrategy::Culture {
            // Tourism is cumulative: delaying a resort or national park by
            // dozens of turns loses visitors that cannot be recovered by an
            // equivalent late-game yield. Treat it as a durable strategic
            // yield so builders seek tourist sites as soon as they unlock.
            let tourism = spec.effects.get("tourism").copied().unwrap_or(0.0)
                + spec.effects.get("appeal_tourism").copied().unwrap_or(0.0) * appeal;
            value += tourism * 35.0;
        }
        // `sweden-builds-open-air-museums`: the museum's Culture and Tourism,
        // which its spec does not carry. Zero with the gene off. See
        // `advanced/sweden_builds_open_air_museums.rs`.
        value += self.open_air_museum_value(g, pos, improvement, strategy);
        if let Some(resource) = &tile.resource {
            // Only the improvement that actually works the resource connects
            // it. A Farm on an Iron hill was scoring the same premium as the
            // Mine, so the deposit read as already handled and no builder ever
            // came back for it.
            let worked = spec.resources.iter().any(|entry| entry == resource);
            value += match g.rules.resources[resource].class.as_str() {
                "luxury" => 14.0 * worked as i32 as f64,
                // A strategic deposit is not a yield: it is the empire's only
                // supply of the material every modern unit costs to build and
                // to upgrade into, and the stockpile it feeds is capped at 50,
                // so a second source still earns its Builder charge. Opening
                // one outranks any ordinary tile improvement in the game.
                "strategic" if worked => 30.0,
                "strategic" => 0.0,
                _ => 4.0,
            };
        }
        // `eureka_chasing_builder`: the boost this improvement on this tile
        // earns. See `advanced/deity_habits.rs`.
        // `quest_boost`: the Envoy a city-state pays for the Eureka or
        // Inspiration this improvement completes — beside the research the
        // line above prices, and independent of it. See
        // `advanced/city_state_quests.rs`.
        value
            + self.conversion_land_penalty(g, pos, improvement)
            + self.builder_boost_premium(g, pos, improvement, value)
            // `boost-planner`: the deadlined side objective this Builder job
            // satisfies, as a share of the job's own value. Zero with the
            // gene off. See `advanced/boost_planner.rs`.
            + self.boost_planner_builder_premium(g, pos, improvement, value)
            + self.quest_boost_builder_premium(g, pos, improvement, strategy)
    }

    /// How far this own city remains from the output that lets it construct
    /// the infrastructure every victory lane depends on. The direct launcher
    /// deliberately names a victory target (Science by default), so this is
    /// a target contract rather than another default-on adaptive gene.
    pub(super) fn city_production_foundation_shortfall(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
    ) -> f64 {
        if self.active_victory_target(g).is_none()
            || !g.cities.get(&cid).is_some_and(|city| city.owner == pid)
        {
            return 0.0;
        }
        ((PRODUCTION_CITY_FOUNDATION_FLOOR - g.city_yields(cid).production.max(0.0))
            / PRODUCTION_CITY_FOUNDATION_FLOOR)
            .clamp(0.0, 1.0)
    }

    /// The extra value of an improvement that raises production in a city
    /// still below its foundation. We price the improvement's *net* printed
    /// production over the standing improvement, so replacing a Mine with a
    /// Farm never earns a production rescue bonus. Ruleset and tree bonuses
    /// only make this conservative base gain larger in play.
    pub(super) fn production_foundation_improvement_bonus(
        &self,
        g: &Game,
        pos: Pos,
        improvement: &str,
        strategy: GrandStrategy,
        city_shortfall: f64,
    ) -> f64 {
        if city_shortfall <= 0.0 {
            return 0.0;
        }
        let standing_production = g.map.tiles[&pos]
            .improvement
            .as_ref()
            .map(|standing| g.rules.improvements[standing].yields.production)
            .unwrap_or(0.0);
        let added_production =
            (g.rules.improvements[improvement].yields.production - standing_production).max(0.0);
        self.yield_value(
            Yields {
                production: added_production,
                ..Yields::default()
            },
            strategy,
        ) * city_shortfall
            * PRODUCTION_FOUNDATION_IMPROVEMENT_MULTIPLIER
    }

    /// The Builder's normal improvement score, plus a bounded local
    /// production-foundation premium for a named victory lane. Keeping this
    /// wrapper at each routing site means a Builder standing in a weak city,
    /// the empire-wide target sweep, and the reachable-job fallback all make
    /// the same decision.
    pub(super) fn production_foundation_improvement_value(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
        improvement: &str,
        strategy: GrandStrategy,
        city_shortfall: f64,
    ) -> f64 {
        self.marginal_improvement_value(g, pid, pos, improvement, strategy)
            + self.production_foundation_improvement_bonus(
                g,
                pos,
                improvement,
                strategy,
                city_shortfall,
            )
    }

    /// The few military unique improvements are not Builder choices, so their
    /// most important effects are not ordinary tile yields. A Feitoria pays
    /// every foreign route, while a Roman Fort or Maori Pa pays when it makes
    /// a pressured frontier harder to cross. Price those effects directly so
    /// a special charge reaches the job its civilization is meant to have.
    fn special_improvement_value(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
        improvement: &str,
        strategy: GrandStrategy,
    ) -> f64 {
        let spec = &g.rules.improvements[improvement];
        let mut value = self.improvement_value(g, pos, improvement, strategy);
        value += spec
            .effects
            .get("foreign_route_gold")
            .copied()
            .unwrap_or(0.0)
            * 6.0;
        value += spec
            .effects
            .get("foreign_route_production")
            .copied()
            .unwrap_or(0.0)
            * 9.0;

        let defensive_effect = spec.effects.get("defense").copied().unwrap_or(0.0) * 3.0
            + spec
                .effects
                .get("grant_fortification")
                .copied()
                .unwrap_or(0.0)
                * 3.0
            + spec
                .effects
                .get("hostile_movement_penalty")
                .copied()
                .unwrap_or(0.0)
                * 4.0
            + spec
                .effects
                .get("maori_heal_after_action")
                .copied()
                .unwrap_or(0.0)
                * 6.0;
        if defensive_effect <= 0.0 {
            return value;
        }
        let frontier = g.nbrs(pos).into_iter().any(|neighbor| {
            g.map
                .get(neighbor)
                .and_then(|tile| tile.owner_city)
                .and_then(|city| g.cities.get(&city))
                .is_some_and(|city| city.owner != pid)
        });
        let hostile_nearby = g.wdisk(pos, 4).into_iter().any(|neighbor| {
            g.unit_ids_at(neighbor).iter().any(|uid| {
                let unit = &g.units[uid];
                unit.owner != pid && g.is_at_war(pid, unit.owner)
            })
        });
        let at_war = g
            .players
            .iter()
            .any(|player| player.id != pid && player.alive && g.is_at_war(pid, player.id));
        let pressure = if hostile_nearby {
            3.0
        } else if at_war && frontier {
            2.0
        } else if frontier {
            1.25
        } else {
            0.5
        };
        value += defensive_effect * pressure;
        if strategy == GrandStrategy::Conquest && (frontier || hostile_nearby) {
            value += defensive_effect;
        }
        value
    }

    fn special_improvement_choice(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        pos: Pos,
        strategy: GrandStrategy,
    ) -> Option<(f64, Name)> {
        BasicAi::special_unit_improvements(g, pid, uid, pos)
            .into_iter()
            .map(|improvement| {
                (
                    self.special_improvement_value(g, pid, pos, &improvement, strategy),
                    improvement,
                )
            })
            .max_by(|(left_value, left), (right_value, right)| {
                left_value
                    .total_cmp(right_value)
                    .then_with(|| right.cmp(left))
            })
    }

    /// Route a military unique with a build charge through the same legal
    /// improvement surface as the rules engine. Unlike an ordinary Builder,
    /// this may deliberately leave our borders for an open foreign coast.
    /// Returning `None` preserves the unit's ordinary military turn whenever
    /// no legal strategic site exists.
    pub(crate) fn advanced_special_improver_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        strategy: GrandStrategy,
    ) -> Option<bool> {
        let current = g.units.get(&uid)?.pos;
        if let Some((_, improvement)) =
            self.special_improvement_choice(g, pid, uid, current, strategy)
        {
            return g
                .apply(
                    pid,
                    &Action::Improve {
                        unit: uid,
                        improvement,
                    },
                )
                .ok()
                .map(|_| true);
        }

        let candidates = {
            let _memo = g.query_memo();
            g.map
                .tiles
                .keys()
                .copied()
                .filter_map(|position| {
                    if position == current {
                        return None;
                    }
                    self.special_improvement_choice(g, pid, uid, position, strategy)
                        .map(|(value, _)| {
                            (value - g.wdist(current, position) as f64 * 0.7, position)
                        })
                })
                .collect::<Vec<_>>()
        };
        let preferred = candidates
            .iter()
            .max_by(|(left_value, left), (right_value, right)| {
                left_value
                    .total_cmp(right_value)
                    .then_with(|| right.cmp(left))
            })
            .map(|(_, position)| *position);
        let fallback: HashSet<Pos> = candidates.iter().map(|(_, position)| *position).collect();
        preferred
            .and_then(|position| g.route_step(uid, position, 0))
            .or_else(|| g.route_step_to_any(uid, &fallback))
            .and_then(|to| {
                g.apply(pid, &Action::Move { unit: uid, to })
                    .ok()
                    .map(|_| true)
            })
    }

    /// Return only improvements that genuinely upgrade the tile. Comparing
    /// candidates in isolation made late-game builders oscillate between a
    /// high-value resort and a lower-value farm on successive turns.
    pub(super) fn worthwhile_improvements(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
        strategy: GrandStrategy,
    ) -> Vec<Name> {
        let appeal = g.tile_appeal(pos).max(0) as f64;
        // `first-luxury-first`: zero for every tile with the gene off, and
        // zero for any improvement that does not open the empire's first copy
        // of a luxury, so the ranking below is the shipped one until it fires.
        // Priced on BOTH sides — a plantation already standing here is not an
        // upgrade to itself — see `advanced/first_luxury.rs`.
        let current_value = g.map.tiles[&pos]
            .improvement
            .as_deref()
            .map(|improvement| {
                self.improvement_value_with_appeal(g, pos, improvement, strategy, appeal)
                    + self.first_luxury_premium(g, pid, pos, improvement)
                    + self.growth_farm_premium(g, pid, pos, improvement)
                    // `growth-prices-the-farm-2`: zero with the gene off.
                    + self.growing_city_food_premium(g, pid, pos, improvement, strategy)
                    // `builders-improve-the-worked-for-production`: zero with
                    // the gene off. See `advanced/worked_production.rs`.
                    + self.worked_production_premium(g, pid, pos, improvement, strategy)
            })
            .unwrap_or(0.0);
        // Score each candidate once and sort the scores. The comparator used
        // to re-derive both sides of every comparison, so ranking eight
        // improvements valued a tile's appeal and resource close to sixty
        // times instead of eight. Same order, same ties.
        let mut choices: Vec<(f64, Name)> = g
            .valid_improvements(pid, pos)
            .into_iter()
            .filter(|improvement| g.rules.improvements[improvement].builder_buildable)
            .map(|improvement| {
                let value =
                    self.improvement_value_with_appeal(g, pos, &improvement, strategy, appeal)
                        + self.first_luxury_premium(g, pid, pos, &improvement)
                        + self.growth_farm_premium(g, pid, pos, &improvement)
                        + self.growing_city_food_premium(g, pid, pos, &improvement, strategy)
                        + self.worked_production_premium(g, pid, pos, &improvement, strategy);
                (value, improvement)
            })
            .filter(|(value, _)| *value > current_value + 0.5)
            .collect();
        choices.sort_by(|(a_value, a), (b_value, b)| {
            b_value.partial_cmp(a_value).unwrap().then(a.cmp(b))
        });
        choices
            .into_iter()
            .map(|(_, improvement)| improvement)
            .collect()
    }

    /// If a Builder is already in a visible Barbarian capture envelope, use
    /// its turn to reach the safest legal neighbouring tile before considering
    /// an improvement. A friendly city is an absolute refuge and breaks ties;
    /// otherwise return toward the nearest own city. `Some(false)` is
    /// intentional: when every neighbour is still capturable, the Builder
    /// must not fall through and spend a charge on the doomed tile.
    fn builder_retreat_from_barbarian_capture(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.builder_barbarian_safety {
            return None;
        }
        let current = g.units[&uid].pos;
        let visible = self.battlefront_visibility(g, pid);
        let threats = self.visible_barbarian_capture_threats(g, pid, &visible);
        let current_risk = self
            .builder_barbarian_capture_risk_with_threats(g, pid, uid, current, &visible, &threats);
        if current_risk <= BUILDER_BARBARIAN_CAPTURE_RISK_LIMIT {
            return None;
        }
        self.builder_targets.remove(&uid);
        let retreat = g
            .nbrs(current)
            .into_iter()
            .filter(|position| g.can_move(uid, *position))
            .filter_map(|position| {
                let risk = self.builder_barbarian_capture_risk_with_threats(
                    g, pid, uid, position, &visible, &threats,
                );
                (risk <= BUILDER_BARBARIAN_CAPTURE_RISK_LIMIT).then(|| {
                    let in_city = g
                        .city_at(position)
                        .is_some_and(|city| g.cities[&city].owner == pid);
                    let city_distance = g
                        .player_city_ids(pid)
                        .into_iter()
                        .map(|city| g.wdist(position, g.cities[&city].pos))
                        .min()
                        .unwrap_or(i32::MAX);
                    (risk, in_city, city_distance, position)
                })
            })
            .min_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.cmp(&left.1))
                    .then(left.2.cmp(&right.2))
                    .then(left.3.cmp(&right.3))
            });
        let Some((risk, _in_city, _city_distance, position)) = retreat else {
            think!(self.journal(), Expansion, Detail, "Builder holds in Barbarian capture reach";
                   "risk {current_risk:.0}; every legal neighbour remains above the \
                    safety limit of {BUILDER_BARBARIAN_CAPTURE_RISK_LIMIT:.0}"; current);
            return Some(false);
        };
        think!(self.journal(), Expansion, Detail, "Builder retreats from Barbarian capture reach";
               "risk {current_risk:.0} here, {risk:.0} at the safe step; a Builder \
                cannot improve a tile after the Barbarian takes it"; position);
        Some(self.base.path_move(g, pid, uid, position))
    }

    /// Take a Builder's next route step only if it is outside a direct
    /// Barbarian capture envelope. If the preferred route enters danger, a
    /// safe sidestep that still improves the route is allowed; otherwise the
    /// Builder holds on its already-safe tile until the visible raider moves.
    pub(super) fn builder_step_toward_barbarian_safe(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> bool {
        if !self.builder_barbarian_safety {
            return self.base.step_toward(g, pid, uid, target);
        }
        let Some(next) = g
            .route_step(uid, target, 0)
            .filter(|position| g.can_move(uid, *position))
        else {
            return self.base.step_toward(g, pid, uid, target);
        };
        let visible = self.battlefront_visibility(g, pid);
        let threats = self.visible_barbarian_capture_threats(g, pid, &visible);
        if self.builder_barbarian_capture_risk_with_threats(g, pid, uid, next, &visible, &threats)
            <= BUILDER_BARBARIAN_CAPTURE_RISK_LIMIT
        {
            return self.base.step_toward(g, pid, uid, target);
        }
        let current = g.units[&uid].pos;
        let current_distance = g.wdist(current, target);
        let sidestep = g
            .nbrs(current)
            .into_iter()
            .filter(|position| *position != next && g.can_move(uid, *position))
            .filter_map(|position| {
                let risk = self.builder_barbarian_capture_risk_with_threats(
                    g, pid, uid, position, &visible, &threats,
                );
                (risk <= BUILDER_BARBARIAN_CAPTURE_RISK_LIMIT).then(|| {
                    let progress = current_distance - g.wdist(position, target);
                    (risk, progress, position)
                })
            })
            // A lateral detour that is no closer to the job just makes a
            // Builder wander while the responder clears the raider. Keep the
            // assignment and wait instead; next turn can take the ordinary
            // route immediately if the threat has gone.
            .filter(|(_, progress, _)| *progress > 0)
            .min_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.cmp(&left.1))
                    .then(left.2.cmp(&right.2))
            });
        sidestep.is_some_and(|(_, _, position)| self.base.path_move(g, pid, uid, position))
    }

    pub(super) fn advanced_builder_step_unfiltered(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        strategy: GrandStrategy,
    ) -> bool {
        if let Some(acted) = self.builder_support_step(g, pid, uid) {
            return acted;
        }
        let current = g.units[&uid].pos;
        if let Some(retreated) = self.builder_retreat_from_barbarian_capture(g, pid, uid) {
            return retreated;
        }
        // The native opt-in and the live capture lessons share the exact
        // reach response: a builder inside it leaves before it takes a job.
        if self.builder_reach_safety_on() {
            if let Some(acted) = self.civilian_flee_step(g, pid, uid) {
                return acted;
            }
        }
        let project = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|city| {
                g.project_contribution_target(pid, city)
                    .map(|position| (g.wdist(current, position), position, city))
            })
            .min();
        if let Some((_, position, city)) = project {
            self.builder_targets.remove(&uid);
            if current == position && g.can_contribute_project(pid, uid, city) {
                return g
                    .apply(pid, &Action::ContributeProject { unit: uid, city })
                    .is_ok();
            }
            let stepped = if self.builder_reach_safety_on() {
                self.builder_step_out_of_reach(g, pid, uid, position)
            } else {
                self.builder_step_toward_barbarian_safe(g, pid, uid, position)
            };
            if stepped {
                return true;
            }
        }
        // `chop-for-expansion`: a Settler under production is a deadline, and
        // the sweep below cannot see a chop — `worthwhile_improvements` ranks
        // improvements, and a builder operation is not one. Placed after the
        // project errand, which is an empire decision already taken, and ahead
        // of repair and improvement, which have no deadline. `None` with the
        // gene off, before anything is read.
        if let Some(acted) = self.conversion_land_builder(g, pid, uid) {
            return acted;
        }
        if let Some(acted) = self.chop_for_expansion_step(g, pid, uid, strategy) {
            return acted;
        }
        // `chop-feeds-the-long-build`: the same errand for a district, a
        // wonder or a space-race project. After the Settler, which is the
        // shorter deadline. `None` with the gene off, before anything is read.
        if let Some(acted) = self.chop_feeds_the_long_build_step(g, pid, uid, strategy) {
            return acted;
        }
        if let Some(acted) = self.air_resource_builder_step(g, pid, uid) {
            return acted;
        }
        let repairable = g.map.get(current).is_some_and(|tile| {
            tile.pillaged
                && tile.improvement.is_some()
                && tile
                    .owner_city
                    .and_then(|city| g.cities.get(&city))
                    .is_some_and(|city| city.owner == pid)
        });
        if repairable {
            self.builder_targets.remove(&uid);
            return g
                .apply(pid, &Action::RepairImprovement { unit: uid })
                .is_ok();
        }
        let mut here = self.worthwhile_improvements(g, pid, current, strategy);
        let here_shortfall = g
            .map
            .get(current)
            .and_then(|tile| tile.owner_city)
            .map(|cid| self.city_production_foundation_shortfall(g, pid, cid))
            .unwrap_or(0.0);
        if here_shortfall > 0.0 {
            here.sort_by(|left, right| {
                self.production_foundation_improvement_value(
                    g,
                    pid,
                    current,
                    right,
                    strategy,
                    here_shortfall,
                )
                .total_cmp(&self.production_foundation_improvement_value(
                    g,
                    pid,
                    current,
                    left,
                    strategy,
                    here_shortfall,
                ))
                .then_with(|| left.cmp(right))
            });
        }
        if !here.is_empty() {
            if let Some(acted) = self.more_useful_builder_job(g, pid, uid, strategy) {
                return acted;
            }
            self.builder_targets.remove(&uid);
            // `order_retry`: `worthwhile_improvements` is already ranked and
            // only its head was ever attempted, so a tile that refuses the
            // best improvement spent the Builder's whole turn. With the gene
            // off this tries the head alone, exactly as before.
            for improvement in here.iter().take(1 + self.order_retry_budget()) {
                think!(self.journal(), Expansion, Detail,
                       "Building a {} at {current:?}", plain(improvement);
                       "worth {:.1} to the {} plan, best of {} that fit this tile",
                       self.production_foundation_improvement_value(
                           g,
                           pid,
                           current,
                           improvement,
                           strategy,
                           here_shortfall,
                       ),
                       strategy.as_str(), here.len(); current);
                if g.apply(
                    pid,
                    &Action::Improve {
                        unit: uid,
                        improvement: Name::new(improvement),
                    },
                )
                .is_ok()
                {
                    // ⚠ Journal AFTER the engine answers. `first-luxury-first`
                    // says nothing while it is off.
                    self.note_first_luxury_opened(g, pid, current, improvement);
                    return true;
                }
            }
            return false;
        }
        let mut reserved: HashSet<Pos> = self
            .builder_targets
            .iter()
            .filter(|(other, _)| **other != uid && g.units.contains_key(other))
            .map(|(_, pos)| *pos)
            .collect();
        // `commitment_patience`: a tile this Builder gave up on stays out of
        // its own sweep until the parking expires.
        if let Some((tile, until)) = self.builder_avoid.get(&uid) {
            if g.turn < *until {
                reserved.insert(*tile);
            }
        }
        // Reading every tile the empire owns: one memo scope so the
        // empire-wide questions each tile asks are answered once for the whole
        // sweep rather than once per tile. The borrow checker rejects the
        // guard the moment anything in here starts mutating the game.
        if self.base.builder_tries_the_next_tile {
            return self.builder_step_to_the_first_reachable_job(g, pid, uid, strategy, &reserved);
        }

        // ⚠ The `builder_tries_the_next_tile` branch above performs the same
        // reach-checked route step itself. Here the normal sweep also filters
        // a job tile a raider could stand on next turn before it is assigned.
        let reach = self
            .builder_reach_safety_on()
            .then(|| self.barbarian_reach(g, pid, current, civilian_safety::REACH_SCAN_RADIUS));
        let job_out_of_reach = |pos: Pos| {
            reach
                .as_ref()
                .is_none_or(|reach| self.builder_job_out_of_reach(g, pid, uid, pos, reach))
        };
        // `builder-prices-travel-in-turns`: the untreated `0.7` a hex unless
        // the gene is on and this Builder has a charge to spare. See
        // `advanced/builder_travel_turns.rs`.
        let travel = self.builder_travel(g, uid);
        let mut untreated_pick: Option<(f64, Pos)> = None;
        let best = {
            let _memo = g.query_memo();
            let current_target = self.builder_targets.get(&uid).copied().filter(|pos| {
                !reserved.contains(pos)
                    && (self.commitment_patience || job_out_of_reach(*pos))
                    && !self
                        .worthwhile_improvements(g, pid, *pos, strategy)
                        .is_empty()
            });
            match current_target {
                Some(pos) => Ok(pos),
                None => {
                    let mut best: Option<(f64, Pos)> = None;
                    for cid in g.player_city_ids(pid) {
                        let city_shortfall = self.city_production_foundation_shortfall(g, pid, cid);
                        for pos in &g.cities[&cid].owned_tiles {
                            if reserved.contains(pos) || !job_out_of_reach(*pos) {
                                continue;
                            }
                            for improvement in self.worthwhile_improvements(g, pid, *pos, strategy)
                            {
                                let value = self.production_foundation_improvement_value(
                                    g,
                                    pid,
                                    *pos,
                                    &improvement,
                                    strategy,
                                    city_shortfall,
                                );
                                let distance = g.wdist(current, *pos);
                                let score = travel.score(value, distance);
                                if best
                                    .map(|(old, bp)| score > old || (score == old && *pos < bp))
                                    .unwrap_or(true)
                                {
                                    best = Some((score, *pos));
                                }
                                // Only to say when the gene changed the pick.
                                if travel.priced_in_turns() {
                                    let untreated = builder_travel_turns::BuilderTravel::LEGACY
                                        .score(value, distance);
                                    if untreated_pick
                                        .map(|(old, bp)| {
                                            untreated > old || (untreated == old && *pos < bp)
                                        })
                                        .unwrap_or(true)
                                    {
                                        untreated_pick = Some((untreated, *pos));
                                    }
                                }
                            }
                        }
                    }
                    Err(best.map(|(_, pos)| pos))
                }
            }
        };
        if let (Err(Some(pos)), Some((_, farther))) = (best, untreated_pick) {
            if pos != farther {
                think!(self.journal(), Expansion, Decision, "Builder takes the nearer work";
                    "builder-prices-travel-in-turns: the job at {pos:?} is {} turn(s) off and the one at {farther:?} {}; per turn of the Builder's time the nearer one is worth more",
                    travel.turns(g.wdist(current, pos)), travel.turns(g.wdist(current, farther)); pos);
            }
        }
        let target = match best {
            Ok(pos) => Some(pos),
            Err(found) => found,
        };
        target.is_some_and(|pos| {
            self.builder_targets.insert(uid, pos);
            self.builder_step_toward_job(g, pid, uid, pos)
        })
    }

    pub(super) fn builder_step_toward_job(
        &mut self,
        game: &mut Game,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> bool {
        let before = game.units[&uid].pos;
        let track_progress = !self.base.legacy_movement;
        let routed = if track_progress {
            game.route_step(uid, target, 0)
        } else {
            None
        };
        let stepped = if self.builder_reach_safety_on() {
            self.builder_step_out_of_reach(game, pid, uid, target)
        } else {
            self.builder_step_toward_barbarian_safe(game, pid, uid, target)
        };
        if stepped {
            let after = game.units[&uid].pos;
            if track_progress
                && game.wdist(before, after) == 1
                && Some(after) != routed
                && game.wdist(after, target) >= game.wdist(before, target)
            {
                let until = game.turn + game.standard_duration(SETTLER_TARGET_HYSTERESIS_TURNS);
                self.builder_targets.remove(&uid);
                self.builder_avoid.insert(uid, (target, until));
                think!(self.journal(), Expansion, Decision, "Builder postpones a job after a safe retreat";
                    "Builder {uid} moves from {before:?} to {after:?}, not along the route or closer to {target:?}; keep the safe step and reconsider that job from turn {until}"; after);
            } else {
                self.builder_targets.insert(uid, target);
            }
        }
        stepped
    }

    /// Every job this Builder could take, best first, under the same score the
    /// untreated branch of [`Self::advanced_builder_step`] maximises.
    ///
    /// Reading every tile the empire owns: one memo scope so the empire-wide
    /// questions each tile asks are answered once for the whole sweep rather
    /// than once per tile. The borrow checker rejects the guard the moment
    /// anything in here starts mutating the game.
    pub(super) fn builder_jobs_ranked(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        strategy: GrandStrategy,
        reserved: &HashSet<Pos>,
    ) -> Vec<Pos> {
        let current = g.units[&uid].pos;
        // `builder-prices-travel-in-turns`; untreated while the gene is off.
        let travel = self.builder_travel(g, uid);
        let _memo = g.query_memo();
        let mut scored: Vec<(f64, Pos)> = Vec::new();
        for cid in g.player_city_ids(pid) {
            let city_shortfall = self.city_production_foundation_shortfall(g, pid, cid);
            for pos in &g.cities[&cid].owned_tiles {
                if reserved.contains(pos) {
                    continue;
                }
                let mut here: Option<f64> = None;
                for improvement in self.worthwhile_improvements(g, pid, *pos, strategy) {
                    let score = travel.score(
                        self.production_foundation_improvement_value(
                            g,
                            pid,
                            *pos,
                            &improvement,
                            strategy,
                            city_shortfall,
                        ),
                        g.wdist(current, *pos),
                    );
                    if here.is_none_or(|old| score > old) {
                        here = Some(score);
                    }
                }
                if let Some(score) = here {
                    scored.push((score, *pos));
                }
            }
        }
        // Descending by score, position as the tiebreak: exactly the order the
        // untreated branch's running `best` would have settled on.
        scored.sort_by(|(a, ap), (b, bp)| {
            b.partial_cmp(a)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(ap.cmp(bp))
        });
        scored.into_iter().map(|(_, pos)| pos).collect()
    }

    /// Walk toward the best job this Builder can actually reach, rather than
    /// toward the best job and nowhere otherwise. Opt-in gene
    /// `builder-tries-the-next-tile`.
    ///
    /// ⚠⚠ THE BEST-SCORING TILE IS NOT ALWAYS A REACHABLE ONE, AND THE
    /// UNTREATED BRANCH ABOVE HAD NO SECOND CHOICE. It scores every owned tile,
    /// pins the winner in `builder_targets`, hands that one position to
    /// `builder_step_toward_barbarian_safe`, and returns whatever that says.
    /// The distance term is `wdist` — a straight line across the map — so a
    /// ridge, a zone of control, a peer standing in the doorway or a tile on
    /// the far side of a bay all score exactly as if the Builder could walk
    /// there. When the step is refused the turn ends; worse, the pin survives,
    /// and the hysteresis at the head of the untreated branch re-selects the
    /// same unreachable tile next turn, and every turn after that.
    ///
    /// Measured at the deployment genome (seed 21000000): 22 Builders stood
    /// still for 25+ turns across eight games, Builders were 25.4% of all major
    /// idle and 24.9% of their own turns, and at t180 there were 25 Builders
    /// alive holding 73 charges with NOT ONE of them in an empire that had run
    /// out of improvable tiles. One caught in the act — builder 312 of seat 5,
    /// thirty turns at (10, 29), three charges, improvable work five tiles away
    /// on its own landmass.
    ///
    /// So: rank every candidate, keep the pin at the head so a Builder already
    /// walking somewhere does not re-shuffle for a tie, and take the first one
    /// the unit can actually start toward. A refused tile is *unpinned* — that
    /// is the half that ends the stall, because a pin that survives its own
    /// refusal is the loop. Bounded to `BUILDER_ROUTE_ATTEMPTS` so a sweep that
    /// finds a hundred jobs cannot become a hundred pathfinds; ordinary turns
    /// succeed on the first and pay nothing.
    pub(super) fn builder_step_to_the_first_reachable_job(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        strategy: GrandStrategy,
        reserved: &HashSet<Pos>,
    ) -> bool {
        // ⚠ THE FAST PATH IS THE WHOLE COST STORY. A Builder already walking to
        // a job it can still do takes one `worthwhile_improvements` call and
        // one step, exactly as the untreated branch does; the empire-wide sweep
        // below is never reached. Ranking every owned tile unconditionally
        // instead — the first draft of this — measured **+11.1 ± 5.2% wall
        // seconds per completed turn** over 24 games, because a pinned Builder
        // paid one sweep of the whole empire per turn to re-derive an answer it
        // already had.
        let pinned = {
            let _memo = g.query_memo();
            self.builder_targets.get(&uid).copied().filter(|pos| {
                !reserved.contains(pos)
                    && !self
                        .worthwhile_improvements(g, pid, *pos, strategy)
                        .is_empty()
            })
        };
        let mut attempts = BUILDER_ROUTE_ATTEMPTS;
        if let Some(pos) = pinned {
            if self.builder_step_toward_job(g, pid, uid, pos) {
                return true;
            }
            // ⚠ A Builder with nothing left to spend refuses EVERY tile, and
            // that is not the failure this gene is for. Stop before paying for
            // a second route and keep the pin: the unit is mid-journey and next
            // turn it should carry on, not re-shuffle. Without this the
            // treatment would drop hysteresis on any turn a Builder ran its
            // movement out.
            if g.units[&uid].moves_left <= 0.0 {
                return false;
            }
            // Refused with movement in hand: this tile cannot be routed to.
            // Drop the pin so it never becomes next turn's answer either — a
            // pin that survives its own refusal is the loop that produced the
            // thirty-turn stall.
            self.builder_targets.remove(&uid);
            attempts -= 1;
        }
        let ranked = self.builder_jobs_ranked(g, pid, uid, strategy, reserved);
        for pos in ranked
            .into_iter()
            .filter(|pos| Some(*pos) != pinned)
            .take(attempts)
        {
            if self.builder_step_toward_job(g, pid, uid, pos) {
                return true;
            }
            if g.units[&uid].moves_left <= 0.0 {
                return false;
            }
        }
        false
    }
}
