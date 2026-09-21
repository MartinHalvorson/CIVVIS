use super::*;

fn urgent_surging_opponent(at_war: bool, opened_at_war: bool) -> (Game, AdvancedAi) {
    let mut g = Game::new_full(2, 40, 24, 936010, 500, 0, false);
    let target = g.found_city_for(1, (20, 12), None);
    g.players[0].met.insert(1);
    g.players[1].dvp = 19;
    g.at_war.clear();
    if at_war {
        g.at_war.insert((0, 1));
    }
    g.turn = 150;
    let mut ai = AdvancedAi::new();
    ai.enable_air_surge_2();
    ai.deny_leaders = true;
    ai.air_surge_plan = Some(AirSurge {
        target_player: 1,
        objective_city: target,
        objective_pos: g.cities[&target].pos,
        body_unit: Name::new("musketman"),
        body_is_cavalry: false,
        opened_at_war,
        phase: if at_war {
            AirSurgePhase::Exploit
        } else {
            AirSurgePhase::Arm
        },
        appointed_turn: 140,
        tech_turn: None,
        declared_turn: (at_war && !opened_at_war).then_some(145),
        last_reviewed_turn: 150,
        recovery_assessments: 0,
    });
    assert!(ai.urgent_victory_threat(&g, 1));
    assert_eq!(ai.air_surge_research_goal(&g, 0), Some(AIR_SURGE_GOAL_TECH));
    (g, ai)
}

#[test]
fn urgent_denial_preserves_an_air_plan_appointed_during_war() {
    let (mut g, mut ai) = urgent_surging_opponent(true, true);
    assert!(!ai.air_surge_opening(&mut g, 0, 1));
    assert!(ai.air_surge_plan.is_some());
    assert_eq!(ai.air_surge_research_goal(&g, 0), Some(AIR_SURGE_GOAL_TECH));
}

#[test]
fn urgent_denial_preserves_a_surge_that_already_declared_war() {
    let (mut g, mut ai) = urgent_surging_opponent(true, false);
    assert!(!ai.air_surge_opening(&mut g, 0, 1));
    assert!(ai.air_surge_plan.is_some());
}

#[test]
fn urgent_denial_still_overrides_peacetime_air_readiness() {
    let (mut g, mut ai) = urgent_surging_opponent(false, false);
    assert!(!ai.air_surge_opening(&mut g, 0, 1));
    assert!(ai.air_surge_plan.is_none());
    assert!(
        !g.is_at_war(0, 1),
        "ordinary urgent opening remains the caller's decision"
    );
}

#[test]
fn domination_denial_releases_readiness_without_discarding_air_research() {
    let (mut g, mut ai) = urgent_surging_opponent(false, false);
    ai.victory_target = Some(VictoryTarget::Domination);
    let before = ai.air_surge_plan.clone();
    assert!(!ai.air_surge_opening(&mut g, 0, 1));
    assert_eq!(ai.air_surge_plan, before);
    assert_eq!(ai.air_surge_research_goal(&g, 0), Some(AIR_SURGE_GOAL_TECH));
    assert!(!g.is_at_war(0, 1));
    assert_eq!(ai.air_surge_cooldown_until, 0);
}

fn staged_domination_denial() -> (Game, AdvancedAi, StrategicPlan) {
    let (mut g, mut ai) = urgent_surging_opponent(false, false);
    ai.victory_target = Some(VictoryTarget::Domination);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
    }
    g.found_city_for(0, (12, 12), None);
    g.found_city_for(0, (10, 18), None);
    g.record_contact(0, 1);
    g.current = 0;
    g.players[0].gold = 10000.0;
    for pos in [(16, 12), (16, 13), (17, 11), (17, 12)] {
        g.map.tiles.get_mut(&pos).unwrap().owner_city = None;
        g.spawn_test_unit("tank", 0, pos);
    }
    let target = ai.air_surge_plan.as_ref().unwrap().objective_city;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    };
    assert!(ai.campaign_staged_for_war(&g, 0, 1, (20, 12), true));
    assert!(ai.threatened_city(&g, 0).is_none());
    (g, ai, plan)
}

#[test]
fn domination_denial_declaration_keeps_the_air_plan_on_the_next_turn() {
    let (mut g, mut ai, plan) = staged_domination_denial();
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g.is_at_war(0, 1));
    let surge = ai
        .air_surge_plan
        .as_ref()
        .expect("keep preparations after declaring");
    assert_eq!(surge.declared_turn, Some(150));
    assert_eq!(surge.phase, AirSurgePhase::Exploit);
    g.turn += 1;
    ai.maintain_air_surge(&g, 0);
    let surge = ai
        .air_surge_plan
        .as_ref()
        .expect("ordinary declaration is our war");
    assert_eq!(surge.appointed_turn, 140);
    assert_eq!(surge.declared_turn, Some(150));
    assert_eq!(ai.air_surge_cooldown_until, 0);
    assert_eq!(ai.air_surge_research_goal(&g, 0), Some(AIR_SURGE_GOAL_TECH));
}

