//! Boosts: eurekas and inspirations, and the tree progress they buy.
//!
//! Whether a boost's trigger is met (`boost_met`) and how far along it is
//! (`boost_progress`); the turn's `check_boosts`; sharing a technology
//! boost with a team; the fraction a boost is worth for a node
//! (`node_boost_frac`, `boost_frac`); granting random boosts and completing
//! random nodes; and finishing a tree node (`apply_tree_completion`).
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    /// Eureka/inspiration fraction (China's Dynastic Cycle: 50% vs 40%).
    /// Boost grant for a named node, for callers that only know the node
    /// (steals, goody huts, and applying a stored boost on selection).
    pub(super) fn node_boost_frac(&self, pid: usize, name: &str, techs: bool) -> f64 {
        let tree = if techs {
            &self.rules.techs
        } else {
            &self.rules.civics
        };
        match tree.get(name).and_then(|spec| spec.boost.as_ref()) {
            Some(boost) => self.boost_frac(pid, boost),
            None => {
                let mut percent = 40.0;
                if self.has_ability(pid, "dynastic_cycle") {
                    percent += 10.0;
                }
                percent / 100.0
            }
        }
    }

    pub(super) fn boost_frac(&self, pid: usize, b: &crate::rules::BoostSpec) -> f64 {
        // China's Dynastic Cycle adds ten percentage points to the shipped
        // per-boost grant (40 everywhere except Near Future Governance's 90).
        let mut percent = b.percent.unwrap_or(40.0);
        if self.has_ability(pid, "dynastic_cycle") {
            percent += 10.0;
        }
        percent / 100.0
    }

    /// Eureka (techs) or Inspiration (civics) boosts on `n` random
    /// not-yet-boosted entries.
    pub(super) fn grant_random_boosts(&mut self, pid: usize, n: usize, techs: bool) {
        let game_speed = self.game_speed;
        for _ in 0..n {
            let cands: Vec<(Name, f64)> = if techs {
                self.rules
                    .techs
                    .iter()
                    .filter(|(name, _)| {
                        let p = &self.players[pid];
                        !p.techs.contains(*name) && !p.boosted_techs.contains(*name)
                    })
                    .map(|(name, s)| (*name, game_speed.scale(s.cost)))
                    .collect()
            } else {
                self.rules
                    .civics
                    .iter()
                    .filter(|(name, _)| {
                        let p = &self.players[pid];
                        !p.civics.contains(*name) && !p.boosted_civics.contains(*name)
                    })
                    .map(|(name, s)| (*name, game_speed.scale(s.cost)))
                    .collect()
            };
            if cands.is_empty() {
                return;
            }
            let (name, cost) = cands[self.rng.below(cands.len())];
            let f = self.node_boost_frac(pid, &name, techs);
            let p = &mut self.players[pid];
            if techs {
                p.boosted_techs.insert(Name::new(&name));
                if p.research.as_deref() == Some(name.as_str()) {
                    p.research_progress += f * cost;
                }
            } else {
                p.boosted_civics.insert(Name::new(&name));
                if p.civic.as_deref() == Some(name.as_str()) {
                    p.civic_progress += f * cost;
                }
            }
        }
    }

    /// Civ VI teams do not pool Science. Completing a technology instead
    /// grants its normal Eureka to every teammate who neither owns nor has
    /// already boosted it; an actively researched node receives the progress
    /// immediately.
    pub(super) fn share_team_technology_boost(&mut self, source: usize, technology: &str) {
        let recipients: Vec<usize> = self
            .team_members(source)
            .into_iter()
            .filter(|recipient| {
                *recipient != source
                    && self.players[*recipient].alive
                    && !self.players[*recipient]
                        .techs
                        .contains(&Name::new(technology))
                    && !self.players[*recipient]
                        .boosted_techs
                        .contains(&Name::new(technology))
            })
            .collect();
        let cost = self.tech_cost(technology);
        for recipient in recipients {
            let fraction = self.node_boost_frac(recipient, technology, true);
            let player = &mut self.players[recipient];
            player.boosted_techs.insert(Name::new(technology));
            if player.research.as_deref() == Some(technology) {
                player.research_progress += fraction * cost;
            }
            self.note(
                recipient,
                "Science",
                format!(
                    "received the {} Eureka from teammate {}",
                    pretty(technology),
                    self.civ_name(source)
                ),
                None,
            );
        }
    }

    pub(super) fn apply_tree_completion(
        &mut self,
        pid: usize,
        technology: bool,
        node: &str,
        first: bool,
    ) {
        let effects = if technology {
            self.rules.techs[node].effects.clone()
        } else {
            self.rules.civics[node].effects.clone()
        };
        *self.players[pid]
            .counters
            .entry(format!("tree_completions:{node}"))
            .or_insert(0) += 1;
        if !technology && first && node == "early_empire" {
            self.reconcile_closed_border_units(None);
        }
        if first {
            self.note_era_first_tree_node(pid, technology, node);
        }
        if first {
            self.players[pid].envoys_free +=
                effects.get("free_envoys").copied().unwrap_or(0.0) as i64;
            *self.players[pid]
                .counters
                .entry("district_governor_titles".to_string())
                .or_insert(0) += effects.get("governor_titles").copied().unwrap_or(0.0) as i64;
            self.players[pid].dvp += effects
                .get("diplomatic_victory_points")
                .copied()
                .unwrap_or(0.0) as i64;
        }
        *self.players[pid]
            .counters
            .entry("district_governor_titles".to_string())
            .or_insert(0) += effects
            .get("governor_titles_per_completion")
            .copied()
            .unwrap_or(0.0) as i64;
        let favor = effects
            .get("diplomatic_favor_per_completion")
            .copied()
            .unwrap_or(0.0);
        *self.players[pid]
            .counters
            .entry("diplomatic_favor".to_string())
            .or_insert(0) += favor as i64;
        self.players[pid].diplomatic_favor += favor;
        if self.players[pid].dvp >= DIPLOMATIC_VICTORY_POINTS {
            self.set_winner(pid, "diplomatic");
        }
    }

    pub(super) fn check_boosts(&mut self, pid: usize) {
        if self.players[pid].is_minor {
            return;
        }
        let game_speed = self.game_speed;
        let techs: Vec<(Name, f64, crate::rules::BoostSpec)> = self
            .rules
            .techs
            .iter()
            .filter_map(|(n, s)| s.boost.clone().map(|b| (*n, game_speed.scale(s.cost), b)))
            .collect();
        for (name, cost, b) in techs {
            let p = &self.players[pid];
            if p.techs.contains(&Name::new(&name)) || p.boosted_techs.contains(&Name::new(&name)) {
                continue;
            }
            if self.boost_met(pid, &b) {
                let f = self.boost_frac(pid, &b);
                let p = &mut self.players[pid];
                p.boosted_techs.insert(Name::new(&name));
                if p.research.as_deref() == Some(name.as_str()) {
                    p.research_progress += f * cost;
                }
                self.dedication_trigger(pid, "eureka", 1);
            }
        }
        let civics: Vec<(Name, f64, crate::rules::BoostSpec)> = self
            .rules
            .civics
            .iter()
            .filter_map(|(n, s)| s.boost.clone().map(|b| (*n, game_speed.scale(s.cost), b)))
            .collect();
        for (name, cost, b) in civics {
            let p = &self.players[pid];
            if p.civics.contains(&Name::new(&name)) || p.boosted_civics.contains(&Name::new(&name))
            {
                continue;
            }
            if self.boost_met(pid, &b) {
                let f = self.boost_frac(pid, &b);
                let p = &mut self.players[pid];
                p.boosted_civics.insert(Name::new(&name));
                if p.civic.as_deref() == Some(name.as_str()) {
                    p.civic_progress += f * cost;
                }
                self.dedication_trigger(pid, "inspiration", 1);
            }
        }
    }

    pub(crate) fn boost_met(&self, pid: usize, b: &crate::rules::BoostSpec) -> bool {
        let (have, need) = self.boost_progress(pid, b);
        have >= need
    }

    /// How far `pid` stands toward a boost trigger, as `(have, need)`: the
    /// trigger is met exactly when `have >= need`, and `boost_met` is that
    /// one comparison. Every arm of the old boolean maps to a pair — a count
    /// against the row's `count`, an "any city / any wonder" test against
    /// the best single value, a yes/no test as `(0 or 1, 1)` — so the engine
    /// keeps a single definition of "met" and a planner chasing a boost can
    /// read the same arm for how much is left. `improve_resource:` and
    /// `airbase_foreign_continent` need one tile whatever the row's count
    /// says, exactly as before. An unknown trigger is `(0, 1)`: never met.
    ///
    /// Live, `player.counters` is never populated by the mirror, so every
    /// counter-backed arm (`kills`, `barbs_killed`, `kill_with:*`, `camps`,
    /// `captures`, `improvements`, `trained:*`) reads `have = 0` on the host
    /// seat. That is the truthful answer for a chase table: the host itself
    /// says when such a boost fires (`boosted_techs` / `boosted_civics`
    /// arrive already-fired), and until then the chase stays open.
    pub(crate) fn boost_progress(&self, pid: usize, b: &crate::rules::BoostSpec) -> (i64, i64) {
        let p = &self.players[pid];
        let n = b.count;
        let cities: Vec<&City> = self.cities.values().filter(|c| c.owner == pid).collect();
        let counter = |key: &str| p.counters.get(key).copied().unwrap_or(0);
        let flag = |met: bool| (i64::from(met), 1);
        let trig = b.trigger.as_str();
        match trig {
            "kills" | "improvements" | "camps" | "captures" | "barbs_killed" | "received_dow"
            | "casus_belli" | "national_park" | "great_people" => (counter(trig), n),
            "artifacts" => (counter("great_work:artifact"), n),
            "cities" => (cities.len() as i64, n),
            "districts" => (
                cities.iter().map(|c| c.districts.len() as i64).sum::<i64>(),
                n,
            ),
            "specialty_districts" => (
                cities
                    .iter()
                    .flat_map(|c| c.districts.keys())
                    .map(|d| self.district_family(*d))
                    .filter(|family| {
                        self.rules
                            .districts
                            .get(family.as_str())
                            .is_some_and(|spec| spec.specialty)
                    })
                    .collect::<BTreeSet<Name>>()
                    .len() as i64,
                n,
            ),
            "pop" => (cities.iter().map(|c| c.pop as i64).max().unwrap_or(0), n),
            "total_pop" => (cities.iter().map(|c| c.pop as i64).sum::<i64>(), n),
            "units" => (
                self.units
                    .values()
                    .filter(|u| u.owner == pid && self.rules.units[u.kind].class == "military")
                    .count() as i64,
                n,
            ),
            "land_units" => (
                self.units
                    .values()
                    .filter(|u| {
                        let spec = &self.rules.units[u.kind];
                        u.owner == pid
                            && spec.class == "military"
                            && !matches!(spec.domain.as_deref(), Some("sea") | Some("air"))
                    })
                    .count() as i64,
                n,
            ),
            "corps" => (
                self.units
                    .values()
                    .filter(|u| u.owner == pid && u.formation >= 1)
                    .count() as i64,
                n,
            ),
            "armies" => (
                self.units
                    .values()
                    .filter(|u| u.owner == pid && u.formation >= 2)
                    .count() as i64,
                n,
            ),
            "coastal_city" => flag(cities.iter().any(|c| {
                self.nbrs(c.pos).iter().any(|nb| {
                    self.map
                        .get(*nb)
                        .map(|t| self.rules.is_water(t))
                        .unwrap_or(false)
                })
            })),
            "war" => flag(
                self.players
                    .iter()
                    .any(|o| o.id != pid && !o.is_barbarian && self.is_at_war(pid, o.id)),
            ),
            "alliances" => (
                p.alliances
                    .values()
                    .filter(|alliance| alliance.ends > self.turn)
                    .count() as i64,
                n,
            ),
            "alliance_level" => (
                p.alliances
                    .values()
                    .filter(|alliance| alliance.ends > self.turn)
                    .map(|alliance| alliance.level as i64)
                    .max()
                    .unwrap_or(i64::MIN),
                n,
            ),
            "pantheon" => flag(p.pantheon.is_some()),
            "religion" => flag(p.religion.is_some()),
            "religion_cities" => (
                p.religion
                    .as_deref()
                    .map(|religion| {
                        self.cities
                            .values()
                            .filter(|c| self.city_religion(c) == Some(religion))
                            .count() as i64
                    })
                    .unwrap_or(0),
                n,
            ),
            "trade_routes" => (
                self.routes
                    .iter()
                    .filter(|route| route.owner == pid && route.ends > self.turn)
                    .count() as i64,
                n,
            ),
            "wonders" => (
                cities.iter().map(|c| c.wonders.len() as i64).sum::<i64>(),
                n,
            ),
            "wonder_era" => (
                cities
                    .iter()
                    .flat_map(|c| c.wonders.keys())
                    .map(|wonder| self.wonder_era(wonder) as i64)
                    .max()
                    .unwrap_or(i64::MIN),
                n,
            ),
            "government_slots" => {
                let slots = self.gov_slots(pid);
                (
                    slots.military + slots.economic + slots.diplomatic + slots.wildcard,
                    n,
                )
            }
            "natural_wonder" => flag(!p.discovered_natural_wonders.is_empty()),
            "met_civ" => flag(self.players.iter().any(|o| {
                o.id != pid
                    && o.alive
                    && !o.is_barbarian
                    && !o.is_minor
                    && self
                        .cities
                        .values()
                        .any(|c| c.owner == o.id && p.explored.contains(&c.pos))
            })),
            "met_city_states" => (
                self.players
                    .iter()
                    .filter(|o| {
                        o.is_minor
                            && o.alive
                            && self
                                .cities
                                .values()
                                .any(|c| c.owner == o.id && p.explored.contains(&c.pos))
                    })
                    .count() as i64,
                n,
            ),
            "discover_continent" => {
                let home = self.home_continent(pid);
                flag(
                    home.is_some()
                        && p.explored.iter().any(|pos| {
                            self.map.get(*pos).is_some_and(|tile| {
                                tile.continent.is_some() && tile.continent != home
                            })
                        }),
                )
            }
            "airbase_foreign_continent" => {
                let home = self.home_continent(pid);
                let have = if home.is_some() {
                    self.owned_tiles_where(pid, |tile| {
                        tile.continent.is_some()
                            && tile.continent != home
                            && (tile.improvement.as_deref() == Some("airstrip")
                                || tile.district.is_some_and(|d| d.starts_with("aerodrome")))
                    })
                } else {
                    0
                };
                (have, 1)
            }
            _ => {
                if let Some(t) = trig.strip_prefix("units_of:") {
                    (
                        self.units
                            .values()
                            .filter(|u| u.owner == pid && u.kind == t)
                            .count() as i64,
                        n,
                    )
                } else if let Some(d) = trig.strip_prefix("district:") {
                    (
                        cities
                            .iter()
                            .flat_map(|c| c.districts.keys())
                            .filter(|have| self.district_family(**have) == d)
                            .count() as i64,
                        n,
                    )
                } else if let Some(bn) = trig.strip_prefix("building:") {
                    (
                        cities
                            .iter()
                            .filter(|c| c.buildings.iter().any(|x| x == bn))
                            .count() as i64,
                        n,
                    )
                } else if let Some(t) = trig.strip_prefix("tech:") {
                    flag(p.techs.contains(&Name::new(t)))
                } else if let Some(c) = trig.strip_prefix("civic:") {
                    flag(p.civics.contains(&Name::new(c)))
                } else if let Some(i) = trig.strip_prefix("improvement:") {
                    (
                        self.owned_tiles_where(pid, |tile| tile.improvement.as_deref() == Some(i)),
                        n,
                    )
                } else if let Some(i) = trig.strip_prefix("improvement_on_resource:") {
                    (
                        self.owned_tiles_where(pid, |tile| {
                            tile.improvement.as_deref() == Some(i) && tile.resource.is_some()
                        }),
                        n,
                    )
                } else if let Some(r) = trig.strip_prefix("improve_resource:") {
                    (
                        self.owned_tiles_where(pid, |tile| {
                            tile.resource.as_deref() == Some(r)
                                && tile.improvement.as_deref().is_some_and(|imp| {
                                    self.rules.resources[r].improvement == imp
                                        || self.rules.improvements.get(imp).is_some_and(|spec| {
                                            spec.resources.iter().any(|c| c == r)
                                        })
                                })
                        }),
                        1,
                    )
                } else if let Some(kind) = trig.strip_prefix("great_person_of:") {
                    (p.gp_claimed.get(kind).copied().unwrap_or(0), n)
                } else if trig.starts_with("kill_with:")
                    || trig.starts_with("kill_kind:")
                    || trig.starts_with("trained:")
                {
                    (counter(trig), n)
                } else if trig == "themed_buildings" {
                    let housed = self.housed_great_work_pieces(pid);
                    (
                        cities
                            .iter()
                            .map(|c| {
                                self.city_theming(
                                    pid,
                                    c.id,
                                    housed.get(&c.id).map(Vec::as_slice).unwrap_or(&[]),
                                )
                                .0 as i64
                            })
                            .sum::<i64>(),
                        n,
                    )
                } else if let Some(d) = trig.strip_prefix("district_appeal:") {
                    (
                        cities
                            .iter()
                            .flat_map(|c| c.districts.iter())
                            .filter(|(have, _)| self.district_family(**have) == d)
                            .map(|(_, pos)| self.tile_appeal(*pos) as i64)
                            .max()
                            .unwrap_or(i64::MIN),
                        n,
                    )
                } else if let Some(bn) = trig.strip_prefix("building_near_mountain:") {
                    let host = self
                        .rules
                        .buildings
                        .get(bn)
                        .and_then(|spec| spec.district)
                        .unwrap_or(crate::name!(""));
                    flag(cities.iter().any(|c| {
                        c.buildings.iter().any(|x| x == bn)
                            && c.districts.iter().any(|(have, pos)| {
                                self.district_family(*have) == host
                                    && self.nbrs(*pos).iter().any(|nb| {
                                        self.map.get(*nb).is_some_and(|t| t.terrain == "mountain")
                                    })
                            })
                    }))
                } else if let Some(spec) = trig.strip_prefix("unit_and_improve:") {
                    flag(spec.split_once(':').is_some_and(|(unit, resource)| {
                        self.units
                            .values()
                            .any(|u| u.owner == pid && u.kind == unit)
                            && self.owned_tiles_where(pid, |tile| {
                                tile.resource.as_deref() == Some(resource)
                                    && tile.improvement.as_deref().is_some_and(|imp| {
                                        self.rules.resources[resource].improvement == imp
                                    })
                            }) >= 1
                    }))
                } else {
                    (0, 1)
                }
            }
        }
    }

    pub(super) fn complete_random_nodes(&mut self, pid: usize, amount: usize, technologies: bool) {
        for _ in 0..amount {
            let candidates: Vec<Name> = if technologies {
                self.rules
                    .techs
                    .keys()
                    .filter(|node| !self.players[pid].techs.contains(*node))
                    .cloned()
                    .collect()
            } else {
                self.rules
                    .civics
                    .keys()
                    .filter(|node| !self.players[pid].civics.contains(*node))
                    .cloned()
                    .collect()
            };
            if candidates.is_empty() {
                break;
            }
            let node = candidates[self.rng.below(candidates.len())];
            if technologies {
                let first = self.players[pid].techs.insert(node);
                self.players[pid].boosted_techs.remove(&node);
                if self.players[pid].research.as_deref() == Some(node.as_str()) {
                    self.players[pid].research = None;
                    self.players[pid].research_progress = 0.0;
                }
                self.apply_tree_completion(pid, true, &node, first);
                if first {
                    self.share_team_technology_boost(pid, &node);
                }
            } else {
                let first = self.players[pid].civics.insert(node);
                self.players[pid].boosted_civics.remove(&node);
                if self.players[pid].civic.as_deref() == Some(node.as_str()) {
                    self.players[pid].civic = None;
                    self.players[pid].civic_progress = 0.0;
                }
                self.apply_tree_completion(pid, false, &node, first);
            }
        }
    }
}
