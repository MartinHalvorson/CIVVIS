use super::bomber_wing::SURGE_BOMBER_BUY_RESERVE;
use super::*;
use crate::name;

/// Two cities before Flight, the first the more productive: the reservation
/// keeps its last specialty slot for the airfield (see `field_slot_tests`).
fn slot_fixture() -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new_full(2, 40, 24, 370700, 500, 0, false);
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
    let target = g.found_city_for(1, (22, 12), None);
    g.players[0].techs.insert(name!("currency"));
    g.current = 0;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    ai.air_surge_plan = Some(AirSurge {
        target_player: 1,
        objective_city: target,
        objective_pos: (22, 12),
        body_unit: name!("knight"),
        body_is_cavalry: true,
        opened_at_war: false,
        phase: AirSurgePhase::Beeline,
        appointed_turn: 1,
        tech_turn: None,
        declared_turn: None,
        last_reviewed_turn: 1,
        recovery_assessments: 0,
    });
    assert!(g.city_yields(first).production > g.city_yields(second).production);
    (g, ai, first, second)
}

fn hub(g: &Game, city: u32) -> Item {
    Item::District {
        district: name!("commercial_hub"),
        pos: g.district_sites(city, name!("commercial_hub"))[0],
    }
}

/// Advanced Flight known, Aluminum banked and Gold in hand; the first city
/// holds the airfield.
fn wing_fixture() -> (Game, AdvancedAi, u32, u32) {
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
    g.players[0].gold = 10_000.0;
    g.players[0].gold_per_turn = 100.0;
    g.players[0].met.insert(1);
    g.at_war.clear();
    g.turn = 150;
    g.current = 0;
    let field = g.district_sites(first, name!("aerodrome"))[0];
    g.cities
        .get_mut(&first)
        .unwrap()
        .districts
        .insert(name!("aerodrome"), field);
    g.map.tiles.get_mut(&field).unwrap().district = Some(name!("aerodrome"));
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
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    (g, ai, first, second)
}

fn bombers(g: &Game) -> usize {
    g.units
        .values()
        .filter(|unit| unit.owner == 0 && unit.kind == "bomber")
        .count()
}

#[test]
fn the_gene_off_holds_no_slot() {
    let (mut g, mut ai, first, _) = slot_fixture();
    assert!(ai.air_surge_reserves_field_slot(&g, 0, first, &hub(&g, first)));
    assert!(ai.surge_field_slot_refusals(&g, 0).is_empty());
    assert!(ai.surge_hold_field_slot(&mut g, 0).is_none());
    assert!(g.can_produce(0, first, &hub(&g, first)));
}

#[test]
fn the_reserved_slot_is_refused_to_every_governor_for_the_pass() {
    let (mut g, mut ai, first, second) = slot_fixture();
    ai.enable_surge_fields_the_bombers();
    let first_hub = hub(&g, first);
    let second_hub = hub(&g, second);
    let refusals = ai.surge_field_slot_refusals(&g, 0);
    assert_eq!(refusals.keys().copied().collect::<Vec<_>>(), vec![first]);
    let held = ai.surge_hold_field_slot(&mut g, 0);
    assert!(held.is_some());
    // The delegated governor's Commercial Hub, the strategic-queue steps
    // and every reservation read this same menu.
    assert!(!g.can_produce(0, first, &first_hub));
    assert!(!g.producible_items(0, first).iter().any(
        |item| matches!(item, Item::District { district, .. } if *district == "commercial_hub")
    ));
    assert!(g.can_produce(0, second, &second_hub));
    assert!(g
        .apply(
            0,
            &Action::Produce {
                city: first,
                item: first_hub.clone(),
            },
        )
        .is_err());
    AdvancedAi::surge_release_field_slot(&mut g, held);
    assert!(g.blocked_production.is_empty());
    assert!(g.blocked_districts.is_empty());
    assert!(g.can_produce(0, first, &first_hub));
}

