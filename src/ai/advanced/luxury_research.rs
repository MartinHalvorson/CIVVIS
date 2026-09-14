//! A luxury research detour needs a missing Amenity and a buildable connection.
//!
//! Version one takes the cheapest printed improvement unlock, including for
//! duplicate luxuries and tiles the Builder cannot improve. Version two keeps
//! that arm intact and asks the engine whether each unlock's complete tech
//! chain would make a missing luxury connectable. Current civic, terrain,
//! ownership, unique-improvement and host-refusal constraints still apply.

use super::{AdvancedAi, Game, Name, Pos};
use std::collections::{BTreeMap, BTreeSet};

/// An Amenity detour should fit within twelve Standard turns of city science.
/// The actual research bill includes prerequisites, speed, Eurekas and progress.
const LUXURY_RESEARCH_STANDARD_TURNS: u32 = 12;

impl AdvancedAi {
    pub fn enable_connect_the_luxury_2(&mut self) {
        self.connect_the_luxury = false;
        self.connect_the_luxury_2 = true;
    }

    pub fn disable_connect_the_luxury_2(&mut self) {
        self.connect_the_luxury_2 = false;
    }

    fn luxury_connectable(g: &Game, pid: usize, pos: Pos, resource: Name) -> bool {
        g.valid_improvements(pid, pos).iter().any(|name| {
            let spec = &g.rules.improvements[name];
            spec.builder_buildable
                && (spec.resources.contains(&resource)
                    || g.rules.resources[&resource].improvement == *name)
        })
    }

    fn luxury_research_path(g: &Game, pid: usize, goal: Name) -> BTreeSet<Name> {
        g.rules
            .tech_ancestors
            .get(goal.as_str())
            .into_iter()
            .flatten()
            .map(|tech| Name::new(tech))
            .chain(std::iter::once(goal))
            .filter(|tech| !g.players[pid].techs.contains(tech))
            .collect()
    }

    fn luxury_research_cost(g: &Game, pid: usize, path: &BTreeSet<Name>) -> f64 {
        let player = &g.players[pid];
        path.iter()
            .map(|tech| {
                let cost = g.tech_cost(tech.as_str());
                // Active progress already includes the Eureka credited by
                // Game::begin_research, so do not subtract it twice.
                if player.research.as_deref() == Some(tech.as_str()) {
                    return (cost - player.research_progress).max(0.0);
                }
                let boost = if player.boosted_techs.contains(tech) {
                    Self::boost_frac(g, tech.as_str(), true)
                        + if g.has_ability(pid, "dynastic_cycle") {
                            0.1
                        } else {
                            0.0
                        }
                } else {
                    0.0
                };
                (cost * (1.0 - boost)).max(0.0)
            })
            .sum()
    }

    pub(super) fn useful_luxury_tech(&self, g: &Game, pid: usize) -> Option<&'static str> {
        if !self.connect_the_luxury_2 {
            return None;
        }
        // Read the current board directly: the Builder's per-turn deficit
        // frame may predate a trade or city change earlier in this turn.
        let cities = g.player_city_ids(pid);
        let deficit: f64 = cities
            .iter()
            .map(|cid| {
                let city = &g.cities[cid];
                (Game::city_amenities_required(city) - g.city_amenities(city)).max(0) as f64
            })
            .sum();
        if deficit <= 0.0 {
            return None;
        }

        let mut plots: BTreeMap<Name, Vec<Pos>> = BTreeMap::new();
        for cid in &cities {
            for pos in &g.cities[cid].owned_tiles {
                let Some(tile) = g.map.get(*pos) else {
                    continue;
                };
                let Some(resource) = tile.resource else {
                    continue;
                };
                if tile.owner_city != Some(*cid)
                    || tile.improvement.is_some()
                    || tile.submerged
                    || !g.resource_visible_to(pid, resource.as_str())
                    || !g
                        .rules
                        .resources
                        .get(&resource)
                        .is_some_and(|spec| spec.class == "luxury")
                    || g.congress_effect_active("luxury_policy", "B", resource.as_str())
                {
                    continue;
                }
                plots.entry(resource).or_default().push(*pos);
            }
        }
        // Imports and city-state access count too. An already buildable copy
        // means the remaining bottleneck is a Builder, not another technology.
        plots.retain(|resource, positions| {
            g.resource_access_count(pid, resource.as_str()) == 0
                && !positions
                    .iter()
                    .any(|pos| Self::luxury_connectable(g, pid, *pos, *resource))
        });
        if plots.is_empty() {
            return None;
        }
        let player = &g.players[pid];
        let goals: BTreeSet<Name> = g
            .rules
            .improvements
            .iter()
            .filter_map(|(name, spec)| {
                let tech = spec.tech?;
                (spec.builder_buildable
                    && !spec.unbuildable
                    && !player.techs.contains(&tech)
                    && g.rules.techs.contains_key(&tech)
                    && spec
                        .civic
                        .is_none_or(|civic| player.civics.contains(&civic))
                    && plots.keys().any(|resource| {
                        spec.resources.contains(resource)
                            || g.rules.resources[resource].improvement == *name
                    }))
                .then_some(tech)
            })
            .collect();
        let science: f64 = cities.iter().map(|cid| g.city_yields(*cid).science).sum();
        let budget = science * f64::from(g.standard_duration(LUXURY_RESEARCH_STANDARD_TURNS));
        let reach: f64 = if g.has_ability(pid, "gifts_for_the_tlatoani") {
            6.0
        } else {
            4.0
        };
        let mut best: Option<(f64, f64, Name)> = None;
        for goal in goals {
            let path = Self::luxury_research_path(g, pid, goal);
            let cost = Self::luxury_research_cost(g, pid, &path);
            if cost > budget {
                continue;
            }
            let mut after = g.speculative_clone();
            after.players[pid].techs.extend(path);
            let connections = plots
                .iter()
                .filter(|(resource, positions)| {
                    positions
                        .iter()
                        .any(|pos| Self::luxury_connectable(&after, pid, *pos, **resource))
                })
                .count();
            if connections == 0 {
                continue;
            }
            let relief = (connections as f64 * reach.min(cities.len() as f64)).min(deficit);
            let value = relief / cost.max(1.0);
            if best.is_none_or(|(old_value, old_cost, old_goal)| {
                value > old_value
                    || (value == old_value
                        && (cost < old_cost || (cost == old_cost && goal < old_goal)))
            }) {
                best = Some((value, cost, goal));
            }
        }
        best.map(|(_, _, goal)| goal.as_str())
    }
}

#[cfg(test)]
mod tests;
