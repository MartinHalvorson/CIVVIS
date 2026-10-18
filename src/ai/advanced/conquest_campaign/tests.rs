use super::*;

fn campaign() -> (Game, AdvancedAi, u32, u32) {
    let mut game = Game::new_full(4, 60, 38, 581144, 250, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.at_war.clear();
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    game.found_city_for(0, (30, 20), None);
    let distant = game.found_city_for(1, (4, 20), None);
    let nearby = game.found_city_for(2, (35, 20), None);
    game.found_city_for(3, (0, 0), None);
    let mut controller = AdvancedAi::targeting(VictoryTarget::Domination);
    controller.battlefront_observation = false;
    (game, controller, distant, nearby)
}

#[test]
fn conquest_campaign_replaces_an_unopenable_frontier() {
    let (game, controller, distant, nearby) = campaign();
    assert!(!AdvancedAi::city_within_declaration_range(
        &game,
        0,
        game.cities[&distant].pos
    ));
    assert_eq!(
        controller.conquest_campaign_objective(
            &game,
            0,
            GrandStrategy::Conquest,
            Some(1),
            Some(distant)
        ),
        (Some(2), Some(nearby))
    );
}

#[test]
fn conquest_campaign_uses_a_reachable_city_of_the_preferred_rival_first() {
    let (mut game, controller, distant, _) = campaign();
    let border = game.found_city_for(1, (36, 18), None);
    assert_eq!(
        controller.conquest_campaign_objective(
            &game,
            0,
            GrandStrategy::Conquest,
            Some(1),
            Some(distant)
        ),
        (Some(1), Some(border))
    );
}

#[test]
fn conquest_campaign_does_not_invent_an_objective_when_no_front_can_open() {
    let (mut game, controller, distant, _) = campaign();
    game.players[2].alive = false;
    assert_eq!(
        controller.conquest_campaign_objective(
            &game,
            0,
            GrandStrategy::Conquest,
            Some(1),
            Some(distant)
        ),
        (None, None)
    );
}

#[test]
fn conquest_campaign_preserves_a_reachable_objective() {
    let (game, controller, _, nearby) = campaign();
    assert_eq!(
        controller.conquest_campaign_objective(
            &game,
            0,
            GrandStrategy::Conquest,
            Some(2),
            Some(nearby)
        ),
        (Some(2), Some(nearby))
    );
}

#[test]
fn conquest_campaign_preserves_an_explicit_target() {
    let (game, mut controller, distant, _) = campaign();
    controller.forced_target_player = Some(1);
    assert_eq!(
        controller.conquest_campaign_objective(
            &game,
            0,
            GrandStrategy::Conquest,
            Some(1),
            Some(distant)
        ),
        (Some(1), Some(distant))
    );
}

#[test]
fn conquest_campaign_does_not_open_a_second_front() {
    let (mut game, controller, distant, _) = campaign();
    game.at_war.insert((0, 3));
    game.at_war.insert((3, 0));
    assert_eq!(
        controller.conquest_campaign_objective(
            &game,
            0,
            GrandStrategy::Conquest,
            Some(1),
            Some(distant)
        ),
        (Some(1), Some(distant))
    );
}

#[test]
fn conquest_campaign_leaves_adaptive_rivals_unchanged() {
    let (game, mut controller, distant, _) = campaign();
    controller.victory_target = None;
    assert_eq!(
        controller.conquest_campaign_objective(
            &game,
            0,
            GrandStrategy::Conquest,
            Some(1),
            Some(distant)
        ),
        (Some(1), Some(distant))
    );
}

#[test]
fn conquest_campaign_leaves_other_plans_unchanged() {
    let (game, controller, distant, _) = campaign();
    for strategy in [GrandStrategy::Recovery, GrandStrategy::Science] {
        assert_eq!(
            controller.conquest_campaign_objective(&game, 0, strategy, Some(1), Some(distant)),
            (Some(1), Some(distant))
        );
    }
}

#[test]
fn conquest_campaign_leaves_other_victory_targets_unchanged() {
    let (game, mut controller, distant, _) = campaign();
    controller.victory_target = Some(VictoryTarget::Science);
    assert_eq!(
        controller.conquest_campaign_objective(
            &game,
            0,
            GrandStrategy::Conquest,
            Some(1),
            Some(distant)
        ),
        (Some(1), Some(distant))
    );
}

#[test]
fn conquest_campaign_respects_existing_far_capital_reach() {
    let (game, mut controller, distant, _) = campaign();
    controller.capital_prey_opens_a_front_2 = true;
    assert!(controller.capital_prey_reaches_far(&game, 0, 1, game.cities[&distant].pos));
    assert_eq!(
        controller.conquest_campaign_objective(
            &game,
            0,
            GrandStrategy::Conquest,
            Some(1),
            Some(distant)
        ),
        (Some(1), Some(distant))
    );
}

#[test]
fn conquest_campaign_does_not_target_unmet_rivals() {
    let (mut game, mut controller, distant, _) = campaign();
    controller.battlefront_observation = true;
    game.players[0].met.clear();
    assert_eq!(
        controller.conquest_campaign_objective(
            &game,
            0,
            GrandStrategy::Conquest,
            Some(1),
            Some(distant)
        ),
        (None, None)
    );
}

#[test]
fn conquest_campaign_keeps_the_current_war_objective() {
    let (mut game, controller, distant, _) = campaign();
    game.at_war.insert((0, 1));
    game.at_war.insert((1, 0));
    assert_eq!(
        controller.conquest_campaign_objective(
            &game,
            0,
            GrandStrategy::Conquest,
            Some(1),
            Some(distant)
        ),
        (Some(1), Some(distant))
    );
}

#[test]
fn conquest_campaign_does_not_replace_the_last_original_capital() {
    let (mut game, controller, distant, _) = campaign();
    for city in game.cities.values_mut() {
        if [2, 3].contains(&city.original_owner) {
            city.owner = 0;
        }
    }
    assert!(AdvancedAi::capture_completes_domination(&game, 0, distant));
    assert_eq!(
        controller.conquest_campaign_objective(
            &game,
            0,
            GrandStrategy::Conquest,
            Some(1),
            Some(distant)
        ),
        (Some(1), Some(distant))
    );
}

#[test]
fn conquest_campaign_does_not_redirect_toward_a_friend() {
    let (mut game, controller, distant, _) = campaign();
    game.players[0].friends_until.insert(2, game.turn + 20);
    assert_eq!(
        controller.conquest_campaign_objective(
            &game,
            0,
            GrandStrategy::Conquest,
            Some(1),
            Some(distant)
        ),
        (None, None)
    );
}

#[test]
fn conquest_campaign_keeps_the_existing_eighteen_tile_boundary() {
    let (mut game, controller, _, _) = campaign();
    let border = game.found_city_for(1, (48, 20), None);
    assert!(AdvancedAi::city_within_declaration_range(
        &game,
        0,
        game.cities[&border].pos
    ));
    assert_eq!(
        controller.conquest_campaign_objective(
            &game,
            0,
            GrandStrategy::Conquest,
            Some(1),
            Some(border)
        ),
        (Some(1), Some(border))
    );
}

#[test]
fn conquest_campaign_keeps_the_expansion_army_on_a_reachable_frontier() {
    let (game, controller, distant, nearby) = campaign();
    assert_eq!(
        controller.conquest_campaign_objective(
            &game,
            0,
            GrandStrategy::Expansion,
            Some(1),
            Some(distant)
        ),
        (Some(2), Some(nearby))
    );
}

fn standing_archers() -> (Game, AdvancedAi, u32) {
    let (mut game, controller, _, nearby) = campaign();
    game.found_city_for(0, (25, 20), None);
    game.current = 0;
    game.turn = 50;
    for technology in [
        "pottery",
        "mining",
        "writing",
        "archery",
        "wheel",
        "bronze_working",
        "iron_working",
        "engineering",
    ] {
        game.players[0].techs.insert(Name::new(technology));
    }
    for _ in 0..3 {
        game.spawn_test_unit("archer", 0, (30, 20));
    }
    (game, controller, nearby)
}

#[test]
fn conquest_campaign_researches_the_standing_archer_upgrade_before_war() {
    let (game, controller, _) = standing_archers();
    assert!(!game.is_at_war(0, 2));
    assert_eq!(
        controller.wartime_modernization_tech(&game, 0).as_deref(),
        Some("machinery")
    );
}

#[test]
fn conquest_campaign_modernization_keeps_writing_first() {
    let (mut game, controller, _) = standing_archers();
    game.players[0].techs.remove(&crate::name!("writing"));
    assert_eq!(controller.wartime_modernization_tech(&game, 0), None);
}

#[test]
fn conquest_campaign_modernization_does_not_beeline_for_two_archers() {
    let (mut game, controller, _) = standing_archers();
    let unit = game.player_unit_ids(0)[0];
    game.remove_unit(unit);
    assert_eq!(controller.wartime_modernization_tech(&game, 0), None);
}

#[test]
fn conquest_campaign_modernization_requires_an_actionable_frontier() {
    let (mut game, controller, _) = standing_archers();
    game.players[2].alive = false;
    assert_eq!(controller.wartime_modernization_tech(&game, 0), None);
}

#[test]
fn conquest_campaign_modernization_keeps_peaceful_other_lanes_unchanged() {
    let (game, mut controller, _) = standing_archers();
    for target in [
        None,
        Some(VictoryTarget::Science),
        Some(VictoryTarget::Religion),
    ] {
        controller.victory_target = target;
        assert_eq!(controller.wartime_modernization_tech(&game, 0), None);
    }
}

#[test]
fn conquest_campaign_modernization_accounts_for_the_full_prerequisite_bill() {
    let (mut game, controller, _) = standing_archers();
    for _ in 0..5 {
        game.spawn_test_unit("heavy_chariot", 0, (30, 20));
    }
    assert_eq!(
        controller.wartime_modernization_tech(&game, 0).as_deref(),
        Some("machinery")
    );
    game.at_war.insert((0, 1));
    assert_eq!(
        controller.wartime_modernization_tech(&game, 0).as_deref(),
        Some("stirrups")
    );
}

#[test]
fn conquest_campaign_modernization_routes_the_actual_peace_research_choice() {
    let (mut game, controller, nearby) = standing_archers();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: Some(2),
        target_city: Some(nearby),
        threatened_city: None,
        desired_cities: 5,
        assessed_turn: game.turn,
        rush: false,
    };
    controller.advanced_research(&mut game, 0, &plan);
    assert_eq!(game.players[0].research.as_deref(), Some("machinery"));
}

