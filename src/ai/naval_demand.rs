//! Fleet demand separates a naval war from a local barbarian defense.
//!
//! A distant barbarian on water does not make every port a war front. Use
//! the existing home-threat radius to count exposed launch sites, with one
//! spare ship. Civilization wars retain their empire-wide fleet budget.

use super::*;

impl BasicAi {
    pub(crate) fn desired_navy(&self, g: &Game, pid: usize) -> usize {
        let coastal_cities: Vec<_> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| self.naval_city_can_launch(g, *cid))
            .collect();
        if coastal_cities.is_empty() || !g.players[pid].techs.contains(&crate::name!("sailing")) {
            return 0;
        }
        let mut desired = 1;
        let settlers_at_sea = g.units.values().any(|unit| {
            unit.owner == pid
                && unit.kind == "settler"
                && g.map
                    .get(unit.pos)
                    .is_some_and(|tile| g.rules.is_water(tile))
        });
        if settlers_at_sea
            || (g.players[pid].techs.contains(&crate::name!("shipbuilding"))
                && g.units
                    .values()
                    .any(|unit| unit.owner == pid && unit.kind == "settler"))
        {
            desired = desired.max(2);
        }
        let naval_war = g.players.iter().any(|enemy| {
            enemy.id != pid
                && enemy.alive
                && !enemy.is_barbarian
                && g.is_at_war(pid, enemy.id)
                && (g.units.values().any(|unit| {
                    unit.owner == enemy.id
                        && g.map
                            .get(unit.pos)
                            .is_some_and(|tile| g.rules.is_water(tile))
                }) || g
                    .player_city_ids(enemy.id)
                    .into_iter()
                    .any(|cid| self.naval_city_can_launch(g, cid)))
        });
        if naval_war {
            desired = desired.max(coastal_cities.len().saturating_add(1).max(2));
        } else {
            if g.players[pid].techs.contains(&crate::name!("cartography"))
                && coastal_cities.len() >= 2
            {
                desired = desired.max(2);
            }
            let raiders: Vec<_> = g
                .units
                .values()
                .filter(|unit| {
                    unit.owner != pid
                        && g.players[unit.owner].alive
                        && g.players[unit.owner].is_barbarian
                        && g.is_at_war(pid, unit.owner)
                        && g.map
                            .get(unit.pos)
                            .is_some_and(|tile| g.rules.is_water(tile))
                })
                .map(|unit| unit.pos)
                .collect();
            let exposed = coastal_cities
                .iter()
                .filter(|cid| {
                    raiders
                        .iter()
                        .any(|pos| g.wdist(g.cities[cid].pos, *pos) <= HOME_THREAT_RADIUS)
                })
                .count();
            if exposed > 0 {
                desired = desired.max(exposed.saturating_add(1).max(2));
            }
        }
        desired
    }
}

#[cfg(test)]
mod tests;
