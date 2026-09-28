use super::super::{GrandStrategy, StrategicPlan};
use super::*;
use crate::name;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new_full(2, 40, 24, 936000, 1000, 0, false);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    let first = g.found_city_for(0, (6, 12), None);
    let second = g.found_city_for(0, (12, 12), None);
    let target = g.found_city_for(1, (20, 12), None);
    for cid in [first, second] {
        g.cities.get_mut(&cid).unwrap().pop = 12;
    }
    let ancestors = g.rules.tech_ancestors[AIR_SURGE_GOAL_TECH].clone();
    for tech in ancestors {
        g.players[0].techs.insert(Name::new(&tech));
    }
    g.players[0].techs.insert(Name::new(AIR_SURGE_GOAL_TECH));
    g.players[0]
        .strategic_resources
        .insert(name!("aluminum"), 400.0);
    g.players[0].gold = 10000.0;
    g.players[0].gold_per_turn = 100.0;
    g.players[0].met.insert(1);
    g.at_war.clear();
    g.turn = 150;
    g.current = 0;
    let mut ai = AdvancedAi::new();
    ai.enable_air_surge_2();
    ai.air_surge_plan = Some(AirSurge {
        target_player: 1,
        objective_city: target,
        objective_pos: g.cities[&target].pos,
        body_unit: name!("musketman"),
        body_is_cavalry: false,
        opened_at_war: false,
        phase: AirSurgePhase::Arm,
        appointed_turn: 140,
        tech_turn: Some(150),
        declared_turn: None,
        last_reviewed_turn: 150,
        recovery_assessments: 0,
    });
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: 150,
        rush: false,
    };
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    (g, ai, plan, first, second)
}

fn stealth_fixture() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let (mut g, ai, plan, first, second) = fixture();
    for tech in g.rules.tech_ancestors["stealth_technology"].clone() {
        g.players[0].techs.insert(Name::new(&tech));
    }
    g.players[0].techs.insert(name!("stealth_technology"));
    for cid in [first, second] {
        let pos = g.district_sites(cid, name!("aerodrome"))[0];
        g.cities
            .get_mut(&cid)
            .unwrap()
            .districts
            .insert(name!("aerodrome"), pos);
        g.map.tiles.get_mut(&pos).unwrap().district = Some(name!("aerodrome"));
    }
    (g, ai, plan, first, second)
}

#[test]
fn launch_escort_starts_before_the_third_bomber() {
    let (mut g, mut ai, _, first, second) = fixture();
    g.players[0].civ = "Gran Colombia".to_string();
    g.players[0].techs.insert(name!("military_science"));
    g.players[0]
        .strategic_resources
        .insert(name!("horses"), 100.0);
    ai.air_surge_plan.as_mut().unwrap().body_unit = name!("llanero");
    ai.air_surge_plan.as_mut().unwrap().body_is_cavalry = true;
    for cid in [first, second] {
        let source = *g.cities[&cid]
            .owned_tiles
            .iter()
            .find(|pos| **pos != g.cities[&cid].pos)
            .unwrap();
        let tile = g.map.tiles.get_mut(&source).unwrap();
        tile.resource = Some(name!("aluminum"));
        tile.improvement = Some(name!("mine"));
    }
    let field = g.district_sites(first, name!("aerodrome"))[0];
    g.cities
        .get_mut(&first)
        .unwrap()
        .districts
        .insert(name!("aerodrome"), field);
    g.map.tiles.get_mut(&field).unwrap().district = Some(name!("aerodrome"));
    for cid in [first, second] {
        g.spawn_test_unit("bomber", 0, g.cities[&cid].pos);
    }
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert_eq!(AdvancedAi::air_surge_bomber_goal(&g, 0), 4);
    assert_eq!(ai.air_surge_status.bombers_committed, 2);
    assert_eq!(ai.air_surge_status.bodies_committed, 0);
    assert!(g.can_produce(
        0,
        first,
        &Item::Unit {
            unit: name!("llanero")
        }
    ));
    assert!(g.can_produce(
        0,
        first,
        &Item::Unit {
            unit: name!("bomber")
        }
    ));

    assert!(ai.air_surge_production(&mut g, 0));
    let escort_city = g
        .player_city_ids(0)
        .into_iter()
        .find(|cid| !g.cities[cid].queue.is_empty())
        .unwrap();
    assert_eq!(
        g.cities[&escort_city].queue.first(),
        Some(&Item::Unit {
            unit: name!("llanero")
        }),
        "two committed bombers must release the first land capturer"
    );

    g.cities.get_mut(&escort_city).unwrap().queue.clear();
    for pos in [(7, 12), (8, 12)] {
        g.spawn_test_unit("llanero", 0, pos);
    }
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert_eq!(ai.air_surge_status.bodies_committed, 2);
    assert!(ai.air_surge_production(&mut g, 0));
    assert_eq!(
        g.cities[&first].queue.first(),
        Some(&Item::Unit {
            unit: name!("bomber")
        }),
        "the third bomber resumes after the launch escort is covered"
    );
}

