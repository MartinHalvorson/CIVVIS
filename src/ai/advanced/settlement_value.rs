//! What a settle site is worth, and finding the best ones.
//!
//! The value of a site: what the city would work as it grows
//! (`settlement_growth_forecast`, `settlement_output_value`), the adjacency its
//! districts could earn (`settlement_adjacency_value_from_positions`), and what
//! the ground offers before any rival is considered
//! (`settlement_static_value_uncached`, cached per turn by
//! `settlement_atlas_values`). What counts against it: the visible threats
//! (`settlement_tile_risk`, `settlement_safety_penalty`), the rival borders near
//! it (`foreign_border_pressure`), and how well it could be held
//! (`defensibility`). The search: `settle_sites` and its cached and scanning
//! forms, ordered first by `settlement_prefilter_score`, with `settle_site_exists`
//! for a caller that only needs to know there is one.
//!
//! `settling.rs` chooses among these sites and walks the Settler there.
//!
//! Moved verbatim out of the `impl AdvancedAi` block in `advanced.rs`
//! (2026-11-06). Methods the rest of the controller calls are `pub(super)`; the
//! code is otherwise unchanged.

use super::*;

impl AdvancedAi {
    pub(super) fn settlement_base_housing(g: &Game, pid: usize, pos: Pos) -> f64 {
        let (fresh, coastal) = g.city_site_water(pid, pos);
        if fresh {
            5.0
        } else if coastal {
            3.0
        } else {
            2.0
        }
    }

    fn settlement_output_value(total: Yields, population: usize) -> f64 {
        let surplus = total.food - 2.0 * population as f64;
        total.production * 2.6
            + total.gold * 0.75
            + total.science * 1.35
            + total.culture * 1.35
            + total.faith * 0.5
            + surplus.max(0.0) * 2.8
            - (-surplus).max(0.0) * 8.0
            // Population itself supplies science/culture and district capacity.
            // Faster growth therefore has value even when the worked yields tie.
            + population as f64 * 1.1
    }

    fn settlement_forecast_rank(state: &SettlementForecastState, horizon: f64) -> f64 {
        let remaining = (horizon - state.elapsed).max(0.0);
        state.accumulated
            + remaining
                * Self::settlement_output_value(state.total, state.reached_population.max(1))
                * 1.05
            + state.reached_population as f64 * 12.0
    }

    /// Forecast which four plots a new city can actually work while growing.
    ///
    /// The old score summed all nineteen plots in two rings, then added the three
    /// best Food values. That treats a population-one settlement as if it works an
    /// entire district immediately and double-counts its center. This model follows
    /// the real bottleneck instead: the free center plus the first, second, third,
    /// and fourth citizen jobs, chosen from the first two rings. First-ring plots
    /// are available immediately; ring two enters only after a short border-growth
    /// delay. Food pays consumption and the engine's exact growth thresholds and
    /// housing bands decide when later jobs begin contributing.
    #[cfg(test)]
    pub(super) fn settlement_growth_forecast(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
    ) -> SettlementGrowthForecast {
        let positions = g.wdisk(pos, 2);
        self.settlement_growth_forecast_from_positions(g, pid, pos, &positions)
    }

