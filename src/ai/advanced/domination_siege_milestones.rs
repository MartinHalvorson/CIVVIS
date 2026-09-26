//! Bounded fatigue relief for a Domination siege that removes substantial defenses.

use super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::Game;

#[derive(Clone, Debug)]
pub(super) struct SiegeMilestone {
    greatest_quarter: i32,
    initial_maximum: i32,
    damage_reference: i32,
    progressed_at: Option<u32>,
    observed_turn: u32,
}

impl AdvancedAi {
    fn domination_siege_present(g: &Game, pid: usize, city: u32) -> bool {
        let Some(city) = g.cities.get(&city) else {
            return false;
        };
        g.units.values().any(|unit| {
            let spec = &g.rules.units[unit.kind];
            unit.owner == pid
                && unit.hp >= 50
                && spec.class == "military"
                && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                && g.wdist(unit.pos, city.pos) <= 3
        })
    }

    pub(super) fn observe_domination_siege_milestones(&mut self, g: &Game, pid: usize) {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination) {
            self.domination_siege_milestones.clear();
            return;
        }
        // Mirror rebuilds renumber city IDs. Owner and position identify the
        // same physical siege without transferring its credit to a reused ID.
        self.domination_siege_milestones.retain(|(owner, pos), _| {
            g.is_at_war(pid, *owner)
                && g.cities
                    .values()
                    .any(|city| city.owner == *owner && city.pos == *pos)
        });
        let visible = g.player_vision_frame(pid);
        for city in g.cities.values().filter(|city| {
            city.owner != pid
                && !g.players[city.owner].is_minor
                && g.is_at_war(pid, city.owner)
                && g.sees(&visible, city.pos)
        }) {
            let maximum = 200 + g.city_max_wall_hp(city);
            let remaining = (city.hp + city.wall_hp).clamp(0, maximum);
            let quarter = (maximum - remaining) * 4 / maximum;
            let milestone = self
                .domination_siege_milestones
                .entry((city.owner, city.pos))
                .or_insert(SiegeMilestone {
                    greatest_quarter: quarter,
                    initial_maximum: maximum,
                    // Already-missing health consumes initial thresholds.
                    damage_reference: maximum,
                    progressed_at: None,
                    observed_turn: g.turn,
                });
            // Keep the original four-quarter budget. Only observed health
            // can raise its damage reference. Upgraded walls can take HP
            // above the old maximum; clamping there would discard real hits.
            // Capacity alone grants nothing, and repairs cannot reuse spent
            // thresholds even after a larger health peak has been observed.
            milestone.damage_reference = milestone.damage_reference.max(remaining);
            let removed =
                (milestone.damage_reference - remaining).clamp(0, milestone.initial_maximum);
            let quarter = removed * 4 / milestone.initial_maximum;
            if quarter > milestone.greatest_quarter {
                // Consume each threshold even without our army: returning to
                // someone else's damaged city must not manufacture progress.
                milestone.greatest_quarter = quarter;
                if g.turn.saturating_sub(milestone.observed_turn) <= 1
                    && Self::domination_siege_present(g, pid, city.id)
                {
                    milestone.progressed_at = Some(g.turn);
                }
            }
            milestone.observed_turn = g.turn;
        }
    }

    pub(super) fn domination_siege_is_progressing(
        &self,
        g: &Game,
        pid: usize,
        other: usize,
        plan: &StrategicPlan,
    ) -> bool {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || plan.strategy != GrandStrategy::Conquest
            || plan.target_player != Some(other)
            || plan.threatened_city.is_some()
            || !g.is_at_war(pid, other)
        {
            return false;
        }
        plan.target_city.is_some_and(|id| {
            let Some(city) = g.cities.get(&id).filter(|city| city.owner == other) else {
                return false;
            };
            Self::domination_siege_present(g, pid, id)
                && self
                    .domination_siege_milestones
                    .get(&(city.owner, city.pos))
                    .is_some_and(|milestone| {
                        milestone
                            .progressed_at
                            .is_some_and(|turn| g.turn.saturating_sub(turn) < 12)
                    })
        })
    }
}

#[cfg(test)]
mod tests;
