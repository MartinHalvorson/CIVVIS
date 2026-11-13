//! The unit turn of the baseline controller: how `BasicAi` moves each unit.
//!
//! `military_step` runs every unit's turn. It takes killing blows and
//! captures (`killing_blow`, `capture_adjacent_civilian`,
//! `pursue_capturable_civilian`), retreats and heals (`retreat_step`,
//! `healing_step`, `step_out_of_reach_to_heal`), and garrisons and answers
//! barbarians (`garrison_assignments`, `barbarian_response_step`). Combat
//! moves go through `tactical_step`. Peaceful units go through
//! `peacetime_step` (exploration with `exploration_goal`/`explore_step`,
//! village collection, patrols, disembarking, modernization and the special
//! improver).
//!
//! Moved verbatim out of the main `impl BasicAi` block in `ai.rs`
//! (2026-11-05). Methods the rest of the controller calls are `pub(super)`;
//! the code is otherwise unchanged.

use super::*;

impl BasicAi {
    /// A remembered capture objective is actionable only while the city still
    /// belongs to a hostile civilization. It can be retained before a formal
    /// declaration, but never pulls a unit through a peace treaty.
    pub(crate) fn capture_objective_target(&self, g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        if !self.unit_objective_memory {
            return None;
        }
        let objective = self.unit_memories.borrow().get(&uid)?.objective?;
        let UnitObjective::CaptureCity { city, target, .. } = objective;
        g.cities
            .get(&city)
            .filter(|known| {
                known.owner != pid && known.pos == target && g.is_at_war(pid, known.owner)
            })
            .map(|_| target)
    }

    /// Remember that entering `position` would expose this unit to a serious
    /// counterattack. The warning always survives briefly; it becomes a real
    /// retreat when that damage would cross the controller's own withdrawal
    /// floor.
    pub(crate) fn remember_dangerous_approach(
        &self,
        g: &Game,
        uid: u32,
        position: Pos,
        expected_damage: f64,
    ) -> bool {
        if !self.unit_objective_memory || !expected_damage.is_finite() || expected_damage <= 0.0 {
            return false;
        }
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        let expected_damage = expected_damage.ceil().clamp(0.0, u32::MAX as f64) as u32;
        let withdraw_floor = self.w.withdraw_hp.round();
        let requires_retreat =
            expected_damage as f64 >= (f64::from(unit.hp) - withdraw_floor).max(0.0);
        let mut memories = self.unit_memories.borrow_mut();
        let memory = memories.entry(uid).or_default();
        memory.danger = Some(UnitDangerMemory {
            position,
            expected_damage,
            observed_turn: g.turn,
            expires_turn: g.turn.saturating_add(UNIT_DANGER_MEMORY_TURNS),
        });
        if requires_retreat {
            let until = g.turn.saturating_add(UNIT_RETREAT_TURNS);
            memory.retreat_until = Some(memory.retreat_until.unwrap_or(until).max(until));
        }
        requires_retreat
    }

    /// The largest single blow anything the controller can see would land on
    /// `uid` at `position` next turn, at the top of the engine's damage roll
    /// and under the engine's own ceiling on one blow. A unit whose hit points
    /// do not exceed this is one attack from being removed from the board.
    pub(super) fn killing_blow(
        g: &Game,
        pid: usize,
        uid: u32,
        position: Pos,
        envelopes: &[(u32, std::sync::Arc<EnvelopeReach>)],
    ) -> f64 {
        (Self::incoming_damage(g, pid, uid, position, envelopes).worst * COMBAT_ROLL_MAX)
            .min(MAX_SINGLE_BLOW)
    }

    fn evacuation_tile(
        g: &Game,
        pid: usize,
        uid: u32,
        position: Pos,
        danger: Option<Pos>,
        envelopes: &[(u32, std::sync::Arc<EnvelopeReach>)],
    ) -> Option<EvacuationTile> {
        g.map.get(position)?;
        let city = g.city_at(position).is_some_and(|city| {
            let owner = g.cities[&city].owner;
            owner == pid || g.suzerain_of(owner) == Some(pid)
        });
        Some(EvacuationTile {
            position,
            incoming: Self::evacuation_incoming_damage(g, pid, uid, position, envelopes),
            healing: g.healing_location(pid, position).rate(),
            city,
            distance_from_danger: danger.map_or(0, |danger| g.wdist(position, danger)),
        })
    }

    /// Order recovery ground by safety before all the softer preferences.
    /// When two tiles are equally safe, better healing, a City Center, and
    /// greater distance from the recorded danger break the tie in that order.
    pub(super) fn evacuation_tile_cmp(left: &EvacuationTile, right: &EvacuationTile) -> Ordering {
        // An exponential estimate can underflow to signed zero. Normalize the
        // entire negligible band so a `-0.0` cannot beat an equally safe City
        // Center before healing gets to break the tie.
        let incoming = |tile: &EvacuationTile| {
            if tile.incoming.abs() > 1e-9 {
                tile.incoming
            } else {
                0.0
            }
        };
        let left_incoming = incoming(left);
        let right_incoming = incoming(right);
        (left_incoming == 0.0)
            .cmp(&(right_incoming == 0.0))
            .then_with(|| right_incoming.total_cmp(&left_incoming))
            .then(left.healing.cmp(&right.healing))
            .then(left.city.cmp(&right.city))
            .then(left.distance_from_danger.cmp(&right.distance_from_danger))
            .then_with(|| right.position.cmp(&left.position))
    }

    fn evacuation_tile_is_better(candidate: EvacuationTile, holding: EvacuationTile) -> bool {
        Self::evacuation_tile_cmp(&candidate, &holding).is_gt()
    }

    /// First legal step toward healing ground that no observed enemy can attack
    /// next turn. City Centers win before ordinary friendly ground; neutral
    /// ground is a last resort when no safe homeward route exists.
    /// Every tile some hostile envelope covers, unioned once per envelope
    /// table instead of once per recovering unit.
    ///
    /// ★★★★ THE KEY IS THE `Arc` ITSELF, AND THAT IS SOUND RATHER THAN
    /// CONVENIENT. `enemy_attack_envelopes` builds a fresh `Arc` whenever the
    /// envelopes change and hands the same one back on a hit, so pointer
    /// identity says "the same table"; and the entry holds that `Arc`, so the
    /// allocation cannot be freed and reused underneath the comparison while
    /// the cache is alive. The union is the identical sorted set
    /// `safe_healing_step` used to rebuild — every recovering unit of one
    /// seat-turn shares one board and therefore one union.
    pub(super) fn covered_tiles(
        &self,
        envelopes: &std::sync::Arc<AttackEnvelopes>,
    ) -> std::sync::Arc<Vec<Pos>> {
        let mut slot = self
            .covered_tiles_cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some((table, covered)) = slot.as_ref() {
            if std::sync::Arc::ptr_eq(table, envelopes) {
                return std::sync::Arc::clone(covered);
            }
        }
        let mut tiles: Vec<Pos> =
            Vec::with_capacity(envelopes.iter().map(|(_, reach)| reach.len()).sum());
        for (_, reach) in envelopes.iter() {
            tiles.extend_from_slice(reach.as_slice());
        }
        if tiles.windows(2).any(|pair| pair[0] >= pair[1]) {
            tiles.sort_unstable();
            tiles.dedup();
        }
        let covered = std::sync::Arc::new(tiles);
        *slot = Some((
            std::sync::Arc::clone(envelopes),
            std::sync::Arc::clone(&covered),
        ));
        covered
    }

    /// Every hostile City Center and Encampment that could bombard something
    /// this turn, as the tiles they strike from. The district half of
    /// [`Self::anything_can_reach`], hoisted so a whole-map scan pays for the
    /// city walk once rather than once per tile.
    fn district_strike_sources(g: &Game, pid: usize) -> Vec<Pos> {
        g.cities
            .values()
            .filter(|city| city.owner != pid && g.is_at_war(pid, city.owner))
            .flat_map(|city| {
                let centre = (city.wall_hp > 0).then_some(city.pos);
                let districts = g
                    .defending_districts(city)
                    .filter(|state| state.hp > 0 && state.wall_hp > 0 && !state.pillaged)
                    .map(|state| state.pos);
                centre.into_iter().chain(districts)
            })
            .collect()
    }

    /// Every route target [`Self::safe_healing_step`] would select before it
    /// asks the moving unit for a path.
    ///
    /// ★★★★ THE KEY COVERS EVERY READ, NOT JUST THE OBVIOUS MAP. The exact
    /// hostile-envelope `Arc` protects coverage, while
    /// `Game::healing_target_stamp` changes for every map, city, player, or
    /// diplomacy input used below. The cache entry retains that `Arc`, so
    /// pointer identity cannot be recycled under a live entry. This is one
    /// immutable full-map scan per shared frontier board rather than one per
    /// recovering unit.
    pub(super) fn safe_healing_targets(
        &self,
        g: &Game,
        pid: usize,
        envelopes: &std::sync::Arc<AttackEnvelopes>,
    ) -> std::sync::Arc<SafeHealingTargets> {
        let stamp = g.healing_target_stamp();
        let slot = self.safe_healing_targets_cache.lock();
        if let Some(cached) = slot.as_ref() {
            if cached.pid == pid
                && cached.stamp == stamp
                && std::sync::Arc::ptr_eq(&cached.envelopes, envelopes)
            {
                return std::sync::Arc::clone(&cached.targets);
            }
        }
        drop(slot);

        let covered = self.covered_tiles(envelopes);
        let strike_sources = Self::district_strike_sources(g, pid);
        let struck = |position: Pos| {
            strike_sources.iter().any(|source| {
                g.wdist(*source, position) <= 2 && g.line_of_sight_from(*source, position)
            })
        };
        let mut cities = HashSet::new();
        let mut friendly_tiles = HashSet::new();
        let mut neutral_tiles = HashSet::new();
        for position in g.map.tiles.keys().copied() {
            let Some(tile) = g.map.get(position) else {
                continue;
            };
            let friendly_owner = tile
                .owner_city
                .and_then(|city| g.cities.get(&city))
                .is_some_and(|city| city.owner == pid || g.suzerain_of(city.owner) == Some(pid));
            let healing = g.healing_location(pid, position).rate();
            if (friendly_owner && healing < 15) || (!friendly_owner && healing != 10) {
                continue;
            }
            let city_here = g.city_at(position);
            let garrisoned = city_here.is_some() || g.defending_district_at(position).is_some();
            if !garrisoned && (covered.binary_search(&position).is_ok() || struck(position)) {
                continue;
            }
            let city = city_here.is_some_and(|city| {
                let owner = g.cities[&city].owner;
                owner == pid || g.suzerain_of(owner) == Some(pid)
            });
            if city {
                cities.insert(position);
            } else if friendly_owner {
                friendly_tiles.insert(position);
            } else {
                neutral_tiles.insert(position);
            }
        }
        let targets = std::sync::Arc::new(SafeHealingTargets {
            cities,
            friendly_tiles,
            neutral_tiles,
        });
        let mut slot = self.safe_healing_targets_cache.lock();
        *slot = Some(SafeHealingTargetCache {
            pid,
            stamp,
            envelopes: std::sync::Arc::clone(envelopes),
            targets: std::sync::Arc::clone(&targets),
        });
        targets
    }

    pub(super) fn safe_healing_step(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        envelopes: &std::sync::Arc<AttackEnvelopes>,
    ) -> Option<Pos> {
        // ★★★★ THIS WALKS THE WHOLE MAP, ONCE PER RECOVERING UNIT PER TURN,
        // and it used to price every tile with `evacuation_tile` — a defender
        // strength, every covering enemy's attack strength, and a scan of
        // every hostile city with `is_at_war` per city — 3,404 times per unit
        // on a 74×46 map. That was half the simulator after #2059. The scan
        // only keeps a tile whose incoming damage is exactly zero, and
        // `evacuation_incoming_damage` is zero precisely when the tile is a
        // garrison district or lies outside every enemy envelope AND out of
        // strike range of every hostile walled city and encampment (each
        // covering source contributes at least the clamp's floor of one).
        // So the safety test is a set lookup, computed once, and the tile
        // loop pays only for its heal-rate read — under a query-memo scope, so
        // `suzerain_of` is answered once per minor, not once per tile.
        let _memo = g.query_memo();
        let targets = self.safe_healing_targets(g, pid, envelopes);
        g.route_step_to_any(uid, &targets.cities)
            .or_else(|| g.route_step_to_any(uid, &targets.friendly_tiles))
            .or_else(|| g.route_step_to_any(uid, &targets.neutral_tiles))
            .filter(|next| g.can_move(uid, *next))
    }

    /// An emergency withdrawal should not preempt a legal attack that leaves
    /// this unit alive outside the remaining enemy envelopes. This is a small
    /// exact forward check, not a combat-score guess: it lets a unit finish a
    /// kill or create a safe trade when that is genuinely better than fleeing.
    /// ``reserve`` is `ranged_hp_reserve`'s answer for this unit: a ranged unit
    /// has not "survived" a trade that leaves it standing but below the hit
    /// points it keeps in hand — that is the near-fatal hit the gene exists to
    /// refuse. Zero for everything else, so nothing else changes.
    pub(super) fn can_survive_by_attacking(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        reserve: i32,
    ) -> bool {
        g.legal_actions_within(pid, ActionFamilies::UNITS)
            .into_iter()
            .filter(|action| {
                matches!(
                    action,
                    Action::Attack { unit, .. }
                        | Action::Ranged { unit, .. }
                        | Action::PriorityTarget { unit, .. }
                        if *unit == uid
                )
            })
            .any(|action| {
                let mut future = g.clone();
                if future.apply(pid, &action).is_err() {
                    return false;
                }
                let Some(survivor) = future.units.get(&uid) else {
                    return false;
                };
                let envelopes = self.enemy_attack_envelopes(&future, pid);
                Self::evacuation_incoming_damage(&future, pid, uid, survivor.pos, &envelopes)
                    + f64::from(reserve)
                    < f64::from(survivor.hp)
            })
    }

