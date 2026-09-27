use super::super::*;
use std::sync::Arc;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, u32, u32) {
    let mut g = Game::new_full(2, 40, 20, 379_700, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let rear = g.found_city_for(0, (2, 8), None);
    let front = g.found_city_for(0, (14, 8), None);
    let objective = g.found_city_for(1, (22, 8), None);
    g.cities.get_mut(&front).unwrap().loyalty = 5.0;
    g.cities.get_mut(&objective).unwrap().pop = 40;
    g.at_war.insert((0, 1));
    g.current = 0;
    g.turn = 154;
    let bomber = g.spawn_test_unit("bomber", 0, (14, 8));
    let victim = g.spawn_test_unit("artillery", 1, (15, 8));
    g.units.get_mut(&victim).unwrap().hp = 1;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(objective),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.air_surge = true;
    assert!(g.city_loyalty_per_turn(&g.cities[&front]) < -5.0);
    assert!(g.city_loyalty_per_turn(&g.cities[&rear]) >= 0.0);
    let rebase = Action::AirRebase {
        unit: bomber,
        to: (2, 8),
    };
    assert!(g.legal_actions(0).contains(&rebase));
    assert!(ai
        .immediate_kill_value(
            &g,
            0,
            &Action::AirStrike {
                unit: bomber,
                target: (15, 8),
            },
            &plan,
        )
        .is_some());
    (g, ai, plan, bomber, front, victim)
}

fn next_owned_turn(g: &mut Game) {
    loop {
        let pid = g.current;
        g.apply(pid, &Action::EndTurn).unwrap();
        if g.current == 0 {
            break;
        }
    }
}

#[test]
fn actual_dispatch_evacuates_before_a_profitable_kill_and_survives_revolt() {
    let (mut g, mut ai, plan, bomber, front, victim) = fixture();
    let mut stayed = g.clone();
    next_owned_turn(&mut stayed);
    assert_ne!(stayed.cities[&front].owner, 0);
    assert!(!stayed.units.contains_key(&bomber));

    ai.advanced_units(&mut g, 0, &plan);
    next_owned_turn(&mut g);
    assert_ne!(g.cities[&front].owner, 0, "real Loyalty tick must revolt");
    assert!(
        g.units.contains_key(&bomber),
        "evacuated Bomber must survive"
    );
    assert_eq!(g.units[&bomber].pos, (2, 8));
    assert!(
        g.units.contains_key(&victim),
        "safety must precede the kill pass"
    );
}

#[test]
fn joint_battle_prepass_also_leaves_the_evacuated_aircraft_alone() {
    let (mut g, mut ai, plan, bomber, front, victim) = fixture();
    ai.enable_battle_planner_2();
    ai.advanced_units(&mut g, 0, &plan);
    assert!(g.units.contains_key(&victim));
    next_owned_turn(&mut g);
    assert_ne!(g.cities[&front].owner, 0);
    assert_eq!(g.units[&bomber].pos, (2, 8));
}

#[test]
fn upgraded_jets_and_aerodrome_positions_survive_actual_revolt() {
    let (mut g, mut ai, plan, bomber, front, _) = fixture();
    let district = (14, 7);
    g.cities
        .get_mut(&front)
        .unwrap()
        .districts
        .insert(crate::name!("aerodrome"), district);
    let tile = g.map.tiles.get_mut(&district).unwrap();
    tile.owner_city = Some(front);
    tile.district = Some(crate::name!("aerodrome"));
    g.relocate(bomber, district);
    g.units.get_mut(&bomber).unwrap().kind = crate::name!("jet_bomber");
    let mut stayed = g.clone();
    next_owned_turn(&mut stayed);
    assert!(!stayed.units.contains_key(&bomber));
    ai.advanced_units(&mut g, 0, &plan);
    next_owned_turn(&mut g);
    assert_ne!(g.cities[&front].owner, 0);
    assert_eq!(g.units[&bomber].pos, (2, 8));
}

#[test]
fn owned_public_rate_is_enough_when_the_pressure_city_is_hidden() {
    let (g, mut ai, plan, bomber, front, _) = fixture();
    let rate = g.city_loyalty_per_turn(&g.cities[&front]);
    let mut view = g.player_decision_view(0);
    assert!(!view.cities.contains_key(&plan.target_city.unwrap()));
    assert_eq!(view.city_loyalty_per_turn(&view.cities[&front]), rate);
    ai.advanced_units(&mut view, 0, &plan);
    assert_eq!(view.units[&bomber].pos, (2, 8));
}

#[test]
fn stable_source_keeps_the_profitable_kill() {
    let (mut g, mut ai, plan, bomber, front, victim) = fixture();
    g.cities.get_mut(&front).unwrap().loyalty = 100.0;
    ai.advanced_units(&mut g, 0, &plan);
    assert!(!g.units.contains_key(&victim));
    assert_eq!(g.units[&bomber].pos, (14, 8));
    next_owned_turn(&mut g);
    assert_eq!(g.cities[&front].owner, 0);
    assert!(g.units.contains_key(&bomber));
}

#[test]
fn off_and_other_victory_lanes_preserve_the_attack_first_dispatch() {
    for target in [
        None,
        Some(VictoryTarget::Science),
        Some(VictoryTarget::Domination),
    ] {
        let (mut g, _, plan, bomber, front, victim) = fixture();
        let mut ai = target.map_or_else(AdvancedAi::new, AdvancedAi::targeting);
        ai.air_surge = target != Some(VictoryTarget::Domination);
        ai.advanced_units(&mut g, 0, &plan);
        assert!(!g.units.contains_key(&victim));
        assert_eq!(g.units[&bomber].pos, (14, 8));
        next_owned_turn(&mut g);
        assert_ne!(g.cities[&front].owner, 0);
        assert!(!g.units.contains_key(&bomber));
    }
}

#[test]
fn a_full_safe_base_cannot_receive_an_illegal_evacuation() {
    let (mut g, mut ai, plan, bomber, _, victim) = fixture();
    g.spawn_test_unit("fighter", 0, (2, 8));
    assert!(!g
        .legal_doctrine_actions(0, bomber)
        .iter()
        .any(|action| { matches!(action, Action::AirRebase { .. }) }));
    ai.advanced_units(&mut g, 0, &plan);
    assert!(!g.units.contains_key(&victim));
    assert_eq!(g.units[&bomber].pos, (14, 8));
}

#[test]
fn multiple_aircraft_cannot_reserve_the_same_last_slot() {
    let (mut g, ai, plan, bomber, _, _) = fixture();
    let second = g.spawn_test_unit("bomber", 0, (14, 8));
    let reserved = ai.evacuate_air_bases(&mut g, 0, &plan);
    assert_eq!(reserved, BTreeSet::from([bomber]));
    assert_eq!(g.units[&bomber].pos, (2, 8));
    assert_eq!(g.units[&second].pos, (14, 8));
}

#[test]
fn a_destination_must_survive_arrival_and_the_next_sortie() {
    let (mut g, ai, plan, bomber, _, _) = fixture();
    let trap = g.found_city_for(0, (11, 4), None);
    Arc::make_mut(&mut g.observed_city_loyalty_per_turn).insert(trap, -17.0);
    g.cities.get_mut(&trap).unwrap().loyalty = 33.0;
    assert!(g
        .legal_doctrine_actions(0, bomber)
        .contains(&Action::AirRebase {
            unit: bomber,
            to: (11, 4),
        }));
    assert!(!ai.air_base_rebase_allowed(&g, 0, bomber, (11, 4)));
    let reserved = ai.evacuate_air_bases(&mut g, 0, &plan);
    assert_eq!(reserved, BTreeSet::from([bomber]));
    assert_eq!(g.units[&bomber].pos, (2, 8));
}

#[test]
fn ordinary_rebase_choice_excludes_a_front_base_without_an_operating_turn() {
    let (mut g, mut ai, plan, bomber, front, _) = fixture();
    g.relocate(bomber, (2, 8));
    g.cities.get_mut(&front).unwrap().loyalty = 33.0;
    Arc::make_mut(&mut g.observed_city_loyalty_per_turn).insert(front, -17.0);
    assert_eq!(ai.advanced_air_action(&g, 0, bomber, &plan), None);
    ai.air_surge = false;
    let expected = Some(Action::AirRebase {
        unit: bomber,
        to: (14, 8),
    });
    assert_eq!(ai.advanced_air_action(&g, 0, bomber, &plan), expected);
    ai.air_surge = true;
    Arc::make_mut(&mut g.observed_city_loyalty_per_turn).insert(front, -16.0);
    assert_eq!(ai.advanced_air_action(&g, 0, bomber, &plan), expected);
}

#[test]
fn spent_planes_and_unknown_source_rates_do_not_invent_a_rebase() {
    let (mut g, ai, plan, bomber, front, _) = fixture();
    g.units.get_mut(&bomber).unwrap().moves_left = 0.0;
    assert!(ai.evacuate_air_bases(&mut g, 0, &plan).is_empty());
    g.units.get_mut(&bomber).unwrap().moves_left = 2.0;
    Arc::make_mut(&mut g.observed_city_loyalty_per_turn).insert(front, f64::NAN);
    assert!(ai.evacuate_air_bases(&mut g, 0, &plan).is_empty());
    assert_eq!(g.units[&bomber].pos, (14, 8));
}

#[test]
fn carriers_and_airstrips_do_not_inherit_city_center_destruction() {
    let (mut g, ai, plan, bomber, front, _) = fixture();
    let base = (14, 7);
    let tile = g.map.tiles.get_mut(&base).unwrap();
    tile.owner_city = Some(front);
    tile.improvement = Some(crate::name!("airstrip"));
    g.relocate(bomber, base);
    assert!(ai.evacuate_air_bases(&mut g, 0, &plan).is_empty());
    g.map.tiles.get_mut(&base).unwrap().improvement = None;
    g.spawn_test_unit("aircraft_carrier", 0, base);
    assert!(ai.evacuate_air_bases(&mut g, 0, &plan).is_empty());
    assert_eq!(g.units[&bomber].pos, base);
}
