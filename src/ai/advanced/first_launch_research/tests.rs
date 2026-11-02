use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

fn board(missing: &[&str]) -> Game {
    let mut g = Game::new(2, 24, 16, 21, 250, 0);
    g.players[0].civ = "Rome".to_string();
    g.players[0].techs = g
        .rules
        .techs
        .keys()
        .copied()
        .filter(|tech| !missing.contains(&tech.as_str()))
        .collect();
    g.players[0].research = None;
    // Keep every era eligible except in the explicit rolling-window test.
    g.world_era = 7;
    g
}

fn plan(strategy: GrandStrategy) -> StrategicPlan {
    StrategicPlan {
        strategy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: 0,
        rush: false,
    }
}

#[test]
fn first_launch_keeps_research_ships_off() {
    opt_in_off_in_both_controllers("first-launch-keeps-research", |ai| {
        ai.first_launch_keeps_research
    });
}

#[test]
fn recovery_keeps_the_missing_prerequisite_then_satellites() {
    let mut g = board(&["advanced_flight", "satellites"]);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_victory_portfolio();
    assert_eq!(
        ai.decision_objective(GrandStrategy::Recovery),
        GrandStrategy::Recovery
    );
    ai.enable_first_launch_keeps_research();
    // The first launch may not be stranded behind irrelevant era backlog.
    g.world_era = 0;
    ai.advanced_research(&mut g, 0, &plan(GrandStrategy::Recovery));
    assert_eq!(g.players[0].research.as_deref(), Some("advanced_flight"));
    g.players[0].techs.insert(crate::name!("advanced_flight"));
    g.players[0].research = None;
    ai.advanced_research(&mut g, 0, &plan(GrandStrategy::Recovery));
    assert_eq!(g.players[0].research.as_deref(), Some("satellites"));
}

#[test]
fn optional_wartime_upgrade_yields_only_with_the_treatment() {
    let mut g = board(&["iron_working", "satellites"]);
    let home = g.units[&g.player_unit_ids(0)[0]].pos;
    g.units.retain(|_, unit| unit.owner != 0);
    for _ in 0..2 {
        g.spawn_test_unit("warrior", 0, home);
    }
    g.at_war.insert((0, 1));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    assert_eq!(
        ai.wartime_modernization_tech(&g, 0).as_deref(),
        Some("iron_working")
    );
    ai.advanced_research(&mut g, 0, &plan(GrandStrategy::Science));
    assert_eq!(
        g.players[0].research.as_deref(),
        Some("iron_working"),
        "control keeps the old priority"
    );
    g.players[0].research = None;
    ai.enable_first_launch_keeps_research();
    ai.advanced_research(&mut g, 0, &plan(GrandStrategy::Science));
    assert_eq!(g.players[0].research.as_deref(), Some("satellites"));
}

#[test]
fn appointed_war_breakthrough_keeps_its_priority() {
    let mut g = board(&["iron_working", "satellites"]);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_first_launch_keeps_research();
    ai.war_plan = Some(WarPlan {
        target_player: 1,
        objective_city: 0,
        breakthrough_tech: crate::name!("iron_working"),
        assault_unit: crate::name!("swordsman"),
        predecessor: None,
        breach_unit: None,
        estimated_research_turns: 1,
        estimated_production_turns: 1,
        estimated_upgrade_gold: 0.0,
        estimated_march_turns: 1,
        phase: WarPhase::Research,
        appointed_turn: 0,
        tech_turn: None,
        declared_turn: None,
        last_reviewed_turn: 0,
        recovery_assessments: 0,
    });
    ai.advanced_research(&mut g, 0, &plan(GrandStrategy::Science));
    assert_eq!(g.players[0].research.as_deref(), Some("iron_working"));
}

#[test]
fn non_science_pre_rocketry_disabled_victory_and_finished_controls() {
    let mut g = board(&["satellites"]);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_first_launch_keeps_research();
    assert_eq!(ai.first_launch_research_goal(&g, 0), None);
    ai.retarget(VictoryTarget::Science);
    assert_eq!(ai.first_launch_research_goal(&g, 0), Some("satellites"));
    g.players[0].techs.remove(&crate::name!("rocketry"));
    assert_eq!(ai.first_launch_research_goal(&g, 0), None);
    g.players[0].techs.insert(crate::name!("rocketry"));
    g.victory_conditions.science = false;
    assert_eq!(ai.first_launch_research_goal(&g, 0), None);
    g.victory_conditions.science = true;
    g.players[0].techs.insert(crate::name!("satellites"));
    assert_eq!(ai.first_launch_research_goal(&g, 0), None);
}

#[test]
fn existing_research_is_not_interrupted() {
    let mut g = board(&["iron_working", "satellites"]);
    g.players[0].research = Some("iron_working".to_string());
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_first_launch_keeps_research();
    ai.advanced_research(&mut g, 0, &plan(GrandStrategy::Recovery));
    assert_eq!(g.players[0].research.as_deref(), Some("iron_working"));
}