fn frontline_production() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let (mut game, mut controller, _, target) = campaign();
    game.players[0].civ = "gran_colombia".into();
    game.players[0].gold = 500.0;
    game.players[0].gold_per_turn = 20.0;
    game.players[0].met.insert(2);
    game.players[2].met.insert(0);
    game.players[0].techs.insert(crate::name!("writing"));
    game.players[0]
        .techs
        .insert(crate::name!("military_tactics"));
    controller.victory_planning = true;
    let home = game.player_city_ids(0)[0];
    let second = game.found_city_for(0, (30, 12), None);
    for _ in 0..3 {
        game.spawn_test_unit("warrior", 0, (30, 20));
    }
    for _ in 0..6 {
        game.spawn_test_unit("archer", 0, (30, 20));
    }
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(2),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 5,
        assessed_turn: game.turn,
        rush: false,
    };
    assert!(game.can_produce(
        0,
        home,
        &Item::Unit {
            unit: crate::name!("pikeman")
        }
    ));
    (game, controller, plan, home, second)
}

#[test]
fn conquest_campaign_frontline_gap_values_the_current_obsolete_cohort_without_actions() {
    let (game, controller, plan, home, _) = frontline_production();
    let actions = game.log.len();
    let gold = game.players[0].gold;
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        3
    );
    assert_eq!(game.log.len(), actions);
    assert_eq!(game.players[0].gold, gold);
    assert!(game.cities[&home].queue.is_empty());
}

