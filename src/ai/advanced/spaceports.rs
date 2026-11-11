//! The Science lane's launch sites: how many Spaceports it wants, which cities
//! carry them, and what those cities build.
//!
//! How many: `adaptive_science_spaceport_lane` and `science_spaceport_target`.
//! Which cities: a pad counts from the moment it is committed, whether it is
//! standing, placed or queued (`city_has_spaceport_commitment`,
//! `science_spaceport_commitments`), and the highest-production cities carry the
//! open slots (`science_spaceport_candidates`,
//! `science_spaceport_retained_cities`, `science_spaceport_city_is_admitted`).
//! What they build: `space_race_production` runs the pass, in which
//! `rebalance_science_spaceport_queues` replaces surplus or slower pads and
//! `repair_stalled_science_project_queues` reclaims a queue whose pad was
//! pillaged; `science_production` and `science_spaceport_production` choose the
//! items.
//!
//! The genes that adjust this live beside it, such as
//! `launch_pad_holds_its_queue.rs` and `science_victory_drive.rs`.
//!
//! Moved verbatim out of the `impl AdvancedAi` block in `advanced.rs`
//! (2026-11-06). Methods the rest of the controller calls are `pub(super)`; the
//! code is otherwise unchanged.

use super::*;

impl AdvancedAi {
    /// The number of Spaceports the current Science plan is allowed to keep
    /// building. The milestones deliberately stage the investment; the hard
    /// cap is a guard for the generic production governor as well as the
    /// dedicated space-race pass.
    ///
    /// An adaptive controller has no explicit `victory_target`, so its
    /// ordinary Science commitment is the launch chain itself. Before that
    /// chain starts, the generic Spaceport valuation retains its historical
    /// behavior.
    pub(super) fn adaptive_science_spaceport_lane(&self, g: &Game, pid: usize) -> bool {
        if !self.victory_planning || self.victory_target.is_some() || !g.victory_conditions.science
        {
            return false;
        }
        [
            "launch_earth_satellite",
            "launch_moon_landing",
            "launch_mars_colony",
            "exoplanet_expedition",
        ]
        .into_iter()
        .any(|project| {
            g.players[pid].science_projects.contains(project)
                || Self::science_project_is_queued(g, pid, project)
        })
    }

    pub(super) fn science_spaceport_target(&self, g: &Game, pid: usize) -> usize {
        let city_count = g.player_city_ids(pid).len();
        if city_count == 0 {
            return 0;
        }
        let completed = &g.players[pid].science_projects;
        let earth_satellite_started = !completed.contains("launch_earth_satellite")
            && Self::science_project_is_queued(g, pid, "launch_earth_satellite");
        let adaptive_science = self.adaptive_science_spaceport_lane(g, pid);
        let desired = if self.science_drive_active()
            || (self.victory_planning
                && (self.space_race_lane(g, pid)
                    || self.raced_target() == Some(VictoryTarget::Science)))
            || adaptive_science
        {
            // The drive starts its second pad as soon as the Earth Satellite
            // is underway, so it is ready for the later parallel laser phase.
            Self::science_drive_desired_pads(completed).max(
                if self.victory_planning
                    && Self::science_project_is_queued(g, pid, "launch_mars_colony")
                {
                    3
                } else if earth_satellite_started {
                    2
                } else {
                    1
                },
            )
        } else if self.space_race_lane(g, pid)
            || self.raced_target() == Some(VictoryTarget::Science)
        {
            if completed.contains("launch_mars_colony") {
                3
            } else if completed.contains("launch_moon_landing") {
                2
            } else {
                1
            }
        } else {
            1
        };
        desired.min(SCIENCE_SPACEPORT_CAP).min(city_count)
    }

    /// A standing Spaceport keeps a city busy with one project at a time;
    /// another one there cannot create useful parallelism.
    pub(super) fn city_has_spaceport(g: &Game, cid: u32) -> bool {
        let pad = crate::name!("spaceport");
        g.cities[&cid]
            .districts
            .keys()
            .any(|district| g.district_family(*district) == pad)
    }