    /// Spend as much of this turn's legal path as possible reaching chosen
    /// recovery ground. `Game::reachable` and `Game::path_to` decide the same
    /// movement, so this can take a wounded unit all the way into a nearby
    /// city rather than stopping one tile short for no tactical reason.
    pub(crate) fn move_to_evacuation_tile(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> bool {
        let Some(path) = g.path_to(uid, target) else {
            return false;
        };
        // ★★★ A WOUNDED UNIT LEAVES CONTACT THROUGH ITS OWN LINE, which is a
        // crossing and not a sequence of steps: `MoveTo` is the only action
        // that may execute one (see `Game::entry_at`), and stepping it a
        // `Move` at a time stops the casualty dead on the tile its relief is
        // standing on. This is the front-line rotation the old rule made
        // impossible, so it is the first thing to check here.
        if path.iter().any(|step| !g.can_stop(uid, *step)) {
            return self.path_walk_to(g, pid, uid, target);
        }
        let mut moved = false;
        for step in path {
            if !g.can_move(uid, step) || !self.tactical_apply_move(g, pid, uid, step) {
                break;
            }
            moved = true;
            if g.units.get(&uid).is_none_or(|unit| unit.moves_left <= 0.0) {
                break;
            }
        }
        moved
    }

    /// Honor a remembered retreat, or create one immediately when the enemy's
    /// exact next-turn attacks are likely to kill this unit. A retreat selects
    /// reachable recovery ground outside those envelopes whenever possible;
    /// ordinary recovery then holds there until the unit is healthy enough to
    /// resume its campaign objective.
    pub(crate) fn retreat_step(&self, g: &mut Game, pid: usize, uid: u32) -> Option<bool> {
        if !self.precise_evacuation {
            return None;
        }
        if g.is_arena() {
            return None;
        }
        let unit = g.units.get(&uid)?;
        let spec = &g.rules.units[unit.kind];
        if unit.owner != pid
            || unit.moves_left <= 0.0
            || spec.class != "military"
            || spec.domain.as_deref() == Some("air")
            || (!spec.is_melee_capable() && !spec.has_ranged_attack())
        {
            return None;
        }
        let here = unit.pos;
        let hp = unit.hp;
        let remembered = self.unit_memories.borrow().get(&uid).and_then(|memory| {
            memory
                .retreat_until
                .filter(|until| g.turn < *until)
                .and(memory.danger)
        });
        let envelopes = self.enemy_attack_envelopes(g, pid);
        // ★★★★ MOST UNITS ARE OUT OF EVERY ENVELOPE AND WERE PAYING A FULL
        // EVACUATION PRICE TO FIND OUT. `retreat_step` runs for every military
        // unit of every seat every turn — 87% of that bill is on city-states
        // and barbarians (`docs/SIMULATOR_PERFORMANCE.md`, 2026-08-22) — and
        // on a quiet tile it priced a heal rate, a City Center's ownership and
        // a suzerain, only to read an incoming pool of `0.0`, conclude "not
        // lethal" and return. `anything_can_reach` is the same source test
        // `incoming_damage` performs anyway, asked before the rest is paid
        // for.
        //
        // ⚠ NOT THE ENVELOPE UNION, AND THAT WAS MEASURED. A first version
        // answered this from `covered_tiles`, one binary search instead of one
        // per envelope — and building an 800-tile union to answer a single
        // membership question cost `sample` 1.8% of the main thread against
        // the 1.2% the skip saves. The union earns its keep across
        // `safe_healing_step`'s whole-map scan and nowhere else.
        //
        // This skips work whose answer was zero, it does not change an answer:
        // incoming `0.0` is below any live unit's hit points, so `lethal` was
        // false and with no remembered danger the tile below returns `None` —
        // which is exactly what an unmapped tile's `?` returns too.
        if remembered.is_none() && hp > 0 && !Self::anything_can_reach(g, pid, here, &envelopes) {
            return None;
        }
        let holding = Self::evacuation_tile(
            g,
            pid,
            uid,
            here,
            remembered.map(|d| d.position),
            &envelopes,
        )?;
        // `veterans-withdraw-early`: the pool is lethal to a veteran once it
        // would leave less than the margin the unit keeps in hand.
        // `ranged-hp-reserve` adds to the same margin: an archer reads the pool
        // as lethal while it would still be left standing, and leaves.
        let margin = self.veteran_hp_margin(g, uid) + self.ranged_hp_reserve(g, uid);
        let lethal =
            holding.incoming > 0.0 && holding.incoming + f64::from(margin) >= f64::from(hp);
        if remembered.is_none() && !lethal {
            return None;
        }
        if lethal && self.can_survive_by_attacking(g, pid, uid, self.ranged_hp_reserve(g, uid)) {
            return None;
        }

        if lethal {
            let expected_damage = holding.incoming.ceil().clamp(1.0, 100.0) as u32;
            let until = g.turn.saturating_add(UNIT_RETREAT_TURNS);
            let mut memories = self.unit_memories.borrow_mut();
            let memory = memories.entry(uid).or_default();
            memory.danger = Some(UnitDangerMemory {
                position: remembered.map_or(here, |danger| danger.position),
                expected_damage,
                observed_turn: g.turn,
                expires_turn: g.turn.saturating_add(UNIT_DANGER_MEMORY_TURNS),
            });
            memory.retreat_until = Some(memory.retreat_until.unwrap_or(until).max(until));
        }

        let danger = self
            .unit_memories
            .borrow()
            .get(&uid)
            .and_then(|memory| memory.danger)
            .map(|danger| danger.position);
        let holding = Self::evacuation_tile(g, pid, uid, here, danger, &envelopes)?;
        let mut candidates: Vec<EvacuationTile> = g
            .reachable(uid)
            .into_iter()
            .filter_map(|position| Self::evacuation_tile(g, pid, uid, position, danger, &envelopes))
            .collect();
        if let Some(step) = self.safe_healing_step(g, pid, uid, &envelopes) {
            if let Some(candidate) = Self::evacuation_tile(g, pid, uid, step, danger, &envelopes) {
                candidates.push(candidate);
            }
        }
        let next = candidates.into_iter().max_by(Self::evacuation_tile_cmp);
        match next {
            Some(next)
                if Self::evacuation_tile_is_better(next, holding)
                    || (lethal && next.incoming <= holding.incoming + 1e-9) =>
            {
                Some(
                    self.move_to_evacuation_tile(g, pid, uid, next.position)
                        || self.fortify_or_stop(g, pid, uid),
                )
            }
            _ => Some(self.fortify_or_stop(g, pid, uid)),
        }
    }

    /// What a candidate tile is worth against the fact that this unit has been
    /// going in circles. Nothing at all for a unit that is getting somewhere —
    /// which is almost every unit, almost always — and for one that is not, a
    /// flat charge for every tile of the footprint it keeps re-entering,
    /// including the one it is standing on. Any tile outside the loop is
    /// thereby worth `LIVELOCK_ESCAPE_VALUE` more than any tile inside it.
    pub(crate) fn livelock_penalty(&self, uid: u32, tile: Pos) -> f64 {
        match self.unit_motion.get(&uid) {
            Some(motion) if motion.looping && motion.tiles.contains(&tile) => {
                -LIVELOCK_ESCAPE_VALUE
            }
            _ => 0.0,
        }
    }

    /// Hit points a promoted unit keeps in hand before it withdraws, or 0
    /// while `veteran_retreat_margin` is off. See that field.
    pub(crate) fn veteran_hp_margin(&self, g: &Game, uid: u32) -> i32 {
        if !self.veteran_retreat_margin {
            return 0;
        }
        let promotions = g.units.get(&uid).map_or(0, |unit| unit.promotions.len()) as i32;
        (promotions * VETERAN_HP_PER_PROMOTION).min(VETERAN_HP_MARGIN_CAP)
    }

    /// `ranged-hp-reserve`: `RANGED_HP_RESERVE` for a ranged unit that is not a
    /// siege piece and not aircraft, else 0. Zero while the gene is off.
    pub(crate) fn ranged_hp_reserve(&self, g: &Game, uid: u32) -> i32 {
        if !self.ranged_hp_reserve {
            return 0;
        }
        let Some(unit) = g.units.get(&uid) else {
            return 0;
        };
        let Some(spec) = g.rules.units.get(unit.kind.as_str()) else {
            return 0;
        };
        if spec.has_ranged_attack() && !spec.siege && spec.domain.as_deref() != Some("air") {
            RANGED_HP_RESERVE
        } else {
            0
        }
    }

    /// 1-ply positional search for wartime marching: score each candidate
    /// tile (stay put or any legal neighbor) by progress toward the target,
    /// adjacent friendly support, and expected incoming damage; take the best.
    pub(super) fn tactical_step(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        target: Pos,
        enemy_ids: &[usize],
        attack_range: i32,
    ) -> bool {
        // Get out of the water before marching anywhere. The penalty in
        // `score` below stops a unit WALKING INTO the sea, but it cannot
        // recover one already out there: every neighbour of a tile in open
        // water is also water, so a uniform penalty cancels and the unit keeps
        // swimming toward the objective. This is the half that reaches the
        // eleven-of-twelve afloat on run `civvis-20260803T130831Z`.
        //
        // It sits in `tactical_step` rather than `military_step` because the
        // Advanced campaign path calls `tactical_step` directly and would
        // otherwise miss the rule entirely — the same "reachable only on one
        // path" mistake that left the #955 defence layer inert.
        if self.come_ashore
            && g.rules.units[g.units[&uid].kind].domain.as_deref() != Some("sea")
            && g.is_embarked(&g.units[&uid])
            && self.disembark_step(g, pid, uid)
        {
            return true;
        }
        let upos = g.units[&uid].pos;
        let u = &g.units[&uid];
        let prefer_dry = self.come_ashore && g.rules.units[u.kind].domain.as_deref() != Some("sea");
        let doctrine = Self::unit_doctrine(g, uid);
        let (preferred_range, progress, threat_caution) = match doctrine {
            UnitDoctrine::Recon => (2, 0.60, 1.35),
            UnitDoctrine::Assault => (1, 1.15, 1.00),
            UnitDoctrine::Mobile => (1, 1.40, 0.80),
            UnitDoctrine::Ranged => (attack_range.max(1), 0.90, 1.15),
            UnitDoctrine::Siege => (attack_range.max(1), 0.80, 1.25),
            UnitDoctrine::Support | UnitDoctrine::Carrier => (2, 0.65, 1.40),
            UnitDoctrine::AirDefense | UnitDoctrine::AirStrike => (attack_range.max(1), 1.0, 1.0),
        };
        // A memory entry is an observation, not privileged map knowledge.
        // Only the durable warning uses enemies this civilization can actually
        // see now.
        let danger_hostiles: Vec<u32> = if self.unit_objective_memory {
            g.units
                .values()
                .filter(|enemy| {
                    enemy_ids.contains(&enemy.owner) && g.unit_visible_to(enemy.id, pid)
                })
                .map(|enemy| enemy.id)
                .collect()
        } else {
            Vec::new()
        };
        let movement_risk =
            (!self.legacy_movement).then(|| self.movement_risk_frame(g, pid, uid, target));
        let score = |g: &Game, tile: Pos| -> f64 {
            let depth_error = (g.wdist(tile, target) - preferred_range).abs();
            let mut s = -3.0 * progress * depth_error as f64;
            let mut adjacent_support = 0;
            for n in g.nbrs(tile) {
                for oid in g.unit_ids_at(n) {
                    let o = &g.units[oid];
                    if g.rules.units[o.kind].class != "military" {
                        continue;
                    }
                    if o.owner == pid && *oid != uid {
                        adjacent_support += 1;
                    } else if self.legacy_movement && enemy_ids.contains(&o.owner) {
                        let attack = effective_strength(g.unit_strength(o, false), o.hp);
                        let defense = effective_strength(g.unit_strength(u, true), u.hp);
                        s -= self.w.mv_threat
                            * threat_caution
                            * 30.0
                            * ((attack - defense) / 25.0).exp();
                    }
                }
            }
            // A pair of neighbors is enough to hold a coherent line. Giving
            // every extra adjacent unit the full bonus makes dense armies
            // refuse to leave their initial cluster even when a safe campaign
            // route is open.
            s += self.w.mv_support * adjacent_support.min(2) as f64;
            if let Some(risk) = &movement_risk {
                s += risk.score(g, pid, uid, tile, self.w.mv_threat * threat_caution);
            }
            // ⭐ The score above has no terrain term at all, and open water is
            // doubly attractive because of it: a sea tile is usually the
            // geometrically shorter road to an objective across a bay, AND it
            // carries no adjacent-enemy threat because the enemies are on
            // land. So the safest, fastest route was always the sea — and an
            // embarked land unit cannot attack (`Game::apply` refuses it),
            // cannot fortify, and defends at the era's flat
            // `embarked_strength` instead of its own.
            //
            // This is the wartime mover, and it is the one that mattered: on
            // live run `civvis-20260803T130831Z` at t174, eleven of twelve land
            // combat units were afloat while the capital sat at 179/200 damage,
            // and the journal read "A land force of 12 will advance | objective
            // (22,21)" — an objective on our OWN landmass. They were not
            // crossing to reach it; they were swimming along it.
            //
            // Sized to outweigh the few tiles of `depth_error` a detour around
            // a bay costs, without removing the option: `route_step` below is
            // untouched, so a real crossing still happens when the land route
            // does not exist.
            if prefer_dry && g.map.get(tile).is_some_and(|t| g.rules.is_water(t)) {
                s -= WATER_MARCH_PENALTY;
            }
            // `risk-is-priced`: the chance the unit dies here times its
            // price, in this score's units. Zero with the gene off. See
            // `advanced/priced_risk.rs`.
            s -= self.priced_risk_cost(g, uid, tile, self.w.mv_threat * threat_caution);
            s + self.livelock_penalty(uid, tile)
        };
        let stay = score(g, upos);
        let holding_role_position = g.wdist(upos, target) == preferred_range;
        let within_minor_front = |position: Pos| {
            !self.minor
                || (self.barb && self.barbarian_tactics)
                || Self::minor_home(g, pid)
                    .is_some_and(|home| g.wdist(home, position) <= MINOR_DEFENSE_RADIUS)
        };
        let mut best: Option<(f64, Pos)> = None;
        // `risk-is-priced`: the best tile its refusal took away, for the
        // journal. `None` with the gene off.
        let mut refused: Option<(f64, Pos, advanced::priced_risk::Refusal)> = None;
        for n in g.nbrs(upos) {
            if !within_minor_front(n) || !g.can_move(uid, n) {
                continue;
            }
            let sc = score(g, n);
            if let Some(refusal) = self.priced_risk_refusal(g, uid, upos, n) {
                if refused
                    .as_ref()
                    .is_none_or(|(b, bp, _)| (sc, n) > (*b, *bp))
                {
                    refused = Some((sc, n, refusal));
                }
                continue;
            }
            if best.map(|(b, bp)| (sc, n) > (b, bp)).unwrap_or(true) {
                best = Some((sc, n));
            }
        }
        if let Some((sc, n, refusal)) = refused {
            if best.is_none_or(|(b, _)| sc > b) {
                self.note_priced_risk_refusal(g, uid, n, &refusal);
            }
        }
        match best {
            Some((sc, n))
                if if holding_role_position {
                    sc > stay + 1e-9
                } else {
                    self.move_beats_holding(g, uid, sc, stay)
                } =>
            {
                if self.unit_objective_memory {
                    let danger = self.projected_counter_damage(g, uid, n, &danger_hostiles);
                    if self.remember_dangerous_approach(g, uid, n, danger) {
                        return self
                            .retreat_step(g, pid, uid)
                            .unwrap_or_else(|| self.fortify_or_stop(g, pid, uid));
                    }
                }
                // ⚠ Through `path_move`, never `g.apply` directly: a unit with
                // movement left is stepped again this same turn, and a raw
                // apply records nothing — so round two happily re-entered the
                // tile round one just left. Net zero ground, TWO emitted
                // orders, and on the Civilization VI side the second usually
                // lands as a MOVE_TO of the unit's own tile: 217 of 217
                // refused moves on run civvis-20260801T224944Z were exactly
                // that, out-and-back pairs from this call site.
                self.tactical_apply_move(g, pid, uid, n)
            }
            _ => {
                // Long-range search is the fallback, not the hot path: most
                // turns keep the original cheap local tactic, while a unit at
                // a genuine obstacle can take the first safe detour step.
                let n = match g.route_step(uid, target, preferred_range) {
                    Some(n) if within_minor_front(n) && g.can_move(uid, n) => n,
                    _ => return false,
                };
                // `risk-is-priced`: never with the gene off.
                if self.priced_risk_refuses(g, uid, upos, n) {
                    return false;
                }
                let routed = score(g, n) + 2.5;
                if !self.move_beats_holding(g, uid, routed, stay) {
                    return false;
                }
                if self.unit_objective_memory {
                    let danger = self.projected_counter_damage(g, uid, n, &danger_hostiles);
                    if self.remember_dangerous_approach(g, uid, n, danger) {
                        return self
                            .retreat_step(g, pid, uid)
                            .unwrap_or_else(|| self.fortify_or_stop(g, pid, uid));
                    }
                }
                self.tactical_apply_move(g, pid, uid, n)
            }
        }
    }

    pub(crate) fn move_beats_holding(
        &self,
        g: &Game,
        uid: u32,
        candidate: f64,
        holding: f64,
    ) -> bool {
        let initiative = if g.units[&uid].moved {
            0.0
        } else {
            FIRST_MOVE_SCORE_BONUS
        };
        candidate + initiative > holding + 1e-9
    }

    /// Apply one pathing move while refusing to undo this unit's immediately
    /// preceding path step in the same turn. Waiting in a dead end preserves
    /// real progress for the auditor, and next turn's route can back out once
    /// before choosing a different greedy branch.
    /// Take a tactical step that has already been selected and validated by
    /// the caller, recording it when `recorded_tactical_step` is on.
    ///
    /// The flag is the whole gate. Off — every tournament entrant, every
    /// replayed ladder, and the frozen `advanced_v1` anchor — this is the
    /// historical raw `g.apply(Move)` and nothing about the step changes. On,
    /// which only the Civilization VI bridge does, the step goes through
    /// `path_move` so the same-turn reversal guard can see it. `path_move`
    /// can additionally refuse a step that the raw apply would have taken
    /// (a reversal, a retread, a minor leaving its defense area); that
    /// asymmetry is the point of the fix, and it is exactly why the anchor
    /// must not be exposed to it.
    pub(crate) fn tactical_apply_move(&self, g: &mut Game, pid: usize, uid: u32, to: Pos) -> bool {
        if !self.recorded_tactical_step {
            return g.apply(pid, &Action::Move { unit: uid, to }).is_ok();
        }
        self.path_move(g, pid, uid, to)
    }

    /// Improvements this particular unit can actually spend one of its own
    /// charges to create on `pos`. Builders use `builder_buildable`; the few
    /// military uniques that carry build charges instead name their actions in
    /// their unit rule. Keeping the intersection here makes every caller obey
    /// the same placement, ownership, Open Borders, and civilization checks.
    pub(crate) fn special_unit_improvements(g: &Game, pid: usize, uid: u32, pos: Pos) -> Vec<Name> {
        let Some(unit) = g.units.get(&uid) else {
            return Vec::new();
        };
        if unit.owner != pid || unit.charges <= 0 {
            return Vec::new();
        }
        let builds = &g.rules.units[unit.kind].builds;
        if builds.is_empty() {
            return Vec::new();
        }
        g.valid_improvements(pid, pos)
            .into_iter()
            .filter(|improvement| builds.contains(improvement))
            .collect()
    }

    /// Spend a unique military unit's improvement charge when that is a real
    /// legal job, or walk it toward the nearest reachable job. `None` means
    /// this unit has no such job and lets ordinary military behavior continue.
    ///
    /// This intentionally searches the whole map rather than only owned city
    /// tiles: a Nau's Feitoria is useful precisely because it must be placed in
    /// foreign territory with Open Borders.
    pub(crate) fn special_improver_step(&self, g: &mut Game, pid: usize, uid: u32) -> Option<bool> {
        let current = g.units.get(&uid)?.pos;
        let here = Self::special_unit_improvements(g, pid, uid, current);
        if let Some(improvement) = Self::best_improvement(g, current, &here) {
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

        let targets = {
            // `valid_improvements` asks several empire-wide questions. The
            // special units are rare, but a memo scope still keeps this map
            // scan proportional to tiles rather than repeated rule queries.
            let _memo = g.query_memo();
            g.map
                .tiles
                .keys()
                .copied()
                .filter(|position| {
                    *position != current
                        && !Self::special_unit_improvements(g, pid, uid, *position).is_empty()
                })
                .collect::<HashSet<_>>()
        };
        g.route_step_to_any(uid, &targets).and_then(|to| {
            g.apply(pid, &Action::Move { unit: uid, to })
                .ok()
                .map(|_| true)
        })
    }

    pub(super) fn is_enemy_tile(&self, g: &Game, pos: Pos, enemy_ids: &[usize]) -> bool {
        for oid in g.unit_ids_at(pos) {
            if enemy_ids.contains(&g.units[oid].owner) {
                return true;
            }
        }
        if let Some(cid) = g.city_at(pos) {
            return enemy_ids.contains(&g.cities[&cid].owner);
        }
        false
    }

    /// Assign the bounded barbarian responder to an empty city under local
    /// pressure. `contested-land-first` also uses the deterministic assignment
    /// to price its own frontier garrison, without reviving the removed
    /// major-war treatment.
    pub(super) fn garrison_assignments(
        &self,
        g: &Game,
        pid: usize,
        enemy_ids: &[usize],
    ) -> Vec<(u32, Pos)> {
        let barbarian_response = g
            .barb_pid
            .is_some_and(|barb| enemy_ids.len() == 1 && enemy_ids[0] == barb);
        if (!barbarian_response && !self.contested_land_first) || self.minor || self.barb {
            return Vec::new();
        }
        // A city wants a garrison when something hostile is close enough to
        // reach it and nothing of ours is standing on it. Worst first: the most
        // strength bearing down, then the most damage already taken.
        let mut wanting: Vec<(i64, Pos)> = Vec::new();
        for city in g.cities.values().filter(|city| city.owner == pid) {
            let held = g.unit_ids_at(city.pos).iter().any(|uid| {
                g.units[uid].owner == pid && g.rules.units[g.units[uid].kind].class == "military"
            });
            if held {
                continue;
            }
            let pressure: f64 = g
                .units
                .values()
                .filter(|enemy| enemy_ids.contains(&enemy.owner))
                .filter(|enemy| g.rules.units[enemy.kind].class == "military")
                .filter(|enemy| !Self::is_barbarian_scout(g, enemy.id))
                .filter(|enemy| g.wdist(enemy.pos, city.pos) <= GARRISON_ALERT_RADIUS)
                .filter(|enemy| {
                    Some(enemy.owner) != g.barb_pid
                        || self.barbarian_raider_counts_as_threat(g, pid, enemy)
                })
                .map(|enemy| effective_strength(g.unit_strength(enemy, true), enemy.hp))
                .sum();
            if pressure <= 0.0 {
                // `contested_land_first`: a frontier city holds a garrison in
                // peacetime too — below any live pressure, the city nearest
                // the neighbour first. See `advanced/contested_land.rs`.
                if let Some(distance) = self.contested_frontier_distance(g, pid, city.pos) {
                    let radius = advanced::contested_land::CONTESTED_LAND_FRONTIER_RADIUS;
                    wanting.push((i64::from(radius - distance), city.pos));
                }
                continue;
            }
            wanting.push((pressure as i64 * 1_000 + city.hp.max(0) as i64, city.pos));
        }
        if wanting.is_empty() {
            return Vec::new();
        }
        wanting.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

        let mut responders: Vec<u32> = g
            .units
            .values()
            .filter(|unit| unit.owner == pid)
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            .filter(|unit| {
                matches!(
                    g.rules.units[unit.kind].domain.as_deref(),
                    None | Some("land")
                )
            })
            .filter(|unit| !self.recovering_units.contains(&unit.id))
            .map(|unit| unit.id)
            .collect();
        responders.sort_unstable();
        // Same bound as the field response, for the same reason, and shared
        // with it: between them the two mechanisms never claim more than half.
        let cap =
            ((responders.len() as f64 * BARBARIAN_RESPONSE_MAX_SHARE).floor() as usize).max(1);

        let mut assigned: Vec<(u32, Pos)> = Vec::new();
        for (_, city) in wanting {
            if assigned.len() >= cap {
                break;
            }
            // One unit per city. A second body on the tile adds nothing a city's
            // own defence does not already do; the rest are better in the field.
            let Some(&(_, defender)) = responders
                .iter()
                .filter(|id| !assigned.iter().any(|(taken, _)| taken == *id))
                .map(|id| (g.wdist(g.units[id].pos, city), *id))
                .filter(|(distance, _)| *distance <= BARBARIAN_RESPONSE_RECALL_RANGE)
                .collect::<Vec<_>>()
                .iter()
                .min()
            else {
                continue;
            };
            assigned.push((defender, city));
        }
        assigned
    }

    /// Walk a barbarian responder to its assigned city and hold it. Standing
    /// on the tile IS the job, so arriving means fortifying rather than
    /// looking for a fight.
    pub(crate) fn barbarian_garrison_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        enemy_ids: &[usize],
    ) -> bool {
        GARRISON_STEP_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let Some((_, city)) = self
            .garrison_assignments(g, pid, enemy_ids)
            .into_iter()
            .find(|(defender, _)| *defender == uid)
        else {
            return false;
        };
        if g.units[&uid].pos == city {
            return self.fortify_or_stop(g, pid, uid);
        }
        self.step_toward(g, pid, uid, city) || self.fortify_or_stop(g, pid, uid)
    }

    pub(crate) fn barbarian_presence_at_home_for_controller(
        &self,
        g: &Game,
        pid: usize,
        camp_radius: i32,
    ) -> bool {
        Self::barbarian_presence_at_home_inner(
            g,
            pid,
            camp_radius,
            self.naval_threat_triage,
            self.enemy_of_my_enemy && !self.minor && !self.barb,
        )
    }

    /// A harmless ship is not a strategic alarm, but a legal shot into it is
    /// still a useful three-XP drill for a ranged unit already in position.
    /// This never creates a chase: admission requires a shot legal right now.
    pub(crate) fn has_harmless_naval_xp_shot(&self, g: &Game, pid: usize, uid: u32) -> bool {
        if !self.naval_threat_triage {
            return false;
        }
        let Some(shooter) = g.units.get(&uid).filter(|unit| unit.owner == pid) else {
            return false;
        };
        if !g.rules.units[shooter.kind].has_ranged_attack() {
            return false;
        }
        let frames = (g.player_vision_frame(pid), g.visibility_viewers(pid));
        let range = g.unit_attack_range(uid);
        g.units.values().any(|target| {
            Some(target.owner) == g.barb_pid
                && Self::is_barbarian_raider(g, target)
                && g.rules.units[target.kind].domain.as_deref() == Some("sea")
                && g.wdist(shooter.pos, target.pos) <= range
                && g.ranged_order_is_legal(pid, uid, target.pos, &frames.0, &frames.1)
                && !Self::naval_raider_can_deal_serious_damage(g, pid, target)
        })
    }

    /// A bounded credit makes a one-damage but experience-bearing shot clear
    /// the ordinary strength threshold. Kills and serious targets remain worth
    /// orders of magnitude more in the exact tactical evaluator.
    pub(crate) fn harmless_naval_xp_bonus(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        target: Pos,
        ranged: bool,
    ) -> f64 {
        if !self.naval_threat_triage || !ranged {
            return 0.0;
        }
        let harmless_ship = g.unit_ids_at(target).iter().any(|other| {
            let unit = &g.units[other];
            Some(unit.owner) == g.barb_pid
                && Self::is_barbarian_raider(g, unit)
                && g.rules.units[unit.kind].domain.as_deref() == Some("sea")
                && !Self::naval_raider_can_deal_serious_damage(g, pid, unit)
        });
        if harmless_ship && g.units.get(&uid).is_some_and(|unit| unit.owner == pid) {
            10.0
        } else {
            0.0
        }
    }

    /// Answer a measured barbarian threat to our own cities with the nearest
    /// sufficient units before the offensive claims them.
    ///
    /// Deliberately NOT a gene. The genome is pinned at 40 and a committed
    /// champion rides that exact length, so the constants below ship fixed and
    /// earn a gene later if measurement says they matter.
    pub(crate) fn barbarian_response_objective(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        enemy_ids: &[usize],
    ) -> Option<Pos> {
        // City-states already guard home and barbarians have no homeland to
        // defend. The dedicated response owns exactly the barbarian seat.
        let barb = g.barb_pid?;
        if self.minor || self.barb || enemy_ids.len() != 1 || enemy_ids[0] != barb {
            return None;
        }
        let my_cities: Vec<Pos> = g
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .map(|city| city.pos)
            .collect();
        if my_cities.is_empty() {
            return None;
        }
        let home_distance = |pos: Pos| -> i32 {
            my_cities
                .iter()
                .map(|city| g.wdist(pos, *city))
                .min()
                .unwrap_or(i32::MAX)
        };

        // Threats, worst first. Severity is an integer so the ordering is
        // identical on every platform: proximity to one of our cities dominates,
        // and strength breaks ties within the same ring. A raider standing ON a
        // city is therefore always answered before one six tiles out, however
        // much bigger the distant one is.
        let mut threats: Vec<(i64, Pos, f64)> = Vec::new();
        for enemy in g.units.values() {
            if !enemy_ids.contains(&enemy.owner)
                || g.rules.units[enemy.kind].class != "military"
                || Self::is_barbarian_scout(g, enemy.id)
            {
                continue;
            }
            let distance = home_distance(enemy.pos);
            if distance > HOME_THREAT_RADIUS {
                continue;
            }
            if Some(enemy.owner) == g.barb_pid
                && !self.barbarian_raider_counts_as_threat(g, pid, enemy)
            {
                continue;
            }
            let strength = effective_strength(g.unit_strength(enemy, true), enemy.hp);
            let severity = (HOME_THREAT_RADIUS - distance) as i64 * 1_000 + strength as i64;
            threats.push((severity, enemy.pos, strength));
        }
        // A camp does not fight back, but it keeps producing what does, and the
        // measured raiders all came from one. Rank it just under a live raider at
        // the same range so a unit already in the empire clears it once the
        // shooting stops rather than leaving the tap running.
        // `camp_party` is the peacetime upgrade: a home raider does not make
        // a major war, but an entire army should not be recalled unless that
        // separate treatment is on.
        let peacetime = self.camp_party;
        let camp_radius = HOME_CAMP_RADIUS;
        for camp in g.barb_camps.keys() {
            let distance = home_distance(*camp);
            if distance > camp_radius {
                continue;
            }
            // `enemy_of_my_enemy`: the neighbour's camp is no threat of ours
            // to answer; its raiders are, when they arrive.
            if self.camp_is_a_neighbours_problem(g, pid, *camp) {
                continue;
            }
            if peacetime {
                // ★★★★ THE CAMP IS THE TAP. See `camp_party`: it ranks as a
                // raider three tiles out — below anything at the gates, above
                // the countryside — and the party is sized to whoever is
                // standing on it.
                let ranked = distance.min(3);
                let defender = g
                    .unit_ids_at(*camp)
                    .iter()
                    .filter_map(|other| g.units.get(other))
                    .filter(|other| {
                        enemy_ids.contains(&other.owner)
                            && g.rules.units[other.kind].class == "military"
                            && !Self::is_barbarian_scout(g, other.id)
                    })
                    .map(|other| effective_strength(g.unit_strength(other, true), other.hp))
                    .fold(0.0_f64, f64::max);
                threats.push((
                    (HOME_THREAT_RADIUS - ranked) as i64 * 1_000 - 1,
                    *camp,
                    defender,
                ));
                continue;
            }
            // A camp past the raider radius ranks below every raider inside
            // it (its severity goes negative), so the guard walks out to it
            // only once the home ring is quiet.
            threats.push((
                (HOME_THREAT_RADIUS - distance) as i64 * 1_000 - 1,
                *camp,
                0.0,
            ));
        }
        if threats.is_empty() {
            return None;
        }
        threats.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

        // Land units only: a ship cannot answer a raider inland, and pulling air
        // units out of the offensive buys nothing a fighter on patrol does not
        // already give. A unit already withdrawn to heal stays withdrawn —
        // `healing_step` ran before this and its judgement outranks ours.
        // Units already holding a city tile are spoken for. Excluding them here
        // is what keeps the two mechanisms from tugging the same unit between a
        // city and a field raider on alternate turns.
        let garrisoned = self.garrison_assignments(g, pid, enemy_ids);
        let responders: Vec<u32> = {
            let mut ids: Vec<u32> = g
                .units
                .values()
                .filter(|unit| unit.owner == pid)
                .filter(|unit| g.rules.units[unit.kind].class == "military")
                .filter(|unit| {
                    // The dedicated responder matches a ship to a sea threat
                    // and a land unit to a land threat.
                    matches!(
                        g.rules.units[unit.kind].domain.as_deref(),
                        None | Some("land") | Some("sea")
                    )
                })
                .filter(|unit| !self.recovering_units.contains(&unit.id))
                .filter(|unit| !garrisoned.iter().any(|(held, _)| *held == unit.id))
                // The peacetime party is the field ARMY: a scout is not sent
                // to clear a camp. See `camp_party`.
                .filter(|unit| !(peacetime && g.rules.units[unit.kind].promotion_class == "recon"))
                .map(|unit| unit.id)
                .collect();
            ids.sort_unstable();
            ids
        };
        // Never recall more than half the army. An empire that pulls everything
        // home to chase raiders has not saved itself, it has lost the war a
        // different way — and the measured game's problem was the opposite
        // extreme, so the correction must not overshoot into it.
        //
        // ⚠ AT LEAST ONE, ALWAYS. A bare `floor` reads 0 for a one-unit army and
        // 0 means "nobody defends", which is the exact behaviour being fixed —
        // and a lone soldier with a raider at the gates is precisely who cannot
        // afford to be somewhere else. The share bounds over-commitment; it does
        // not license ignoring the threat entirely.
        //
        // ⚠ THE BUDGET IS THE WHOLE ARMY'S, NOT THIS LIST'S. `responders` has
        // already had the garrison removed, so sizing the cap off it would let
        // garrison and field recall each take half of a shrinking remainder and
        // together take far more than half. Size it off the full eligible army
        // and subtract what the garrison spent.
        let eligible = responders.len() + garrisoned.len();
        let budget = ((eligible as f64 * BARBARIAN_RESPONSE_MAX_SHARE).floor() as usize).max(1);
        // ★★★★ IN PEACETIME THE WHOLE FIELD ARMY IS THE BUDGET. See
        // `camp_party`: the half share protects an offensive, and there is
        // none; a three-unit army with one garrisoned used to read a cap of
        // zero here and leave the raiders and their camp to nobody.
        let cap = if peacetime {
            responders.len()
        } else {
            budget.saturating_sub(garrisoned.len())
        };
        if cap == 0 || !responders.contains(&uid) {
            return None;
        }

        let mut committed: HashSet<u32> = HashSet::new();
        for (_, threat, strength) in threats {
            if committed.len() >= cap {
                break;
            }
            // A defender that would spend ten turns walking home is not a
            // defender. Past that range this unit keeps its offensive job and a
            // nearer one answers instead.
            // Sea answers sea and land answers land: a ship cannot chase a
            // raider inland, and a spearman cannot wade out to a galley.
            let threat_on_water = g.map.get(threat).is_some_and(|tile| g.rules.is_water(tile));
            let mut nearest: Vec<(i32, u32)> = responders
                .iter()
                .filter(|id| !committed.contains(id))
                .filter(|id| {
                    let sea = g.rules.units[g.units[id].kind].domain.as_deref() == Some("sea");
                    sea == threat_on_water
                })
                .map(|id| (g.wdist(g.units[id].pos, threat), *id))
                .filter(|(distance, _)| *distance <= BARBARIAN_RESPONSE_RECALL_RANGE)
                .collect();
            nearest.sort_unstable();

            // Send enough to win rather than enough to trade. A camp needs no
            // margin, so `needed` of zero commits exactly one unit and stops.
            let needed = strength * BARBARIAN_RESPONSE_MARGIN;
            let mut answered = 0.0;
            for (_, responder) in nearest {
                committed.insert(responder);
                if responder == uid {
                    return Some(threat);
                }
                answered += effective_strength(
                    g.unit_strength(&g.units[&responder], false),
                    g.units[&responder].hp,
                );
                if answered >= needed || committed.len() >= cap {
                    break;
                }
            }
        }
        None
    }

    /// Default production controllers answer a barbarian home threat. The
    /// caller runs its local attack scan first; this method owns the bounded
    /// garrison/recall assignment and march to the selected threat.
    pub(crate) fn barbarian_response_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        if self.minor || self.barb {
            return false;
        }
        let Some(barb) = g.barb_pid else {
            return false;
        };
        if !self.barbarian_presence_at_home_for_controller(g, pid, HOME_CAMP_RADIUS) {
            return false;
        }
        let enemies = [barb];
        if self.barbarian_garrison_step(g, pid, uid, &enemies) {
            return true;
        }
        // An empty camp does not appear in `is_enemy_tile`, and
        // `tactical_step` deliberately holds a melee unit one tile from its
        // target. Enter it directly before that range-keeping logic turns a
        // free clear into an idle standoff.
        if self.clear_adjacent_empty_barbarian_camp(g, pid, uid) {
            return true;
        }
        let Some(threat) = self.barbarian_response_objective(g, pid, uid, &enemies) else {
            return false;
        };
        let radius = if g.rules.units[g.units[&uid].kind].has_ranged_attack() {
            g.unit_attack_range(uid).max(1)
        } else {
            1
        };
        if g.wdist(g.units[&uid].pos, threat) > radius {
            return self.step_toward(g, pid, uid, threat);
        }
        self.tactical_step(g, pid, uid, threat, &enemies, radius)
    }