#[test]
fn conquest_campaign_frontline_gap_reaches_the_saturated_production_score() {
    let (mut game, controller, plan, home, _) = frontline_production();
    let counts = controller.counts(&game, 0);
    assert!(counts.military >= 9);
    let item = Item::Unit {
        unit: crate::name!("pikeman"),
    };
    let eligible = controller.production_value(&game, 0, home, &item, &plan, &counts);
    game.players[0].gold_per_turn = 0.0;
    let unfunded = controller.production_value(&game, 0, home, &item, &plan, &counts);
    assert!(eligible > 0.0, "eligible saturated score {eligible}");
    assert!(unfunded < -1_000.0, "unfunded saturated score {unfunded}");
}

#[test]
fn conquest_campaign_frontline_gap_counts_wounded_modern_bodies_toward_the_cap() {
    let (mut game, controller, plan, home, _) = frontline_production();
    game.spawn_test_unit("pikeman", 0, (29, 20));
    let wounded = game.spawn_test_unit("man_at_arms", 0, (31, 20));
    game.units.get_mut(&wounded).unwrap().hp = 25;
    game.spawn_test_unit("pikeman", 0, (30, 21));
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        0
    );
}

#[test]
fn conquest_campaign_frontline_gap_counts_other_cities_queues() {
    let (mut game, controller, plan, home, second) = frontline_production();
    game.cities.get_mut(&second).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("pikeman"),
    }];
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        2
    );
    game.spawn_test_unit("man_at_arms", 0, (31, 20));
    game.spawn_test_unit("pikeman", 0, (30, 21));
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        0
    );
}

