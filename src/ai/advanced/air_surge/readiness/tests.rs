use super::*;

fn fixture() -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new_full(2, 40, 24, 371500, 2000, 0, false);
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
        tile.improvement = None;
    }
    let first = g.found_city_for(0, (6, 12), None);
    let second = g.found_city_for(0, (12, 12), None);
    g.found_city_for(1, (24, 12), None);
    for tech in g.rules.tech_ancestors[AIR_SURGE_GOAL_TECH].clone() {
        g.players[0].techs.insert(Name::new(&tech));
    }
    g.players[0].techs.insert(crate::name!("advanced_flight"));
    g.players[0].techs.insert(crate::name!("currency"));
    g.players[0]
        .strategic_resources
        .insert(crate::name!("aluminum"), 100.0);
    g.players[0].gold = 1000.0;
    g.players[0].gold_per_turn = 30.0;
    g.at_war.clear();
    g.current = 0;
    g.turn = 150;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    assert!(ai.air_surge_plan.is_none());
    (g, ai, first, second)
}

#[test]
fn peace_keeps_research_and_reserves_a_legal_first_airfield() {
    let (mut g, mut ai, first, _) = fixture();
    g.players[0].techs.remove(&crate::name!("advanced_flight"));
    assert_eq!(ai.air_surge_research_goal(&g, 0), Some(AIR_SURGE_GOAL_TECH));
    let hub = Item::District {
        district: crate::name!("commercial_hub"),
        pos: g.district_sites(first, crate::name!("commercial_hub"))[0],
    };
    assert!(
        ai.air_surge_city_production_value(&g, 0, first, &hub, 5.0)
            .unwrap()
            < 0.0
    );
    assert!(ai.air_surge_production(&mut g, 0));
    let item = g.cities[&first].queue.first().unwrap().clone();
    assert!(matches!(item, Item::District { district, .. } if district == "aerodrome"));
    assert!(
        ai.air_surge_city_production_value(&g, 0, first, &item, 5.0)
            .unwrap()
            > 7000.0
    );
    assert!(
        !ai.air_surge_production(&mut g, 0),
        "one base fulfils preparation"
    );
    assert!(ai.air_surge_plan.is_none());
    assert!(!g.is_at_war(0, 1));
}

#[test]
fn queues_two_bombers_and_retains_the_last_without_ordering_a_third() {
    let (mut g, mut ai, first, second) = fixture();
    crate::game::install_test_district(&mut g, first, "aerodrome");
    crate::game::install_test_district(&mut g, second, "aerodrome");
    assert!(ai.air_surge_production(&mut g, 0));
    assert!(ai.air_surge_production(&mut g, 0));
    assert!(!ai.air_surge_production(&mut g, 0));
    let bomber = Item::Unit {
        unit: crate::name!("bomber"),
    };
    assert_eq!(g.cities[&first].queue.first(), Some(&bomber));
    assert_eq!(g.cities[&second].queue.first(), Some(&bomber));
    assert!(
        ai.air_surge_city_production_value(&g, 0, first, &bomber, 5.0)
            .unwrap()
            > 7000.0
    );
}

#[test]
fn science_and_preindustrial_seats_keep_their_original_priorities() {
    let (mut g, mut ai, _, _) = fixture();
    g.players[0]
        .techs
        .remove(&crate::name!("industrialization"));
    assert!(!ai.air_surge_production(&mut g, 0));
    assert_eq!(ai.air_surge_research_goal(&g, 0), None);
    g.players[0].techs.insert(crate::name!("industrialization"));
    ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_air_surge_2();
    assert!(!ai.air_surge_production(&mut g, 0));
}

#[test]
fn completed_newer_bombers_end_preparation() {
    let (mut g, mut ai, first, second) = fixture();
    g.spawn_test_unit("jet_bomber", 0, g.cities[&first].pos);
    g.spawn_test_unit("jet_bomber", 0, g.cities[&second].pos);
    assert!(!ai.air_surge_production(&mut g, 0));
    assert_eq!(ai.air_surge_research_goal(&g, 0), None);
}

#[test]
fn aircraft_need_fuel_and_an_upkeep_reserve() {
    // Each observation gets a fresh board. Direct fixture mutations after a
    // menu query would otherwise leave its cached production catalog stale.
    for case in 0..3 {
        let (mut g, mut ai, first, _) = fixture();
        crate::game::install_test_district(&mut g, first, "aerodrome");
        if case == 0 {
            g.players[0].strategic_resources.clear();
        }
        if case == 1 {
            g.players[0].gold = 20.0;
        }
        g.players[0].gold_per_turn = -10.0;
        assert_eq!(ai.air_surge_production(&mut g, 0), case == 2);
    }
}

#[test]
fn preparation_does_not_displace_an_occupied_queue() {
    let (mut g, mut ai, first, second) = fixture();
    let item = Item::Building {
        building: crate::name!("granary"),
    };
    for cid in [first, second] {
        g.cities.get_mut(&cid).unwrap().queue = vec![item.clone()];
    }
    assert!(!ai.air_surge_production(&mut g, 0));
    assert_eq!(g.cities[&first].queue.first(), Some(&item));
    assert_eq!(g.cities[&second].queue.first(), Some(&item));
}

#[test]
fn an_immediate_home_threat_suspends_preparation() {
    let (mut g, mut ai, first, _) = fixture();
    g.at_war.insert((0, 1));
    let pos = g.cities[&first].pos;
    g.spawn_test_unit("modern_armor", 1, (pos.0 + 1, pos.1));
    g.spawn_test_unit("modern_armor", 1, (pos.0, pos.1 + 1));
    assert!(ai.threatened_city(&g, 0).is_some());
    assert_eq!(ai.air_surge_research_goal(&g, 0), None);
    assert!(!ai.air_surge_production(&mut g, 0));
}
