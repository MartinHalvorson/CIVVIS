use super::*;
use crate::ai::advanced::VictoryTarget;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, Vec<u32>) {
    let mut g = Game::new_full(2, 40, 24, 377300, 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (10, 10), None);
    let target = g.found_city_for(1, (20, 10), None);
    g.current = 0;
    g.at_war.insert((0, 1));
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    g.players[0]
        .strategic_resources
        .insert(crate::name!("aluminum"), 40.0);
    let cavalry = g.spawn_test_unit("cavalry", 0, (16, 10));
    let bombers = (0..2)
        .map(|_| g.spawn_test_unit("bomber", 0, (10, 10)))
        .collect();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: 1,
        rush: false,
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.air_surge = true;
    (g, ai, plan, cavalry, bombers)
}

#[test]
fn cavalry_reveals_a_city_before_the_bombers_and_keeps_an_exit() {
    let (mut g, mut ai, plan, cavalry, _) = fixture();
    let target = g.cities[&plan.target_city.unwrap()].pos;
    assert!(
        !g.player_can_see(0, target),
        "remembered city starts outside sight"
    );
    let city = g.cities.get_mut(&plan.target_city.unwrap()).unwrap();
    city.wall_hp = 400;
    city.hp = 200;
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(city.id, 400);
    // Give the city enough observed strength that two sorties cannot capture it.
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(city.id, 100.0);
    let before = g.log.len();
    let reserved = ai.plan_air_city_assault(&mut g, 0, &plan);
    assert!(reserved.contains(&cavalry));
    let report = ai.planned_air_city_assault().unwrap();
    assert!(report.moved_to_spot);
    assert_eq!(report.aircraft.len(), 2);
    assert_eq!(
        g.units[&cavalry].pos,
        (16, 10),
        "return before the enemy turn"
    );
    let actions: Vec<_> = g
        .log
        .iter()
        .skip(before)
        .map(|(_, action)| action)
        .collect();
    let first_bomb = actions
        .iter()
        .position(|a| matches!(a, Action::AirStrike { .. }))
        .unwrap();
    let last_bomb = actions
        .iter()
        .rposition(|a| matches!(a, Action::AirStrike { .. }))
        .unwrap();
    assert!(first_bomb > 0);
    assert!(last_bomb + 1 < actions.len(), "retreat follows the volley");
    assert!(g.cities[&plan.target_city.unwrap()].wall_hp < 400);
}

#[test]
fn a_weakened_city_is_captured_after_the_sorties() {
    let (mut g, mut ai, plan, cavalry, _) = fixture();
    let cid = plan.target_city.unwrap();
    g.cities.get_mut(&cid).unwrap().hp = 30;
    let reserved = ai.plan_air_city_assault(&mut g, 0, &plan);
    assert!(reserved.contains(&cavalry));
    assert_eq!(g.cities[&cid].owner, 0);
    assert_eq!(g.units[&cavalry].pos, (20, 10));
    assert!(!ai.planned_air_city_assault().unwrap().aircraft.is_empty());
}

#[test]
fn no_maneuver_without_a_return_budget_aircraft_or_the_opt_in() {
    for case in ["movement", "aircraft", "opt_in", "peace", "home"] {
        let (mut g, mut ai, mut plan, cavalry, bombers) = fixture();
        match case {
            "movement" => g.units.get_mut(&cavalry).unwrap().moves_left = 2.0,
            "aircraft" => {
                for uid in bombers {
                    g.remove_unit(uid);
                }
            }
            "opt_in" => ai.air_surge = false,
            "peace" => g.at_war.clear(),
            "home" => plan.strategy = GrandStrategy::Recovery,
            _ => unreachable!(),
        }
        let before = g.log.len();
        assert!(
            ai.plan_air_city_assault(&mut g, 0, &plan).is_empty(),
            "{case}"
        );
        assert_eq!(g.log.len(), before, "{case}");
    }
}

