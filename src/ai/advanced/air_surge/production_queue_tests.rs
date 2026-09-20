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
