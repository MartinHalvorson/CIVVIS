//! Reveal fuel needed by an existing Domination army before optional upgrades.

use super::{AdvancedAi, VictoryTarget};
use crate::{game::Game, name::Name};
use std::collections::BTreeMap;

impl AdvancedAi {
    pub(super) fn standing_army_fuel_goal(&self, g: &Game, pid: usize) -> Option<Name> {
        if !self.victory_planning
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !g.players.iter().any(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && g.is_at_war(pid, other.id)
            })
        {
            return None;
        }
        let mut goals: BTreeMap<Name, f64> = BTreeMap::new();
        for unit in g
            .units
            .values()
            .filter(|unit| unit.owner == pid && !unit.free_upkeep)
        {
            let spec = &g.rules.units[unit.kind];
            if spec.class != "military" || spec.resource_maintenance <= 0.0 {
                continue;
            }
            let Some(resource) = spec.requires_resource else {
                continue;
            };
            if g.strategic_stockpile(pid, resource) > 0.0 {
                continue;
            }
            let Some(tech) = g.rules.resources[resource].tech else {
                continue;
            };
            if g.players[pid].techs.contains(&tech) {
                continue;
            }
            // Supply the largest existing combat investment first. A single
            // granted endgame unit matters even without an upgrade successor.
            *goals.entry(tech).or_default() += spec.strength.max(spec.ranged_attack_strength());
        }
        goals
            .into_iter()
            .max_by(|(a, av), (b, bv)| av.total_cmp(bv).then_with(|| b.cmp(a)))
            .map(|(tech, _)| tech)
    }
}

#[cfg(test)]
mod tests;