    /// A placed district is a commitment before it is a completed district.
    /// Civilization VI removes the district from the production queue as soon
    /// as the player chooses its plot, while the host state reports the plot
    /// as `complete: false`. The mirror preserves that fact on the tile, not
    /// in `City::districts` or `City::queue`.
    pub(super) fn city_has_spaceport_foundation(g: &Game, cid: u32) -> bool {
        let pad = crate::name!("spaceport");
        g.cities[&cid].owned_tiles.iter().any(|position| {
            g.map
                .get(*position)
                .and_then(|tile| tile.district_foundation.as_ref())
                .is_some_and(|foundation| g.district_family(foundation.district) == pad)
        })
    }

    /// A queued Spaceport is a real future launch site even when another item
    /// precedes it in the city queue.
    fn city_queues_spaceport(g: &Game, cid: u32) -> bool {
        let pad = crate::name!("spaceport");
        g.cities[&cid].queue.iter().any(|item| {
            matches!(item, Item::District { district, .. }
                if g.district_family(*district) == pad)
        })
    }

    /// An uncompleted launch project is a real race commitment as soon as it
    /// is in a city queue. It can sit behind another item while the first pad
    /// is still finishing that item, but the next Spaceport can already be
    /// built in parallel in another city.
    pub(super) fn science_project_is_queued(g: &Game, pid: usize, project: &str) -> bool {
        g.player_city_ids(pid).into_iter().any(|cid| {
            g.cities[&cid]
                .queue
                .iter()
                .any(|item| matches!(item, Item::Project { project: queued } if queued == project))
        })
    }

    /// A city can expose the same in-progress pad through both the queue and
    /// its foundation on a native board. Count the city once, so changing the
    /// queue item does not turn one placed pad into two commitments.
    pub(super) fn city_has_spaceport_commitment(g: &Game, cid: u32) -> bool {
        Self::city_has_spaceport(g, cid)
            || Self::city_queues_spaceport(g, cid)
            || Self::city_has_spaceport_foundation(g, cid)
    }

    /// Count every standing, placed, and queued Spaceport. Queue depth matters:
    /// a pad behind a project still consumes one of the race's deliberately
    /// small launch-site budget, and a placed foundation remains committed
    /// after Civilization VI removes it from the queue.
    pub(super) fn science_spaceport_commitments(g: &Game, pid: usize) -> usize {
        g.player_city_ids(pid)
            .into_iter()
            .filter(|cid| Self::city_has_spaceport_commitment(g, *cid))
            .count()
    }