    pub(super) fn nearest_enemy(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        enemy_ids: &[usize],
    ) -> Option<Pos> {
        // Majors chase barbarians only near home and only when this unit's
        // doctrine would accept the eventual attack. This keeps scouts and
        // wounded units from shadowing raiders they will never strike.
        let pos = g.units[&uid].pos;
        let ranged = g.rules.units[g.units[&uid].kind].has_ranged_attack();
        let my_cities: Vec<Pos> = g
            .cities
            .values()
            .filter(|c| c.owner == pid)
            .map(|c| c.pos)
            .collect();
        let near_home = |tpos: Pos| -> bool {
            if self.barb || my_cities.is_empty() {
                return true;
            }
            my_cities.iter().map(|c| g.wdist(tpos, *c)).min().unwrap() <= 6
        };
        let mut best: Option<(i32, Pos)> = None;
        // The camp's reported target is the raider's assignment. Prefer it
        // before the generic nearest-enemy scan, while still allowing a
        // civilian or escort encountered on the way to become the immediate
        // target through the normal scan below.
        if self.barb && self.barbarian_tactics {
            if let Some(home) = Self::barbarian_home(g, uid) {
                if g.wdist(pos, home) > BARBARIAN_RETURN_RADIUS {
                    return Some(home);
                }
            }
            if let Some(target) = Self::barbarian_assigned_target(g, uid)
                .filter(|target| Self::barbarian_target_allowed(g, uid, *target))
            {
                if enemy_ids.iter().any(|enemy| {
                    g.cities
                        .values()
                        .any(|city| city.owner == *enemy && city.pos == target)
                        || g.unit_ids_at(target)
                            .iter()
                            .any(|other| enemy_ids.contains(&g.units[other].owner))
                }) {
                    return Some(target);
                }
            }
        }
        for c in g.cities.values() {
            if enemy_ids.contains(&c.owner)
                && (!self.minor
                    || (self.barb && self.barbarian_tactics)
                    || Self::minor_home(g, pid)
                        .is_some_and(|home| g.wdist(home, c.pos) <= MINOR_DEFENSE_RADIUS))
                && self.barbarian_target_allowed_for_controller(g, uid, c.pos)
            {
                let d = g.wdist(pos, c.pos);
                if best.map(|b| (d, c.pos) < b).unwrap_or(true) {
                    best = Some((d, c.pos));
                }
            }
        }
        for u in g.units.values() {
            if enemy_ids.contains(&u.owner)
                && (!self.minor
                    || (self.barb && self.barbarian_tactics)
                    || Self::minor_home(g, pid)
                        .is_some_and(|home| g.wdist(home, u.pos) <= MINOR_DEFENSE_RADIUS))
                && self.barbarian_target_allowed_for_controller(g, uid, u.pos)
            {
                if Some(u.owner) == g.barb_pid
                    && (!near_home(u.pos)
                        || self.exchange_score(g, uid, u.pos, ranged)
                            <= self.attack_threshold(g, uid, u.pos)
                        || !self.barbarian_raider_counts_as_threat(g, pid, u))
                {
                    continue;
                }
                let d = g.wdist(pos, u.pos);
                if best.map(|b| (d, u.pos) < b).unwrap_or(true) {
                    best = Some((d, u.pos));
                }
            }
        }
        if !self.barb {
            if let Some(bp) = g.barb_pid {
                if enemy_ids.contains(&bp) {
                    // A camp counts as home ground out to the same six-tile
                    // ring a raider does.
                    let camp_radius = HOME_THREAT_RADIUS;
                    let camp_near_home = |tpos: Pos| -> bool {
                        my_cities.is_empty()
                            || my_cities.iter().map(|c| g.wdist(tpos, *c)).min().unwrap()
                                <= camp_radius
                    };
                    for cpos in g.barb_camps.keys() {
                        if camp_near_home(*cpos)
                            // `enemy_of_my_enemy`: not the neighbour's camp.
                            && !self.camp_is_a_neighbours_problem(g, pid, *cpos)
                            && self.exchange_score(g, uid, *cpos, ranged)
                                > self.attack_threshold(g, uid, *cpos)
                        {
                            let d = g.wdist(pos, *cpos);
                            if best.map(|b| (d, *cpos) < b).unwrap_or(true) {
                                best = Some((d, *cpos));
                            }
                        }
                    }
                }
            }
        }
        best.map(|(_, p)| p)
    }