    pub(super) fn settlement_growth_forecast_from_positions(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
        positions: &[Pos],
    ) -> SettlementGrowthForecast {
        let mut center = self.site_work_yields(g, pos, &g.map.tiles[&pos]);
        center.food = center.food.max(2.0);
        center.production = center.production.max(1.0);

        let mut candidates = Vec::new();
        for work in positions.iter().copied() {
            if work == pos {
                continue;
            }
            let Some(tile) = g.map.get(work) else {
                continue;
            };
            if tile.owner_city.is_some()
                || !g.rules.is_passable(tile)
                || tile.district.is_some()
                || tile.district_foundation.is_some()
                || tile.wonder.is_some()
                || tile.improvement.as_deref().is_some_and(|improvement| {
                    g.rules.improvements[improvement]
                        .effects
                        .get("unworkable")
                        .copied()
                        .unwrap_or(0.0)
                        > 0.0
                })
            {
                continue;
            }
            let resource_value = tile.resource.as_ref().map_or(0.0, |resource| {
                match g.rules.resources[resource].class.as_str() {
                    "luxury" => 5.0,
                    "strategic" => 4.0,
                    _ => 1.5,
                }
            });
            candidates.push(SettlementWorkTile {
                pos: work,
                ring: g.wdist(pos, work),
                yields: {
                    let mut paid = self.site_work_yields(g, work, tile);
                    // `russia-settles-the-tundra`: zero with the gene off and
                    // for any other civilization.
                    paid.add(self.mother_russia_tile_yields(g, pid, tile));
                    paid
                },
                resource_value,
            });
        }
        candidates.sort_by_key(|tile| (tile.ring, tile.pos));
        if candidates.is_empty() {
            return SettlementGrowthForecast {
                score: Self::settlement_output_value(center, 1) - 20.0,
                reached_population: 1,
                ..SettlementGrowthForecast::default()
            };
        }

        let horizon = g.standard_duration(SETTLEMENT_FORECAST_HORIZON) as f64;
        let ring_two_at = g.standard_duration(SETTLEMENT_SECOND_RING_DELAY) as f64;
        let housing = Self::settlement_base_housing(g, pid, pos);
        let mut beam = vec![SettlementForecastState {
            selected: [0; SETTLEMENT_FORECAST_POPULATION],
            selected_len: 0,
            total: center,
            elapsed: 0.0,
            accumulated: 0.0,
            reached_population: 0,
            reached_four_at: None,
        }];
        let mut terminal = Vec::new();

        for population in 1..=SETTLEMENT_FORECAST_POPULATION {
            let mut next = Vec::new();
            for state in beam.drain(..) {
                if state.elapsed >= horizon - 1e-9 {
                    terminal.push(state);
                    continue;
                }
                for (index, tile) in candidates.iter().enumerate() {
                    if state.selected[..state.selected_len].contains(&index)
                        || (tile.ring == 2 && state.elapsed + 1e-9 < ring_two_at)
                    {
                        continue;
                    }
                    let mut branch = state.clone();
                    branch.selected[branch.selected_len] = index;
                    branch.selected_len += 1;
                    branch.total.add(tile.yields);
                    branch.reached_population = population;
                    let output = Self::settlement_output_value(branch.total, population);

                    if population == SETTLEMENT_FORECAST_POPULATION {
                        branch.reached_four_at = Some(branch.elapsed);
                        branch.accumulated += (horizon - branch.elapsed).max(0.0) * output;
                        branch.elapsed = horizon;
                        terminal.push(branch);
                        continue;
                    }

                    let surplus = branch.total.food - 2.0 * population as f64;
                    let growth_rate =
                        surplus.max(0.0) * Game::housing_growth_mult(housing - population as f64);
                    let remaining = (horizon - branch.elapsed).max(0.0);
                    let growth_turns = if growth_rate > 1e-9 {
                        g.growth_cost(population as i32) / growth_rate
                    } else {
                        f64::INFINITY
                    };
                    let duration = growth_turns.min(remaining);
                    branch.accumulated += duration * output;
                    branch.elapsed += duration;
                    if growth_turns <= remaining + 1e-9 {
                        next.push(branch);
                    } else {
                        terminal.push(branch);
                    }
                }
            }
            let mut forecast_order =
                |left: &SettlementForecastState, right: &SettlementForecastState| {
                    Self::settlement_forecast_rank(right, horizon)
                        .total_cmp(&Self::settlement_forecast_rank(left, horizon))
                        .then_with(|| {
                            left.selected[..left.selected_len]
                                .cmp(&right.selected[..right.selected_len])
                        })
                };
            // Only the best beam-width states survive. Partition first so a
            // large branch layer does not pay a full sort for discarded
            // states; sort the retained prefix with the same total ordering
            // to preserve deterministic tie-breaking.
            if next.len() > SETTLEMENT_FORECAST_BEAM {
                next.select_nth_unstable_by(SETTLEMENT_FORECAST_BEAM - 1, &mut forecast_order);
                next.truncate(SETTLEMENT_FORECAST_BEAM);
            }
            next.sort_by(&mut forecast_order);
            beam = next;
            if beam.is_empty() {
                break;
            }
        }
        for mut state in beam {
            let output =
                Self::settlement_output_value(state.total, state.reached_population.max(1));
            state.accumulated += (horizon - state.elapsed).max(0.0) * output;
            state.elapsed = horizon;
            terminal.push(state);
        }
        let Some(best) = terminal.into_iter().max_by(|left, right| {
            left.accumulated
                .total_cmp(&right.accumulated)
                .then_with(|| {
                    right.selected[..right.selected_len].cmp(&left.selected[..left.selected_len])
                })
        }) else {
            return SettlementGrowthForecast::default();
        };
        let worked = best
            .selected
            .iter()
            .take(best.selected_len)
            .map(|index| candidates[*index])
            .collect::<Vec<_>>();
        let resource_value = worked
            .iter()
            .enumerate()
            .map(|(index, tile)| tile.resource_value * (1.0 - index as f64 * 0.14))
            .sum::<f64>();
        let reached_population = best.reached_population.max(1);
        let growth_failure = SETTLEMENT_FORECAST_POPULATION.saturating_sub(reached_population);
        SettlementGrowthForecast {
            // Convert the forty-turn output integral back to the scale used by
            // `settle_sites`, then make failure to reach useful population explicit.
            score: best.accumulated / horizon.max(1.0) * 5.0
                + resource_value
                + reached_population as f64 * 3.0
                - growth_failure as f64 * 12.0,
            worked,
            turns_to_four: best.reached_four_at,
            reached_population,
        }
    }

