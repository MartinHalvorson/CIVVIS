use super::super::*;
use std::sync::Arc;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, u32, Pos) {
    let mut g = Game::new_full(2, 32, 20, 367_600, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    let base = (2, 8);
    g.found_city_for(0, base, None);
    g.found_city_for(0, (9, 4), None);
    g.found_city_for(1, (24, 4), None);
    let objective = g.found_city_for(1, (13, 8), None);
    g.at_war.insert((0, 1));
    g.current = 0;
    g.turn = 154;
    let city = g.cities.get_mut(&objective).unwrap();
    city.wall_hp = 398;
    city.hp = 200;
    Arc::make_mut(&mut g.observed_city_strength).insert(objective, 90.9141);
    Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(objective, 400);
    let bomber = g.spawn_test_unit("bomber", 0, base);
    let taker = g.spawn_test_unit("tank", 0, (12, 8));
    let mine = (11, 7);
    let tile = g.map.tiles.get_mut(&mine).unwrap();
    tile.owner_city = Some(objective);
    tile.improvement = Some(crate::name!("mine"));
    tile.pillaged = false;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(objective),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    assert_eq!(g.wdist(base, g.cities[&objective].pos), 11);
    assert_eq!(g.unit_attack_range(bomber), 10);
    assert!(!g.cities[&objective].is_capital);
    assert_eq!(g.city_max_wall_hp(&g.cities[&objective]), 400);
    assert!(g.legal_actions(0).contains(&Action::AirPillage {
        unit: bomber,
        target: mine
    }));
    (
        g,
        AdvancedAi::targeting(VictoryTarget::Domination),
        plan,
        bomber,
        taker,
        mine,
    )
}

#[test]
fn a_siege_bomber_rebases_for_a_better_city_strike() {
    let (g, ai, plan, bomber, _, _) = fixture();
    let action = ai.advanced_air_action(&g, 0, bomber, &plan);
    assert_eq!(
        action,
        Some(Action::AirRebase {
            unit: bomber,
            to: (9, 4)
        })
    );
    assert_eq!(
        g.units[&bomber].pos,
        (2, 8),
        "forecast must not move the real aircraft"
    );
}

#[test]
fn a_city_without_a_nearby_capture_unit_keeps_the_current_mission() {
    let (mut g, ai, plan, bomber, taker, mine) = fixture();
    g.units.get_mut(&taker).unwrap().kind = crate::name!("field_cannon");
    assert_eq!(
        ai.advanced_air_action(&g, 0, bomber, &plan),
        Some(Action::AirPillage {
            unit: bomber,
            target: mine
        })
    );
}

#[test]
fn a_valuable_immediate_kill_still_precedes_rebasing() {
    let (mut g, ai, plan, bomber, _, _) = fixture();
    let target = (10, 8);
    let enemy = g.spawn_test_unit("artillery", 1, target);
    g.units.get_mut(&enemy).unwrap().hp = 1;
    assert_eq!(
        ai.advanced_air_action(&g, 0, bomber, &plan),
        Some(Action::AirStrike {
            unit: bomber,
            target
        })
    );
}

#[test]
fn other_victory_lanes_keep_the_current_air_mission() {
    let (g, _, plan, bomber, _, mine) = fixture();
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    assert_eq!(
        ai.advanced_air_action(&g, 0, bomber, &plan),
        Some(Action::AirPillage {
            unit: bomber,
            target: mine
        })
    );
}

#[test]
fn an_ineffective_city_strike_does_not_displace_a_useful_mission() {
    let (mut g, ai, plan, bomber, _, mine) = fixture();
    Arc::make_mut(&mut g.observed_city_strength).insert(plan.target_city.unwrap(), 200.0);
    assert_eq!(
        ai.advanced_air_action(&g, 0, bomber, &plan),
        Some(Action::AirPillage {
            unit: bomber,
            target: mine
        })
    );
}

#[test]
fn home_defense_keeps_the_current_mission() {
    let (g, ai, mut plan, bomber, _, mine) = fixture();
    plan.threatened_city = g.player_city_ids(0).first().copied();
    assert_eq!(
        ai.advanced_air_action(&g, 0, bomber, &plan),
        Some(Action::AirPillage {
            unit: bomber,
            target: mine
        })
    );
}
