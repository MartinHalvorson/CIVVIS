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
fn conquest_campaign_frontline_gap_values_two_current_bodies_without_actions() {
    let (game, controller, plan, home, _) = frontline_production();
    let actions = game.log.len();
    let gold = game.players[0].gold;
    assert_eq!(
        controller.conquest_campaign_frontline_gap(&game, 0, home, &plan, crate::name!("pikeman")),
        2
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
        1
    );
    game.spawn_test_unit("man_at_arms", 0, (31, 20));
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
        1
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
        2
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

fn frontline_research() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let (mut game, controller, plan, home, second) = frontline_production();
    game.players[0].civ = "Gran Colombia".into();
    assert!(game.rules.civs.get("Gran Colombia").is_some());
    let archers: Vec<_> = game
        .units
        .values()
        .filter(|unit| unit.owner == 0 && unit.kind == "archer")
        .map(|unit| unit.id)
        .collect();
    for unit in archers {
        game.remove_unit(unit);
    }
    game.players[0]
        .techs
        .remove(&crate::name!("military_tactics"));
    for technology in [
        "pottery",
        "mining",
        "archery",
        "wheel",
        "bronze_working",
        "iron_working",
        "currency",
        "mathematics",
    ] {
        game.players[0].techs.insert(Name::new(technology));
    }
    game.current = 0;
    game.turn = 60;
    let science = AdvancedAi::war_science_per_turn(&game, 0);
    std::sync::Arc::make_mut(&mut game.host_research_quotes)
        .entry(0)
        .or_default()
        .insert(
            crate::name!("military_tactics"),
            crate::game::HostResearchQuote {
                cost: Some(science * 10.0),
                progress: Some(0.0),
            },
        );
    (game, controller, plan, home, second)
}

#[test]
fn conquest_campaign_frontline_tech_selects_bounded_resource_free_unlock_without_actions() {
    let (game, controller, plan, _, _) = frontline_research();
    let actions = game.log.len();
    assert_eq!(controller.wartime_modernization_tech(&game, 0), None);
    assert_eq!(
        controller
            .conquest_campaign_frontline_tech(&game, 0, &plan)
            .as_deref(),
        Some("military_tactics")
    );
    assert_eq!(game.log.len(), actions);
    assert!(game.players[0].research.is_none());
}

#[test]
fn conquest_campaign_frontline_tech_reaches_the_actual_research_choice() {
    let (mut game, controller, plan, _, _) = frontline_research();
    controller.advanced_research(&mut game, 0, &plan);
    assert_eq!(
        game.players[0].research.as_deref(),
        Some("military_tactics")
    );
}

#[test]
fn conquest_campaign_frontline_tech_preserves_standing_cohort_priority() {
    let (mut game, controller, plan, _, _) = frontline_research();
    for _ in 0..3 {
        game.spawn_test_unit("archer", 0, (30, 20));
    }
    assert_eq!(
        controller.wartime_modernization_tech(&game, 0).as_deref(),
        Some("machinery")
    );
    controller.advanced_research(&mut game, 0, &plan);
    assert_eq!(game.players[0].research.as_deref(), Some("engineering"));
}

#[test]
fn conquest_campaign_frontline_tech_yields_to_wounded_or_queued_modern_bodies() {
    let (mut game, controller, plan, home, _) = frontline_research();
    let modern = game.spawn_test_unit("pikeman", 0, (30, 20));
    game.units.get_mut(&modern).unwrap().hp = 1;
    assert_eq!(
        controller.conquest_campaign_frontline_tech(&game, 0, &plan),
        None
    );
    game.remove_unit(modern);
    game.cities.get_mut(&home).unwrap().queue = vec![Item::Formation {
        unit: crate::name!("knight"),
        formation: 1,
    }];
    assert_eq!(
        controller.conquest_campaign_frontline_tech(&game, 0, &plan),
        None
    );
}

#[test]
fn conquest_campaign_frontline_tech_yields_to_a_currently_legal_modern_order() {
    let (mut game, controller, plan, home, _) = frontline_research();
    game.players[0].techs.insert(crate::name!("stirrups"));
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 20.0);
    assert!(game.can_produce(
        0,
        home,
        &Item::Unit {
            unit: crate::name!("knight")
        }
    ));
    assert_eq!(
        controller.conquest_campaign_frontline_tech(&game, 0, &plan),
        None
    );
}

#[test]
fn conquest_campaign_frontline_tech_requires_three_obsolete_frontliners() {
    let (mut game, controller, plan, _, _) = frontline_research();
    let obsolete = game
        .units
        .values()
        .find(|unit| unit.owner == 0 && unit.kind == "warrior")
        .unwrap()
        .id;
    game.remove_unit(obsolete);
    assert_eq!(
        controller.conquest_campaign_frontline_tech(&game, 0, &plan),
        None
    );
}

