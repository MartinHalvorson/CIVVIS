//! Settling: where `AdvancedAi` founds its cities and how it gets the
//! Settler there alive.
//!
//! `advanced_settler_step` (through `advanced_settler_step_inner`) moves each
//! Settler. Site choice is `best_settler_target`, the shared site scan
//! (`settle_sites_scanning_shared`), `best_reachable_settle_site_except` and
//! the site values (`settle_value`, the static and atlas values, adjacency,
//! defensibility, foreign-border and city-state exclusions). Getting there is
//! walk costs, the visible-threat route check and detour, and the safe-step
//! helpers. Keeping the Settler alive is the guard bindings,
//! `settler_escort_step`, `stacked_guard_step` and `stacked_escort_pace`.
//! The early-settler home memory and `founds_where_it_stands` finish it.
//!
//! Moved verbatim out of the one `impl AdvancedAi` block in `advanced.rs`
//! (2026-11-04). Methods the rest of the controller calls are `pub(super)`;
//! the code is otherwise unchanged. The scoring behind `settle_value` is in
//! `settlement_value.rs`.

use super::*;

impl AdvancedAi {
    /// Does this turn resolve the delay `settler_blocked_turns` measures?
    ///
    /// ⚠ Named rather than inlined because the inline version was one operand of
    /// an `||` shared with `settler_stalls`, and the two counters want OPPOSITE
    /// answers on a retarget. `settler_stalls` bounds commitment to one target, so
    /// a retarget must reset it. `settler_blocked_turns` measures total unresolved
    /// delay and, by its own documentation, "survives a target change so a stranded
    /// civilian stops monopolizing the empire-wide in-flight allowance".
    ///
    /// Only progress toward the SAME target resolves anything. A stranded settler
    /// retargets every turn precisely because it can reach nothing, so treating a
    /// retarget as progress cleared the counter every turn and
    /// `settler_in_flight_allowed` never saw it cross
    /// `SETTLER_REPLACEMENT_BLOCKED_TURNS`.
    pub(super) fn settler_delay_is_resolved(retargeted: bool, closer: bool) -> bool {
        closer && !retargeted
    }

    /// Price the future district economy through the engine's canonical
    /// adjacency calculator. The calculator treats the proposed center as a
    /// city and counts planned foundations, while this wrapper keeps the
    /// delayed district yields half-weighted and capped in the site score.
    pub(super) fn settlement_adjacency_value(&self, g: &Game, pid: usize, pos: Pos) -> f64 {
        if !self.adjacency_site_planning {
            return 0.0;
        }
        let positions = g.wdisk(pos, 2);
        self.settlement_adjacency_value_from_positions(g, pid, pos, &positions)
    }

    /// The historical score retained for the frozen Advanced control and for
    /// the old `defensible_sites` experiment. Keeping this branch explicit
    /// prevents the new default from silently rewriting the control arm.
    pub(super) fn legacy_settle_value(&self, g: &Game, pid: usize, pos: Pos) -> f64 {
        let tile = &g.map.tiles[&pos];
        let mut value = 0.0;
        for p in g.wdisk(pos, 2) {
            let Some(t) = g.map.get(p) else { continue };
            if t.owner_city.is_some() && p != pos {
                continue;
            }
            let y = g.rules.tile_yields(t);
            let ring_discount = if g.wdist(pos, p) <= 1 { 1.0 } else { 0.45 };
            value += ring_discount * Self::settlement_yield_value(y);
            if let Some(resource) = &t.resource {
                value += match g.rules.resources[resource].class.as_str() {
                    "luxury" => 5.0,
                    "strategic" => 4.0,
                    _ => 1.5,
                } * ring_discount;
            }
        }
        value += self.coastal_city_site_bonus(g, pos, &g.wdisk(pos, 2));
        let fresh = tile.has_river()
            || g.nbrs(pos).iter().any(|p| {
                g.map
                    .get(*p)
                    .is_some_and(|t| t.feature.as_deref() == Some("oasis"))
            });
        let coastal = g
            .nbrs(pos)
            .iter()
            .any(|p| g.map.get(*p).is_some_and(|t| g.rules.is_water(t)));
        value += if fresh {
            14.0
        } else if coastal {
            6.0
        } else {
            -5.0
        };
        value += self.settlement_adjacency_value(g, pid, pos);
        let enemy_distance = g
            .cities
            .values()
            .filter(|c| c.owner != pid && !g.players[c.owner].is_barbarian)
            .map(|c| g.wdist(pos, c.pos))
            .min()
            .unwrap_or(20);
        if enemy_distance < 6 {
            value -= (6 - enemy_distance) as f64 * 6.0;
        }
        if self.defensible_sites {
            value += self.defensibility(g, pid, pos);
        }
        value
    }

    pub(super) fn settle_value(&self, g: &Game, pid: usize, pos: Pos) -> f64 {
        if !self.settlement_safety {
            return self.legacy_settle_value(g, pid, pos);
        }
        let visible = self.battlefront_visibility(g, pid);
        self.settle_value_visible(g, pid, pos, &visible)
    }

    /// The live bridge's host safety layer can refuse a Settler's next leg
    /// even when the native route scorer discounts a stacked escort.  Keep a
    /// bounded route-recovery policy active for that bridge-only case: the
    /// capture-lessons treatment already identifies the host's civilian
    /// semantics, while `settlement_safety` keeps the repair out of the
    /// historical and evaluator controllers.
    pub(super) fn settler_routing_recovery_on(&self) -> bool {
        self.live_settler_capture_lessons && self.settlement_safety
    }

    pub(super) fn settler_threat_detour_on(&self) -> bool {
        self.settler_threat_detour || self.settler_routing_recovery_on()
    }

    pub(super) fn settler_target_hysteresis_on(&self) -> bool {
        self.settler_target_hysteresis
            || self.settler_target_hysteresis_2
            // The deployment bridge's watchdog (`settler-never-idles`) is
            // the bounded recovery owner.  Keeping this conjunct means a
            // screen's explicit hysteresis-withholding arm remains a true
            // ablation even when it enables the broader live universe.
            || (self.settler_never_idles && self.settler_routing_recovery_on())
    }

    /// Whether a cached-target failure is intrinsic to the site, rather than
    /// depending on this Settler's route, its guard, or the visible threat
    /// frame. Version-2 hysteresis may share only these failures: its memory is
    /// consulted by every Settler, while safety pricing deliberately depends on
    /// the mover's support.
    pub(super) fn settler_target_drop_is_empire_wide(why: &str) -> bool {
        matches!(
            why,
            "the tile left the map"
                | "water"
                | "impassable"
                | "a city within three tiles"
                | "another empire's ground"
                | "the host blocked the plot"
                | "a settler was lost within three tiles"
        )
    }

    /// The first six legal founding rings beside a revealed city-state are not
    /// a safe frontier. A rival Suzerain can turn the city-state hostile without
    /// warning, then use the city-state's mobile siege stack to take an
    /// un-walled colony before it can prepare a defence. A city center the seat
    /// has already charted remains evidence after fog closes over it.
    ///
    /// This is part of the standard `settlement_safety` policy rather than an
    /// opt-in defence gene: it prevents the exposed city from being founded,
    /// while preserving fog honesty and the frozen legacy controller.
    pub(super) fn city_state_settlement_exclusion(&self, g: &Game, pid: usize) -> BTreeSet<Pos> {
        if !self.settlement_safety {
            return BTreeSet::new();
        }
        let mut excluded = g
            .cities
            .values()
            .filter(|city| {
                city.owner != pid
                    && g.players
                        .get(pid)
                        .is_some_and(|player| player.explored.contains(&city.pos))
                    && g.players.get(city.owner).is_some_and(|player| {
                        player.alive && player.is_minor && !player.is_barbarian
                    })
            })
            .flat_map(|city| g.wdisk(city.pos, CITY_STATE_SETTLEMENT_BUFFER))
            .collect::<BTreeSet<_>>();
        for site in self.air_resource_settlement_sites(g, pid, &excluded) {
            excluded.remove(&site);
        }
        excluded
    }

    pub(super) fn settlement_route_risk(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        path: &[Pos],
        visible: &TileBits,
    ) -> (f64, f64) {
        let mut previous = g.units[&uid].pos;
        let mut movement_cost = 0.0;
        let mut risk = 0.0;
        for (index, position) in path.iter().enumerate() {
            movement_cost += g.step_cost(previous, *position);
            let tile_risk = self.settlement_tile_risk(g, pid, Some(uid), *position, visible);
            risk += tile_risk * if index < 3 { 1.15 } else { 0.75 };
            previous = *position;
        }
        (movement_cost, risk)
    }

