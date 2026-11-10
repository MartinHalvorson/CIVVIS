//! Embarkation: whether a unit is embarked, can embark, and what it fights
//! with when it is.
//!
//! `is_embarked`, `unit_is_embarked_at`, `has_embarkation`,
//! `embarkation_facility_at` and `unit_unembarked_strength`.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    /// Flat Combat Strength this seat receives from the difficulty setting.
    pub fn handicap_combat_strength(&self, pid: usize) -> f64 {
        if !self.takes_handicap(pid) {
            return 0.0;
        }
        let spec = self.difficulty_spec();
        if self.is_human_seat(pid) {
            spec.human_combat_strength
        } else {
            spec.ai_combat_strength
        }
    }

    pub(super) fn city_state_envoy_strength(&self, minor: usize) -> f64 {
        self.players
            .get(minor)
            .filter(|state| state.alive && state.is_minor && !state.is_barbarian)
            .and_then(|_| self.suzerain_of(minor))
            .map(|suzerain| self.envoys_at(suzerain, minor) as f64)
            .unwrap_or(0.0)
    }

    pub(super) fn unit_is_embarked_at(&self, unit: &Unit, pos: Pos) -> bool {
        // The host's flag is the fact while the unit stands where it was read; the
        // tile rule below is the model, and the two disagree exactly when the
        // mirrored tile is behind the unit (the tile export runs every fourth turn).
        if let Some((observed_at, embarked)) = unit.host_embarked {
            if observed_at == pos {
                return embarked;
            }
        }
        let domain = self.rules.units[unit.kind].domain.as_deref();
        !matches!(domain, Some("sea" | "air"))
            && unit.kind != "giant_death_robot"
            && self.map.get(pos).is_some_and(|tile| {
                self.rules.is_water(tile) && tile.wonder.as_deref() != Some("golden_gate_bridge")
            })
    }

    pub fn is_embarked(&self, unit: &Unit) -> bool {
        self.unit_is_embarked_at(unit, unit.pos)
    }

    /// Whether this unit type has learned how to embark onto Coast tiles.
    /// Sailing unlocks Builders, Celestial Navigation unlocks Traders, and
    /// Shipbuilding unlocks every remaining land unit (Gathering Storm).
    /// Māori Mana starts with the complete embarkation package even though
    /// this compact ruleset does not inject its two free technologies into the
    /// player's research ledger.
    pub(super) fn has_embarkation(&self, owner: usize, kind: &str) -> bool {
        if self.has_ability(owner, "mana") {
            return true;
        }
        match kind {
            "builder" => self.tree_effect(owner, "builder_embark") > 0.0,
            "trader" => self.tree_effect(owner, "trader_embark") > 0.0,
            _ => self.tree_effect(owner, "land_unit_embark") > 0.0,
        }
    }

    /// Expansion1_Leaders.xml: TRAIT_COMBAT_BONUS_FOR_CAVALRY grants
    /// GENGHIS_KHAN_CAVALRY_BONUS (Amount=3) to light/heavy cavalry or the
    /// CLASS_MONGOLIAN_KESHIG tag. Other ranged cavalry do not qualify.
    pub(super) fn mongolian_cavalry_strength(&self, u: &Unit) -> f64 {
        if !self.owns_civ_unique(u.owner, "Mongolia")
            || self
                .observed_leader_types
                .get(&u.owner)
                .is_some_and(|leader| leader != "LEADER_GENGHIS_KHAN")
        {
            return 0.0;
        }
        // Without a host leader reading, the rules roster's Mongolian leader
        // is Genghis Khan. A host reading overrides that default on clones too.
        let spec = &self.rules.units[u.kind];
        if matches!(
            spec.promotion_class.as_str(),
            "light_cavalry" | "heavy_cavalry"
        ) || u.kind == "keshig"
        {
            3.0
        } else {
            0.0
        }
    }

    pub(super) fn unit_unembarked_strength(&self, u: &Unit) -> f64 {
        let mut s = self.rules.units[u.kind].strength.max(1.0)
            + self.government_combat_bonus(u, true)
            + self.unit_formation_bonus(u)
            + self.promotion_effect(u, "combat_all")
            + self.congress_military_strength_bonus(u)
            + self.religious_combat_belief_bonus(u)
            + self.handicap_combat_strength(u.owner);
        if !matches!(
            self.rules.units[u.kind].domain.as_deref(),
            Some("sea" | "air")
        ) {
            s += self.adjacent_friendly_unit_effect(u, "adjacent_combat_strength");
        }
        s += self.taxis_holy_city_strength(u.owner);
        if self.has_ability(u.owner, "gifts_for_the_tlatoani") {
            s += self.empire_luxuries(u.owner) as f64; // Montezuma
        }
        s += self.civ_effect(u.owner, "combat_strength");
        s += self.mongolian_cavalry_strength(u);
        if u.kind == "nihang" {
            let encampment_buildings = ["barracks", "armory", "military_academy"]
                .iter()
                .filter(|family| {
                    self.cities
                        .values()
                        .filter(|city| city.owner == u.owner)
                        .any(|city| self.city_has_building_family(city, Name::new(family)))
                })
                .count() as f64;
            s += 15.0 * encampment_buildings;
            if self.promotion_effect(u, "lahore_suzerain_combat") > 0.0
                && self.grants_city_state_unique_bonus(u.owner, "Lahore")
            {
                s += self.promotion_effect(u, "lahore_suzerain_combat");
            }
        }
        // Foreign Ministry: "City State units gain +4 Combat Strength if you
        // are the City State's Suzerain (including Leveraged Units)". Two rows,
        // `BUILDING_GOV_CITYSTATES_UNIT_STRENGTH` for the units a city-state
        // still holds and `_LEVERAGED_UNIT_STRENGTH` for a levied one, whose
        // `owner` is the Suzerain while `levied_from` names the city-state.
        let city_state_patron = if u.levied_from.is_some() {
            Some(u.owner)
        } else if self.players[u.owner].is_minor {
            self.suzerain_of(u.owner)
        } else {
            None
        };
        if let Some(patron) = city_state_patron {
            s += self.empire_building_sum(patron, |building| {
                building
                    .effects
                    .get("levied_unit_combat_strength")
                    .copied()
                    .unwrap_or(0.0)
            });
        }
        let city_state_origin = u
            .levied_from
            .or_else(|| self.players[u.owner].is_minor.then_some(u.owner));
        if let Some(minor) = city_state_origin {
            s += self.city_state_envoy_strength(minor);
        }
        let spec = &self.rules.units[u.kind];
        if spec.resource_maintenance > 0.0 {
            if let Some(resource) = spec.requires_resource.as_deref() {
                s -= self.strategic_shortage_penalty(u.owner, Name::new(resource)) as f64;
            }
        }
        let friendly = self.wdisk(u.pos, 1).into_iter().any(|position| {
            self.map
                .get(position)
                .and_then(|tile| tile.owner_city)
                .and_then(|cid| self.cities.get(&cid))
                .is_some_and(|city| city.owner == u.owner)
        });
        if friendly {
            s += self.policy_effect(u.owner, "friendly_combat");
        }
        let wounded_penalty = (10.0 - u.hp.clamp(0, 100) as f64 / 10.0).round();
        if self.promotion_effect(u, "no_wounded_penalty") > 0.0 {
            s += wounded_penalty;
        }
        s += wounded_penalty * self.policy_effect(u.owner, "wounded_penalty_reduction_pct") / 100.0;
        s
    }

    fn strategic_shortage_penalty(&self, pid: usize, resource: impl AsName) -> i32 {
        self.players[pid]
            .strategic_resource_shortages
            .get(&resource.as_name())
            .copied()
            .unwrap_or(0)
            .clamp(0, 20)
    }

    /// Harbors and coastal City Centers remove the embark/disembark
    /// surcharge. A pillaged Harbor no longer provides that service.
    pub(super) fn embarkation_facility_at(&self, pos: Pos) -> bool {
        if self.city_at(pos).is_some() {
            return self.nbrs(pos).into_iter().any(|neighbor| {
                self.map
                    .get(neighbor)
                    .is_some_and(|tile| self.rules.is_water(tile))
            });
        }
        let Some(tile) = self.map.get(pos) else {
            return false;
        };
        !tile.pillaged
            && tile
                .district
                .is_some_and(|district| self.district_is_family(district, crate::name!("harbor")))
    }
}
