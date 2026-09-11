//! Research the missing wall-breaking capability before an expedition stalls.
//!
//! A Conquest target with observed walls is a concrete technology debt. This
//! opt-in unlocks the cheapest missing land siege technology only while the
//! empire has neither a siege unit committed nor an unlocked siege design.
//! Production, strategic materials and later modernization remain separate
//! decisions. Existing defensive and appointed breakthrough goals run first.

use super::{AdvancedAi, GrandStrategy, StrategicPlan};
use crate::game::{Game, Item};
use crate::name::Name;

impl AdvancedAi {
    pub(super) fn domination_siege_research_goal(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<Name> {
        if !self.domination_siege_research || plan.strategy != GrandStrategy::Conquest {
            return None;
        }
        let city = g.cities.get(&plan.target_city?)?;
        if Some(city.owner) != plan.target_player
            || city.owner == pid
            || g.same_team(pid, city.owner)
            || !self.campaign_target_legal(g, pid, city.owner)
        {
            return None;
        }
        let walls = if self.battlefront_observation {
            let report = self.remembered_city(city.id)?;
            if report.owner != city.owner {
                return None;
            }
            report.wall_hp
        } else {
            city.wall_hp
        };
        if walls <= 0 {
            return None;
        }
        let land_siege = |kind: Name| {
            let spec = &g.rules.units[kind];
            spec.class == "military"
                && spec.siege
                && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
        };
        if g.units
            .values()
            .any(|unit| unit.owner == pid && land_siege(unit.kind))
            || g.cities
                .values()
                .filter(|city| city.owner == pid)
                .any(|city| {
                    city.queue
                        .iter()
                        .any(|item| matches!(item, Item::Unit { unit } if land_siege(*unit)))
                })
        {
            return None;
        }
        let designs: Vec<_> = g
            .rules
            .units
            .iter()
            .filter(|(kind, spec)| {
                land_siege(**kind)
                    && spec.buildable
                    && g.player_unit_replacement(pid, **kind) == **kind
                    && spec
                        .unique_to
                        .as_deref()
                        .is_none_or(|civ| civ == g.players[pid].civ)
                    && spec
                        .obsolete_tech
                        .is_none_or(|tech| !g.players[pid].techs.contains(&tech))
            })
            .collect();
        // If the design is already unlocked, another technology does not fix
        // a production or strategic-resource shortage. Give those systems time
        // to field it instead of escalating the research goal every turn.
        if designs.iter().any(|(_, spec)| {
            spec.tech
                .is_none_or(|tech| g.players[pid].techs.contains(&tech))
        }) {
            return None;
        }
        designs
            .into_iter()
            .filter_map(|(_, spec)| spec.tech)
            .min_by(|left, right| {
                Self::war_remaining_research_cost(g, pid, *left)
                    .total_cmp(&Self::war_remaining_research_cost(g, pid, *right))
                    .then_with(|| left.cmp(right))
            })
    }
}

#[cfg(test)]
mod tests;