    /// Follow the chosen route unless its next tile is in a visible direct
    /// attack envelope. In that case try a legal lower-risk sidestep that
    /// still makes progress; if none exists, hold and let the target search
    /// reconsider rather than donating the settler to the threat.
    pub(super) fn settler_step_toward_safe(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> bool {
        let Some(waypoint) = self.settler_refusal_waypoint(g, uid, target) else {
            return false;
        };
        self.settlement_unit_step_toward_safe(g, pid, uid, Some(uid), false, waypoint)
    }

    /// Move either a Settler or the military leader of its linked formation
    /// without walking the civilian into a visible capture envelope.
    ///
    /// `risk_uid` is the actual mover. An outmatched formation leader uses raw
    /// civilian capture risk: being linked does not make its Settler expendable,
    /// while roughly peer-strength escorts retain the ordinary movement behavior.
    /// A live Firaxis run lost three Settlers this way; the first strength-5
    /// Slinger walked its Settler beside visible strength-20 Warriors because the
    /// escort path used the generic mover and bypassed this check entirely.
    pub(super) fn settlement_unit_step_toward_safe(
        &self,
        g: &mut Game,
        pid: usize,
        moving_uid: u32,
        risk_uid: Option<u32>,
        formation_leader: bool,
        target: Pos,
    ) -> bool {
        let move_without_guard = |g: &mut Game| {
            if g.units[&moving_uid].kind == "settler" {
                self.base.settler_step_toward(g, pid, moving_uid, target)
            } else {
                self.base.step_toward(g, pid, moving_uid, target)
            }
        };
        if !self.settlement_safety {
            return move_without_guard(g);
        }
        let Some(next) = g
            .route_step(moving_uid, target, 0)
            .filter(|next| g.can_move(moving_uid, *next))
        else {
            return move_without_guard(g);
        };
        let visible = self.battlefront_visibility(g, pid);
        let formation_outmatched = formation_leader
            && g.units.values().any(|unit| {
                if unit.owner == pid
                    || !g.is_at_war(pid, unit.owner)
                    || !g.sees(&visible, unit.pos)
                    || !g.unit_visible_to(unit.id, pid)
                {
                    return false;
                }
                let spec = &g.rules.units[unit.kind];
                if spec.class != "military" || matches!(spec.domain.as_deref(), Some("sea" | "air"))
                {
                    return false;
                }
                let attack_range = if spec.has_ranged_attack() {
                    g.unit_attack_range(unit.id)
                } else {
                    1
                };
                let attack_reach = attack_range + spec.moves.ceil() as i32;
                let attacker =
                    crate::game::effective_strength(g.unit_strength(unit, false), unit.hp);
                let escort = &g.units[&moving_uid];
                let defender =
                    crate::game::effective_strength(g.unit_strength(escort, false), escort.hp);
                g.wdist(unit.pos, next) <= attack_reach && attacker > defender * 1.5
            });
        let planned_risk = self.settlement_tile_risk_with_support(
            g,
            pid,
            risk_uid,
            next,
            &visible,
            !formation_outmatched,
        );
        if planned_risk <= SETTLER_STEP_RISK_LIMIT || (formation_leader && !formation_outmatched) {
            // Keep ordinary movement behavior unchanged. This guard intervenes
            // only when an outmatched escort's route enters a capture envelope.
            return move_without_guard(g);
        }

        let current = g.units[&moving_uid].pos;
        let current_distance = g.wdist(current, target);
        let mut alternatives = g
            .nbrs(current)
            .into_iter()
            .filter(|position| {
                *position != next
                    && g.map.get(*position).is_some()
                    && g.can_move(moving_uid, *position)
            })
            .map(|position| {
                let risk = self.settlement_tile_risk_with_support(
                    g,
                    pid,
                    risk_uid,
                    position,
                    &visible,
                    !formation_outmatched,
                );
                let progress = current_distance - g.wdist(position, target);
                let score =
                    progress as f64 * 8.0 - risk * 1.5 - g.step_cost(current, position) * 0.5;
                (score, risk, position, progress)
            })
            .collect::<Vec<_>>();
        alternatives.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.2.cmp(&b.2)));
        for &(_, risk, position, progress) in &alternatives {
            if risk + 5.0 >= planned_risk || (progress < 0 && risk > planned_risk * 0.5) {
                continue;
            }
            if self.base.path_move(g, pid, moving_uid, position) {
                return true;
            }
        }
        // `standing-still-is-a-risk`: every neighbour was refused, so the
        // guard is about to leave the unit where it stands — a tile whose own
        // risk it has never measured. Price staying the same way, and take
        // the safest reachable neighbour that strictly improves on it.
        // Safety orders the pick; progress toward the target breaks ties.
        if self.standing_still_is_a_risk {
            let staying = self.settlement_tile_risk_with_support(
                g,
                pid,
                risk_uid,
                current,
                &visible,
                !formation_outmatched,
            );
            // Only a tile that is ITSELF over the limit is worth leaving. A
            // standstill on safe ground is the guard working: the route is
            // dangerous and waiting costs only tempo. Firing there traded
            // tempo for nothing and read -8.3 pp on a 24-game probe, against
            // +2.81 done Δpp on the same batch — the mechanism was right and
            // the trigger was too wide.
            if staying <= SETTLER_STEP_RISK_LIMIT {
                return false;
            }
            let mut safer: Vec<&(f64, f64, Pos, i32)> = alternatives
                .iter()
                .filter(|(_, risk, _, _)| *risk < staying)
                .collect();
            safer.sort_by(|a, b| a.1.total_cmp(&b.1).then(b.3.cmp(&a.3)).then(a.2.cmp(&b.2)));
            for &&(_, _, position, _) in &safer {
                if self.base.path_move(g, pid, moving_uid, position) {
                    return true;
                }
            }
        }
        false
    }

    /// `settle_sites_scanning` for a caller that scans the same board for
    /// the same seat from more than one origin; see `SettlementScanShared`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn settle_sites_scanning_shared(
        &self,
        g: &Game,
        pid: usize,
        from: Pos,
        radius: i32,
        prefilter_limit: Option<usize>,
        mut score_cache: Option<&mut BTreeMap<Pos, f64>>,
        stop_at_first: bool,
        include_emergency: bool,
        shared: Option<&mut SettlementScanShared>,
    ) -> Vec<(Pos, f64)> {
        let mut sites = Vec::new();
        let mut emergency_sites = Vec::new();
        let visible = self.battlefront_visibility(g, pid);
        let distance_penalty = if self.settlement_safety {
            if radius > 12 {
                0.78
            } else {
                1.25
            }
        } else if radius > 12 {
            0.45
        } else {
            SETTLE_DISTANCE_PENALTY
        };
        // The old candidate loop asked every city whether it was within the
        // four-tile founding exclusion radius. Global searches can visit the
        // whole Ludicrous map, so materialize that small union once instead
        // of repeating a city scan for every tile. The union was built only
        // for `radius > 12`, which left the two radii the settler path
        // actually walks — the radius-8 local pass and the radius-11 scan the
        // Settler production arm runs for every city — paying an O(cities)
        // `wdist` scan per plot of the disk. `wdisk(pos, 3)` is exactly the
        // set of plots at `wdist <= 3`, so the union answers the same
        // question at every radius.
        // A caller scanning from several origins hands in the answers the
        // previous origin already derived (`SettlementScanShared`); a lone
        // scan derives its own, exactly as before.
        let mut own_exclusions = None;
        let (exclusions, mut prefilter_cache) = match shared {
            Some(shared) => (&mut shared.exclusions, Some(&mut shared.prefilter)),
            None => (&mut own_exclusions, None),
        };
        let (city_exclusion, city_state_exclusion) = exclusions.get_or_insert_with(|| {
            (
                g.cities
                    .values()
                    .flat_map(|city| g.wdisk(city.pos, 3))
                    .collect::<BTreeSet<_>>(),
                self.city_state_settlement_exclusion(g, pid),
            )
        });
        let mut candidates = g
            .wdisk(from, radius)
            .into_iter()
            .filter_map(|pos| {
                let tile = g.map.get(pos)?;
                // A site the HOST engine refused is not settleable however good it looks;
                // see `Game::blocked_city_sites`, which is empty in an ordinary game.
                if g.blocked_city_sites.contains(&pos)
                    || g.rules.is_water(tile)
                    || !g.rules.is_passable(tile)
                    || g.tile_is_natural_wonder(tile)
                    || city_exclusion.contains(&pos)
                    || city_state_exclusion.contains(&pos)
                    || tile.owner_city.is_some_and(|cid| {
                        // A tile can name a city absent from the current table.
                        // Keep that unresolved claim out of settlement candidates.
                        g.cities.get(&cid).is_none_or(|city| city.owner != pid)
                    })
                {
                    return None;
                }
                let prefilter_score = prefilter_limit
                    .map(|_| {
                        if let Some(cached) = prefilter_cache
                            .as_deref()
                            .and_then(|cache| cache.get(&pos).copied())
                        {
                            return cached;
                        }
                        // `contested_land_first`: a contested site is never
                        // cut before it is priced.
                        let score = self
                            .settlement_prefilter_score_with_water_science_wonder(g, pid, pos)
                            + self.contested_land_credit(g, pid, pos);
                        if let Some(cache) = prefilter_cache.as_deref_mut() {
                            cache.insert(pos, score);
                        }
                        score
                    })
                    .unwrap_or(0.0);
                Some((pos, prefilter_score))
            })
            .collect::<Vec<_>>();
        let mut overflow = Vec::new();
        if let Some(limit) = prefilter_limit {
            candidates.sort_by(|left, right| right.1.total_cmp(&left.1).then(left.0.cmp(&right.0)));
            if candidates.len() > limit {
                overflow = candidates.split_off(limit);
            }
        }
        // ⚠ The batch prefill, not the scoring loop, is what a ranking scan
        // pays for: it computes the static value of EVERY candidate up front,
        // and that value is a growth forecast over each site's radius-2 disk.
        // An existence query stops at the first qualifying site, so prefilling
        // all ~154 of them is work it can never read. Leave those positions to
        // `settlement_atlas_value`'s own on-demand fill, which caches into the
        // same atlas and so still serves a later ranking scan this turn.
        //
        // ★★★★ THE PREFILTER ONLY SAVES WHAT IT DOES NOT PRE-VALUE. The split
        // above exists so the expensive value is paid for the top `limit`
        // candidates alone, and the batch handed the overflow straight back —
        // every plot the prefilter had just excluded was valued anyway, one
        // 19-plot × districts adjacency sweep each. The overflow is read only
        // by the `sites.is_empty()` fallback below, which reaches
        // `settlement_atlas_value` per plot on demand and computes the
        // identical number there. Same reasoning as the line above, one gate
        // further in.
        if self.settlement_safety && self.battlefront_frame.is_some() && !stop_at_first {
            let atlas_positions = candidates.iter().map(|(pos, _)| *pos).collect::<Vec<_>>();
            self.settlement_atlas_values(g, pid, &atlas_positions);
        }
        let mut score_site = |pos| {
            // The local and global radius passes in `best_settler_target` can
            // contain the same plot. Their distance penalties differ, but
            // the expensive site value does not, so share just that exact
            // value within the immutable decision. The cache never crosses
            // an action or a turn.
            let site_value = if let Some(value) = score_cache
                .as_deref_mut()
                .and_then(|cache| cache.get(&pos).copied())
            {
                value
            } else {
                let value = if self.settlement_safety {
                    self.settle_value_visible(g, pid, pos, &visible)
                } else {
                    self.legacy_settle_value(g, pid, pos)
                };
                if let Some(cache) = score_cache.as_deref_mut() {
                    cache.insert(pos, value);
                }
                value
            };
            let distance = g.wdist(from, pos);
            let overreach = if self.settlement_safety {
                distance.saturating_sub(8) as f64 * SETTLE_DISTANCE_PENALTY
            } else {
                0.0
            };
            let value = site_value - distance as f64 * distance_penalty - overreach;
            if value >= 12.0 {
                Some((pos, value, true))
            } else if self.settlement_safety
                && radius > 12
                && value >= SETTLEMENT_EMERGENCY_SITE_MIN
            {
                Some((pos, value, false))
            } else {
                None
            }
        };
        for (pos, _) in candidates {
            if let Some((pos, value, normal)) = score_site(pos) {
                if normal {
                    sites.push((pos, value));
                    // An existence query is already answered; the remaining
                    // candidates can only change which site is best, and no
                    // caller of this path asks that.
                    if stop_at_first {
                        return sites;
                    }
                } else {
                    emergency_sites.push((pos, value));
                }
            }
        }
        // A cheap local estimate can miss every worthwhile site on a sparse
        // map. Pay the full scan only in that failure case, preserving the
        // existing fallback behavior without taxing ordinary global searches.
        if sites.is_empty() {
            for (pos, _) in overflow {
                if let Some((pos, value, normal)) = score_site(pos) {
                    if normal {
                        sites.push((pos, value));
                        if stop_at_first {
                            return sites;
                        }
                    } else {
                        emergency_sites.push((pos, value));
                    }
                }
            }
        }
        if sites.is_empty() || include_emergency {
            sites.extend(emergency_sites);
        }
        if self.campus_adjacency_threshold_2 && !stop_at_first && sites.len() > 1 {
            self.campus_threshold_settle_rerank(g, &mut sites);
        }
        sites.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
        sites
    }

    /// `campus_adjacency_threshold_2`: credit each of the best
    /// `CAMPUS_THRESHOLD_SETTLE_CANDIDATES` sites `CAMPUS_THRESHOLD_SETTLE_SHARE` of
    /// its own value when a plot in its first three rings could host a Campus
    /// at raw Science adjacency of `CAMPUS_MULTIPLIER_ADJACENCY_THRESHOLD` —
    /// the plot the district pricing later finds or does not find. Never
    /// lowers a site.
    pub(super) fn campus_threshold_settle_rerank(&self, g: &Game, sites: &mut [(Pos, f64)]) {
        sites.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
        let top = sites.len().min(CAMPUS_THRESHOLD_SETTLE_CANDIDATES);
        for site in sites.iter_mut().take(top) {
            if site.1 > 0.0 && Self::campus_threshold_plot_near(g, site.0) {
                site.1 += site.1 * CAMPUS_THRESHOLD_SETTLE_SHARE;
            }
        }
    }

    /// Whether a plot within three tiles of `site` (not the site itself)
    /// could carry a Campus at the multiplier's raw adjacency. The plot test
    /// is the city-free half of `Game::district_sites`: land, passable, no
    /// district, wonder, district-blocking feature or non-bonus resource.
    pub(super) fn campus_threshold_plot_near(g: &Game, site: Pos) -> bool {
        let campus = crate::name!("campus");
        g.wdisk(site, 3).into_iter().any(|pos| {
            if pos == site {
                return false;
            }
            let Some(tile) = g.map.get(pos) else {
                return false;
            };
            if g.rules.is_water(tile)
                || !g.rules.is_passable(tile)
                || tile.district.is_some()
                || tile.district_foundation.is_some()
                || tile.wonder.is_some()
                || tile.feature.as_ref().is_some_and(|feature| {
                    g.rules
                        .features
                        .get(feature.as_str())
                        .is_some_and(|feature| feature.natural_wonder || feature.blocks_district)
                })
                || tile.resource.as_ref().is_some_and(|resource| {
                    !matches!(
                        g.rules.resources[resource].class.as_str(),
                        "bonus" | "artifact"
                    )
                })
            {
                return false;
            }
            let mut raw = 0.0;
            for source in g.district_adjacency_sources(campus, pos) {
                if source.source != "adjacency_bonus" {
                    raw += source.yields.science;
                }
            }
            raw >= CAMPUS_MULTIPLIER_ADJACENCY_THRESHOLD
        })
    }

    pub(super) fn best_reachable_settle_site_except(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        radius: i32,
        avoid: Option<Pos>,
    ) -> Option<(Pos, f64)> {
        let mut score_cache = BTreeMap::new();
        let mut walk_costs = SettlerWalkCosts::new();
        self.best_reachable_settle_site_except_cached(
            g,
            pid,
            uid,
            radius,
            avoid,
            &mut score_cache,
            &mut walk_costs,
        )
    }

    // Two caches, one per shared derivation, beside the ranking's own six
    // inputs; the callers that hold both are the ones documented on
    // `settler_walk_costs_cached`.

    #[allow(clippy::too_many_arguments)]
    pub(super) fn best_reachable_settle_site_except_cached(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        radius: i32,
        avoid: Option<Pos>,
        score_cache: &mut BTreeMap<Pos, f64>,
        walk_costs: &mut SettlerWalkCosts,
    ) -> Option<(Pos, f64)> {
        let from = g.units[&uid].pos;
        let mut candidates = self
            .settle_sites_with_limit_cached(
                g,
                pid,
                from,
                radius,
                (self.settlement_safety && radius > 12)
                    .then_some(SETTLEMENT_GLOBAL_PREFILTER_LIMIT),
                Some(score_cache),
            )
            .into_iter()
            .filter(|(position, value)| {
                Some(*position) != avoid
                    && self.early_settler_site_allowed(g, pid, uid, *position)
                    && !self.settler_site_is_dead(uid, *position)
                    && (!self.settler_threat_detour_on()
                        || !self.settler_threat_deferrals.contains_key(position))
                    && !self.settler_target_reserved_by_other(g, pid, uid, *position)
                    // `settler-target-floor`: not worth the walk is not a target.
                    && self.settler_target_clears_floor(g, from, *position, *value)
            })
            .collect::<Vec<_>>();
        // ★★★★ THE CAPITAL'S WALK IS PRICED IN TURNS, AT THE GAME'S SPEED.
        //
        // Before this, the first Settler walked to any site worth +3 within
        // two hexes (`current_value + 3.0` in `settler_step`), whatever the
        // walk cost in turns. Live `civvis-20260901T212354Z` (Online, 250
        // turns): a site worth 117 against 86 in place, two floodplain hexes
        // away, founded the capital on turn 6 — every turn without a capital
        // is a turn of no yields, no research and no production, and on
        // Online each is a larger share of the game than on Standard. The
        // operator's rule: Online settles on turn 1 or 2, turn 3 at a
        // stretch; slower speeds may spend a few more. See
        // `opening_walk_budget` for the table and `opening_walk_pays` for the
        // price; both are gated to the FIRST city of a Settler that can found
        // where it stands, so a Settler that cannot found in place is never
        // left without a candidate. It rides the `settle-sooner` gene — the
        // gene that already prices every Settler's walk in turns — which the
        // deployment genome and the live seat carry and `legacy()` does not,
        // so the Elo anchor's play is untouched
        // (`advanced_v1_plays_the_same_game_it_always_did`).
        if self.settle_sooner && g.player_city_ids(pid).is_empty() && g.can_found_city(uid) {
            let stand = self.settle_value(g, pid, from);
            let costs = Self::settler_walk_costs_cached(g, uid, radius, walk_costs);
            let (budget, per_turn) = Self::opening_walk_budget(g.game_speed);
            // Priced on `settle_value` itself — the number the journal prints —
            // not on the scan's ranking score, which carries its own terms.
            candidates.retain(|(position, _)| {
                *position == from
                    || Self::opening_walk_pays(
                        stand,
                        self.settle_value(g, pid, *position),
                        Self::opening_walk_turns(g, uid, from, *position, costs),
                        budget,
                        per_turn,
                    )
            });
        }
        // See `settle_sooner`: the walk is priced in TURNS as well as tiles,
        // each turn dearer the longer this Settler has already been out, and
        // the price is applied to the ranking itself — `path_to` below is a
        // one-turn flood, so only this ordering reaches a site more than a
        // turn away (`first_reachable_settle_site` takes the first reachable
        // candidate in ranked order). Ground the walk cannot reach within the
        // radius is priced by hex distance at one point a step.
        let turn_price = self.settle_sooner_walk_price(g, uid);
        if let Some((_, moves)) = turn_price {
            let costs = Self::settler_walk_costs_cached(g, uid, radius, walk_costs);
            for (position, value) in candidates.iter_mut() {
                let movement_cost = costs
                    .get(position)
                    .copied()
                    .unwrap_or_else(|| g.wdist(from, *position) as f64 * moves.max(1.0) / 2.0);
                *value -= Self::settle_sooner_walk_cost(turn_price, movement_cost);
            }
            candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
        }
        if !self.settlement_safety {
            return BasicAi::first_reachable_settle_site(g, uid, &candidates);
        }
        let visible = self.battlefront_visibility(g, pid);
        // The one-turn routed branch: the candidate value already carries
        // the gene's walk price, so it is not charged again here.
        //
        // `path_to` builds a full movement flood from this unchanged Settler
        // position.  This loop used to repeat that identical flood for every
        // one of up to 160 scored sites; retain its exact path/tie behavior,
        // but build the paths once and look up each candidate.
        let paths = g.paths_to(uid);
        let mut routed = candidates
            .iter()
            .take(SETTLEMENT_ROUTE_CANDIDATE_LIMIT)
            .filter_map(|(position, site_value)| {
                let path = paths.get(position)?;
                let (movement_cost, risk) = self.settlement_route_risk(g, pid, uid, path, &visible);
                Some((*position, *site_value - movement_cost * 0.8 - risk))
            })
            .collect::<Vec<_>>();
        if routed.is_empty() {
            return BasicAi::first_reachable_settle_site(g, uid, &candidates);
        }
        routed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
        let near = routed.into_iter().next();
        // `settler-walks-to-the-better-site`: the one-turn pick yields to a
        // far better ranked site beyond this turn's walk. `None` with the
        // gene off. See `advanced/far_settle_site.rs`.
        if let Some(far) = near
            .and_then(|near| self.better_site_beyond_the_turn(g, uid, &candidates, &paths, near))
        {
            return Some(far);
        }
        near
    }

    /// Remember the capital that existed when a live Settler was first
    /// observed on its founding city while the empire still had only one
    /// city. The bridge rebuilds unit ids every turn, so this anchor is
    /// carried in the same remapped ledger as the target and capture memory.
    /// A later founding must not silently make the second Settler's home
    /// become whichever new city is nearest.
    pub(super) fn remember_early_settler_home(&mut self, g: &Game, pid: usize, uid: u32) {
        if !self.live_settler_capture_lessons
            || self.early_settler_homes.contains_key(&uid)
            || !g.units.get(&uid).is_some_and(|unit| {
                unit.owner == pid
                    && unit.kind == "settler"
                    && g.city_at(unit.pos)
                        .is_some_and(|city| g.cities[&city].owner == pid)
            })
        {
            return;
        }
        let cities = g.player_city_ids(pid);
        if cities.len() == 1 {
            self.early_settler_homes
                .insert(uid, g.cities[&cities[0]].pos);
        }
    }

    /// The capital anchor for a Settler born during the one-city opening.
    /// This is deliberately host-only: native boards do not have the live
    /// bridge's capture semantics that motivated the bounded home corridor.
    pub(super) fn early_settler_home(&self, uid: u32) -> Option<Pos> {
        self.live_settler_capture_lessons
            .then(|| self.early_settler_homes.get(&uid).copied())
            .flatten()
    }

    /// Whether a candidate site stays inside the early Settler's home
    /// corridor. Emergency threat movement may briefly leave the corridor,
    /// but deliberate target selection and exhaustion must not send it there.
    pub(super) fn early_settler_site_allowed(
        &self,
        g: &Game,
        _pid: usize,
        uid: u32,
        site: Pos,
    ) -> bool {
        self.early_settler_home(uid)
            .is_none_or(|home| g.wdist(home, site) <= EARLY_SETTLER_HOME_RADIUS)
    }

    /// A live early Settler that was pushed outside its home corridor has a
    /// non-negotiable objective: get back toward its original capital before
    /// choosing another site. Returning `None` means the Settler is still
    /// within the deliberate-march envelope.
    pub(super) fn early_settler_homeward_target(&self, g: &Game, uid: u32) -> Option<Pos> {
        let home = self.early_settler_home(uid)?;
        (g.wdist(g.units[&uid].pos, home) > EARLY_SETTLER_HOME_RADIUS).then_some(home)
    }

    /// Spend the turn bringing an early Settler back toward its capital. The
    /// live capture-aware route guard remains in charge of the actual step, so
    /// an emergency return cannot trade distance discipline for a capture.
    pub(super) fn return_early_settler_home(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        home: Pos,
    ) -> bool {
        let current = g.units[&uid].pos;
        self.settler_targets.remove(&uid);
        self.settler_relaxed_targets.remove(&uid);
        self.settler_stalls.remove(&uid);
        self.settler_closest.remove(&uid);
        self.settler_retreats.remove(&uid);
        think!(self.journal(), Expansion, Detail,
               "Settler returns toward its capital";
               "it is {} tiles from its early home at {:?}; \
                the live safety corridor is {} tiles, so expansion waits for the \
                return", g.wdist(current, home), home, EARLY_SETTLER_HOME_RADIUS; home);
        if self.formationless_settler_escort() && self.settlement_safety {
            self.settler_step_out_of_reach(g, pid, uid, home)
        } else {
            self.settler_step_toward_safe(g, pid, uid, home)
        }
    }

    /// The price of one turn of this Settler's walk and the Settler's own
    /// pace, under `settle_sooner`; `None` when the gene is off. The price
    /// begins at `SETTLE_SOONER_TURN_PRICE` and adds that base price for every
    /// `SETTLE_SOONER_PATIENCE` standard turns the Settler has already been
    /// walking (`settler_walk_started`, stamped by `advanced_settler_step`).
    pub(super) fn settle_sooner_walk_price(&self, g: &Game, uid: u32) -> Option<(f64, f64)> {
        if !self.settle_sooner {
            return None;
        }
        let waited = self
            .settler_walk_started
            .get(&uid)
            .map_or(0, |started| g.turn.saturating_sub(*started));
        let patience = g.standard_duration(SETTLE_SOONER_PATIENCE).max(1) as f64;
        let moves = g.unit_max_moves(uid).max(1.0);
        Some((
            SETTLE_SOONER_TURN_PRICE * (1.0 + waited as f64 / patience),
            moves,
        ))
    }

    /// What a route of `movement_cost` movement points costs under
    /// `settle_sooner`: the turns it takes at the Settler's pace, each at
    /// the price `settle_sooner_walk_price` set. Zero when the gene is off.
    pub(super) fn settle_sooner_walk_cost(
        turn_price: Option<(f64, f64)>,
        movement_cost: f64,
    ) -> f64 {
        turn_price.map_or(0.0, |(price, moves)| (movement_cost / moves).ceil() * price)
    }

    /// How many turns the first Settler spends reaching `target`: the flood's
    /// movement cost in turns, floored at the hex distance. The floor is
    /// deliberate: the sites the scorer likes sit on rivers, the host charges a
    /// river crossing the unit's whole turn, and the board's river edges are
    /// read off a mask that has been wrong before (`rv` in `mirror.rs`). On
    /// `civvis-20260901T212354Z` the board priced a two-hex floodplain walk
    /// at one turn; the host took a turn per hex. A hex a turn is what the
    /// opening walk costs in practice, and pricing it so errs toward founding.
    pub(super) fn opening_walk_turns(
        g: &Game,
        uid: u32,
        from: Pos,
        target: Pos,
        costs: &BTreeMap<Pos, f64>,
    ) -> Option<u32> {
        let moves = g.unit_max_moves(uid).max(1.0);
        let by_cost = costs
            .get(&target)
            .map(|cost| (cost / moves).ceil() as u32)?;
        Some(by_cost.max(g.wdist(from, target).max(0) as u32))
    }

    /// Movement points from the Settler's tile to every tile it can walk to
    /// within `radius` hexes of it — a bounded Dijkstra over
    /// `Game::step_cost` across the ground `unit_can_traverse` admits.
    ///
    /// ⚠ `Game::path_to` is a ONE-TURN flood: it stops where the unit's
    /// movement runs out, so the route-priced branch of
    /// `best_reachable_settle_site_except_cached` only ever sees sites the
    /// Settler reaches this turn, and every farther site is ranked by the
    /// per-tile discount alone. That is why the walk was under-priced
    /// (`settler_walk_census`: a mean 7.3 turns a founding) and why this
    /// gene needs its own multi-turn cost. The long-range router
    /// (`route_distance`) counts hex steps at cost one each, which is the
    /// route the Settler will actually walk; terrain is what turns steps
    /// into turns, and this reads it. A tile the walk cannot reach is
    /// absent, and the caller prices it by hex distance instead.
    pub(super) fn settle_sooner_walk_costs(g: &Game, uid: u32, radius: i32) -> BTreeMap<Pos, f64> {
        let Some(unit) = g.units.get(&uid) else {
            return BTreeMap::new();
        };
        let start = unit.pos;
        let mut best: BTreeMap<Pos, f64> = BTreeMap::new();
        best.insert(start, 0.0);
        // (cost, pos) min-heap keyed on the cost in thousandths so it orders;
        // `Pos` breaks ties deterministically.
        let key = |cost: f64| (cost * 1000.0).round() as i64;
        let mut frontier = std::collections::BinaryHeap::new();
        frontier.push(std::cmp::Reverse((key(0.0), start)));
        while let Some(std::cmp::Reverse((cost_key, cur))) = frontier.pop() {
            let cost = best[&cur];
            if key(cost) < cost_key {
                continue;
            }
            for next in g.nbrs(cur) {
                if g.wdist(start, next) > radius || !g.unit_can_traverse(uid, next) {
                    continue;
                }
                let total = cost + g.step_cost(cur, next);
                if best.get(&next).is_none_or(|known| total < *known) {
                    best.insert(next, total);
                    frontier.push(std::cmp::Reverse((key(total), next)));
                }
            }
        }
        best
    }

    /// `settle_sooner_walk_costs` for one Settler decision, built at most
    /// once per radius.
    ///
    /// The flood is a pure function of the board, the Settler and the radius,
    /// and one decision asks for it repeatedly: the ranking prices the opening
    /// walk and the per-turn walk from the same flood, `best_settler_target`
    /// ranks a local and a global radius, and a threat detour re-asks the
    /// whole ranking up to `SETTLER_THREAT_DETOUR_RETRIES` times against the
    /// same unchanged `&Game`. Every one of those calls rebuilt the flood —
    /// 3% of a ladder game's CPU in the 2026-09-26 profile, most of it the
    /// same answer. A `SettlerWalkCosts` lives no longer than the shared
    /// `&Game` borrow of the decision that owns it, so an entry can never go
    /// stale: the board cannot change while it exists.
    pub(super) fn settler_walk_costs_cached<'c>(
        g: &Game,
        uid: u32,
        radius: i32,
        walk_costs: &'c mut SettlerWalkCosts,
    ) -> &'c BTreeMap<Pos, f64> {
        walk_costs
            .entry(radius)
            .or_insert_with(|| Self::settle_sooner_walk_costs(g, uid, radius))
    }

    pub(super) fn best_settler_target(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        local_radius: i32,
        avoid: Option<Pos>,
    ) -> Option<(Pos, f64)> {
        let mut walk_costs = SettlerWalkCosts::new();
        self.best_settler_target_cached(g, pid, uid, local_radius, avoid, &mut walk_costs)
    }

    /// `best_settler_target` sharing the Settler's walk floods with a caller
    /// that asks the ranking more than once against the same `&Game`; see
    /// `settler_walk_costs_cached`.
    pub(super) fn best_settler_target_cached(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        local_radius: i32,
        avoid: Option<Pos>,
        walk_costs: &mut SettlerWalkCosts,
    ) -> Option<(Pos, f64)> {
        if let Some(site) = self.wonder_clearance_site(g, pid, uid) {
            return Some((site, self.settle_value(g, pid, site)));
        }
        let mut score_cache = BTreeMap::new();
        let local = self.best_reachable_settle_site_except_cached(
            g,
            pid,
            uid,
            local_radius,
            avoid,
            &mut score_cache,
            walk_costs,
        );
        if !self.settlement_safety {
            let global = self.best_reachable_settle_site_except_cached(
                g,
                pid,
                uid,
                g.map.width + g.map.height,
                avoid,
                &mut score_cache,
                walk_costs,
            );
            return match (local, global) {
                (Some(local), Some(global)) if global.1 > local.1 + 5.0 => Some(global),
                (Some(local), _) => Some(local),
                (None, global) => global,
            };
        }
        let global_radius = if g.players[pid].techs.contains(&crate::name!("shipbuilding")) {
            g.map.width + g.map.height
        } else {
            SETTLER_LAND_FALLBACK_RADIUS.max(local_radius)
        };
        if global_radius <= local_radius {
            return local;
        }
        let global = self.best_reachable_settle_site_except_cached(
            g,
            pid,
            uid,
            global_radius,
            avoid,
            &mut score_cache,
            walk_costs,
        );
        match (local, global) {
            (Some(local), Some(global)) if global.0 != local.0 => {
                let from = g.units[&uid].pos;
                let extra = (g.wdist(from, global.0) - g.wdist(from, local.0)).max(0) as f64;
                let premium = SETTLER_GLOBAL_PREMIUM + extra * SETTLER_EXTRA_TRAVEL_PRICE;
                if global.1 > local.1 + premium {
                    Some(global)
                } else {
                    Some(local)
                }
            }
            (Some(local), _) => Some(local),
            (None, global) => global,
        }
    }

    /// The cached site can remain legal and well-scored while a visible enemy
    /// makes its NEXT route step unsafe. That is a different failure from
    /// `settler_target_hysteresis`, whose target validation only sees the site
    /// itself. A civilian already stopped in a visible zone of control is the
    /// other concrete route-block shape: `route_step` correctly returns no
    /// path then, but the own unit's stop plus the visible risk is enough to
    /// explain the blockage without reading an unseen enemy.
    pub(super) fn settler_target_has_visible_route_threat(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> bool {
        if !self.settlement_safety {
            return false;
        }
        let Some((current, zoc_stopped)) =
            g.units.get(&uid).map(|unit| (unit.pos, unit.zoc_stopped))
        else {
            return false;
        };
        if current == target {
            return false;
        }
        let visible = self.battlefront_visibility(g, pid);
        match g.route_step(uid, target, 0) {
            Some(next) => {
                // The host bridge refuses a visible hostile leg even when a
                // stacked guard would make that destination safe in the
                // native model.  Price the live recovery probe without the
                // escort discount so it can defer the same leg the host will
                // hold, while native/evaluator detours retain their original
                // support-aware semantics.
                let risk = if self.settler_routing_recovery_on() {
                    self.settlement_tile_risk_with_support(g, pid, Some(uid), next, &visible, false)
                } else {
                    self.settlement_tile_risk(g, pid, Some(uid), next, &visible)
                };
                risk > SETTLER_STEP_RISK_LIMIT
            }
            None => {
                zoc_stopped
                    && self.settlement_tile_risk(g, pid, Some(uid), current, &visible)
                        > SETTLER_STEP_RISK_LIMIT
            }
        }
    }

    /// Defer a threatened corridor for the whole empire and keep the current
    /// Settler productive at the highest-ranked alternate it can approach
    /// safely. The target is deliberately *deferred*, not retired: a guard or
    /// the ordinary military response may clear the blocker, and the original
    /// site belongs back in the ranking soon afterward.
    pub(super) fn detour_settler_around_visible_threat(
        &mut self,
        g: &Game,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> Option<Pos> {
        if !self.settler_threat_detour_on()
            || !self.settler_target_has_visible_route_threat(g, pid, uid, target)
        {
            return None;
        }

        if self.reserve_wonder_clearance(g, pid, uid, target) {
            return None;
        }
        let until = g.turn + g.standard_duration(SETTLER_THREAT_DETOUR_TURNS);
        let previous = self.settler_threat_deferrals.insert(target, until);
        let avoid = self.settler_avoid.get(&uid).map(|(position, _)| *position);
        // `best_settler_target` already orders candidates by city value,
        // travel cost, and route risk. A threat can cover more than one top
        // candidate, though, so temporarily set aside each unsafe runner-up
        // and ask again instead of mistaking the first one for the only one.
        // These short-lived scratch entries are restored below; only `target`
        // remains deferred for the empire after this decision.
        let mut scratch = Vec::new();
        let mut fallback = None;
        // Every retry ranks against the same unchanged board from the same
        // tile; only the deferral set moves. Share the walk floods.
        let mut walk_costs = SettlerWalkCosts::new();
        // `settler-detour-stays-near`: rank only the sites about as close as
        // the one being left. See `SETTLER_DETOUR_NEAR_SLACK`.
        let near_limit = self.settler_detour_stays_near.then(|| {
            let current = g.units[&uid].pos;
            (g.wdist(current, target) + SETTLER_DETOUR_NEAR_SLACK)
                .clamp(SETTLER_DETOUR_NEAR_FLOOR, SETTLER_DETOUR_NEAR_CEILING)
        });
        let mut near_scores = BTreeMap::new();
        for _ in 0..SETTLER_THREAT_DETOUR_RETRIES {
            let ranked = match near_limit {
                Some(limit) => self.best_reachable_settle_site_except_cached(
                    g,
                    pid,
                    uid,
                    limit,
                    avoid,
                    &mut near_scores,
                    &mut walk_costs,
                ),
                None => self.best_settler_target_cached(g, pid, uid, 8, avoid, &mut walk_costs),
            };
            let Some((candidate, _)) = ranked else {
                break;
            };
            if !self.settler_target_has_visible_route_threat(g, pid, uid, candidate) {
                fallback = Some(candidate);
                break;
            }
            let old = self.settler_threat_deferrals.insert(candidate, until);
            scratch.push((candidate, old));
        }
        for (candidate, old) in scratch {
            match old {
                Some(old_until) => {
                    self.settler_threat_deferrals.insert(candidate, old_until);
                }
                None => {
                    self.settler_threat_deferrals.remove(&candidate);
                }
            }
        }
        // `detour-keeps-the-site-worth`: the fallback above is the best site
        // whose approach is SAFE, which is not the same as a site worth the
        // walk. Refuse one that gives up more of the deferred site's value
        // than the floor allows; the caller keeps the original target and the
        // deferral is rolled back below, exactly as a missing alternate is.
        let fallback = fallback.filter(|candidate| {
            if !self.detour_keeps_the_site_worth {
                return true;
            }
            let kept = self.settle_value(g, pid, target);
            kept <= 0.0
                || self.settle_value(g, pid, *candidate) >= kept * SETTLER_DETOUR_VALUE_FLOOR
        });
        let Some(fallback) = fallback else {
            match previous {
                Some(old_until) => {
                    self.settler_threat_deferrals.insert(target, old_until);
                }
                None => {
                    self.settler_threat_deferrals.remove(&target);
                }
            }
            if let Some(limit) = near_limit {
                think!(self.journal(), Expansion, Detail,
                       "Settler keeps {target:?} through a blocked approach";
                       "no safe site lies within {limit} tiles, and a far detour is how \
                        the last settlers spent twenty turns crossing the empire; the \
                        guard holds here until the approach clears";
                       target);
            }
            return None;
        };

        self.settler_targets.insert(uid, fallback);
        self.settler_stalls.remove(&uid);
        self.settler_closest.remove(&uid);
        think!(self.journal(), Expansion, Detail,
               "Settler detours around a visible threat";
               "the approach to {target:?} is unsafe, so every Settler leaves it alone for \
                {SETTLER_THREAT_DETOUR_TURNS} standard turns while the army can clear the \
                blocker; {fallback:?} is the best safe alternate";
               fallback);
        Some(fallback)
    }

    pub(super) fn guard_is_bound_to_settler(&self, settler: u32, guard: u32) -> bool {
        self.settler_guard_ids(settler)
            .into_iter()
            .flatten()
            .any(|bound| bound == guard)
    }

    pub(super) fn all_reserved_civilian_guards(&self) -> BTreeSet<u32> {
        self.settler_guards
            .values()
            .chain(self.settler_sea_guards.values())
            .copied()
            .chain(self.builder_support.values().map(|support| support.guard))
            // Clearing the approach is already a civilian-support assignment;
            // ordinary escort selection must not dismantle its force.
            .chain(
                self.wonder_clearance
                    .values()
                    .flat_map(|r| r.units.iter().copied()),
            )
            .collect()
    }

    pub(super) fn clear_settler_guard_bindings(&mut self, settler: u32) -> bool {
        let land = self.settler_guards.remove(&settler).is_some();
        let sea = self.settler_sea_guards.remove(&settler).is_some();
        let cleared = land || sea;
        if cleared {
            self.guard_wait.remove(&settler);
        }
        cleared
    }

    /// Bind a military unit to the stacking layer its domain actually uses.
    /// A guard cannot serve two settlers, even when it is being rebound after
    /// a live-board ID remap.
    pub(super) fn bind_settler_guard(&mut self, g: &Game, settler: u32, guard: u32) {
        let Some(unit) = g.units.get(&guard) else {
            return;
        };
        // `amphibious-siege-train-2`: a member of the crossing guards no one.
        if self.amphibious_locked(guard) {
            return;
        }
        let sea = g.rules.units[unit.kind].domain.as_deref() == Some("sea");
        self.settler_guards.retain(|bound_settler, bound_guard| {
            *bound_settler == settler || *bound_guard != guard
        });
        self.settler_sea_guards
            .retain(|bound_settler, bound_guard| {
                *bound_settler == settler || *bound_guard != guard
            });
        if sea {
            self.settler_sea_guards.insert(settler, guard);
        } else {
            self.settler_guards.insert(settler, guard);
        }
    }

    pub(super) fn long_settler_escort_journey(
        &mut self,
        g: &Game,
        uid: u32,
    ) -> Option<SettlerEscortJourney> {
        let target = self.settler_targets.get(&uid).copied()?;
        if let Some(journey) = self.settler_escort_journeys.get(&uid).copied() {
            if journey.target == target {
                return Some(journey);
            }
            self.settler_escort_journeys.remove(&uid);
            self.clear_settler_guard_bindings(uid);
        }
        let distance = g.route_distance(uid, target, 0)?;
        if distance < SETTLER_ESCORT_DISTANCE as usize {
            return None;
        }
        let journey = SettlerEscortJourney {
            home: g.units[&uid].pos,
            target,
        };
        self.settler_escort_journeys.insert(uid, journey);
        let home = journey.home;
        think!(self.journal(), Expansion, Detail, "Settler begins an escorted expedition";
               "the route from {home:?} to {target:?} is {distance} tiles, so a land guard stays \
                with it and a naval guard joins while it is afloat");
        Some(journey)
    }

    pub(super) fn long_settler_escort_active(&self, uid: u32, target: Pos) -> bool {
        self.settler_escort_journeys
            .get(&uid)
            .is_some_and(|journey| journey.target == target)
    }

    pub(super) fn guard_matches_escort_layer(
        g: &Game,
        unit: &crate::game::Unit,
        sea: bool,
    ) -> bool {
        let spec = &g.rules.units[unit.kind];
        spec.class == "military"
            && spec.domain.as_deref() != Some("air")
            && (spec.domain.as_deref() == Some("sea")) == sea
    }

    pub(super) fn escort_guard_can_hold(
        &self,
        g: &Game,
        pid: usize,
        guard: u32,
        pos: Pos,
        sea: bool,
        visible: Option<&Arc<TileBits>>,
    ) -> bool {
        g.units.get(&guard).is_some_and(|unit| {
            unit.owner == pid
                && Self::guard_matches_escort_layer(g, unit, sea)
                && (!self.settler_guard_holds_on()
                    || (unit.hp >= STACKED_GUARD_MIN_HP
                        && visible.is_some_and(|visible| {
                            !self.guard_outmatched_at(g, pid, unit, pos, visible)
                        })))
        })
    }

    /// The settler half of `stacked_escort`: keep a guard bound, and hold
    /// position — briefly and boundedly — when the guard has fallen off the
    /// pair.
    ///
    /// Returns `Some(acted)` when the turn was spent waiting or shedding a
    /// stale formation link; `None` when the settler should march normally,
    /// either because its guard is stacked (it will mirror the step when the
    /// military loop reaches it) or because no guard can come and expansion
    /// must not wait for one.
    ///
    /// There is deliberately NO minimum distance here, unlike the
    /// formation-era `SETTLER_ESCORT_DISTANCE`: both settlers lost on run
    /// civvis-20260815T081505Z were captured within one tile of their site —
    /// the doorstep is where the guard matters most, because a civilian
    /// always arrives with 0 moves and must survive a full enemy turn before
    /// it can found.
    pub(super) fn stacked_escort_pace(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.formationless_settler_escort() {
            return None;
        }
        // ★★★ REPAIRED 2026-08-20 (cycle three) after the 6p whole-genome
        // screen priced escort-always at **−2.1 pp wins (z −4.8)** for this
        // flag and **−2.3 (z −5.4)** for `settler_stack_discipline` over
        // 13,386 game-pairs (seeds 46000000..): every settler bound a
        // military guard full-time and waited for it, in a regime where the
        // recorded census says the native engine loses ~zero civilians
        // (settlers.py: live seat 43 of 127 captured; native 0.00). The
        // machinery was army-thinning overhead paid against a risk that was
        // not there. It now engages ON THREAT: a guard is bound and the
        // pacing below runs only while a visible hostile military unit
        // stands within [`SETTLER_ESCORT_THREAT_RADIUS`] of the settler —
        // the recorded live captures (t22–t25 of run civvis-20260815T230003Z
        // among them) all had exactly such a unit in sight. On quiet ground
        // any bound guard is RELEASED back to the army and the settler
        // marches normally. The live seat keeps its protection the moment a
        // raider shows; the native game stops paying for ghosts.
        // Shedding a stale formation link is bookkeeping about a channel
        // this treatment retires — it happens whether or not anything
        // threatens the settler, so it runs before the threat gate.
        if g.units[&uid].linked_to.is_some() {
            return Some(g.apply(pid, &Action::UnlinkUnits { unit: uid }).is_ok());
        }
        let threatened = {
            let position = g.units[&uid].pos;
            g.units.values().any(|unit| {
                unit.owner != pid
                    && g.is_at_war(pid, unit.owner)
                    && g.unit_visible_to(unit.id, pid)
                    && g.rules.units[unit.kind].class == "military"
                    && g.rules.units[unit.kind].domain.as_deref() != Some("air")
                    && !(self.barbarian_scouts_are_scouts
                        && !self.live_barbarian_scouts_capture
                        && g.players[unit.owner].is_barbarian
                        && g.rules.units[unit.kind].promotion_class == "recon")
                    && g.wdist(unit.pos, position) <= SETTLER_ESCORT_THREAT_RADIUS
            })
        };
        // See `live_settler_capture_lessons`: a known camp is a raider that
        // has not stepped out of the fog yet. Six of the twenty-four captures
        // of 2026-08-28 followed "no visible hostile within 8 tiles" by one
        // turn, and the nest that took six settlers in run
        // civvis-20260829T000643Z was a camp the board knew about.
        let camp_near = self.live_settler_capture_lessons && {
            let position = g.units[&uid].pos;
            // ... and the ground that took a settler is not quiet because
            // nothing is visible on it today (civvis-20260829T022749Z t103).
            self.settler_capture_scars.contains_key(&position)
                || g.barb_camps
                    .keys()
                    .any(|camp| g.wdist(*camp, position) <= SETTLER_ESCORT_THREAT_RADIUS)
        };
        // A quiet local walk still gives its guard back to the army. A route
        // that was long when the Settler left home is different: the point of
        // the escort is to make the unobserved middle and the first island
        // night survivable, not merely to react after a raider is visible.
        let journey = self.long_settler_escort_journey(g, uid);
        if !threatened && !camp_near && journey.is_none() {
            if self.clear_settler_guard_bindings(uid) {
                think!(self.journal(), Expansion, Detail, "The settler's guard stands down";
                       "no visible hostile within {SETTLER_ESCORT_THREAT_RADIUS} tiles and \
                        this is not a long expedition; the army gets its body back");
            }
            return None;
        }
        let current = g.units[&uid].pos;
        let afloat = g
            .map
            .get(current)
            .is_some_and(|tile| g.rules.is_water(tile));
        // A naval guard cannot shield a Settler after it disembarks. The land
        // guard remains assigned so it can cover the one hostile turn before
        // the city is founded.
        if !afloat {
            self.settler_sea_guards.remove(&uid);
        }
        // The live safety floor already refuses to price a stacked guard as
        // protection when it is wounded or a visible hostile can break it.
        // Keep the assignment itself to that same definition: otherwise a
        // guard that has ceased to protect is retained forever, and the
        // Settler waits beside the exact unit that will die first.
        let visible = self
            .settler_guard_holds_on()
            .then(|| self.battlefront_visibility(g, pid));
        if let Some(guard) = self.settler_guards.get(&uid).copied() {
            if !self.escort_guard_can_hold(g, pid, guard, current, false, visible.as_ref()) {
                self.settler_guards.remove(&uid);
                self.guard_wait.remove(&uid);
            }
        }
        if let Some(guard) = self.settler_sea_guards.get(&uid).copied() {
            if !afloat
                || !self.escort_guard_can_hold(g, pid, guard, current, true, visible.as_ref())
            {
                self.settler_sea_guards.remove(&uid);
            }
        }
        // On water, a naval military unit occupies a second legal stack with
        // the embarked land guard and Settler. The live water floor below
        // refuses an unguarded water leg, so search the whole routeable sea
        // rather than only the local eight-tile escort radius. A hull can be
        // several turns away after the army has built it for another job;
        // binding it now lets `stacked_guard_step` bring it over instead of
        // leaving the Settler in a permanent hold. Disconnected lakes and
        // trapped coastal hulls are excluded by the same long-range router
        // the guard will use. The sea guard is still best effort: if no hull
        // can reach this water body, the safety floor keeps the expedition
        // stopped rather than exposing it.
        if afloat && !self.settler_sea_guards.contains_key(&uid) {
            let taken = self.all_reserved_civilian_guards();
            // `religion-seat-charts-the-rivals`: the founder's one charting
            // ship keeps charting. `None` with the gene off.
            let charting_ship = self.religion_seat_charting_ship(g, pid);
            let sea_guard = g
                .player_unit_ids(pid)
                .into_iter()
                .filter(|candidate| {
                    let unit = &g.units[candidate];
                    unit.linked_to.is_none()
                        && !taken.contains(candidate)
                        && charting_ship != Some(*candidate)
                        && Self::guard_matches_escort_layer(g, unit, true)
                        && (!self.settler_guard_holds_on()
                            || (unit.hp >= STACKED_GUARD_MIN_HP
                                && !self.guard_outmatched_at(
                                    g,
                                    pid,
                                    unit,
                                    current,
                                    visible.as_ref().expect("computed under the flag"),
                                )))
                })
                .filter_map(|candidate| {
                    let route_distance = g.route_distance(candidate, current, 0)?;
                    Some((candidate, route_distance))
                })
                .min_by_key(|(candidate, route_distance)| {
                    let unit = &g.units[candidate];
                    let spec = &g.rules.units[unit.kind];
                    (
                        *route_distance,
                        spec.has_ranged_attack(),
                        std::cmp::Reverse(g.unit_strength(unit, true) as i64),
                        *candidate,
                    )
                })
                .map(|(candidate, _)| candidate);
            if let Some(guard) = sea_guard {
                self.bind_settler_guard(g, uid, guard);
                think!(self.journal(), Expansion, Detail, "A naval guard joins the settler";
                       "the sea layer shares the water leg while the land guard remains \
                        responsible for the landing; the hull can route to the embarked \
                        Settler and will close the gap on its military turn");
            }
        }
        if !self.settler_guards.contains_key(&uid) {
            let taken = self.all_reserved_civilian_guards();
            let threatened = self.plan.as_ref().and_then(|plan| plan.threatened_city);
            let holds_threatened_city = |position: Pos| {
                threatened.is_some_and(|city| {
                    g.cities
                        .get(&city)
                        .is_some_and(|city| g.wdist(position, city.pos) <= 3)
                })
            };
            let guard = g
                .player_unit_ids(pid)
                .into_iter()
                .filter(|candidate| {
                    let unit = &g.units[candidate];
                    let spec = &g.rules.units[unit.kind];
                    spec.class == "military"
                        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                        && unit.linked_to.is_none()
                        && !taken.contains(candidate)
                        && !self.conquest_unit_committed(g, pid, *candidate)
                        && g.wdist(unit.pos, current) <= SETTLER_ESCORT_SEARCH_RADIUS
                        && !holds_threatened_city(unit.pos)
                        && (!self.settler_guard_holds_on()
                            || (unit.hp >= STACKED_GUARD_MIN_HP
                                && !self.guard_outmatched_at(
                                    g,
                                    pid,
                                    unit,
                                    current,
                                    visible.as_ref().expect("computed under the flag"),
                                )))
                })
                .min_by_key(|candidate| {
                    let unit = &g.units[candidate];
                    let spec = &g.rules.units[unit.kind];
                    (
                        g.wdist(unit.pos, current),
                        spec.siege,
                        spec.has_ranged_attack(),
                        std::cmp::Reverse(g.unit_strength(unit, true) as i64),
                        *candidate,
                    )
                });
            if let Some(guard) = guard {
                think!(self.journal(), Expansion, Detail, "A guard joins the settler";
                       "it will share the settler's tile; a stacked civilian cannot \
                        be captured");
                self.bind_settler_guard(g, uid, guard);
            }
        }
        // The water leg paces against the naval guard when it has one. At a
        // shoreline, switch back to the land guard before moving ashore: the
        // latter is the unit that can survive beside the new city overnight.
        let next_is_water = self
            .settler_targets
            .get(&uid)
            .and_then(|target| g.route_step(uid, *target, 0))
            .is_some_and(|next| g.map.get(next).is_some_and(|tile| g.rules.is_water(tile)));
        let guard = if afloat && next_is_water {
            self.settler_sea_guards
                .get(&uid)
                .copied()
                .or_else(|| self.settler_guards.get(&uid).copied())
        } else {
            self.settler_guards.get(&uid).copied()
        };
        let guard = guard?;
        if g.units[&guard].pos == current {
            self.guard_wait.remove(&uid);
            return None;
        }
        // A completed Settler starts on one of our city tiles, which is the
        // safest place it will be all trip.  Do not spend its first actionable
        // turn fortifying there just because a newly assigned guard has not
        // caught up: retain the assignment so the guard still follows, but
        // hand the Settler to the ordinary safe-step route immediately.  That
        // route can still reject a genuinely unsafe departure or take a safer
        // sidestep; it is the open-ended city wait that costs the expansion
        // window.
        if g.city_at(current)
            .is_some_and(|city| g.cities.get(&city).is_some_and(|city| city.owner == pid))
        {
            self.guard_wait.remove(&uid);
            think!(self.journal(), Expansion, Detail,
                   "Settler departs its city before its guard";
                   "the guard remains assigned, while the Settler takes the first safe step \
                    toward its target now");
            return None;
        }
        // Counted once per turn: `advance_unit_serial` re-enters a unit up to
        // eight times a turn, and each of those is the same wait.
        let (last_turn, waited) = self.guard_wait.get(&uid).copied().unwrap_or((u32::MAX, 0));
        let waited = if last_turn == g.turn {
            waited
        } else {
            waited.saturating_add(1)
        };
        self.guard_wait.insert(uid, (g.turn, waited));
        // ⭐ `escort-cap-holds`: THE CAP IS THE WHOLE POINT OF THE CAP.
        // `unstacked_settler_step_is_capturable` is the one predicate in this
        // wait that reads the VISIBLE frame only, and in the opening a visible
        // hostile near the next step is the weather rather than the exception,
        // so the two-turn ceiling is suspended indefinitely by it. Measured on
        // the live King seat (2026-08-29, twenty-eight settler losses in
        // eleven runs): EIGHT settlers were captured while parked waiting for
        // a guard, and the median settler was ten turns old at its loss. The
        // wait it is paying for does not prevent that — a parked civilian is
        // exactly what a raider walks onto — while the ten losses to a hostile
        // that was not visible at all say the predicate suspending the cap
        // cannot see the thing that kills the settler either. With the gene on
        // the cap releases on schedule and the ordinary safe-step route, which
        // still refuses a genuinely unsafe departure, gets the settler back.
        if waited > STACKED_ESCORT_PATIENCE
            && (self.escort_cap_holds || !self.unstacked_settler_step_is_capturable(g, pid, uid))
        {
            // The guard is livelocked or refused. It keeps chasing, but the
            // settler stops paying for it; the counter stays saturated until
            // the guard actually re-stacks, so waiting and marching cannot
            // oscillate. A visibly capturable next step is different: holding
            // on the current tile is safer than spending the Settler merely to
            // break that livelock.
            return None;
        }
        // ★★★★ WAITING IS ONLY SAFE ON A SAFE TILE. Run civvis-20260815T230003Z:
        // the host refused the founding at (29,18) on t22, the settler then
        // stood there "waiting for its guard" on t23 and t24 with a barbarian
        // warrior three tiles away and its guard on the ADJACENT tile — not
        // stacked, so not protecting — and on t25 the barbarians walked off
        // with it (a barbarian Settler appears at (28,16)). One Settler and
        // twenty turns of the opening lost; the second city landed at t37.
        // Standing still is the one thing a threatened, unstacked settler must
        // not do: step toward the guard instead — it closes the gap the guard
        // is closing from the other side, so they stack a turn sooner — and
        // wait only when no such step exists.
        // Any hostile that can reach or approach the tile counts (risk > 0):
        // the question here is not "is this a good tile to walk onto" (the
        // step limit) but "can something take an unstacked civilian off it
        // before the guard arrives" — a lone warrior two tiles out already can.
        if self.settlement_safety {
            let visible = self.battlefront_visibility(g, pid);
            let risk = self.settlement_tile_risk(g, pid, Some(uid), current, &visible);
            let guard_pos = g.units[&guard].pos;
            if risk > 0.0 {
                // See `settler_marches_alone_when_clear`: nothing hostile can
                // come near the road, so the fallback gives way.
                if self.settler_marches_alone(g, pid, uid, waited) {
                    return None;
                }
                think!(self.journal(), Expansion, Detail, "Settler falls back toward its guard";
                       "its tile is threatened (risk {risk:.0}) and the guard is {} tiles \
                        away — waiting here is how the last settler was captured",
                       g.wdist(current, guard_pos); guard_pos);
                if self.settler_step_toward_safe(g, pid, uid, guard_pos) {
                    return Some(true);
                }
            } else if g.wdist(current, guard_pos) <= ADJACENT_GUARD_MARCH_DISTANCE {
                // ★★★★ QUIET GROUND, GUARD ONE STEP BEHIND: MARCH. The wait
                // exists so a civilian is never left alone in reach of a
                // threat, and `settlement_tile_risk` has just said nothing
                // visible can reach it; a guard one tile back re-stacks on
                // its own move. Waiting anyway cost the first settler of run
                // civvis-20260816T084206Z three desyncs of two turns each on a
                // four-tile walk (t27–t37): ten turns for a city four tiles
                // from the capital, and the second city at t37 against t20 in
                // the game before. Two or more tiles behind, or any visible
                // risk, keeps the hold and the fallback exactly as they were.
                self.guard_wait.remove(&uid);
                return None;
            } else if self.escort_cap_holds {
                // ⭐ `escort-cap-holds`, second half: A ZERO RISK READING IS
                // NOT QUIET GROUND, IT IS AN EMPTY VISION FRAME.
                // `settlement_tile_risk` prices what the seat can SEE, and the
                // branch above releases the settler only when its guard is
                // already one tile behind. Everything else — a guard three
                // tiles back, or one that will never arrive — fortifies in
                // place on a bare tile outside any city, which is precisely
                // the posture that lost eight of the twenty-eight settlers of
                // 2026-08-29 and the posture ten more died in without a
                // hostile visible anywhere near them the turn before. By this
                // point the settler is provably not stacked (a stacked guard
                // returned above) and provably not standing in one of our own
                // cities (the city departure returned above), so there is
                // nothing here to protect it. Hand it to the ordinary
                // safe-step route, which prices the step it is about to take
                // against `SETTLER_STEP_RISK_LIMIT` rather than against zero
                // and can still refuse it — the guard stays assigned and keeps
                // chasing either way.
                think!(self.journal(), Expansion, Detail,
                       "Settler marches while its guard catches up";
                       "nothing visible can reach this tile and the guard is {} tiles \
                        back — standing still on open ground outside a city is how eight \
                        of the last twenty-eight settlers were taken",
                       g.wdist(current, guard_pos); guard_pos);
                self.guard_wait.remove(&uid);
                return None;
            }
        }
        // See `escort_patience_runs_out`. Deliberately the LAST word: every
        // protective branch above has already had its say -- a threatened tile
        // has tried to step back toward the guard, and a guard one tile behind
        // on quiet ground has already released the settler -- so what is left
        // here is a settler that would otherwise stand still indefinitely
        // because something is visible near its next step. That is the
        // ordinary weather of the opening, and standing in it until turn 32
        // is how the game is lost.
        if self.escort_patience_runs_out && waited >= ESCORT_PATIENCE_CEILING {
            think!(self.journal(), Expansion, Detail, "Settler stops waiting for its guard";
                   "{waited} turns waited and the guard has not stacked; the third \
                    city cannot wait longer than the opening does");
            return None;
        }
        // See `settler_marches_alone_when_clear`: nothing hostile can come
        // near the road and this turn's steps are in sight, so the wait gives
        // way; the guard stays assigned and keeps following.
        if self.settler_marches_alone(g, pid, uid, waited) {
            return None;
        }
        think!(self.journal(), Expansion, Detail, "Settler waits for its guard";
               "{waited} turn(s) so far; marching alone is how the last two settlers \
                were captured");
        if self.settler_never_idles {
            // See `settler_never_idles`: the wait is a hold, and a hold is
            // the watchdog's to bound — report the turn as not spent.
            return Some(false);
        }
        Some(self.base.fortify_or_stop(g, pid, uid))
    }

    /// Would a city founded at `site` right now bleed Loyalty fast enough to
    /// revolt before it becomes a useful city? This asks the engine's complete
    /// Loyalty calculation of a speculative city. The live mirror's
    /// current-plot protection catches only its -8/turn emergency; target
    /// selection also avoids a steady loss that would flip the city within its
    /// forty-turn growth forecast.
    /// Why a site would not hold, if it would not: the forecast revolt, or —
    /// under `frontier_loyalty` — a colony beyond the empire's reach on fogged
    /// ground. `None` when the site is judged sound.
    pub(super) fn settle_site_loyalty_verdict(
        &self,
        g: &Game,
        pid: usize,
        site: Pos,
    ) -> Option<String> {
        if let Some(why) = self.settle_site_frontier_loyalty_verdict(g, pid, site) {
            return Some(why);
        }
        // `exhaustion-loyalty-guard` and an explicit Science lane: the
        // forecast below sums the rival cities the mirror can see, and
        // under-reads the ones it cannot.
        // Run civvis-20260829T090147Z founded Arretium at t29 four tiles from
        // a visible Mongolian city and five from Rome; the forecast passed it,
        // the engine read −7.2 Loyalty a turn from the first turn, and the
        // city flipped at t42 in peacetime. The exhaustion search's sieve
        // (`inside_rival_sphere`) already refuses such a plot; the preferred
        // search now asks the same question first.
        if (self.exhaustion_loyalty_guard
            || self.active_victory_target(g) == Some(VictoryTarget::Science))
            && Self::inside_rival_sphere(g, pid, site)
        {
            return Some(
                "it stands nearer a visible rival major's city than any of ours, inside that \
                 city's nine-tile Loyalty reach, and the forecast under-reads the rival cities \
                 it cannot see"
                    .to_string(),
            );
        }
        Self::settle_site_forecast_revolt(g, pid, site).map(|(loyalty_per_turn, turns_to_flip)| {
            format!(
                "the city would lose {:.1} Loyalty a turn beside its neighbours and revolt in about {:.0} turns",
                -loyalty_per_turn,
                turns_to_flip.ceil()
            )
        })
    }

    /// Whether `uid` retired `pos` after standing on it unable to found. Expired
    /// entries are pruned at the top of `advanced_settler_step`.
    pub(super) fn settler_site_is_dead(&self, uid: u32, pos: Pos) -> bool {
        self.settler_dead_sites
            .get(&uid)
            .is_some_and(|sites| sites.contains_key(&pos))
            || self.settler_capture_scars.contains_key(&pos)
    }

    /// Whether another live Settler already owns this site for the current
    /// empire.  Normal target selection is sequential, but the live bridge can
    /// have several Settlers in flight at once; without this check each one can
    /// independently choose the same fallback after a target is retired.
    pub(super) fn settler_target_reserved_by_other(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        pos: Pos,
    ) -> bool {
        // Target reservations are part of the bounded recovery/commitment
        // contract. Keep the historical controller's ordinary site ranking
        // untouched; the frozen `advanced_v1` anchor has neither commitment
        // nor never-idles recovery enabled.
        if !self.settler_commit && !self.settler_never_idles {
            return false;
        }
        self.settler_targets.iter().any(|(other, target)| {
            *other != uid
                && *target == pos
                && g.units
                    .get(other)
                    .is_some_and(|unit| unit.owner == pid && unit.kind == "settler")
        })
    }

    /// The Settler's turn: the decision routine below and — under
    /// `settler-never-idles` — a watchdog that marches a Settler the routine
    /// has held for `SETTLER_IDLE_PATIENCE` turns. See
    /// `advanced/settler_never_idles.rs`.
    pub(super) fn advanced_settler_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let acted = self.advanced_settler_step_inner(g, pid, uid);
        // `founding-voyage`: a cityless seat's embarked Settler with nowhere to
        // go sails toward land. See `advanced/maori_founds_from_the_sea.rs`.
        if !acted && self.founding_voyage_step(g, pid, uid) {
            return true;
        }
        if acted || !self.settler_never_idles {
            return acted;
        }
        let Some(unit) = g.units.get(&uid) else {
            return acted;
        };
        // A Settler the routine left without a target already ran the
        // exhaustion search this turn (`settler_stranded`), or retired a
        // doomed arrival for next turn's pick; asking again would only
        // repeat the scan and the journal line. Once its patience expires,
        // however, a missing target is exactly the fog/frontier recovery
        // case the watchdog must handle — see `settler_frontier_step`.
        if unit.owner != pid
            || unit.moves_left <= 0.0
            || self.settler_idle_streak(uid) < settler_never_idles::SETTLER_IDLE_PATIENCE
        {
            return acted;
        }
        if !self.settler_targets.contains_key(&uid) {
            return self.settler_frontier_step(g, pid, uid);
        }
        self.settler_watchdog_step(g, pid, uid)
    }

    pub(super) fn advanced_settler_step_inner(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> bool {
        let current = g.units[&uid].pos;
        if self.live_settler_capture_lessons {
            self.settler_last_seen.insert(uid, current);
            self.remember_early_settler_home(g, pid, uid);
        }
        if self.settler_never_idles {
            self.note_settler_idle(g, uid);
        }
        if self.settle_sooner || self.fog_yields_to_a_stranded_settler {
            // The walk clock starts the first turn the Settler is stepped,
            // whichever branch below steps it. See `settle_sooner` and
            // `advanced/fog_yields_stranded.rs`.
            self.settler_walk_started.entry(uid).or_insert(g.turn);
        }
        if let Some(sites) = self.settler_dead_sites.get_mut(&uid) {
            sites.retain(|_, until| *until > g.turn);
        }
        if self.settler_threat_detour_on() {
            self.settler_threat_deferrals
                .retain(|_, until| *until > g.turn);
        }
        if self
            .settler_avoid
            .get(&uid)
            .is_some_and(|(_, until)| *until <= g.turn)
        {
            self.settler_avoid.remove(&uid);
        }
        // ★★★★ STANDING ON A TARGET THAT CANNOT BE FOUNDED IS A DEAD END, and
        // until now nothing left it. The founding branch below fires only when
        // `can_found_city` agrees; when it does not — the host refused the
        // plot, the plot proved to be a city-state's, or the mirror's loyalty
        // forecast doomed it the moment the settler arrived
        // (`block_loyalty_doomed_settler_sites` evaluates only the settler's
        // CURRENT plot) — the cached target still passed every check the
        // re-validation below makes, so the settler journaled "HELD short …
        // it is already standing on its target" every turn. Measured on run
        // civvis-20260815T190904Z: the settler built at t126 reached (53,27)
        // at t151, stood there 25 turns and died at t176 without founding;
        // the empire held 7 cities from t98 to t202. Retire the site for a
        // while — the same bounded avoidance the stall counter uses — so the
        // next pick, which already refuses `blocked_city_sites`, runs now.
        // ⚠ Behind `settler_commit`, which every default constructor leaves off
        // and the frozen `advanced_v1` anchor never sets, so the ladder is
        // untouched (`elo_anchor_never_reaches_the_settler_commit_path`).
        if let Some(target) = self.settler_targets.get(&uid).copied() {
            if self.settler_commit && target == current && !g.can_found_city(uid) {
                think!(self.journal(), Expansion, Detail,
                       "Settler abandons {target:?}";
                       "it stands on the site and the city cannot be founded there \
                        — the target is retired and a new one will be chosen";
                       target);
                // A SET with a long expiry, not the single `settler_avoid`
                // slot: the mirror's doom forecast is recomputed only for the
                // plot the settler stands on, so a site retired for the stall
                // counter's eight turns was re-picked the moment the settler
                // stepped off it, and the stall path's own `settler_avoid`
                // write then evicted it (replayed: abandoned t151, marching
                // back at t156). Thirty standard turns outlasts the walk to the
                // next site, and a doomed frontier is usually several plots.
                self.settler_dead_sites.entry(uid).or_default().insert(
                    target,
                    g.turn + g.standard_duration(SETTLER_DEAD_SITE_AVOID_TURNS),
                );
                self.settler_targets.remove(&uid);
                self.settler_stalls.remove(&uid);
                self.settler_blocked_turns.remove(&uid);
            }
        }
        // An expansion settler can replace its exposed civilian with a city
        // before the retreat rules run, even short of its preferred target.
        if self.settler_founds_for_shelter(g, pid, uid) {
            return true;
        }
        // ★★★★ A SETTLER ON A THREATENED TILE RETREATS BEFORE IT DOES ANYTHING
        // ELSE. Every hold in this routine — waiting out a forecast, a
        // safe-step guard that refuses every neighbour, a guard that has not
        // arrived — used to leave the settler exactly where it stood, which is
        // no safer than the steps it refused and often worse. Run
        // civvis-20260815T233405Z: the settler that reached coastal (69,22) on
        // t111 with a barbarian Galley three hexes out held there and was
        // taken on t112; the next one held on the same shore on t125–130 and
        // was taken on t131. A guard standing on the tile is protection and
        // is left alone; otherwise, when the tile itself is over the step
        // limit, take the neighbour that lowers the risk most and let the
        // target wait for the next turn's march.
        // See `civilian_out_of_reach`: the same retreat, native, against the
        // exact tiles a raider could stand on next turn.
        // The opt-in owns the broad, pre-emptive route rule below.  The live
        // bridge still needs this narrow floor for the stronger fact that a
        // Settler is already inside a visible barbarian's exact capture reach:
        // leaving it there while every one-step retreat is also covered loses
        // it before it receives another turn. `civilian_flee_step` returns
        // `None` unless that direct-capture condition holds, then searches the
        // unit's whole remaining move instead of only its adjacent tiles.
        //
        // Do not promote `civilian_out_of_reach` here. That opt-in also owns
        // the ordinary route-step avoidance policy; this is only the live
        // formationless bridge's emergency survival floor, gated by the
        // standard settlement-safety policy.
        if self.civilian_out_of_reach
            || (self.formationless_settler_escort() && self.settlement_safety)
        {
            if let Some(acted) = self.civilian_flee_step(g, pid, uid) {
                return acted;
            }
        }
        // `settler-shelters-from-the-war`: an enemy major's soldier near the
        // Settler sends it to a city of ours, ahead of the march. Exact no-op
        // while the gene is off. See `advanced/settler_shelters_from_the_war.rs`.
        if let Some(acted) = self.settler_shelters_from_the_war_step(g, pid, uid) {
            return acted;
        }
        if let Some(home) = self.early_settler_homeward_target(g, uid) {
            return self.return_early_settler_home(g, pid, uid, home);
        }
        if self.formationless_settler_escort() && self.settlement_safety {
            // Protected where it stands: inside a city, or sharing the tile
            // with any of our own military units (the assigned guard or not).
            // `settler_guard_holds_on` also includes the live capture-lessons
            // bridge.  The production ledger may withhold the screenable
            // `settler-guard-holds` arm while that HostOnly repair is still
            // active; computing visibility from the raw field here leaves
            // `guard_outmatched_at` with `None` and panics exactly when a
            // wounded escort has to be rejected.
            let visible_now = self
                .settler_guard_holds_on()
                .then(|| self.battlefront_visibility(g, pid));
            let guarded_here = g.city_at(current).is_some()
                || g.unit_ids_at(current).iter().any(|other| {
                    *other != uid
                        && g.units.get(other).is_some_and(|unit| {
                            unit.owner == pid
                                && g.rules.units[unit.kind].class == "military"
                                // See `settler_guard_holds`: only the bound
                                // guard is held here, and only while it could
                                // hold. Anyone else walks off; an outmatched
                                // guard is broken and the settler taken.
                                && (!self.settler_guard_holds_on()
                                    || (self.guard_is_bound_to_settler(uid, *other)
                                        && g.rules.units[unit.kind].domain.as_deref() != Some("air")
                                        && (g.rules.units[unit.kind].domain.as_deref() != Some("sea")
                                            || g.map.get(current).is_some_and(|tile| {
                                                g.rules.is_water(tile)
                                            }))
                                        && unit.hp >= STACKED_GUARD_MIN_HP
                                        && !self.guard_outmatched_at(
                                            g,
                                            pid,
                                            unit,
                                            current,
                                            visible_now.as_ref().expect("computed under the flag"),
                                        )))
                        })
                });
            if !guarded_here {
                let visible = self.battlefront_visibility(g, pid);
                let here = self.settlement_tile_risk(g, pid, Some(uid), current, &visible);
                if here > SETTLER_STEP_RISK_LIMIT {
                    let mut retreat: Option<(f64, Pos)> = None;
                    for step in g.nbrs(current) {
                        if !g.can_move(uid, step) {
                            continue;
                        }
                        let risk = self.settlement_tile_risk(g, pid, Some(uid), step, &visible);
                        if risk < here && retreat.is_none_or(|(best, _)| risk < best) {
                            retreat = Some((risk, step));
                        }
                    }
                    if let Some((risk, step)) = retreat {
                        think!(self.journal(), Expansion, Detail,
                               "Settler retreats from {current:?}";
                               "its tile is threatened (risk {here:.0}); stepping to {step:?} \
                                (risk {risk:.0}) rather than waiting to be taken"; step);
                        // ★★★★ A TARGET THE SETTLER KEEPS RETREATING FROM IS ONE
                        // IT WILL NOT REACH. Run civvis-20260816T075807Z: a
                        // barbarian Galley sat off the coast four tiles from the
                        // capital from t30 to t80, every site was coastal, and two
                        // settlers spent t33–t87 stepping back (risk 101 → 48 →
                        // 5), marching back in and stepping back again — the third
                        // city came at t84 and the tally read 457 against 847.
                        // The retreat is right; repeating it forever is not.
                        // After `SETTLER_RETREAT_LIMIT` retreats while committed
                        // to one target, that target is retired the way a site
                        // the settler cannot found is (`settler_dead_sites`),
                        // and the next pick looks elsewhere.
                        if let Some(target) = self.settler_targets.get(&uid).copied() {
                            let count = match self.settler_retreats.get(&uid) {
                                Some((committed, count)) if *committed == target => count + 1,
                                _ => 1,
                            };
                            self.settler_retreats.insert(uid, (target, count));
                            if count >= SETTLER_RETREAT_LIMIT {
                                think!(self.journal(), Expansion, Detail,
                                       "Settler gives up on {target:?}";
                                       "retreated {count} times while walking there; the \
                                        approach is not clearing, so the site is retired for \
                                        {SETTLER_DEAD_SITE_AVOID_TURNS} standard turns"; target);
                                self.settler_dead_sites.entry(uid).or_default().insert(
                                    target,
                                    g.turn + g.standard_duration(SETTLER_DEAD_SITE_AVOID_TURNS),
                                );
                                self.settler_targets.remove(&uid);
                                self.settler_stalls.remove(&uid);
                                self.settler_blocked_turns.remove(&uid);
                                self.settler_retreats.remove(&uid);
                            }
                        }
                        if g.apply(
                            pid,
                            &Action::Move {
                                unit: uid,
                                to: step,
                            },
                        )
                        .is_ok()
                        {
                            return true;
                        }
                    }
                }
            }
        }
        let avoid = self.settler_avoid.get(&uid).map(|(position, _)| *position);
        // Search only the immediate neighborhood for the capital. The target
        // is fixed after the first assessment, preventing a rolling optimum
        // from leading the settler across the map for many compounding turns.
        if g.player_city_ids(pid).is_empty() {
            let visible = self.battlefront_visibility(g, pid);
            let cached = self.settler_targets.get(&uid).copied().filter(|target| {
                let Some(tile) = g.map.get(*target) else {
                    return false;
                };
                !g.rules.is_water(tile)
                    && g.rules.is_passable(tile)
                    && !g.cities.values().any(|city| g.wdist(city.pos, *target) < 4)
                    && tile
                        .owner_city
                        .is_none_or(|cid| g.cities[&cid].owner == pid)
                    && !self.settler_site_is_dead(uid, *target)
                    && (!self.settler_threat_detour_on()
                        || !self.settler_threat_deferrals.contains_key(target))
                    && !self.settler_target_reserved_by_other(g, pid, uid, *target)
                    && (*target == current || g.route_step(uid, *target, 0).is_some())
                    && (!self.settlement_safety
                        || self.settlement_tile_risk(g, pid, Some(uid), *target, &visible)
                            <= SETTLER_STEP_RISK_LIMIT)
            });
            if cached.is_none() {
                self.settler_targets.remove(&uid);
            }
            let target = cached.or_else(|| {
                let current_value = self.settle_value(g, pid, current);
                let local = self.best_reachable_settle_site_except(g, pid, uid, 2, avoid);
                let target = if g.can_found_city(uid) {
                    Some(
                        local
                            .filter(|(_, value)| *value > current_value + 3.0)
                            .map(|(pos, _)| pos)
                            .unwrap_or(current),
                    )
                } else {
                    local
                        .or_else(|| {
                            if self.settlement_safety {
                                self.best_settler_target(g, pid, uid, 8, avoid)
                            } else {
                                self.best_reachable_settle_site_except(
                                    g,
                                    pid,
                                    uid,
                                    g.map.width + g.map.height,
                                    avoid,
                                )
                            }
                        })
                        .or_else(|| {
                            self.base
                                .best_reachable_settle_site(g, pid, uid, g.map.width + g.map.height)
                                .filter(|(pos, _)| {
                                    !self.settler_site_is_dead(uid, *pos)
                                        && (!self.settler_threat_detour_on()
                                            || !self.settler_threat_deferrals.contains_key(pos))
                                        && !self.settler_target_reserved_by_other(g, pid, uid, *pos)
                                })
                        })
                        .map(|(pos, _)| pos)
                };
                if let Some(target) = target {
                    self.settler_targets.insert(uid, target);
                }
                target
            });
            let target = target.map(|target| {
                self.detour_settler_around_visible_threat(g, pid, uid, target)
                    .unwrap_or(target)
            });
            if target == Some(current) && g.can_found_city(uid) {
                // `capital-leaves-the-frost`: on snow or tundra, step toward
                // warmer ground and look before founding, while the speed's
                // opening budget allows. `false` with the gene off.
                if self.frost_capital_look(g, pid, uid, current) {
                    return true;
                }
                if self.settlement_safety
                    && self.settlement_tile_risk(g, pid, Some(uid), current, &visible)
                        > SETTLER_STEP_RISK_LIMIT
                {
                    self.settler_targets.remove(&uid);
                    return false;
                }
                self.settler_targets.remove(&uid);
                // ⚠ Journal AFTER the engine answers. Run
                // civvis-20260815T081505Z printed "Founding a city" twice
                // (t35, t45) for cities that never existed, and the study
                // that caught it had to cross-check orders.sqlite to learn
                // the why-log cannot be trusted on foundings. A Decision
                // line now asserts an applied action, never an intention.
                // ⚠ Priced BEFORE the apply, for the reason in the city branch.
                let worth = self.settle_value(g, pid, current);
                let yielded = self.hidden_border_yield_before_founding(g, pid, current);
                let fogged = self.fog_yield_before_founding(g, pid, current);
                let founded = g.apply(pid, &Action::FoundCity { unit: uid }).is_ok();
                if founded {
                    self.note_hidden_border_founding(current, yielded);
                    self.note_fog_founding(current, fogged);
                    think!(self.journal(), Expansion, Decision, "Founding the capital at {current:?}";
                           "the site is worth {:.1}", worth; current);
                } else {
                    think!(self.journal(), Expansion, Detail, "Founding refused at {current:?}";
                           "the engine would not take the capital here"; current);
                }
                return founded;
            }
            if let Some(target) = target {
                // The walk's length in TURNS is what `opening_walk_pays`
                // priced; it is journaled so a replay can be read against it.
                let walk_turns = Self::opening_walk_turns(
                    g,
                    uid,
                    current,
                    target,
                    &Self::settle_sooner_walk_costs(g, uid, 3),
                );
                think!(self.journal(), Expansion, Detail,
                       "Walking the first settler toward {target:?}";
                       "worth {:.1} against {:.1} where it stands, {} turn(s) away",
                       self.settle_value(g, pid, target),
                       self.settle_value(g, pid, current),
                       walk_turns.map_or("?".to_string(), |turns| turns.to_string()); target);
                let moved = self.settler_step_toward_safe(g, pid, uid, target);
                if moved {
                    self.settler_stalls.remove(&uid);
                    self.settler_blocked_turns.remove(&uid);
                } else if self.settler_commit {
                    *self.settler_blocked_turns.entry(uid).or_insert(0) += 1;
                    let stalls = self.settler_stalls.entry(uid).or_insert(0);
                    *stalls += 1;
                    if *stalls >= SETTLER_STALL_LIMIT {
                        if self.founds_where_it_stands(g, pid, uid, current) {
                            return true;
                        }
                        self.settler_avoid
                            .insert(uid, (target, g.turn + g.standard_duration(8)));
                        self.settler_targets.remove(&uid);
                        self.settler_stalls.remove(&uid);
                    }
                } else {
                    self.settler_targets.remove(&uid);
                }
                return moved;
            }
            return false;
        }
        // See `settler_walk_deadline`: a walker past its deadline founds the
        // best legal site within reach before the target search runs.
        if self.settler_walk_deadline {
            if let Some(acted) = self.settler_walk_deadline_step(g, pid, uid) {
                return acted;
            }
        }
        // See `stalled_settler_takes_a_safe_site`: past its walk allowance a
        // Settler takes a safe site within a few tiles before the target
        // search hands it another exposed one.
        if let Some(acted) = self.stalled_settler_step(g, pid, uid) {
            return acted;
        }
        let visible = self.battlefront_visibility(g, pid);
        // The checks in dropping order, each with a name. Run 212725Z spent
        // t224-t248 with FOUR settlers orbiting one site — "sets (22, 21)
        // aside" every hysteresis period, per settler, staggered — and the
        // journal could not say WHICH check flickered, so the lane's next
        // measurement had nothing to bite on (the promotion-block class:
        // no clean discriminator). Same conditions, same order, same
        // short-circuit as the closure this replaces; only the first
        // failure's name escapes.
        let target_drop_reason = |target: Pos| -> Option<&'static str> {
            let Some(tile) = g.map.get(target) else {
                return Some("the tile left the map");
            };
            if g.rules.is_water(tile) {
                return Some("water");
            }
            if !g.rules.is_passable(tile) {
                return Some("impassable");
            }
            if g.cities.values().any(|c| g.wdist(c.pos, target) < 4) {
                return Some("a city within three tiles");
            }
            if !tile
                .owner_city
                .is_none_or(|cid| g.cities[&cid].owner == pid)
            {
                return Some("another empire's ground");
            }
            if Some(target) == avoid {
                return Some("the site it is avoiding");
            }
            if !self.early_settler_site_allowed(g, pid, uid, target) {
                return Some("outside the early Settler's home corridor");
            }
            // A site the host or the mirror has since blocked is not a
            // target, however it was chosen: `best_settler_target`
            // refuses `blocked_city_sites` when it PICKS a site, but a
            // site can join that set after the pick (see the standing
            // case at the top of this function for the measured cost).
            if g.blocked_city_sites.contains(&target) {
                return Some("the host blocked the plot");
            }
            if self.settler_capture_scars.contains_key(&target) {
                return Some("a settler was lost within three tiles");
            }
            if self.settler_site_is_dead(uid, target) {
                return Some("marked dead for this settler");
            }
            if self.settler_target_reserved_by_other(g, pid, uid, target) {
                return Some("another settler already owns the site");
            }
            if self.air_resource_colony_target_refused(g, pid, uid, target) {
                return Some("the resource colony lost its guarded supply permission");
            }
            // `commitment_patience`: a threat is a hold, not a drop — the
            // ledger retires the site if the hold outlasts its patience.
            if self.settler_threat_detour_on()
                && !self.commitment_patience
                && self.settler_threat_deferrals.contains_key(&target)
            {
                return Some("the approach is temporarily deferred for a visible threat");
            }
            // A momentarily unavailable route is not a bad site. Under
            // `settler_commit` the stall counter decides when to give up,
            // not a single blocked turn. Without commitment, however, a
            // geometric route whose first step cannot be paid or entered
            // this turn must be dropped through the hysteresis path. The
            // route planner deliberately ignores the current movement
            // allowance, so checking only `route_step(...).is_some()` lets a
            // live bridge remove the target and immediately pick it again.
            let route_can_advance = target == current
                || g.route_step(uid, target, 0).is_some_and(|next| {
                    // Without the hysteresis gene this is the frozen
                    // historical controller: preserve its geometric-route
                    // behavior and routing-cache side effect exactly.
                    !self.settler_target_hysteresis_on() || g.can_move(uid, next)
                })
                || self.settler_commit;
            if !route_can_advance {
                return Some("the route's next step is not legal this turn");
            }
            if self.settlement_safety
                && !self.commitment_patience
                && self.settlement_tile_risk(g, pid, Some(uid), target, &visible)
                    > SETTLER_STEP_RISK_LIMIT
            {
                return Some("step risk above the limit");
            }
            None
        };
        let cached_drop = self
            .settler_targets
            .get(&uid)
            .copied()
            .and_then(target_drop_reason);
        if let Some(site) = self.settler_targets.get(&uid).copied().filter(|_| {
            cached_drop == Some("the resource colony lost its guarded supply permission")
        }) {
            // The ordinary economic ranking must not immediately reuse
            // a supply target whose stronger permission just failed.
            self.settler_dead_sites.entry(uid).or_default().insert(
                site,
                g.turn + g.standard_duration(SETTLER_DEAD_SITE_AVOID_TURNS),
            );
        }
        let valid_target = self
            .settler_targets
            .get(&uid)
            .copied()
            .filter(|_| cached_drop.is_none());
        // The rate forecast is a live-bridge repair, not a production or
        // tournament policy. The separate frontier guard is host-only and
        // must still run when the deployment ledger holds the rate alarm off:
        // without it, a fogged colony reaches its target and is founded into
        // the exact immediate Loyalty collapse the guard exists to prevent.
        // Either guard also owns safe exhaustion below, so `BasicAi` cannot
        // immediately reselect a site this controller just retired.
        let science_targeted = self.active_victory_target(g) == Some(VictoryTarget::Science);
        let loyalty_guard_active =
            self.base.loyalty_rate_alarm || self.frontier_loyalty || science_targeted;
        // See `settler_target_hysteresis`: a cached site the validation just
        // dropped stays out of the next picks, so a threat flickering at the
        // edge of sight cannot flip the settler between two sites every frame.
        if self.settler_target_hysteresis_on() && valid_target.is_none() {
            if let Some(dropped) = self.settler_targets.get(&uid).copied() {
                if Some(dropped) != avoid {
                    self.settler_avoid.insert(
                        uid,
                        (
                            dropped,
                            g.turn + g.standard_duration(SETTLER_TARGET_HYSTERESIS_TURNS),
                        ),
                    );
                    let why = cached_drop.unwrap_or("no cached site");
                    think!(self.journal(), Expansion, Detail,
                           "Settler sets {dropped:?} aside";
                           "the cached site stopped passing its checks ({why}); it stays out of the \
                            next picks for {SETTLER_TARGET_HYSTERESIS_TURNS} standard turns so a \
                            threat flickering at the edge of sight cannot flip the march every frame";
                           dropped);
                    // See `settler_target_hysteresis_2`: share only a failure
                    // that is really the site's. A route, support, or threat
                    // rejection is local to this Settler; sharing it can make
                    // every expansion unit discard a route another's guard can
                    // safely complete.
                    if self.settler_target_hysteresis_2
                        && Self::settler_target_drop_is_empire_wide(why)
                    {
                        let until = g.turn + g.standard_duration(SETTLER_TARGET_HYSTERESIS_TURNS);
                        let others: Vec<u32> = g
                            .player_unit_ids(pid)
                            .into_iter()
                            .filter(|other| *other != uid && g.units[other].kind == "settler")
                            .collect();
                        for other in others {
                            self.settler_dead_sites
                                .entry(other)
                                .or_default()
                                .insert(dropped, until);
                        }
                    }
                }
            }
        }
        let avoid = if self.settler_target_hysteresis_on() {
            self.settler_avoid.get(&uid).map(|(position, _)| *position)
        } else {
            avoid
        };
        // Preserve a colony already under way, but replace a merely cached
        // home-island target once the overseas gene finds a discovered foreign
        // landfall and the home landmass has genuinely run short of room.
        let overseas_target = if self.overseas_settlement
            && self.early_settler_home(uid).is_none()
            && !(valid_target == Some(current) && g.can_found_city(uid))
        {
            let home_landmass = BasicAi::capital_landmass(g, pid);
            valid_target
                .filter(|target| !home_landmass.contains(target))
                .or_else(|| self.overseas_settlement_target(g, pid, uid, avoid, &home_landmass))
        } else {
            None
        };
        if let Some(target) = overseas_target {
            self.settler_targets.insert(uid, target);
            self.settler_stalls.remove(&uid);
            self.settler_closest.remove(&uid);
        }
        let target = overseas_target.or(valid_target).or_else(|| {
            if let Some(site) = self.air_resource_colony_settler_target(g, pid, uid, avoid) {
                self.air_resource_colony_target = Some((uid, site));
                self.settler_targets.insert(uid, site);
                think!(self.journal(), Expansion, Detail,
                       "Settler chooses the Bomber supply colony";
                       "the known resource is needed by the committed airfield; its route, \
                        nearby defender and Loyalty checks pass"; site);
                return Some(site);
            }
            // Ask the forecast before the walk. The mirror can only reject a
            // doomed site after the Settler stands on it, which previously
            // spent a long frontier walk merely to discover the city would
            // revolt almost immediately. Retiring a rejected site reuses the
            // bounded dead-site set from the current-plot repair (#1689), so
            // the next candidate is genuinely distinct.
            let mut rejected = 0;
            loop {
                // The preferred ranking is allowed to pay a premium for a
                // rich, distant site. That is normally right, but three
                // distinct frontier colonies can all fail the live Loyalty
                // forecast while an already-ranked, reachable local site
                // remains. Do not turn that bounded safety retry into a
                // parked Settler: after the preferred attempts, ask the local
                // ranking once.
                // It carries the same site, route, dead-site, and threat
                // filters; it merely declines the global premium.
                let local_fallback = rejected >= SETTLE_TARGET_FORECAST_RETRIES;
                let candidate = if local_fallback {
                    self.best_reachable_settle_site_except(g, pid, uid, 8, avoid)
                } else {
                    self.best_settler_target(g, pid, uid, 8, avoid)
                };
                let Some((pos, _)) = candidate else {
                    break None;
                };
                let verdict = if self.base.loyalty_rate_alarm || science_targeted {
                    self.settle_site_loyalty_verdict(g, pid, pos)
                } else {
                    self.settle_site_frontier_loyalty_verdict(g, pid, pos)
                };
                let Some(why) = verdict else {
                    if local_fallback {
                        think!(self.journal(), Expansion, Detail,
                               "Settler falls back to a nearby safe site";
                               "{SETTLE_TARGET_FORECAST_RETRIES} higher-priority sites failed the \
                                live Loyalty forecast; this ranked local site still passes it";
                               pos);
                    }
                    self.settler_targets.insert(uid, pos);
                    break Some(pos);
                };
                think!(self.journal(), Expansion, Detail,
                       "Settler refuses {pos:?} before walking there";
                       "{why}; the site is retired and the next best is asked"; pos);
                self.settler_dead_sites.entry(uid).or_default().insert(
                    pos,
                    g.turn + g.standard_duration(SETTLER_DEAD_SITE_AVOID_TURNS),
                );
                if local_fallback {
                    break None;
                }
                rejected += 1;
            }
        });
        // See `settler_never_idles`: exhaustion asks two wider questions
        // before it holds, and a hold is never silent.
        let target = target.or_else(|| {
            if !self.settler_never_idles || self.settler_stranded_recently(g, uid) {
                return None;
            }
            let site = self.settler_exhaustion_target(g, pid, uid)?;
            self.settler_targets.insert(uid, site);
            self.settler_relaxed_targets.insert(uid, site);
            self.settler_stalls.remove(&uid);
            self.settler_closest.remove(&uid);
            Some(site)
        });
        let Some(mut target) = target else {
            if self.settler_never_idles {
                return self.settler_stranded(g, pid, uid);
            }
            // Do not let a safe exhaustion fall through to `BasicAi`, which
            // cannot see `settler_dead_sites` and may select the very plot the
            // live forecast just retired. A later board can reveal a safe site
            // or change the Loyalty calculation; until then, holding preserves
            // the Settler rather than knowingly founding a doomed city.
            if loyalty_guard_active {
                return false;
            }
            if self.early_settler_home(uid).is_some() {
                // The live early corridor is a hard safety boundary, not a
                // preference the baseline fallback may override when the
                // local search is empty.
                return false;
            }
            return self.base.settler_step(g, pid, uid);
        };
        if let Some(fallback) = self.detour_settler_around_visible_threat(g, pid, uid, target) {
            target = fallback;
        }
        if current == target && g.can_found_city(uid) {
            // A cached target may have been safe when it was chosen but become
            // loyalty-doomed while the Settler walked there: a nearby rival can
            // grow, found, or become newly visible during that trip. The fresh
            // target loop above forecasts before marching, but intentionally
            // retains a valid cache rather than paying for a speculative city
            // each turn. Recheck once at arrival, when founding would otherwise
            // make the loss irreversible.
            let relaxed = self.settler_never_idles
                && self.settler_relaxed_targets.get(&uid) == Some(&current);
            let arrival_verdict = if self.air_resource_colony_refused(g, pid, current) {
                Some("the resource colony no longer has its required supply need, friendly city-states, defense and Loyalty".to_string())
            } else if relaxed && !science_targeted {
                // See `relaxed_arrival_verdict`: a site the exhaustion search
                // chose is judged at arrival by the rule that chose it.
                self.relaxed_arrival_verdict(g, pid, current)
            } else if self.base.loyalty_rate_alarm || science_targeted {
                self.settle_site_loyalty_verdict(g, pid, current)
            } else {
                self.settle_site_frontier_loyalty_verdict(g, pid, current)
            };
            if let Some(why) = arrival_verdict {
                think!(self.journal(), Expansion, Detail,
                       "Settler abandons loyalty-doomed arrival at {current:?}";
                       "{why}, so the cached site is retired before founding and the next \
                        target will be distinct"; current);
                self.settler_dead_sites.entry(uid).or_default().insert(
                    current,
                    g.turn + g.standard_duration(SETTLER_DEAD_SITE_AVOID_TURNS),
                );
                self.settler_targets.remove(&uid);
                self.settler_stalls.remove(&uid);
                self.settler_blocked_turns.remove(&uid);
                self.settler_closest.remove(&uid);
                return false;
            }
            if self.settlement_safety
                && self.settlement_tile_risk(g, pid, Some(uid), current, &visible)
                    > SETTLER_STEP_RISK_LIMIT
            {
                self.settler_targets.remove(&uid);
                return false;
            }
            self.settler_targets.remove(&uid);
            // ⚠ Price the site BEFORE the engine answers. `settle_value` reads
            // the ground the settler is standing on, and founding changes that
            // ground: the growth forecast that carries `growth_readiness` now
            // runs over a worked city centre. Measured on live Chieftain run
            // `civvis-20260819T041034Z`, the value logged after the apply is
            // ANTI-CORRELATED with the city it became — Ravenna (-3.1) and
            // Aquileia (-1.4) are the seat's two best non-capital cities at
            // 14.2 and 13.1 science, while the one site that priced positive
            // (+2.2) became Lugdunum at 2.25. A study reading those numbers
            // concludes the seat settles bad ground and is wrong; the wide
            // opening is measured (#554: 23.0% -> 52.3% win rate) and being
            // picky about sites cost 41 Elo as `city_target_floor`.
            //
            // The APPLY still gates the line — see the capital branch: a
            // Decision asserts an applied action, never an intention.
            let worth = self.settle_value(g, pid, current);
            // `contested_land_first`: read the front before the apply as
            // well — founding here makes this city our nearest, and the gap
            // it closed is gone.
            let contested = self.contested_land_front(g, pid, current);
            let yielded = self.hidden_border_yield_before_founding(g, pid, current);
            let fogged = self.fog_yield_before_founding(g, pid, current);
            let founded = g.apply(pid, &Action::FoundCity { unit: uid }).is_ok();
            if founded {
                self.note_hidden_border_founding(current, yielded);
                self.note_fog_founding(current, fogged);
                if let Some((front, credit)) = contested {
                    *g.players[pid]
                        .counters
                        .entry("contested_land:founded".to_string())
                        .or_insert(0) += 1;
                    think!(self.journal(), Expansion, Detail, "The city claims contested ground";
                           "{current:?} lies {} tiles from {}'s city at {:?}, worth {credit:.1} \
                            for the gap it closes",
                           g.wdist(current, front.their_city),
                           g.players[front.rival].civ,
                           front.their_city; current);
                }
                think!(self.journal(), Expansion, Decision, "Founding a city at {current:?}";
                       "the site is worth {:.1}; the empire holds {} cities and wants {}",
                       worth,
                       g.player_city_ids(pid).len(),
                       self.plan.as_ref().map_or(0, |plan| plan.desired_cities); current);
            } else {
                think!(self.journal(), Expansion, Detail, "Founding refused at {current:?}";
                       "the engine would not take the city; the settler will re-plan"; current);
            }
            return founded;
        }
        if let Some(acted) = self.stacked_escort_pace(g, pid, uid) {
            return acted;
        }
        if let Some(escort) = g.units[&uid].linked_to.filter(|peer| {
            g.units.get(peer).is_some_and(|escort| {
                g.rules.units[escort.kind].class == "military"
                    && g.rules.units[escort.kind].domain.as_deref() != Some("sea")
            })
        }) {
            let distance = g.wdist(current, target);
            let stalled_turns = {
                let entry = self.escort_march.entry(uid).or_insert((distance, 0));
                if distance < entry.0 {
                    *entry = (distance, 0);
                } else {
                    entry.1 = entry.1.saturating_add(1);
                }
                entry.1
            };
            if self.escort_unstick && stalled_turns >= 2 {
                self.escort_march.remove(&uid);
                if g.apply(pid, &Action::UnlinkUnits { unit: escort }).is_ok() {
                    think!(self.journal(), Expansion, Detail,
                           "Escort released a stalled settler";
                           "{distance} tiles to {target:?} unchanged for two turns \
                            — the formation was not walking, so the settler will";
                           target);
                    // fall through to the ordinary self-march below
                } else {
                    return true;
                }
            } else {
                think!(self.journal(), Expansion, Detail, "Settler advancing with its escort";
                       "{} tiles remain to {target:?}", distance; target);
                return true;
            }
        }
        if let Some(escort) = g.units[&uid].linked_to.filter(|peer| {
            g.units
                .get(peer)
                .is_some_and(|escort| g.rules.units[escort.kind].domain.as_deref() == Some("sea"))
        }) {
            if g.wdist(current, target) == 1 {
                return g.apply(pid, &Action::UnlinkUnits { unit: escort }).is_ok();
            }
            return false;
        }
        think!(self.journal(), Expansion, Detail, "Settler marching to {target:?}";
               "{} tiles away, the site is worth {:.1}",
               g.wdist(current, target), self.settle_value(g, pid, target); target);
        // See `civilian_out_of_reach`: the step is refused inside a raider's
        // reach unless the guard walks in too or the city is founded now.
        let moved = if self.civilian_out_of_reach || self.long_settler_escort_active(uid, target) {
            self.settler_step_out_of_reach(g, pid, uid, target)
        } else {
            self.settler_step_toward_safe(g, pid, uid, target)
        };
        // ★★★★ "marching" is printed ABOVE, before the step is attempted, so the
        // journal has never been able to tell a march from a hold. Measured on the
        // live ladder: settlers cross **0.78 tiles/turn on 2 movement points**,
        // stand still on **45%** of their turn-transitions, and **91% of those
        // standstills carry that very "marching" line** — because it prints either
        // way. Nothing recorded that the step failed, and nothing recorded why.
        //
        // It is not the engine refusing: **0 of 1455** settler standstills carry a
        // `move_refused`, and **84%** emit no `MOVE_TO` at all. It is not a
        // self-tile step either — naming `self_tile_move` by unit kind (#950)
        // returned 249 events, 100% military, **zero settlers**. So the cause is
        // here, on our own board, and this is the only place that can say it.
        //
        // Gated on the journal level: the reason costs a second `route_step`, and
        // A* is not worth paying for on a turn nobody is reading.
        if !moved && self.journal().wants(crate::reasoning::Level::Detail) {
            // ⚠ Name the BLOCKER, not just the fact of blocking. Replaying four
            // live runs with only the coarse reason returned 83 holds against 292
            // march attempts, and 67 of them — 81% — were "the next tile refuses
            // the unit", which is a symptom and not yet a repair.
            // ⚠ `route_step` returns `None` for three different reasons and only
            // one of them is a routing failure: it also returns `None` when the
            // unit already stands on `target` (`wdist <= range`, with range 0),
            // and when a zone of control has it locked in formation. Folding
            // those into "no route" would repeat the very mistake this block
            // exists to fix — the coarse reason bucketed 67 of 83 holds into a
            // symptom rather than a cause.
            //
            // The already-arrived case is separated below. The ZoC lock is NOT
            // yet distinguishable from here: `formation_movement_locked_by_zoc`
            // is private to `game`, so a settler held by a zone of control
            // still reports "no route". Read that bucket as "no route OR ZoC
            // lock" until the predicate is exposed.
            // A step the settler cannot PAY for is the end of its move, not a
            // hold: Civilization VI charges the whole terrain cost before
            // entering, so a settler that has already walked with 1 point left
            // stops in front of a 2-point hill and walks on next turn. Replays
            // of 19 live "the next tile refuses it and nothing is standing
            // there" holds from October 7-8 were all this shortfall (1-2 points
            // left against a 2-5 point step), and the 47 runs' settlers stood
            // still on only 13% of their turns to t60 against a median 6 HELD
            // lines a game. Named apart so the idle census and the HELD counts
            // stop reading an ordinary turn's end as a stall.
            let end_of_move = (current != target)
                .then(|| g.route_step(uid, target, 0))
                .flatten()
                .filter(|step| {
                    !g.can_move(uid, *step) && g.unit_ids_at(*step).iter().all(|o| *o == uid)
                })
                .and_then(|step| g.step_movement_shortfall(uid, step))
                .filter(|(left, _)| *left > 0.0);
            if let Some((left, cost)) = end_of_move {
                think!(self.journal(), Expansion, Detail, "Settler ends its move short of {target:?}";
                       "{} tiles away with {left:.0} movement left; the next tile costs {cost:.0}, \
                        so it walks on next turn",
                       g.wdist(current, target); target);
            } else {
                let why = if current == target {
                    "it is already standing on its target".to_string()
                } else {
                    match g.route_step(uid, target, 0) {
                        None => "no route to it on our own board".to_string(),
                        Some(step) if !g.can_move(uid, step) => {
                            let occupant = g
                                .unit_ids_at(step)
                                .iter()
                                .filter(|other| **other != uid)
                                .find_map(|other| {
                                    g.units.get(other).map(|unit| (unit.owner, unit.kind))
                                });
                            match occupant {
                                Some((owner, kind)) if owner == pid => {
                                    format!("our own {kind} is standing on the next tile")
                                }
                                Some((_, kind)) => format!("a foreign {kind} holds the next tile"),
                                None if g.units[&uid].moves_left <= 0.0 => {
                                    "it had no movement left".to_string()
                                }
                                None => "the next tile refuses it and nothing is standing there"
                                    .to_string(),
                            }
                        }
                        Some(_) => "the safe-step guard rejected every neighbour".to_string(),
                    }
                };
                think!(self.journal(), Expansion, Detail, "Settler HELD short of {target:?}";
                       "{} tiles away and it did not move — {why}",
                       g.wdist(current, target); target);
                // `settler-founds-under-raiders`: held by the safe-step guard a
                // second turn running, among barbarians only, it founds where
                // it stands. See `advanced/settler_raiders.rs`.
                if current != target
                    && matches!(g.route_step(uid, target, 0), Some(step) if g.can_move(uid, step))
                    && self.settler_blocked_turns.get(&uid).copied().unwrap_or(0) >= 1
                    && self.settler_founds_among_raiders(g, pid, uid)
                {
                    return true;
                }
            }
        }
        if !self.settler_commit {
            if moved {
                self.settler_stalls.remove(&uid);
            } else {
                self.settler_targets.remove(&uid);
            }
            return moved;
        }
        // ⚠ Under commitment the counter has to measure PROGRESS, not motion.
        // `settler_step_toward_safe` may legitimately step sideways or back to
        // stay out of a captor's reach, and such a step returns `true` — so a
        // motion test clears the counter every turn and the bounded commitment
        // documented on `settler_commit` can never expire. A settler beside a
        // roaming hostile then dodges for the rest of the game while holding a
        // target it is walking away from.
        //
        // Measured on run `civvis-20260802T041527Z` (Russia, one city, score
        // 149 against a best rival's 1030): target held at axial (63,12) while
        // the journal reported 7, then 8, then 9 tiles away on t244-t246, with
        // the stall counter pinned at zero throughout.
        //
        // The yardstick is the CLOSEST the settler has come to this target, not
        // last turn's distance, so an oscillation between two tiles cannot keep
        // buying turns either. It is stored beside the target and compared
        // against it, which makes a retarget self-invalidating.
        let distance = g.wdist(g.units[&uid].pos, target);
        // ⚠⚠⚠ TWO DIFFERENT QUESTIONS, AND COLLAPSING THEM DISARMED THE ONE
        // MECHANISM THAT RESCUES A STRANDED SETTLER.
        //
        // `retargeted` asks "is this a different target than last turn"; `closer`
        // asks "did this settler actually make progress". `advanced` is their OR,
        // which is right for `settler_stalls` — that counter bounds commitment to
        // ONE target, so a retarget must reset it.
        //
        // It is wrong for `settler_blocked_turns`, whose own documentation says it
        // "survives a target change so a stranded civilian stops monopolizing the
        // empire-wide in-flight allowance". A stranded settler is *precisely* the
        // one that retargets every turn — it can reach nothing, so the site ranking
        // hands it a different best tile each turn — so clearing on `advanced`
        // cleared it every single turn. The counter never reached
        // `SETTLER_REPLACEMENT_BLOCKED_TURNS` (3) and the replacement branch in
        // `settler_in_flight_allowed` never once ran in a live game.
        //
        // Measured on live run `civvis-20260804T011632Z` (Rome/Trajan, 5e16528):
        // one settler stranded from t113 to t213 — 84 turns at FULL movement, the
        // journal retargeting each turn — and NO settler was ordered between t85
        // and the end of the run, while `desired_cities` stood at 6 or 7 for 109
        // turns and the empire held 5 cities it could plainly afford to grow past.
        let (retargeted, closer) = match self.settler_closest.get(&uid) {
            Some((cached, closest)) => (*cached != target, distance < *closest),
            None => (false, true),
        };
        let advanced = retargeted || closer;
        if advanced {
            self.settler_closest.insert(uid, (target, distance));
            self.settler_stalls.remove(&uid);
            if Self::settler_delay_is_resolved(retargeted, closer) {
                self.settler_blocked_turns.remove(&uid);
            }
        } else {
            *self.settler_blocked_turns.entry(uid).or_insert(0) += 1;
            let stalls = self.settler_stalls.entry(uid).or_insert(0);
            *stalls += 1;
            if *stalls >= SETTLER_STALL_LIMIT {
                let here = g.units[&uid].pos;
                if self.founds_where_it_stands(g, pid, uid, here) {
                    return true;
                }
                self.settler_avoid
                    .insert(uid, (target, g.turn + g.standard_duration(8)));
                self.settler_targets.remove(&uid);
                self.settler_stalls.remove(&uid);
                self.settler_closest.remove(&uid);
            }
        }
        moved
    }

    /// A stalled fallback must not plant an unsupported outpost in front of a
    /// known hostile city. Rival city centres in the live export are gated on
    /// plots the player has revealed, so unlike a full-board scan this uses no
    /// information the seat has not earned. A city does not move after it has
    /// been revealed; its last reported owner is refreshed by each revealed
    /// export, and the current war state decides whether it remains hostile.
    pub(super) fn live_stalled_settlement_is_unsupported_frontline(
        &self,
        g: &Game,
        pid: usize,
        here: Pos,
    ) -> Option<(i32, i32)> {
        // `loyalty_rate_alarm` is a live-bridge-only flag. Reuse that boundary
        // because this is another current-board survival repair, rather than
        // changing an evaluator or tournament Settler fallback.
        if !self.base.loyalty_rate_alarm {
            return None;
        }
        let friendly_distance = g
            .player_city_ids(pid)
            .into_iter()
            .map(|city| g.wdist(here, g.cities[&city].pos))
            .min()?;
        g.cities
            .values()
            .filter(|city| city.owner != pid && g.is_at_war(pid, city.owner))
            .map(|city| g.wdist(here, city.pos))
            .filter(|enemy_distance| {
                *enemy_distance <= STALLED_SETTLEMENT_ENEMY_CITY_REACH
                    && *enemy_distance < friendly_distance
            })
            .min()
            .map(|enemy_distance| (enemy_distance, friendly_distance))
    }

    /// A city here beats no city, once the settler has given up on reaching a
    /// better one.
    ///
    /// Only ever reached from the two stall-expiry branches, so it cannot make
    /// a settler settle early — the counter has already run its full length
    /// against a target the unit could not approach. The safety check is the
    /// same one the ordinary found path applies, plus live-only guards against
    /// the loyalty-doomed or unsupported hostile frontier the failed route may
    /// have reached.
    pub(super) fn founds_where_it_stands(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        here: Pos,
    ) -> bool {
        if !self.settler_founds_when_stalled
            || !g.can_found_city(uid)
            || self.air_resource_colony_refused(g, pid, here)
            || self.air_resource_colony_target_refused(g, pid, uid, here)
        {
            return false;
        }
        let visible = self.battlefront_visibility(g, pid);
        if self.settlement_safety
            && self.settlement_tile_risk(g, pid, Some(uid), here, &visible)
                > SETTLER_STEP_RISK_LIMIT
        {
            // `settler-founds-under-raiders`: with barbarians the only threat
            // and a city that would outlast them, the city is the safer piece.
            if !self.settler_threat_is_raiders_only(g, pid, here, &visible)
                || !self.raiders_spare_the_city(g, pid, uid, here, &visible)
            {
                return false;
            }
            think!(self.journal(), Expansion, Detail,
                   "Settler founds among raiders at {here:?}";
                   "settler-founds-under-raiders: every hostile in reach is a barbarian, \
                    and the city would outlast them where the Settler would be taken"; here);
        }
        // A stalled route may reach a tile that the ordinary target loop had
        // already rejected. Keep the fallback behind the same Loyalty
        // protections as target selection and arrival: the optional rate
        // forecast, plus the always-shipped frontier guard. An explicit
        // Science lane opts into the full verdict as well.
        let science_targeted = self.active_victory_target(g) == Some(VictoryTarget::Science);
        let loyalty_verdict = if self.base.loyalty_rate_alarm || science_targeted {
            self.settle_site_loyalty_verdict(g, pid, here)
        } else {
            self.settle_site_frontier_loyalty_verdict(g, pid, here)
        };
        if let Some(why) = loyalty_verdict {
            think!(self.journal(), Expansion, Detail,
                   "Settler declines a loyalty-doomed fallback at {here:?}";
                   "{why}; the fallback site is retired rather than founded"; here);
            self.settler_dead_sites.entry(uid).or_default().insert(
                here,
                g.turn + g.standard_duration(SETTLER_DEAD_SITE_AVOID_TURNS),
            );
            return false;
        }
        if let Some((enemy_distance, friendly_distance)) =
            self.live_stalled_settlement_is_unsupported_frontline(g, pid, here)
        {
            think!(self.journal(), Expansion, Detail,
                   "Settler declines an unsupported frontier at {here:?}";
                   "a known hostile city is {enemy_distance} tiles away, closer than \
                    the nearest friendly city at {friendly_distance}; the fallback site \
                    is retired rather than founded"; here);
            self.settler_dead_sites.entry(uid).or_default().insert(
                here,
                g.turn + g.standard_duration(SETTLER_DEAD_SITE_AVOID_TURNS),
            );
            return false;
        }
        self.settler_targets.remove(&uid);
        self.settler_stalls.remove(&uid);
        self.settler_closest.remove(&uid);
        // ⚠ Journal AFTER the engine answers; see the capital branch.
        let yielded = self.hidden_border_yield_before_founding(g, pid, here);
        let fogged = self.fog_yield_before_founding(g, pid, here);
        let founded = g.apply(pid, &Action::FoundCity { unit: uid }).is_ok();
        if founded {
            self.note_hidden_border_founding(here, yielded);
            self.note_fog_founding(here, fogged);
            think!(self.journal(), Expansion, Decision,
                   "Founding where the settler stands at {here:?}";
                   "it stalled {SETTLER_STALL_LIMIT} turns short of a better site \
                    worth {:.1} against {:.1} here",
                   0.0, self.settle_value(g, pid, here); here);
        }
        founded
    }

    /// Assign exactly one nearby land unit to a Settler and let that unit lead
    /// the formation. Settlers act before military units, so an unescorted
    /// civilian first records its target (or its obstruction) and the chosen
    /// escort can then close the gap in the same turn. Once linked, only the
    /// military leader issues movement orders and both pieces move together.
    /// The military half of `stacked_escort`: shadow the bound settler with
    /// plain moves onto its own tile, and hold that tile once there.
    ///
    /// Returns `None` when this unit guards nobody, so the ordinary military
    /// planner runs. Settlers act before military units in `advanced_units`,
    /// so by the time a guard acts the settler's tile for this turn is final;
    /// a guard that started the turn stacked pays the same terrain costs from
    /// the same tile and therefore always keeps up. No `LinkUnits` is ever
    /// issued: the pair is two ordinary units that happen to share a tile,
    /// which is legal for a military/civilian pair in both rule sets and is
    /// the arrangement Civilization VI cannot capture through.
    pub(super) fn stacked_guard_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        let unit = g.units.get(&uid)?.clone();
        let spec = &g.rules.units[unit.kind];
        if spec.class != "military" || spec.domain.as_deref() == Some("air") {
            return None;
        }
        let (settler, naval_binding) = self
            .settler_guards
            .iter()
            .find_map(|(settler, guard)| (*guard == uid).then_some((*settler, false)))
            .or_else(|| {
                self.settler_sea_guards
                    .iter()
                    .find_map(|(settler, guard)| (*guard == uid).then_some((*settler, true)))
            })?;
        if (spec.domain.as_deref() == Some("sea")) != naval_binding {
            if naval_binding {
                self.settler_sea_guards.remove(&settler);
            } else {
                self.settler_guards.remove(&settler);
            }
            return None;
        }
        let Some(settler_pos) = g
            .units
            .get(&settler)
            .filter(|unit| unit.owner == pid && unit.kind == "settler")
            .map(|unit| unit.pos)
        else {
            // Founded, died, or was captured: the guard goes back to the army.
            self.clear_settler_guard_bindings(settler);
            self.settler_escort_journeys.remove(&settler);
            return None;
        };
        if naval_binding
            && !g
                .map
                .get(settler_pos)
                .is_some_and(|tile| g.rules.is_water(tile))
        {
            // The settler has reached shore. A naval unit cannot guard the
            // landfall, and keeping its movement reserved would only strand
            // it beside a city that the land escort already covers.
            self.settler_sea_guards.remove(&settler);
            return None;
        }
        if unit.pos == settler_pos {
            // `guard-breaks-the-pin`: the raider whose zone of control holds
            // the Settler on this tile is cut down when the trade is worth
            // it, instead of the pair fortifying beside it turn after turn.
            // See `advanced/siege_response.rs`.
            if self.guard_breaks_the_pin(g, pid, uid, settler_pos) {
                return Some(true);
            }
            think!(self.journal(), Expansion, Detail, "Guard stands with its settler";
                   "sharing the tile blocks capture outright"; settler_pos);
            return Some(self.base.fortify_or_stop(g, pid, uid));
        }
        if let Some(acted) = self.escort_naval_memory_step(g, pid, uid, settler_pos) {
            return Some(acted);
        }
        // A guard that cannot close this turn stays a guard: the settler's
        // own bounded patience decides when to stop waiting, and an idle
        // chase still shadows the route. Standing down here would hand the
        // unit back to a doctrine that owes the settler nothing.
        let moved = self.base.step_toward(g, pid, uid, settler_pos);
        if !moved {
            return Some(self.base.fortify_or_stop(g, pid, uid));
        }
        Some(true)
    }

    pub(super) fn settler_escort_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
    ) -> Option<bool> {
        if self.formationless_settler_escort() {
            return self.stacked_guard_step(g, pid, uid);
        }
        if !self.settlement_safety {
            return None;
        }
        let unit = g.units.get(&uid)?.clone();
        let spec = &g.rules.units[unit.kind];
        if spec.class != "military" || matches!(spec.domain.as_deref(), Some("sea" | "air")) {
            return None;
        }

        if let Some(settler) = unit.linked_to.filter(|peer| {
            g.units
                .get(peer)
                .is_some_and(|peer| peer.owner == pid && peer.kind == "settler")
        }) {
            let Some(target) = self.settler_targets.get(&settler).copied() else {
                return Some(g.apply(pid, &Action::UnlinkUnits { unit: uid }).is_ok());
            };
            let before = g.units[&uid].pos;
            let moved = before != target
                && self.settlement_unit_step_toward_safe(g, pid, uid, Some(uid), true, target);
            let after = g.units.get(&uid).map_or(before, |escort| escort.pos);
            let distance = g.wdist(after, target);
            let advanced = moved
                && after != before
                && (!self.linked_settler_progress
                    || match self.settler_closest.get(&settler) {
                        Some((cached, closest)) => *cached != target || distance < *closest,
                        None => true,
                    });
            if advanced {
                if self.linked_settler_progress {
                    self.settler_closest.insert(settler, (target, distance));
                }
                self.settler_stalls.remove(&settler);
                self.settler_blocked_turns.remove(&settler);
                think!(self.journal(), Expansion, Detail, "Escort opening the route";
                       "the Settler formation advances toward {target:?}"; target);
            } else if self.linked_settler_progress || !moved {
                *self.settler_blocked_turns.entry(settler).or_insert(0) += 1;
                let stalls = {
                    let stalls = self.settler_stalls.entry(settler).or_insert(0);
                    *stalls += 1;
                    *stalls
                };
                if stalls >= SETTLER_STALL_LIMIT {
                    self.settler_avoid
                        .insert(settler, (target, g.turn + g.standard_duration(8)));
                    self.settler_targets.remove(&settler);
                    self.settler_stalls.remove(&settler);
                    self.settler_closest.remove(&settler);
                    think!(self.journal(), Expansion, Decision,
                           "Escort abandoning a route without progress";
                           "the formation failed to get closer to {target:?}; the site will be \
                            reconsidered after a cooldown"; target);
                    let unlinked = g.apply(pid, &Action::UnlinkUnits { unit: uid }).is_ok();
                    return Some(moved || unlinked || self.base.fortify_or_stop(g, pid, uid));
                }
            }
            return Some(moved || self.base.fortify_or_stop(g, pid, uid));
        }
        if unit.linked_to.is_some() || unit.moves_left <= 0.0 {
            return None;
        }

        let holds_threatened_city = |candidate: u32| {
            plan.threatened_city.is_some_and(|city| {
                g.cities
                    .get(&city)
                    .is_some_and(|city| g.wdist(g.units[&candidate].pos, city.pos) <= 3)
            })
        };
        let eligible_escort = |candidate: u32| {
            let candidate = &g.units[&candidate];
            let candidate_spec = &g.rules.units[candidate.kind];
            candidate.owner == pid
                && candidate.linked_to.is_none()
                && candidate.moves_left > 0.0
                && candidate_spec.class == "military"
                && !matches!(candidate_spec.domain.as_deref(), Some("sea" | "air"))
                && !holds_threatened_city(candidate.id)
                && !self.conquest_unit_committed(g, pid, candidate.id)
        };

        let mut settlers: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|settler| {
                let civilian = &g.units[settler];
                let Some(target) = self.settler_targets.get(settler) else {
                    return false;
                };
                civilian.kind == "settler"
                    && civilian.linked_to.is_none()
                    && g.map
                        .get(civilian.pos)
                        .is_some_and(|tile| !g.rules.is_water(tile))
                    && (g.wdist(civilian.pos, *target) >= SETTLER_ESCORT_DISTANCE
                        || self
                            .settler_blocked_turns
                            .get(settler)
                            .copied()
                            .unwrap_or(0)
                            > 0)
            })
            .collect();
        settlers.sort_by_key(|settler| (g.wdist(g.units[settler].pos, unit.pos), *settler));

        for settler in settlers {
            let settler_pos = g.units[&settler].pos;
            let designated = g
                .player_unit_ids(pid)
                .into_iter()
                .filter(|candidate| {
                    eligible_escort(*candidate)
                        && g.wdist(g.units[candidate].pos, settler_pos)
                            <= SETTLER_ESCORT_SEARCH_RADIUS
                })
                .min_by_key(|candidate| {
                    let candidate_unit = &g.units[candidate];
                    let candidate_spec = &g.rules.units[candidate_unit.kind];
                    (
                        g.wdist(candidate_unit.pos, settler_pos),
                        candidate_spec.siege,
                        candidate_spec.has_ranged_attack(),
                        std::cmp::Reverse(g.unit_strength(candidate_unit, true) as i64),
                        *candidate,
                    )
                });
            // ⚠⚠ SAY WHY NO ESCORT WAS ATTACHED.
            //
            // Fleet data: settlers escorted in their first three turns found a
            // city 78% of the time against 61% unescorted (n=437), and **31% of
            // settlers depart alone**. Of those, 98% had a military unit
            // somewhere and 68% had one within THREE tiles — so the escort is
            // available and simply not assigned.
            //
            // Three candidate causes were eliminated from recorded runs
            // (`settler_targets` is populated 99% of the time; this step runs
            // BEFORE `doctrine_action`; the unescorted settlers travel FURTHER,
            // not less, so `SETTLER_ESCORT_DISTANCE` is not skipping them). The
            // survivor is `holds_threatened_city`, which rules out every unit
            // within 3 tiles of the plan's threatened city — and a settler is
            // born AT a city, which is exactly the one under threat.
            //
            // That could not be confirmed because `plan.threatened_city` appears
            // in no log. This line is how the next live run answers it, rather
            // than another round of inference. Emitted once per settler by the
            // NEAREST candidate only, so it costs one line per settler-turn.
            if designated.is_none() && self.journal().wants(crate::reasoning::Level::Detail) {
                let nearest_first = g
                    .player_unit_ids(pid)
                    .into_iter()
                    .filter(|c| {
                        let cu = &g.units[c];
                        g.rules.units[cu.kind].class == "military"
                            && !matches!(
                                g.rules.units[cu.kind].domain.as_deref(),
                                Some("sea" | "air")
                            )
                            && g.wdist(cu.pos, settler_pos) <= SETTLER_ESCORT_SEARCH_RADIUS
                    })
                    .min_by_key(|c| (g.wdist(g.units[c].pos, settler_pos), *c));
                if nearest_first == Some(uid) {
                    let mut linked = 0;
                    let mut spent = 0;
                    let mut guarding = 0;
                    let mut near = 0;
                    for candidate in g.player_unit_ids(pid) {
                        let cu = &g.units[&candidate];
                        let spec = &g.rules.units[cu.kind];
                        if spec.class != "military"
                            || matches!(spec.domain.as_deref(), Some("sea" | "air"))
                            || g.wdist(cu.pos, settler_pos) > SETTLER_ESCORT_SEARCH_RADIUS
                        {
                            continue;
                        }
                        near += 1;
                        if cu.linked_to.is_some() {
                            linked += 1;
                        } else if cu.moves_left <= 0.0 {
                            spent += 1;
                        } else if holds_threatened_city(candidate) {
                            guarding += 1;
                        }
                    }
                    think!(self.journal(), Expansion, Detail,
                           "No escort for the settler";
                           "{near} land unit(s) within {SETTLER_ESCORT_SEARCH_RADIUS}: \
                            {linked} already linked, {spent} out of movement, \
                            {guarding} held by a threatened city");
                }
            }
            if designated != Some(uid) {
                continue;
            }
            if unit.pos == settler_pos {
                return Some(
                    g.apply(
                        pid,
                        &Action::LinkUnits {
                            unit: uid,
                            with: settler,
                        },
                    )
                    .is_ok(),
                );
            }
            let moved = self.base.step_toward(g, pid, uid, settler_pos);
            if moved
                && g.units
                    .get(&uid)
                    .is_some_and(|escort| escort.pos == settler_pos)
            {
                let _ = g.apply(
                    pid,
                    &Action::LinkUnits {
                        unit: uid,
                        with: settler,
                    },
                );
            }
            return Some(moved);
        }
        None
    }
}
