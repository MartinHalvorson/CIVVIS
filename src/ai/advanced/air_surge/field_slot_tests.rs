use super::super::StrategicPlan;
use super::*;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new_full(2, 40, 24, 370700, 500, 0, false);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
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
    let target = g.found_city_for(1, (22, 12), None);
    g.players[0].techs.insert(crate::name!("currency"));
    g.current = 0;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    ai.air_surge_plan = Some(AirSurge {
        target_player: 1,
        objective_city: target,
        objective_pos: (22, 12),
        body_unit: crate::name!("knight"),
        body_is_cavalry: true,
        opened_at_war: false,
        phase: AirSurgePhase::Beeline,
        appointed_turn: 1,
        tech_turn: None,
        declared_turn: None,
        last_reviewed_turn: 1,
        recovery_assessments: 0,
    });
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: 1,
        rush: false,
    };
    assert!(g.city_yields(first).production > g.city_yields(second).production);
    (g, ai, plan, first, second)
}

fn hub(g: &Game, city: u32) -> Item {
    Item::District {
        district: crate::name!("commercial_hub"),
        pos: g.district_sites(city, "commercial_hub")[0],
    }
}

#[test]
fn reserves_only_the_productive_citys_last_slot_before_flight() {
    let (g, ai, plan, first, second) = fixture();
    assert!(!g.players[0].techs.contains(&crate::name!("flight")));
    assert!(ai.production_value(&g, 0, first, &hub(&g, first), &plan, &ai.counts(&g, 0)) < 0.0);
    assert!(!ai.air_surge_reserves_field_slot(&g, 0, second, &hub(&g, second)));
    let field = Item::District {
        district: crate::name!("aerodrome"),
        pos: g.district_sites(first, "aerodrome")[0],
    };
    assert!(!ai.air_surge_reserves_field_slot(&g, 0, first, &field));
}

#[test]
fn releases_extra_capacity_and_an_ended_air_plan() {
    let (mut g, mut ai, _, first, _) = fixture();
    let item = hub(&g, first);
    g.cities.get_mut(&first).unwrap().pop = 4;
    assert!(!ai.air_surge_reserves_field_slot(&g, 0, first, &item));
    g.cities.get_mut(&first).unwrap().pop = 1;
    ai.air_surge_plan = None;
    assert_eq!(
        ai.air_surge_city_production_value(&g, 0, first, &item, 5.0),
        None
    );
}

#[test]
fn committed_airfield_releases_the_other_citys_slot() {
    let (mut g, ai, _, first, second) = fixture();
    let item = hub(&g, first);
    let field = Item::District {
        district: crate::name!("aerodrome"),
        pos: g.district_sites(second, "aerodrome")[0],
    };
    g.cities.get_mut(&second).unwrap().queue.push(field);
    assert_eq!(
        ai.air_surge_city_production_value(&g, 0, first, &item, 5.0),
        None
    );
}