    /// Naval forces should not select an attractive but unreachable inland
    /// target. Waterborne enemies (including embarked land units) come first,
    /// followed by coastal cities that melee ships can actually capture.
    pub(crate) fn nearest_enemy_for_unit(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        enemy_ids: &[usize],
    ) -> Option<Pos> {
        let unit = &g.units[&uid];
        if g.rules.units[unit.kind].domain.as_deref() != Some("sea") {
            return self.nearest_enemy(g, pid, uid, enemy_ids);
        }
        g.units
            .values()
            .filter(|enemy| {
                enemy_ids.contains(&enemy.owner)
                    && Self::waterborne(g, enemy.id)
                    && (!self.minor
                        || (self.barb && self.barbarian_tactics)
                        || Self::minor_home(g, pid)
                            .is_some_and(|home| g.wdist(home, enemy.pos) <= MINOR_DEFENSE_RADIUS))
                    && self.barbarian_target_allowed_for_controller(g, uid, enemy.pos)
            })
            .map(|enemy| (g.wdist(unit.pos, enemy.pos), 0, enemy.pos))
            .chain(
                g.cities
                    .values()
                    .filter(|city| {
                        enemy_ids.contains(&city.owner)
                            && Self::city_is_coastal(g, city.id)
                            && (!self.minor
                                || (self.barb && self.barbarian_tactics)
                                || Self::minor_home(g, pid).is_some_and(|home| {
                                    g.wdist(home, city.pos) <= MINOR_DEFENSE_RADIUS
                                }))
                            && self.barbarian_target_allowed_for_controller(g, uid, city.pos)
                    })
                    .map(|city| (g.wdist(unit.pos, city.pos), 1, city.pos)),
            )
            .min()
            .map(|(_, _, pos)| pos)
    }

    /// Objective for a ship assigned to colony protection. A linked ship
    /// leads the formation toward the settler's persistent colony site; an
    /// unlinked ship first closes on the embarked settler so they can link on
    /// a later command phase.
    pub(crate) fn naval_approach(g: &Game, uid: u32, target: Pos) -> Option<Pos> {
        let current = g.units.get(&uid)?.pos;
        let mut approaches: Vec<Pos> = g
            .nbrs(target)
            .into_iter()
            .filter(|pos| g.unit_can_traverse(uid, *pos))
            .collect();
        approaches.sort_by_key(|pos| (g.wdist(current, *pos), *pos));
        approaches
            .into_iter()
            .find(|pos| *pos == current || g.route_step(uid, *pos, 0).is_some())
    }

    fn naval_escort_objective(&self, g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        let unit = &g.units[&uid];
        if g.rules.units[unit.kind].domain.as_deref() != Some("sea") {
            return None;
        }
        if let Some(settler) = unit.linked_to.filter(|peer| {
            g.units
                .get(peer)
                .is_some_and(|peer| peer.owner == pid && peer.kind == "settler")
        }) {
            return self
                .settler_targets
                .get(&settler)
                .copied()
                .and_then(|target| Self::naval_approach(g, uid, target))
                .or_else(|| Some(g.units[&settler].pos));
        }
        g.units
            .values()
            .filter(|settler| {
                settler.owner == pid
                    && settler.kind == "settler"
                    && settler.linked_to.is_none()
                    && g.map
                        .get(settler.pos)
                        .is_some_and(|tile| g.rules.is_water(tile))
            })
            .min_by_key(|settler| (g.wdist(unit.pos, settler.pos), settler.id))
            .map(|settler| settler.pos)
    }

    /// How much still-unknown ground a unit could expose from `target`.
    ///
    /// This deliberately reads only the acting civilization's explored set and
    /// the scout's own sight radius.  It does not inspect hidden cities,
    /// units, resources, or city-state identities: a Scout learns about a
    /// possible first contact by opening more of the frontier, rather than by
    /// knowing where a contact sits under fog.
    pub(super) fn frontier_reveal_value(g: &Game, pid: usize, uid: u32, target: Pos) -> usize {
        g.wdisk(target, g.unit_sight(uid))
            .into_iter()
            .filter(|pos| !g.players[pid].explored.contains(pos))
            .count()
    }

    /// Known, usable foreign land a ship would be able to see after reaching
    /// `target`. This is limited to the player's explored set: an explorer may
    /// choose a visible landfall, but never a hidden island from full-map data.
    fn island_landfall_value(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        target: Pos,
        home_landmass: &BTreeSet<Pos>,
    ) -> usize {
        g.wdisk(target, g.unit_sight(uid))
            .into_iter()
            .filter(|pos| {
                !home_landmass.contains(pos)
                    && g.players[pid].explored.contains(pos)
                    && self.valid_settle_site(g, pid, *pos)
            })
            .count()
    }

    /// Ground another own explorer is already committed to; empty unless
    /// `explore_commit` holds goals. See `explore_commit`.
    fn reserved_explore_goals(&self, g: &Game, pid: usize, uid: u32) -> Vec<Pos> {
        if !self.explore_commit {
            return Vec::new();
        }
        self.explore_goal
            .borrow()
            .iter()
            .filter(|(other, (_, since))| {
                **other != uid
                    && g.turn.saturating_sub(*since) <= EXPLORE_COMMIT_TURNS
                    && g.units.get(other).is_some_and(|unit| unit.owner == pid)
            })
            .map(|(_, (goal, _))| *goal)
            .collect()
    }

    /// Whether first-contact Envoys are still on the map. The controller knows
    /// how many minor powers joined the game, but not where their cities are;
    /// this only changes the depth of a fog-first search and never reads a
    /// hidden city, unit, or tile memory.
    pub(super) fn city_state_contacts_remain(g: &Game, pid: usize) -> bool {
        g.players.iter().any(|player| {
            player.alive
                && player.is_minor
                && !player.is_barbarian
                && !player.is_free_city
                && !g.has_met(pid, player.id)
        })
    }

