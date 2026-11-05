//! Healing: how fast a unit heals, where, and from what.
//!
//! `unit_heal_rate` (and its `_at`/`_for` forms), healing locations and the
//! stamp that caches them, and Holy Waters' heal.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    /// Classify a tile from a unit owner's perspective for passive healing.
    /// Districts use the district rate, while any foreign civilization's
    /// territory is rival territory even when Open Borders or peace applies.
    pub fn healing_location(&self, owner: usize, pos: Pos) -> HealingLocation {
        let tile = self.map.get(pos);
        let territory_owner = tile
            .and_then(|tile| tile.owner_city)
            .and_then(|cid| self.cities.get(&cid))
            .map(|city| city.owner);
        if self.city_at(pos).is_some() || tile.and_then(|tile| tile.district.as_ref()).is_some() {
            return HealingLocation::District;
        }
        match territory_owner {
            Some(tile_owner) if tile_owner == owner => HealingLocation::FriendlyTerritory,
            Some(tile_owner) if self.suzerain_of(tile_owner) == Some(owner) => {
                HealingLocation::FriendlyTerritory
            }
            Some(_) => HealingLocation::EnemyTerritory,
            None => HealingLocation::NeutralTerritory,
        }
    }

    /// Generations for every mutable input to the cached safe-healing target
    /// scan, plus the small diplomacy state that has no mutation wrapper.
    ///
    /// The caller keeps the three direct generations separate, avoiding a
    /// needless hash collision across them. `at_war`, the arena mode, and the
    /// barbarian seat all affect `is_at_war`; the latter two are static for a
    /// game, but retaining them makes the cache's complete input explicit.
    pub(crate) fn healing_target_stamp(&self) -> (u64, u64, u64, u64) {
        let mut wars = vision_key(&[self.at_war.len() as u64]);
        for &(a, b) in &self.at_war {
            wars = vision_key(&[wars, a as u64, b as u64]);
        }
        (
            self.map.tiles.epoch(),
            self.cities.generation(),
            self.players.epoch(),
            vision_key(&[
                wars,
                self.barb_pid.map_or(u64::MAX, |pid| pid as u64),
                self.is_arena() as u64,
            ]),
        )
    }

    /// What a unit standing in nuclear fallout loses at the start of its turn.
    ///
    /// The shipped WMDs table carries the blast radius, the ICBM range and how
    /// long the fallout lasts, but not what standing in it costs — the same gap
    /// the per-ring blast damage has. This is the one number here: enough that
    /// holding irradiated ground is a decision rather than an oversight, and
    /// slow enough that a full-health unit can still cross a contaminated
    /// corridor and live.
    pub const FALLOUT_UNIT_DAMAGE: i32 = 20;

    /// Holy Waters: `HOLY_WATERS_HEALING` attaches a +10 healing modifier to
    /// every Holy Site in a city following the religion, and
    /// `MODIFIER_ALL_UNITS_ADJUST_HEAL_RELIGION_PER_TURN` applies it to that
    /// religion's units on the district's own plot or its adjacent ring.
    fn holy_waters_heal(&self, unit: &Unit) -> i32 {
        if self.rules.units[unit.kind].class != "religious" {
            return 0;
        }
        let Some(religion) = unit.religion.as_deref() else {
            return 0;
        };
        let amount = self.religion_belief_effect(religion, "holy_waters_heal");
        if amount == 0.0 {
            return 0;
        }
        let beside_a_qualifying_holy_site = self.wdisk(unit.pos, 1).into_iter().any(|position| {
            self.map.get(position).is_some_and(|tile| {
                tile.district.is_some_and(|district| {
                    self.district_is_family(district, crate::name!("holy_site"))
                }) && tile
                    .owner_city
                    .and_then(|city_id| self.cities.get(&city_id))
                    .is_some_and(|city| self.city_religion(city) == Some(religion))
            })
        });
        if beside_a_qualifying_holy_site {
            amount.round() as i32
        } else {
            0
        }
    }

    pub fn unit_heal_rate(&self, uid: u32) -> i32 {
        self.unit_heal_rate_for(&self.units[&uid])
    }

    /// Evaluate a recovery destination without moving the live unit. The
    /// projected position controls embarkation, territory and nearby support.
    pub(crate) fn unit_heal_rate_at(&self, uid: u32, pos: Pos) -> i32 {
        let mut unit = self.units[&uid].clone();
        unit.pos = pos;
        self.unit_heal_rate_for(&unit)
    }

    fn unit_heal_rate_for(&self, unit: &Unit) -> i32 {
        let spec = &self.rules.units[unit.kind];
        // Barbarian units never recover passively in Civ VI. They can still
        // receive an immediate healing plunder reward, such as from a Farm.
        if self.players[unit.owner].is_barbarian {
            return 0;
        }
        // Nothing recovers on an arena. A Tactics game is one engagement, not
        // a campaign: there is no friendly territory to fall back into and no
        // city to garrison, so every hex heals at the same neutral rate and
        // both sides can always afford to break contact. Measured with
        // healing on, two even armies ground each other down to about half
        // and then stalled for four hundred turns — each side pulling a
        // damaged unit out, standing it still until it was whole again, and
        // walking it back — so the mode's own victory condition, last army
        // standing, was never reached. Permanent damage is also what makes
        // the tactics matter: a trade you win stays won, and focusing fire on
        // one defender is worth doing. A board that asks for a campaign
        // (`TacticsRules::heal`) gets the neutral rate everywhere instead.
        if self.is_arena() && !self.tactics.heal {
            return 0;
        }
        // Nothing recovers in fallout. A blast that only cost yields would make
        // ground zero the safest place on the map to park a wounded army.
        if self.fallout_at(unit.pos) {
            return 0;
        }
        if spec.domain.as_deref() == Some("air") && self.air_capacity_at(unit.owner, unit.pos) <= 0
        {
            return 0;
        }
        // Bound once, below the guards that mean "cannot recover at all" — a
        // barbarian, an arena, fallout, a grounded aircraft — because a
        // healing modifier lifts a rate, it does not create one.
        let holy_site_heal = self.pantheon_holy_site_heal(unit);
        let holy_waters_heal = self.holy_waters_heal(unit);
        if self
            .map
            .get(unit.pos)
            .and_then(|tile| tile.owner_city)
            .and_then(|city_id| self.cities.get(&city_id))
            .is_some_and(|city| {
                city.owner == unit.owner
                    && self.governor_effect(unit.owner, city.id, "unit_heal_full") > 0.0
            })
        {
            return 100;
        }
        if spec.class == "religious" {
            let mut best: f64 = 0.0;
            if self.map.get(unit.pos).is_some_and(|tile| {
                tile.improvement.as_deref() == Some("monastery") && !tile.pillaged
            }) {
                best = best.max(
                    self.rules.improvements["monastery"]
                        .effects
                        .get("religious_unit_heal_rate")
                        .copied()
                        .unwrap_or(0.0),
                );
            }
            for position in self.wdisk(unit.pos, 1) {
                let Some(tile) = self.map.get(position) else {
                    continue;
                };
                let Some(district) = tile.district else {
                    continue;
                };
                if !self.district_is_family(district, crate::name!("holy_site")) {
                    continue;
                }
                let Some(city) = tile.owner_city.and_then(|cid| self.cities.get(&cid)) else {
                    continue;
                };
                if city.owner != unit.owner {
                    continue;
                }
                let mut faith = self
                    .district_yields(Name::new(district.as_str()), position)
                    .faith;
                faith += city
                    .buildings
                    .iter()
                    .filter_map(|name| {
                        let building = &self.rules.buildings[name];
                        building
                            .district
                            .is_some_and(|district| {
                                self.district_is_family(district, crate::name!("holy_site"))
                            })
                            .then_some(building.yields.faith)
                    })
                    .sum::<f64>();
                best = best.max(3.0 * faith);
            }
            let chaplain = self
                .wdisk(unit.pos, 1)
                .into_iter()
                .flat_map(|position| self.unit_ids_at(position).iter().copied())
                .filter(|other| self.units[other].owner == unit.owner)
                .map(|other| self.promotion_effect(&self.units[&other], "adjacent_heal"))
                .fold(0.0, f64::max);
            best = best.max(chaplain);
            return best.round() as i32 + holy_site_heal + holy_waters_heal;
        }
        if spec.domain.as_deref() == Some("sea")
            && self
                .map
                .get(unit.pos)
                .and_then(|tile| tile.owner_city)
                .and_then(|city_id| self.cities.get(&city_id))
                .is_some_and(|city| {
                    city.owner == unit.owner
                        && self.city_district_effect(city, "naval_heal_full") > 0.0
                })
        {
            return 100;
        }
        let emergency_heal = if spec.class == "military" {
            self.map
                .get(unit.pos)
                .and_then(|tile| tile.owner_city)
                .and_then(|city| self.cities.get(&city))
                .and_then(|city| {
                    self.players[unit.owner]
                        .counters
                        .get(&format!("emergency_heal_in:{}", city.owner))
                })
                .copied()
                .unwrap_or(0) as i32
        } else {
            0
        };
        let location = self.healing_location(unit.owner, unit.pos);
        let naval_or_embarked = spec.domain.as_deref() == Some("sea") || self.is_embarked(unit);
        let support_heal = if spec.class == "military" {
            self.adjacent_support_effect(unit, "adjacent_heal").round() as i32
        } else {
            0
        };
        if naval_or_embarked {
            let friendly = self
                .map
                .get(unit.pos)
                .and_then(|tile| tile.owner_city)
                .and_then(|cid| self.cities.get(&cid))
                .is_some_and(|city| {
                    city.owner == unit.owner || self.suzerain_of(city.owner) == Some(unit.owner)
                });
            if friendly {
                20 + emergency_heal + support_heal + holy_site_heal
            } else {
                // Auxiliary Ships, Supply Fleet and Supercarrier do not heal a
                // ship as if it were home: they pay 5 in enemy territory and
                // 10 in neutral.
                let promoted = match location {
                    HealingLocation::EnemyTerritory => {
                        self.promotion_effect(unit, "heal_enemy_territory")
                    }
                    HealingLocation::NeutralTerritory => {
                        self.promotion_effect(unit, "heal_neutral_territory")
                    }
                    _ => 0.0,
                } as i32;
                promoted + emergency_heal + support_heal + holy_site_heal
            }
        } else {
            location.rate() + emergency_heal + support_heal + holy_site_heal
        }
    }
}
