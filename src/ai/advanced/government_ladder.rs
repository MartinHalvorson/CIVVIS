//! Government upgrades priced by the civic path that still has to be paid.

use super::{
    AdvancedAi, Game, GrandStrategy, Name, GOVERNMENT_LADDER_BEHIND_WINDOW,
    GOVERNMENT_LADDER_WINDOW,
};
use std::collections::BTreeSet;

/// A late government must leave twenty Standard turns to use its extra slots.
const GOVERNMENT_USE_STANDARD_TURNS: u32 = 20;

impl AdvancedAi {
    pub fn enable_government_ladder_3(&mut self) {
        self.government_ladder = false;
        self.government_ladder_2 = false;
        self.government_ladder_3 = true;
    }

    pub fn disable_government_ladder_3(&mut self) {
        self.government_ladder_3 = false;
    }

    fn government_route_capacity(g: &Game, name: &str) -> i64 {
        g.rules.governments.get(name).map_or(0, |spec| {
            spec.slots.military + spec.slots.economic + spec.slots.diplomatic + spec.slots.wildcard
        })
    }

    fn government_route_cost(g: &Game, pid: usize, goal: Name) -> f64 {
        let player = &g.players[pid];
        let path: BTreeSet<Name> = g
            .rules
            .civic_ancestors
            .get(goal.as_str())
            .into_iter()
            .flatten()
            .map(|node| Name::new(node))
            .chain(std::iter::once(goal))
            .filter(|node| !player.civics.contains(node))
            .collect();
        let cost: f64 = path
            .iter()
            .map(|node| {
                let full = g.civic_cost(node.as_str());
                if player.civic.as_deref() == Some(node.as_str()) {
                    // The active study's progress already includes its Inspiration.
                    return (full - player.civic_progress).max(0.0);
                }
                let boost = if player.boosted_civics.contains(node) {
                    Self::boost_frac(g, node.as_str(), false)
                        + if g.has_ability(pid, "dynastic_cycle") {
                            0.1
                        } else {
                            0.0
                        }
                } else {
                    0.0
                };
                (full * (1.0 - boost)).max(0.0)
            })
            .sum();
        // Unassigned Culture is spent on the first selected prerequisite once,
        // not on every node in the path.
        (cost
            - if player.civic.is_none() {
                player.civic_overflow
            } else {
                0.0
            })
        .max(0.0)
    }

    pub(super) fn government_ladder_route(
        &self,
        g: &Game,
        pid: usize,
        objective: GrandStrategy,
    ) -> Option<&'static str> {
        if !self.government_ladder_3 {
            return None;
        }
        let capacity = |name: &str| Self::government_route_capacity(g, name);
        let player = &g.players[pid];
        let ours = player.government.as_deref().map_or(0, &capacity);
        let behind = g
            .players
            .iter()
            .filter(|rival| {
                rival.id != pid && rival.alive && !rival.is_minor && !rival.is_barbarian
            })
            .any(|rival| rival.government.as_deref().map_or(0, &capacity) > ours);
        // Preserve v2's initiation window. The extra check below asks when the
        // path will finish, rather than treating today's date as completion.
        let window = if behind {
            GOVERNMENT_LADDER_BEHIND_WINDOW
        } else {
            GOVERNMENT_LADDER_WINDOW
        };
        if g.turn as f64 > g.max_turns.max(1) as f64 * window {
            return None;
        }
        let priorities = Self::government_priorities(objective, false);
        let culture_match = Self::culture_government_match(g, pid, objective);
        let culture: f64 = g
            .player_city_ids(pid)
            .iter()
            .map(|cid| g.city_yields(*cid).culture)
            .sum();
        let research_turns = g
            .max_turns
            .saturating_sub(g.turn)
            .saturating_sub(g.standard_duration(GOVERNMENT_USE_STANDARD_TURNS));
        let budget = culture.max(0.0) * f64::from(research_turns);
        g.rules
            .governments
            .iter()
            .filter(|(name, _)| {
                self.government_capacity_fallback
                    || priorities.contains(&name.as_str())
                    || culture_match.as_deref() == Some(name.as_str())
            })
            .filter_map(|(name, spec)| {
                let extra = capacity(name)
                    .checked_sub(ours)
                    .filter(|extra| *extra > 0)?;
                let goal = spec.civic?;
                if player.civics.contains(&goal) || !g.rules.civics.contains_key(&goal) {
                    return None;
                }
                let cost = Self::government_route_cost(g, pid, goal);
                (research_turns > 0 && cost <= budget).then_some((cost / extra as f64, cost, goal))
            })
            .min_by(|a, b| {
                a.0.total_cmp(&b.0)
                    .then(a.1.total_cmp(&b.1))
                    .then(a.2.cmp(&b.2))
            })
            .map(|(_, _, goal)| goal.as_str())
    }
}

#[cfg(test)]
mod tests;
