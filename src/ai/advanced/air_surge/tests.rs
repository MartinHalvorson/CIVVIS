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
