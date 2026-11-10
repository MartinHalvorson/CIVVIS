//! Loyalty: how loyal each city is, and what happens when it is lost.
//!
//! `loyalty_change_for_city` and its pressure breakdown, `city_loyalty_per_turn`,
//! the effects of happiness and of friends on loyalty, the yield and growth
//! multipliers loyalty applies, a city's `loyalty_state`, and the turn's
//! `process_loyalty`. When a city declares independence, the Free City
//! player and its defenders come from here too.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    pub(super) fn ensure_free_city_player(&mut self) -> usize {
        if let Some(player) = self.players.iter().find(|player| player.is_free_city) {
            return player.id;
        }
        let pid = self.players.len();
        let mut player = Player::new(pid, "Free Cities", true);
        player.alive = false;
        player.is_barbarian = true;
        player.is_free_city = true;
        self.players.push(player);
        pid
    }

    pub(super) fn place_city_state_starting_unit(&mut self, kind: &str, owner: usize, center: Pos) {
        if self.place_new_unit(kind, owner, center).is_some() {
            return;
        }
        let want_sea = self.rules.units[kind].domain.as_deref() == Some("sea");
        let mut candidates = self.wdisk(center, 6);
        candidates.sort_by_key(|position| (self.wdist(center, *position), *position));
        let position = candidates.into_iter().find(|position| {
            self.map.get(*position).is_some_and(|tile| {
                self.rules.is_passable(tile)
                    && self.rules.is_water(tile) == want_sea
                    && self.unit_ids_at(*position).is_empty()
                    && tile
                        .owner_city
                        .is_none_or(|city| self.cities[&city].owner == owner)
            })
        });
        if let Some(position) = position {
            self.spawn_unit(kind, owner, position);
        }
    }

    // -------------------------------------------------- difficulty and speed

    /// Gilgamesh: standing beside him, and beside the people he stands with.
    pub(super) fn loyalty_to_friends(&self, observer: usize, subject: usize) -> f64 {
        let mut score: f64 = 0.0;
        if self.are_allied(observer, subject) {
            score += 1.0;
        } else if self.are_friends(observer, subject) {
            score += 0.6;
        }
        if self.is_at_war(observer, subject) {
            score -= 1.0;
        }
        // Attacking his friends is the same offence as attacking him.
        for player in &self.players {
            if player.id == observer || player.id == subject || player.is_barbarian {
                continue;
            }
            let befriended =
                self.are_friends(observer, player.id) || self.are_allied(observer, player.id);
            if befriended && self.is_at_war(subject, player.id) {
                score -= 0.5;
            }
        }
        score.clamp(-1.0, 1.0)
    }

    /// Happiness is a direct Loyalty source in Rise & Fall / Gathering
    /// Storm: Happy/Ecstatic add +3/+6; Displeased and every worse state
    /// subtract -3/-6 respectively.
    pub(super) fn happiness_loyalty_delta(surplus: i64) -> f64 {
        if surplus >= 5 {
            6.0
        } else if surplus >= 3 {
            3.0
        } else if surplus >= 0 {
            0.0
        } else if surplus >= -2 {
            -3.0
        } else {
            -6.0
        }
    }

    /// Rise & Fall / Gathering Storm loyalty yield band, taken from the
    /// shipped `LoyaltyLevels` table (`YieldChange` per level, one step per
    /// display band): Loyal 76-100 pays 0, Wavering 51-75 pays -0.25,
    /// Disloyal 26-50 pays -0.5, and Unrest 0-25 pays -1.0 — a city in
    /// Unrest really does produce nothing.
    ///
    /// The Civilopedia prose ("below 75 … penalized 25%; below 25 … 50%
    /// yields and 75% growth") describes two steps and disagrees with the
    /// table the game actually runs on. The table wins; do not "fix" this
    /// back from the prose.
    pub(super) fn loyalty_yield_mult(loyalty: f64) -> f64 {
        if loyalty > 75.0 {
            1.0
        } else if loyalty > 50.0 {
            0.75
        } else if loyalty > 25.0 {
            0.50
        } else {
            0.0
        }
    }

    /// `LoyaltyLevels.GrowthChange` is the multiplier outright: Loyal 1.0,
    /// Wavering 0.75, Disloyal 0.25, Unrest 0.0.
    pub(super) fn loyalty_growth_mult(loyalty: f64) -> f64 {
        if loyalty > 75.0 {
            1.0
        } else if loyalty > 50.0 {
            0.75
        } else if loyalty > 25.0 {
            0.25
        } else {
            0.0
        }
    }

    pub fn loyalty_state(loyalty: f64) -> &'static str {
        if loyalty > 75.0 {
            "loyal"
        } else if loyalty > 50.0 {
            "wavering"
        } else if loyalty > 25.0 {
            "disloyal"
        } else {
            "unrest"
        }
    }

    /// Calculate one city's complete per-turn Loyalty change. Keeping this
    /// read-only makes the exact value available to clients and AI without
    /// duplicating (and eventually drifting from) the turn processor.
    pub(super) fn loyalty_change_for_city(&self, pid: usize, cid: u32) -> LoyaltyChange {
        self.loyalty_change_for_city_with_pressure(pid, cid, None)
    }

    pub(super) fn loyalty_change_for_city_with_pressure(
        &self,
        pid: usize,
        cid: u32,
        population_pressure: Option<f64>,
    ) -> LoyaltyChange {
        // Loyalty weighs this city against every other within range, and asks
        // several of them the same empire-wide questions.
        let _memo = self.query_memo();
        let city = &self.cities[&cid];
        let cpos = city.pos;
        let protected = self
            .cities
            .values()
            .filter(|source| source.owner == pid)
            .flat_map(|source| source.wonders.iter())
            .any(|(wonder, position)| {
                self.rules.wonders[wonder]
                    .effects
                    .get("regional_full_loyalty_range")
                    .is_some_and(|range| self.wdist(*position, cpos) <= *range as i32)
            });
        if protected {
            return LoyaltyChange {
                delta: 100.0 - city.loyalty,
                locked: true,
                ..LoyaltyChange::default()
            };
        }

        let age_factor = |owner: usize| match self.players[owner].age.as_str() {
            "golden" | "heroic" => 1.5,
            "dark" => 0.5,
            _ => 1.0,
        };
        let mut domestic_pressure = 0.0;
        let mut foreign_pressure = 0.0;
        let mut pressure_by_civ: BTreeMap<usize, f64> = BTreeMap::new();
        for source in self.cities.values() {
            let owner = &self.players[source.owner];
            // A true barbarian speaks for nobody. The Free Cities seat carries
            // the same hostile flag so that everyone is at war with it, so it
            // has to be named apart from them here.
            if owner.is_barbarian && !owner.is_free_city {
                continue;
            }
            // A minor civilization projects no pressure onto its neighbours:
            // the shipped population tooltip drops the "also applies to other
            // cities within 9 tiles" clause for minors. Its own Citizens still
            // speak for the city they live in, and that is the whole domestic
            // side of a city-state's or Free City's own balance. Skipping them
            // outright left every city-state comparing 0 against its
            // neighbours and reading a permanent -20.
            if (owner.is_minor || owner.is_free_city) && source.id != cid {
                continue;
            }
            let distance = self.wdist_capped(source.pos, cpos, 9);
            if distance > 9 {
                continue;
            }
            // "Each Citizen exerts a base pressure of 1, but it can be
            // modified. For example, Citizens in a Capital city exert an
            // additional 1 pressure." Every per-citizen modifier stacks onto
            // that base before the distance falloff is applied to the total.
            let mut pressure_per_citizen = age_factor(source.owner)
                + self.city_active_project_effect(source, "citizen_loyalty_pressure");
            if self
                .observed_city_palaces
                .get(&source.id)
                .copied()
                .unwrap_or(source.is_capital && source.original_owner == source.owner)
            {
                pressure_per_citizen += 1.0;
            }
            let pressure = source.pop as f64 * (10.0 - distance as f64) * pressure_per_citizen;
            if source.owner == pid {
                domestic_pressure += pressure;
            } else if self.same_team(pid, source.owner) {
                // Team cities are neutral rather than one population-pressure
                // empire, matching their permanent-alliance semantics.
                continue;
            } else if !self
                .alliance_with(pid, source.owner)
                .is_some_and(|alliance| alliance.kind == "cultural")
            {
                foreign_pressure += pressure;
                *pressure_by_civ.entry(source.owner).or_insert(0.0) += pressure;
            }
        }

        let mut delta = population_pressure.unwrap_or(
            (10.0 * (domestic_pressure - foreign_pressure)
                / (domestic_pressure.min(foreign_pressure) + 0.5))
                .clamp(-20.0, 20.0),
        );
        // `IDENTITY_PER_TURN_FROM_FREE_CITIES` 10 and
        // `IDENTITY_PER_TURN_FROM_CITY_STATES` 20 — a Free City's desire for
        // independence and a city-state's base strength as itself. The Free
        // Cities seat is also a minor, so it answers first.
        if self.players[pid].is_free_city {
            delta += 10.0;
        } else if self.players[pid].is_minor {
            delta += 20.0;
        }
        delta += Self::happiness_loyalty_delta(self.city_amenity_surplus(city));
        if self.city_yields(cid).food + f64::EPSILON < 2.0 * city.pop as f64 {
            delta -= 4.0;
        }
        if self.city_governor_active(pid, cid) {
            delta += 8.0 + self.policy_effect(pid, "governor_loyalty");
        } else {
            delta += self
                .cities
                .values()
                .filter(|source| source.owner == pid)
                .map(|source| self.city_building_effect(source, "no_governor_loyalty"))
                .sum::<f64>();
        }
        if let Some(victor_city) = self.established_governor_city(pid, "victor") {
            if self.wdist(self.cities[&victor_city].pos, cpos) <= 9 {
                // Gathering Storm applies the aura to Victor's own city too,
                // despite the Civilopedia saying "other cities."
                delta += self.governor_effect(pid, victor_city, "nearby_city_loyalty");
            }
        }
        let garrison_formation = self
            .unit_ids_at(cpos)
            .iter()
            .filter(|uid| {
                self.units[uid].owner == pid
                    && self.rules.units[self.units[uid].kind].class == "military"
            })
            .map(|uid| self.units[uid].formation)
            .max();
        let garrisoned = garrison_formation.is_some();
        if let Some(formation) = garrison_formation {
            delta += self.policy_effect(pid, "garrison_loyalty");
            delta += if formation > 0 {
                self.civ_effect(pid, "formation_garrison_loyalty")
            } else {
                self.civ_effect(pid, "garrison_loyalty")
            };
        }
        // Occupation. Rise & Fall charged a flat -1 to -5; Gathering Storm
        // rescaled it to "Loyalty penalties based on the conqueror's
        // Grievances caused against the city's original owner" —
        // `IDENTITY_PER_TURN_FROM_OCCUPATION_MULTIPLIER` 25 percent of those
        // Grievances, held between `_MIN` 0 and `_MAX` 10. The penalty is
        // charged whether or not a garrison stands there; what a garrison buys
        // is `IDENTITY_PER_TURN_FROM_MARTIAL_LAW` +8, which is why holding a
        // fresh conquest down with troops raises its Loyalty rather than
        // merely stopping the bleed.
        let occupier = city
            .occupied_from
            .filter(|former| self.players.get(*former).is_some_and(|player| player.alive));
        if let Some(former) = occupier {
            let grievances = self.players[former]
                .grievances
                .get(&pid)
                .copied()
                .unwrap_or(0.0);
            delta -= (0.25 * grievances).clamp(0.0, 10.0);
            if garrisoned {
                delta += 8.0;
            }
        }
        if let Some(founded_religion) = self.players[pid].religion.as_deref() {
            if let Some(city_religion) = self.city_religion(city) {
                delta += if city_religion == founded_religion {
                    3.0
                } else {
                    -3.0
                };
            }
        }
        // Amani's Emissary pressure applies to every city in range that is
        // not owned by her civilization, including from a city-state post.
        delta += (0..self.players.len())
            .filter(|diplomat| *diplomat != pid)
            .filter_map(|diplomat| {
                let source = self.established_governor_city(diplomat, "amani")?;
                (self.wdist(self.cities.get(&source)?.pos, cpos) <= 9)
                    .then(|| self.governor_effect(diplomat, source, "foreign_city_loyalty"))
            })
            .sum::<f64>();
        delta += city
            .districts
            .iter()
            .filter(|(district, position)| self.district_is_active(city, district, **position))
            .map(|(district, position)| {
                let spec = &self.rules.districts[district];
                spec.loyalty
                    + if self.on_foreign_continent(pid, *position) {
                        spec.effects
                            .get("foreign_continent_loyalty")
                            .copied()
                            .unwrap_or(0.0)
                    } else {
                        0.0
                    }
            })
            .sum::<f64>();
        delta += self.city_building_effect(city, "loyalty_per_turn");
        // Improvement loyalty is city-scoped even when the tile is not being
        // worked: Sweden's Open-Air Museum stabilizes its host city, and a
        // Spanish Mission beside an overseas City Center anchors that colony.
        delta += city
            .owned_tiles
            .iter()
            .filter(|position| !self.map.tiles[position].pillaged)
            .filter_map(|position| self.map.tiles[position].improvement)
            .map(|improvement| {
                self.rules.improvements[improvement]
                    .effects
                    .get("city_loyalty")
                    .copied()
                    .unwrap_or(0.0)
            })
            .sum::<f64>();
        if self.on_foreign_continent(pid, cpos) {
            delta += self
                .nbrs(cpos)
                .iter()
                .filter(|position| {
                    let tile = &self.map.tiles[position];
                    tile.owner_city == Some(cid)
                        && !tile.pillaged
                        && tile.improvement.as_deref() == Some("mission")
                })
                .map(|_| {
                    self.rules.improvements["mission"]
                        .effects
                        .get("foreign_continent_city_loyalty")
                        .copied()
                        .unwrap_or(0.0)
                })
                .sum::<f64>();
        }
        if self.on_foreign_continent(pid, cpos) {
            delta += self.policy_effect(pid, "foreign_continent_city_loyalty");
        }
        // Automated Workforce's ADJUST_IDENTITY_PER_TURN (-5), every city.
        delta += self.policy_effect(pid, "city_loyalty");
        delta += self
            .cities
            .values()
            .filter(|source| source.owner == pid)
            .flat_map(|source| source.wonders.keys().map(move |wonder| (source, wonder)))
            .map(|(source, wonder)| {
                let spec = &self.rules.wonders[wonder];
                if spec.regional_loyalty > 0.0
                    && spec.regional_range > 0
                    && self.wdist(source.pos, cpos) <= spec.regional_range
                {
                    spec.regional_loyalty
                } else {
                    0.0
                }
            })
            .sum::<f64>();
        if self.congress_effect_active("migration_treaty", "A", &pid.to_string()) {
            delta -= 5.0;
        } else if self.congress_effect_active("migration_treaty", "B", &pid.to_string()) {
            delta += 5.0;
        }
        if self.active_emergencies.iter().any(|emergency| {
            emergency.ends > self.turn && emergency.target == pid && emergency.city == cid
        }) {
            delta += 20.0;
        }

        LoyaltyChange {
            delta,
            foreign_pressure: pressure_by_civ,
            locked: false,
        }
    }

    pub fn city_loyalty_per_turn(&self, city: &City) -> f64 {
        if let Some(delta) = self.observed_city_loyalty_per_turn.get(&city.id) {
            return *delta;
        }
        self.loyalty_change_for_city(city.owner, city.id).delta
    }

    /// Free Cities immediately field two current, generic melee defenders.
    /// Their shared seat inherits technology before this is called, so later
    /// revolts naturally upgrade from Warriors through the melee line.
    fn free_city_defender(&self, free_cities: usize) -> Option<Name> {
        self.rules
            .units
            .iter()
            .filter(|(unit, spec)| {
                spec.class == "military"
                    && spec.buildable
                    && spec.unique_to.is_none()
                    && spec.promotion_class == "melee"
                    && self.unlocked(free_cities, &spec.tech, &spec.civic)
                    && !self.unit_is_obsolete(free_cities, unit)
            })
            .max_by(|(left_name, left), (right_name, right)| {
                left.strength
                    .total_cmp(&right.strength)
                    .then_with(|| left.cost.total_cmp(&right.cost))
                    .then_with(|| right_name.cmp(left_name))
            })
            .map(|(unit, _)| *unit)
    }

    /// Apply population pressure and every local Loyalty source. A major city
    /// first revolts into the hostile Free Cities seat at 100 Loyalty. A Free
    /// City at zero then joins the living major that accumulated the greatest
    /// population pressure during its independence.
    ///
    /// A city-state runs the same rules as anybody else. It is not exempt —
    /// what keeps it in place is the +20 base strength it carries and the fact
    /// that its own Citizens are the whole domestic side of its balance. An
    /// overwhelmed city-state revolts into a Free City like a major's city
    /// does. Barbarians hold no Loyalty at all — but the Free Cities seat is
    /// flagged as one so that everybody is at war with it, so it is named
    /// apart from them.
    pub(super) fn process_loyalty(&mut self, pid: usize) {
        let player = &self.players[pid];
        if player.is_barbarian && !player.is_free_city {
            return;
        }

        enum Flip {
            Revolt(u32),
            Join(u32, usize),
        }

        let mut flips = Vec::new();
        for cid in self.player_city_ids(pid) {
            let change = self.loyalty_change_for_city(pid, cid);
            if change.locked {
                self.cities.get_mut(&cid).unwrap().loyalty = 100.0;
                continue;
            }
            if self.players[pid].is_free_city {
                let city = self.cities.get_mut(&cid).unwrap();
                for (civilization, pressure) in change.foreign_pressure {
                    *city.free_city_pressure.entry(civilization).or_insert(0.0) += pressure;
                }
            }
            let city = self.cities.get_mut(&cid).unwrap();
            city.loyalty = (city.loyalty + change.delta).clamp(0.0, 100.0);
            if city.loyalty > 0.0 {
                continue;
            }
            if self.players[pid].is_free_city {
                let new_owner = city
                    .free_city_pressure
                    .iter()
                    .filter(|(owner, _)| {
                        self.players.get(**owner).is_some_and(|player| {
                            player.alive && !player.is_minor && !player.is_barbarian
                        })
                    })
                    .max_by(|(left_owner, left), (right_owner, right)| {
                        left.total_cmp(right)
                            .then_with(|| right_owner.cmp(left_owner))
                    })
                    .map(|(owner, _)| *owner);
                if let Some(new_owner) = new_owner {
                    flips.push(Flip::Join(cid, new_owner));
                }
            } else {
                flips.push(Flip::Revolt(cid));
            }
        }

        for flip in flips {
            match flip {
                Flip::Revolt(cid) => {
                    let old_owner = self.cities[&cid].owner;
                    let free_cities = self.ensure_free_city_player();
                    // A Free City inherits enough civic/technical knowledge
                    // to retain the infrastructure present when it revolts.
                    let techs = self.players[old_owner].techs.clone();
                    let civics = self.players[old_owner].civics.clone();
                    self.players[free_cities].techs.extend(techs);
                    self.players[free_cities].civics.extend(civics);
                    let position = self.cities[&cid].pos;
                    self.transfer_city(cid, free_cities, false);
                    let city = self.cities.get_mut(&cid).unwrap();
                    city.loyalty = 100.0;
                    city.captured_from = None;
                    city.occupied_from = None;
                    city.free_city_pressure.clear();
                    self.reveal(free_cities, position, 3);
                    if let Some(defender) = self.free_city_defender(free_cities) {
                        for _ in 0..2 {
                            self.place_city_state_starting_unit(&defender, free_cities, position);
                        }
                    }
                }
                Flip::Join(cid, new_owner) => {
                    self.transfer_city(cid, new_owner, false);
                    self.add_historic_moment(
                        new_owner,
                        "MOMENT_CITY_TRANSFERRED_DISLOYAL_FREE_CITY",
                    );
                    let city = self.cities.get_mut(&cid).unwrap();
                    city.loyalty = 100.0;
                    city.captured_from = None;
                    city.occupied_from = None;
                    city.free_city_pressure.clear();
                }
            }
        }
    }

    // -------------------------------------------------- world congress
}