    pub(super) fn settlement_adjacency_value_from_positions(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
        positions: &[Pos],
    ) -> f64 {
        if !self.adjacency_site_planning {
            return 0.0;
        }
        let potential = g.settlement_adjacency_summary_from_positions(pid, pos, positions);
        (Self::settlement_yield_value(potential) * 0.5).min(24.0)
    }

    /// What a settlement scan pays for one point of each yield.
    ///
    /// One vector, four readers: the prefilter's own tile score, the adjacency
    /// projection it is compared against, the frozen legacy site value, and
    /// the wonder-site projection in `wonder_sites.rs`. They were four
    /// hand-copied literals, which is the shape where a re-weighting lands on
    /// three of four scans and the ranking quietly stops agreeing with the
    /// score it is a prefilter for.
    const SETTLEMENT_YIELD_WEIGHTS: Yields = Yields {
        food: 2.0,
        production: 2.2,
        gold: 0.7,
        science: 1.2,
        culture: 1.2,
        faith: 0.4,
    };

    /// `yields` priced at [`AdvancedAi::SETTLEMENT_YIELD_WEIGHTS`], summed in
    /// the order the four copies summed it — a settle ranking is decided on
    /// hundredths, so the association is part of the answer.
    pub(super) fn settlement_yield_value(yields: Yields) -> f64 {
        let weights = Self::SETTLEMENT_YIELD_WEIGHTS;
        yields.food * weights.food
            + yields.production * weights.production
            + yields.gold * weights.gold
            + yields.science * weights.science
            + yields.culture * weights.culture
            + yields.faith * weights.faith
    }

    /// Order a global candidate scan before invoking the full growth,
    /// adjacency, and safety model. This deliberately uses only local yields
    /// and resources, so it is cheap and cannot observe hidden state.
    pub(super) fn settlement_prefilter_score(g: &Game, pid: usize, pos: Pos) -> f64 {
        let value_of = |tile: &crate::world::Tile, weight: f64| {
            let yields = g.rules.tile_yields(tile);
            let resource = tile.resource.as_ref().map_or(0.0, |resource| {
                match g.rules.resources[resource].class.as_str() {
                    "luxury" => 5.0,
                    "strategic" => 4.0,
                    _ => 1.5,
                }
            });
            weight * (Self::settlement_yield_value(yields) + resource)
        };
        let tile = &g.map.tiles[&pos];
        let mut value = value_of(tile, 1.5);
        let (fresh, coastal) = g.city_site_water(pid, pos);
        for neighbor in g.nbrs(pos) {
            let Some(tile) = g.map.get(neighbor) else {
                continue;
            };
            value += value_of(tile, 1.0);
        }
        value
            + if fresh {
                14.0
            } else if coastal {
                6.0
            } else {
                -5.0
            }
    }

    /// Compute the portion of the live settlement score that is independent
    /// of the settler's origin and of current unit positions. The caller adds
    /// live safety/support terms after retrieving this value from the atlas.
    pub(super) fn settlement_static_value_uncached(&self, g: &Game, pid: usize, pos: Pos) -> f64 {
        let positions = g.wdisk(pos, 2);
        let forecast = self.settlement_growth_forecast_from_positions(g, pid, pos, &positions);
        let housing = Self::settlement_base_housing(g, pid, pos);
        let horizon = g.standard_duration(SETTLEMENT_FORECAST_HORIZON) as f64;
        let growth_readiness = forecast.turns_to_four.map_or_else(
            || {
                -4.0 * SETTLEMENT_FORECAST_POPULATION.saturating_sub(forecast.reached_population)
                    as f64
            },
            |turns| 8.0 * (1.0 - turns / horizon.max(1.0)).max(0.0),
        );
        let dependable_jobs = forecast
            .worked
            .iter()
            .take(SETTLEMENT_FORECAST_POPULATION)
            .filter(|tile| tile.yields.food >= 2.0 || tile.yields.production >= 2.0)
            .count() as f64;
        let mut value =
            forecast.score + (housing - 2.0) * 4.0 + growth_readiness + dependable_jobs * 0.75;
        value += self.settlement_adjacency_value_from_positions(g, pid, pos, &positions);
        value += self.coastal_city_site_bonus(g, pos, &positions);
        // `fresh-water-prices-the-site`: zero with the gene off. See
        // `advanced/fresh_water_site.rs`.
        value += self.fresh_water_site_bonus(g, pid, pos);
        // `nusantara-settles-the-shore`: zero with the gene off and for any
        // other civilization. See `advanced/nusantara_settles_the_shore.rs`.
        value += self.nusantara_shore_site_bonus(g, pid, pos);
        value += self.wonder_footprint_value(g, &positions);
        value += self.water_science_wonder_site_value(g, pid, pos);
        value += Self::early_city_water_adjustment(g, pid, pos);
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
        value
    }

