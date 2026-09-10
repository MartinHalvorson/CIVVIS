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
mod tests {
    use super::*;
    use crate::{ai::VictoryTarget, name};

    fn board() -> (Game, AdvancedAi, StrategicPlan) {
        let mut g = Game::new_full(2, 30, 20, 109_102_000, 300, 0, false);
        for pid in 0..2 {
            let settler = g
                .player_unit_ids(pid)
                .into_iter()
                .find(|id| g.units[id].kind == "settler")
                .unwrap();
            g.found_city_for(pid, g.units[&settler].pos, None);
        }
        g.current = 0;
        g.turn = 90;
        g.record_contact(0, 1);
        let target = g.player_city_ids(1)[0];
        g.cities.get_mut(&target).unwrap().wall_hp = 100;
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.enable_domination_siege_research();
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(target),
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: g.turn,
            rush: false,
        };
        (g, ai, plan)
    }

    #[test]
    fn missing_siege_research_unlocks_engineering_before_the_war() {
        let (mut g, ai, plan) = board();
        assert!(!g.is_at_war(0, 1));
        assert_eq!(
            ai.domination_siege_research_goal(&g, 0, &plan),
            Some(name!("engineering"))
        );
        let ancestors = g.rules.tech_ancestors["engineering"].clone();
        g.players[0]
            .techs
            .extend(ancestors.iter().map(|tech| Name::new(tech)));
        ai.advanced_research(&mut g, 0, &plan);
        assert_eq!(g.players[0].research.as_deref(), Some("engineering"));
    }

    #[test]
    fn siege_research_stops_for_unlocked_fielded_or_queued_capability() {
        let (mut g, ai, plan) = board();
        g.players[0].techs.insert(name!("engineering"));
        assert_eq!(ai.domination_siege_research_goal(&g, 0, &plan), None);
        g.players[0].techs.remove(&name!("engineering"));
        let home = g.player_city_ids(0)[0];
        g.cities.get_mut(&home).unwrap().queue.push(Item::Unit {
            unit: name!("catapult"),
        });
        assert_eq!(ai.domination_siege_research_goal(&g, 0, &plan), None);
        g.cities.get_mut(&home).unwrap().queue.clear();
        let unit = g.spawn_test_unit("catapult", 0, g.cities[&home].pos);
        assert_eq!(ai.domination_siege_research_goal(&g, 0, &plan), None);
        g.remove_unit(unit);
        assert!(ai.domination_siege_research_goal(&g, 0, &plan).is_some());
    }

    #[test]
    fn siege_research_requires_a_walled_enemy_conquest_objective_and_is_reversible() {
        let (mut g, mut ai, mut plan) = board();
        assert!(!AdvancedAi::new().domination_siege_research);
        assert!(!AdvancedAi::legacy().domination_siege_research);
        assert!(super::super::GENES
            .iter()
            .any(|gene| gene.tag == "domination-siege-research" && gene.opt_in()));
        ai.disable_domination_siege_research();
        assert_eq!(ai.domination_siege_research_goal(&g, 0, &plan), None);
        ai.enable_domination_siege_research();
        for strategy in [
            GrandStrategy::Expansion,
            GrandStrategy::Recovery,
            GrandStrategy::Science,
        ] {
            plan.strategy = strategy;
            assert_eq!(ai.domination_siege_research_goal(&g, 0, &plan), None);
        }
        plan.strategy = GrandStrategy::Conquest;
        let target = plan.target_city.unwrap();
        g.cities.get_mut(&target).unwrap().wall_hp = 0;
        assert_eq!(ai.domination_siege_research_goal(&g, 0, &plan), None);
        g.cities.get_mut(&target).unwrap().wall_hp = 100;
        plan.target_player = Some(0);
        assert_eq!(ai.domination_siege_research_goal(&g, 0, &plan), None);
        plan.target_player = Some(1);
        ai.battlefront_observation = true;
        assert_eq!(
            ai.domination_siege_research_goal(&g, 0, &plan),
            None,
            "unknown walls must not be read from hidden state"
        );
        let scout = g.spawn_test_unit("scout", 0, g.cities[&target].pos);
        ai.belief.observe(&g, 0);
        assert!(ai.domination_siege_research_goal(&g, 0, &plan).is_some());
        g.remove_unit(scout);
        g.cities.get_mut(&target).unwrap().wall_hp = 0;
        assert!(
            ai.domination_siege_research_goal(&g, 0, &plan).is_some(),
            "the last observed walls persist until a fresh sighting replaces them"
        );
    }
}