#[test]
fn an_airfield_holds_no_further_slot() {
    let (mut g, mut ai, first, second) = slot_fixture();
    ai.enable_surge_fields_the_bombers();
    let field = Item::District {
        district: name!("aerodrome"),
        pos: g.district_sites(second, name!("aerodrome"))[0],
    };
    g.cities.get_mut(&second).unwrap().queue.push(field);
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert!(ai.surge_field_slot_refusals(&g, 0).is_empty());
    assert!(g.can_produce(0, first, &hub(&g, first)));
}

#[test]
fn the_airfield_is_ranked_with_the_launch_wing() {
    let (g, mut ai, first, _) = wing_fixture();
    assert_eq!(ai.surge_launch_wing_turns(&g, 0, first), 0.0);
    ai.enable_surge_fields_the_bombers();
    let bomber = Item::Unit {
        unit: name!("bomber"),
    };
    let rate = g.city_yields(first).production * g.item_prod_mult(0, first, Some(&bomber));
    let expected =
        AIR_SURGE_LAUNCH_BOMBERS as f64 * g.item_cost_for_city(0, first, &bomber) / rate.max(0.1);
    assert!((ai.surge_launch_wing_turns(&g, 0, first) - expected).abs() < 1e-9);
    assert!(expected > 0.0);
}

#[test]
fn a_second_field_may_carry_the_whole_wing() {
    let (_, mut ai, _, _) = wing_fixture();
    assert_eq!(ai.surge_wing_missing(2, 4), 2);
    assert_eq!(ai.surge_wing_missing(0, 3), 0);
    ai.enable_surge_fields_the_bombers();
    assert_eq!(ai.surge_wing_missing(2, 4), 4);
    assert_eq!(ai.surge_wing_missing(0, 3), 3);
    assert_eq!(ai.surge_wing_missing(2, 0), 2);
}

#[test]
fn an_airfield_city_buys_a_bomber_when_the_treasury_carries_it() {
    let (mut g, mut ai, first, _) = wing_fixture();
    assert!(!ai.surge_buy_a_bomber(&mut g, 0, None), "the gene is off");
    ai.enable_surge_fields_the_bombers();
    let cost = g
        .unit_purchase_cost(0, first, "bomber", "gold")
        .expect("the airfield city prices a Bomber");
    let gold = g.players[0].gold;
    assert!(ai.surge_buy_a_bomber(&mut g, 0, None));
    assert_eq!(bombers(&g), 1);
    assert!((g.players[0].gold - (gold - cost)).abs() < 1e-6);
}

#[test]
fn the_purchase_keeps_the_reserve_and_spares_a_threatened_city() {
    let (mut g, mut ai, first, _) = wing_fixture();
    ai.enable_surge_fields_the_bombers();
    let cost = g.unit_purchase_cost(0, first, "bomber", "gold").unwrap();
    g.players[0].gold = cost + SURGE_BOMBER_BUY_RESERVE - 1.0;
    assert!(!ai.surge_buy_a_bomber(&mut g, 0, None));
    g.players[0].gold = 10_000.0;
    assert!(!ai.surge_buy_a_bomber(&mut g, 0, Some(first)));
    assert_eq!(bombers(&g), 0);
}

#[test]
fn the_surge_buys_while_its_wing_is_short() {
    let (mut g, mut ai, _, _) = wing_fixture();
    ai.enable_surge_fields_the_bombers();
    assert!(ai.air_surge_status.aerodromes > 0);
    assert!(ai.air_surge_status.bombers_committed < AdvancedAi::air_surge_bomber_goal(&g, 0));
    ai.air_surge_production(&mut g, 0);
    assert_eq!(bombers(&g), 1);
    // Stock: the gene off buys nothing.
    let (mut g, mut ai, _, _) = wing_fixture();
    ai.air_surge_production(&mut g, 0);
    assert_eq!(bombers(&g), 0);
}