    /// Evaluate static site values, using the persistent pool only for a
    /// sufficiently large miss batch. Every result is returned in input order
    /// and each candidate's arithmetic is independent, so worker scheduling
    /// cannot affect tie-breaking or floating-point reduction order.
    fn settlement_static_values_uncached(
        &self,
        g: &Game,
        pid: usize,
        positions: &[Pos],
    ) -> Vec<(Pos, f64)> {
        let pool = self
            .work_pool
            .as_ref()
            .filter(|pool| pool.threads() > 1 && positions.len() >= 32);
        if let Some(pool) = pool {
            let active = pool
                .threads()
                .min(positions.len())
                .min(SETTLEMENT_SCORE_MAX_WORKERS);
            let states = (0..active)
                .map(|_| (g.speculative_clone(), self.clone()))
                .collect::<Vec<_>>();
            let positions = Arc::new(positions.to_vec());
            let worker_positions = Arc::clone(&positions);
            let values = pool.map_stateful_limited(
                positions.len(),
                SETTLEMENT_SCORE_MAX_WORKERS,
                states,
                move |(game, ai), indices| {
                    indices
                        .map(|index| {
                            let pos = worker_positions[index];
                            (index, ai.settlement_static_value_uncached(&game, pid, pos))
                        })
                        .collect()
                },
            );
            return positions.iter().copied().zip(values).collect();
        }

        positions
            .iter()
            .copied()
            .map(|pos| (pos, self.settlement_static_value_uncached(g, pid, pos)))
            .collect()
    }

    /// Fill the turn-scoped atlas for the supplied positions. The atlas is
    /// deliberately disabled outside an active controller turn: public
    /// advice/evaluator calls can mutate a game between queries without
    /// passing through this controller's lifecycle, and those callers retain
    /// the original uncached semantics.
    pub(super) fn settlement_atlas_values(&self, g: &Game, pid: usize, positions: &[Pos]) {
        if self.battlefront_frame.is_none() || positions.is_empty() {
            return;
        }
        let misses = {
            let mut atlas = self.settlement_atlas.borrow_mut();
            if !atlas.matches(g, pid) {
                atlas.start(g, pid);
            }
            positions
                .iter()
                .copied()
                .filter(|pos| !atlas.values.contains_key(pos))
                .collect::<Vec<_>>()
        };
        if misses.is_empty() {
            return;
        }
        let values = self.settlement_static_values_uncached(g, pid, &misses);
        let mut atlas = self.settlement_atlas.borrow_mut();
        if !atlas.matches(g, pid) {
            atlas.start(g, pid);
        }
        atlas.values.extend(values);
    }

    pub(super) fn settlement_atlas_value(&self, g: &Game, pid: usize, pos: Pos) -> f64 {
        self.settlement_atlas_values(g, pid, &[pos]);
        self.settlement_atlas
            .borrow()
            .values
            .get(&pos)
            .copied()
            .unwrap_or_else(|| self.settlement_static_value_uncached(g, pid, pos))
    }

