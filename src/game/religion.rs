//! Religion: pantheons, founding, beliefs, followers, and theological
//! combat.
//!
//! The pantheon (`do_choose_pantheon`, `pantheon_effect`, the Holy Site
//! heal), founding a religion (`do_found_religion`, `religions_founded`,
//! `religion_founder`), beliefs and their effects (`religion_belief_effect`,
//! `founder_belief_yields`, combat belief bonuses), a city's religion and
//! followers, and religious units: theological strength and its preview,
//! `do_theological_attack`, `do_heal_religious` and `do_evangelize_belief`.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

/// One row of a Civ VI city banner's religion list as a live host showed it
/// (`DLC/Expansion2/UI/CityBanners/CityBannerManager.lua:1938-1950`).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ObservedCityFaith {
    /// The faith by its CIVVIS name; `None` for citizens following none
    /// (the host's pantheon row).
    pub religion: Option<String>,
    /// Citizens following it.
    pub followers: i32,
    /// Pressure accumulated over the city's life, the banner tooltip's figure.
    pub pressure: f64,
    /// Pressure arriving each turn.
    pub per_turn: f64,
}

impl Game {
    /// What the host's banner showed of `religion` in city `cid`: its lifetime
    /// pressure and the lifetime pressure of every row together, the
    /// religionless row included. `None` when the host has shown no rows for
    /// the city, which is every city of a simulated game.
    pub fn observed_city_faith_pressure(&self, cid: u32, religion: &str) -> Option<(f64, f64)> {
        let rows = self.observed_city_faiths.get(&cid)?;
        let own = rows
            .iter()
            .filter(|row| row.religion.as_deref() == Some(religion))
            .map(|row| row.pressure)
            .sum();
        let total = rows.iter().map(|row| row.pressure).sum();
        Some((own, total))
    }

    /// God of Healing: `GOD_OF_HEALING_UNIT_HEALING_MODIFIER`,
    /// `MODIFIER_PLAYER_UNITS_ADJUST_HEAL_PER_TURN` with `Amount 30` and
    /// `Type ALL`, over `PLOT_ADJACENT_INCLUDE_HOLY_SITE`
    /// (`REQUIREMENT_PLOT_ADJACENT_DISTRICT_TYPE_MATCHES`, `DISTRICT_HOLY_SITE`,
    /// `MinRange 0`) — so the Holy Site's own plot counts as well as the ring
    /// around it, and every class heals, religious units included.
    ///
    /// ⚠ The requirement set names no owner; the shipped text does — "in YOUR
    /// Holy Site district, or any adjacent tiles" — so a rival's Holy Site does
    /// not heal this army. No id in this belief appears in
    /// `Expansion2_RemoveData.xml`.
    pub(super) fn pantheon_holy_site_heal(&self, unit: &Unit) -> i32 {
        let amount = self.pantheon_effect(unit.owner, "holy_site_heal");
        if amount == 0.0 {
            return 0;
        }
        let beside_a_holy_site = self.wdisk(unit.pos, 1).into_iter().any(|position| {
            self.map.get(position).is_some_and(|tile| {
                tile.district.is_some_and(|district| {
                    self.district_is_family(district, crate::name!("holy_site"))
                }) && tile
                    .owner_city
                    .and_then(|city_id| self.cities.get(&city_id))
                    .is_some_and(|city| city.owner == unit.owner)
            })
        });
        if beside_a_holy_site {
            amount.round() as i32
        } else {
            0
        }
    }

    pub fn religions_founded(&self) -> usize {
        self.players.iter().filter(|p| p.religion.is_some()).count()
    }

    pub(super) fn pantheon_effect(&self, pid: usize, effect: &str) -> f64 {
        self.players[pid]
            .pantheon
            .as_ref()
            .and_then(|belief| self.rules.beliefs.pantheon.get(belief))
            .and_then(|belief| belief.effects.get(effect))
            .copied()
            .unwrap_or(0.0)
    }

    /// Total Pressure competing for this city's Citizens, atheists included.
    fn city_total_pressure(&self, city: &City) -> f64 {
        city.pressure
            .values()
            .copied()
            .filter(|pressure| *pressure > 0.0)
            .sum::<f64>()
            + city.atheist_pressure.max(0.0)
    }