#[test]
fn observed_frame_budget_must_cover_both_spotting_and_bombardment() {
    for left in [0, 1, 2] {
        let (mut g, mut ai, plan, _, _) = fixture();
        ai.observe_air_assault_frame(BTreeSet::new(), left);
        let reserved = ai.plan_air_city_assault(&mut g, 0, &plan);
        assert_eq!(!reserved.is_empty(), left == 2, "frames left: {left}");
    }
}

#[test]
fn spent_bombers_allow_capture_from_the_next_observed_board() {
    let (mut g, mut ai, plan, cavalry, bombers) = fixture();
    g.relocate(cavalry, (18, 10));
    let cid = plan.target_city.unwrap();
    g.cities.get_mut(&cid).unwrap().hp = 1;
    for uid in bombers {
        g.units.get_mut(&uid).unwrap().moves_left = 0.0;
    }
    ai.observe_air_assault_frame(BTreeSet::from([(20, 10)]), 0);
    assert!(ai
        .plan_air_city_assault(&mut g, 0, &plan)
        .contains(&cavalry));
    assert_eq!(g.cities[&cid].owner, 0);
    assert!(ai.planned_air_city_assault().unwrap().aircraft.is_empty());
}

#[test]
fn an_observed_failed_breach_makes_the_cavalry_withdraw() {
    let (mut g, mut ai, plan, cavalry, bombers) = fixture();
    g.relocate(cavalry, (18, 10));
    let cid = plan.target_city.unwrap();
    g.cities.get_mut(&cid).unwrap().wall_hp = 400;
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(cid, 400);
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(cid, 100.0);
    std::sync::Arc::make_mut(&mut g.observed_city_ranged_strength).insert(cid, 100.0);
    for uid in bombers {
        g.units.get_mut(&uid).unwrap().moves_left = 0.0;
    }
    ai.observe_air_assault_frame(BTreeSet::from([(20, 10)]), 0);
    assert!(ai
        .plan_air_city_assault(&mut g, 0, &plan)
        .contains(&cavalry));
    assert_eq!(g.cities[&cid].owner, 1);
    assert!(g.wdist(g.units[&cavalry].pos, (20, 10)) > 2);
    assert!(ai.planned_air_city_assault().unwrap().aircraft.is_empty());
}

#[test]
fn refused_sorties_do_not_prevent_the_observed_cavalry_retreat() {
    let (mut g, mut ai, plan, cavalry, bombers) = fixture();
    g.relocate(cavalry, (18, 10));
    let cid = plan.target_city.unwrap();
    g.cities.get_mut(&cid).unwrap().wall_hp = 400;
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(cid, 400);
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(cid, 100.0);
    std::sync::Arc::make_mut(&mut g.observed_city_ranged_strength).insert(cid, 100.0);
    for uid in bombers {
        std::sync::Arc::make_mut(&mut g.blocked_strikes).insert((uid, (20, 10)));
    }
    ai.observe_air_assault_frame(BTreeSet::from([(20, 10)]), 0);
    assert!(ai
        .plan_air_city_assault(&mut g, 0, &plan)
        .contains(&cavalry));
    assert_eq!(g.cities[&cid].owner, 1);
    assert!(g.wdist(g.units[&cavalry].pos, (20, 10)) > 2);
    assert!(ai.planned_air_city_assault().unwrap().aircraft.is_empty());
}

#[test]
fn the_ordinary_unit_loop_does_not_spend_the_reserved_spotter_again() {
    let (mut g, mut ai, plan, cavalry, _) = fixture();
    let cid = plan.target_city.unwrap();
    g.cities.get_mut(&cid).unwrap().wall_hp = 400;
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(cid, 400);
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(cid, 100.0);
    ai.advanced_units(&mut g, 0, &plan);
    assert!(ai.planned_air_city_assault().is_some());
    assert_eq!(g.units[&cavalry].pos, (16, 10));
}
