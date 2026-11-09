//! Native Sparta (033727Z/t75) held 200 garrison HP but only 90/100 wall
//! HP while enemy Crossbowmen stood beside it. The hull-only local-defense
//! check left its Campus running; a single synthetic hull hit reached the
//! existing Pikeman response. Damaged walls with a nearby armed enemy need
//! that same response before the city's inner health starts falling.

use super::*;

impl BasicAi {
    /// Old wall damage alone does not hold the economy: an armed hostile
    /// must still stand within its normal attack distance of this city.
    pub(super) fn walls_need_local_defense(g: &Game, pid: usize, cid: u32) -> bool {
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        let max_wall = g.city_max_wall_hp(city);
        if max_wall <= 0 || city.wall_hp >= max_wall {
            return false;
        }
        g.units.values().any(|unit| {
            let spec = &g.rules.units[unit.kind];
            let armed =
                spec.class == "military" && (spec.is_melee_capable() || spec.has_ranged_attack());
            let range = if spec.has_ranged_attack() {
                spec.range.max(1)
            } else {
                1
            };
            unit.owner != pid
                && unit.hp > 0
                && g.is_at_war(pid, unit.owner)
                && armed
                && g.wdist(unit.pos, city.pos) <= range
        })
    }
}

#[cfg(test)]
mod tests;
