//! An opt-in research alarm for the army that can fight field units now.
//!
//! Indonesia061004Z researched toward Machinery through Engineering, then
//! left that goal. At turn70 its Horseman had11HP, while nearby barbarians
//! included Men-at-Arms and Crossbowmen. The old quality comparison counts
//! that Horseman at its full36 strength, and a Catapult at35 bombard strength.
//! This is a mechanism witness, not evidence a different research order wins.
//!
//! The new alarm keeps the old radius, visibility and quality threshold, but
//! applies the engine's wound penalty and its bombard penalty against land
//! units. With battlefront observation, the Science urgency gate requires a
//! visible raider's actual City-Center attack reach, rather than also demanding
//! 1.25 times the city's own strength: that second comparison otherwise masks
//! the field gap. Priority ordering, upgrade paths, resource requirements,
//! production and Gold reserves retain their own rules. No new sticky goal.

use super::*;
use crate::ai::{BARBARIAN_MILITARY_GAP_MARGIN, BARBARIAN_MILITARY_GAP_RATIO, HOME_THREAT_RADIUS};
use crate::game::{effective_strength, Unit};

impl AdvancedAi {
    pub(super) fn barbarian_field_attack_is_imminent(g: &Game, pid: usize) -> bool {
        let Some(barb) = g.barb_pid else { return false };
        let cities = g.player_city_ids(pid);
        g.units.values().any(|unit| {
            unit.owner == barb
                && BasicAi::is_barbarian_raider(g, unit)
                && g.player_can_see(pid, unit.pos)
                && g.unit_visible_to(unit.id, pid)
                && cities.iter().any(|cid| {
                    g.wdist(g.cities[cid].pos, unit.pos) <= HOME_THREAT_RADIUS
                        && g.attack_reach(unit.id).contains(&g.cities[cid].pos)
                })
        })
    }

    fn barbarian_field_power(g: &Game, unit: &Unit) -> f64 {
        let spec = &g.rules.units[unit.kind];
        // Same land-target bombard penalty as Game::ranged_strike_strengths.
        // This remains a tier alarm, not a terrain/promotion matchup forecast.
        let ranged = spec.ranged_strength.max(spec.bombard_strength - 17.0);
        effective_strength(spec.strength.max(ranged), unit.hp)
    }

    fn barbarian_field_quality_gap(g: &Game, pid: usize) -> bool {
        let Some(barb) = g.barb_pid else { return false };
        let Some(player) = g.players.get(pid) else {
            return false;
        };
        if player.is_minor || player.is_barbarian || barb == pid {
            return false;
        }
        let cities = g.player_city_ids(pid);
        let strongest_raider = g
            .units
            .values()
            .filter(|unit| {
                unit.owner == barb
                    && BasicAi::is_barbarian_raider(g, unit)
                    && g.player_can_see(pid, unit.pos)
                    && cities
                        .iter()
                        .any(|cid| g.wdist(g.cities[cid].pos, unit.pos) <= HOME_THREAT_RADIUS)
            })
            .map(|unit| Self::barbarian_field_power(g, unit))
            .fold(0.0, f64::max);
        let strongest_own = g
            .units
            .values()
            .filter(|unit| {
                let spec = &g.rules.units[unit.kind];
                unit.owner == pid
                    && spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
            })
            .map(|unit| Self::barbarian_field_power(g, unit))
            .fold(0.0, f64::max);
        strongest_raider > 0.0
            && strongest_raider
                > strongest_own * BARBARIAN_MILITARY_GAP_RATIO + BARBARIAN_MILITARY_GAP_MARGIN
    }

    pub(super) fn barbarian_research_upgrade_goal(&self, g: &Game, pid: usize) -> Option<String> {
        if !self.barbarian_research_prices_the_field {
            return BasicAi::barbarian_military_research_goal(g, pid);
        }
        Self::barbarian_field_quality_gap(g, pid)
            .then(|| BasicAi::barbarian_military_upgrade_goal(g, pid))
            .flatten()
    }
}

#[cfg(test)]
mod tests;
