use super::*;
use std::sync::Arc;

fn fixture() -> (Game, AdvancedAi, u32, u32, StrategicPlan) {
    let mut g = Game::new_full(2, 36, 22, 15_4051, 650, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (4, 8), None);
    g.found_city_for(0, (8, 12), None);
    let walled = g.found_city_for(1, (12, 8), None);
    let open = g.found_city_for(1, (16, 8), None);
    let prior = g.cities.get_mut(&walled).unwrap();
    prior.buildings.push(crate::name!("walls"));
    prior.wall_hp = 100;
    g.cities.get_mut(&open).unwrap().hp = 180;
    Arc::make_mut(&mut g.observed_city_strength).insert(walled, 100.0);
    Arc::make_mut(&mut g.observed_city_strength).insert(open, 45.0);
    g.record_contact(0, 1);
    g.at_war.insert((0, 1));
    g.current = 0;
    g.turn = 170;
    g.players[0].techs.extend(
        g.rules.tech_ancestors[AIR_SURGE_GOAL_TECH]
            .iter()
            .map(|tech| Name::new(tech)),
    );
    g.players[0].techs.insert(Name::new(AIR_SURGE_GOAL_TECH));
    g.players[0]
        .strategic_resources
        .insert(crate::name!("aluminum"), 400.0);
    for pos in [(8, 12), (8, 12)] {
        g.spawn_test_unit("bomber", 0, pos);
    }
    for pos in [(15, 8), (15, 9)] {
        g.spawn_test_unit("cuirassier", 0, pos);
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    ai.enable_siege_commitment();
    ai.air_surge_plan = Some(AirSurge {
        target_player: 1,
        objective_city: walled,
        objective_pos: g.cities[&walled].pos,
        body_unit: crate::name!("cuirassier"),
        body_is_cavalry: true,
        opened_at_war: true,
        phase: AirSurgePhase::Exploit,
        appointed_turn: 150,
        tech_turn: Some(160),
        declared_turn: Some(169),
        last_reviewed_turn: 170,
        recovery_assessments: 0,
    });
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert!(ai.air_surge_status.wing_ready());
    assert!(ai.air_surge_status.escort_ready());
    assert_eq!(
        ai.capture_opportunity_city(&g, 0, walled, GrandStrategy::Conquest),
        Some(open),
        "the existing land planner selects this open foothold"
    );
    assert!(AdvancedAi::air_surge_in_range(&g, 0, g.cities[&open].pos));
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(open),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, walled, open, plan)
}

#[test]
fn a_ready_wing_does_not_erase_the_land_planners_open_foothold() {
    let (g, mut ai, _, open, mut plan) = fixture();
    ai.apply_air_surge_to_strategy(&g, 0, &mut plan);
    ai.air_surge_adopt_campaign(&g, 0, &plan);
    assert_eq!(
        plan.target_city,
        Some(open),
        "the ready air overlay must preserve the selected open foothold"
    );
    let wing = ai.air_surge_plan.as_ref().unwrap();
    assert_eq!(
        wing.objective_city, open,
        "wing and land army share the target"
    );
    assert_eq!(wing.appointed_turn, 150);
    assert_eq!(wing.tech_turn, Some(160));
    assert_eq!(wing.declared_turn, Some(169));
    assert_eq!(wing.phase, AirSurgePhase::Exploit);
}