#[test]
fn conquest_campaign_frontline_tech_counts_full_missing_prerequisites_and_host_progress() {
    let (mut game, controller, plan, _, _) = frontline_research();
    let science = AdvancedAi::war_science_per_turn(&game, 0);
    game.players[0].techs.remove(&crate::name!("mathematics"));
    let quotes = std::sync::Arc::make_mut(&mut game.host_research_quotes)
        .entry(0)
        .or_default();
    quotes.insert(
        crate::name!("mathematics"),
        crate::game::HostResearchQuote {
            cost: Some(science * 3.0),
            progress: Some(0.0),
        },
    );
    assert_eq!(
        controller.conquest_campaign_frontline_tech(&game, 0, &plan),
        None
    );
    std::sync::Arc::make_mut(&mut game.host_research_quotes)
        .get_mut(&0)
        .unwrap()
        .get_mut(&crate::name!("military_tactics"))
        .unwrap()
        .progress = Some(science * 2.0);
    assert_eq!(
        controller
            .conquest_campaign_frontline_tech(&game, 0, &plan)
            .as_deref(),
        Some("military_tactics")
    );
}

#[test]
fn conquest_campaign_frontline_tech_rejects_more_than_twelve_current_science_turns() {
    let (mut game, controller, plan, _, _) = frontline_research();
    let science = AdvancedAi::war_science_per_turn(&game, 0);
    std::sync::Arc::make_mut(&mut game.host_research_quotes)
        .get_mut(&0)
        .unwrap()
        .get_mut(&crate::name!("military_tactics"))
        .unwrap()
        .cost = Some(science * 13.0);
    assert_eq!(
        controller.conquest_campaign_frontline_tech(&game, 0, &plan),
        None
    );
}

#[test]
fn conquest_campaign_frontline_tech_reserves_two_bodies_and_all_queued_upkeep() {
    let (mut game, controller, plan, _, second) = frontline_research();
    game.players[0].gold_per_turn = 5.0;
    assert_eq!(
        controller.conquest_campaign_frontline_tech(&game, 0, &plan),
        None
    );
    game.players[0].gold_per_turn = 6.0;
    assert_eq!(
        controller
            .conquest_campaign_frontline_tech(&game, 0, &plan)
            .as_deref(),
        Some("military_tactics")
    );
    game.cities.get_mut(&second).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("archer"),
    }];
    assert_eq!(
        controller.conquest_campaign_frontline_tech(&game, 0, &plan),
        None
    );
    game.players[0].gold_per_turn = 7.0;
    assert_eq!(
        controller
            .conquest_campaign_frontline_tech(&game, 0, &plan)
            .as_deref(),
        Some("military_tactics")
    );
}

#[test]
fn conquest_campaign_frontline_tech_preserves_treasury_and_finite_income_guards() {
    let (mut game, controller, plan, _, _) = frontline_research();
    for gold in [29.0, f64::NAN, f64::INFINITY] {
        game.players[0].gold = gold;
        assert_eq!(
            controller.conquest_campaign_frontline_tech(&game, 0, &plan),
            None
        );
    }
    game.players[0].gold = 500.0;
    for income in [-1.0, f64::NAN, f64::INFINITY] {
        game.players[0].gold_per_turn = income;
        assert_eq!(
            controller.conquest_campaign_frontline_tech(&game, 0, &plan),
            None
        );
    }
}

#[test]
fn conquest_campaign_frontline_tech_preserves_lane_emergency_and_war_guards() {
    let (mut game, mut controller, mut plan, home, _) = frontline_research();
    plan.threatened_city = Some(home);
    assert_eq!(
        controller.conquest_campaign_frontline_tech(&game, 0, &plan),
        None
    );
    plan.threatened_city = None;
    plan.rush = true;
    assert_eq!(
        controller.conquest_campaign_frontline_tech(&game, 0, &plan),
        None
    );
    plan.rush = false;
    plan.strategy = GrandStrategy::Science;
    assert_eq!(
        controller.conquest_campaign_frontline_tech(&game, 0, &plan),
        None
    );
    plan.strategy = GrandStrategy::Conquest;
    controller.victory_planning = false;
    assert_eq!(
        controller.conquest_campaign_frontline_tech(&game, 0, &plan),
        None
    );
    controller.victory_planning = true;
    game.at_war.insert((0, 2));
    assert_eq!(
        controller.conquest_campaign_frontline_tech(&game, 0, &plan),
        None
    );
}

#[test]
fn conquest_campaign_frontline_tech_requires_writing_and_named_legal_target() {
    let (mut game, controller, mut plan, _, _) = frontline_research();
    game.players[0].techs.remove(&crate::name!("writing"));
    assert_eq!(
        controller.conquest_campaign_frontline_tech(&game, 0, &plan),
        None
    );
    game.players[0].techs.insert(crate::name!("writing"));
    plan.target_city = None;
    assert_eq!(
        controller.conquest_campaign_frontline_tech(&game, 0, &plan),
        None
    );
}

#[test]
fn conquest_campaign_frontline_tech_yields_after_unlock_and_rejects_host_material_bill() {
    let (mut game, controller, plan, home, second) = frontline_research();
    let item = Item::Unit {
        unit: crate::name!("pikeman"),
    };
    let key = Game::production_block_key(&item);
    game.replace_host_unit_resource_prices(
        [
            (home, [(key.clone(), 1.0)].into()),
            (second, [(key, 1.0)].into()),
        ]
        .into(),
    );
    assert_eq!(
        controller.conquest_campaign_frontline_tech(&game, 0, &plan),
        None
    );
    game.replace_host_unit_resource_prices(Default::default());
    game.players[0]
        .techs
        .insert(crate::name!("military_tactics"));
    assert!(game.can_produce(0, home, &item));
    assert_eq!(
        controller.conquest_campaign_frontline_tech(&game, 0, &plan),
        None
    );
}
