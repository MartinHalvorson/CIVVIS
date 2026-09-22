use super::*;

fn threatened_culture() -> (Game, StrategicPlan) {
    let mut g = Game::new(2, 24, 16, 774_4071, 250, 0);
    g.players[0].civ = "Greece".to_string();
    let home = g.units[&g.player_unit_ids(0)[0]].pos;
    g.found_city_for(0, home, None);
    for uid in g.player_unit_ids(0) {
        g.remove_unit(uid);
    }
    g.spawn_test_unit("hoplite", 0, home);
    g.spawn_test_unit("hoplite", 0, home);
    g.players[0]
        .techs
        .extend([crate::name!("mining"), crate::name!("bronze_working")]);
    g.at_war.insert((0, 1));
    let plan = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        target_player: None,
        target_city: None,
        threatened_city: Some(g.player_city_ids(0)[0]),
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, plan)
}

#[test]
fn besieged_culture_unlocks_walls_before_pikeman_prerequisites() {
    let (mut g, plan) = threatened_culture();
    let ai = AdvancedAi::targeting(VictoryTarget::Culture);
    assert_eq!(
        ai.wartime_modernization_tech(&g, 0).as_deref(),
        Some("military_tactics")
    );
    g.players[0].research = None;
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("masonry"));
    g.players[0].techs.insert(crate::name!("masonry"));
    assert!(g.can_produce(
        0,
        plan.threatened_city.unwrap(),
        &crate::game::Item::Building {
            building: crate::name!("walls"),
        }
    ));
    g.players[0].research = None;
    ai.advanced_research(&mut g, 0, &plan);
    assert_ne!(g.players[0].research.as_deref(), Some("masonry"));
}

#[test]
fn wall_unlock_requires_an_unwalled_culture_city_in_a_defensive_war() {
    let (mut g, mut plan) = threatened_culture();
    let ai = AdvancedAi::targeting(VictoryTarget::Culture);
    assert!(ai.defensive_walls_research_goal(&g, 0, &plan).is_some());
    assert!(AdvancedAi::targeting(VictoryTarget::Science)
        .defensive_walls_research_goal(&g, 0, &plan)
        .is_none());
    plan.strategy = GrandStrategy::Expansion;
    assert!(ai.defensive_walls_research_goal(&g, 0, &plan).is_none());
    plan.strategy = GrandStrategy::Recovery;
    let city = plan.threatened_city.take().unwrap();
    // A power-deficit recovery is the warning; the city need not already
    // be in the attacker's one-turn capture radius.
    assert!(ai.defensive_walls_research_goal(&g, 0, &plan).is_some());
    plan.threatened_city = Some(city);
    g.at_war.clear();
    assert!(ai.defensive_walls_research_goal(&g, 0, &plan).is_none());
    g.at_war.insert((0, 1));
    g.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    assert!(ai.defensive_walls_research_goal(&g, 0, &plan).is_none());
}

fn exposed_domination() -> (Game, StrategicPlan, u32) {
    let (mut g, mut plan) = threatened_culture();
    plan.strategy = GrandStrategy::Expansion;
    plan.threatened_city = None;
    g.players[1].is_barbarian = true;
    let city = g.player_city_ids(0)[0];
    let center = g.cities[&city].pos;
    let near = g
        .nbrs(center)
        .into_iter()
        .find(|p| {
            g.map
                .get(*p)
                .is_some_and(|t| g.rules.is_passable(t) && !g.rules.is_water(t))
        })
        .unwrap();
    let attacker = g.spawn_test_unit("knight", 1, near);
    let visible = g.player_vision_frame(0);
    assert!(AdvancedAi::imminent_city_attack(&g, 0, city, &visible));
    (g, plan, attacker)
}

#[test]
fn exposed_domination_unlocks_walls_before_waiting_for_damage_or_recovery() {
    let (mut g, plan, _) = exposed_domination();
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert_eq!(g.cities[&g.player_city_ids(0)[0]].hp, 200);
    g.players[0].research = None;
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("masonry"));
}

#[test]
fn domination_recovery_keeps_the_existing_major_war_wall_unlock() {
    let (g, plan) = threatened_culture();
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert_eq!(
        ai.defensive_walls_research_goal(&g, 0, &plan),
        Some(crate::name!("masonry"))
    );
}

#[test]
fn domination_wall_warning_requires_visible_competitive_attack_on_unwalled_city() {
    let (mut g, plan, attacker) = exposed_domination();
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let city = g.player_city_ids(0)[0];
    assert!(ai.defensive_walls_research_goal(&g, 0, &plan).is_some());
    g.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    assert!(ai.defensive_walls_research_goal(&g, 0, &plan).is_none());
    g.cities.get_mut(&city).unwrap().buildings.clear();
    g.remove_unit(attacker);
    assert!(ai.defensive_walls_research_goal(&g, 0, &plan).is_none());
}