#[test]
fn active_package_keeps_existing_and_queued_bomber_generations() {
    let (mut g, mut ai, plan, first, second) = stealth_fixture();
    g.spawn_test_unit("jet_bomber", 0, g.cities[&first].pos);
    g.spawn_test_unit("fighter", 0, g.cities[&second].pos);
    let queued = Item::Formation {
        unit: name!("jet_bomber"),
        formation: 1,
    };
    g.cities.get_mut(&second).unwrap().queue = vec![queued.clone()];
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert_eq!(
        ai.air_surge_status.bombers, 1,
        "the upgraded aircraft still supplies the wing"
    );
    assert_eq!(
        ai.air_surge_status.bombers_committed, 2,
        "a queued successor supplies one aircraft, including a formation"
    );
    assert_eq!(ai.air_surge_standing_package(&g, 0).1, 1);
    let legal_queue = Item::Unit {
        unit: name!("jet_bomber"),
    };
    g.cities.get_mut(&second).unwrap().queue = vec![legal_queue.clone()];
    // Preserve a nearly finished successor, like the observed native queue.
    g.cities.get_mut(&second).unwrap().production =
        g.item_cost_for_city(0, second, &legal_queue) - 10.0;
    let value = ai.production_value(&g, 0, second, &legal_queue, &plan, &ai.counts(&g, 0));
    assert!(
        value > 7000.0,
        "the final successor queue lost package priority: {value}"
    );
    ai.preempt_margin = 1.25;
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&second].queue.first(), Some(&legal_queue));
    g.cities.get_mut(&second).unwrap().queue = vec![Item::Unit {
        unit: name!("bomber"),
    }];
    g.cities.get_mut(&second).unwrap().production = 0.0;
    assert_eq!(
        ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap())
            .bombers_committed,
        2
    );
}

#[test]
fn two_upgraded_bombers_fill_the_launch_quota_without_ordering_a_third() {
    let (mut g, mut ai, _, first, second) = stealth_fixture();
    for cid in [first, second] {
        g.spawn_test_unit("jet_bomber", 0, g.cities[&cid].pos);
    }
    for pos in [(6, 12), (7, 12), (8, 12), (9, 12)] {
        g.spawn_test_unit("modern_armor", 0, pos);
    }
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert_eq!(AdvancedAi::air_surge_bomber_goal(&g, 0), 2);
    assert!(ai.air_surge_status.wing_ready());
    assert!(!ai.air_surge_production(&mut g, 0));
    assert!(g
        .player_city_ids(0)
        .into_iter()
        .all(|cid| g.cities[&cid].queue.is_empty()));
}

#[test]
fn active_package_trains_a_legal_successor_after_the_base_bomber_is_obsolete() {
    let (mut g, mut ai, _, first, _) = stealth_fixture();
    assert!(g.can_produce(
        0,
        first,
        &Item::Unit {
            unit: name!("jet_bomber")
        }
    ));
    assert!(!g.can_produce(
        0,
        first,
        &Item::Unit {
            unit: name!("bomber")
        }
    ));
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert!(ai.air_surge_production(&mut g, 0));
    assert!(
        g.player_city_ids(0)
            .into_iter()
            .any(|cid| g.cities[&cid].queue.first()
                == Some(&Item::Unit {
                    unit: name!("jet_bomber")
                })),
        "the active package must use a trainable bomber generation"
    );
}

#[test]
fn two_upgraded_bombers_use_their_range_without_treating_fighters_as_a_wing() {
    let (mut g, mut ai, _, first, second) = stealth_fixture();
    let objective = (12 + g.rules.units["bomber"].range + 2, 12);
    let closest = [first, second]
        .into_iter()
        .map(|cid| g.wdist(g.cities[&cid].pos, objective))
        .min()
        .unwrap();
    assert!(closest > g.rules.units["bomber"].range);
    assert!(closest <= g.rules.units["jet_bomber"].range);
    ai.air_surge_plan.as_mut().unwrap().objective_pos = objective;
    for cid in [first, second] {
        g.spawn_test_unit("jet_bomber", 0, g.cities[&cid].pos);
    }
    let status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert_eq!(status.bombers, 2);
    assert!(
        status.wing_ready(),
        "two upgraded bombers can reach the appointed objective"
    );
    for uid in g.player_unit_ids(0) {
        g.remove_unit(uid);
    }
    for cid in [first, second] {
        g.spawn_test_unit("fighter", 0, g.cities[&cid].pos);
    }
    let status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert_eq!(status.bombers, 0);
    assert!(!status.wing_ready());
}

