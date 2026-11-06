//! The navy: which ships, where they can launch and chart, and the water
//! research behind them.
//!
//! Where a hull can go: `city_is_coastal`, `city_has_open_water`,
//! `naval_city_can_launch`, and `waterborne` and `empire_is_coastal`. Whether
//! the empire wants a sea scout: `recon_is_the_missing_arm`,
//! `naval_recon_major_war`, `naval_recon_yields_to_land_recon`. What a sea scout
//! could chart: `naval_recon_waterway` and `naval_recon_can_chart_from`, with
//! `unseen_water_remains` and `is_sea_frontier`. What to build and research:
//! `best_naval_unit`, `naval_counts`, `naval_ambitions`, `water_research_goal`.
//!
//! Moved verbatim out of the `impl BasicAi` blocks in `ai.rs` (2026-11-06).
//! Methods the rest of `ai` calls are `pub(super)`, the same reach a private
//! method of `ai` had; the code is otherwise unchanged.

use super::*;

impl BasicAi {
    pub(super) fn best_naval_unit(&self, g: &Game, pid: usize, cid: u32) -> Option<Name> {
        // Open water, not merely water. See `city_has_open_water`: this is the
        // enqueue path that was building the stranded lake fleet.
        let launchable = if self.open_water_navy {
            Self::city_has_open_water(g, cid)
        } else {
            Self::city_is_coastal(g, cid)
        };
        if !launchable {
            return None;
        }
        let (total, melee, ranged, raiders, carriers) = Self::naval_counts(g, pid);
        let has_aircraft = g.units.values().any(|unit| {
            unit.owner == pid && g.rules.units[unit.kind].domain.as_deref() == Some("air")
        });
        g.rules
            .units
            .iter()
            .filter(|(name, spec)| {
                spec.class == "military"
                    && spec.domain.as_deref() == Some("sea")
                    && g.can_produce(pid, cid, &Item::Unit { unit: **name })
            })
            .map(|(name, spec)| {
                let power = spec.strength.max(spec.ranged_attack_strength());
                let role = match spec.promotion_class.as_str() {
                    // A navy without melee ships can bombard but never take a
                    // coastal city; preserve at least half the fleet for that
                    // capturing/screening role.
                    "naval_melee" => 42.0 * (melee <= ranged + raiders) as i32 as f64,
                    "naval_ranged" => 34.0 * (ranged < melee.max(1)) as i32 as f64,
                    "naval_raider" => 22.0 * (total >= 2 && raiders == 0) as i32 as f64,
                    "naval_carrier" => {
                        if has_aircraft && carriers == 0 {
                            30.0
                        } else {
                            -120.0
                        }
                    }
                    _ => 0.0,
                };
                (power * 3.0 + role - spec.cost * 0.04, *name)
            })
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then_with(|| b.1.cmp(&a.1)))
            .map(|(_, name)| name)
    }

    /// Whether this empire has stopped being able to find anything.
    ///
    /// ★★★★★ THE SCOUTS DIE AND NOTHING REPLACES THEM, so the map stops being
    /// revealed and the game is played against whichever rivals happen to live
    /// next door. `pick_item` is handed counts of settlers, builders, traders,
    /// siege support, military, melee and ranged — recon is not among them, and
    /// `OPENING_MENU` is the only place a scout is ever named. Once the openers
    /// are gone the empire is permanently blind and nothing in the build order
    /// notices.
    ///
    /// Measured on live run `civvis-20260808T142724Z`, 251 turns:
    ///
    /// | turn | 25 | 75 | 100 | 150 | 200 | 250 |
    /// |------|----|----|-----|-----|-----|-----|
    /// | recon units | 1 | 2 | **0** | **0** | **0** | **0** |
    /// | majors met | 1 | 2 | 2 | 2 | 2 | 4 |
    /// | total units | 5 | 12 | 13 | 20 | 11 | 22 |
    ///
    /// Zero recon from turn ~100 to the end — 150 turns — while the army grew
    /// to 22. Not one of the run's logged production decisions is a scout. At
    /// the final tile export **2,631 of 3,404 plots (77%) had never been seen**.
    /// The cost is not map trivia: of five major rivals, two were met in the
    /// first 40 turns and the other three stayed invisible, so the eventual
    /// winner was first seen on **turn 215 already holding 927 points** against
    /// our ~540. For 86% of that game every strategic assessment — campaign
    /// target, runaway-rival pressure, victory-lane denial — ran against the
    /// two weakest civilizations on the board, and reported us in the lead.
    ///
    /// Built from the BOARD like [`BasicAi::siege_is_the_missing_arm`] above,
    /// and for the same reason: this lives in `BasicAi`, the strategic plan
    /// does not reach here, and both facts it needs are already present. The
    /// live bridge mirrors a full-size map and reveals into `explored` as a
    /// strict subset of it, so the unexplored test is as live against
    /// Civilization VI as it is in a native game.
    ///
    /// ⚠ Bounded by one eye before city two and two afterwards. A developed
    /// empire that has missed a majority of at least three city-states can add
    /// exactly one third eye, but only after both ordinary Scouts are live;
    /// this cannot turn the opening or a two-Scout rebuild into Scout spam.
    /// Every arm also releases once there is no land left to chart.
    fn city_state_sweep_needs_third_eye(
        g: &Game,
        pid: usize,
        city_count: usize,
        owned_recon: usize,
    ) -> bool {
        if city_count < RECON_CITY_STATE_SWEEP_CITY_FLOOR || owned_recon < RECON_ARM_MAX {
            return false;
        }
        let (met, unmet) = g
            .players
            .iter()
            .filter(|player| {
                player.alive && player.is_minor && !player.is_barbarian && !player.is_free_city
            })
            .fold((0usize, 0usize), |(met, unmet), player| {
                if g.has_met(pid, player.id) {
                    (met + 1, unmet)
                } else {
                    (met, unmet + 1)
                }
            });
        unmet >= RECON_CITY_STATE_SWEEP_UNMET_MIN && unmet > met
    }

    pub(super) fn recon_is_the_missing_arm(&self, g: &Game, pid: usize) -> bool {
        if !self.recon_replacement || self.minor || self.barb {
            return false;
        }
        let owned_recon = g
            .units
            .values()
            .filter(|unit| {
                unit.owner == pid && Self::unit_doctrine(g, unit.id) == UnitDoctrine::Recon
            })
            .count();
        // ★★★★ A SCOUT ALREADY ON THE WAY IS AN EYE. This counted only recon
        // units standing on the map, so on any turn the empire had none,
        // EVERY city that came to `pick_item` read "the empire has no eyes"
        // and answered it: run civvis-20260816T213447Z, t139, five cities
        // start Skirmishers; t140, all seven start Rangers, and seven Rangers
        // are built. One in a queue anywhere is the arm being rebuilt.
        let queued_recon = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                g.cities[cid].queue.iter().any(|item| {
                    matches!(item, Item::Unit { unit }
                    if g.rules.units.get(unit.as_str()).is_some_and(|spec| {
                        spec.class == "military" && spec.promotion_class == "recon"
                    }))
                })
            })
            .count();
        // A one-city empire gets the original single opening Scout. Once it
        // has two cities, retain one spare explorer: the last live match had
        // eleven city-states but met only five after its only Scout spent the
        // closing turns retreating around a hostile coast. The city gate keeps
        // that redundancy from stealing the initial Settler/Builder cadence.
        let city_count = g.player_city_ids(pid).len();
        let recon_arm_target =
            if Self::city_state_sweep_needs_third_eye(g, pid, city_count, owned_recon) {
                RECON_CITY_STATE_SWEEP_ARM_MAX
            } else if city_count >= RECON_SECOND_EYE_CITY_FLOOR {
                RECON_ARM_MAX
            } else {
                1
            };
        if owned_recon + queued_recon >= recon_arm_target {
            return false;
        }
        // Somewhere left to go. A land scout is what this buys, so ask about
        // ground it could actually walk to rather than the whole globe —
        // otherwise an empire that has charted its continent keeps building
        // scouts to stare at an ocean.
        g.map.tiles.iter().any(|(pos, tile)| {
            !g.players[pid].explored.contains(pos)
                && g.rules.is_passable(tile)
                && !g.rules.is_water(tile)
        })
    }

    /// Whether the sea arm needs its wartime redundancy. Keep this separate
    /// from the arm count so version 3 can yield an idle peacetime queue to a
    /// land scout without weakening the existing second-eye exception.
    pub(super) fn naval_recon_major_war(&self, g: &Game, pid: usize) -> bool {
        g.players.iter().any(|enemy| {
            enemy.id != pid
                && enemy.alive
                && !enemy.is_minor
                && (!enemy.is_barbarian || self.sea_answers)
                && g.is_at_war(pid, enemy.id)
                && (g.units.values().any(|unit| {
                    unit.owner == enemy.id
                        && g.map
                            .get(unit.pos)
                            .is_some_and(|tile| g.rules.is_water(tile))
                }) || (!enemy.is_barbarian
                    && g.player_city_ids(enemy.id)
                        .into_iter()
                        .any(|cid| Self::city_is_coastal(g, cid))))
        })
    }

    /// V3's sea scout is an information complement, not a way to crowd out
    /// the scout that can reveal the empire's own continent. The basic
    /// picker already ranks `recon_pick` ahead of `naval_recon_pick`; the
    /// advanced idle-queue reserver uses this explicit gate to preserve that
    /// same ordering. A naval war remains a production exception because its
    /// second eye may be the only hull still free to chart.
    pub(crate) fn naval_recon_yields_to_land_recon(&self, g: &Game, pid: usize) -> bool {
        self.naval_recon_3
            && !self.naval_recon_major_war(g, pid)
            && self.recon_is_the_missing_arm(g, pid)
    }

    pub(crate) fn city_is_coastal(g: &Game, cid: u32) -> bool {
        g.cities.get(&cid).is_some_and(|city| {
            g.nbrs(city.pos)
                .into_iter()
                .any(|pos| g.map.get(pos).is_some_and(|tile| g.rules.is_water(tile)))
        })
    }

    /// ★★★★★ WHETHER A HULL BUILT HERE HAS ANYWHERE TO SAIL.
    ///
    /// `city_is_coastal` asks only whether some adjacent tile is water, and a
    /// **lake is water**. Firaxis lets a lakeside city build a Galley and the
    /// Galley then spends the whole game on the lake — a real trap in the real
    /// game, faithfully reproduced, and the AI walked into it every time.
    ///
    /// Measured over three 150-turn six-player games: majors built 53 naval
    /// hulls, **20 of which never moved once**, 12 of those sitting on `lake`
    /// terrain. Only 17.2% of major naval unit-turns involved any movement,
    /// and a Galley was idle on 54.3% of its own turns — the largest single
    /// block of wasted major unit-turns in `audit`. Tracing the enqueue showed
    /// 12 of 15 naval builds in one game came from a city whose only adjacent
    /// water was lake.
    ///
    /// The stranded hull costs three times over: the production that built it,
    /// the gold maintenance it draws for the rest of the game, and — because
    /// it counts toward `desired_navy` — the fleet that could have sailed and
    /// now never gets ordered.
    ///
    /// Open water specifically, which is also Firaxis' own rule for a Harbour.
    /// Deliberately the six neighbours and not a flood fill: the exact
    /// question is the size of the connected body, and `production_value`
    /// cannot afford a flood fill per candidate per city per turn.
    pub(crate) fn city_has_open_water(g: &Game, cid: u32) -> bool {
        g.cities.get(&cid).is_some_and(|city| {
            let open_water = |pos| {
                g.map
                    .get(pos)
                    .is_some_and(|tile| matches!(tile.terrain.as_str(), "coast" | "ocean"))
            };
            g.nbrs(city.pos).into_iter().any(open_water)
                // An inland city can launch through its completed Harbor.
                || city.districts.iter().any(|(district, pos)| {
                    g.district_family(*district) == "harbor" && open_water(*pos)
                })
        })
    }

    /// Use the same launch rule for the fleet budget and both production scorers.
    pub(crate) fn naval_city_can_launch(&self, g: &Game, cid: u32) -> bool {
        if self.open_water_navy {
            Self::city_has_open_water(g, cid)
        } else {
            Self::city_is_coastal(g, cid)
        }
    }

    /// Whether there is still water this empire has not seen — the sea's
    /// "somewhere left to go", read by the production arm and by the
    /// explorer roster alike.
    ///
    /// ★★★★★ ON THE LIVE SEAT THIS WAS FALSE FOR THE WHOLE GAME, EVERY GAME.
    /// The original test — a water tile outside `explored` — describes a
    /// native board, where the whole map exists and `explored` is the part
    /// of it the seat has looked at. The Civilization VI mirror has no such
    /// tile: it writes only revealed plots, `explored` IS the revealed set,
    /// and everything else is `unknown` terrain, which is not water. So the
    /// naval recon arm was dead on the seat it was built for — 25 live runs
    /// on 2026-08-18, zero sea-scout reservations, zero hulls in a 169-turn
    /// game with Cartography — while its tests, all on native boards, stayed
    /// green. Unseen water on a mirror is the fog at the edge of charted
    /// water: an `unknown` tile the mirror's frontier growth has marked
    /// `assumed_navigable`. Native boards never set that flag, so their
    /// answer is exactly the old one. See `mirror::grow_frontier`.
    pub(crate) fn unseen_water_remains(g: &Game, pid: usize) -> bool {
        g.map.tiles.iter().any(|(pos, tile)| {
            (g.rules.is_unknown(tile) && tile.assumed_navigable)
                || (!g.players[pid].explored.contains(pos)
                    && g.rules.is_passable(tile)
                    && g.rules.is_water(tile))
        })
    }

    /// Whether this civilization's naval units can cross Ocean tiles. This
    /// mirrors the sea portion of `Game::traversal_class`: a Galley penned
    /// behind deep water before Cartography has not found an open route.
    pub(super) fn naval_recon_can_cross_ocean(g: &Game, pid: usize) -> bool {
        g.has_ability(pid, "mana")
            || g.tree_effect(pid, "ocean_navigation") > 0.0
            || (g.has_ability(pid, "knarr")
                && g.players[pid].techs.contains(&crate::name!("shipbuilding")))
    }

    /// The navigable, known-water component reached from `starts`. Unknown
    /// terrain is deliberately not treated as water here: it may be a useful
    /// frontier to probe, but cannot turn a visible two-hex lake into an open
    /// sea by assumption.
    ///
    /// ⚠ **This is the reference definition, and production does not call it.**
    /// Nothing ever needed the component itself — both callers immediately
    /// reduced it to one boolean — while building it walked every tile of the
    /// ocean. `naval_recon_can_chart_from` decides the same boolean without
    /// materializing the set; this stays as the thing that check is tested
    /// against, so "they agree" is an assertion rather than a claim.
    #[cfg(test)]
    pub(super) fn naval_recon_waterway<I>(g: &Game, pid: usize, starts: I) -> BTreeSet<Pos>
    where
        I: IntoIterator<Item = Pos>,
    {
        let can_cross_ocean = Self::naval_recon_can_cross_ocean(g, pid);
        let navigable_water = |pos: Pos| {
            g.map.get(pos).is_some_and(|tile| {
                g.rules.is_passable(tile)
                    && g.rules.is_water(tile)
                    && (tile.terrain != "ocean" || can_cross_ocean)
            })
        };
        let mut waterway = BTreeSet::new();
        let mut frontier = VecDeque::new();
        for pos in starts {
            if navigable_water(pos) && waterway.insert(pos) {
                frontier.push_back(pos);
            }
        }
        while let Some(pos) = frontier.pop_front() {
            for neighbor in g.nbrs(pos) {
                if navigable_water(neighbor) && waterway.insert(neighbor) {
                    frontier.push_back(neighbor);
                }
            }
        }
        waterway
    }

    /// `naval_recon_waterway_can_chart` over the component reached from
    /// `starts`, decided while the walk runs and stopped the moment the answer
    /// is fixed.
    ///
    /// ★★★★★ THE COMPONENT WAS NEVER THE QUESTION, AND BUILDING IT WAS 13% OF
    /// THE SIMULATOR. The predicate is `size >= 4` — monotone — and two
    /// existential quantifiers over the same set. All three are settled by a
    /// prefix of the walk, so a coastal city on an unexplored sea is answered
    /// from about four tiles instead of the ocean's several thousand. The
    /// worst case is unchanged: a fully-charted body with no frontier still
    /// has to be exhausted to say "no".
    ///
    /// ⚠ The early exit is exact, not a heuristic, and this is the argument.
    /// Every tile counted here is a tile the reference set contains, so a
    /// `true` from a prefix is a `true` from the whole; and a walk that runs
    /// to exhaustion has dequeued exactly the reference set, so a `false` is
    /// the reference's `false`. `naval_recon_early_exit_agrees_with_the_full_walk`
    /// asserts the agreement over generated maps rather than trusting this
    /// paragraph. `city_has_open_water` above already records the rule this
    /// restores: "`production_value` cannot afford a flood fill per candidate
    /// per city per turn" — which is exactly what this arm was doing.
    pub(super) fn naval_recon_can_chart_from<I>(g: &Game, pid: usize, starts: I) -> bool
    where
        I: IntoIterator<Item = Pos>,
    {
        let can_cross_ocean = Self::naval_recon_can_cross_ocean(g, pid);
        let navigable_water = |pos: Pos| {
            g.map.get(pos).is_some_and(|tile| {
                g.rules.is_passable(tile)
                    && g.rules.is_water(tile)
                    && (tile.terrain != "ocean" || can_cross_ocean)
            })
        };
        // ★★★★ THE VISITED SET WAS 8.3% OF THE MAIN THREAD, ALL BY ITSELF.
        // A `BTreeSet<Pos>` here paid a node allocation and a tree descent per
        // ocean tile, and the walk's worst case — a charted body with no
        // frontier, which is most of a settled game — has to exhaust the
        // component to say "no". `sample` on the standard screen shape put
        // `BTreeSet<Pos>::insert` at 9.02% of the main thread and **92% of
        // those samples were this line**, not the attack envelopes
        // `docs/SIMULATOR_PERFORMANCE.md` had attributed the row to.
        //
        // `TileGrid::index_of` is documented for exactly this: "callers that
        // keep their own per-tile table index it by this, so the table is
        // dense and in the same order as the map itself". The walk is
        // unchanged — `seen` is only ever tested and set, never iterated, so
        // the frontier order, the `tiles` count and the early exit are the
        // ones a set gave.
        let mut seen = vec![false; g.map.tiles.len()];
        let mut first_visit = |pos: Pos| match g.map.tiles.index_of(pos) {
            Some(index) if !seen[index] => {
                seen[index] = true;
                true
            }
            _ => false,
        };
        let mut frontier = VecDeque::new();
        for pos in starts {
            if navigable_water(pos) && first_visit(pos) {
                frontier.push_back(pos);
            }
        }
        let mut tiles = 0usize;
        let mut reaches_open_water = false;
        let mut charts_something = false;
        while let Some(pos) = frontier.pop_front() {
            tiles += 1;
            if !reaches_open_water {
                reaches_open_water = g
                    .map
                    .get(pos)
                    .is_some_and(|tile| matches!(tile.terrain.as_str(), "coast" | "ocean"));
            }
            if !charts_something {
                charts_something = !g.players[pid].explored.contains(&pos)
                    || g.nbrs(pos).into_iter().any(|neighbor| {
                        g.map
                            .get(neighbor)
                            .is_some_and(|tile| Self::is_sea_frontier(g, tile))
                    });
            }
            if tiles >= NAVAL_RECON_MIN_WATERWAY_TILES && reaches_open_water && charts_something {
                return true;
            }
            for neighbor in g.nbrs(pos) {
                if navigable_water(neighbor) && first_visit(neighbor) {
                    frontier.push_back(neighbor);
                }
            }
        }
        tiles >= NAVAL_RECON_MIN_WATERWAY_TILES && reaches_open_water && charts_something
    }

    /// A waterway pays for a dedicated scout only when it is genuinely roomy,
    /// reaches open water, and has either unseen water or a live-mirror
    /// frontier at its edge.
    ///
    /// ★★★★★ THE OPEN-WATER TEST IS THE ONE THAT WAS MISSING, AND IT COST MORE
    /// THAN THE SIZE TEST SAVED. `NAVAL_RECON_MIN_WATERWAY_TILES` is 4 — room
    /// for one full Galley move — so any lake of four tiles licensed a hull.
    /// A lake is a closed system: the scout charts all of it within a couple
    /// of turns, the frontier test then goes false, and the hull is stranded
    /// there for the rest of the game, drawing maintenance and counting
    /// against `desired_navy` so the fleet that could sail is never built.
    ///
    /// Measured over three 150-turn six-player games before the fix: majors
    /// built 53 naval hulls, **20 of which never moved once**, 12 of those
    /// sitting on `lake` terrain. Only 17.2% of major naval unit-turns
    /// involved any movement, and a Galley was idle on 54.3% of its own turns
    /// — the largest single block of wasted major unit-turns in `audit`.
    /// Tracing the enqueue showed 12 of 15 naval builds in one game came from
    /// a city whose only adjacent water was lake, all of them through this
    /// arm rather than through `production_value`, which is why the naval gate
    /// there never saw them.
    ///
    /// A size floor cannot express this: the question is not how much water
    /// there is, it is whether the water goes anywhere. Ocean tiles are
    /// already excluded from the flood fill until the civ can cross them, so
    /// a Galley's coastal waterway still qualifies on its `coast` tiles.
    #[cfg(test)]
    pub(super) fn naval_recon_waterway_can_chart(
        g: &Game,
        pid: usize,
        waterway: &BTreeSet<Pos>,
    ) -> bool {
        waterway.len() >= NAVAL_RECON_MIN_WATERWAY_TILES
            && waterway.iter().any(|pos| {
                g.map
                    .get(*pos)
                    .is_some_and(|tile| matches!(tile.terrain.as_str(), "coast" | "ocean"))
            })
            && waterway.iter().any(|pos| {
                !g.players[pid].explored.contains(pos)
                    || g.nbrs(*pos).into_iter().any(|neighbor| {
                        g.map
                            .get(neighbor)
                            .is_some_and(|tile| Self::is_sea_frontier(g, tile))
                    })
            })
    }

    /// An `unknown` tile a ship may plan toward: the mirror's frontier, in
    /// either of its priors. The land prior counts too, as it always has —
    /// the frontier makes no land/water claim — and the sea prior is the one
    /// a coast revealed to the horizon otherwise never gets.
    fn is_sea_frontier(g: &Game, tile: &crate::world::Tile) -> bool {
        g.rules.is_unknown(tile) && (g.rules.is_passable(tile) || tile.assumed_navigable)
    }

    /// Whether a city can launch a naval scout into water that can still
    /// reveal something useful. General coastal logic intentionally remains
    /// broad — a two-tile lake is coastal for yields and harbor rules — while
    /// reconnaissance needs an actual route.
    pub(super) fn city_has_naval_recon_launch(g: &Game, pid: usize, cid: u32) -> bool {
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        Self::naval_recon_can_chart_from(g, pid, g.nbrs(city.pos))
    }

    /// A ship stranded in a small lake is not the empire's naval eye. Keep
    /// this separate from a ship's combat availability: a linked escort or a
    /// hull with no fresh-water route must not suppress a real explorer.
    pub(crate) fn naval_recon_ship_can_chart(g: &Game, pid: usize, uid: u32) -> bool {
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        let spec = &g.rules.units[unit.kind];
        if unit.owner != pid || spec.class != "military" || spec.domain.as_deref() != Some("sea") {
            return false;
        }
        Self::naval_recon_can_chart_from(g, pid, std::iter::once(unit.pos).chain(g.nbrs(unit.pos)))
    }

    pub(crate) fn empire_is_coastal(g: &Game, pid: usize) -> bool {
        g.player_city_ids(pid)
            .into_iter()
            .any(|cid| Self::city_is_coastal(g, cid))
    }

    /// Navigation is treated as a capability chain rather than an incidental
    /// unit unlock. Coastal empires first launch ships, then unlock general
    /// embarkation and harbors, and finally cross ocean once expansion has had
    /// time to reach the edge of its home landmass.
    /// Whether an empire has any use for the naval chain beyond Sailing:
    /// ships of its own, more than one coast to join up, room to settle
    /// overseas, or a war it may have to fight at sea.
    fn naval_ambitions(g: &Game, pid: usize) -> bool {
        let coastal_cities = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| Self::city_is_coastal(g, *cid))
            .count();
        if coastal_cities >= 2 {
            return true;
        }
        let owns_ships = g.units.values().any(|unit| {
            unit.owner == pid && g.rules.units[unit.kind].domain.as_deref() == Some("sea")
        });
        if owns_ships {
            return true;
        }
        // A civilization still expanding may need to cross water to do it; a
        // single-city minor never does.
        if !g.players[pid].is_minor && g.player_city_ids(pid).len() >= 2 {
            return true;
        }
        g.players.iter().any(|enemy| {
            enemy.id != pid
                && enemy.alive
                && !enemy.is_barbarian
                && g.is_at_war(pid, enemy.id)
                && (g
                    .player_city_ids(enemy.id)
                    .into_iter()
                    .any(|cid| Self::city_is_coastal(g, cid))
                    || g.units.values().any(|unit| {
                        unit.owner == enemy.id
                            && g.map
                                .get(unit.pos)
                                .is_some_and(|tile| g.rules.is_water(tile))
                    }))
        })
    }

    pub(crate) fn water_research_goal(g: &Game, pid: usize) -> Option<&'static str> {
        if !Self::empire_is_coastal(g, pid) {
            return None;
        }
        let player = &g.players[pid];
        if !player.techs.contains(&crate::name!("sailing")) {
            return Some("sailing");
        }
        // Past Sailing the naval chain gets expensive, and it overrides every
        // other research priority while it is being pursued. An empire with
        // nothing afloat and nowhere to sail spends those turns unable to
        // unlock the buildings that make its cities work - which left one-city
        // city-states grinding Shipbuilding for seventy turns on one
        // technology. Ask for a reason before committing to the rest.
        if !Self::naval_ambitions(g, pid) {
            return None;
        }
        if !player.techs.contains(&crate::name!("shipbuilding")) {
            return Some("shipbuilding");
        }
        if !player.techs.contains(&crate::name!("celestial_navigation"))
            && (g.turn >= 30 || g.player_city_ids(pid).len() >= 2)
        {
            return Some("celestial_navigation");
        }
        let has_ocean = g.map.tiles.values().any(|tile| tile.terrain == "ocean");
        let has_expansion_unit = g
            .units
            .values()
            .any(|unit| unit.owner == pid && unit.kind == "settler");
        if has_ocean
            && !player.techs.contains(&crate::name!("cartography"))
            && (g.turn >= 55 || g.player_city_ids(pid).len() >= 3 || has_expansion_unit)
        {
            return Some("cartography");
        }
        let naval_war = g.players.iter().any(|enemy| {
            enemy.id != pid
                && enemy.alive
                && g.is_at_war(pid, enemy.id)
                && (g.units.values().any(|unit| {
                    unit.owner == enemy.id
                        && g.map
                            .get(unit.pos)
                            .is_some_and(|tile| g.rules.is_water(tile))
                }) || g
                    .player_city_ids(enemy.id)
                    .into_iter()
                    .any(|cid| Self::city_is_coastal(g, cid)))
        });
        if naval_war && player.techs.contains(&crate::name!("cartography")) {
            if !player.techs.contains(&crate::name!("square_rigging")) {
                return Some("square_rigging");
            }
            // After the first dedicated naval-ranged unlock, pursue later
            // fleet upgrades only when their era's prerequisite is already in
            // hand. This keeps naval readiness current without dragging an
            // ancient empire through an entire industrial branch at once.
            for (goal, prerequisite) in [
                ("steam_power", "industrialization"),
                ("refining", "rifling"),
                ("electricity", "steam_power"),
                ("combined_arms", "combustion"),
                ("lasers", "nuclear_fission"),
                ("telecommunications", "computers"),
            ] {
                if player.techs.contains(&Name::new(prerequisite))
                    && !player.techs.contains(&Name::new(goal))
                {
                    return Some(goal);
                }
            }
        }
        None
    }

    pub(crate) fn waterborne(g: &Game, uid: u32) -> bool {
        let unit = &g.units[&uid];
        g.rules.units[unit.kind].domain.as_deref() == Some("sea")
            || g.map
                .get(unit.pos)
                .is_some_and(|tile| g.rules.is_water(tile))
    }

    pub(crate) fn naval_counts(g: &Game, pid: usize) -> (usize, usize, usize, usize, usize) {
        let mut counts = (0, 0, 0, 0, 0);
        let mut add = |kind: &str| {
            let spec = &g.rules.units[kind];
            if spec.class != "military" || spec.domain.as_deref() != Some("sea") {
                return;
            }
            counts.0 += 1;
            match spec.promotion_class.as_str() {
                "naval_melee" => counts.1 += 1,
                "naval_ranged" => counts.2 += 1,
                "naval_raider" => counts.3 += 1,
                "naval_carrier" => counts.4 += 1,
                _ => {}
            }
        };
        for uid in g.player_unit_ids(pid) {
            add(&g.units[&uid].kind);
        }
        for cid in g.player_city_ids(pid) {
            if let Some(Item::Unit { unit }) = g.cities[&cid].queue.first() {
                add(unit);
            }
        }
        counts
    }
}