    /// Cities that can carry the still-unfilled Spaceport slots, in production
    /// order. A queued or placed pad counts as a candidate so an already-good
    /// commitment is retained; a completed pad is permanent and therefore
    /// outside this list.
    fn science_spaceport_candidates(&self, g: &Game, pid: usize) -> Vec<u32> {
        let pad = crate::name!("spaceport");
        let _memo = g.query_memo();
        let mut candidates: Vec<u32> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                !Self::city_has_spaceport(g, *cid)
                    && (Self::city_has_spaceport_commitment(g, *cid)
                        || g.district_sites(*cid, pad).into_iter().any(|pos| {
                            g.can_produce(pid, *cid, &Item::District { district: pad, pos })
                        }))
            })
            .collect();
        candidates.sort_by(|left, right| {
            g.city_yields(*right)
                .production
                .total_cmp(&g.city_yields(*left).production)
                .then_with(|| left.cmp(right))
        });
        candidates
    }

    /// The unfinished launch-site cities the plan should retain. Completed
    /// pads cannot be moved, so a legacy run that already has three simply
    /// stops every additional queue rather than trying to undo them.
    pub(super) fn science_spaceport_retained_cities(&self, g: &Game, pid: usize) -> BTreeSet<u32> {
        let completed = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| Self::city_has_spaceport(g, *cid))
            .count();
        let target = self
            .science_spaceport_target(g, pid)
            .max(completed)
            .min(SCIENCE_SPACEPORT_CAP);
        let candidates = self.science_spaceport_candidates(g, pid);
        let committed: Vec<u32> = candidates
            .iter()
            .copied()
            .filter(|cid| Self::city_has_spaceport_commitment(g, *cid))
            .collect();
        // Preserve cities that have already paid the irreversible placement
        // cost first. Only genuinely uncommitted cities may fill the slots
        // left after those foundations and queued pads are accounted for.
        let mut retained: BTreeSet<u32> = committed
            .iter()
            .copied()
            .take(target.saturating_sub(completed))
            .collect();
        let open = target.saturating_sub(completed + committed.len());
        retained.extend(
            candidates
                .into_iter()
                .filter(|cid| !Self::city_has_spaceport_commitment(g, *cid))
                .take(open),
        );
        retained
    }

    /// Whether this city is one of the highest-production cities still needed
    /// for a Science Spaceport. This is consulted by the generic production
    /// scorer, which otherwise sees each city in isolation and can start a
    /// fourth (or seventh) pad after the dedicated pass has done its work.
    pub(super) fn science_spaceport_city_is_admitted(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
    ) -> bool {
        self.science_spaceport_retained_cities(g, pid)
            .contains(&cid)
    }

    /// Replace surplus or slower in-progress Spaceports with the best useful
    /// local work. `Action::Produce` banks the paused district progress, so a
    /// future genuine need can resume it; meanwhile the city can finish its
    /// research/production infrastructure or, once that is exhausted, run a
    /// deliberately valued district project.
    pub(super) fn rebalance_science_spaceport_queues(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        let pad = crate::name!("spaceport");
        let retained = self.science_spaceport_retained_cities(g, pid);
        let completed = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| Self::city_has_spaceport(g, *cid))
            .count();
        let target = self
            .science_spaceport_target(g, pid)
            .max(completed)
            .min(SCIENCE_SPACEPORT_CAP);
        let excess: Vec<u32> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                !retained.contains(cid)
                    && matches!(
                        g.cities[cid].queue.first(),
                        Some(Item::District { district, .. })
                            if g.district_family(*district) == pad
                    )
            })
            .collect();
        if excess.is_empty() {
            return;
        }

        let mut counts = self.counts(g, pid);
        for cid in excess {
            let replacement = {
                let _memo = g.query_memo();
                let items: Vec<Item> = g
                    .producible_items(pid, cid)
                    .into_iter()
                    .filter(|item| {
                        !matches!(item, Item::District { district, .. }
                            if g.district_family(*district) == pad)
                    })
                    .collect();
                let normally_valued = items
                    .iter()
                    .cloned()
                    .map(|item| {
                        let value = self.card_boosted_value(
                            g,
                            pid,
                            cid,
                            &item,
                            self.production_value(g, pid, cid, &item, plan, &counts),
                        );
                        (value, format!("{item:?}"), item)
                    })
                    .filter(|(value, _, _)| *value > -1_000.0)
                    .max_by(|left, right| {
                        left.0
                            .total_cmp(&right.0)
                            .then_with(|| right.1.cmp(&left.1))
                    });
                // Every ordinary production score can correctly go negative
                // late in a game. That must not leave
                // an already-illegal fourth Spaceport running, though. In
                // that exceptional case reclaim the queue for a concrete
                // repair/building, a Builder, or finally a Science/Industrial
                // project. The ordinary scorer above remains the route used
                // whenever it has a viable local choice.
                normally_valued.or_else(|| {
                    items
                        .into_iter()
                        .filter_map(|item| {
                            let fallback_rank = match &item {
                                Item::Repair { .. } => 4,
                                Item::Building { .. } => 3,
                                Item::Unit { unit } if *unit == "builder" => 2,
                                Item::Project { project }
                                    if g.rules.projects.get(project).is_some_and(|spec| {
                                        spec.district.is_some_and(|district| {
                                            matches!(
                                                g.district_family(district).as_str(),
                                                "campus" | "industrial_zone"
                                            )
                                        })
                                    }) =>
                                {
                                    1
                                }
                                _ => return None,
                            };
                            Some((fallback_rank, format!("{item:?}"), item))
                        })
                        .max_by(|left, right| {
                            left.0.cmp(&right.0).then_with(|| right.1.cmp(&left.1))
                        })
                        .map(|(_, description, item)| (0.0, description, item))
                })
            };
            let Some((score, _, item)) = replacement else {
                continue;
            };
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: item.clone(),
                },
            )
            .is_err()
            {
                continue;
            }
            if self.journal().wants(crate::reasoning::Level::Decision) {
                let city_name = g.cities[&cid].name.clone();
                think!(self.journal(), Cities, Decision,
                       "{} replaces an excess Spaceport", city_name;
                       "the Science plan keeps at most {target} launch sites; {} is worth {score:.0}",
                       Self::plain_item(&item));
            }
            counts.add_item(g, &item);
        }
    }

    /// The science lane's production pass: the Spaceport queues rebalanced,
    /// then `science_production`.
    pub(super) fn space_race_production(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if self.victory_target == Some(VictoryTarget::Domination) {
            return;
        }
        self.rebalance_science_spaceport_queues(g, pid, plan);
        self.science_production(g, pid);
        self.repair_stalled_science_project_queues(g, pid);
    }

    /// Reclaim a committed science queue whose Spaceport has been pillaged.
    ///
    /// `Game::process_city` correctly stalls a launch project while every
    /// Spaceport in that city is pillaged, but the strategic governor normally
    /// treats a non-empty queue as a commitment and never asks the repair menu
    /// again. That makes a spy or raid turn a recoverable infrastructure loss
    /// into an indefinitely frozen race: `Item::Repair { repair: "district" }`
    /// is legal, but the queue head hides it. Switching through `Produce` banks
    /// the project's progress by its normal item key, repairs the launch site,
    /// and lets the next science pass resume the same rung.
    pub(super) fn repair_stalled_science_project_queues(&self, g: &mut Game, pid: usize) {
        if self.raced_target() != Some(VictoryTarget::Science)
            && !self.science_endgame_committed(g, pid)
        {
            return;
        }
        let is_science_project = |project: &str| {
            matches!(
                project,
                "launch_earth_satellite"
                    | "launch_moon_landing"
                    | "launch_mars_colony"
                    | "exoplanet_expedition"
                    | "lagrange_laser_station"
                    | "terrestrial_laser_station"
            )
        };
        for cid in g.player_city_ids(pid) {
            let Some(Item::Project { project }) = g.cities[&cid].queue.first() else {
                continue;
            };
            if !is_science_project(project) {
                continue;
            }
            let spaceports: Vec<crate::Pos> = g.cities[&cid]
                .districts
                .iter()
                .filter_map(|(district, position)| {
                    (g.district_family(*district) == "spaceport").then_some(*position)
                })
                .collect();
            if spaceports.is_empty()
                || spaceports
                    .iter()
                    .any(|position| !g.map.tiles[position].pillaged)
            {
                continue;
            }
            let Some(repair) = spaceports.into_iter().find_map(|position| {
                let repair = Item::Repair {
                    repair: crate::name!("district"),
                    pos: position,
                };
                g.can_produce(pid, cid, &repair).then_some(repair)
            }) else {
                continue;
            };
            let project_name = project.to_string();
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: repair.clone(),
                },
            )
            .is_ok()
                && self.journal().wants(crate::reasoning::Level::Decision)
            {
                think!(self.journal(), Cities, Decision,
                    "{} repairs its pillaged Spaceport before resuming {}",
                    g.cities[&cid].name, project_name;
                    "the committed launch queue was stalled because every Spaceport district was inactive; its progress remains banked by item");
            }
        }
    }

    pub(super) fn science_production(&self, g: &mut Game, pid: usize) {
        if self.victory_target == Some(VictoryTarget::Domination) {
            return;
        }
        if self.schedule_science_endgame(g, pid) {
            self.science_spaceport_production(g, pid);
            return;
        }
        // See `lane_space_race`: every gate below asks for an EXPLICITLY
        // assigned Science target, so the adaptive agent production ships —
        // which has no target at all — races the space race at one pad, and
        // only in a city with nothing else queued. One answer per turn; the
        // three tests below read it.
        let races_science = self.space_race_lane(g, pid);
        let completed = g.players[pid].science_projects.clone();
        let project = if !completed.contains("launch_earth_satellite") {
            "launch_earth_satellite"
        } else if !completed.contains("launch_moon_landing") {
            "launch_moon_landing"
        } else if !completed.contains("launch_mars_colony") {
            "launch_mars_colony"
        } else if !completed.contains("exoplanet_expedition") {
            "exoplanet_expedition"
        } else {
            "lagrange_laser_station"
        };
        let project_item = Item::Project {
            project: Name::new(project),
        };
        let parallel_project = matches!(
            project,
            "lagrange_laser_station" | "terrestrial_laser_station"
        );
        let already_queued = !parallel_project
            && g.player_city_ids(pid).iter().any(|cid| {
                matches!(
                    g.cities[cid].queue.first(),
                    Some(Item::Project { project: queued }) if queued == project
                )
            });
        if !already_queued {
            let project_city = {
                let _memo = g.query_memo();
                g.player_city_ids(pid)
                    .into_iter()
                    .filter(|cid| {
                        g.cities[cid]
                            .districts
                            .contains_key(crate::name!("spaceport"))
                            && g.can_produce(pid, *cid, &project_item)
                            && !matches!(
                                g.cities[cid].queue.first(),
                                Some(Item::Project { project: queued }) if queued == project
                            )
                            && (races_science
                                || self.raced_target() == Some(VictoryTarget::Science)
                                || g.cities[cid].queue.is_empty())
                    })
                    .min_by(|a, b| {
                        if races_science || self.raced_target() == Some(VictoryTarget::Science) {
                            Self::science_project_build_turns(g, pid, *a, &project_item)
                                .total_cmp(&Self::science_project_build_turns(
                                    g,
                                    pid,
                                    *b,
                                    &project_item,
                                ))
                                .then_with(|| a.cmp(b))
                        } else {
                            g.city_yields(*b)
                                .production
                                .total_cmp(&g.city_yields(*a).production)
                                .then_with(|| a.cmp(b))
                        }
                    })
            };
            if let Some(city) = project_city {
                let _ = g.apply(
                    pid,
                    &Action::Produce {
                        city,
                        item: project_item,
                    },
                );
                return;
            }
        }

        self.science_spaceport_production(g, pid);
    }

    fn science_spaceport_production(&self, g: &mut Game, pid: usize) {
        // Choosing the pad is a read: one memo for the whole sweep, dropped
        // before the claim below writes. See `Game::query_memo`.
        let memo = g.query_memo();
        let races_science = self.space_race_lane(g, pid);
        let city_ids = g.player_city_ids(pid);
        if Self::science_spaceport_commitments(g, pid) >= self.science_spaceport_target(g, pid) {
            return;
        }
        let mut best: Option<(f64, u32, Pos)> = None;
        for cid in city_ids {
            if Self::city_has_spaceport_commitment(g, cid) {
                continue;
            }
            if !self.science_spaceport_city_is_admitted(g, pid, cid) {
                continue;
            }
            if !races_science
                && self.raced_target() != Some(VictoryTarget::Science)
                && !g.cities[&cid].queue.is_empty()
            {
                continue;
            }
            for item in g.producible_items(pid, cid) {
                let Item::District { district, pos } = item else {
                    continue;
                };
                if g.district_family(district) != "spaceport" {
                    continue;
                }
                let production = g.city_yields(cid).production;
                if best
                    .map(|old| {
                        production > old.0 || (production == old.0 && (cid, pos) < (old.1, old.2))
                    })
                    .unwrap_or(true)
                {
                    best = Some((production, cid, pos));
                }
            }
        }
        drop(memo);
        if let Some((_, city, pos)) = best {
            let _ = g.apply(
                pid,
                &Action::Produce {
                    city,
                    item: Item::District {
                        district: crate::name!("spaceport"),
                        pos,
                    },
                },
            );
        }
    }
}
