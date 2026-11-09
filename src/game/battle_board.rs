//! What the battle solver reads from the rules, said once.
//!
//! The solver (`ai/advanced/battle_solver`) plays fights forward on a compact
//! model thousands of times a turn, so it cannot ask `Game` per blow. It asks
//! here, once per snapshot, for the part of each exchange that does not change
//! as the fight moves. What does change — where the units stand, flanks,
//! support, fortification, wounds — the model adds back the way the engine
//! does. `ai/advanced/battle_solver/extract_tests.rs` holds the sum to
//! `melee_exchange_strengths` and `ranged_strike_strengths` on real positions.

use super::*;

impl Game {
    /// `melee_exchange_strengths`' two bases, before flanks, support, ground,
    /// river, fortification and wounds.
    pub(crate) fn melee_pair_bases(&self, uid: u32, did: u32) -> Option<(f64, f64)> {
        let attacker = self.units.get(&uid)?;
        let defender = self.units.get(&did)?;
        Some((
            self.unit_unembarked_strength(attacker)
                + self.matchup_bonus(uid, defender, true)
                + self.vs_bonus(attacker.owner, defender.owner),
            self.unit_strength(defender, true) - 3.0 * defender.fortify_turns.clamp(0, 2) as f64
                + self.matchup_bonus(did, attacker, false)
                + self.vs_bonus(defender.owner, attacker.owner),
        ))
    }

    /// `ranged_strike_strengths`' two bases, before ground, fortification and
    /// wounds.
    pub(crate) fn ranged_pair_bases(&self, uid: u32, did: u32) -> Option<(f64, f64)> {
        let shooter = self.units.get(&uid)?;
        let defender = self.units.get(&did)?;
        let spec = &self.rules.units[shooter.kind];
        let defender_is_sea = self.rules.units[defender.kind].domain.as_deref() == Some("sea");
        let mut att = self.unit_ranged_attack_strength(shooter)
            + self.matchup_bonus(uid, defender, true)
            + if defender_is_sea {
                self.promotion_effect(shooter, "ranged_vs_units")
                    + self.promotion_effect(shooter, "ranged_vs_naval")
                    + self.promotion_effect(shooter, "siege_vs_naval")
            } else {
                self.promotion_effect(shooter, "ranged_vs_land")
                    + self.promotion_effect(shooter, "ranged_vs_units")
                    + self.promotion_effect(shooter, "siege_vs_land")
            }
            + self.vs_bonus(shooter.owner, defender.owner);
        if (spec.bombard_strength > 0.0 && !defender_is_sea)
            || (spec.ranged_strength > 0.0
                && spec.domain.as_deref() != Some("sea")
                && defender_is_sea)
        {
            att -= 17.0;
        }
        let def = self.unit_strength(defender, true)
            - 3.0 * defender.fortify_turns.clamp(0, 2) as f64
            + self.visibility_combat_bonus(defender.owner, shooter.owner)
            + self.ranged_defense_bonus(defender, false)
            + self.vs_bonus(defender.owner, shooter.owner);
        Some((att, def))
    }

    /// A unit's strength against a city: `(melee, ranged)` before its wounds.
    pub(crate) fn city_pair_bases(&self, uid: u32, cid: u32) -> Option<(f64, f64)> {
        let unit = self.units.get(&uid)?;
        let city = self.cities.get(&cid)?;
        let spec = &self.rules.units[unit.kind];
        let melee = self.unit_unembarked_strength(unit) + self.vs_bonus(unit.owner, city.owner);
        let mut ranged = self.unit_ranged_attack_strength(unit)
            + self.promotion_effect(unit, "ranged_vs_district")
            + self.gdr_siege_bonus(unit)
            + self.vs_bonus(unit.owner, city.owner);
        if spec.ranged_strength > 0.0 && spec.domain.as_deref() != Some("sea") {
            ranged -= 17.0;
        }
        Some((melee, ranged))
    }

    /// The movement a unit pays to walk `from` → `to`, or `None` where its
    /// domain cannot go.
    pub(crate) fn battle_step_cost(&self, uid: u32, from: Pos, to: Pos) -> Option<f64> {
        (self.unit_can_traverse(uid, to) && self.map.get(to).is_some())
            .then(|| self.unit_step_cost(uid, from, to))
    }

    pub(crate) fn battle_crosses_river(&self, from: Pos, to: Pos) -> bool {
        self.crosses_river(from, to)
    }

    pub(crate) fn battle_flanks_unlocked(&self, owner: usize) -> bool {
        self.flanking_support_unlocked(owner)
    }

    pub(crate) fn battle_line_of_sight(&self, from: Pos, to: Pos) -> bool {
        self.has_line_of_sight(from, to, false)
    }

    pub(crate) fn battle_max_attacks(&self, uid: u32) -> i32 {
        self.unit_max_attacks(uid)
    }
}