#[test]
fn a_mixed_wing_needs_two_aircraft_with_enough_range() {
    let (mut g, mut ai, _, first, second) = stealth_fixture();
    let objective = (12 + g.rules.units["bomber"].range + 2, 12);
    ai.air_surge_plan.as_mut().unwrap().objective_pos = objective;
    g.spawn_test_unit("bomber", 0, g.cities[&first].pos);
    g.spawn_test_unit("jet_bomber", 0, g.cities[&second].pos);
    let status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert_eq!(status.bombers, 2);
    assert!(
        !status.wing_ready(),
        "one long-range aircraft does not supply a two-plane strike"
    );
}

#[test]
fn reserved_airfield_keeps_priority_after_status_refresh() {
    let (mut g, mut ai, plan, _, _) = fixture();
    assert!(ai.air_surge_production(&mut g, 0));
    assert_eq!(ai.air_surge_status.aerodromes_committed, 1);
    let cid = g
        .player_city_ids(0)
        .into_iter()
        .find(|cid| !g.cities[cid].queue.is_empty())
        .unwrap();
    let item = g.cities[&cid].queue[0].clone();
    assert!(matches!(item, Item::District { .. }));
    let value = ai.production_value(&g, 0, cid, &item, &plan, &ai.counts(&g, 0));
    assert!(value > 7000.0, "reserved airfield lost priority: {value}");
    ai.preempt_margin = 1.25;
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&cid].queue.first(), Some(&item));
}

#[test]
fn final_queued_bomber_retains_priority_without_ordering_a_third() {
    let (mut g, mut ai, plan, first, second) = fixture();
    let bomber = Item::Unit {
        unit: name!("bomber"),
    };
    g.spawn_test_unit("bomber", 0, (6, 12));
    g.cities.get_mut(&first).unwrap().queue = vec![bomber.clone()];
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert_eq!(AdvancedAi::air_surge_bomber_goal(&g, 0), 2);
    assert_eq!(ai.air_surge_status.bombers_committed, 2);
    let counts = ai.counts(&g, 0);
    let committed = ai.production_value(&g, 0, first, &bomber, &plan, &counts);
    let duplicate = ai.production_value(&g, 0, second, &bomber, &plan, &counts);
    assert!(
        committed > duplicate + 5000.0,
        "last bomber lost priority: {committed}"
    );
    assert!(
        duplicate < 7000.0,
        "third bomber gained package priority: {duplicate}"
    );
}

#[test]
fn second_queued_airfield_is_reserved_only_until_the_launch_wing_is_committed() {
    let (mut g, mut ai, _, first, second) = fixture();
    let field =
        |g: &Game, cid| {
            g.producible_items(0, cid).into_iter().find(|item|
        matches!(item, Item::District { district, .. } if district == "aerodrome")).unwrap()
        };
    let a = field(&g, first);
    let b = field(&g, second);
    g.cities.get_mut(&first).unwrap().queue = vec![a];
    g.cities.get_mut(&second).unwrap().queue = vec![b.clone()];
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert_eq!(ai.air_surge_status.aerodromes_committed, 2);
    assert!(ai
        .air_surge_city_production_value(&g, 0, second, &b, 2.0)
        .is_some());
    let third = g.found_city_for(0, (6, 18), None);
    g.cities.get_mut(&third).unwrap().pop = 12;
    let c = field(&g, third);
    g.cities.get_mut(&third).unwrap().queue = vec![c.clone()];
    assert_eq!(
        ai.air_surge_city_production_value(&g, 0, third, &c, 2.0),
        None
    );
    g.cities.get_mut(&third).unwrap().queue.clear();
    g.spawn_test_unit("bomber", 0, (6, 12));
    g.spawn_test_unit("bomber", 0, (12, 12));
    assert_eq!(
        ai.air_surge_city_production_value(&g, 0, second, &b, 2.0),
        None
    );
    ai.air_surge_plan = None;
    assert_eq!(
        ai.air_surge_city_production_value(&g, 0, second, &b, 2.0),
        None
    );
}

#[test]
fn capture_body_quota_preserves_its_last_queue_but_releases_excess() {
    let (mut g, ai, _, first, second) = fixture();
    let body = Item::Unit {
        unit: name!("musketman"),
    };
    for pos in [(6, 12), (7, 12), (8, 12)] {
        g.spawn_test_unit("musketman", 0, pos);
    }
    g.cities.get_mut(&first).unwrap().queue = vec![body.clone()];
    assert!(ai
        .air_surge_city_production_value(&g, 0, first, &body, 2.0)
        .is_some());
    assert_eq!(
        ai.air_surge_city_production_value(&g, 0, second, &body, 2.0),
        None
    );
    g.spawn_test_unit("musketman", 0, (9, 12));
    assert_eq!(
        ai.air_surge_city_production_value(&g, 0, first, &body, 2.0),
        None
    );
}