#[test]
fn conquest_campaign_frontline_gap_does_not_self_block_a_current_queue() {
    let (mut game, controller, plan, home, _) = frontline_production();
    game.cities.get_mut(&home).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("pikeman"),
    }];
    game.spawn_test_unit("man_at_arms", 0, (31, 20));
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        2
    );
}

#[test]
fn conquest_campaign_frontline_gap_needs_three_obsolete_frontliners() {
    let (mut game, controller, plan, home, _) = frontline_production();
    let warrior = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == crate::name!("warrior"))
        .unwrap();
    game.remove_unit(warrior);
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        0
    );
}

#[test]
fn conquest_campaign_frontline_gap_preserves_other_lanes_and_legacy() {
    let (game, mut controller, plan, home, _) = frontline_production();
    controller.victory_target = Some(VictoryTarget::Science);
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        0
    );
    controller.victory_target = Some(VictoryTarget::Domination);
    controller.victory_planning = false;
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        0
    );
}

#[test]
fn conquest_campaign_frontline_gap_preserves_emergency_and_active_war_priorities() {
    let (mut game, controller, mut plan, home, _) = frontline_production();
    plan.threatened_city = Some(home);
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        0
    );
    plan.threatened_city = None;
    game.at_war.insert((0, 2));
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        0
    );
}

#[test]
fn conquest_campaign_frontline_gap_requires_writing_and_a_named_living_objective() {
    let (mut game, controller, mut plan, home, _) = frontline_production();
    game.players[0].techs.remove(&crate::name!("writing"));
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        0
    );
    game.players[0].techs.insert(crate::name!("writing"));
    plan.target_player = None;
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        0
    );
    plan.target_player = Some(2);
    game.players[2].alive = false;
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        0
    );
}

#[test]
fn conquest_campaign_frontline_gap_keeps_current_research_and_material_permission() {
    let (mut game, controller, plan, home, _) = frontline_production();
    game.players[0]
        .techs
        .remove(&crate::name!("military_tactics"));
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        0
    );
    game.players[0].techs.insert(crate::name!("stirrups"));
    assert!(!game.can_produce(
        0,
        home,
        &Item::Unit {
            unit: crate::name!("knight")
        }
    ));
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("knight")),
        0
    );
}

#[test]
fn conquest_campaign_frontline_gap_reserves_upkeep_for_queued_bodies() {
    let (mut game, controller, plan, home, second) = frontline_production();
    game.players[0].gold_per_turn = 3.9;
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        0
    );
    game.players[0].gold_per_turn = 4.0;
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        3
    );
    game.cities.get_mut(&second).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("heavy_chariot"),
    }];
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        0
    );
    game.cities.get_mut(&second).unwrap().queue.clear();
    game.players[0].gold = 20.0;
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        0
    );
}

#[test]
fn conquest_campaign_frontline_gap_never_invents_an_old_or_ranged_force_gap() {
    let (game, controller, plan, home, _) = frontline_production();
    for unit in [
        crate::name!("warrior"),
        crate::name!("archer"),
        crate::name!("swordsman"),
        crate::name!("catapult"),
    ] {
        assert_eq!(
            controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, unit),
            0
        );
    }
}