    /// Choose an unexplored target for a reconnaissance unit.
    ///
    /// The frozen controllers keep the historical nearest-fog rule.  The
    /// production controller looks a few rings beyond that first tile and
    /// prefers the position that exposes the most unknown ground.  This makes
    /// a Scout fan into a fresh region instead of spending turns shaving the
    /// same local fringe, which is the practical route to earlier civ and
    /// city-state contact.
    pub(super) fn exploration_goal(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        dry_only: bool,
    ) -> Option<Pos> {
        // Goals this unit has proved the host will not walk it to. Empty unless
        // the bridge asks; see `explore_dead_targets`.
        let dead: HashSet<Pos> = if self.explore_dead_targets {
            self.explore_dead
                .borrow()
                .get(&uid)
                .map(|dead| {
                    dead.iter()
                        .filter(|(_, expiry)| **expiry > g.turn)
                        .map(|(pos, _)| *pos)
                        .collect()
                })
                .unwrap_or_default()
        } else {
            HashSet::new()
        };
        let origin = g.units[&uid].pos;
        // `contact-fleet`: while the contact gap is open, a ship that can
        // cross Ocean takes the same long look-ahead and foreign-landfall
        // ranking as a full home continent. See `advanced/contact_fleet.rs`.
        let island_home = if (self.island_exploration || self.contact_fleet)
            && !dry_only
            && g.rules.units[g.units[&uid].kind].domain.as_deref() == Some("sea")
        {
            let landmass = Self::capital_landmass(g, pid);
            (!landmass.is_empty()
                && ((self.island_exploration
                    && self.main_landmass_settlement_room(
                        g,
                        pid,
                        &landmass,
                        ISLAND_EXPLORATION_HOME_SITE_LIMIT + 1,
                    ) <= ISLAND_EXPLORATION_HOME_SITE_LIMIT)
                    || (self.contact_fleet
                        && Self::naval_recon_can_cross_ocean(g, pid)
                        && self.contact_gap_open(g, pid))))
            .then_some(landmass)
        } else {
            None
        };
        let opening_wonder_recon = self.garrison_under_fire
            && g.player_city_ids(pid).len() <= 2
            && advanced::AdvancedAi::early_archers_window_open(g, pid);
        // Visible hostiles at war with us: ground around them is not a goal.
        // Live vision only — a threat the seat cannot see does not steer it.
        let threats: Vec<Pos> =
            if (self.explore_commit || opening_wonder_recon) && !g.players[pid].is_barbarian {
                let visible = g.player_vision_frame(pid);
                g.units
                    .values()
                    .filter(|unit| {
                        unit.owner != pid
                            && g.is_at_war(pid, unit.owner)
                            && g.rules.units[unit.kind].class == "military"
                            && g.sees(&visible, unit.pos)
                    })
                    .map(|unit| unit.pos)
                    .collect()
            } else {
                Vec::new()
            };
        let threatened = |pos: Pos| {
            threats
                .iter()
                .any(|t| g.wdist(*t, pos) <= EXPLORE_COMMIT_THREAT_RADIUS)
        };
        // See `wonder_ring_recon`: a natural-wonder pocket beside home is
        // walked before any frontier, held goal or not. V1 clears every nearby
        // wonder's unseen pocket nearest-tile first; V2 preserves every trigger
        // and allows at most one extra tile to reveal more of that pocket.
        if self.wonder_ring_recon || self.wonder_ring_recon_2 || opening_wonder_recon {
            let reserved = self.reserved_explore_goals(g, pid, uid);
            let wonder_goal = if self.wonder_ring_recon_2 || opening_wonder_recon {
                self.wonder_ring_goal_2(g, pid, uid, dry_only, &dead, &threats, &reserved)
            } else {
                self.wonder_ring_goal(g, pid, uid, dry_only, &dead, &threats, &reserved)
            };
            if let Some(goal) = wonder_goal {
                if self.explore_commit {
                    self.explore_goal.borrow_mut().insert(uid, (goal, g.turn));
                }
                return Some(goal);
            }
        }
        // A held goal is kept while it is still worth walking to; see
        // `explore_commit`. Reached, revealed, written off, threatened, or
        // aged out, it is dropped here and a fresh one chosen below.
        if self.explore_commit {
            let held = self.explore_goal.borrow().get(&uid).copied();
            if let Some((goal, since)) = held {
                let wanted = g.turn.saturating_sub(since) <= EXPLORE_COMMIT_TURNS
                    && goal != origin
                    && !g.players[pid].explored.contains(&goal)
                    && !dead.contains(&goal)
                    && !threatened(goal)
                    && g.unit_can_traverse(uid, goal)
                    && (!dry_only || g.map.get(goal).is_some_and(|tile| !g.rules.is_water(tile)));
                if wanted {
                    return Some(goal);
                }
                self.explore_goal.borrow_mut().remove(&uid);
            }
        }
        let reserved = self.reserved_explore_goals(g, pid, uid);
        // See `EXPLORE_FAILED_BEARING_PENALTY`. Cylinder and arena only: a
        // globe's storage coordinates carry no bearing.
        let unit_vector = |to: Pos| -> Option<(f64, f64)> {
            let mut delta = (to.0 - origin.0, to.1 - origin.1);
            if g.map.topology.wraps_east_west() {
                for shift in [-g.map.width, g.map.width] {
                    let wrapped = (delta.0 + shift, delta.1);
                    if crate::hex::distance((0, 0), wrapped) < crate::hex::distance((0, 0), delta) {
                        delta = wrapped;
                    }
                }
            }
            let (x, y) = (
                delta.0 as f64 + delta.1 as f64 / 2.0,
                delta.1 as f64 * 0.866,
            );
            let length = (x * x + y * y).sqrt();
            (length > 0.0).then(|| (x / length, y / length))
        };
        let failed_bearings: Vec<(f64, f64)> = if g.map.sphere().is_none() {
            dead.iter()
                .filter(|pos| g.wdist(origin, **pos) >= EXPLORE_FAILED_BEARING_MIN_DISTANCE)
                .filter_map(|pos| unit_vector(*pos))
                .collect()
        } else {
            Vec::new()
        };
        let failed_bearing = |target: Pos| -> i32 {
            match unit_vector(target) {
                Some(heading)
                    if failed_bearings
                        .iter()
                        .any(|failed| failed.0 * heading.0 + failed.1 * heading.1 >= 0.5) =>
                {
                    EXPLORE_FAILED_BEARING_PENALTY
                }
                _ => 0,
            }
        };
        // The ordinary committed sweep stops eight rings beyond the first
        // fog, which is right once the table is known. A live Civ VI host can
        // accept a fog order yet leave the unit unmoved, and carries the target
        // retirement guard that proves that case. While that live bridge still
        // has city-states to meet, send one Scout on a continent-crossing
        // search. The destination is route-checked below, so a foreign border
        // cannot consume the whole commitment.
        let contact_sweep = self.explore_commit
            && self.explore_dead_targets
            && g.rules.units[g.units[&uid].kind].promotion_class == "recon"
            && Self::city_state_contacts_remain(g, pid);
        let lookahead = if contact_sweep {
            EXPLORE_CONTACT_SWEEP_LOOKAHEAD
        } else if self.explore_commit || island_home.is_some() {
            EXPLORE_COMMIT_LOOKAHEAD
        } else {
            EXPLORATION_FRONTIER_LOOKAHEAD
        };
        // Home is the capital (the palace), or the oldest city, or — before
        // the first city — where the unit stands, so the first goal is simply
        // the deepest fog.
        let home = if self.explore_commit {
            g.player_city_ids(pid)
                .into_iter()
                .filter_map(|cid| g.cities.get(&cid))
                .min_by_key(|city| (std::cmp::Reverse(g.city_has_palace(city)), city.id))
                .map(|city| city.pos)
        } else {
            None
        };
        let mut candidates = Vec::new();
        let mut first_candidate_ring = None;
        let mut radius = 1;
        let mut examined = 0;
        let _memo = g.query_memo();
        // Every candidate must be both unexplored and on the map: traversal
        // rejects missing tiles. Equal sets therefore prove the ring walk
        // cannot find a goal. Compare the positions too, since a loaded save
        // can contain off-map explored entries with the same count.
        if g.players[pid].explored.len() == g.map.tiles.len()
            && g.players[pid].explored.iter().eq(g.map.tiles.keys())
        {
            return None;
        }
        let mut globe_rings = g
            .map
            .sphere()
            .and_then(|sphere| sphere.outward_ring_search(origin));
        while examined < g.map.tiles.len() {
            let ring = if let Some(search) = &mut globe_rings {
                search.next_ring()
            } else {
                g.wring(origin, radius)
            };
            if ring.is_empty() {
                break;
            }
            examined += ring.len();
            let mut ring_candidates: Vec<Pos> = ring
                .into_iter()
                .filter(|pos| {
                    !g.players[pid].explored.contains(pos)
                        && !dead.contains(pos)
                        && g.unit_can_traverse(uid, *pos)
                        && (!dry_only
                            || g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile)))
                        && !reserved
                            .iter()
                            .any(|held| g.wdist(*pos, *held) <= EXPLORE_COMMIT_SEPARATION)
                        && !threatened(*pos)
                })
                .collect();
            if !ring_candidates.is_empty() {
                first_candidate_ring.get_or_insert(radius);
                candidates.append(&mut ring_candidates);
            }
            let Some(first) = first_candidate_ring else {
                radius += 1;
                continue;
            };
            if !(self.explore_commit || island_home.is_some()) || radius >= first + lookahead {
                break;
            }
            radius += 1;
        }
        let chosen = if contact_sweep {
            // Rank exactly as the committed sweep would, but keep trying the
            // best horizons until one has a route through territory the unit
            // may actually enter. `route_step` keeps a Scout on its side of a
            // known closed border without inspecting hidden actors.
            //
            // The committed sweep's rank includes the charted-terrain rival
            // prior, and so does this one. Ranked on raw reveal alone, the
            // sweep's twenty-ring horizon sends the Scout to whichever map
            // corner shows the most fog — ocean fog counts the same as land —
            // and on the live King Tiny Pangaea (2026-09-30) it spent turns
            // 16–35 in the empty north-west corner while Portugal's Porto
            // stood twelve tiles east of the capital, first seen on turn 52.
            candidates.sort_by_cached_key(|target| {
                std::cmp::Reverse((
                    island_home.as_ref().map_or(0, |home_landmass| {
                        self.island_landfall_value(g, pid, uid, *target, home_landmass)
                    }),
                    Self::frontier_reveal_value(g, pid, uid, *target) as i32 * 4
                        + Self::rival_frontier_prior(g, pid, uid, *target, home)
                        - failed_bearing(*target),
                    home.map_or(0, |home| g.wdist(home, *target)),
                    std::cmp::Reverse(g.wdist(origin, *target)),
                    std::cmp::Reverse(*target),
                ))
            });
            candidates
                .iter()
                .take(EXPLORE_CONTACT_SWEEP_ROUTE_PROBES)
                .find(|target| g.route_step(uid, **target, 0).is_some())
                .copied()
                // If the sampled long horizons are barred, return to the
                // ordinary local commitment rather than holding a known-bad
                // far destination for twenty turns. The candidates are still
                // in rank order, so this is the same preference the regular
                // committed sweep uses inside its normal horizon.
                .or_else(|| {
                    first_candidate_ring.and_then(|first| {
                        candidates
                            .iter()
                            .find(|target| {
                                g.wdist(origin, **target) <= first + EXPLORE_COMMIT_LOOKAHEAD
                            })
                            .copied()
                    })
                })
        } else if let Some(home_landmass) = island_home.as_ref() {
            // Once home is nearly full, a visible foreign landfall outranks an
            // equally revealing patch of empty water. If none is visible, the
            // normal frontier term still sends the ship into fresh water.
            candidates.into_iter().max_by_key(|target| {
                (
                    self.island_landfall_value(g, pid, uid, *target, home_landmass),
                    Self::frontier_reveal_value(g, pid, uid, *target),
                    home.map_or(0, |home| g.wdist(home, *target)),
                    std::cmp::Reverse(g.wdist(origin, *target)),
                    std::cmp::Reverse(*target),
                )
            })
        } else if self.explore_commit {
            // Information gain plus a bounded, charted-terrain rival prior.
            // Distance from home breaks ties so explorers fan outward.
            candidates.into_iter().max_by_key(|target| {
                (
                    Self::frontier_reveal_value(g, pid, uid, *target) as i32 * 4
                        + Self::rival_frontier_prior(g, pid, uid, *target, home)
                        - failed_bearing(*target),
                    home.map_or(0, |home| g.wdist(home, *target)),
                    std::cmp::Reverse(g.wdist(origin, *target)),
                    std::cmp::Reverse(*target),
                )
            })
        } else {
            candidates
                .into_iter()
                .min_by_key(|target| (g.wdist(origin, *target), *target))
        };
        if self.explore_commit {
            if let Some(goal) = chosen {
                self.explore_goal.borrow_mut().insert(uid, (goal, g.turn));
            }
        }
        chosen
    }

    fn is_village(g: &Game, pos: Pos) -> bool {
        matches!(
            g.map.get(pos).and_then(|tile| tile.improvement.as_deref()),
            Some("goody_hut" | "meteor_goody")
        )
    }

    fn is_meteor_site(g: &Game, pos: Pos) -> bool {
        g.map.get(pos).and_then(|tile| tile.improvement.as_deref()) == Some("meteor_goody")
    }

    /// A village is an explorer's prize first. A nearby ordinary military unit
    /// may collect one only as a bounded fallback, and never while escorting a
    /// civilian. Settlers, Builders, and every other civilian role are absent
    /// by construction: their dedicated movement must not turn into a distant
    /// goody-hut chase.
    fn village_collector_role(g: &Game, pid: usize, uid: u32) -> Option<u8> {
        let unit = g.units.get(&uid)?;
        let spec = &g.rules.units[unit.kind];
        if unit.owner != pid
            || unit.moves_left <= 0.0
            || unit.linked_to.is_some()
            || spec.class != "military"
            || spec.domain.as_deref() == Some("air")
        {
            return None;
        }
        Some(u8::from(Self::unit_doctrine(g, uid) != UnitDoctrine::Recon))
    }

    fn village_seek_radius(role: u8, meteor: bool) -> i32 {
        match (role, meteor) {
            (0, false) => VILLAGE_SEEK_RADIUS,
            (0, true) => METEOR_SEEK_RADIUS,
            (_, false) => VILLAGE_MILITARY_SEEK_RADIUS,
            (_, true) => METEOR_MILITARY_SEEK_RADIUS,
        }
    }

    /// Deterministic ownership of a longer village errand. Recon has first
    /// claim to an ordinary village, while a meteor goes to the closest
    /// eligible military unit: its guaranteed free unit is too valuable to
    /// leave for a farther Scout. The ID only settles an exact tie. A unit
    /// that has already spent its movement cannot keep a live collector from
    /// taking the prize now.
    fn village_collector_rank(
        g: &Game,
        pid: usize,
        uid: u32,
        village: Pos,
    ) -> Option<(u8, i32, u32)> {
        let role = Self::village_collector_role(g, pid, uid)?;
        let meteor = Self::is_meteor_site(g, village);
        let radius = Self::village_seek_radius(role, meteor);
        let distance = g.wdist(g.units[&uid].pos, village);
        (distance > 0 && distance <= radius && g.unit_can_traverse(uid, village)).then_some((
            if meteor { 0 } else { role },
            distance,
            uid,
        ))
    }

    fn village_collector_owns(g: &Game, pid: usize, uid: u32, village: Pos) -> bool {
        let Some(own) = Self::village_collector_rank(g, pid, uid, village) else {
            return false;
        };
        g.player_unit_ids(pid)
            .into_iter()
            .filter_map(|other| Self::village_collector_rank(g, pid, other, village))
            .min()
            == Some(own)
    }

    /// Return the village this military unit should collect now. A reachable,
    /// actually seen site is taken immediately by whichever unit can enter it.
    /// For a longer detour, a Scout wins ordinary villages, while a meteor
    /// outranks them and goes to the closest eligible military collector.
    pub(super) fn village_collection_target(&self, g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        if self.minor || self.barb || g.players[pid].is_barbarian {
            return None;
        }
        let role = Self::village_collector_role(g, pid, uid)?;
        let origin = g.units[&uid].pos;
        if !self.village_seeking {
            if !self.hut_collection {
                return None;
            }
            let visible = g.player_vision_frame(pid);
            return g
                .reachable(uid)
                .into_iter()
                .filter(|pos| g.sees(&visible, *pos))
                .filter(|pos| Self::is_village(g, *pos))
                .min_by_key(|pos| (!Self::is_meteor_site(g, *pos), g.wdist(origin, *pos), *pos));
        }
        let known_sites: Vec<(Pos, bool)> = g
            .wdisk(origin, Self::village_seek_radius(role, true))
            .into_iter()
            .filter(|pos| *pos != origin)
            .filter(|pos| g.players[pid].explored.contains(pos))
            .filter(|pos| Self::is_village(g, *pos))
            .map(|pos| (pos, Self::is_meteor_site(g, pos)))
            .collect();
        let known_meteors: Vec<Pos> = known_sites
            .iter()
            .filter_map(|(pos, meteor)| (*meteor).then_some(*pos))
            .collect();
        let known_villages: Vec<Pos> = known_sites
            .into_iter()
            .filter(|(pos, meteor)| {
                !meteor && g.wdist(origin, *pos) <= Self::village_seek_radius(role, false)
            })
            .map(|(pos, _)| pos)
            .collect();

        // Production has charted the target already, so do not clone player
        // vision for every military unit with no local site. A meteor takes
        // precedence even over a reachable ordinary village: the former is a
        // guaranteed free advanced unit, while the latter is a random roll.
        // The legacy immediate-pickup arm still uses current sight below,
        // preserving its information boundary when bounded seeking is off.
        let reachable = g.reachable(uid);
        if let Some(meteor) = known_meteors
            .into_iter()
            .filter(|pos| {
                reachable.contains(pos) || Self::village_collector_owns(g, pid, uid, *pos)
            })
            .min_by_key(|pos| (g.wdist(origin, *pos), *pos))
        {
            return Some(meteor);
        }
        if let Some(village) = reachable
            .iter()
            .copied()
            .filter(|pos| known_villages.contains(pos))
            .min_by_key(|pos| (g.wdist(origin, *pos), *pos))
        {
            return Some(village);
        }
        if self.hut_collection {
            let visible = g.player_vision_frame(pid);
            if let Some(village) = reachable
                .into_iter()
                .filter(|pos| g.sees(&visible, *pos))
                .filter(|pos| Self::is_village(g, *pos))
                .min_by_key(|pos| (!Self::is_meteor_site(g, *pos), g.wdist(origin, *pos), *pos))
            {
                return Some(village);
            }
        }

        known_villages
            .into_iter()
            .filter(|pos| Self::village_collector_owns(g, pid, uid, *pos))
            .min_by_key(|pos| (g.wdist(origin, *pos), *pos))
    }

    /// Spend this movement step on a charted tribal village before generic
    /// exploration, staging, or patrol. The target selector is intentionally
    /// military-only; civilian movement never reaches this method.
    pub(crate) fn village_collection_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let Some(village) = self.village_collection_target(g, pid, uid) else {
            return false;
        };
        self.step_toward(g, pid, uid, village)
    }

    pub(super) fn explore_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        if self.village_collection_step(g, pid, uid) {
            return true;
        }
        let upos = g.units[&uid].pos;
        // `unit_can_traverse` says yes to open water for every land unit the
        // moment embarkation unlocks, so from that turn on the unexplored
        // ocean is a legal exploration goal for the whole army. That is how a
        // Crossbowman leaves the capital tile and spends the next 47 turns
        // pacing between two sea hexes. A land unit explores land.
        let dry_only =
            self.come_ashore && g.rules.units[g.units[&uid].kind].domain.as_deref() != Some("sea");
        let mut goal = self.exploration_goal(g, pid, uid, dry_only);
        if self.explore_dead_targets {
            // ★★★★ A goal this unit was already sent at, from this very tile,
            // for `EXPLORE_STUCK_TURNS` turns running, is one the host will
            // not walk it to: retire it and its ring (an impassable edge is
            // rarely one plot wide) and aim elsewhere. See `explore_dead_targets`.
            if let Some(target) = goal {
                let (stuck, stale) = {
                    let mut last = self.explore_last.borrow_mut();
                    let distance = g.wdist(upos, target);
                    match last.get_mut(&uid) {
                        // The same goal on a later turn. Standing on the same
                        // tile is one more turn of proof the order went
                        // nowhere; not coming any closer is the same proof for
                        // a unit the host moves in circles. Live King
                        // 2026-09-30T225143Z bounced its Scout between two
                        // tiles from t13 to t28, and 2026-10-01T000033Z circled
                        // four tiles from t13 to t44, each move resetting a
                        // same-tile count.
                        Some(entry) if entry.0 == target => {
                            if g.turn > entry.3 {
                                if entry.1 == upos {
                                    entry.2 += 1;
                                } else {
                                    entry.2 = 0;
                                    entry.1 = upos;
                                }
                                if distance < entry.4 {
                                    entry.4 = distance;
                                    entry.5 = 0;
                                } else {
                                    entry.5 += 1;
                                }
                                entry.3 = g.turn;
                            }
                            (entry.2, entry.5)
                        }
                        // A new goal: start counting.
                        _ => {
                            last.insert(uid, (target, upos, 0, g.turn, distance, 0));
                            (0, 0)
                        }
                    }
                };
                if stuck >= EXPLORE_STUCK_TURNS || stale >= EXPLORE_NO_PROGRESS_TURNS {
                    let stuck = stuck.max(stale);
                    self.retire_exploration_target(g, uid, target);
                    if stale >= EXPLORE_NO_PROGRESS_TURNS {
                        let expiry = g.turn + EXPLORE_DEAD_TARGET_TURNS;
                        let mut dead = self.explore_dead.borrow_mut();
                        let unit_dead = dead.entry(uid).or_default();
                        for pos in g.wdisk(target, EXPLORE_NO_PROGRESS_RING) {
                            unit_dead.insert(pos, expiry);
                        }
                    }
                    think!(self.journal, Military, Detail,
                           "{} {uid} gives up on {target:?}", g.units[&uid].kind;
                           "ordered there {stuck} turns running from {upos:?} without coming \
                            closer; the host will not walk it there, so that ground is retired \
                            for {EXPLORE_DEAD_TARGET_TURNS} turns";
                           upos);
                    goal = self.exploration_goal(g, pid, uid, dry_only);
                }
            }
        }
        if let Some(target) = goal {
            if self.step_toward(g, pid, uid, target) {
                return true;
            }
        } else {
            // Nothing hidden is reachable, and the route search below would
            // only flood the unit's whole region to prove the same thing.
            return false;
        }

        // If the chosen frontier target was unreachable, search for the
        // nearest hidden tile by an actual traversable route instead.
        let goals: HashSet<Pos> = {
            let _memo = g.query_memo();
            g.map
                .tiles
                .iter()
                .filter(|(pos, tile)| {
                    !g.players[pid].explored.contains(pos)
                        && g.unit_can_traverse(uid, **pos)
                        && (!dry_only || !g.rules.is_water(tile))
                })
                .map(|(pos, _)| *pos)
                .collect()
        };
        let next = match g.route_step_to_any(uid, &goals) {
            Some(p) if g.can_move(uid, p) => p,
            _ => return false,
        };
        // The exhaustive search is the greedy walk's fallback, so it must
        // honour the same refusals — otherwise a Scout barred from retreading
        // its loop by the cheap path takes the identical step here.
        self.path_move(g, pid, uid, next)
    }

    /// Walk an embarked land unit back onto dry land, preferring our own
    /// territory.
    ///
    /// The pre-existing come-ashore rule in `peacetime_step` is welded to
    /// `modernization_step`, which returns `false` before it does anything at
    /// all when `unit_upgrade_target` is `None`. So the only embarked units
    /// ever told to land were the ones that happened to have an unlocked,
    /// affordable successor waiting; everything else stayed at sea for the
    /// rest of the game. Measured across 133 live runs, land combat units
    /// spend a mean **15%** of their unit-turns embarked (p90 48%, worst run
    /// 84%) — and **21.7% mean while one of our own cities is taking damage**,
    /// 92.8% in the worst case. An embarked unit cannot attack at all and
    /// defends at `embarked_strength`, so those turns are not merely
    /// misplaced, they are spent as a non-combatant.
    ///
    /// The nearest shore is chosen by distance and walked with `step_toward`,
    /// the same mover `modernization_step` uses to bring a unit home — the
    /// exhaustive `route_step_to_any` is only the fallback, because a
    /// water-to-land step is a disembarkation and the cheap route search
    /// handles that transition where a raw goal-set flood does not.
    pub(crate) fn disembark_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        if g.rules.units[g.units[&uid].kind].domain.as_deref() == Some("sea")
            || !g.is_embarked(&g.units[&uid])
        {
            return false;
        }
        // A landing site has to be one the unit can actually end its move on.
        // Occupied tiles are excluded deliberately: our own territory is
        // usually one small city whose tile the garrison already holds, and
        // aiming at it parks the castaway one hex offshore forever — closing
        // to shore distance 1 and never landing, which is the defect wearing
        // a different hat.
        let (home, any): (HashSet<Pos>, HashSet<Pos>) = {
            let _memo = g.query_memo();
            let landable: Vec<(Pos, bool)> = g
                .map
                .tiles
                .iter()
                .filter(|(pos, tile)| {
                    !g.rules.is_water(tile)
                        && g.rules.is_passable(tile)
                        && g.unit_can_traverse(uid, **pos)
                        && g.unit_ids_at(**pos).is_empty()
                })
                .map(|(pos, tile)| {
                    let ours = tile
                        .owner_city
                        .and_then(|cid| g.cities.get(&cid))
                        .is_some_and(|city| city.owner == pid);
                    (*pos, ours)
                })
                .collect();
            (
                landable
                    .iter()
                    .filter(|(_, ours)| *ours)
                    .map(|(pos, _)| *pos)
                    .collect(),
                landable.iter().map(|(pos, _)| *pos).collect(),
            )
        };
        // Home first, then any shore at all — a castaway whose territory is
        // unreachable still has to get out of the water before it can do
        // anything, so the fallback runs when the preferred set fails to move
        // it, not merely when that set is empty.
        for goals in [home, any] {
            if goals.is_empty() {
                continue;
            }
            let upos = g.units[&uid].pos;
            if let Some(shore) = goals
                .iter()
                .copied()
                .min_by_key(|pos| (g.wdist(upos, *pos), *pos))
            {
                if self.step_toward(g, pid, uid, shore) && g.units[&uid].pos != upos {
                    return true;
                }
            }
            if let Some(next) = g
                .route_step_to_any(uid, &goals)
                .filter(|pos| g.can_move(uid, *pos))
            {
                if self.path_move(g, pid, uid, next) {
                    return true;
                }
            }
        }
        false
    }

    /// Move an otherwise idle military unit between useful frontier posts.
    /// Targets persist across turns, avoiding random-looking oscillation; a
    /// new post is selected only after the old one is reached or invalidated.
    pub(super) fn patrol_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let current = g.units[&uid].pos;
        let previous = self.patrol_targets.get(&uid).copied();
        if let Some(target) = previous {
            if target != current && self.patrol_tile(g, pid, uid, target) {
                if let Some(next) = g
                    .route_step(uid, target, 0)
                    .filter(|pos| g.can_move(uid, *pos))
                {
                    // A dense frontier can offer two adjacent posts, and a
                    // unit that keeps swapping between them is pacing, not
                    // patrolling. `path_move` declines the retread, which
                    // sends the selection below after a post it has not
                    // already worn out.
                    if self.path_move(g, pid, uid, next) {
                        return true;
                    }
                }
            }
            self.patrol_targets.remove(&uid);
        }

        let domain = g.rules.units[g.units[&uid].kind]
            .domain
            .as_deref()
            .unwrap_or("land")
            .to_string();
        let posts = self.patrol_posts_for(g, pid, uid, &domain, false);
        if posts.is_empty() {
            return false;
        }

        let start = previous
            .and_then(|target| posts.binary_search(&target).ok().map(|index| index + 1))
            .unwrap_or(uid as usize % posts.len());
        // Trying a bounded number of distributed posts avoids an expensive
        // all-map path search when a unit is isolated on another landmass.
        for offset in 0..posts.len().min(24) {
            let target = posts[(start + offset) % posts.len()];
            // A post inside the footprint this unit has been circling is not a
            // destination; it is where the circling has been happening.
            if target == current || self.retreads_a_loop(uid, target) {
                continue;
            }
            let Some(next) = g
                .route_step(uid, target, 0)
                .filter(|pos| g.can_move(uid, *pos))
            else {
                continue;
            };
            if !self.path_move(g, pid, uid, next) {
                continue;
            }
            self.patrol_targets.insert(uid, target);
            return true;
        }
        false
    }

    /// The largest single blow the observed enemy could land on this unit
    /// where it stands, at the top of the engine's damage roll. Zero when the
    /// gene is off, so a caller can compare against it unconditionally.
    ///
    /// ⚠ COSTS NOTHING EXTRA WHERE IT IS CALLED, AND THE FILTER IS WHY. The
    /// envelope table is the most expensive thing this controller computes —
    /// one movement flow field per visible enemy, and #2059 turned a per-unit
    /// recompute of it into a six-fold slowdown of every simulation on the
    /// fleet. The set of units answered below is exactly `retreat_step`'s,
    /// which `healing_step` has already run on this board, so the fetch is a
    /// hit on that cache under the same key rather than a fresh table. The
    /// price of that discipline is that a support unit — a Battering Ram, a
    /// Siege Tower — keeps the constant withdrawal floor it has always had.
    ///
    /// A garrison is answered before the fetch: an attack on a City Center or
    /// an Encampment damages the district, not the formation standing in it.
    /// See `one_shot_recovery`.
    fn one_shot_killing_blow(&self, g: &Game, pid: usize, uid: u32) -> f64 {
        if !self.one_shot_recovery {
            return 0.0;
        }
        let Some(unit) = g.units.get(&uid) else {
            return 0.0;
        };
        let spec = &g.rules.units[unit.kind];
        // Air units are not on the board between missions; they rebase.
        if unit.owner != pid
            || unit.moves_left <= 0.0
            || spec.class != "military"
            || spec.domain.as_deref() == Some("air")
            || (!spec.is_melee_capable() && !spec.has_ranged_attack())
        {
            return 0.0;
        }
        let here = unit.pos;
        if g.city_at(here).is_some() || g.defending_district_at(here).is_some() {
            return 0.0;
        }
        let envelopes = self.enemy_attack_envelopes(g, pid);
        Self::killing_blow(g, pid, uid, here, &envelopes)
    }

    /// Recovery ground an enemy can strike next turn is not recovery ground.
    /// Score every tile this unit can reach the way an evacuation does and
    /// take the best of them, so a unit healing under a shooter that has just
    /// arrived steps out of its reach instead of fortifying beneath it.
    ///
    /// ★★ THE ROUTE STEP CANNOT OFFER THIS MOVE. `safe_healing_step` returns
    /// the first hop of a route to distant healing ground, so the one step
    /// sideways that leaves the envelope is invisible to it, and so is every
    /// tile that is merely less exposed than this one. When it finds nothing,
    /// the caller fortifies where it stands and heals under the axe.
    /// See `one_shot_recovery`.
    fn step_out_of_reach_to_heal(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        envelopes: &std::sync::Arc<AttackEnvelopes>,
    ) -> bool {
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        if unit.moves_left <= 0.0 {
            return false;
        }
        let here = unit.pos;
        let Some(holding) = Self::evacuation_tile(g, pid, uid, here, None, envelopes) else {
            return false;
        };
        // Nothing can reach this tile next turn, so healing on it is right.
        if holding.incoming <= 1e-9 {
            return false;
        }
        let mut candidates: Vec<EvacuationTile> = g
            .reachable(uid)
            .into_iter()
            .filter_map(|position| Self::evacuation_tile(g, pid, uid, position, None, envelopes))
            .collect();
        if let Some(step) = self.safe_healing_step(g, pid, uid, envelopes) {
            candidates.extend(Self::evacuation_tile(g, pid, uid, step, None, envelopes));
        }
        // ⚠ NO DANGER ANCHOR, ON PURPOSE. With one, `evacuation_tile_cmp`
        // depends on where the unit is standing, and a unit stepping to the
        // best tile from A can find A the best tile again from B. Without one
        // the order is fixed over tiles, this only ever moves to a strictly
        // better tile, and a turn cannot be spent walking in a circle.
        let Some(next) = candidates.into_iter().max_by(Self::evacuation_tile_cmp) else {
            return false;
        };
        Self::evacuation_tile_is_better(next, holding)
            && self.move_to_evacuation_tile(g, pid, uid, next.position)
    }

    pub(super) fn healing_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> Option<bool> {
        // There is no recovery on a Tactics arena, because nothing heals
        // there. A unit that dropped below the withdrawal line would be put
        // into `recovering_units`, sent looking for friendly ground that does
        // not exist, and left fortified in a corner for the rest of the
        // battle waiting for hit points that never come — permanently out of
        // a fight it is still perfectly able to influence. On a battlefield a
        // damaged unit fights on, and the fact that it is damaged is exactly
        // why the enemy is coming for it. A board that heals is a campaign,
        // and recovery is exactly the thing it exists to measure.
        if g.is_arena() && !g.tactics.heal {
            self.recovering_units.remove(&uid);
            return None;
        }
        // This is deliberately before the ordinary withdrawal threshold: a
        // fresh-looking unit can still be dead to several exact enemy attack
        // envelopes next turn, and needs to evacuate before it spends this
        // turn on a low-value attack or march.
        if self.precise_evacuation {
            if let Some(acted) = self.retreat_step(g, pid, uid) {
                // A threat-created retreat is recovery, not merely a two-turn
                // positional dodge. Keep this unit at the safe destination until
                // the normal rejoin threshold says it has actually healed up.
                self.recovering_units.insert(uid);
                return Some(acted);
            }
        }
        let withdraw_at_hp = self.w.withdraw_hp.round() as i32;
        let return_at_hp = self.w.rejoin_hp.max(self.w.withdraw_hp + 5.0).round() as i32;
        // `veterans-withdraw-early`: a promoted unit leaves the line with
        // hit points in hand and rejoins only once it has them back.
        let veteran_margin = self.veteran_hp_margin(g, uid);
        let withdraw_at_hp = withdraw_at_hp + veteran_margin;
        let return_at_hp = return_at_hp.max(withdraw_at_hp + 5);

        let hp = g.units[&uid].hp;
        // ★★★ `one_shot_recovery`: THE WITHDRAWAL LINE IS THE ENEMY'S TO SET.
        // A constant cannot know that the thing across the river hits for 70,
        // so a unit on 60 reads healthy right up to the turn it is removed
        // from the board. This asks what one blow would do to this unit here,
        // treats a unit that would not survive one as already in recovery
        // whatever its hit points say, and keeps it there while the offer
        // stands — rejoining at the static line walks it back under the same
        // gun on the same hit points.
        let killing_blow = self.one_shot_killing_blow(g, pid, uid);
        let one_blow_from_death =
            killing_blow > 0.0 && f64::from(hp - veteran_margin) <= killing_blow;
        if hp >= return_at_hp && !one_blow_from_death {
            self.recovering_units.remove(&uid);
            return None;
        }
        if hp <= withdraw_at_hp || one_blow_from_death {
            self.recovering_units.insert(uid);
        }
        if !self.recovering_units.contains(&uid) {
            return None;
        }

        if self.precise_evacuation {
            let envelopes = self.enemy_attack_envelopes(g, pid);
            let here = g.units[&uid].pos;
            let holding = Self::evacuation_tile(g, pid, uid, here, None, &envelopes);
            // Once safely inside friendly borders, spending the turn stationary
            // is faster than sacrificing another healing tick to chase a city.
            // Keep the threat check in front of this: friendly ground covered by
            // an enemy envelope is not a place to wait merely because it heals.
            if g.unit_heal_rate(uid) >= 15 && holding.is_some_and(|tile| tile.incoming <= 1e-9) {
                return Some(self.fortify_or_stop(g, pid, uid));
            }
            // ★★★ `one_shot_recovery`, the other half: GROUND UNDER AN ENEMY'S
            // NEXT TURN IS NOT RECOVERY GROUND, whatever it heals at. The hold
            // above has just refused this tile, and the route hop below can
            // only offer the first step toward distant healing ground — one
            // tile, chosen without asking whether it is still under the same
            // shooter. This scores every tile the unit can actually reach,
            // which is what puts a wounded unit inside the City Center two
            // steps away instead of one tile nearer to it.
            if self.one_shot_recovery && self.step_out_of_reach_to_heal(g, pid, uid, &envelopes) {
                return Some(true);
            }
            if let (Some(holding), Some(next)) =
                (holding, self.safe_healing_step(g, pid, uid, &envelopes))
            {
                if let Some(candidate) = Self::evacuation_tile(g, pid, uid, next, None, &envelopes)
                {
                    if Self::evacuation_tile_is_better(candidate, holding) {
                        return Some(
                            self.move_to_evacuation_tile(g, pid, uid, candidate.position)
                                || self.fortify_or_stop(g, pid, uid),
                        );
                    }
                }
            }
        }

        // If every safe recovery route is unavailable, keep the historical
        // friendly-ground recovery rather than resuming a bad attack.
        if g.unit_heal_rate(uid) >= 15 {
            return Some(self.fortify_or_stop(g, pid, uid));
        }

        let friendly_tiles: HashSet<Pos> = g
            .map
            .tiles
            .keys()
            .filter(|pos| g.healing_location(pid, **pos).rate() >= 15)
            .copied()
            .collect();
        if let Some(next) = g
            .route_step_to_any(uid, &friendly_tiles)
            .filter(|pos| g.can_move(uid, *pos))
        {
            return Some(self.path_move(g, pid, uid, next));
        }

        // If home is unreachable (for example, an isolated naval unit), wait
        // and use the neutral/enemy rate instead of continuing a bad attack.
        Some(self.fortify_or_stop(g, pid, uid))
    }

    pub(super) fn military_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        // The barbarian seat has no city to serve as `minor_home`. Its camp
        // guard and engine-managed Scout are already moved by the world phase;
        // every other military unit is a real raider.
        if self.barb
            && self.barbarian_tactics
            && (g.barb_camp_guards.values().any(|guard| *guard == uid)
                || g.barb_scout_homes.contains_key(&uid))
        {
            return self.fortify_or_stop(g, pid, uid);
        }
        if self.minor && (!self.barb || !self.barbarian_tactics) {
            let Some(home) = Self::minor_home(g, pid) else {
                return self.fortify_or_stop(g, pid, uid);
            };
            if g.wdist(g.units[&uid].pos, home) > MINOR_DEFENSE_RADIUS {
                if let Some(next) = g
                    .route_step(uid, home, 0)
                    .filter(|next| g.can_move(uid, *next))
                {
                    return self.path_move(g, pid, uid, next) || self.fortify_or_stop(g, pid, uid);
                }
                return self.fortify_or_stop(g, pid, uid);
            }
        }
        if let Some(acted) = self.healing_step(g, pid, uid) {
            return acted;
        }
        if self.unit_objective_memory {
            if let Some(acted) = self.retreat_step(g, pid, uid) {
                return acted;
            }
        }
        let upos = g.units[&uid].pos;
        let rules = std::sync::Arc::clone(&g.rules);
        let spec = &rules.units[g.units[&uid].kind];
        let doctrine = Self::unit_doctrine(g, uid);
        // A settler in the barbarians' hands is never declined: recapture
        // costs one move and returns a full production payment (usually our
        // own settler walking to a camp), so it must neither be refused by
        // the duplicate/unusable guard below nor trip its freeze.
        // `barb_pid`, not `is_barbarian` — Free Cities carry the flag too.
        let barb_rescue: Option<usize> = if self.barbarian_settler_capture {
            g.barb_pid
        } else {
            None
        };
        let adjacent_enemy_settler = g.nbrs(upos).into_iter().any(|position| {
            g.unit_ids_at(position).iter().any(|other| {
                g.units[other].owner != pid
                    && g.is_at_war(pid, g.units[other].owner)
                    && g.units[other].kind == "settler"
                    && barb_rescue != Some(g.units[other].owner)
            })
        });
        let decline_settlers = adjacent_enemy_settler
            && (!self.barb || !self.barbarian_tactics)
            && (self.minor
                || g.units
                    .values()
                    .any(|unit| unit.owner == pid && unit.kind == "settler")
                || !self.has_practical_settle_site(g, pid));
        if self.condemn_heretic_here(g, pid, uid) {
            return true;
        }
        if self.capture_adjacent_civilian(g, pid, uid, decline_settlers) {
            return true;
        }
        if self.pursue_capturable_civilian(g, pid, uid, decline_settlers) {
            return true;
        }
        if decline_settlers {
            // Do not let generic tactical movement bypass the same guard and
            // walk onto a Settler this civilization cannot use.
            return self.fortify_or_stop(g, pid, uid);
        }
        if !self.minor {
            if let Some(action) = self.doctrine_action(g, pid, uid) {
                return g.apply(pid, &action).is_ok();
            }
        }
        if matches!(doctrine, UnitDoctrine::AirDefense | UnitDoctrine::AirStrike) {
            return false;
        }
        let mut enemy_ids: Vec<usize> = g
            .players
            .iter()
            .filter(|o| {
                o.id != pid
                    && o.alive
                    && g.is_at_war(pid, o.id)
                    && (!self.minor
                        || (self.barb && self.barbarian_tactics)
                        || Self::minor_enemy_near_home(g, pid, o.id))
            })
            .map(|o| o.id)
            .collect();
        // A barbarian home threat is a local defense problem, not a major-war
        // campaign. Admit the seat for the attack scan and the dedicated
        // bounded responder assignment below; unclaimed wartime units still
        // use their ordinary major-war list after that assignment gets first
        // claim.
        if !self.minor && !self.barb {
            if let Some(barb) = g.barb_pid {
                if self.barbarian_tactics
                    && (self.barbarian_presence_at_home_for_controller(g, pid, HOME_CAMP_RADIUS)
                        || self.has_harmless_naval_xp_shot(g, pid, uid))
                    && !enemy_ids.contains(&barb)
                {
                    enemy_ids.push(barb);
                }
            }
        }
        if !enemy_ids.is_empty() {
            self.patrol_targets.remove(&uid);
            // Pick the best role-adjusted exchange among all attackable tiles.
            // A scout needs a clear opportunity; an assault unit presses a
            // thinner edge, and siege spends its attacks on districts.
            let radius = if spec.has_ranged_attack() {
                g.unit_attack_range(uid).max(1)
            } else {
                1
            };
            let mut best: Option<(f64, Pos, Action)> = None;
            // Hoisted out of the candidate loop below (see
            // `Game::ranged_order_is_legal`): neither frame can move while
            // this loop applies nothing, so it is built at most once here
            // rather than once per candidate tile. Built lazily — most units
            // reach no enemy tile. `player_vision_frame` hands back the
            // engine's own `Arc`, so even that one build is a refcount bump
            // rather than a `TileBits` clone.
            let mut vision_frames: Option<(
                std::sync::Arc<crate::world::TileBits>,
                std::collections::BTreeSet<usize>,
            )> = None;
            for pos in g.wdisk(upos, radius) {
                if pos == upos
                    || g.map.get(pos).is_none()
                    || !self.is_enemy_tile(g, pos, &enemy_ids)
                    || (self.minor
                        && (!self.barb || !self.barbarian_tactics)
                        && Self::minor_home(g, pid)
                            .is_none_or(|home| g.wdist(home, pos) > MINOR_DEFENSE_RADIUS))
                {
                    continue;
                }
                let distance = g.wdist(upos, pos);
                let mut modes = Vec::with_capacity(2);
                // ⚠ Candidates here are scored statically, not on a clone, so
                // an order the engine refuses can win the argmax outright and
                // the unit does nothing offensive at all — the legal shot
                // that came second is shadowed, not merely delayed. Census:
                // 519 authoritative refusals in one deployment game, all from
                // this loop. Behind `legal_tactical_candidates` the engine's
                // own predicates screen the set; the frozen identities keep
                // the historical candidates (see the field's note).
                if spec.has_ranged_attack()
                    && distance <= g.unit_attack_range(uid)
                    && (!(self.legal_tactical_candidates || self.naval_threat_triage) || {
                        let frames = vision_frames.get_or_insert_with(|| {
                            (g.player_vision_frame(pid), g.visibility_viewers(pid))
                        });
                        g.ranged_order_is_legal(pid, uid, pos, &frames.0, &frames.1)
                    })
                {
                    modes.push((
                        true,
                        Action::Ranged {
                            unit: uid,
                            target: pos,
                        },
                    ));
                }
                if g.units[&uid].kind == "spec_ops"
                    && distance <= g.unit_attack_range(uid)
                    && g.priority_support_target_at(pid, pos).is_some()
                    && (!self.legal_tactical_candidates || {
                        let frames = vision_frames.get_or_insert_with(|| {
                            (g.player_vision_frame(pid), g.visibility_viewers(pid))
                        });
                        g.units[&uid].attacks_left > 0
                            && g.combat_target_visible_at(pid, pos, &frames.0, &frames.1)
                            && g.unit_has_line_of_sight(uid, pos)
                    })
                {
                    modes.push((
                        true,
                        Action::PriorityTarget {
                            unit: uid,
                            target: pos,
                        },
                    ));
                }
                if spec.is_melee_capable()
                    && distance == 1
                    && (!self.legal_tactical_candidates || g.melee_order_is_legal(pid, uid, pos))
                {
                    modes.push((
                        false,
                        Action::Attack {
                            unit: uid,
                            target: pos,
                        },
                    ));
                }
                for (ranged, action) in modes {
                    let capture =
                        !ranged && g.city_at(pos).is_some_and(|cid| g.cities[&cid].hp <= 0);
                    let utility = if matches!(action, Action::PriorityTarget { .. }) {
                        Self::priority_target_score(g, pid, pos) as f64 - 55.0
                    } else {
                        self.exchange_score(g, uid, pos, ranged)
                            - self.attack_threshold(g, uid, pos)
                            + self.harmless_naval_xp_bonus(g, pid, uid, pos, ranged)
                            + if capture { 500.0 } else { 0.0 }
                    };
                    if best
                        .as_ref()
                        .map(|(old, old_pos, _)| {
                            utility > *old || (utility == *old && pos < *old_pos)
                        })
                        .unwrap_or(true)
                    {
                        best = Some((utility, pos, action));
                    }
                }
            }
            if let Some((utility, _, action)) = best {
                if utility > 0.0 && g.apply(pid, &action).is_ok() {
                    return true;
                }
            }
            if let Some(action) = self.barbarian_raid_action(g, pid, uid) {
                if g.apply(pid, &action).is_ok() {
                    return true;
                }
            }
            if !self.barb && self.barbarian_tactics && self.barbarian_response_step(g, pid, uid) {
                return true;
            }
            // A camp outside the local-defense assignment can still be a
            // free, visible clear on this unit's next step. Combat, pillage,
            // and any emergency garrison above retain their precedence.
            if self.clear_adjacent_empty_barbarian_camp(g, pid, uid) {
                return true;
            }
            let hostile_water_unit = g
                .units
                .values()
                .any(|enemy| enemy_ids.contains(&enemy.owner) && Self::waterborne(g, enemy.id));
            if !hostile_water_unit {
                if let Some(target) = self.naval_escort_objective(g, pid, uid) {
                    if target != upos && self.step_toward(g, pid, uid, target) {
                        return true;
                    }
                    if g.units[&uid].linked_to.is_some_and(|peer| {
                        g.units
                            .get(&peer)
                            .is_some_and(|unit| unit.kind == "settler")
                    }) {
                        return self.fortify_or_stop(g, pid, uid);
                    }
                }
            }
            if !self.minor
                && doctrine == UnitDoctrine::Recon
                && self.should_explore(g, pid, uid, true)
                && self.explore_step(g, pid, uid)
            {
                return true;
            }
            // On a capture-the-flag arena the ENEMY's flag outranks every
            // march this unit could make: standing on it wins the battle
            // outright, so any unit not spending its turn on a worthwhile
            // attack above is a unit going after it. Ours is not a target —
            // a side cannot capture its own flag — so the aim is theirs.
            // `arena_enemy_flag` is `None` on every world, so they walk past.
            //
            return match g
                .arena_enemy_flag(pid, upos)
                .or_else(|| self.capture_objective_target(g, pid, uid))
                .or_else(|| self.nearest_enemy_for_unit(g, pid, uid, &enemy_ids))
            {
                Some(t) => self.tactical_step(g, pid, uid, t, &enemy_ids, radius),
                None => self.peacetime_step(g, pid, uid, true),
            };
        }
        if let Some(action) = self.barbarian_raid_action(g, pid, uid) {
            if g.apply(pid, &action).is_ok() {
                return true;
            }
        }
        self.peacetime_step(g, pid, uid, false)
    }

    /// Civilian capture is movement, not combat. Feeding an undefended
    /// Settler or Builder into `Action::Attack` is rejected by the engine and
    /// used to leave entire armies surrounding it forever. Take the free unit
    /// with a legal move, while declining a duplicate/unusable Settler.
    pub(crate) fn capture_adjacent_civilian(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        decline_settlers: bool,
    ) -> bool {
        if g.rules.units[g.units[&uid].kind].class != "military" {
            return false;
        }
        let origin = g.units[&uid].pos;
        let target = g
            .nbrs(origin)
            .into_iter()
            .filter_map(|position| {
                if self.minor
                    && (!self.barb || !self.barbarian_tactics)
                    && Self::minor_home(g, pid)
                        .is_none_or(|home| g.wdist(home, position) > MINOR_DEFENSE_RADIUS)
                {
                    return None;
                }
                let value = g
                    .unit_ids_at(position)
                    .iter()
                    .filter_map(|other| {
                        let other = &g.units[other];
                        if other.owner == pid || !g.is_at_war(pid, other.owner) {
                            return None;
                        }
                        // A barbarian-held settler outranks everything and is
                        // never declined; see `barbarian_settler_capture`.
                        let barb_rescue =
                            self.barbarian_settler_capture && g.barb_pid == Some(other.owner);
                        match other.kind.as_str() {
                            "settler" if barb_rescue => Some(4),
                            "settler" if !decline_settlers => Some(3),
                            "settler" => None,
                            "builder" => Some(2),
                            _ if matches!(
                                g.rules.units[other.kind].class.as_str(),
                                "civilian" | "support"
                            ) =>
                            {
                                Some(1)
                            }
                            // A religious unit is condemned, not captured, and
                            // only the barbarian hunt steps onto one here; a
                            // major's answer is `AdvancedAi::condemn_step`.
                            _ if self.hunts_heretics()
                                && g.rules.units[other.kind].class == "religious" =>
                            {
                                Some(1)
                            }
                            _ => None,
                        }
                    })
                    .max()?;
                g.can_move(uid, position).then_some((value, position))
            })
            .max_by_key(|(value, position)| (*value, std::cmp::Reverse(*position)))
            .map(|(_, position)| position);
        let Some(to) = target else {
            return false;
        };
        if g.apply(pid, &Action::Move { unit: uid, to }).is_err() {
            return false;
        }
        // Arriving on a religious unit with movement left is the whole hunt.
        self.condemn_heretic_here(g, pid, uid);
        true
    }

    /// Whether this controller is the barbarian seat hunting religious units.
    fn hunts_heretics(&self) -> bool {
        self.barb && self.barbarian_tactics && self.barbarian_heretic_hunt
    }

    /// The barbarian half of the religious hunt: a raider standing on a
    /// religious unit's tile with movement left condemns it. `g.apply` holds
    /// the engine's own legality (a military mover, war, the same tile,
    /// movement left), so this only chooses the target — the weakest first,
    /// then the lowest id, for a stable replay. Nothing for a major: its
    /// answer is `AdvancedAi::condemn_step`, and the frozen anchor never
    /// carries `barbarian_tactics`.
    pub(crate) fn condemn_heretic_here(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        if !self.hunts_heretics() {
            return false;
        }
        let unit = &g.units[&uid];
        if g.rules.units[unit.kind].class != "military" || unit.moves_left <= 0.0 {
            return false;
        }
        let here = unit.pos;
        let target_unit = g
            .unit_ids_at(here)
            .iter()
            .filter(|oid| {
                let other = &g.units[oid];
                other.owner != pid
                    && g.rules.units[other.kind].class == "religious"
                    && g.is_at_war(pid, other.owner)
            })
            .min_by_key(|oid| (g.units[oid].hp, *oid));
        let Some(target_unit) = target_unit else {
            return false;
        };
        let kind = g.units[&uid].kind.as_str();
        think!(self.journal, Military, Decision,
               "{kind} {uid} condemns the heretic on its tile";
               "a religious unit is wiped by the movement a military unit \
                spends standing on it, and the barbarians spend it";
               here);
        g.apply(
            pid,
            &Action::CondemnHeretic {
                unit: uid,
                target_unit: *target_unit,
            },
        )
        .is_ok()
    }

    /// The rescue pursuit: a capturable civilian within reach is taken by
    /// walking onto it, not watched from adjacency.
    ///
    /// Adjacent-only capture lost a settler on the live seat
    /// (`civvis-20260818T222844Z` t27–t33): the pursuer stepped to the tile
    /// *beside* the settler every turn — the position a ranged attack wants,
    /// and the engine rejects attacks on an undefended civilian — while the
    /// barbarians walked it home to a camp. One step per call; the unit loop
    /// composes the walk within the turn, and the adjacent branch above
    /// finishes the capture. Settlers and builders only: a generic support
    /// pickup is not worth pulling a formation apart, and stays
    /// adjacent-only. The barbarian hunt (`barbarian_heretic_hunt`) also
    /// walks onto a religious unit, which it condemns rather than captures.
    ///
    /// Two callers, two horizons. The barbarian hunter chases within this
    /// turn's movement and its raid ring, exactly as before. A major under
    /// `barbarian_settler_capture` chases a BARBARIAN-HELD settler out to
    /// `BARBARIAN_SETTLER_PURSUIT_RADIUS` — a settler walks two tiles a turn
    /// and a camp is where it is going, so the turn after next is the last
    /// cheap one — unless the unit is the only soldier in one of our cities,
    /// which chases only what it can take this turn. Everything else a major
    /// pursues stays within this turn's reach.
    pub(crate) fn pursue_capturable_civilian(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        decline_settlers: bool,
    ) -> bool {
        // ★★★★★ THE BARBARIANS IN THIS ENGINE DO NOT HUNT CIVILIANS, AND THE
        // ONES IN CIVILIZATION VI DO NOTHING ELSE.
        //
        // MEASURED, `ai_eval live live_without_camp_reach`, 6 pairs / 36
        // seat-games each, 6 players, 150 turns, online speed:
        // **civilians lost to barbarians 0.31 and 0.25 per game**. The live
        // Civilization VI seat on the same shape lost **8 Settlers and 2
        // Builders in 104 turns** on run civvis-20260821T130446Z and **4 and 4
        // by turn 62** on civvis-20260821T133955Z. That is a factor of roughly
        // THIRTY, and it is a fidelity bug rather than a tuning question: an
        // undefended Settler is the single most valuable thing on a
        // Civilization VI map and the barbarian AI beelines for it.
        //
        // The cause is this gate. `capture_adjacent_civilian` is ungated, so a
        // barbarian takes a Settler that happens to end its turn next to one —
        // an accident. The PURSUIT, which is what actually costs the empire its
        // walkers, was gated behind `civilian_rescue`, a flag the barbarian
        // seat never carries (`BasicAi::new()` ships it off and the barbarian
        // controller is a `BasicAi`). So raiders marched at the camp's reported
        // CITY and walked past Settlers on the road.
        //
        // ⚠ THIS CHANGES EVERY SIMULATED GAME, deliberately. Any measurement
        // that priced a settler-protection treatment before this date was
        // pricing it against an opponent that did not take settlers, and read
        // as noise for that reason. The raid leash still applies below, so this
        // is a barbarian that hunts what is in its own raid ring, not one that
        // chases a walker across the map.
        let barbarian_hunter = self.barb && self.barbarian_tactics;
        let major_rescue = self.barbarian_settler_capture && !barbarian_hunter;
        if (!barbarian_hunter && !major_rescue)
            || g.rules.units[g.units[&uid].kind].class != "military"
        {
            return false;
        }
        let origin = g.units[&uid].pos;
        let reach = g.units[&uid].moves_left.floor() as i32;
        // The barbarian's contract is unchanged: no step with under two
        // movement. A major with one point left still takes the step that
        // puts it beside tomorrow's capture.
        if reach < if major_rescue { 1 } else { 2 } {
            return false;
        }
        // A unit standing on or beside one of our own settlers is plausibly
        // its escort; rescuing one civilian must not expose another.
        let beside_own_settler = g.nbrs(origin).into_iter().chain([origin]).any(|position| {
            g.unit_ids_at(position).iter().any(|other| {
                *other != uid && g.units[other].owner == pid && g.units[other].kind == "settler"
            })
        });
        if beside_own_settler {
            return false;
        }
        // The only soldier in one of our cities does not leave it for a walk
        // of several turns; it still takes a capture it can make this turn.
        let lone_garrison = g
            .city_at(origin)
            .is_some_and(|cid| g.cities[&cid].owner == pid)
            && g.unit_ids_at(origin)
                .iter()
                .filter(|other| {
                    let other = &g.units[other];
                    other.owner == pid && g.rules.units[other.kind].class == "military"
                })
                .count()
                <= 1;
        let far_horizon = if major_rescue && !lone_garrison {
            reach.max(BARBARIAN_SETTLER_PURSUIT_RADIUS)
        } else {
            reach
        };
        let barb_rescue: Option<usize> = g.barb_pid;
        let minor_gate = (self.minor && (!self.barb || !self.barbarian_tactics))
            .then(|| Self::minor_home(g, pid));
        let goal = g
            .units
            .values()
            .filter_map(|other| {
                if other.owner == pid || !g.is_at_war(pid, other.owner) {
                    return None;
                }
                let value = match other.kind.as_str() {
                    "settler" if barb_rescue == Some(other.owner) => 4,
                    "settler" if !decline_settlers => 3,
                    "builder" => 2,
                    _ if barbarian_hunter
                        && self.barbarian_heretic_hunt
                        && g.rules.units[other.kind].class == "religious" =>
                    {
                        1
                    }
                    _ => return None,
                };
                if let Some(home) = minor_gate {
                    if home.is_none_or(|home| g.wdist(home, other.pos) > MINOR_DEFENSE_RADIUS) {
                        return None;
                    }
                }
                let distance = g.wdist(origin, other.pos);
                // Only a barbarian-held settler is worth the longer walk.
                let horizon = if value == 4 { far_horizon } else { reach };
                if distance < 2 || distance > horizon {
                    return None;
                }
                // The raid ring is what keeps a successful raid from becoming
                // an all-map chase; a hunted civilian is not an exemption from
                // it. See `barbarian_target_allowed_for_controller`.
                if !self.barbarian_target_allowed_for_controller(g, uid, other.pos) {
                    return None;
                }
                // An incidental pickup is not a city assault. Majors leave
                // civilians under known bombardment to the tactical planner;
                // barbarian raids retain their existing behavior.
                if major_rescue
                    && g.cities.values().any(|city| {
                        Self::city_centre_strikes(g, pid, city, other.pos)
                            || Self::city_encampment_strikes(g, pid, city, other.pos)
                    })
                {
                    return None;
                }
                // A civilian standing under an enemy military unit cannot be
                // captured by entering; that tile is an attack problem.
                let guarded = g.unit_ids_at(other.pos).iter().any(|oid| {
                    let blocker = &g.units[oid];
                    blocker.owner != pid && g.rules.units[blocker.kind].class == "military"
                });
                if guarded {
                    return None;
                }
                Some((
                    value,
                    std::cmp::Reverse(distance),
                    std::cmp::Reverse(other.pos),
                ))
            })
            .max()
            .map(|(_, _, std::cmp::Reverse(position))| position);
        let Some(goal) = goal else {
            return false;
        };
        let Some(next) = g
            .route_step(uid, goal, 0)
            .filter(|next| g.can_move(uid, *next))
        else {
            return false;
        };
        // A safe prize can still have a dangerous approach. Recheck each
        // issued step, since native movement may end before reaching the goal.
        if major_rescue
            && g.cities.values().any(|city| {
                Self::city_centre_strikes(g, pid, city, next)
                    || Self::city_encampment_strikes(g, pid, city, next)
            })
        {
            return false;
        }
        let kind = g.units[&uid].kind.as_str();
        think!(self.journal, Military, Decision,
               "{kind} {uid} marches to rescue a capturable civilian";
               "an undefended enemy civilian is {far_horizon} or fewer tiles away; \
                walking onto it captures it, and a settler taken back from \
                the barbarians repays its whole production";
               goal);
        if !self.path_move(g, pid, uid, next) {
            return false;
        }
        // A raider that lands on a religious unit with movement to spare
        // finishes the hunt in the same step.
        self.condemn_heretic_here(g, pid, uid);
        true
    }

    /// Minors guard home; majors explore, then garrison the nearest city.
    ///
    /// `at_war` is threaded in because `military_step` *falls through to here*
    /// when it can find no objective — which at war is the common case, since
    /// `nearest_enemy_for_unit` needs a visible enemy and a besieging stack is
    /// routinely fogged. The old call site passed a hardcoded `false`, so
    /// `should_explore`'s opening `if at_war { return false }` was dead by
    /// construction and a unit whose homeland was under attack went
    /// exploring. Off, the historical `false` is preserved exactly.
    pub(super) fn peacetime_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        at_war: bool,
    ) -> bool {
        let upos = g.units[&uid].pos;
        let at_war = at_war && self.come_ashore;
        if self.minor && (!self.barb || !self.barbarian_tactics) {
            let cities = g.player_city_ids(pid);
            if cities.is_empty() {
                return false;
            }
            let cap = g.cities[&cities[0]].pos;
            if g.wdist(upos, cap) > 2 {
                return self.step_toward(g, pid, uid, cap);
            }
            return self.fortify_or_stop(g, pid, uid);
        }
        if self.barb && self.barbarian_tactics {
            let Some(home) = Self::barbarian_home(g, uid) else {
                return self.fortify_or_stop(g, pid, uid);
            };
            if g.wdist(upos, home) > 1 {
                return self.step_toward(g, pid, uid, home);
            }
            return self.fortify_or_stop(g, pid, uid);
        }
        // `garrison-before-the-surprise`: a stronger major's soldier near a
        // city with no soldier drafts the nearest idle one home, ahead of
        // every errand below.
        if let Some(acted) = self.garrison_before_the_surprise_step(g, pid, uid) {
            return acted;
        }
        // An adjacent empty camp is a completed objective, not a route to
        // price: enter it before this otherwise peaceful unit starts a longer
        // village, escort, or patrol task.
        if self.clear_adjacent_empty_barbarian_camp(g, pid, uid) {
            return true;
        }
        // Tribal villages expire the instant another civilization reaches
        // them. Let the chosen Scout — or the bounded military fallback when
        // no Scout can take it — collect before this unit receives a generic
        // escort, improvement, or patrol assignment. Civilians never enter
        // this branch; their dedicated steps retain their local jobs.
        if self.village_seeking && self.village_collection_step(g, pid, uid) {
            return true;
        }
        if let Some(target) = self.naval_escort_objective(g, pid, uid) {
            if target != upos && self.step_toward(g, pid, uid, target) {
                return true;
            }
            if g.units[&uid].linked_to.is_some_and(|peer| {
                g.units
                    .get(&peer)
                    .is_some_and(|unit| unit.kind == "settler")
            }) {
                return self.fortify_or_stop(g, pid, uid);
            }
        }
        let special_improver = {
            let unit = &g.units[&uid];
            unit.owner == pid && unit.charges > 0 && !g.rules.units[unit.kind].builds.is_empty()
        };
        if special_improver {
            if let Some(acted) = self.special_improver_step(g, pid, uid) {
                return acted;
            }
        }
        // Coming ashore outranks exploring. A Recon unit explores for as long
        // as any unseen tile remains, which on an ocean map is forever, so
        // Scouts and Skirmishers walked off across the water and never came
        // back: 11% of every empire's land army ended its games embarked,
        // where no upgrade is offered at all. Only that case jumps the queue —
        // a unit merely away from home still explores first and modernizes
        // afterwards, because taking recon off the map early costs more than
        // the delayed upgrade is worth.
        if g.is_embarked(&g.units[&uid]) && self.modernization_step(g, pid, uid) {
            return true;
        }
        // ...and coming ashore has to outrank exploring for the units that
        // have no upgrade waiting too, which is most of them. See
        // `disembark_step`.
        if self.come_ashore && self.disembark_step(g, pid, uid) {
            return true;
        }
        if self.should_explore(g, pid, uid, at_war) && self.explore_step(g, pid, uid) {
            return true;
        }
        if self.modernization_step(g, pid, uid) {
            return true;
        }
        // `retainers-garrison-the-short-cities`: an idle soldier garrisons
        // an Amenity-short city while Retainers is slotted.
        if let Some(acted) = self.retainers_garrison_step(g, pid, uid) {
            return acted;
        }
        if self.patrol_step(g, pid, uid) {
            return true;
        }
        self.fortify_or_stop(g, pid, uid)
    }

    /// Walk a unit that has outlived its era back inside the borders, where
    /// it can be upgraded. Gold upgrades are only offered in friendly
    /// territory, so a unit that spends its whole life on frontier patrol or
    /// in no-man's-land can never modernize however rich its owner is; the
    /// modernization pass simply never sees it. Only units whose successor is
    /// already unlocked and paid for make the trip.
    ///
    /// An embarked unit is included deliberately: it is the case that most
    /// needs the trip, since no upgrade at all is offered at sea.
    pub(super) fn modernization_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let target = {
            let unit = &g.units[&uid];
            let upos = unit.pos;
            if g.unit_upgrade_target(pid, unit.kind).is_none() {
                return false;
            }
            let at_home = g
                .map
                .get(upos)
                .and_then(|tile| tile.owner_city)
                .and_then(|cid| g.cities.get(&cid))
                .is_some_and(|city| city.owner == pid);
            if at_home {
                return false; // already somewhere the upgrade can be taken
            }
            let Some((_, gold, _)) = g.unit_upgrade_price(pid, &unit.kind) else {
                return false;
            };
            if g.players[pid].gold < gold {
                return false; // no point marching home to an empty treasury
            }
            g.player_city_ids(pid)
                .into_iter()
                .map(|cid| g.cities[&cid].pos)
                .min_by_key(|pos| (g.wdist(upos, *pos), *pos))
        };
        match target {
            Some(pos) => self.step_toward(g, pid, uid, pos),
            None => false,
        }
    }
}

#[cfg(test)]
mod fully_charted_tests;