#[test]
fn defensive_wall_goal_does_not_discard_in_progress_research() {
    let (mut g, plan, _) = exposed_domination();
    g.players[0].research = Some("iron_working".into());
    g.players[0].research_progress = 17.0;
    AdvancedAi::targeting(VictoryTarget::Domination).advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("iron_working"));
    assert_eq!(g.players[0].research_progress, 17.0);
}

#[test]
fn weak_or_distant_hostile_does_not_force_wall_research() {
    let (mut g, plan, attacker) = exposed_domination();
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let near = g.units[&attacker].pos;
    g.remove_unit(attacker);
    let weak = g.spawn_test_unit("scout", 1, near);
    assert!(ai.defensive_walls_research_goal(&g, 0, &plan).is_none());
    g.remove_unit(weak);
    let center = g.cities[&g.player_city_ids(0)[0]].pos;
    let far = *g
        .map
        .tiles
        .keys()
        .find(|p| g.wdist(center, **p) > 8)
        .unwrap();
    g.spawn_test_unit("knight", 1, far);
    assert!(ai.defensive_walls_research_goal(&g, 0, &plan).is_none());
}

#[test]
fn reachable_approach_warns_before_the_one_turn_attack_envelope() {
    let (mut g, plan, attacker) = exposed_domination();
    let city = g.player_city_ids(0)[0];
    let center = g.cities[&city].pos;
    g.remove_unit(attacker);
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = Some(crate::name!("forest"));
    }
    let approach = *g
        .map
        .tiles
        .keys()
        .find(|p| g.wdist(center, **p) == 3)
        .unwrap();
    let attacker = g.spawn_test_unit("knight", 1, approach);
    let observer = g
        .nbrs(approach)
        .into_iter()
        .find(|p| g.wdist(center, *p) > 3)
        .unwrap();
    g.spawn_test_unit("scout", 0, observer);
    let visible = g.player_vision_frame(0);
    assert!(g.sees(&visible, approach));
    assert!(g.unit_visible_to(attacker, 0));
    assert!(
        crate::game::effective_strength(
            g.unit_strength(&g.units[&attacker], false),
            g.units[&attacker].hp
        ) >= g.city_strength(city) * IMMINENT_ATTACK_STRENGTH_RATIO
    );
    assert!(!AdvancedAi::imminent_city_attack(&g, 0, city, &visible));
    assert!(g
        .route_distance(attacker, center, 1)
        .is_some_and(|steps| steps <= 4));
    assert_eq!(
        AdvancedAi::targeting(VictoryTarget::Domination)
            .defensive_walls_research_goal(&g, 0, &plan),
        Some(crate::name!("masonry"))
    );
}

fn exposed_queue() -> (Game, AdvancedAi, u32, u32) {
    let (mut g, _, attacker) = exposed_domination();
    g.players[1].is_barbarian = false;
    g.players[0].techs.insert(crate::name!("masonry"));
    let city = g.player_city_ids(0)[0];
    g.cities.get_mut(&city).unwrap().queue = vec![crate::game::Item::Building {
        building: crate::name!("monument"),
    }];
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.base.garrison_under_fire = true;
    (g, ai, city, attacker)
}

#[test]
fn exposed_domination_queue_defends_without_being_the_named_threat() {
    let (mut g, ai, city, _) = exposed_queue();
    let claim = ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, None);
    assert_eq!(claim.as_ref().map(|(city, _)| *city), Some(city));
    assert!(AdvancedAi::active_queue_answers_siege(
        &g,
        &g.cities[&city].queue[0]
    ));
}

#[test]
fn exposed_queue_requires_a_strong_enemy_in_an_active_major_war() {
    let (mut g, ai, city, attacker) = exposed_queue();
    g.at_war.clear();
    assert!(ai
        .redirect_unsafe_city_queue_for_defense(&mut g, 0, None)
        .is_none());
    g.at_war.insert((0, 1));
    let near = g.units[&attacker].pos;
    g.remove_unit(attacker);
    g.spawn_test_unit("scout", 1, near);
    assert!(ai
        .redirect_unsafe_city_queue_for_defense(&mut g, 0, None)
        .is_none());
    assert_eq!(
        g.cities[&city].queue[0],
        crate::game::Item::Building {
            building: crate::name!("monument")
        }
    );
}

#[test]
fn exposed_queue_does_not_retask_a_walled_or_non_domination_city() {
    let (mut g, mut ai, city, _) = exposed_queue();
    ai.victory_target = Some(VictoryTarget::Science);
    assert!(ai
        .redirect_unsafe_city_queue_for_defense(&mut g, 0, None)
        .is_none());
    ai.victory_target = Some(VictoryTarget::Domination);
    g.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    assert!(ai
        .redirect_unsafe_city_queue_for_defense(&mut g, 0, None)
        .is_none());
}