fn delegated_frontline() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let (mut game, mut controller, mut plan, home, second) = frontline_production();
    game.players[0].civ = "Gran Colombia".into();
    assert!(game.rules.civs.contains_key(&game.players[0].civ));
    game.turn = 61;
    game.current = 0;
    game.players[0].research = Some("sailing".into());
    game.players[0].civic = Some("craftsmanship".into());
    for city in [home, second] {
        let held = game.cities.get_mut(&city).unwrap();
        held.pop = 8;
        held.production = 0.0;
        held.queue.clear();
        held.production_progress.clear();
    }
    game.cities
        .get_mut(&plan.target_city.unwrap())
        .unwrap()
        .wall_hp = 100;
    controller.enable_lane_delegates_production_2();
    controller.base.book_pos = 4;
    plan.strategy = GrandStrategy::Expansion;
    plan.assessed_turn = game.turn;
    controller.plan = Some(plan.clone());
    assert!(controller.lane_delegates_now(Some(VictoryTarget::Domination), false));
    (game, controller, plan, home, second)
}

#[test]
fn conquest_campaign_delegated_frontline_places_one_current_legal_order() {
    let (mut game, controller, plan, _, _) = delegated_frontline();
    let before = game.log.len();
    let gold = game.players[0].gold;
    let (city, item) = controller
        .reserve_delegated_conquest_frontline(&mut game, 0, &plan)
        .unwrap();
    assert_eq!(
        item,
        Item::Unit {
            unit: crate::name!("pikeman")
        }
    );
    assert_eq!(game.cities[&city].queue.first(), Some(&item));
    assert_eq!(game.log.len(), before + 1);
    assert_eq!(game.players[0].gold, gold);
    assert!(
        matches!(&game.log.last().unwrap().1, Action::Produce { city: ordered, item: ordered_item } if *ordered == city && *ordered_item == item)
    );
}

#[test]
fn conquest_campaign_delegated_frontline_reaches_the_normal_take_turn_route() {
    let (mut game, mut controller, _, _, _) = delegated_frontline();
    let before = game.log.len();
    controller.take_turn(&mut game, 0);
    assert!(game.log.iter().skip(before).any(|(player, action)| *player == 0 && matches!(action, Action::Produce { item: Item::Unit { unit }, .. } if unit.as_str() == "pikeman")), "plan {:?}; gold {} income {}; queues {:?}; actions {:?}", controller.plan, game.players[0].gold, game.players[0].gold_per_turn, game.player_city_ids(0).iter().map(|city| (*city, game.cities[city].queue.clone())).collect::<Vec<_>>(), game.log.iter().skip(before).collect::<Vec<_>>());
    assert!(game
        .player_city_ids(0)
        .iter()
        .any(|city| game.cities[city].queue.first()
            == Some(&Item::Unit {
                unit: crate::name!("pikeman")
            })));
}

#[test]
fn conquest_campaign_delegated_frontline_survives_the_ordinary_delegated_governor() {
    let (mut game, mut controller, plan, _, _) = delegated_frontline();
    let (city, item) = controller
        .reserve_delegated_conquest_frontline(&mut game, 0, &plan)
        .unwrap();
    controller.delegated_cities(&mut game, 0, &plan);
    assert_eq!(game.cities[&city].queue.first(), Some(&item));
}

#[test]
fn conquest_campaign_delegated_frontline_repeated_calls_stop_at_two_queued_bodies() {
    let (mut game, controller, plan, _, _) = delegated_frontline();
    game.found_city_for(0, (25, 17), None);
    assert!(controller
        .reserve_delegated_conquest_frontline(&mut game, 0, &plan)
        .is_some());
    assert!(controller
        .reserve_delegated_conquest_frontline(&mut game, 0, &plan)
        .is_some());
    assert_eq!(
        controller.reserve_delegated_conquest_frontline(&mut game, 0, &plan),
        None
    );
    assert_eq!(
        game.player_city_ids(0)
            .iter()
            .filter(|city| game.cities[city].queue.first()
                == Some(&Item::Unit {
                    unit: crate::name!("pikeman")
                }))
            .count(),
        2
    );
}

