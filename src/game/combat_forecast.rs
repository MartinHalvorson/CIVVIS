//! What a blow would do, before it is struck.
//!
//! Every attack in a 4X is a decision about two numbers, and until this the
//! player had to make it blind: the strengths existed only inside the resolver.
//! This reads the same exchange the resolver will use
//! (`melee_exchange_strengths`, `ranged_strike_strengths`,
//! `city_melee_exchange_strengths`, `ranged_city_strengths`) and centres it the
//! same way (`expected_damage`), so a forecast cannot drift from the blow.
//!
//! It answers only for a strike `legal_actions` offers the seat right now. That
//! is what keeps it from reading a unit through the fog: the enumeration has
//! already asked about war, range, line of sight and visibility. Encampments
//! answer nothing rather than something approximate.

use super::*;

/// One blow, as the player should expect it. The roll spreads the centre by
/// a fifth either way (`damage`), which is what `low`/`high` report.
#[derive(Clone, Debug, PartialEq)]
pub struct CombatForecast {
    pub ranged: bool,
    pub attacker_strength: f64,
    pub defender_strength: f64,
    /// The unit type or city name standing in the way.
    pub defender_name: String,
    pub defender_is_city: bool,
    pub defender_hp: i32,
    /// Outer defenses still standing, for a city. They take the blow first.
    pub defender_walls: i32,
    pub damage_dealt: f64,
    /// Zero for a shot: nothing answers one.
    pub damage_taken: f64,
}

impl CombatForecast {
    /// The lowest and highest a centre can roll, as `damage` rounds them.
    pub fn roll_range(mean: f64) -> (i32, i32) {
        if mean <= 0.0 {
            return (0, 0);
        }
        (
            ((mean * 0.8).round() as i32).clamp(1, 100),
            ((mean * 1.2).round() as i32).clamp(1, 100),
        )
    }
}

impl Game {
    pub fn combat_forecast(
        &self,
        pid: usize,
        uid: u32,
        target: Pos,
        ranged: bool,
    ) -> Option<CombatForecast> {
        let offered = self.legal_actions(pid).iter().any(|action| match action {
            Action::Attack { unit, target: to } => !ranged && *unit == uid && *to == target,
            Action::Ranged { unit, target: to } => ranged && *unit == uid && *to == target,
            _ => false,
        });
        if !offered || self.defending_district_at(target).is_some() {
            return None;
        }
        // A garrison cannot be targeted: the blow hits the city.
        let city = self.city_at(target).filter(|cid| {
            let owner = self.cities[cid].owner;
            owner != pid && self.is_at_war(pid, owner)
        });
        if let Some(cid) = city {
            return self.city_forecast(uid, cid, ranged);
        }
        let reading = |unit: &Unit| effective_strength(self.unit_strength(unit, true), unit.hp);
        // The strongest defender steps up, exactly as `do_attack` picks one.
        let defender = self
            .unit_ids_at(target)
            .iter()
            .filter_map(|id| self.units.get(id))
            .filter(|unit| {
                let spec = &self.rules.units[unit.kind];
                unit.owner != pid
                    && self.is_at_war(pid, unit.owner)
                    && spec.class == "military"
                    && spec.domain.as_deref() != Some("air")
                    && (!ranged || self.unit_currently_visible_to(unit.id, pid))
            })
            .max_by(|a, b| reading(a).total_cmp(&reading(b)))?;
        let (attacker_strength, defender_strength) = if ranged {
            self.ranged_strike_strengths(uid, defender.id, target)?
        } else {
            self.melee_exchange_strengths(uid, defender.id)?
        };
        Some(CombatForecast {
            ranged,
            attacker_strength,
            defender_strength,
            defender_name: defender.kind.to_string(),
            defender_is_city: false,
            defender_hp: defender.hp,
            defender_walls: 0,
            damage_dealt: expected_damage(attacker_strength, defender_strength),
            damage_taken: if ranged {
                0.0
            } else {
                expected_damage(defender_strength, attacker_strength)
            },
        })
    }

    fn city_forecast(&self, uid: u32, cid: u32, ranged: bool) -> Option<CombatForecast> {
        let city = self.cities.get(&cid)?;
        let (attacker_strength, defender_strength) = if ranged {
            self.ranged_city_strengths(uid, cid)?
        } else {
            self.city_melee_exchange_strengths(uid, cid)?
        };
        Some(CombatForecast {
            ranged,
            attacker_strength,
            defender_strength,
            defender_name: city.name.to_string(),
            defender_is_city: true,
            defender_hp: city.hp,
            defender_walls: city.wall_hp,
            damage_dealt: expected_damage(attacker_strength, defender_strength),
            damage_taken: if ranged {
                0.0
            } else {
                expected_damage(defender_strength, attacker_strength)
            },
        })
    }
}
