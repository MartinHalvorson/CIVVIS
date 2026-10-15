use super::*;
use std::sync::Arc;

fn fixture() -> (Game, AdvancedAi, u32, u32, StrategicPlan) {
    fixture_with_players(2)
}

fn fixture_with_players(players: usize) -> (Game, AdvancedAi, u32, u32, StrategicPlan) {
    let mut g = Game::new_full(players, 36, 22, 15_4051, 650, 0, false);
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
    ai.apply_air_surge_to_strategy(&g, 0, &mut plan);
    assert_eq!(
        plan.target_city,
        Some(open),
        "the next frame keeps the handoff"
    );
    assert_eq!(ai.air_surge_plan.as_ref().unwrap().objective_city, open);
}

#[test]
fn the_handoff_does_not_replace_existing_siege_or_victory_guards() {
    for case in [
        "version_one",
        "other_lane",
        "prior_hp",
        "prior_walls",
        "open_walls",
        "no_war",
        "home_emergency",
        "no_land_choice",
    ] {
        let (mut g, mut ai, walled, open, mut plan) = fixture();
        match case {
            "version_one" => ai.disable_air_surge_2(),
            "other_lane" => ai.victory_target = Some(VictoryTarget::Culture),
            "prior_hp" => g.cities.get_mut(&walled).unwrap().hp = 199,
            "prior_walls" => g.cities.get_mut(&walled).unwrap().wall_hp = 99,
            "open_walls" => g.cities.get_mut(&open).unwrap().wall_hp = 100,
            "no_war" => g.at_war.clear(),
            "home_emergency" => plan.threatened_city = g.player_city_ids(0).first().copied(),
            "no_land_choice" => plan.target_city = None,
            _ => unreachable!(),
        }
        ai.apply_air_surge_to_strategy(&g, 0, &mut plan);
        ai.air_surge_adopt_campaign(&g, 0, &plan);
        assert_eq!(plan.target_city, Some(walled), "{case}");
        assert_eq!(
            ai.air_surge_plan.as_ref().unwrap().objective_city,
            walled,
            "{case}"
        );
    }
}

#[test]
fn a_home_recovery_keeps_its_own_plan_and_the_held_appointment() {
    let (g, mut ai, walled, open, mut plan) = fixture();
    plan.strategy = GrandStrategy::Recovery;
    ai.apply_air_surge_to_strategy(&g, 0, &mut plan);
    ai.air_surge_adopt_campaign(&g, 0, &plan);
    assert_eq!(plan.strategy, GrandStrategy::Recovery);
    assert_eq!(plan.target_city, Some(open));
    assert_eq!(ai.air_surge_plan.as_ref().unwrap().objective_city, walled);
}

#[test]
fn a_ready_wing_does_not_follow_a_different_rivals_land_city() {
    let (mut g, mut ai, walled, _, mut plan) = fixture_with_players(3);
    let other = g.found_city_for(2, (16, 12), None);
    g.record_contact(0, 2);
    g.at_war.insert((0, 2));
    plan.target_player = Some(2);
    plan.target_city = Some(other);
    ai.apply_air_surge_to_strategy(&g, 0, &mut plan);
    ai.air_surge_adopt_campaign(&g, 0, &plan);
    assert_eq!(plan.target_player, Some(1));
    assert_eq!(plan.target_city, Some(walled));
    assert_eq!(ai.air_surge_plan.as_ref().unwrap().objective_city, walled);
}

#[test]
fn the_land_foothold_must_also_be_under_the_wings_reach() {
    let (mut g, mut ai, walled, _, mut plan) = fixture();
    let far = g.found_city_for(1, (24, 18), None);
    g.cities.get_mut(&far).unwrap().hp = 180;
    Arc::make_mut(&mut g.observed_city_strength).insert(far, 45.0);
    for uid in g.player_unit_ids(0) {
        if g.units[&uid].kind == crate::name!("cuirassier") {
            g.remove_unit(uid);
        }
    }
    for pos in [(23, 18), (23, 17)] {
        g.spawn_test_unit("cuirassier", 0, pos);
    }
    assert_eq!(
        ai.capture_opportunity_city(&g, 0, walled, GrandStrategy::Conquest),
        Some(far)
    );
    assert!(!AdvancedAi::air_surge_in_range(&g, 0, g.cities[&far].pos));
    plan.target_city = Some(far);
    ai.apply_air_surge_to_strategy(&g, 0, &mut plan);
    ai.air_surge_adopt_campaign(&g, 0, &plan);
    assert_eq!(plan.target_city, Some(walled));
    assert_eq!(ai.air_surge_plan.as_ref().unwrap().objective_city, walled);
}

#[test]
fn the_city_stopping_a_launched_rival_does_not_yield_to_an_open_foothold() {
    let (mut g, mut ai, walled, _, mut plan) = fixture();
    let pos = g.cities[&walled].pos;
    g.cities
        .get_mut(&walled)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), pos);
    g.players[1]
        .science_projects
        .insert("exoplanet_expedition".into());
    assert_eq!(ai.air_surge_suppression_city(&g, 0, 1), Some(walled));
    ai.apply_air_surge_to_strategy(&g, 0, &mut plan);
    ai.air_surge_adopt_campaign(&g, 0, &plan);
    assert_eq!(plan.target_city, Some(walled));
    assert_eq!(ai.air_surge_plan.as_ref().unwrap().objective_city, walled);
}

#[test]
fn the_ready_handoff_leaves_building_and_incomplete_packages_alone() {
    for case in ["beeline", "arm", "bombers", "escort", "other_denial"] {
        let (g, mut ai, walled, _, plan) = fixture();
        match case {
            "beeline" => ai.air_surge_plan.as_mut().unwrap().phase = AirSurgePhase::Beeline,
            "arm" => ai.air_surge_plan.as_mut().unwrap().phase = AirSurgePhase::Arm,
            "bombers" => ai.air_surge_status.bombers = 1,
            "escort" => ai.air_surge_status.bodies = 1,
            "other_denial" => ai.air_surge_status.denial_elsewhere = true,
            _ => unreachable!(),
        }
        ai.air_surge_follow_open_foothold(&g, 0, &plan);
        assert_eq!(
            ai.air_surge_plan.as_ref().unwrap().objective_city,
            walled,
            "{case}"
        );
    }
}