#[test]
fn conquest_campaign_delegated_frontline_preserves_capital_and_other_committed_queues() {
    let (mut game, controller, plan, home, second) = delegated_frontline();
    let library = Item::Building {
        building: crate::name!("library"),
    };
    for city in [home, second] {
        game.cities.get_mut(&city).unwrap().queue = vec![library.clone()];
    }
    let before = game.log.len();
    assert_eq!(
        controller.reserve_delegated_conquest_frontline(&mut game, 0, &plan),
        None
    );
    assert_eq!(game.log.len(), before);
    assert!(game
        .player_city_ids(0)
        .iter()
        .all(|city| game.cities[city].queue.first() == Some(&library)));
}

#[test]
fn conquest_campaign_delegated_frontline_respects_wounded_body_and_formation_caps() {
    let (mut game, controller, plan, _, second) = delegated_frontline();
    let wounded = game.spawn_test_unit("pikeman", 0, (29, 20));
    game.units.get_mut(&wounded).unwrap().hp = 25;
    game.spawn_test_unit("pikeman", 0, (30, 21));
    game.cities.get_mut(&second).unwrap().queue = vec![Item::Formation {
        unit: crate::name!("pikeman"),
        formation: 1,
    }];
    let before = game.log.len();
    assert_eq!(
        controller.reserve_delegated_conquest_frontline(&mut game, 0, &plan),
        None
    );
    assert_eq!(game.log.len(), before);
}

#[test]
fn conquest_campaign_delegated_frontline_is_a_noop_without_delegation_or_domination() {
    let (mut game, mut controller, plan, _, _) = delegated_frontline();
    let before = game.log.len();
    controller.lane_delegates_production_2 = false;
    assert_eq!(
        controller.reserve_delegated_conquest_frontline(&mut game, 0, &plan),
        None
    );
    controller.enable_lane_delegates_production_2();
    controller.victory_target = Some(VictoryTarget::Science);
    assert_eq!(
        controller.reserve_delegated_conquest_frontline(&mut game, 0, &plan),
        None
    );
    assert_eq!(game.log.len(), before);
}

#[test]
fn conquest_campaign_delegated_frontline_preserves_emergency_and_major_war_guards() {
    let (mut game, controller, mut plan, home, _) = delegated_frontline();
    let before = game.log.len();
    plan.threatened_city = Some(home);
    assert_eq!(
        controller.reserve_delegated_conquest_frontline(&mut game, 0, &plan),
        None
    );
    plan.threatened_city = None;
    game.at_war.insert((0, 2));
    assert_eq!(
        controller.reserve_delegated_conquest_frontline(&mut game, 0, &plan),
        None
    );
    assert_eq!(game.log.len(), before);
}

#[test]
fn conquest_campaign_delegated_frontline_preserves_current_funding_and_unlock_guards() {
    let (mut game, controller, plan, _, _) = delegated_frontline();
    let before = game.log.len();
    game.players[0].gold = 0.0;
    assert_eq!(
        controller.reserve_delegated_conquest_frontline(&mut game, 0, &plan),
        None
    );
    game.players[0].gold = 500.0;
    game.players[0].gold_per_turn = 0.0;
    assert_eq!(
        controller.reserve_delegated_conquest_frontline(&mut game, 0, &plan),
        None
    );
    game.players[0].gold_per_turn = 20.0;
    game.players[0]
        .techs
        .remove(&crate::name!("military_tactics"));
    assert_eq!(
        controller.reserve_delegated_conquest_frontline(&mut game, 0, &plan),
        None
    );
    assert_eq!(game.log.len(), before);
}

#[test]
fn conquest_campaign_delegated_frontline_uses_the_current_city_item_eta() {
    let (mut game, controller, plan, home, second) = delegated_frontline();
    let item = Item::Unit {
        unit: crate::name!("pikeman"),
    };
    for (city, turns) in [(home, 10.0), (second, 2.0)] {
        std::sync::Arc::make_mut(&mut game.host_buildable)
            .entry(city)
            .or_default()
            .insert(
                Game::production_block_key(&item),
                crate::game::HostMenuEntry {
                    cost: Some(100.0),
                    turns: Some(turns),
                },
            );
    }
    assert_eq!(
        controller.reserve_delegated_conquest_frontline(&mut game, 0, &plan),
        Some((second, item))
    );
}

fn cohort_sized_frontline() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let (game, controller, plan, home, second) = delegated_frontline();
    assert_eq!(game.players[0].civ, "Gran Colombia");
    assert_eq!(
        game.units
            .values()
            .filter(|unit| unit.owner == 0 && unit.hp > 0 && unit.kind == "warrior")
            .count(),
        3
    );
    (game, controller, plan, home, second)
}

