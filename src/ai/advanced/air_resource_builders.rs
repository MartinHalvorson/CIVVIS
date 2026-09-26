//! Connect nearby strategic deposits for the committed bomber wing.

use super::{AdvancedAi, VictoryTarget};
use crate::game::{Action, Game};

impl AdvancedAi {
    /// A client resource supplies the Suzerain, but ordinary Builder sweeps
    /// enumerate only our own cities. Claim one nearby, safe supply job.
    pub(super) fn air_resource_builder_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.air_surge_enabled()
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || self.threatened_city(g, pid).is_some()
        {
            return None;
        }
        let builder = g.units.get(&uid)?;
        if builder.owner != pid
            || builder.kind != "builder"
            || builder.charges <= 0
            || builder.moves_left <= 0.0
            || builder.linked_to.is_some()
            || self.builder_support.contains_key(&uid)
        {
            return None;
        }
        let current = builder.pos;
        let resource = self.air_resource_shortfall(g, pid)?;
        let improvement = crate::name::Name::new(&g.rules.resources[resource].improvement);
        let visible = g.player_vision_frame(pid);
        let mut jobs = Vec::new();
        for city in g
            .cities
            .values()
            .filter(|city| city.owner == pid || g.suzerain_of(city.owner) == Some(pid))
        {
            for pos in &city.owned_tiles {
                let tile = &g.map.tiles[pos];
                if tile.resource != Some(resource)
                    || tile.flooded
                    || tile.submerged
                    || !g.sees(&visible, *pos)
                    || g.wdist(current, *pos) > 6
                    || self.builder_targets.iter().any(|(other, target)| {
                        *other != uid
                            && target == pos
                            && g.units.get(other).is_some_and(|unit| {
                                unit.owner == pid && unit.kind == "builder" && unit.charges > 0
                            })
                    })
                {
                    continue;
                }
                let repair = tile.pillaged && tile.improvement == Some(improvement);
                if tile.improvement == Some(improvement) && !repair {
                    continue;
                }
                if !repair && !g.valid_improvements(pid, *pos).contains(&improvement) {
                    continue;
                }
                let Some(distance) = g.route_distance(uid, *pos, 0).filter(|steps| *steps <= 6)
                else {
                    continue;
                };
                // No credit for nearby military units: they may move away
                // before the host executes this Builder's request.
                if self.settlement_tile_risk_with_support(g, pid, Some(uid), *pos, &visible, false)
                    > 0.0
                {
                    continue;
                }
                jobs.push((distance, *pos, repair));
            }
        }
        jobs.sort_unstable();
        for (_, pos, repair) in jobs.into_iter().take(6) {
            let acted = if current == pos {
                let action = if repair {
                    Action::RepairImprovement { unit: uid }
                } else {
                    Action::Improve {
                        unit: uid,
                        improvement,
                    }
                };
                g.apply(pid, &action).is_ok()
            } else {
                let Some(next) = g.route_step(uid, pos, 0).filter(|next| {
                    g.can_move(uid, *next)
                        && g.sees(&visible, *next)
                        && self.settlement_tile_risk_with_support(
                            g,
                            pid,
                            Some(uid),
                            *next,
                            &visible,
                            false,
                        ) <= 0.0
                }) else {
                    continue;
                };
                self.base.path_move(g, pid, uid, next)
            };
            if !acted {
                continue;
            }
            if current == pos {
                self.builder_targets.remove(&uid);
            } else {
                self.builder_targets.insert(uid, pos);
            }
            crate::think!(self.journal(), Military, Decision,
                "Builder connects the bomber wing's resource supply";
                "builder {}; {} at {pos:?}; {}",
                uid, resource, if current == pos { "connected" } else { "approaching" }; pos);
            return Some(true);
        }
        None
    }
}

#[cfg(test)]
mod tests;