    pub(super) fn settle_value_visible(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
        visible: &TileBits,
    ) -> f64 {
        let mut value = if self.battlefront_frame.is_some() {
            self.settlement_atlas_value(g, pid, pos)
        } else {
            self.settlement_static_value_uncached(g, pid, pos)
        };
        value -= self.settlement_safety_penalty(g, pid, pos, visible);
        // `early-cities-keep-their-distance`: zero with the gene off and from
        // the third city on. See `advanced/early_city_distance.rs`.
        value -= self.early_city_rival_penalty(g, pid, pos);
        // `capital-leaves-the-frost`: the first city's frozen ground. Zero
        // with the gene off and once the empire holds a city.
        value -= self.frost_capital_penalty(g, pid, pos);
        // `contested_land_first`: the ground between us and a neighbour,
        // while the army can hold it. Zero with the gene off.
        value += self.contested_land_credit(g, pid, pos);
        if self.defensible_sites {
            value += self.defensibility(g, pid, pos);
        }
        // `chokepoint-siting` / `canal-city`: the gates this city's own
        // ground would hold, and the two seas its center would join. Both
        // return 0.0 with their gene off. See `advanced/chokepoints.rs`.
        value += self.chokepoint_site_bonus(g, pid, pos);
        value += self.canal_city_bonus(g, pid, pos);
        value
    }

    /// Penalize a site for threats the acting player can actually see. Hidden
    /// units do not enter this term: a full-state route would make CivVis
    /// settlers clairvoyant, while an obvious visible attack envelope is
    /// precisely the risk the player can reasonably avoid.
    pub(super) fn settlement_tile_risk(
        &self,
        g: &Game,
        pid: usize,
        uid: Option<u32>,
        pos: Pos,
        visible: &TileBits,
    ) -> f64 {
        self.settlement_tile_risk_with_support(g, pid, uid, pos, visible, true)
    }