    /// A city's majority religion: the faith followed by more than half of its
    /// Citizens. Citizens are split by Pressure share, so this is the faith
    /// holding more than half of all Pressure — atheist Pressure counted.
    /// Below that share the city simply has no majority religion.
    pub fn city_religion<'a>(&self, city: &'a City) -> Option<&'a str> {
        let total = self.city_total_pressure(city);
        if total <= f64::EPSILON {
            return None;
        }
        city.pressure
            .iter()
            .filter(|(_, v)| **v > 0.0 && **v * 2.0 > total)
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap().then(b.0.cmp(a.0)))
            .map(|(r, _)| r.as_str())
    }

    pub(super) fn religion_founder(&self, religion: &str) -> Option<usize> {
        self.players
            .iter()
            .find(|p| p.religion.as_deref() == Some(religion))
            .map(|p| p.id)
    }

    pub(super) fn religion_belief_effect(&self, religion: &str, effect: &str) -> f64 {
        if !self.rules.effect_index.beliefs(effect) {
            return 0.0;
        }
        let Some(founder) = self.religion_founder(religion) else {
            return 0.0;
        };
        self.players[founder]
            .religion_beliefs
            .iter()
            .filter_map(|belief| {
                self.rules
                    .beliefs
                    .founder
                    .get(belief)
                    .or_else(|| self.rules.beliefs.follower.get(belief))
                    .or_else(|| self.rules.beliefs.enhancer.get(belief))
                    .or_else(|| self.rules.beliefs.worship.get(belief))
            })
            .filter_map(|belief| belief.effects.get(effect))
            .sum()
    }

    pub(super) fn city_religion_belief_effect(&self, city: &City, effect: &str) -> f64 {
        self.city_religion(city)
            .map(|religion| self.religion_belief_effect(religion, effect))
            .unwrap_or(0.0)
    }

    /// The map stores religious pressure rather than a separate follower
    /// roster. Divide population proportionally among the represented faiths,
    /// matching Civ VI's `Population * Pressure / Sum of all Religious and
    /// Atheist Pressures`, rounded to the nearest Citizen.
    pub(super) fn religious_followers_in_city(&self, city: &City, religion: &str) -> f64 {
        let total = self.city_total_pressure(city);
        if total <= f64::EPSILON {
            return 0.0;
        }
        let pressure = city.pressure.get(religion).copied().unwrap_or(0.0).max(0.0);
        (city.pop.max(0) as f64 * pressure / total)
            .round()
            .clamp(0.0, city.pop.max(0) as f64)
    }

    pub(super) fn founder_belief_yields(&self, pid: usize) -> Yields {
        let Some(religion) = self.players[pid].religion.as_deref() else {
            return Yields::default();
        };
        let following = self
            .cities
            .values()
            .filter(|city| self.city_religion(city) == Some(religion))
            .count() as f64;
        let foreign_following = self
            .cities
            .values()
            .filter(|city| city.owner != pid && self.city_religion(city) == Some(religion))
            .count() as f64;
        let followers = self
            .cities
            .values()
            .map(|city| self.religious_followers_in_city(city, religion))
            .sum::<f64>();
        let foreign_followers = self
            .cities
            .values()
            .filter(|city| city.owner != pid)
            .map(|city| self.religious_followers_in_city(city, religion))
            .sum::<f64>();
        let effect = |name| self.religion_belief_effect(religion, name);
        // Lay Ministry and Sacred Places (`BELIEF_YIELD_PER_DISTRICT`,
        // `BELIEF_YIELD_PER_CITY_WITH_WONDER`): counted over the cities that
        // follow the religion, wherever they stand, and paid to the founder.
        let (holy_sites, theater_squares, wonder_cities) = if effect("faith_per_holy_site")
            + effect("culture_per_theater_square")
            + effect("yield_per_wonder_city")
            > 0.0
        {
            self.cities
                .values()
                .filter(|city| self.city_religion(city) == Some(religion))
                .fold((0.0, 0.0, 0.0), |(holy, theater, wonders), city| {
                    let districts = |family: &str| {
                        city.districts
                            .iter()
                            .filter(|(district, position)| {
                                self.district_is_family(district, Name::new(family))
                                    && self.district_is_active(city, district, **position)
                            })
                            .count() as f64
                    };
                    (
                        holy + districts("holy_site"),
                        theater + districts("theater_square"),
                        wonders + if city.wonders.is_empty() { 0.0 } else { 1.0 },
                    )
                })
        } else {
            (0.0, 0.0, 0.0)
        };
        let (campuses, commercial_hubs) =
            if effect("science_per_campus") + effect("gold_per_commercial_hub") > 0.0 {
                self.cities
                    .values()
                    .filter(|city| self.city_religion(city) == Some(religion))
                    .fold((0.0, 0.0), |(campuses, commercial_hubs), city| {
                        let districts = |family: &str| {
                            city.districts
                                .iter()
                                .filter(|(district, position)| {
                                    self.district_is_family(district, Name::new(family))
                                        && self.district_is_active(city, district, **position)
                                })
                                .count() as f64
                        };
                        (
                            campuses + districts("campus"),
                            commercial_hubs + districts("commercial_hub"),
                        )
                    })
            } else {
                (0.0, 0.0)
            };
        let sacred_places = effect("yield_per_wonder_city") * wonder_cities;
        Yields {
            // ⚠ Every arm here has a belief that sets it, and that is
            // checked: `tools/civvis_inert.py` fails when this function prices
            // a key no belief supplies.
            //
            // ⚠⚠ THE SHAPES HERE ARE GATHERING STORM'S, NOT THE BASE GAME'S,
            // AND THE TWO DISAGREE ABOUT EXACTLY THESE BELIEFS. Vanilla pays
            // Tithe per follower, World Church and Pilgrimage on what is
            // foreign, and ships Church Property; `Expansion2_RemoveData.xml`
            // deletes all four of those modifiers and `Expansion2_Beliefs.xml`
            // replaces them with the per-city and per-follower forms below.
            // #2049 read the domestic/foreign distinction out of a compiled
            // cache that happened to hold the base game and changed these to
            // match it; #2050 put them back. See `docs/FIDELITY.md`.
            gold: effect("gold_per_city") * following
                + (effect("gold_per_followers") * followers).floor()
                + effect("gold_per_commercial_hub") * commercial_hubs
                + sacred_places,
            culture: (effect("culture_per_followers") * followers).floor()
                + (effect("culture_per_foreign_followers") * foreign_followers).floor()
                + effect("culture_per_theater_square") * theater_squares
                + sacred_places,
            // Cross-Cultural Dialogue counts followers abroad only.
            science: (effect("science_per_foreign_followers") * foreign_followers).floor()
                + effect("science_per_campus") * campuses
                + sacred_places,
            faith: effect("faith_per_city") * following
                + effect("faith_per_foreign_city") * foreign_following
                + effect("faith_per_holy_site") * holy_sites
                + sacred_places,
            ..Yields::default()
        }
    }

    /// Just War and Defender of the Faith, which ship the same requirement set
    /// bar one argument: both are TEST_ALL of NOT `CLASS_RELIGIOUS`, NOT
    /// `CLASS_SUPPORT`, founding the religion, and standing near a city that
    /// follows it -- `FriendlyCity` 0 for Just War and 1 for Defender. The two
    /// inverted tags matter: these are army beliefs, and an Apostle gets
    /// nothing from either in theological combat.
    pub(super) fn religious_combat_belief_bonus(&self, unit: &Unit) -> f64 {
        let (owner, position) = (unit.owner, unit.pos);
        let class = self.rules.units[unit.kind].class.as_str();
        if matches!(class, "religious" | "religious_apostle" | "support") {
            return 0.0;
        }
        let Some(religion) = self.players[owner].religion.as_deref() else {
            return 0.0;
        };
        let Some(city) = self
            .map
            .get(position)
            .and_then(|tile| tile.owner_city)
            .and_then(|city| self.cities.get(&city))
            .filter(|city| self.city_religion(city) == Some(religion))
        else {
            return 0.0;
        };
        let friendly = city.owner == owner
            || self.suzerain_of(city.owner) == Some(owner)
            || self.are_allied(owner, city.owner)
            || self.players[owner]
                .friends_until
                .get(&city.owner)
                .is_some_and(|until| *until > self.turn);
        if friendly {
            self.religion_belief_effect(religion, "friendly_city_combat_strength")
        } else {
            self.religion_belief_effect(religion, "foreign_city_combat_strength")
        }
    }

    pub(super) fn award_religious_unity_envoys(&mut self) {
        let awards: BTreeSet<(usize, usize)> = self
            .cities
            .values()
            .filter(|city| {
                self.players[city.owner].is_minor && !self.players[city.owner].is_barbarian
            })
            .filter_map(|city| {
                let religion = self.city_religion(city)?;
                let founder = self.religion_founder(religion)?;
                (self.religion_belief_effect(religion, "envoy_first_city_state_conversion") > 0.0)
                    .then_some((founder, city.owner))
            })
            .collect();
        for (founder, minor) in awards {
            let key = format!("religious_unity:{minor}");
            if self.players[founder]
                .counters
                .get(&key)
                .copied()
                .unwrap_or(0)
                > 0
            {
                continue;
            }
            self.players[founder].counters.insert(key, 1);
            match self.players[founder]
                .envoys
                .iter_mut()
                .find(|(city_state, _)| *city_state == minor)
            {
                Some((_, envoys)) => *envoys += 1,
                None => self.players[founder].envoys.push((minor, 1)),
            }
        }
    }

    pub(super) fn do_choose_pantheon(&mut self, pid: usize, belief: &str) -> Result<(), String> {
        if self.players[pid].is_minor || self.players[pid].is_barbarian {
            return Err("minor civilizations cannot choose pantheons".into());
        }
        if self.players[pid].pantheon.is_some() {
            return Err("pantheon already chosen".into());
        }
        let cost = self.pantheon_faith_cost();
        if self.players[pid].faith < cost {
            // ⚠ The price, not a literal. The old message said "needs 25 faith" on
            // every speed, so a refusal on Online named a number the game was not
            // charging and read as a CIVVIS bug rather than a scaling one.
            return Err(format!("needs {cost:.0} faith"));
        }
        if !self.rules.beliefs.pantheon.contains_key(belief) {
            return Err("no such pantheon belief".into());
        }
        if self
            .players
            .iter()
            .any(|p| p.pantheon.as_deref() == Some(belief))
            || self.blocked_pantheons.contains(&Name::new(belief))
        {
            return Err("belief already taken".into());
        }
        self.players[pid].pantheon = Some(belief.to_string());
        // ⚠⚠ THE PRICE THE CHECK ASKED FOR, NOT A SECOND LITERAL. #1044 made the
        // check above speed-aware and left this deduction at a bare 25.0, so on any
        // speed but Standard the engine charged MORE than it demanded: Online asks
        // for 12.5 and this took 25, dropping a player who founded at 13 faith to
        // -12. A check and a charge that disagree is worse than both being wrong.
        self.players[pid].faith -= cost;
        let capital = self
            .player_city_ids(pid)
            .into_iter()
            .find(|city| self.city_capital_flag(&self.cities[city]))
            .or_else(|| self.player_city_ids(pid).into_iter().min());
        if let Some(city) = capital {
            for (effect, unit) in [("free_builder", "builder"), ("free_settler", "settler")] {
                if self.pantheon_effect(pid, effect) > 0.0 {
                    self.place_new_unit(unit, pid, self.cities[&city].pos);
                }
            }
        }
        let first_in_world = !self
            .players
            .iter()
            .any(|other| other.id != pid && other.pantheon.is_some());
        self.players[pid]
            .counters
            .insert("historic_moment:pantheon_founded".to_string(), 1);
        self.add_historic_moment(
            pid,
            if first_in_world {
                "MOMENT_PANTHEON_FOUNDED_FIRST_IN_WORLD"
            } else {
                "MOMENT_PANTHEON_FOUNDED"
            },
        );
        Ok(())
    }

    pub(super) fn do_found_religion(
        &mut self,
        pid: usize,
        follower: &str,
        founder: &str,
    ) -> Result<(), String> {
        if !self.players[pid].prophet_pending {
            return Err("no great prophet available".into());
        }
        if self.religions_founded() >= self.max_religions() {
            return Err("this map has reached its religion limit".into());
        }
        if !self.rules.beliefs.follower.contains_key(follower)
            || !self.rules.beliefs.founder.contains_key(founder)
        {
            return Err("no such belief".into());
        }
        let taken = |b: &str| {
            self.players
                .iter()
                .any(|p| p.religion_beliefs.iter().any(|x| x == b))
        };
        if taken(follower) || taken(founder) {
            return Err("belief already taken".into());
        }
        let can_found_here = |city: &City| {
            self.city_has_district_family(city, crate::name!("holy_site"))
                || city.wonders.keys().any(|wonder| {
                    self.rules.wonders[wonder]
                        .effects
                        .get("religion_founding_site")
                        .copied()
                        .unwrap_or(0.0)
                        > 0.0
                })
        };
        let holy_site_cities: Vec<u32> = self
            .cities
            .values()
            .filter(|city| city.owner == pid && can_found_here(city))
            .map(|city| city.id)
            .collect();
        let holy = holy_site_cities
            .first()
            .copied()
            .ok_or_else(|| "needs a city with a holy site or Stonehenge".to_string())?;
        let name =
            Self::RELIGION_NAMES[self.religions_founded() % Self::RELIGION_NAMES.len()].to_string();
        self.players[pid].prophet_pending = false;
        self.players[pid].religion = Some(name.clone());
        self.players[pid].holy_city = Some(holy);
        let first_in_world = !self
            .players
            .iter()
            .any(|other| other.id != pid && other.religion.is_some());
        self.players[pid]
            .counters
            .insert("historic_moment:religion_founded".to_string(), 1);
        self.add_historic_moment(
            pid,
            if first_in_world {
                "MOMENT_RELIGION_FOUNDED_FIRST_IN_WORLD"
            } else {
                "MOMENT_RELIGION_FOUNDED"
            },
        );
        self.players[pid].religion_beliefs = vec![follower.to_string(), founder.to_string()];
        for cid in holy_site_cities {
            self.cities
                .get_mut(&cid)
                .unwrap()
                .pressure
                .insert(name.clone(), 1000.0);
        }
        Ok(())
    }

    pub(super) fn city_ignores_foreign_religion(&self, city: &City, religion: &str) -> bool {
        self.governor_effect(city.owner, city.id, "ignore_foreign_religious_pressure") > 0.0
            && self.players[city.owner].religion.as_deref() != Some(religion)
    }

    pub(super) fn religious_combat_pressure(
        &mut self,
        winner: Option<(&str, usize)>,
        loser: &str,
        pos: Pos,
        radius: i32,
        amount: f64,
    ) {
        let pressure_loss = (1.0
            - self
                .religion_belief_effect(loser, "theological_combat_loss_reduction_pct")
                .clamp(0.0, 100.0)
                / 100.0)
            .max(0.0);
        let targets: Vec<u32> = self
            .cities
            .values()
            .filter(|city| self.wdist(city.pos, pos) <= radius)
            .map(|city| city.id)
            .collect();
        for cid in targets {
            // Imported majority/warning values are not pressure amounts.
            // Removing a religious unit still changes the unit board, but
            // only a later host snapshot can report nearby conversions.
            if self.observed_city_religion_markers.contains(&cid) {
                continue;
            }
            if self.city_ignores_foreign_religion(&self.cities[&cid], loser)
                || winner.is_some_and(|(religion, _)| {
                    self.city_ignores_foreign_religion(&self.cities[&cid], religion)
                })
            {
                continue;
            }
            let previous_religion = self.city_religion(&self.cities[&cid]).map(str::to_string);
            let city_owner = self.cities[&cid].owner;
            {
                let city = self.cities.get_mut(&cid).unwrap();
                let losing = city.pressure.entry(loser.to_string()).or_insert(0.0);
                *losing = (*losing - amount * pressure_loss).max(0.0);
                if let Some((religion, _)) = winner {
                    *city.pressure.entry(religion.to_string()).or_insert(0.0) += amount;
                }
            }
            if let Some((religion, converter)) = winner {
                let converted = previous_religion.as_deref() != Some(religion)
                    && self.city_religion(&self.cities[&cid]) == Some(religion);
                if converted {
                    self.break_promises_on_conversion(converter, city_owner);
                }
            }
        }
    }

    pub(super) fn theological_strength(&self, unit: &Unit) -> f64 {
        let mut strength = self.rules.units[unit.kind].religious_strength
            + self.promotion_effect(unit, "religious_strength");
        strength += self.adjacent_friendly_unit_effect(unit, "adjacent_religious_strength");
        strength += self.gov_effects(unit.owner).religious_strength;
        strength += self.policy_effect(unit.owner, "religious_strength");
        strength += self.taxis_holy_city_strength(unit.owner);
        if self
            .map
            .get(unit.pos)
            .and_then(|tile| tile.owner_city)
            .and_then(|city_id| self.cities.get(&city_id))
            .is_some_and(|city| city.owner == unit.owner)
        {
            strength += self.policy_effect(unit.owner, "home_religious_strength");
        }
        strength += self.religious_combat_belief_bonus(unit);
        if let Some(city) = self
            .map
            .get(unit.pos)
            .and_then(|tile| tile.owner_city)
            .and_then(|city_id| self.cities.get(&city_id))
            .filter(|city| city.owner == unit.owner)
        {
            strength += self.governor_effect(unit.owner, city.id, "theological_strength");
        }
        if self.wdisk(unit.pos, 1).into_iter().any(|position| {
            self.unit_ids_at(position).iter().any(|other| {
                self.units[other].owner == unit.owner
                    && self.units[other].kind == "guru"
                    && self.units[other].religion == unit.religion
            })
        }) {
            strength += self.empire_wonder_effect(unit.owner, "guru_adjacent_religious_strength");
        }
        let Some(religion) = unit.religion.as_deref() else {
            return strength;
        };
        if self.congress_effect_active("world_religion", "A", religion) {
            strength += 10.0;
        }
        strength += self.theological_holy_ground(unit, religion);
        // The unit's own id never stands beside its own tile; the filter
        // matters only to a probe read off its tile (see
        // `theological_exchange_preview_with_defender_at`).
        strength
            + 2.0
                * self
                    .nbrs(unit.pos)
                    .into_iter()
                    .flat_map(|pos| self.unit_ids_at(pos))
                    .filter(|id| {
                        **id != unit.id && self.units[id].religion.as_deref() == Some(religion)
                    })
                    .count() as f64
    }

    /// The tile-bound part of [`Self::theological_strength`]: land of a city
    /// of this faith, beside one of its Holy Sites, and its Holy City's land.
    fn theological_holy_ground(&self, unit: &Unit, religion: &str) -> f64 {
        let mut ground = 0.0;
        if self
            .map
            .get(unit.pos)
            .and_then(|tile| tile.owner_city)
            .and_then(|cid| self.cities.get(&cid))
            .is_some_and(|city| self.city_religion(city) == Some(religion))
        {
            ground += 5.0;
        }
        if self.wdisk(unit.pos, 1).into_iter().any(|position| {
            let Some(tile) = self.map.get(position) else {
                return false;
            };
            tile.district.is_some_and(|district| {
                self.district_is_family(district, crate::name!("holy_site"))
            }) && tile
                .owner_city
                .and_then(|cid| self.cities.get(&cid))
                .is_some_and(|city| self.city_religion(city) == Some(religion))
        }) {
            ground += 5.0;
        }
        if self
            .religion_founder(religion)
            .and_then(|founder| self.players[founder].holy_city)
            .and_then(|cid| self.cities.get(&cid))
            .is_some_and(|city| city.owned_tiles.contains(&unit.pos))
        {
            ground += 15.0;
        }
        ground
    }

    /// The exchange a `TheologicalAttack` from `uid` on `target` would resolve,
    /// both rolls taken at their mean (the strengths `do_theological_attack`
    /// fights with, no RNG): `(blow dealt, blow received, defender hp)`.
    /// `None` when there is no such attacker or no opposing religious unit.
    pub(crate) fn theological_exchange_preview(
        &self,
        uid: u32,
        target: Pos,
    ) -> Option<(f64, f64, i32)> {
        self.theological_exchange_reading(uid, target, false)
    }

    /// [`Self::theological_exchange_preview`] with the strength a live host
    /// adds to a theological fight beyond [`Self::theological_strength`]: each
    /// side's difficulty bonus and its visibility edge over the other. The
    /// host's own preview of 75 religious strikes on 2026-11-09 named "+4
    /// Combat Strength bonus due to difficulty" on 68 defenders and "+3" or
    /// "+6 Intel on opponent's movements" on 55.
    ///
    /// It also takes the attacker's Holy Ground back out: in 346 previews
    /// from 2026-11-05 to 11-09 the term appears on 79 defenders and on no
    /// attacker, including attackers standing on their own faith's land.
    pub(crate) fn theological_exchange_with_host_edge(
        &self,
        uid: u32,
        target: Pos,
    ) -> Option<(f64, f64, i32)> {
        self.theological_exchange_reading(uid, target, true)
    }

    fn theological_exchange_reading(
        &self,
        uid: u32,
        target: Pos,
        host_edge: bool,
    ) -> Option<(f64, f64, i32)> {
        let attacker = self.units.get(&uid)?;
        let attacker_religion = attacker.religion.clone()?;
        let defender_id = self.units_at(target).into_iter().find(|other_id| {
            let defender = &self.units[other_id];
            defender.owner != attacker.owner
                && self.rules.units[defender.kind].class == "religious"
                && defender
                    .religion
                    .as_deref()
                    .is_some_and(|r| r != attacker_religion)
        })?;
        let defender = &self.units[&defender_id];
        let defender_religion = defender.religion.clone()?;
        let edge = |owner: usize, opponent: usize| {
            if host_edge {
                self.handicap_combat_strength(owner) + self.visibility_combat_bonus(owner, opponent)
            } else {
                0.0
            }
        };
        let attacker_ground = if host_edge {
            self.theological_holy_ground(attacker, &attacker_religion)
        } else {
            0.0
        };
        let att = effective_strength(
            self.theological_strength(attacker) - attacker_ground
                + self.religious_alliance_combat_bonus(attacker.owner, &defender_religion)
                + edge(attacker.owner, defender.owner),
            attacker.hp,
        );
        let def = effective_strength(
            self.theological_strength(defender)
                + self.religious_alliance_combat_bonus(defender.owner, &attacker_religion)
                + edge(defender.owner, attacker.owner),
            defender.hp,
        );
        Some((
            expected_damage(att, def),
            expected_damage(def, att),
            defender.hp,
        ))
    }

    /// [`Self::theological_exchange_preview`] with `defender` read as if it
    /// stood on `at`: the tile-bound terms of its strength (home territory, a
    /// city or Holy Site of its faith, its Holy City, the units of its faith
    /// beside it) follow it there. `None` when either unit is missing or
    /// faithless, or the two share a faith.
    pub(crate) fn theological_exchange_preview_with_defender_at(
        &self,
        uid: u32,
        defender: u32,
        at: Pos,
    ) -> Option<(f64, f64, i32)> {
        let attacker = self.units.get(&uid)?;
        let attacker_religion = attacker.religion.clone()?;
        let mut probe = self.units.get(&defender)?.clone();
        let defender_religion = probe.religion.clone()?;
        if defender_religion == attacker_religion {
            return None;
        }
        probe.pos = at;
        let att = effective_strength(
            self.theological_strength(attacker)
                + self.religious_alliance_combat_bonus(attacker.owner, &defender_religion),
            attacker.hp,
        );
        let def = effective_strength(
            self.theological_strength(&probe)
                + self.religious_alliance_combat_bonus(probe.owner, &attacker_religion),
            probe.hp,
        );
        Some((
            expected_damage(att, def),
            expected_damage(def, att),
            probe.hp,
        ))
    }

    pub(super) fn do_theological_attack(
        &mut self,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> Result<(), String> {
        let attacker = self.own_unit(pid, uid)?;
        if !matches!(attacker.kind.as_str(), "apostle" | "inquisitor") || attacker.moves_left <= 0.0
        {
            return Err("unit cannot initiate theological combat".into());
        }
        if self.wdist(attacker.pos, target) != 1 {
            return Err("target not adjacent".into());
        }
        let attacker_religion = attacker
            .religion
            .clone()
            .ok_or_else(|| "attacker has no religion".to_string())?;
        let defender_id = self
            .units_at(target)
            .into_iter()
            .find(|other_id| {
                let defender = &self.units[other_id];
                let spec = &self.rules.units[defender.kind];
                defender.owner != pid
                    && spec.class == "religious"
                    && defender
                        .religion
                        .as_deref()
                        .is_some_and(|r| r != attacker_religion)
            })
            .ok_or_else(|| "no opposing religious unit".to_string())?;
        let defender = self.units[&defender_id].clone();
        let defender_religion = defender.religion.clone().unwrap();
        let att = effective_strength(
            self.theological_strength(&attacker)
                + self.religious_alliance_combat_bonus(attacker.owner, &defender_religion),
            attacker.hp,
        );
        let def = effective_strength(
            self.theological_strength(&defender)
                + self.religious_alliance_combat_bonus(defender.owner, &attacker_religion),
            defender.hp,
        );
        let dealt = damage(att, def, &mut self.rng);
        let received = damage(def, att, &mut self.rng);
        self.apply_unit_damage(uid, defender_id, dealt);
        self.apply_unit_damage(defender_id, uid, received);
        {
            let unit = self.units.get_mut(&uid).unwrap();
            unit.moves_left = 0.0;
            unit.attacks_left = 0;
            unit.acted = true;
        }
        let attacker_dead = self.units[&uid].hp <= 0;
        let defender_dead = self.units[&defender_id].hp <= 0;
        if defender_dead {
            if self.promotion_effect(&defender, "relic_on_death") > 0.0 {
                let (owner, era) = (defender.owner, self.world_era);
                self.grant_great_work(owner, "relic", era, "martyr");
            }
            self.remove_unit(defender_id);
            self.religious_combat_pressure(
                Some((&attacker_religion, attacker.owner)),
                &defender_religion,
                target,
                10,
                250.0,
            );
        }
        if attacker_dead {
            if self.promotion_effect(&attacker, "relic_on_death") > 0.0 {
                let (owner, era) = (attacker.owner, self.world_era);
                self.grant_great_work(owner, "relic", era, "martyr");
            }
            self.remove_unit(uid);
            self.religious_combat_pressure(
                Some((&defender_religion, defender.owner)),
                &attacker_religion,
                attacker.pos,
                10,
                250.0,
            );
        }
        self.check_religious_victory();
        Ok(())
    }

    pub(super) fn do_heal_religious(&mut self, pid: usize, uid: u32) -> Result<(), String> {
        let guru = self.own_unit(pid, uid)?;
        if guru.kind != "guru" || guru.charges <= 0 || guru.moves_left <= 0.0 {
            return Err("not a Guru with a healing charge".into());
        }
        let religion = guru.religion.clone();
        let targets: Vec<u32> = self
            .player_unit_ids(pid)
            .into_iter()
            .filter(|other_id| self.wdist(guru.pos, self.units[other_id].pos) <= 1)
            .filter(|other_id| {
                self.rules.units[self.units[other_id].kind].class == "religious"
                    && self.units[other_id].religion == religion
                    && self.units[other_id].hp < 100
            })
            .collect();
        if targets.is_empty() {
            return Err("no damaged adjacent religious units".into());
        }
        for target in targets {
            self.units.get_mut(&target).unwrap().hp = (self.units[&target].hp + 40).min(100);
        }
        let guru = self.units.get_mut(&uid).unwrap();
        guru.charges -= 1;
        guru.moves_left = 0.0;
        guru.acted = true;
        if guru.charges <= 0 {
            self.remove_unit(uid);
        }
        Ok(())
    }

    pub(super) fn do_evangelize_belief(
        &mut self,
        pid: usize,
        uid: u32,
        belief: &str,
    ) -> Result<(), String> {
        let apostle = self.own_unit(pid, uid)?;
        if apostle.kind != "apostle"
            || apostle.moves_left <= 0.0
            || apostle.religion != self.players[pid].religion
            || self.players[pid].religion_beliefs.len() >= 4
        {
            return Err("Apostle cannot evangelize another belief".into());
        }
        if !self.rules.beliefs.enhancer.contains_key(belief)
            && !self.rules.beliefs.worship.contains_key(belief)
        {
            return Err("belief is not an enhancer or worship belief".into());
        }
        let enhancer = self.rules.beliefs.enhancer.contains_key(belief);
        let category_already_chosen = self.players[pid].religion_beliefs.iter().any(|chosen| {
            if enhancer {
                self.rules.beliefs.enhancer.contains_key(chosen)
            } else {
                self.rules.beliefs.worship.contains_key(chosen)
            }
        });
        if category_already_chosen {
            return Err("religion already has a belief in that category".into());
        }
        if self
            .players
            .iter()
            .any(|player| player.religion_beliefs.iter().any(|taken| taken == belief))
        {
            return Err("belief already taken".into());
        }
        self.players[pid].religion_beliefs.push(belief.to_string());
        if self.players[pid].religion_beliefs.len() == 4 {
            self.first_historic_moment(
                pid,
                "religion_fully_developed",
                Some("MOMENT_BELIEF_ADDED_MAX_BELIEFS_REACHED"),
                Some("MOMENT_BELIEF_ADDED_MAX_BELIEFS_REACHED_FIRST_IN_WORLD"),
            );
        }
        self.remove_unit(uid);
        Ok(())
    }
}
