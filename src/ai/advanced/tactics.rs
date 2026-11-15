//! The war's tactical layer: how `AdvancedAi` groups and moves its army.
//!
//! `rebuild_force_groups` forms the army into force groups each turn and
//! `domain_objective`/`force_focus_target` give each one its objective;
//! `coordinated_tactical_step` moves a group against it. Around those sit the
//! war-staging steps (`campaign_staging_step`, `rush_siege_step`,
//! `wartime_reinforcement_step`, the timed-war objective/finisher steps and
//! `war_return_home_step`), the opening strike's value
//! (`strike_opening_value`), and the air arm (`advanced_air_action`).
//!
//! Moved verbatim out of the one `impl AdvancedAi` block in `advanced.rs`
//! (2026-11-04). Methods the rest of the controller calls are `pub(super)`;
//! the code is otherwise unchanged.

use super::*;

impl AdvancedAi {
    /// Drive one melee unit of an ancient rush directly at the objective
    /// capital, bypassing the force-group heuristics entirely.
    ///
    /// Four separate attempts to make those heuristics conduct a siege made
    /// things measurably worse (see the rejected notes on `focus_target` and
    /// on the `relieving`/`Muster` postures). They are tuned for a field
    /// campaign between comparable armies, and a rush is not that: it is four
    /// melee units against an unwalled capital holding a garrison of 0.7,
    /// inside a window that shuts.
    ///
    /// The routine is the Monte Carlo's own recipe, in order:
    ///
    /// 1. **Walk into a depleted city.** An ordinary ranged attack cannot take
    ///    a city below 1 HP, so a city at 0 was opened by a Bombard and is
    ///    standing open for whoever steps in.
    /// 2. **Attack from the ring** if already on it.
    /// 3. **Take a free ring tile.** Every melee unit on the ring both adds a
    ///    blow and helps seal the siege, and an unsealed city heals 20 HP a
    ///    turn — which is what actually defeated this lane before: the stack
    ///    reached the 3-5 tile staging ring at full strength (measured max 4)
    ///    while the city's own ring never held more than two.
    ///
    /// Returns `None` when this unit is not part of a live rush, leaving the
    /// ordinary wartime behaviour untouched.
    pub(super) fn rush_siege_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
    ) -> Option<bool> {
        if !plan.rush {
            return None;
        }
        let target = plan.target_player?;
        if !g.is_at_war(pid, target) {
            return None;
        }
        let cid = plan.target_city?;
        let city = g.cities.get(&cid).filter(|city| city.owner == target)?;
        let objective = city.pos;
        let depleted = city.hp <= 0;
        let spec = &g.rules.units[g.units.get(&uid)?.kind];
        if !spec.is_melee_capable() || matches!(spec.domain.as_deref(), Some("sea" | "air")) {
            return None;
        }
        let here = g.units[&uid].pos;

        if g.wdist(here, objective) <= 1 {
            if depleted && g.can_move(uid, objective) {
                // The city is open. Walking in *is* the capture.
                return Some(
                    g.apply(
                        pid,
                        &Action::Move {
                            unit: uid,
                            to: objective,
                        },
                    )
                    .is_ok(),
                );
            }
            let attacked = g
                .apply(
                    pid,
                    &Action::Attack {
                        unit: uid,
                        target: objective,
                    },
                )
                .is_ok();
            if attacked {
                return Some(true);
            }
            // Out of moves this turn: hold the ring rather than give it up.
            // A vacated ring tile is 20 HP a turn handed back.
            return Some(self.base.fortify_or_stop(g, pid, uid));
        }

        // Close on a free ring tile — but *which* ring tile decides whether
        // this is a siege or a queue.
        //
        // `district_under_siege` needs every passable neighbour of the city
        // occupied or covered by our zone of control, and a city that is not
        // besieged heals 20 HP a turn. A ZOC unit covers its own ring tile
        // plus both ring-neighbours, three of six, so **two units placed three
        // apart seal the ring and two units side by side do not**. Routing to
        // the nearest free tile bunches them, which is why an unsealed capital
        // took 23 turns to fall against a Monte Carlo estimate of three.
        //
        // So prefer the free tile furthest from the ones we already hold, and
        // only then the one we can reach.
        let held: Vec<Pos> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|other| *other != uid)
            .filter_map(|other| g.units.get(&other))
            .filter(|other| g.rules.units[other.kind].is_melee_capable())
            .map(|other| other.pos)
            .filter(|pos| g.wdist(*pos, objective) <= 1)
            .collect();
        let mut ring: Vec<Pos> = g
            .wdisk(objective, 1)
            .into_iter()
            .filter(|pos| {
                *pos != objective
                    && g.unit_can_traverse(uid, *pos)
                    && g.unit_ids_at(*pos)
                        .iter()
                        .all(|other| g.units.get(other).is_some_and(|other| other.owner != pid))
            })
            .collect();
        if ring.is_empty() {
            return None;
        }
        // Spread first, then nearness to us, then position for determinism.
        ring.sort_by_key(|pos| {
            let spread = held
                .iter()
                .map(|other| g.wdist(*pos, *other))
                .min()
                .unwrap_or(i32::MAX);
            (std::cmp::Reverse(spread), g.wdist(here, *pos), pos.0, pos.1)
        });
        // Walk the preference order: the best tile may be unroutable this
        // turn, and giving up on it would leave the unit standing still.
        for goal in ring.iter().take(3) {
            let goals: HashSet<Pos> = std::iter::once(*goal).collect();
            if let Some(next) = g
                .route_step_to_any(uid, &goals)
                .filter(|pos| g.can_move(uid, *pos))
            {
                return Some(
                    g.apply(
                        pid,
                        &Action::Move {
                            unit: uid,
                            to: next,
                        },
                    )
                    .is_ok(),
                );
            }
        }
        None
    }

    /// Redirect an otherwise idle field unit to the active conquest front.
    /// Returning `Some` means the campaign owns this unit's peacetime order,
    /// including holding a completed staging position; `None` leaves ordinary
    /// patrol, exploration, and naval-escort behavior unchanged.
    pub(super) fn campaign_staging_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
    ) -> Option<bool> {
        // `edge-unit-takes-the-soft-city`: a body of the edge force walks to
        // its stand 3-4 tiles from the soft city and holds it. `None` with
        // the gene off and for every other unit. See
        // `advanced/edge_unit_takes_the_soft_city.rs`.
        if let Some(acted) = self.edge_unit_staging_step(g, pid, uid) {
            return Some(acted);
        }
        let opening = self.conquest_staging_objective(g, pid, uid, plan);
        let (target, objective) = if let Some((target, objective, _)) = opening {
            (target, objective)
        } else {
            if plan.strategy != GrandStrategy::Conquest {
                return None;
            }
            let target = plan.target_player?;
            let objective = plan
                .target_city
                .and_then(|city| g.cities.get(&city))
                .filter(|city| city.owner == target)
                .map(|city| city.pos)?;
            (target, objective)
        };
        let on_opening_ring = |position| {
            opening.is_none_or(|(_, _, rally)| {
                g.wdist(position, rally) <= early_conquest::CONQUEST_ASSEMBLY_RADIUS
            })
        };
        if !self.campaign_target_legal(g, pid, target) || g.is_at_war(pid, target) {
            return None;
        }
        let unit = &g.units[&uid];
        let spec = &g.rules.units[unit.kind];
        if !matches!(spec.class.as_str(), "military" | "support")
            || matches!(spec.domain.as_deref(), Some("sea" | "air"))
            || (BasicAi::unit_doctrine(g, uid) == UnitDoctrine::Recon
                && self.base.has_exploration_target(g, pid, uid))
        {
            return None;
        }
        // An appointed campaign owns an exact package, not every military
        // unit in the empire. Scouts, home defenders, and unrelated ranged
        // units keep their normal jobs while the four assault bodies and real
        // breach element assemble without independently changing target.
        if let Some(war) = self.war_plan.as_ref().filter(|war| {
            opening.is_none()
                && war.target_player == target
                && war.objective_city == plan.target_city.unwrap_or(0)
        }) {
            let designated =
                self.war_body_kind(g, pid, war, unit.kind) || war.breach_unit == Some(unit.kind);
            if !designated {
                return None;
            }
        }
        let already_staged = on_opening_ring(unit.pos)
            && self.campaign_staging_position(g, pid, target, uid, objective, unit.pos);
        if already_staged && !g.rules.is_water(&g.map.tiles[&unit.pos]) {
            return Some(self.base.fortify_or_stop(g, pid, uid));
        }

        let current = unit.pos;
        let goals: HashSet<Pos> = {
            let _memo = g.query_memo();
            g.wdisk(
                objective,
                self.campaign_staging_reach(g, pid, target, objective),
            )
            .into_iter()
            .filter(|position| {
                on_opening_ring(*position)
                    && self.campaign_staging_position(g, pid, target, uid, objective, *position)
                    && g.unit_ids_at(*position).is_empty()
            })
            .collect()
        };
        // Prefer an accessible dry assembly area. A coastal city can own
        // every beach on its island, leaving its nominal dry staging ring
        // behind closed borders. In that case embark next to a landing shore
        // and wait offshore until the assembled army can declare.
        let (water, dry): (HashSet<_>, HashSet<_>) = goals
            .into_iter()
            .partition(|position| g.rules.is_water(&g.map.tiles[position]));
        let (next, overland) = if let Some(next) = g.route_step_to_any(uid, &dry) {
            (next, true)
        } else if already_staged {
            return Some(self.base.fortify_or_stop(g, pid, uid));
        } else {
            (g.route_step_to_any(uid, &water)?, false)
        };
        if !g.can_move(uid, next) {
            return None;
        }
        // Do not use an Open Borders shortcut through the intended victim.
        // The next turn's route search will find a lawful way around it.
        let next_territory = g.map.tiles[&next]
            .owner_city
            .and_then(|city| g.cities.get(&city))
            .map(|city| city.owner);
        if next_territory == Some(target) {
            return Some(self.base.fortify_or_stop(g, pid, uid));
        }
        debug_assert_ne!(next, current);
        // `march-uses-its-moves`: walk as far toward the dry ring as this
        // turn's movement reaches, never through the victim's land.
        if overland && self.march_uses_its_moves {
            let victim_free = |g: &Game, _: Pos, path: &[Pos]| {
                path.iter().all(|pos| {
                    g.map.tiles[pos]
                        .owner_city
                        .and_then(|city| g.cities.get(&city))
                        .map(|city| city.owner)
                        != Some(target)
                })
            };
            if let Some(dest) = self.march_destination(g, uid, &dry, next, victim_free) {
                let kind = g.units[&uid].kind;
                if self.base.path_walk_to(g, pid, uid, dest) {
                    think!(self.journal(), Military, Detail,
                        "Campaign march: the {} walks {} tiles toward the staging ring", plain(&kind), g.wdist(current, dest);
                        "its movement reaches {:?}, {} tiles from the objective, where the router's single step reached {:?}",
                        dest, g.wdist(dest, objective), next;
                        objective);
                    return Some(true);
                }
            }
        }
        // `risk-is-priced`: the router's single step is refused like any
        // other. Never with the gene off.
        if self.base.priced_risk_refuses(g, uid, current, next) {
            return Some(self.base.fortify_or_stop(g, pid, uid));
        }
        Some(
            g.apply(
                pid,
                &Action::Move {
                    unit: uid,
                    to: next,
                },
            )
            .is_ok(),
        )
    }

    /// March a rear unit toward the campaign objective while the war is on.
    ///
    /// `campaign_staging_step` assembles the army before the declaration and
    /// then deliberately stands down: at war, every field unit belongs to a
    /// force group. But groups are cliques at `command_radius`, so the
    /// reinforcement trickle — fresh production, healed units, released
    /// garrisons — forms one- and two-body groups at home whose local
    /// strength at the objective can never clear `LOCAL_SUPERIORITY_FLOOR`.
    /// They hold forever, and the front fights the whole war with whatever
    /// staged on declaration day. Measured on live run
    /// `civvis-20260808T033223Z`: the why ledger shows land forces of one,
    /// two and three against the same objective from t217 to t225, every one
    /// "too weak locally to advance", while the empire fielded 10-14 units.
    ///
    /// Deliberately narrow, in order of what it declines: the homeland keeps
    /// first claim (any threatened city stands the march down empire-wide),
    /// the front group and any group already moving keep their measured
    /// posture logic, a unit in contact belongs to the tactical layer, and a
    /// unit below `withdraw_hp` heals where it stands. What remains is
    /// exactly the standing-still rear, and it walks the same road the
    /// pre-war assembly walked: routed to the objective's 3..=5 staging
    /// ring, never onto the city tile itself. Groups are rebuilt from the
    /// board every turn, so an arrival within `command_radius` of the front
    /// merges into it and takes the front's orders from then on.
    pub(super) fn wartime_reinforcement_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
        group: Option<&ForceGroup>,
        enemies: &[usize],
    ) -> Option<bool> {
        if !self.war_reinforcement || plan.strategy != GrandStrategy::Conquest {
            return None;
        }
        if plan.threatened_city.is_some() {
            return None;
        }
        let target = plan.target_player?;
        if !g.is_at_war(pid, target) {
            return None;
        }
        let objective = plan
            .target_city
            .and_then(|city| g.cities.get(&city))
            .filter(|city| city.owner == target)
            .map(|city| city.pos)?;
        let rear = group.is_none_or(|group| {
            matches!(group.posture, ForcePosture::Hold | ForcePosture::Muster)
                && g.wdist(group.anchor, objective) > REINFORCEMENT_FRONT_RADIUS
        });
        if !rear {
            return None;
        }
        let unit = &g.units[&uid];
        // Arrival belongs to this unit, not the rearward group anchor. The
        // routing goals exclude occupied tiles, including our own tile, so
        // an arrived reinforcement otherwise shuttles between staging posts
        // and consumes its turn before the tactical controller can act.
        if g.wdist(unit.pos, objective) <= 5 {
            return None;
        }
        let spec = &g.rules.units[unit.kind];
        if !matches!(spec.class.as_str(), "military" | "support")
            || matches!(spec.domain.as_deref(), Some("sea" | "air"))
            || (BasicAi::unit_doctrine(g, uid) == UnitDoctrine::Recon
                && self.base.has_exploration_target(g, pid, uid))
        {
            return None;
        }
        if (unit.hp as f64) <= self.base.w.withdraw_hp {
            return None;
        }
        let in_contact = g
            .units
            .values()
            .any(|enemy| enemies.contains(&enemy.owner) && g.wdist(unit.pos, enemy.pos) <= 2);
        if in_contact {
            return None;
        }
        let here = unit.pos;
        let kind = unit.kind;
        // The same road the pre-war assembly walks: route to the objective's
        // staging ring, never onto the city tile itself (a city is attacked,
        // not entered, so a route aimed at its tile finds nothing).
        let goals: HashSet<Pos> = {
            let _memo = g.query_memo();
            g.wdisk(objective, 5)
                .into_iter()
                .filter(|position| {
                    (3..=5).contains(&g.wdist(*position, objective))
                        && g.unit_ids_at(*position).is_empty()
                        && g.city_at(*position).is_none()
                        && g.map
                            .get(*position)
                            .is_some_and(|tile| !g.rules.is_water(tile))
                })
                .collect()
        };
        // Over dry land when a reasonable land road exists; see
        // `siege_train::march_step`.
        let dry_step = siege_train::dry_march_step(g, uid, objective, 5).filter(|position| {
            g.map
                .get(*position)
                .is_some_and(|tile| !g.rules.is_water(tile))
        });
        let routed = dry_step.is_none();
        let next = dry_step
            .or_else(|| g.route_step_to_any(uid, &goals))
            .filter(|position| *position != here && g.can_move(uid, *position))?;
        // A step the battle planner's rotation would pull straight back out
        // is a wasted turn twice over. Live King civvis-20261001T010043Z: a
        // heavy chariot walked (20,17) -> (22,20) toward Warsaw at turn 99,
        // was ordered back to (20,17) at 100 and to (17,17) at 101, and the
        // Krakow siege staged nothing for nineteen turns. Hold at the edge
        // instead; the next arrivals share the reach and lower the reading.
        if self.battle_planner_2 || self.battle_planner_3 {
            let hp = g.units[&uid].hp;
            let mut field = battle_planner::DangerField::with_reach(g, pid, true);
            if self.shared_danger {
                field.share(g);
            }
            if field.rotation_danger(next, uid)
                > f64::from(hp - battle_planner::ROTATE_DANGER_MARGIN)
            {
                if self.journal().wants(crate::reasoning::Level::Detail) {
                    think!(self.journal(), Military, Detail,
                           "Reinforcement {} holds at the edge of the front", plain(&kind);
                           "the next step toward {:?} would read as exposed and be rotated back out",
                           objective;
                           objective);
                }
                return Some(self.base.fortify_or_stop(g, pid, uid));
            }
        }
        // `march-uses-its-moves`: walk as far toward the staging ring as this
        // turn's movement reaches, to a tile the rotation would not pull
        // straight back out.
        if routed && self.march_uses_its_moves {
            let hp = g.units[&uid].hp;
            let mut rotation = (self.battle_planner_2 || self.battle_planner_3).then(|| {
                let mut field = battle_planner::DangerField::with_reach(g, pid, true);
                if self.shared_danger {
                    field.share(g);
                }
                field
            });
            let unexposed = |_: &Game, pos: Pos, _: &[Pos]| {
                rotation.as_mut().is_none_or(|field| {
                    field.rotation_danger(pos, uid)
                        <= f64::from(hp - battle_planner::ROTATE_DANGER_MARGIN)
                })
            };
            if let Some(dest) = self.march_destination(g, uid, &goals, next, unexposed) {
                if self.base.path_walk_to(g, pid, uid, dest) {
                    think!(self.journal(), Military, Detail,
                           "Reinforcement {} walks {} tiles toward the front", plain(&kind), g.wdist(here, dest);
                           "its movement reaches {:?}, {} tiles from {:?}, where the router's single step reached {:?}",
                           dest, g.wdist(dest, objective), objective, next;
                           objective);
                    return Some(true);
                }
            }
        }
        if self.journal().wants(crate::reasoning::Level::Detail) {
            think!(self.journal(), Military, Detail,
                   "Reinforcing the campaign with {}", plain(&kind);
                   "the front at {:?} is {} tiles away and this unit's group cannot \
                    advance from here",
                   objective, g.wdist(here, objective);
                   objective);
        }
        // `risk-is-priced`: as in `campaign_staging_step`.
        if self.base.priced_risk_refuses(g, uid, here, next) {
            return Some(self.base.fortify_or_stop(g, pid, uid));
        }
        Some(
            g.apply(
                pid,
                &Action::Move {
                    unit: uid,
                    to: next,
                },
            )
            .is_ok(),
        )
    }

    /// A predecessor that missed the breakthrough upgrade because it was
    /// abroad returns to the nearest owned city before joining the staging
    /// column. This keeps Mobilize from marching an obsolete body away from
    /// the only territory where its reserved upgrade can be executed.
    pub(super) fn war_return_home_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        let war = self.war_plan.as_ref()?;
        if !matches!(war.phase, WarPhase::Mobilize | WarPhase::Stage)
            || war.predecessor != Some(g.units.get(&uid)?.kind)
        {
            return None;
        }
        let here = g.units[&uid].pos;
        let in_owned_territory = g.map.get(here).is_some_and(|tile| {
            tile.owner_city
                .and_then(|city| g.cities.get(&city))
                .is_some_and(|city| city.owner == pid)
        });
        if in_owned_territory {
            return Some(self.base.fortify_or_stop(g, pid, uid));
        }
        let home = g
            .player_city_ids(pid)
            .into_iter()
            .min_by_key(|city| (g.wdist(here, g.cities[city].pos), *city))
            .map(|city| g.cities[&city].pos)?;
        Some(self.base.step_toward(g, pid, uid, home))
    }

    /// Finish the appointed city as soon as ranged/siege setup opens it. The
    /// normal unit order already resolves those roles before melee; this hook
    /// only claims a body when the city is actually depleted, so it does not
    /// repeat the rejected ancient behavior of ignoring field defenders.
    pub(super) fn timed_war_finisher_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        let war = self
            .war_plan
            .as_ref()
            .filter(|war| war.phase == WarPhase::Exploit)?;
        let unit = g.units.get(&uid)?;
        if !Self::war_ready_body_kind(g, pid, war, unit.kind)
            || !g.is_at_war(pid, war.target_player)
        {
            return None;
        }
        let city = g
            .cities
            .get(&war.objective_city)
            .filter(|city| city.owner == war.target_player && city.hp <= 0)?;
        let objective = city.pos;
        if g.can_move(uid, objective) {
            return Some(
                g.apply(
                    pid,
                    &Action::Move {
                        unit: uid,
                        to: objective,
                    },
                )
                .is_ok(),
            );
        }
        if g.wdist(unit.pos, objective) <= 1 {
            return Some(
                g.apply(
                    pid,
                    &Action::Attack {
                        unit: uid,
                        target: objective,
                    },
                )
                .is_ok(),
            );
        }
        Some(self.base.step_toward(g, pid, uid, objective))
    }

    /// Press the pinned objective without blinding the tactical solver. A
    /// package unit yields to ordinary combat whenever a field defender is in
    /// its immediate way; otherwise siege fires first (the unit loop already
    /// orders it before melee) and assault bodies close directly on the city.
    pub(super) fn timed_war_objective_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if let Some(finished) = self.timed_war_finisher_step(g, pid, uid) {
            return Some(finished);
        }
        let war = self
            .war_plan
            .as_ref()
            .filter(|war| war.phase == WarPhase::Exploit)?
            .clone();
        if !g.is_at_war(pid, war.target_player) {
            return None;
        }
        let city = g
            .cities
            .get(&war.objective_city)
            .filter(|city| city.owner == war.target_player)?;
        let objective = city.pos;
        let unit = g.units.get(&uid)?.clone();
        let is_body = Self::war_ready_body_kind(g, pid, &war, unit.kind);
        let is_breach = war.breach_unit == Some(unit.kind);
        if !is_body && !is_breach {
            return None;
        }
        let defender_in_the_way = g
            .nbrs(unit.pos)
            .into_iter()
            .flat_map(|position| g.unit_ids_at(position))
            .any(|other| {
                g.units.get(other).is_some_and(|other| {
                    other.owner == war.target_player
                        && g.rules.units[other.kind].class == "military"
                })
            });
        if defender_in_the_way {
            return None;
        }

        let spec = &g.rules.units[unit.kind];
        if is_breach && spec.has_ranged_attack() {
            if g.wdist(unit.pos, objective) <= g.unit_attack_range(uid)
                && g.apply(
                    pid,
                    &Action::Ranged {
                        unit: uid,
                        target: objective,
                    },
                )
                .is_ok()
            {
                return Some(true);
            }
            return self
                .base
                .step_toward_range(g, pid, uid, objective, g.unit_attack_range(uid).max(1))
                .then_some(true);
        }
        if is_body && g.wdist(unit.pos, objective) <= 1 {
            return g
                .apply(
                    pid,
                    &Action::Attack {
                        unit: uid,
                        target: objective,
                    },
                )
                .is_ok()
                .then_some(true);
        }
        (is_body && self.base.step_toward(g, pid, uid, objective)).then_some(true)
    }

    /// Whether a ranged unit standing on `tile` would be in range of something
    /// and unable to shoot any of it.
    ///
    /// `Game::do_ranged` refuses a shot with `"line of sight blocked"`, and
    /// **that is the single largest reason CIVVIS's army does not attack.**
    /// PR #989 made the forward model's refusals name themselves; replaying
    /// run `civvis-20260803T005930Z` (Kongo vs Korea, 188 turns of war), of 87
    /// declined attacks **45 were the model refusing the action outright**,
    /// and they broke down as:
    ///
    /// ```text
    /// line of sight blocked                          25
    /// cannot attack while embarked                   19
    /// nothing to attack                               9
    /// siege units cannot move and attack in one turn  5
    /// no combat target                                4
    /// not enough movement to attack                   3
    /// ```
    ///
    /// 27 of the 45 were a Field Cannon. The cause is that the movement
    /// scorer picks a tile by `wdist` to the target and by `preferred_depth`
    /// — which for a ranged unit *is* its attack range — and **never asks
    /// whether the target can be seen from there.** So the unit marches
    /// exactly into range and then cannot fire.
    ///
    /// Line of sight binds at exactly distance 2: `has_line_of_sight` returns
    /// true unconditionally at 1 (adjacent fire) and at 3+ (lobbed shots).
    /// Range-2 units are therefore the whole population, which is why Field
    /// Cannons, Crossbowmen and Trebuchets dominate the census and Archers
    /// barely appear.
    ///
    /// A tile is only blind if it is in range of at least one hostile *and*
    /// sighted on none of them — a tile out of range entirely is not blind, it
    /// is just far, and [`AdvancedAi::strike_opening_value`] and the depth
    /// term already speak to that.
    /// What a tile is worth for the attack it opens.
    ///
    /// Movement otherwise prices only progress, support, and incoming threat;
    /// this term gives a reachable attack its own bounded value.
    ///
    /// The hole is not neutral, it is one-sided. Standing where an enemy can
    /// reach costs `mv_threat * threat_caution * 30.0 * exp(..)` — **-15.0 at
    /// parity for a Vanguard on the shipped weights** — while closing one hex
    /// is worth `objective_progress * progress` = +2.9 plus `role_spacing`
    /// +2.0. **Contact is priced at roughly three times what closing is worth,
    /// and the attack that contact buys is worth nothing at all.** A melee
    /// unit therefore never volunteers for the tile it has to occupy to
    /// swing.
    ///
    /// Measured on live run `civvis-20260803T005930Z` (Kongo vs Korea, war
    /// t64-251), joining the run's own `orders.sqlite` to its per-turn state:
    ///
    /// - **7 melee ATTACK orders in 188 turns of war**, six of them inside one
    ///   seven-turn window, against 1546 MOVE_TO.
    /// - Of 1787 military unit-turns, **622 (35%) sat 2-4 hexes from a target
    ///   without closing**, and **52% of those were under an `Engage`
    ///   posture** — ordered to attack, declining to approach. 390 of the 622
    ///   were melee, `UNIT_HEAVY_CHARIOT` alone accounting for 156 with four
    ///   movement to spend.
    /// - Movement itself is healthy: 67% of war `MOVE_TO` orders arrived and
    ///   **0% named the unit's own tile**, so this is not
    ///   `civvis-civ6-the-order-that-goes-nowhere`.
    ///
    /// The repair credits a tile for the strike it makes available, on the
    /// same scale as the threat it charges, so the two are commensurable. Only
    /// targets the group is actually prosecuting count — a hostile that merely
    /// wanders into range is already handled by the threat term, and paying
    /// for it here would pull the column off its objective.
    ///
    /// Odds are `attack / (attack + defense)`, bounded in `(0, 1)` by
    /// construction, so unlike `mv_threat`'s uncapped `exp` this term cannot
    /// run away. At even odds it returns `0.5 * 30.0 *
    /// STRIKE_OPENING_SCALE` = +5.2, which offsets a third of the parity
    /// threat penalty rather than erasing it: the intent is to make contact
    /// *considerable*, not automatic.
    pub(super) fn strike_opening_value(&self, input: StrikeOpening<'_>) -> f64 {
        let StrikeOpening {
            g,
            pid,
            uid,
            tile,
            group,
            enemies,
            visible,
        } = input;
        if self
            .defensive_firing_radius(g, pid, uid)
            .is_some_and(|(city, radius)| g.wdist(tile, city) > radius)
        {
            return f64::NEG_INFINITY;
        }
        if !self.strike_opening || group.posture != ForcePosture::Engage {
            return 0.0;
        }
        let unit = &g.units[&uid];
        let spec = &g.rules.units[unit.kind];
        if spec.class != "military" {
            return 0.0;
        }
        let reach = if spec.has_ranged_attack() {
            g.unit_attack_range(uid).max(1)
        } else {
            1
        };
        let attack = crate::game::effective_strength(
            g.unit_strength(unit, false)
                .max(g.unit_ranged_attack_strength(unit)),
            unit.hp,
        );
        if attack <= 0.0 {
            return 0.0;
        }
        // The objective the group is prosecuting, plus any hostile standing on
        // it. Anything else in range is the threat term's business.
        let focus = group.focus_target.unwrap_or(group.objective);
        let mut best_defense: Option<f64> = None;
        if g.wdist(tile, focus) <= reach {
            if let Some(city) = g.city_at(focus).filter(|city| {
                enemies.contains(&g.cities[city].owner)
                    && visible.is_none_or(|visible| g.sees(visible, g.cities[city].pos))
            }) {
                best_defense = Some(g.city_strength(city));
            }
        }
        for enemy in g.units.values() {
            if !enemies.contains(&enemy.owner)
                || g.rules.units[enemy.kind].class != "military"
                || g.wdist(tile, enemy.pos) > reach
            {
                continue;
            }
            if visible.is_some_and(|visible| {
                !(g.sees(visible, enemy.pos) && self.battlefront_unit_visible(g, pid, enemy.id))
            }) {
                continue;
            }
            let defense = crate::game::effective_strength(g.unit_strength(enemy, true), enemy.hp);
            best_defense = Some(best_defense.map_or(defense, |had: f64| had.min(defense)));
        }
        let Some(defense) = best_defense else {
            return 0.0;
        };
        let odds = attack / (attack + defense.max(1.0));
        STRIKE_OPENING_SCALE * 30.0 * odds
    }

    pub(super) fn force_domain(g: &Game, uid: u32) -> ForceDomain {
        if g.rules.units[g.units[&uid].kind].domain.as_deref() == Some("sea") {
            ForceDomain::Sea
        } else {
            ForceDomain::Land
        }
    }

    pub(super) fn force_role(g: &Game, uid: u32) -> ForceRole {
        match BasicAi::unit_doctrine(g, uid) {
            UnitDoctrine::Recon => ForceRole::Recon,
            UnitDoctrine::Assault => ForceRole::Vanguard,
            UnitDoctrine::Mobile => ForceRole::Mobile,
            UnitDoctrine::Ranged => ForceRole::Ranged,
            UnitDoctrine::Siege => ForceRole::Siege,
            UnitDoctrine::Support | UnitDoctrine::Carrier => ForceRole::Support,
            UnitDoctrine::AirDefense | UnitDoctrine::AirStrike => ForceRole::AirStrike,
        }
    }

    /// `relief_column_marches`: where a holding group stands while a city of
    /// ours is threatened and the group is beyond `THREAT_RELIEF_RADIUS` of
    /// it — the city itself, so a column too weak to break the siege still
    /// reinforces the garrison instead of standing at its own centroid. The
    /// mover's role spacing then arranges it around the centre the way it
    /// arranges a ring around any objective. `None` for a group inside the
    /// radius (it holds its ring as before), with no threatened city, and
    /// with the gene off.
    pub(super) fn relief_hold_point(&self, g: &Game, group: &ForceGroup) -> Option<Pos> {
        if !self.relief_column_marches {
            return None;
        }
        let city = self
            .plan
            .as_ref()?
            .threatened_city
            .and_then(|cid| g.cities.get(&cid))?;
        (g.wdist(group.anchor, city.pos) > THREAT_RELIEF_RADIUS).then_some(city.pos)
    }

    fn force_anchor(g: &Game, units: &[u32]) -> Pos {
        units
            .iter()
            .map(|uid| {
                let pos = g.units[uid].pos;
                let total: i32 = units
                    .iter()
                    .map(|other| g.wdist(pos, g.units[other].pos))
                    .sum();
                (total, *uid, pos)
            })
            .min()
            .map(|(_, _, pos)| pos)
            .unwrap_or((0, 0))
    }

    pub(super) fn domain_objective(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        domain: ForceDomain,
        anchor: Pos,
        enemies: &[usize],
    ) -> Pos {
        // On a capture-the-flag arena the enemy's flag is the objective: the
        // battle ends the moment a unit of ours stands on it, and it sits at
        // the far end of the field with their army around it, so marching on
        // it is also marching into the fight. Our own flag is not a target —
        // capturing it is not a thing that can happen — so the aim is
        // specifically theirs. `arena_enemy_flag` is `None` on every world
        // and on every arena whose match did not ask for flags, so
        // everything else walks past this untouched.
        if domain == ForceDomain::Land {
            if let Some(flag) = g.arena_enemy_flag(pid, anchor) {
                return flag;
            }
        }
        let visible = self.battlefront_visibility(g, pid);
        // An ancient rush keeps its objective. `threatened_city` outranks
        // `target_city` here and is an empire-wide fact, so the turn the
        // victim's counter-raid puts any city of ours under pressure the whole
        // column re-aims — homeward, or at whatever hostile unit is nearest
        // that city. Measured: melee standing adjacent to a rival capital runs
        // at **0.03 per civilization** at turn 50 while 1.01 sits on the
        // staging ring three to five tiles out. The stack marches, declares,
        // and then never closes the last three tiles.
        //
        // Trading a city for their capital is the whole bet of a rush, and it
        // is a bet the census says pays: their capital is unwalled and holds a
        // garrison of 0.7, ours is not the one under threat yet.
        let rush_objective = plan
            .rush
            .then(|| plan.target_city.and_then(|cid| g.cities.get(&cid)))
            .flatten();
        if domain == ForceDomain::Land {
            if let Some(city) = rush_objective {
                return city.pos;
            }
        }
        let threatened_enemy = plan.threatened_city.and_then(|cid| {
            let city = g.cities.get(&cid)?;
            g.units
                .values()
                .filter(|unit| {
                    enemies.contains(&unit.owner)
                        && (!self.battlefront_observation
                            || (g.sees(&visible, unit.pos)
                                && self.battlefront_unit_visible(g, pid, unit.id)))
                        && match domain {
                            ForceDomain::Sea => BasicAi::waterborne(g, unit.id),
                            ForceDomain::Land => !BasicAi::waterborne(g, unit.id),
                        }
                        && g.wdist(city.pos, unit.pos) <= 8
                })
                // Nearest THE CITY first — see `relief_targets_the_siege`. With
                // the flag off this term is a constant and the ordering is the
                // historical one, nearest the force.
                .min_by_key(|unit| {
                    let besieging = if self.relief_targets_the_siege {
                        g.wdist(city.pos, unit.pos)
                    } else {
                        0
                    };
                    (besieging, g.wdist(anchor, unit.pos), unit.id)
                })
                .map(|unit| unit.pos)
        });
        if let Some(pos) = threatened_enemy {
            return pos;
        }

        let planned = plan
            .threatened_city
            .or(plan.target_city)
            .and_then(|cid| g.cities.get(&cid).map(|city| city.pos));
        if domain == ForceDomain::Land {
            return planned
                .or_else(|| {
                    if self.battlefront_observation {
                        g.units
                            .values()
                            .filter(|unit| enemies.contains(&unit.owner))
                            .filter(|unit| {
                                g.sees(&visible, unit.pos)
                                    && self.battlefront_unit_visible(g, pid, unit.id)
                            })
                            .min_by_key(|unit| (g.wdist(anchor, unit.pos), unit.id))
                            .map(|unit| unit.pos)
                    } else {
                        self.base.nearest_enemy_from(g, pid, anchor, enemies)
                    }
                })
                .unwrap_or(anchor);
        }

        // Fleets interdict hostile ships first. Against a land objective they
        // share the campaign but receive a reachable coastal approach tile.
        if let Some(pos) = g
            .units
            .values()
            .filter(|unit| {
                enemies.contains(&unit.owner)
                    && (!self.battlefront_observation
                        || (g.sees(&visible, unit.pos)
                            && self.battlefront_unit_visible(g, pid, unit.id)))
                    && BasicAi::waterborne(g, unit.id)
            })
            .min_by_key(|unit| (g.wdist(anchor, unit.pos), unit.id))
            .map(|unit| unit.pos)
        {
            return pos;
        }
        // During colonization, a fleet without an immediate contact screens
        // the embarked settler. Once the civilian is linked, its naval leader
        // will carry the pair all the way to the persistent colony objective.
        if let Some(pos) = g
            .units
            .values()
            .filter(|unit| {
                unit.owner == pid
                    && unit.kind == "settler"
                    && g.map
                        .get(unit.pos)
                        .is_some_and(|tile| g.rules.is_water(tile))
            })
            .min_by_key(|unit| (g.wdist(anchor, unit.pos), unit.id))
            .map(|unit| unit.pos)
        {
            return pos;
        }

        let coastal_campaign_city = planned
            .filter(|pos| {
                g.city_at(*pos)
                    .is_some_and(|cid| BasicAi::city_is_coastal(g, cid))
            })
            .or_else(|| {
                g.cities
                    .values()
                    .filter(|city| {
                        enemies.contains(&city.owner)
                            && (!self.battlefront_observation || g.sees(&visible, city.pos))
                            && BasicAi::city_is_coastal(g, city.id)
                    })
                    .min_by_key(|city| (g.wdist(anchor, city.pos), city.id))
                    .map(|city| city.pos)
            });
        coastal_campaign_city
            .and_then(|city_pos| {
                let approach = |radius| {
                    g.wdisk(city_pos, radius)
                        .into_iter()
                        .filter(|pos| {
                            g.map.get(*pos).is_some_and(|tile| {
                                g.rules.is_water(tile)
                                    && g.rules.is_passable(tile)
                                    && (tile.terrain != "ocean"
                                        || g.players[pid]
                                            .techs
                                            .contains(&crate::name!("cartography")))
                            })
                        })
                        .min_by_key(|pos| (g.wdist(anchor, *pos), *pos))
                };
                // Adjacent water lets melee ships capture after ranged ships
                // remove defenses. Radius three is only a fallback for cities
                // behind a narrow land/coast configuration.
                approach(1).or_else(|| approach(3))
            })
            .unwrap_or(anchor)
    }

    pub(super) fn force_focus_target(
        &self,
        g: &Game,
        pid: usize,
        units: &[u32],
        enemies: &[usize],
        plan: &StrategicPlan,
    ) -> Option<Pos> {
        let visible = self.battlefront_visibility(g, pid);
        // `relief_column_marches`: with a city under threat, only a
        // combatant or a city can be the force's focus. See
        // `rebuild_force_groups`.
        let combatants_only = self.relief_column_marches && plan.threatened_city.is_some();
        let is_contact = |other: u32| {
            enemies.contains(&g.units[&other].owner)
                && (!combatants_only || g.rules.units[g.units[&other].kind].class == "military")
        };
        let mut targets = BTreeSet::new();
        for uid in units {
            let unit = &g.units[uid];
            let spec = &g.rules.units[unit.kind];
            if spec.class != "military" || (!spec.is_melee_capable() && !spec.has_ranged_attack()) {
                continue;
            }
            let radius = if spec.has_ranged_attack() {
                g.unit_attack_range(*uid).max(1)
            } else {
                1
            };
            for pos in g.wdisk(unit.pos, radius) {
                let visible_enemy = if self.battlefront_observation {
                    g.sees(&visible, pos)
                        && (g
                            .city_at(pos)
                            .is_some_and(|city| enemies.contains(&g.cities[&city].owner))
                            || g.unit_ids_at(pos).iter().any(|other| {
                                is_contact(*other) && self.battlefront_unit_visible(g, pid, *other)
                            }))
                } else if combatants_only {
                    g.city_at(pos)
                        .is_some_and(|city| enemies.contains(&g.cities[&city].owner))
                        || g.units_at(pos).into_iter().any(is_contact)
                } else {
                    self.base.is_enemy_tile(g, pos, enemies)
                };
                if pos != unit.pos && visible_enemy {
                    targets.insert(pos);
                }
            }
        }
        targets.into_iter().max_by(|a, b| {
            let value = |target: Pos| -> f64 {
                let mut score = 0.0;
                let mut attackers = 0;
                for uid in units {
                    let unit = &g.units[uid];
                    let spec = &g.rules.units[unit.kind];
                    if spec.class != "military"
                        || (!spec.is_melee_capable() && !spec.has_ranged_attack())
                    {
                        continue;
                    }
                    let distance = g.wdist(unit.pos, target);
                    let mut exchange = f64::NEG_INFINITY;
                    if spec.has_ranged_attack() && distance <= g.unit_attack_range(*uid) {
                        exchange = exchange.max(self.base.exchange_score(g, *uid, target, true));
                    }
                    if spec.is_melee_capable() && distance == 1 {
                        exchange = exchange.max(self.base.exchange_score(g, *uid, target, false));
                    }
                    if exchange.is_finite() {
                        score += exchange.max(-20.0);
                        attackers += 1;
                    }
                }
                score += attackers as f64 * 8.0;
                if plan
                    .target_city
                    .is_some_and(|cid| g.cities.get(&cid).is_some_and(|city| city.pos == target))
                {
                    score += 35.0;
                }
                if let Some(hp) = g
                    .unit_ids_at(target)
                    .iter()
                    .filter_map(|uid| {
                        (enemies.contains(&g.units[uid].owner)
                            && (!self.battlefront_observation
                                || self.battlefront_unit_visible(g, pid, *uid)))
                        .then_some(g.units[uid].hp)
                    })
                    .min()
                {
                    score += (100 - hp) as f64 * 0.4;
                }
                score
            };
            value(*a)
                .partial_cmp(&value(*b))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.cmp(a))
        })
    }

    pub(super) fn rebuild_force_groups(&mut self, g: &Game, pid: usize, plan: &StrategicPlan) {
        // `objective-board`: the groups are the board's task forces. Every
        // caller of this function reads the same `force_groups` afterwards.
        // See `advanced/objective_board.rs`.
        if self.objective_board {
            self.board_rebuild_force_groups(g, pid, plan);
            return;
        }
        self.force_groups.clear();
        let enemies: Vec<usize> = g
            .players
            .iter()
            .filter(|player| {
                player.id != pid
                    && player.alive
                    && !player.is_barbarian
                    && g.is_at_war(pid, player.id)
            })
            .map(|player| player.id)
            .collect();
        if enemies.is_empty() {
            return;
        }
        // `one_war_at_a_time`: a group is aimed at the front, and at a second
        // front only where it stands at a threatened city of ours. Local
        // strength is still measured against every enemy. See
        // `advanced/one_war.rs`.
        let objective_enemies = self.one_war_objective_enemies(g, plan.threatened_city, &enemies);
        let visible = self.battlefront_visibility(g, pid);

        let mut remaining: BTreeSet<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let spec = &g.rules.units[g.units[uid].kind];
                // Aircraft receive missions from the air-operations evaluator.
                // Counting them as land units makes a thin ground army appear
                // assembled and locally superior even though aircraft cannot
                // occupy its front, screen siege, or capture the objective.
                let field_unit = matches!(spec.class.as_str(), "military" | "support")
                    && spec.domain.as_deref() != Some("air");
                field_unit
                    && !self.builder_guard_reserved(*uid)
                    && !(BasicAi::unit_doctrine(g, *uid) == UnitDoctrine::Recon
                        && self.base.has_exploration_target(g, pid, *uid))
            })
            .collect();
        let command_radius = self.base.w.command_radius.round().max(1.0) as i32;
        while let Some(seed) = remaining.iter().next().copied() {
            remaining.remove(&seed);
            let domain = Self::force_domain(g, seed);
            let mut units = vec![seed];
            loop {
                let additions: Vec<u32> = remaining
                    .iter()
                    .copied()
                    .filter(|candidate| {
                        Self::force_domain(g, *candidate) == domain
                            // `command_radius` is the maximum separation inside
                            // one force, not a graph edge. Using `any` made a
                            // six-tile chain merge a front-line army with every
                            // reinforcement back to the capital; the moving
                            // medoid then ordered the whole column to muster
                            // home indefinitely.
                            && units.iter().all(|member| {
                                g.wdist(g.units[member].pos, g.units[candidate].pos)
                                    <= command_radius
                            })
                    })
                    .collect();
                if additions.is_empty() {
                    break;
                }
                for uid in additions {
                    remaining.remove(&uid);
                    units.push(uid);
                }
            }
            units.sort_unstable();
            let anchor = Self::force_anchor(g, &units);
            let objective = self.domain_objective(g, pid, plan, domain, anchor, &objective_enemies);
            // ⚠ MEASURED AND REJECTED: pinning `focus_target` to the objective
            // city for a rush, on the theory that the first defender met
            // otherwise pulls the column off the capital. It made things
            // worse — blows on cities by turn 60 fell 6.1 to 2.9, first
            // capture slipped turn 65 to 86 — and the city's own ring still
            // never held more than two. A rush that walks past the defenders
            // to stand on the ring is a rush that gets killed on the ring.
            let focus_target = self.force_focus_target(g, pid, &units, &objective_enemies, plan);
            let muster_radius = self.base.w.muster_radius.round().max(1.0) as i32;
            let readiness = units
                .iter()
                .filter(|uid| {
                    g.wdist(g.units[uid].pos, anchor) <= muster_radius
                        && g.units[uid].hp as f64 > self.base.w.withdraw_hp
                })
                .count() as f64
                / units.len().max(1) as f64;
            let local_strength_ratio =
                self.local_strength_ratio(g, pid, &units, &enemies, objective);
            let average_hp = units.iter().map(|uid| g.units[uid].hp).sum::<i32>() as f64
                / units.len().max(1) as f64;
            let forcing_focus = focus_target.is_some_and(|target| {
                let low_hp_unit = g.unit_ids_at(target).iter().any(|unit| {
                    enemies.contains(&g.units[unit].owner)
                        && (!self.battlefront_observation
                            || (g.sees(&visible, target)
                                && self.battlefront_unit_visible(g, pid, *unit)))
                        && g.units[unit].hp <= 35
                });
                let capturable_city = g.city_at(target).is_some_and(|city| {
                    enemies.contains(&g.cities[&city].owner)
                        && (!self.battlefront_observation || g.sees(&visible, target))
                        && g.cities[&city].hp <= 40
                        && g.cities[&city].wall_hp <= 0
                        && units.iter().any(|unit| {
                            g.rules.units[g.units[unit].kind].is_melee_capable()
                                && g.wdist(g.units[unit].pos, target) <= 1
                        })
                });
                low_hp_unit || capturable_city
            });
            let arena = g.is_arena();
            // `relief_column_marches`: how far this group stands from the
            // threatened city, when there is one. Inside
            // `THREAT_RELIEF_RADIUS` it is part of the defence and holds its
            // ring; beyond it, it is a column that has to get there.
            let relief_distance = plan
                .threatened_city
                .and_then(|cid| g.cities.get(&cid))
                .map(|city| g.wdist(anchor, city.pos));
            let relieving = if self.relief_column_marches {
                relief_distance.is_some_and(|distance| distance <= THREAT_RELIEF_RADIUS)
            } else {
                plan.threatened_city.is_some()
            };
            let relief_column = self.relief_column_marches
                && !arena
                && relief_distance.is_some_and(|distance| distance > THREAT_RELIEF_RADIUS);
            // While a city is threatened a column's contact is a combatant:
            // an Apostle or a Builder standing beside the army must not hold
            // it in "Engage" nine hexes from the siege (t160 of the same run,
            // "crossbowman declines apostle" on every frame).
            let combatants_only = self.relief_column_marches && plan.threatened_city.is_some();
            // Recovery is a bounded defensive posture when it was selected
            // solely because the empire is globally outmatched. A fogged
            // enemy city can still make the local ratio read 3.00, but that
            // is not evidence that the whole army should spend its brief
            // recovery window advancing away from home. In the live Rome run
            // at t106, eight units marched on Puteoli under exactly that
            // label; no home city had been named yet, so the later relief
            // force started several turns farther from Cumae than it needed
            // to be.
            //
            // A named threatened city keeps the established relief route,
            // and a finishable unit or city remains an Engage posture. The
            // direct tactical scan also runs before this movement posture,
            // so holding the front does not decline an ordinary nearby trade.
            // `bounded_recovery` is false in both the frozen and stock
            // production controllers, preserving their source-contract path;
            // the live bridge gets only the temporary global-outmatch case.
            let global_recovery_holds_front = !arena
                && self.bounded_recovery
                && plan.strategy == GrandStrategy::Recovery
                && plan.threatened_city.is_none();
            // A Tactics arena has none of the three reasons a Civ army stands
            // still, so it does none of the three standing-still postures.
            //
            // `Recover` assumes time heals: nothing heals on an arena, so a
            // battered force only gets more battered by waiting. `Hold`
            // assumes something better is coming: no reinforcement is coming,
            // because there is no city to build one in. And the superiority
            // gate below assumes an army is an investment with a homeland
            // behind it worth preserving — but on a battlefield the army *is*
            // the game, and a force that declines every even fight simply
            // loses the field to the clock. Measured with the gate on, two
            // even armies ground each other down to five apiece and then
            // circled for three hundred turns without either closing, so
            // "last army standing" was never reached. This is deliberately
            // the whole of the difference: everything below is the shipped
            // Civ doctrine, unchanged, and an arena only stops asking to be
            // let out of the fight.
            let locally_superior = arena || local_strength_ratio >= LOCAL_SUPERIORITY_FLOOR;
            // ⚠ MEASURED AND REJECTED: letting a rush ignore `relieving` and
            // `Muster` — on the theory that a stack sized against one
            // undefended capital should never stand still — made it *worse*.
            // Over the same 12 maps, captures fell 9/12 to 6/12 and the median
            // first capture slipped from turn 79 to 96. The two standing-still
            // postures are load-bearing even for a rush; do not retry this
            // without a different mechanism.
            let posture = if !arena && average_hp <= self.base.w.withdraw_hp + 10.0 {
                ForcePosture::Recover
            } else if global_recovery_holds_front && !forcing_focus {
                ForcePosture::Hold
            } else if (focus_target.is_some()
                && (locally_superior || plan.threatened_city.is_some() || forcing_focus))
                || (units.iter().any(|uid| {
                    g.units.values().any(|enemy| {
                        enemies.contains(&enemy.owner)
                            && (!self.battlefront_observation
                                || (g.sees(&visible, enemy.pos)
                                    && self.battlefront_unit_visible(g, pid, enemy.id)))
                            && g.wdist(g.units[uid].pos, enemy.pos) <= 2
                            && (!combatants_only || g.rules.units[enemy.kind].class == "military")
                            && (locally_superior
                                || plan.threatened_city.is_some()
                                || enemy.hp <= 35)
                    })
                }))
            {
                ForcePosture::Engage
            } else if relief_column && locally_superior {
                // `relief_column_marches`: a column strong enough to break
                // the siege marches on it now — it does not wait to muster,
                // because the city cannot wait for its stragglers.
                ForcePosture::Advance
            } else if !arena && (relieving || local_strength_ratio < LOCAL_SUPERIORITY_FLOOR) {
                ForcePosture::Hold
            } else if !arena && units.len() > 1 && readiness + 1e-9 < self.base.w.muster_readiness {
                ForcePosture::Muster
            } else {
                ForcePosture::Advance
            };
            self.force_groups.push(ForceGroup {
                id: units[0],
                domain,
                units,
                anchor,
                objective,
                focus_target,
                posture,
                readiness,
                local_strength_ratio,
            });
        }
        self.force_groups.sort_by_key(|group| group.id);
        // What the armies have been told to do. This is the layer between the
        // grand strategy and the individual attacks below it, and it is the
        // one an observer cannot infer from watching units move: a group that
        // holds and a group that has nowhere to go look identical on the map.
        if self.journal().wants(crate::reasoning::Level::Decision) {
            for group in &self.force_groups {
                let held = match group.posture {
                    ForcePosture::Hold if group.local_strength_ratio < LOCAL_SUPERIORITY_FLOOR => {
                        " — too weak locally to advance"
                    }
                    ForcePosture::Hold => " — held back to cover a threat",
                    ForcePosture::Muster => " — still gathering",
                    _ => "",
                };
                think!(self.journal(), Military, Decision,
                       "A {} force of {} will {}",
                       group.domain.as_str(), group.units.len(), group.posture.as_str();
                       "objective {:?}, {:.0}% ready, {:.2} local strength against the \
                        enemy there{held}",
                       group.objective, group.readiness * 100.0, group.local_strength_ratio;
                       group.objective);
            }
        }
        for group in &self.force_groups {
            self.census.count_posture(group.posture);
            if group.posture == ForcePosture::Hold {
                // Attribute the disjunct that actually held the group, not
                // whichever flag happened to be set. A group below the
                // superiority floor holds on its own account whether or not
                // a city is threatened, so counting it as threat-held
                // overstated the threat term: measured against the shipped
                // census it read 61% where the causal share was 34%.
                if group.local_strength_ratio < LOCAL_SUPERIORITY_FLOOR {
                    self.census.hold_weak += 1;
                } else {
                    self.census.hold_threatened += 1;
                }
            }
        }
    }

    pub(super) fn coordinated_tactical_step(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        group: &ForceGroup,
        enemies: &[usize],
        decline_settlers: bool,
    ) -> bool {
        // The force order is committed against the turn-start frame.  Its
        // one-ply mover must use that same evidence: otherwise a hidden
        // hostile's current strength or position can still reroute a unit
        // after the force planner correctly declined to see it.
        // Avoid building a visibility frame for the frozen legacy control;
        // its deliberate full-state behavior remains bit-for-bit on its
        // original path.
        // ⭐⭐ Get out of the water before marching anywhere.
        //
        // This is the mover the DEPLOYED controller actually uses. `BasicAi`'s
        // `tactical_step` is the native/frozen path and never runs here — a
        // probe on live run `civvis-20260803T130831Z` at t174 showed it is not
        // called once, while this function moved the whole army. A fix landed
        // only there would have been inert in exactly the game that motivated
        // it, the same way #955's defence layer was.
        //
        // The `score` below is objective progress, cohesion, threat, spacing,
        // screen and local superiority — with NO terrain term. Open water is
        // therefore doubly attractive: it is usually the geometrically shorter
        // road to an objective across a bay, AND it carries no threat penalty
        // because the enemies are standing on land. An embarked land unit
        // cannot attack (`Game::apply` refuses it), cannot fortify, and defends
        // at the era's flat `embarked_strength`.
        if self.base.come_ashore
            && g.rules.units[g.units[&uid].kind].domain.as_deref() != Some("sea")
            && g.is_embarked(&g.units[&uid])
            && !self.amphibious_member(uid)
            && self.base.disembark_step(g, pid, uid)
        {
            return true;
        }
        let visible = self
            .battlefront_observation
            .then(|| self.battlefront_visibility(g, pid));
        // The score closes over this snapshot while the selected move below
        // may mutate the board. Holding a clone instead of a borrow keeps the
        // decision's turn-start unit frame explicit and lets a route escape
        // apply before the ordinary local fallback is considered.
        let unit = g.units[&uid].clone();
        let upos = unit.pos;
        let prefer_dry =
            self.base.come_ashore && g.rules.units[unit.kind].domain.as_deref() != Some("sea");
        let role = Self::force_role(g, uid);
        let spec = &g.rules.units[unit.kind];
        let target = match group.posture {
            ForcePosture::Hold if self.relief_column_marches => {
                self.relief_hold_point(g, group).unwrap_or(group.anchor)
            }
            ForcePosture::Muster | ForcePosture::Hold | ForcePosture::Recover => group.anchor,
            ForcePosture::Engage => group.focus_target.unwrap_or(group.objective),
            ForcePosture::Advance => group.objective,
        };
        let preferred_depth = match role {
            ForceRole::Recon => spec.range.max(2),
            ForceRole::Vanguard | ForceRole::Mobile => 1,
            ForceRole::Ranged | ForceRole::Siege => g.unit_attack_range(uid),
            ForceRole::Support => 2,
            ForceRole::AirStrike => spec.range.max(3),
        };
        let vanguard_depth = group
            .units
            .iter()
            .filter(|other| {
                **other != uid
                    && g.units.contains_key(other)
                    && matches!(
                        Self::force_role(g, **other),
                        ForceRole::Vanguard | ForceRole::Mobile
                    )
            })
            .map(|other| g.wdist(g.units[other].pos, target))
            .min();
        // The production memory path needs the same fog-safe hostile frame
        // and records only the danger this controller was allowed to see at
        // turn start.
        let danger_hostiles: Vec<u32> = if self.base.unit_objective_memory {
            g.units
                .values()
                .filter(|other| {
                    enemies.contains(&other.owner)
                        && g.unit_visible_to(other.id, pid)
                        && visible.as_ref().is_none_or(|visible| {
                            g.sees(visible, other.pos)
                                && self.battlefront_unit_visible(g, pid, other.id)
                        })
                })
                .map(|other| other.id)
                .collect()
        } else {
            Vec::new()
        };
        // How an arena prices the two halves of an advance differently, and
        // why it has to.
        //
        // The shipped weights charge `mv_threat * caution * expected damage`
        // for standing where the enemy can reach — up to about 15 — and pay
        // `objective_progress * progress` — about 3 — for each hex closer to
        // the objective. In a Civ world that is right: a city does not move,
        // an army that waits one more turn is an army with one more unit in
        // it, and there is always something else the empire is doing
        // meanwhile. On a battlefield all three are false, and the ratio is
        // fatal, because both sides' threat ranges cover the same ground: a
        // force that will not stand inside the enemy's reach and a force that
        // will not be stood over are the same force, and neither ever moves.
        // Measured on an even 10x10: both armies stopped exactly five hexes
        // apart — a horseman's four moves plus its attack — and held there
        // for three hundred turns while both sides' own orders read
        // "Advance".
        //
        // So an arena closes harder and flinches less. Only these two terms
        // move: everything that decides WHICH tile to close on — cover,
        // cohesion, role spacing, screening, the strike opening, the blind
        // ranged tile — is the shipped weighting, and so is every decision
        // about whether to actually attack once contact is made.
        const ARENA_ADVANCE_URGENCY: f64 = 3.0;
        const ARENA_THREAT_CAUTION: f64 = 0.35;
        let arena = g.is_arena();
        // `close-as-a-body` and `screen-the-shooters` read the same hostile
        // frame the threat term below reads. Nothing here runs with both
        // genes off; see `advanced/close_as_a_body.rs`.
        let field_enemies: Vec<Pos> = if self.close_as_a_body || self.screen_the_shooters {
            g.units
                .values()
                .filter(|other| {
                    let enemy_spec = &g.rules.units[other.kind];
                    enemies.contains(&other.owner)
                        && enemy_spec.class == "military"
                        && (enemy_spec.is_melee_capable() || enemy_spec.has_ranged_attack())
                        && visible.as_ref().is_none_or(|visible| {
                            g.sees(visible, other.pos)
                                && self.battlefront_unit_visible(g, pid, other.id)
                        })
                })
                .map(|other| other.pos)
                .collect()
        } else {
            Vec::new()
        };
        let body_pace = self.body_pace(g, uid, group, target, &field_enemies);
        let screen_frame = self.screen_frame(g, uid, group, &field_enemies);
        let movement_risk = (!self.base.legacy_movement).then(|| {
            let mut risk =
                self.base
                    .movement_risk_frame_for_group(g, pid, uid, target, Some(&group.units));
            risk.retain_enemies(|enemy| {
                visible.as_ref().is_none_or(|frame| {
                    g.sees(frame, g.units[&enemy].pos)
                        && self.battlefront_unit_visible(g, pid, enemy)
                })
            });
            risk
        });
        let score = |g: &Game, tile: Pos| -> f64 {
            let objective_distance = g.wdist(tile, target);
            let (progress, cohesion, threat_caution, spacing) = match role {
                ForceRole::Recon => (0.55, 0.40, 1.35, 1.25),
                ForceRole::Vanguard => (1.15, 1.00, 1.00, 1.00),
                ForceRole::Mobile => (1.40, 0.65, 0.80, 1.00),
                ForceRole::Ranged => (0.90, 1.10, 1.15, 1.50),
                ForceRole::Siege => (0.80, 1.30, 1.25, 1.70),
                ForceRole::Support => (0.65, 1.50, 1.40, 1.20),
                ForceRole::AirStrike => (1.20, 0.20, 0.75, 0.50),
            };
            let (progress, threat_caution) = if arena {
                (
                    progress * ARENA_ADVANCE_URGENCY,
                    threat_caution * ARENA_THREAT_CAUTION,
                )
            } else {
                (progress, threat_caution)
            };
            let mut value = -self.base.w.objective_progress * progress * objective_distance as f64;
            if let Some(pace) = &body_pace {
                value -= self.body_pace_penalty(g, tile, target, pace, progress);
            }
            let nearest_friend = group
                .units
                .iter()
                .filter(|other| **other != uid && g.units.contains_key(other))
                .map(|other| g.wdist(tile, g.units[other].pos))
                .min();
            if let Some(distance) = nearest_friend {
                value -= self.base.w.cohesion * cohesion * (distance - 2).max(0) as f64;
                if distance == 1 {
                    value += self.base.w.mv_support;
                }
            }
            if let Some(risk) = &movement_risk {
                value += risk.score(g, pid, uid, tile, self.base.w.mv_threat * threat_caution);
            } else {
                // `defend-where-you-stand`: priced once for this tile, and only
                // if something actually reaches it. `None` with the gene off,
                // and then the defence below is the snapshot's, as before. See
                // `advanced/engine_pricing.rs`.
                let mut standing_defense: Option<f64> = None;
                for enemy in g.units.values().filter(|other| {
                    enemies.contains(&other.owner)
                        && visible.as_ref().is_none_or(|visible| {
                            g.sees(visible, other.pos)
                                && self.battlefront_unit_visible(g, pid, other.id)
                        })
                }) {
                    let enemy_spec = &g.rules.units[enemy.kind];
                    if enemy_spec.class != "military"
                        || (!enemy_spec.is_melee_capable() && !enemy_spec.has_ranged_attack())
                    {
                        continue;
                    }
                    let radius = if enemy_spec.has_ranged_attack() {
                        g.unit_attack_range(enemy.id).max(1)
                    } else {
                        1
                    };
                    if g.wdist(tile, enemy.pos) <= radius {
                        let attack = crate::game::effective_strength(
                            g.unit_strength(enemy, false),
                            enemy.hp,
                        );
                        let defense = *standing_defense.get_or_insert_with(|| {
                            let base = self
                                .base
                                .defence_base_where_it_would_stand(g, uid, tile)
                                .unwrap_or_else(|| g.unit_strength(&unit, true));
                            crate::game::effective_strength(base, unit.hp)
                        });
                        value -= self.base.w.mv_threat
                            * threat_caution
                            * 30.0
                            * ((attack - defense) / 25.0).exp();
                    }
                }
            }
            // `risk-is-priced`: the chance the unit dies here times its
            // price, in this score's units. Zero with the gene off. See
            // `advanced/priced_risk.rs`.
            let per_hp = self.base.w.mv_threat * threat_caution;
            value -= self.base.priced_risk_cost(g, uid, tile, per_hp);

            // Walking into the sea costs most of the unit; the score has no
            // other term that notices. Sized to outweigh the few tiles of
            // objective progress a detour around a bay gives up, without
            // removing the option — a real crossing still scores best when no
            // land route exists.
            if prefer_dry && g.map.get(tile).is_some_and(|t| g.rules.is_water(t)) {
                value -= crate::ai::WATER_MARCH_PENALTY;
            }
            value += self.strike_opening_value(StrikeOpening {
                g,
                pid,
                uid,
                tile,
                group,
                enemies,
                visible: visible.as_deref(),
            });
            if g.wdist(tile, target) <= 5 {
                value -= self.base.w.role_spacing
                    * spacing
                    * (g.wdist(tile, target) - preferred_depth).abs() as f64;
                if matches!(
                    role,
                    ForceRole::Recon | ForceRole::Ranged | ForceRole::Siege | ForceRole::AirStrike
                ) {
                    if let Some(front_depth) = vanguard_depth {
                        value -= self.base.w.screen
                            * (front_depth - g.wdist(tile, target)).max(0) as f64;
                    }
                }
            }
            if let Some(frame) = &screen_frame {
                value += self.screen_bonus(g, tile, frame);
            }
            // `early-conquest-opening`: a strike-force body does not end its
            // move beside ground it cannot see unless a friendly stands with
            // it. 234 of 304 live unit deaths carried `no_visible_threat`.
            // Zero with the gene off and for every unit outside the force.
            // See `advanced/early_conquest.rs`.
            if let Some(frame) = visible.as_ref() {
                value -= self.conquest_blind_tile_penalty(g, pid, uid, tile, frame);
            }
            // The posture selector's superiority gate is already arena-off
            // (an army that declines every even fight loses the field to the
            // clock), but this second gate — a per-tile brake on any closing
            // move while locally weaker — was still on, and it is the
            // standoff: approaching the enemy mass lowers the local ratio, so
            // every advancing group brakes exactly at the edge of enemy
            // reach, on both sides at once. Measured on a 20x20 no-city
            // arena: armies parked three to six hexes apart with zero units
            // in contact for 40-62% of the battle, grinding out the clock
            // through accidental skirmishes.
            if !arena && group.local_strength_ratio < 1.0 {
                let advance = g.wdist(upos, target) - objective_distance;
                value -= self.base.w.local_superiority
                    * (1.0 - group.local_strength_ratio)
                    * advance.max(0) as f64;
            }
            value + self.base.livelock_penalty(uid, tile)
        };

        let stay = score(g, upos);
        let holding_role_position = g.wdist(upos, target) == preferred_depth;
        let stop_range = if matches!(
            role,
            ForceRole::Recon | ForceRole::Ranged | ForceRole::Siege | ForceRole::AirStrike
        ) {
            preferred_depth
        } else {
            1
        };

        // The ordinary one-ply scorer keeps its historical priority on every
        // healthy unit. A unit that has spent a whole six-turn window inside
        // two or three tiles is different evidence: its locally best moves
        // have already failed as a route. Let A* nominate the next detour
        // step before greedy scoring repeats that proof. The router can need
        // to cross one of the recorded tiles to leave the pocket, so this
        // path is paired with BasicAi's narrowly scoped retread exception.
        //
        // A route is not a license to walk into a kill zone. Its score may be
        // at most the same eight points that already price escaping a
        // livelock; a larger tactical loss still leaves the unit holding.
        let route_escape = if matches!(group.posture, ForcePosture::Advance | ForcePosture::Engage)
            && self.base.live_livelock_route_escape(uid)
        {
            g.route_step(uid, target, stop_range)
                .filter(|pos| g.can_move(uid, *pos))
                // `risk-is-priced`: never with the gene off.
                .filter(|pos| !self.base.priced_risk_refuses(g, uid, upos, *pos))
                .filter(|pos| {
                    !(decline_settlers
                        && g.unit_ids_at(*pos).iter().any(|other| {
                            let other = &g.units[other];
                            other.owner != pid
                                && g.is_at_war(pid, other.owner)
                                && other.kind == "settler"
                        }))
                })
        } else {
            None
        };
        if let Some(pos) = route_escape {
            let routed = score(g, pos) + crate::ai::LIVELOCK_ESCAPE_VALUE;
            if self.base.move_beats_holding(g, uid, routed, stay) {
                if self.base.unit_objective_memory {
                    // `group-steps-read-the-field`: see `group_step_danger`.
                    let danger = self.group_step_danger(
                        g,
                        pid,
                        uid,
                        pos,
                        self.base
                            .projected_counter_damage(g, uid, pos, &danger_hostiles),
                    );
                    if self.base.remember_dangerous_approach(g, uid, pos, danger) {
                        return self
                            .base
                            .retreat_step(g, pid, uid)
                            .unwrap_or_else(|| self.base.fortify_or_stop(g, pid, uid));
                    }
                }
                if self
                    .base
                    .tactical_apply_livelock_route_escape(g, pid, uid, pos)
                {
                    return true;
                }
            }
        }
        let mut best: Option<(f64, Pos)> = None;
        // `risk-is-priced`: the best tile its refusal took away, for the
        // journal. `None` with the gene off.
        let mut refused: Option<(f64, Pos, super::priced_risk::Refusal)> = None;
        for pos in g.nbrs(upos).into_iter().filter(|pos| {
            g.can_move(uid, *pos)
                && !(decline_settlers
                    && g.unit_ids_at(*pos).iter().any(|other| {
                        let other = &g.units[other];
                        other.owner != pid
                            && g.is_at_war(pid, other.owner)
                            && other.kind == "settler"
                    }))
        }) {
            let candidate = score(g, pos);
            if let Some(refusal) = self.base.priced_risk_refusal(g, uid, upos, pos) {
                if refused.as_ref().is_none_or(|(old, old_pos, _)| {
                    candidate > *old || (candidate == *old && pos < *old_pos)
                }) {
                    refused = Some((candidate, pos, refusal));
                }
                continue;
            }
            if best
                .map(|(old, old_pos)| candidate > old || (candidate == old && pos < old_pos))
                .unwrap_or(true)
            {
                best = Some((candidate, pos));
            }
        }
        if let Some((candidate, pos, refusal)) = refused {
            if best.is_none_or(|(chosen, _)| candidate > chosen) {
                self.base.note_priced_risk_refusal(g, uid, pos, &refusal);
            }
        }
        if let Some((candidate, pos)) = best {
            // Not on an arena: the danger memory exists to walk a world
            // army around a kill zone it can come back to with more force,
            // and its retreat floor is `withdraw_hp` — but nothing heals on
            // an arena and no more force is coming, so after the first
            // exchange half of both armies sits near the floor and every
            // approach reads as a reason to turn around. Measured: the
            // armies touched, traded, separated, and spent 40-62% of the
            // battle with zero units in contact.
            if !arena && self.base.unit_objective_memory {
                let advancing = g.wdist(pos, target) < g.wdist(upos, target);
                let danger = self.group_step_danger(
                    g,
                    pid,
                    uid,
                    pos,
                    self.base
                        .projected_counter_damage(g, uid, pos, &danger_hostiles),
                );
                if advancing && self.base.remember_dangerous_approach(g, uid, pos, danger) {
                    return self
                        .base
                        .retreat_step(g, pid, uid)
                        .unwrap_or_else(|| self.base.fortify_or_stop(g, pid, uid));
                }
            }
            let should_move = if holding_role_position {
                candidate > stay + 1e-9
            } else {
                self.base.move_beats_holding(g, uid, candidate, stay)
            };
            if should_move {
                // ⚠ Through `path_move`, never `g.apply` directly: it records
                // the step, and a unit stepped again this turn must not
                // re-enter the tile it just left. This site emitted 50 of the
                // 88 same-turn out-and-back pairs on the replay of run
                // civvis-20260801T224944Z — net zero ground, two orders, and
                // the second refused by Civilization VI as a MOVE_TO of the
                // unit's own tile.
                return self.base.tactical_apply_move(g, pid, uid, pos);
            }
        }

        if g.wdist(upos, target) > stop_range {
            if let Some(pos) = g
                .route_step(uid, target, stop_range)
                .filter(|pos| g.can_move(uid, *pos))
                // `risk-is-priced`: never with the gene off.
                .filter(|pos| !self.base.priced_risk_refuses(g, uid, upos, *pos))
                .filter(|pos| {
                    !(decline_settlers
                        && g.unit_ids_at(*pos).iter().any(|other| {
                            let other = &g.units[other];
                            other.owner != pid
                                && g.is_at_war(pid, other.owner)
                                && other.kind == "settler"
                        }))
                })
            {
                if self.base.move_beats_holding(g, uid, score(g, pos), stay) {
                    if self.base.unit_objective_memory {
                        let danger = self.group_step_danger(
                            g,
                            pid,
                            uid,
                            pos,
                            self.base
                                .projected_counter_damage(g, uid, pos, &danger_hostiles),
                        );
                        if self.base.remember_dangerous_approach(g, uid, pos, danger) {
                            return self
                                .base
                                .retreat_step(g, pid, uid)
                                .unwrap_or_else(|| self.base.fortify_or_stop(g, pid, uid));
                        }
                    }
                    // Same rule as the local arm above: recorded through
                    // `path_move` so the next same-turn step cannot undo it.
                    return self.base.tactical_apply_move(g, pid, uid, pos);
                }
            }
        }
        self.base.fortify_or_stop(g, pid, uid)
    }

    /// Evaluate infrastructure bombing on an exact cloned position. Besides
    /// the pillaged layer, this prices interception losses and the operational
    /// disruption from scattering aircraft out of a disabled air base.
    pub(super) fn air_pillage_value(&self, g: &Game, pid: usize, uid: u32, target: Pos) -> f64 {
        let attacker = &g.units[&uid];
        let attacker_spec = &g.rules.units[attacker.kind];
        let before_tile = &g.map.tiles[&target];
        let before_aircraft: Vec<(u32, f64)> = g
            .unit_ids_at(target)
            .iter()
            .filter_map(|unit| {
                let candidate = &g.units[unit];
                (candidate.owner != pid
                    && g.rules.units[candidate.kind].domain.as_deref() == Some("air"))
                .then_some((*unit, g.rules.units[candidate.kind].cost))
            })
            .collect();
        let city_id = before_tile.owner_city;
        let before_pillaged_buildings = city_id
            .and_then(|city| g.cities.get(&city))
            .map(|city| city.pillaged_buildings.clone())
            .unwrap_or_default();
        let action = Action::AirPillage { unit: uid, target };
        let mut after = g.speculative_clone();
        if after.apply(pid, &action).is_err() {
            return f64::NEG_INFINITY;
        }

        let attacker_loss = match after.units.get(&uid) {
            Some(survivor) => {
                (attacker.hp - survivor.hp).max(0) as f64 * (1.4 + attacker_spec.cost / 700.0)
            }
            None => 260.0 + attacker_spec.cost * 0.7,
        };
        let mut value = -attacker_loss;
        let after_tile = &after.map.tiles[&target];
        if let Some(improvement) = before_tile.improvement.as_deref() {
            if !before_tile.pillaged && after_tile.pillaged {
                value += match improvement {
                    "airstrip" => 185.0,
                    "oil_well" | "offshore_oil_rig" | "mine" | "quarry" => 115.0,
                    "farm" | "fishing_boats" => 65.0,
                    _ => 85.0,
                };
            }
        } else if let Some(district) = before_tile.district {
            if !before_tile.pillaged && after_tile.pillaged {
                value += match g.district_family(district).as_str() {
                    "aerodrome" | "industrial_zone" | "campus" | "spaceport" => 175.0,
                    "commercial_hub" | "harbor" | "holy_site" | "theater_square" => 145.0,
                    _ => 115.0,
                };
            } else if let Some(city) = city_id.and_then(|city| after.cities.get(&city)) {
                value += city
                    .pillaged_buildings
                    .difference(&before_pillaged_buildings)
                    .map(|building| 80.0 + after.rules.buildings[building].cost * 0.32)
                    .sum::<f64>();
            }
        }
        for (aircraft, cost) in before_aircraft {
            value += match after.units.get(&aircraft) {
                None => 150.0 + cost * 0.55,
                Some(unit) if unit.pos != target => 55.0 + cost * 0.08,
                _ => 0.0,
            };
        }
        value
    }

    pub(super) fn priority_target_value(&self, g: &Game, pid: usize, uid: u32, target: Pos) -> f64 {
        let Some(defender_id) = g.priority_support_target_at(pid, target) else {
            return f64::NEG_INFINITY;
        };
        let attacker = &g.units[&uid];
        let attacker_spec = &g.rules.units[attacker.kind];
        let defender = &g.units[&defender_id];
        let defender_spec = &g.rules.units[defender.kind];
        let mut after = g.speculative_clone();
        if after
            .apply(pid, &Action::PriorityTarget { unit: uid, target })
            .is_err()
        {
            return f64::NEG_INFINITY;
        }
        let attacker_loss = match after.units.get(&uid) {
            Some(survivor) => {
                (attacker.hp - survivor.hp).max(0) as f64 * (1.4 + attacker_spec.cost / 700.0)
            }
            None => 260.0 + attacker_spec.cost * 0.7,
        };
        let target_value = match after.units.get(&defender_id) {
            None => 175.0 + defender_spec.cost * 0.55,
            Some(survivor) => {
                (defender.hp - survivor.hp).max(0) as f64 * (1.0 + defender_spec.cost / 500.0)
            }
        };
        target_value
            + if defender_spec.anti_air_strength > 0.0 {
                120.0
            } else if matches!(defender.kind.as_str(), "drone" | "observation_balloon") {
                55.0
            } else if matches!(defender.kind.as_str(), "medic" | "supply_convoy") {
                40.0
            } else {
                0.0
            }
            - attacker_loss
    }

    /// Choose among exact air-strike or air-pillage results, a useful patrol,
    /// and a rebase
    /// that materially improves reach to the active front. Fighters preserve
    /// interception coverage when hostile aircraft threaten the theater;
    /// bombers avoid suicidal missions and reposition when no profitable
    /// strike is available.
    pub(super) fn advanced_air_action(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
    ) -> Option<Action> {
        if let Some(action) = self.culture_air_priority_action(g, pid, uid, plan) {
            return Some(action);
        }
        let unit = &g.units[&uid];
        let doctrine = BasicAi::unit_doctrine(g, uid);
        let legal = g.legal_doctrine_actions(pid, uid);
        let best_strike = legal
            .iter()
            .filter_map(|action| match action {
                Action::AirStrike { unit, target } if *unit == uid => Some((
                    self.air_strike_value(g, pid, uid, *target, plan),
                    *target,
                    action.clone(),
                )),
                _ => None,
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.cmp(&left.1))
            });
        let best_pillage = legal
            .iter()
            .filter_map(|action| match action {
                Action::AirPillage { unit, target } if *unit == uid => Some((
                    self.air_pillage_value(g, pid, uid, *target),
                    *target,
                    action.clone(),
                )),
                _ => None,
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.cmp(&left.1))
            });
        let best_priority = legal
            .iter()
            .filter_map(|action| match action {
                Action::PriorityTarget { unit, target } if *unit == uid => Some((
                    self.priority_target_value(g, pid, uid, *target),
                    *target,
                    action.clone(),
                )),
                _ => None,
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.cmp(&left.1))
            });
        let best_attack = best_strike
            .clone()
            .into_iter()
            .chain(best_priority.clone())
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.cmp(&left.1))
            });
        let best_mission =
            best_attack
                .clone()
                .into_iter()
                .chain(best_pillage)
                .max_by(|left, right| {
                    left.0
                        .total_cmp(&right.0)
                        .then_with(|| right.1.cmp(&left.1))
                });

        let objective = match doctrine {
            UnitDoctrine::AirDefense => plan.threatened_city.or(plan.target_city),
            _ => plan.target_city.or(plan.threatened_city),
        }
        .and_then(|city| g.cities.get(&city).map(|city| city.pos))
        .or_else(|| {
            g.units
                .values()
                .filter(|other| other.owner != pid && g.is_at_war(pid, other.owner))
                .min_by_key(|other| (g.wdist(unit.pos, other.pos), other.id))
                .map(|other| other.pos)
        });
        let best_rebase = objective.and_then(|objective| {
            let current_distance = g.wdist(unit.pos, objective);
            legal
                .iter()
                .filter_map(|action| match action {
                    Action::AirRebase { unit, to }
                        if *unit == uid && self.air_base_rebase_allowed(g, pid, uid, *to) =>
                    {
                        let distance = g.wdist(*to, objective);
                        let improvement = current_distance - distance;
                        let range = g.unit_attack_range(uid);
                        // Award new reach, rather than paying again for an
                        // objective already in range. Keep the frozen anchor.
                        let reaches = (distance <= range
                            && (self.base.legacy_movement || current_distance > range))
                            as i32;
                        Some((
                            improvement as f64 * 18.0 + reaches as f64 * 35.0,
                            *to,
                            action.clone(),
                        ))
                    }
                    _ => None,
                })
                .filter(|(value, _, _)| *value > 0.0)
                .max_by(|left, right| {
                    left.0
                        .total_cmp(&right.0)
                        .then_with(|| right.1.cmp(&left.1))
                })
        });

        if doctrine == UnitDoctrine::AirStrike {
            // `bombers-open-the-siege-walls`: see `siege_train::siege_wall_sortie`.
            if let Some(action) = self.siege_wall_sortie(g, pid, uid, &legal, plan) {
                return Some(action);
            }
            if let (Some((mission_value, _, _)), Some((_, to, action))) =
                (best_mission.as_ref(), best_rebase.as_ref())
            {
                if *mission_value > 0.0
                    && self.air_campaign_rebase_is_better(g, pid, uid, plan, *to, *mission_value)
                {
                    return Some(action.clone());
                }
            }
            return best_mission
                .filter(|(value, _, _)| *value > 0.0)
                .map(|(_, _, action)| action)
                .or_else(|| best_rebase.map(|(_, _, action)| action));
        }

        let patrol = legal
            .iter()
            .filter_map(|action| match action {
                Action::AirPatrol {
                    unit: action_unit,
                    to,
                } if *action_unit == uid => {
                    let city_cover = g
                        .cities
                        .values()
                        .filter(|city| city.owner == pid && g.wdist(*to, city.pos) <= 1)
                        .map(|city| {
                            70.0 + city.pop as f64 * 4.0
                                + if Some(city.id) == plan.threatened_city {
                                    90.0
                                } else {
                                    0.0
                                }
                        })
                        .sum::<f64>();
                    let force_cover = g
                        .units
                        .values()
                        .filter(|other| {
                            other.owner == pid
                                && other.id != uid
                                && g.wdist(*to, other.pos) <= 1
                                && g.rules.units[other.kind].class == "military"
                        })
                        .map(|other| g.rules.units[other.kind].cost * 0.035)
                        .sum::<f64>();
                    let objective_distance = objective.map_or(0, |pos| g.wdist(*to, pos));
                    let existing = (unit.air_patrol_pos == Some(*to)) as i32 as f64 * 8.0;
                    Some((
                        city_cover + force_cover + existing - objective_distance as f64 * 2.0,
                        *to,
                        action.clone(),
                    ))
                }
                _ => None,
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.cmp(&left.1))
            })
            .map(|(_, _, action)| action);
        let hostile_air_threat = g
            .units
            .values()
            .filter(|other| {
                other.owner != pid
                    && g.is_at_war(pid, other.owner)
                    && g.rules.units[other.kind].domain.as_deref() == Some("air")
            })
            .map(|other| {
                let distance = g.wdist(unit.pos, other.pos);
                if distance <= g.air_rebase_range(uid) {
                    80.0 + g.rules.units[other.kind].cost * 0.08
                } else {
                    0.0
                }
            })
            .fold(0.0_f64, f64::max);
        let defended_city = plan.threatened_city.is_some_and(|city| {
            g.cities.get(&city).is_some_and(|city| {
                legal.iter().any(|action| {
                    matches!(action, Action::AirPatrol { unit, to }
                        if *unit == uid && g.wdist(*to, city.pos) <= 1)
                })
            })
        });
        let patrol_value = hostile_air_threat
            + if defended_city { 55.0 } else { 0.0 }
            + if g
                .players
                .iter()
                .any(|other| other.id != pid && g.is_at_war(pid, other.id))
            {
                16.0
            } else {
                5.0
            };
        if let Some((value, _, action)) = best_attack {
            if value > patrol_value {
                return Some(action);
            }
        }
        if let Some((value, _, action)) = best_rebase {
            if value > patrol_value && hostile_air_threat <= 0.0 {
                return Some(action);
            }
        }
        patrol
    }
}