    pub(super) fn settlement_tile_risk_with_support(
        &self,
        g: &Game,
        pid: usize,
        uid: Option<u32>,
        pos: Pos,
        visible: &TileBits,
        discount_support: bool,
    ) -> f64 {
        let escorted = discount_support
            && uid
                .and_then(|unit| g.units.get(&unit))
                .and_then(|unit| unit.linked_to)
                .and_then(|escort| g.units.get(&escort))
                .is_some_and(|escort| {
                    escort.owner == pid
                        && g.rules.units[escort.kind].class == "military"
                        && g.rules.units[escort.kind].domain.as_deref() != Some("air")
                });
        // See `settler_stack_discipline`: a civilian mover — a Settler, or an
        // opted-in Builder rather than the military leader of a formation — is
        // captured by any hostile that can enter its tile, and is protected
        // only by a unit standing ON it.
        let civilian_mover = uid.and_then(|unit| g.units.get(&unit)).is_some_and(|unit| {
            let civilian = g.rules.units[unit.kind].class != "military";
            civilian
                && (self.live_formationless_settler_shadow
                    || (self.builder_barbarian_safety && unit.kind == "builder"))
        });
        let escort_reach = if civilian_mover { 0 } else { 1 };
        // A stacked guard mirrors the settler's step (`stacked_guard_step`
        // runs after the settler and walks onto its tile), so a settler that
        // starts the move with its bound guard on its own tile is priced as
        // if the guard will stand on the destination too — which is how a
        // pair walks through raider country at all. A guard too hurt to
        // survive an attack is no shield.
        let stacked_guard = civilian_mover
            && uid.is_some_and(|settler| {
                let settler_pos = g.units[&settler].pos;
                let target_is_water = g.map.get(pos).is_some_and(|tile| g.rules.is_water(tile));
                self.settler_guard_ids(settler)
                    .into_iter()
                    .flatten()
                    .any(|guard| {
                        g.units.get(&guard).is_some_and(|guard| {
                            let sea = g.rules.units[guard.kind].domain.as_deref() == Some("sea");
                            guard.owner == pid
                            // See `settler_guard_holds`: the guard mirrors the
                            // settler's step from its tile, or already stands
                            // on the destination and holds there.
                            && (guard.pos == settler_pos
                                || (self.settler_guard_holds_on() && guard.pos == pos))
                            && guard.hp >= STACKED_GUARD_MIN_HP
                            && g.rules.units[guard.kind].class == "military"
                            && g.rules.units[guard.kind].domain.as_deref() != Some("air")
                            // The naval layer protects the water leg only.
                            // The embarked land guard is the one that walks
                            // ashore with the Settler.
                            && (!sea || target_is_water)
                            // See `settler_guard_holds`: a guard the first
                            // hostile in reach would break is no protection.
                            && (!self.settler_guard_holds_on()
                                || !self.guard_outmatched_at(g, pid, guard, pos, visible))
                        })
                    })
            });
        // See `settler_guard_holds`: for a civilian only the bound guard counts —
        // any other own unit on the tile follows its own doctrine and walks
        // off, and that is how the settlers of civvis-20260819T025840Z were
        // taken one tile outside their own capital.
        let nearby_escort = discount_support
            && (stacked_guard
                || ((!civilian_mover || !self.settler_guard_holds_on())
                    && g.units.values().any(|unit| {
                        unit.owner == pid
                            && g.wdist(unit.pos, pos) <= escort_reach
                            && g.rules.units[unit.kind].class == "military"
                            && g.rules.units[unit.kind].domain.as_deref() != Some("air")
                            && g.rules.units[unit.kind].domain.as_deref() != Some("sea")
                            && (!civilian_mover || unit.hp >= STACKED_GUARD_MIN_HP)
                    })));
        // A civilian inside one of our own cities cannot be taken until the
        // city is.
        if civilian_mover
            && g.city_at(pos)
                .is_some_and(|cid| g.cities[&cid].owner == pid)
        {
            return 0.0;
        }
        // ★★★★ A WARSHIP OFF THE COAST IS A THREAT TO A SETTLER ON THE SHORE.
        // Sea units were skipped outright, so a settler's step safety, its
        // site risk and its guard-wait fallback all read the shoreline as
        // quiet. Run civvis-20260815T233405Z lost TWO Settlers to one
        // barbarian Galley: at t111 the settler stood on coastal (69,22)
        // with the Galley three hexes out and was gone on t112; the next
        // walked the same shore at t125–130 with the Galley two hexes out
        // and was gone on t131 — no land hostile in sight either time. A
        // naval melee unit reaches any coastal land tile its movement puts
        // it beside, so count sea units exactly there and nowhere inland.
        let coastal = g.nbrs(pos).iter().any(|neighbour| {
            g.map
                .get(*neighbour)
                .is_some_and(|tile| g.rules.is_water(tile))
        });
        let mut risk = 0.0;
        for unit in g.units.values() {
            if unit.owner == pid
                || !g.sees(visible, unit.pos)
                || !g.is_at_war(pid, unit.owner)
                || !g.unit_visible_to(unit.id, pid)
            {
                continue;
            }
            let spec = &g.rules.units[unit.kind];
            let domain = spec.domain.as_deref();
            if spec.class != "military"
                || domain == Some("air")
                || (domain == Some("sea") && !coastal)
            {
                continue;
            }
            // See `barbarian_scouts_are_scouts`: the host's barbarian scouts
            // explore and run home; they neither attack nor capture.
            if self.barbarian_scouts_are_scouts
                && g.players[unit.owner].is_barbarian
                && spec.promotion_class == "recon"
            {
                continue;
            }
            let distance = g.wdist(unit.pos, pos);
            // Combat strength contributes only inside one of the threat ranges below.
            // Compute it only after a range test succeeds.
            let strength =
                || crate::game::effective_strength(g.unit_strength(unit, false), unit.hp).max(1.0);
            // ★★★★ FOR A CIVILIAN, REACH IS CAPTURE. See
            // `settler_stack_discipline`: a hostile land unit that can enter
            // the tile next turn takes the civilian whatever its range, so it
            // is priced at the capture tier; one tile beyond its movement it
            // is a threat to the turn after, the approach tier.
            if civilian_mover && domain != Some("sea") {
                let capture_reach = spec.moves.ceil() as i32;
                if distance <= capture_reach {
                    risk += 34.0 + strength() * 0.45;
                } else if distance <= capture_reach + 1 {
                    risk += 12.0 + strength() * 0.15;
                }
                continue;
            }
            let attack_range = if spec.has_ranged_attack() {
                g.unit_attack_range(unit.id)
            } else {
                1
            };
            let approach_range = attack_range + spec.moves.ceil() as i32;
            if domain == Some("sea") {
                // A ship moves and strikes in one turn and a civilian on the
                // shore cannot answer it: anything the ship can reach this
                // turn is as good as in range, not merely "approaching".
                if distance <= approach_range {
                    risk += 34.0 + strength() * 0.45;
                }
                continue;
            }
            if distance <= attack_range {
                risk += 34.0 + strength() * 0.45;
            } else if distance <= approach_range {
                risk += 12.0 + strength() * 0.15;
            }
        }
        // ★★★★ A KILL THE BOARD PREDICTS IS NOT A KILL THE HOST MADE. See
        // `turn_start_hostiles`: a hostile that stood here at the start of
        // the turn and is gone from the board now was removed by our own
        // planning — the finishing volley, an earlier unit's strike — and the
        // host may well leave it standing on a few hit points. For a
        // civilian's step it is priced exactly as if it were still there.
        if civilian_mover && self.turn_start_hostiles_turn == Some(g.turn) {
            for gone in &self.turn_start_hostiles {
                if g.units.contains_key(&gone.id) || !g.sees(visible, gone.pos) {
                    continue;
                }
                let distance = g.wdist(gone.pos, pos);
                if gone.sea {
                    if coastal && distance <= gone.capture_reach + 1 {
                        risk += 34.0 + gone.strength * 0.45;
                    }
                    continue;
                }
                if distance <= gone.capture_reach {
                    risk += 34.0 + gone.strength * 0.45;
                } else if distance <= gone.capture_reach + 1 {
                    risk += 12.0 + gone.strength * 0.15;
                }
            }
        }
        for city in g.cities.values() {
            if city.owner == pid || !g.is_at_war(pid, city.owner) || !g.sees(visible, city.pos) {
                continue;
            }
            let distance = g.wdist(city.pos, pos);
            if distance <= 3 {
                risk += (4 - distance) as f64 * 7.0;
            }
        }
        for camp in g.barb_camps.keys() {
            if g.sees(visible, *camp) {
                let distance = g.wdist(*camp, pos);
                if distance <= 3 {
                    risk += (4 - distance) as f64 * 6.0;
                }
            }
        }
        risk * if escorted {
            0.25
        } else if nearby_escort && civilian_mover {
            // A unit on the tile: the civilian cannot be taken while it stands.
            0.15
        } else if nearby_escort {
            0.45
        } else {
            1.0
        }
    }

