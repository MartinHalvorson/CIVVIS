//! Spread early recon across known land frontiers to seek the first major.
//! No hidden city positions or terrain are read to choose a frontier.
//! Production retains v1: the initial extra-Scout experiment made more map
//! visible but did not improve contact timing (see the opening diagnostic).

use super::AdvancedAi;
use crate::game::Game;
use crate::{think, Pos};

impl AdvancedAi {
    fn neighbor_contact_targets(&self, g: &Game, pid: usize, uid: u32) -> Vec<Pos> {
        if !self.early_contact_window_2
            || g.is_arena()
            || self.base.minor
            || self.base.barb
            || g.turn > g.standard_duration(60)
            || g.tree_effect(pid, "open_borders") > 0.0
            || self.settler_guards.values().any(|guard| *guard == uid)
        {
            return Vec::new();
        }
        let Some(unit) = g.units.get(&uid) else {
            return Vec::new();
        };
        if unit.owner != pid
            || unit.hp < 70
            || g.is_embarked(unit)
            || g.rules.units[unit.kind].promotion_class != "recon"
        {
            return Vec::new();
        }
        let rivals: Vec<_> = g
            .players
            .iter()
            .filter(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && !other.is_free_city
                    && !g.same_team(pid, other.id)
            })
            .collect();
        if rivals.is_empty() || rivals.iter().any(|other| g.has_met(pid, other.id)) {
            return Vec::new();
        }
        let Some(home) = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .min_by_key(|pos| g.wdist(*pos, unit.pos))
        else {
            return Vec::new();
        };
        let scouts: Vec<_> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|id| {
                g.rules.units[g.units[id].kind].promotion_class == "recon"
                    && !g.is_embarked(&g.units[id])
            })
            .collect();
        let rank = scouts.iter().position(|id| *id == uid).unwrap_or(0);
        let directions: Vec<_> = g.nbrs(home).into_iter().collect();
        if directions.is_empty() {
            return Vec::new();
        }
        let preferred = rank * directions.len() / scouts.len().max(1);
        let explored = &g.players[pid].explored;
        let mut candidates: Vec<_> = explored
            .iter()
            .copied()
            .filter_map(|pos| {
                let distance = g.wdist(unit.pos, pos);
                if distance == 0
                    || distance > 6
                    || g.wdist(home, pos) > 18
                    || !g
                        .map
                        .get(pos)
                        .is_some_and(|tile| !g.rules.is_water(tile) && g.rules.is_passable(tile))
                    || !g
                        .nbrs(pos)
                        .into_iter()
                        .any(|next| !explored.contains(&next))
                {
                    return None;
                }
                let unseen = g
                    .wdisk(pos, 2)
                    .into_iter()
                    .filter(|p| !explored.contains(p))
                    .count() as i32;
                let sector = (0..directions.len())
                    .min_by_key(|i| (g.wdist(directions[*i], pos), *i))
                    .unwrap();
                let gap = sector.abs_diff(preferred);
                let sector_gap = if scouts.len() > 1 {
                    gap.min(directions.len() - gap) as i32
                } else {
                    0
                };
                let crowd = scouts
                    .iter()
                    .filter(|id| **id != uid)
                    .map(|id| (4 - g.wdist(g.units[id].pos, pos)).max(0))
                    .sum::<i32>();
                let score =
                    5 * unseen + g.wdist(home, pos) - 3 * distance - 4 * sector_gap - 4 * crowd;
                Some((score, pos))
            })
            .collect();
        candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        candidates
            .into_iter()
            .take(12)
            .map(|(_, pos)| pos)
            .collect()
    }

    pub(super) fn neighbor_contact_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let targets = self.neighbor_contact_targets(g, pid, uid);
        if targets.is_empty() {
            return false;
        }
        let visible = self.battlefront_visibility(g, pid);
        let dangers: Vec<_> = g
            .units
            .values()
            .filter(|unit| {
                unit.owner != pid
                    && g.is_at_war(pid, unit.owner)
                    && g.rules.units[unit.kind].class == "military"
                    && g.sees(&visible, unit.pos)
                    && self.battlefront_unit_visible(g, pid, unit.id)
            })
            .map(|unit| (unit.pos, g.unit_attack_range(unit.id).max(1) + 1))
            .collect();
        for target in targets {
            if dangers
                .iter()
                .any(|(pos, radius)| g.wdist(*pos, target) <= *radius)
            {
                continue;
            }
            let Some(next) = g
                .route_step(uid, target, 0)
                .filter(|next| g.can_move(uid, *next))
            else {
                continue;
            };
            if dangers
                .iter()
                .any(|(pos, radius)| g.wdist(*pos, next) <= *radius)
            {
                continue;
            }
            if self.base.path_move(g, pid, uid, next) {
                think!(self.journal(), Military, Detail,
                       "Scout {uid} searches the frontier at {target:?}";
                       "no rival major has been met; the route spreads recon across nearby land frontiers";
                       target);
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests;
