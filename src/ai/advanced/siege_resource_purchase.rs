//! Connect resources for a Domination army and a sustainable Bomber wing before surplus shopping.

use super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::{Action, Game, Item};
use crate::name::Name;
use std::collections::BTreeSet;

impl AdvancedAi {
    pub(super) fn siege_resource_purchase(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || plan.threatened_city.is_some()
            || self.threatened_city(g, pid).is_some()
        {
            return false;
        }
        // Buy only what an existing unit can use through a researched upgrade.
        // Melee units need their first resource while the Domination empire
        // is still expanding: waiting for Conquest leaves its capture army
        // obsolete before it can assemble. Siege keeps its campaign gate.
        let mut needed: BTreeSet<Name> = g
            .units
            .values()
            .filter(|u| u.owner == pid)
            .filter_map(|u| {
                if !matches!(
                    plan.strategy,
                    GrandStrategy::Conquest | GrandStrategy::Expansion
                ) {
                    return None;
                }
                let held = &g.rules.units[u.kind];
                if held.promotion_class != "melee"
                    && (held.promotion_class != "siege" || plan.strategy != GrandStrategy::Conquest)
                {
                    return None;
                }
                let next = &g.rules.units[g.player_unit_replacement(pid, held.upgrade_to?)];
                if !next.buildable
                    || next
                        .tech
                        .is_some_and(|t| !g.players[pid].techs.contains(&t))
                {
                    return None;
                }
                let resource = next.requires_resource?;
                (g.resource_visible_to(pid, resource.as_str())
                    && g.strategic_stockpile(pid, resource) <= 0.0
                    && g.strategic_resource_rate(pid, resource.as_str()) <= 0.0)
                    .then_some(resource)
            })
            .collect();
        // Radio reveals Aluminum before Advanced Flight. A committed airfield
        // and an active beeline let a Builder connect it during that window.
        // After the breakthrough, keep supplying the wing even when the
        // appointment changes. A stockpile does not replace sustainable income.
        let mut air_resource = None;
        if let Some(bomber) = Self::air_surge_bomber(g, pid) {
            let spec = &g.rules.units[bomber];
            if let (Some(tech), Some(field), Some(resource)) =
                (spec.tech, spec.requires_district, spec.requires_resource)
            {
                let has_field = g.cities.values().filter(|c| c.owner == pid).any(|c| {
                    c.districts.iter().any(|(district, pos)| {
                        g.district_family(*district) == g.district_family(field)
                            && g.map.get(*pos).is_some_and(|tile| !tile.pillaged)
                    })
                });
                let preparing = self.air_surge_enabled()
                    && (self.air_surge_active()
                        || self.air_surge_research_goal(g, pid)
                            == Some(super::air_surge::AIR_SURGE_GOAL_TECH));
                let field_committed = has_field
                    || g.cities.values().filter(|c| c.owner == pid).any(|c| {
                        c.queue.iter().any(|item| {
                            matches!(item,
                            Item::District { district, .. }
                            if g.district_family(*district) == g.district_family(field))
                        })
                    });
                if ((g.players[pid].techs.contains(&tech) && has_field)
                    || (preparing && field_committed))
                    && g.resource_visible_to(pid, resource.as_str())
                {
                    let mut bombers = 0usize;
                    let mut demand = 0.0;
                    for unit in g.units.values().filter(|u| u.owner == pid) {
                        let held = &g.rules.units[unit.kind];
                        bombers += usize::from(held.promotion_class == "air_bomber");
                        if !unit.free_upkeep && held.requires_resource == Some(resource) {
                            demand += held.resource_maintenance;
                        }
                    }
                    for city in g.cities.values().filter(|c| c.owner == pid) {
                        for item in &city.queue {
                            if let Item::Unit { unit } | Item::Formation { unit, .. } = item {
                                let queued = &g.rules.units[*unit];
                                bombers += usize::from(queued.promotion_class == "air_bomber");
                                if queued.requires_resource == Some(resource) {
                                    demand += queued.resource_maintenance;
                                }
                            }
                        }
                    }
                    demand += super::air_surge::AIR_SURGE_LAUNCH_BOMBERS.saturating_sub(bombers)
                        as f64
                        * spec.resource_maintenance;
                    if g.strategic_resource_rate(pid, resource.as_str()) + f64::EPSILON < demand {
                        needed.insert(resource);
                        air_resource = Some(resource);
                    }
                }
            }
        }
        if needed.is_empty() {
            return false;
        }
        // Keep a small cash buffer and six turns of any current deficit.
        // Emergency defence already runs before this pass, and a threatened
        // home city vetoes the exception entirely.
        let floor = 40.0 + 6.0 * (-g.players[pid].gold_per_turn).max(0.0);
        let mut options = g
            .legal_purchase_actions(pid)
            .into_iter()
            .filter_map(|action| {
                let Action::BuyPlot { pos, cost, .. } = &action else {
                    return None;
                };
                let resource = g.map.get(*pos)?.resource?;
                (needed.contains(&resource) && g.players[pid].gold >= cost + floor)
                    .then_some((*cost, *pos, resource, action))
            })
            .collect::<Vec<_>>();
        options.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        for (_, pos, resource, action) in options {
            let connects = |board: &Game, at| {
                board.valid_improvements(pid, at).iter().any(|name| {
                    let spec = &board.rules.improvements[name];
                    spec.builder_buildable
                        && (spec.resources.contains(&resource)
                            || board.rules.resources[resource].improvement == *name)
                })
            };
            // An unconnected deposit already owned needs a Builder first.
            // A healthy mine may be insufficient for the growing air wing;
            // only its repair or improvement backlog defers another purchase.
            if g.cities.values().filter(|c| c.owner == pid).any(|c| {
                c.owned_tiles.iter().any(|p| {
                    g.map.get(*p).is_some_and(|t| {
                        t.resource == Some(resource)
                            && !t.flooded
                            && (air_resource != Some(resource)
                                || t.pillaged
                                || (*p != c.pos
                                    && !t.improvement.is_some_and(|name| {
                                        let spec = &g.rules.improvements[name];
                                        spec.resources.contains(&resource)
                                            || g.rules.resources[resource].improvement == name
                                    })))
                            && (connects(g, *p)
                                || t.improvement.is_some_and(|name| {
                                    let spec = &g.rules.improvements[name];
                                    spec.resources.contains(&resource)
                                        || g.rules.resources[resource].improvement == name
                                }))
                    })
                })
            }) {
                continue;
            }
            let mut after = g.speculative_clone();
            if after.apply(pid, &action).is_err() || !connects(&after, pos) {
                continue;
            }
            let visible = after.player_vision_frame(pid);
            let can_connect = after.units.values().any(|u| {
                u.owner == pid
                    && u.kind == "builder"
                    && u.charges > 0
                    && after.wdist(u.pos, pos) <= 4
                    && after
                        .route_distance(u.id, pos, 0)
                        .is_some_and(|steps| steps <= 4)
                    && self.settlement_tile_risk(&after, pid, Some(u.id), pos, &visible)
                        < super::SETTLER_STEP_RISK_LIMIT
            });
            if can_connect && g.apply(pid, &action).is_ok() {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests;