#[test]
fn domination_denial_keeps_peace_deadline_and_does_not_invent_a_declaration() {
    let (mut g, mut ai, plan) = staged_domination_denial();
    ai.peace_until = g.turn + 5;
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1));
    let surge = ai
        .air_surge_plan
        .as_ref()
        .expect("research survives a held war");
    assert_eq!(surge.declared_turn, None);
    assert_eq!(surge.appointed_turn, 140);
}

#[test]
fn domination_denial_still_requires_a_solvent_staged_army() {
    for blocker in ["treasury", "staging"] {
        let (mut g, mut ai, plan) = staged_domination_denial();
        match blocker {
            "treasury" => {
                ai.war_needs_a_treasury = true;
                g.players[0].gold = 0.0;
                g.players[0].gold_per_turn = -50.0;
                assert!(!ai.war_is_affordable(&g, 0));
            }
            "staging" => {
                for uid in g.player_unit_ids(0) {
                    g.remove_unit(uid);
                }
                assert!(!ai.campaign_staged_for_war(&g, 0, 1, (20, 12), true));
            }
            _ => unreachable!(),
        }
        ai.advanced_diplomacy(&mut g, 0, &plan);
        assert!(!g.is_at_war(0, 1), "{blocker}");
        assert_eq!(ai.air_surge_plan.as_ref().unwrap().declared_turn, None);
    }
}

#[test]
fn denial_still_cancels_air_preparations_for_other_lanes_and_home_danger() {
    for lane in [VictoryTarget::Science, VictoryTarget::Culture] {
        let (mut g, mut ai) = urgent_surging_opponent(false, false);
        ai.victory_target = Some(lane);
        assert!(!ai.air_surge_opening(&mut g, 0, 1));
        assert!(ai.air_surge_plan.is_none());
    }
    let (mut g, mut ai, _) = staged_domination_denial();
    let barbarian = g.players.iter().find(|p| p.is_barbarian).unwrap().id;
    g.at_war.insert((0, barbarian));
    for _ in 0..5 {
        g.spawn_test_unit("modern_armor", barbarian, (11, 12));
    }
    assert!(ai.threatened_city(&g, 0).is_some());
    assert!(!ai.air_surge_opening(&mut g, 0, 1));
    assert!(ai.air_surge_plan.is_none());
}

#[test]
fn joining_an_air_war_requires_its_actual_matching_declaration() {
    let (mut g, mut ai, _) = staged_domination_denial();
    let before = ai.air_surge_plan.clone();
    ai.air_surge_join_declared_war(&g, 0, 1);
    assert_eq!(ai.air_surge_plan, before);
    g.at_war.insert((0, 1));
    ai.air_surge_join_declared_war(&g, 0, 0);
    assert_eq!(ai.air_surge_plan, before);
    ai.air_surge_join_declared_war(&g, 0, 1);
    ai.air_surge_join_declared_war(&g, 0, 1);
    assert_eq!(ai.air_surge_census.declarations, 1);
}

#[test]
fn urgent_domination_target_opening_war_keeps_the_existing_air_investment() {
    let (mut g, mut ai, _) = staged_domination_denial();
    assert!(!ai.air_surge_opening(&mut g, 0, 1));
    g.at_war.insert((0, 1));
    let counter = ai.choose_air_surge(&g, 0).expect("reachable counterattack");
    assert_eq!(counter.target_player, 1);
    g.turn += 1;
    ai.maintain_air_surge(&g, 0);
    let surge = ai
        .air_surge_plan
        .as_ref()
        .expect("retain the urgent counterattack");
    assert!(surge.opened_at_war);
    assert_eq!(surge.declared_turn, None, "we did not declare this war");
    assert_eq!(surge.appointed_turn, 140);
    assert_eq!(surge.phase, AirSurgePhase::Exploit);
    assert_eq!(ai.air_surge_cooldown_until, 0);
    assert_eq!(ai.air_surge_research_goal(&g, 0), Some(AIR_SURGE_GOAL_TECH));
}

#[test]
fn an_unexpected_nonurgent_war_still_ends_the_elective_air_plan() {
    let (mut g, mut ai, _) = staged_domination_denial();
    g.players[1].dvp = 0;
    assert!(!ai.urgent_victory_threat(&g, 1));
    g.at_war.insert((0, 1));
    ai.maintain_air_surge(&g, 0);
    assert!(ai.air_surge_plan.is_none());
    assert!(ai.air_surge_cooldown_until > g.turn);
}