    pub(super) fn settlement_safety_penalty(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
        visible: &TileBits,
    ) -> f64 {
        let support_distance = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.wdist(pos, g.cities[&cid].pos))
            .min();
        let isolation = support_distance
            .filter(|distance| *distance > 6)
            .map(|distance| (distance - 6).min(8) as f64 * 3.5)
            .unwrap_or(0.0);
        // ★★★★ A SITE PRESSED AGAINST A RIVAL'S BORDER IS A PROVOCATION, and
        // on the live Settler seat it has been answered with a war three times
        // in five games, each within ten turns of the founding, each costing
        // cities: T021044Z Puteoli (t65, "worth −22.7", beside Babylon → war
        // t67, two cities lost), T040537Z Ostia (t53) and Mediolanum (t66)
        // beside an unseen Khmer → war t61, both captured and the capital with
        // them, T045316Z Lugdunum/Mediolanum (t52/56) beside Sweden → war t60.
        // Firaxis' AI holds a grievance for settling near it and reads a
        // border-hugging city as the weak one to take; nothing in the site
        // value knew a border was there — the loyalty forecast asks about
        // cities, and the cities were in fog. The border is visible ground.
        // Behind `settlement_safety`, the live seat's settler-survival gate.
        // `contested_land_first`: pressing a neighbour's border is the point
        // where the ground is contested and the army can hold it. See
        // `advanced/contested_land.rs`.
        let provocation =
            if self.settlement_safety && !self.contested_land_waives_provocation(g, pid, pos) {
                self.foreign_border_pressure(g, pid, pos)
            } else {
                0.0
            };
        self.settlement_tile_risk(g, pid, None, pos, visible) + isolation + provocation
    }

    /// How much of the ground within `FOREIGN_BORDER_RADIUS` of `pos` a rival
    /// major already owns, priced per tile and capped. See
    /// `settlement_safety_penalty`.
    pub(super) fn foreign_border_pressure(&self, g: &Game, pid: usize, pos: Pos) -> f64 {
        let owned = g
            .wdisk(pos, FOREIGN_BORDER_RADIUS)
            .into_iter()
            .filter(|tile| {
                g.map
                    .get(*tile)
                    .and_then(|t| t.owner_city)
                    .and_then(|city| g.cities.get(&city))
                    .map(|city| city.owner)
                    .is_some_and(|owner| {
                        owner != pid
                            && g.players
                                .get(owner)
                                .is_some_and(|p| p.alive && !p.is_minor && !p.is_barbarian)
                    })
            })
            .count();
        (owned as f64 * FOREIGN_BORDER_TILE_PENALTY).min(FOREIGN_BORDER_PENALTY_CAP)
    }

    /// How well a site can be held, on the same scale `settle_value` uses for
    /// everything else. Never positive: this only ever discounts a site.
    ///
    /// Two terms, for the two things the shipped score is silent about. A camp
    /// or barbarian city nearby is what actually takes new cities, and it is
    /// filtered out of the rival-proximity penalty above. Distance from the
    /// empire's own nearest city decides whether help can arrive at all: the
    /// measured loss lands at city age ten, which is less time than a soldier
    /// needs to cross open ground.
    ///
    /// Six tiles is the same threshold the rival-proximity penalty uses, and
    /// the isolation term is capped so a first city — which has no other city
    /// to be near — cannot be discounted without bound.
    pub(super) fn defensibility(&self, g: &Game, pid: usize, pos: Pos) -> f64 {
        let camp_distance = g
            .barb_camps
            .keys()
            .chain(
                g.cities
                    .values()
                    .filter(|city| g.players[city.owner].is_barbarian)
                    .map(|city| &city.pos),
            )
            .map(|camp| g.wdist(pos, *camp))
            .min()
            .unwrap_or(20);
        let exposure = match camp_distance < 6 {
            true => (6 - camp_distance) as f64 * 7.0,
            false => 0.0,
        };
        let support_distance = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.wdist(pos, g.cities[&cid].pos))
            .min()
            .unwrap_or(0);
        let isolation = match support_distance > 6 {
            true => (support_distance - 6).min(6) as f64 * 5.0,
            false => 0.0,
        };
        -(exposure + isolation)
    }

    /// Rank settleable ground the way this agent would, for a caller outside the
    /// engine.
    ///
    /// `civvis-advise` uses this to turn a mirrored Civilization VI board into a
    /// settle plan the control mod can prefer. It is deliberately the SAME ranking
    /// the agent settles by — a plan derived from a second, parallel notion of a good
    /// site would be measuring something no CIVVIS game has ever played.
    ///
    /// ⚠ `from` and the returned positions are AXIAL, like everything else in the
    /// engine. The Civilization VI export is OFFSET; convert at the boundary with
    /// `hex::offset_to_axial`. Getting this wrong is silent — both are pairs of small
    /// integers — and it cost this project a run that reported "no legal revealed
    /// site" on a map with 323 revealed plots.
    pub fn settle_ranking(&self, g: &Game, pid: usize, from: Pos, radius: i32) -> Vec<(Pos, f64)> {
        self.settle_sites(g, pid, from, radius)
    }

    pub(super) fn settle_sites(
        &self,
        g: &Game,
        pid: usize,
        from: Pos,
        radius: i32,
    ) -> Vec<(Pos, f64)> {
        self.settle_sites_with_limit(g, pid, from, radius, None)
    }

    pub(super) fn settle_sites_with_limit(
        &self,
        g: &Game,
        pid: usize,
        from: Pos,
        radius: i32,
        prefilter_limit: Option<usize>,
    ) -> Vec<(Pos, f64)> {
        self.settle_sites_with_limit_cached(g, pid, from, radius, prefilter_limit, None)
    }

    pub(super) fn settle_sites_with_limit_cached(
        &self,
        g: &Game,
        pid: usize,
        from: Pos,
        radius: i32,
        prefilter_limit: Option<usize>,
        score_cache: Option<&mut BTreeMap<Pos, f64>>,
    ) -> Vec<(Pos, f64)> {
        self.settle_sites_scanning(
            g,
            pid,
            from,
            radius,
            prefilter_limit,
            score_cache,
            false,
            false,
        )
    }

    /// Whether any site this search would return exists, without ranking the
    /// ones it does not need.
    ///
    /// ★★★★★ FOUR OF THE SIX `best_settle_site` CALLERS ASK `.is_some()`. They
    /// discard a fully scored and sorted candidate list down to one bit, and
    /// the list is not cheap: each candidate runs `settle_value_visible`, whose
    /// static half is a growth forecast over the site's radius-2 disk.
    ///
    /// Measured on a 6-player 74×46 150-turn online game: 965 scans scored
    /// **148,673** candidates, and the first qualifying site appeared after
    /// **1.4** of them on average — every scan found one, none ran dry. An
    /// existence query therefore needs about 1% of the scoring the ranking
    /// query needs.
    ///
    /// This shares the ranking implementation rather than restating the
    /// candidate filter, because a second copy of that filter is a second
    /// place for "settleable" to drift.
    pub(super) fn settle_site_exists(&self, g: &Game, pid: usize, from: Pos, radius: i32) -> bool {
        !self
            .settle_sites_scanning(g, pid, from, radius, None, None, true, false)
            .is_empty()
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn settle_sites_scanning(
        &self,
        g: &Game,
        pid: usize,
        from: Pos,
        radius: i32,
        prefilter_limit: Option<usize>,
        score_cache: Option<&mut BTreeMap<Pos, f64>>,
        stop_at_first: bool,
        include_emergency: bool,
    ) -> Vec<(Pos, f64)> {
        self.settle_sites_scanning_shared(
            g,
            pid,
            from,
            radius,
            prefilter_limit,
            score_cache,
            stop_at_first,
            include_emergency,
            None,
        )
    }
}