#[test]
fn conquest_campaign_cohort_sized_frontline_prices_the_existing_obsolete_army() {
    let (mut game, controller, plan, home, _) = cohort_sized_frontline();
    for _ in 0..2 {
        game.spawn_test_unit("warrior", 0, (30, 20));
    }
    let before = game.log.len();
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        5
    );
    assert_eq!(game.log.len(), before);
}

#[test]
fn conquest_campaign_cohort_sized_frontline_keeps_one_slot_after_two_owned_bodies() {
    let (mut game, controller, plan, home, _) = cohort_sized_frontline();
    for _ in 0..2 {
        game.spawn_test_unit("pikeman", 0, (30, 20));
    }
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        1
    );
}

#[test]
fn conquest_campaign_cohort_sized_frontline_counts_owned_and_queued_formations() {
    let (mut game, controller, plan, home, second) = cohort_sized_frontline();
    for _ in 0..2 {
        game.spawn_test_unit("warrior", 0, (30, 20));
        game.spawn_test_unit("pikeman", 0, (30, 20));
    }
    game.cities.get_mut(&second).unwrap().queue = vec![Item::Formation {
        unit: crate::name!("pikeman"),
        formation: 1,
    }];
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        2
    );
}

#[test]
fn conquest_campaign_cohort_sized_frontline_reserves_a_funded_third_body() {
    let (mut game, controller, plan, _, _) = cohort_sized_frontline();
    for _ in 0..2 {
        game.spawn_test_unit("pikeman", 0, (30, 20));
    }
    let before = game.log.len();
    let (city, item) = controller
        .reserve_delegated_conquest_frontline(&mut game, 0, &plan)
        .unwrap();
    assert_eq!(
        item,
        Item::Unit {
            unit: crate::name!("pikeman")
        }
    );
    assert_eq!(game.cities[&city].queue.first(), Some(&item));
    assert_eq!(game.log.len(), before + 1);
}

#[test]
fn conquest_campaign_cohort_sized_frontline_stops_when_the_existing_cohort_is_matched() {
    let (mut game, controller, plan, home, _) = cohort_sized_frontline();
    for _ in 0..3 {
        game.spawn_test_unit("pikeman", 0, (30, 20));
    }
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        0
    );
}

#[test]
fn conquest_campaign_cohort_sized_frontline_retains_the_three_obsolete_body_floor() {
    let (mut game, controller, plan, home, _) = cohort_sized_frontline();
    let warrior = game
        .units
        .values()
        .find(|unit| unit.owner == 0 && unit.kind == "warrior")
        .unwrap()
        .id;
    game.remove_unit(warrior);
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        0
    );
}

#[test]
fn conquest_campaign_cohort_sized_frontline_keeps_the_current_treasury_floor() {
    let (mut game, controller, plan, home, _) = cohort_sized_frontline();
    game.players[0].gold = 29.0;
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        0
    );
}

#[test]
fn conquest_campaign_cohort_sized_frontline_reserves_existing_queued_upkeep() {
    let (mut game, controller, plan, home, second) = cohort_sized_frontline();
    game.players[0].gold_per_turn = 4.0;
    game.cities.get_mut(&second).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("heavy_chariot"),
    }];
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        0
    );
}

#[test]
fn conquest_campaign_cohort_sized_frontline_keeps_the_threatened_city_veto() {
    let (game, controller, mut plan, home, _) = cohort_sized_frontline();
    plan.threatened_city = Some(home);
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        0
    );
}

#[test]
fn conquest_campaign_cohort_sized_frontline_still_limits_concurrent_current_tier_queues() {
    let (mut game, controller, plan, home, second) = cohort_sized_frontline();
    for _ in 0..2 {
        game.spawn_test_unit("warrior", 0, (30, 20));
    }
    let third = game.found_city_for(0, (25, 17), None);
    for city in [second, third] {
        game.cities.get_mut(&city).unwrap().queue = vec![Item::Unit {
            unit: crate::name!("pikeman"),
        }];
    }
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        0
    );
    assert_eq!(
        controller.reserve_delegated_conquest_frontline(&mut game, 0, &plan),
        None
    );
}
