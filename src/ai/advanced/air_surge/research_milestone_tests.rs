use super::*;
use crate::ai::{
    advanced::{WarPhase, WarPlan},
    BasicAi,
};

fn radio_with_renaissance_backlog() -> (Game, AdvancedAi, StrategicPlan) {
    let mut g = Game::new_full(2, 40, 24, 936017, 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (6, 12), None);
    let target = g.found_city_for(1, (25, 12), None);
    g.players[0].techs.extend(
        g.rules
            .techs
            .iter()
            .filter(|(_, spec)| spec.era < 3)
            .map(|(tech, _)| *tech),
    );
    g.players[0].techs.extend(
        g.rules.tech_ancestors["advanced_flight"]
            .iter()
            .map(|tech| Name::new(tech)),
    );
    g.current = 0;
    g.turn = 142;
    g.at_war.clear();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.air_surge_plan = Some(AirSurge {
        target_player: 1,
        objective_city: target,
        objective_pos: (25, 12),
        body_unit: crate::name!("knight"),
        body_is_cavalry: true,
        opened_at_war: false,
        phase: AirSurgePhase::Beeline,
        appointed_turn: 110,
        tech_turn: None,
        declared_turn: None,
        last_reviewed_turn: 142,
        recovery_assessments: 0,
    });
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: 142,
        rush: false,
    };
    assert!(g.players[0].techs.contains(&crate::name!("radio")));
    assert!(!g.players[0].techs.contains(&crate::name!("printing")));
    assert!(g
        .available_techs(0)
        .contains(&crate::name!("advanced_flight")));
    assert!(!BasicAi::era_window_techs(&g, 0).contains(&crate::name!("advanced_flight")));
    (g, ai, plan)
}

#[test]
fn appointed_air_breakthrough_finishes_before_unrelated_era_backlog() {
    for fighting in [false, true] {
        let (mut g, mut ai, plan) = radio_with_renaissance_backlog();
        if fighting {
            g.at_war.insert((0, 1));
            let surge = ai.air_surge_plan.as_mut().unwrap();
            surge.phase = AirSurgePhase::Exploit;
            surge.opened_at_war = true;
        }
        ai.advanced_research(&mut g, 0, &plan);
        assert_eq!(
            g.players[0].research.as_deref(),
            Some("advanced_flight"),
            "fighting={fighting}"
        );
    }
}

#[test]
fn unappointed_research_keeps_the_era_window() {
    let (mut g, mut ai, plan) = radio_with_renaissance_backlog();
    ai.air_surge_plan = None;
    ai.advanced_research(&mut g, 0, &plan);
    assert_ne!(g.players[0].research.as_deref(), Some("advanced_flight"));
}

#[test]
fn air_milestone_still_requires_its_prerequisites() {
    let (mut g, ai, plan) = radio_with_renaissance_backlog();
    g.players[0].techs.remove(&crate::name!("radio"));
    assert!(!g
        .available_techs(0)
        .contains(&crate::name!("advanced_flight")));
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("radio"));
}

#[test]
fn air_milestone_does_not_cancel_research_already_in_progress() {
    let (mut g, ai, plan) = radio_with_renaissance_backlog();
    g.players[0].research = Some("printing".to_string());
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("printing"));
}

#[test]
fn an_appointed_land_breakthrough_keeps_research_priority() {
    let (mut g, mut ai, plan) = radio_with_renaissance_backlog();
    g.players[0].techs.insert(crate::name!("gunpowder"));
    ai.war_plan = Some(WarPlan {
        target_player: 1,
        objective_city: plan.target_city.unwrap(),
        breakthrough_tech: crate::name!("metal_casting"),
        assault_unit: crate::name!("bombard"),
        predecessor: None,
        breach_unit: None,
        estimated_research_turns: 1,
        estimated_production_turns: 3,
        estimated_upgrade_gold: 0.0,
        estimated_march_turns: 2,
        phase: WarPhase::Research,
        appointed_turn: 130,
        tech_turn: None,
        declared_turn: None,
        last_reviewed_turn: 142,
        recovery_assessments: 0,
    });
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("metal_casting"));
}
