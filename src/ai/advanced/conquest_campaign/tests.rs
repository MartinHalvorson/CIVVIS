use super::*;

fn bankruptcy_cleanup_campaign(kind: &str) -> (Game, AdvancedAi) {
    let mut game = Game::new_full(2, 24, 16, 20_261_017, 250, 0, false);
    game.units.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    game.found_city_for(0, (4, 4), None);
    game.found_city_for(1, (18, 10), None);
    game.current = 0;
    game.turn = 125;
    game.players[0].gold = 0.0;
    game.players[0].gold_per_turn = -1.0;
    game.players[0].policies.clear();
    for position in [(6, 6), (6, 7), (7, 6), (7, 7)] {
        game.spawn_test_unit(kind, 0, position);
    }
    let mut controller = AdvancedAi::targeting(VictoryTarget::Domination);
    controller.enable_bankruptcy_exits_the_drain();
    (game, controller)
}

#[test]
fn conquest_campaign_bankruptcy_preserves_units_with_zero_discounted_upkeep() {
    let (mut game, controller) = bankruptcy_cleanup_campaign("archer");
    game.players[0]
        .policies
        .insert(crate::name!("conscription"));
    assert_eq!(game.policy_effect(0, "unit_maintenance_discount"), 1.0);
    assert_eq!(game.unit_gold_maintenance(0), 0.0);
    controller.exit_the_drain(&mut game, 0);
    assert_eq!(game.player_unit_ids(0).len(), 4);
}

#[test]
fn conquest_campaign_bankruptcy_preserves_units_with_zero_host_quoted_upkeep() {
    let (mut game, controller) = bankruptcy_cleanup_campaign("horseman");
    for identifier in game.player_unit_ids(0) {
        Arc::make_mut(&mut game.host_unit_facts).insert(
            identifier,
            crate::game::HostUnitFacts {
                maintenance: Some(0.0),
                ..Default::default()
            },
        );
    }
    assert_eq!(game.unit_gold_maintenance(0), 0.0);
    controller.exit_the_drain(&mut game, 0);
    assert_eq!(game.player_unit_ids(0).len(), 4);
}

#[test]
fn conquest_campaign_bankruptcy_does_not_repeat_a_sufficient_batch_this_turn() {
    let (mut game, controller) = bankruptcy_cleanup_campaign("horseman");
    controller.exit_the_drain(&mut game, 0);
    assert_eq!(game.player_unit_ids(0).len(), 3);
    assert_eq!(game.players[0].gold_per_turn, -1.0);
    controller.exit_the_drain(&mut game, 0);
    assert_eq!(game.player_unit_ids(0).len(), 3);
}

#[test]
fn conquest_campaign_bankruptcy_caps_the_whole_turn_not_each_frame() {
    let (mut game, controller) = bankruptcy_cleanup_campaign("horseman");
    game.players[0].gold_per_turn = -100.0;
    controller.exit_the_drain(&mut game, 0);
    assert_eq!(game.player_unit_ids(0).len(), 1);
    controller.exit_the_drain(&mut game, 0);
    assert_eq!(game.player_unit_ids(0).len(), 1);
}

#[test]
fn conquest_campaign_bankruptcy_allows_a_fresh_pass_next_turn() {
    let (mut game, controller) = bankruptcy_cleanup_campaign("horseman");
    controller.exit_the_drain(&mut game, 0);
    assert_eq!(game.player_unit_ids(0).len(), 3);
    game.turn += 1;
    game.players[0].gold = 0.0;
    game.players[0].gold_per_turn = -1.0;
    controller.exit_the_drain(&mut game, 0);
    assert_eq!(game.player_unit_ids(0).len(), 2);
}

#[test]
fn conquest_campaign_bankruptcy_no_candidate_does_not_consume_the_pass() {
    let (mut game, controller) = bankruptcy_cleanup_campaign("horseman");
    let city = game.player_city_ids(0)[0];
    let home = game.cities[&city].pos;
    for identifier in game.player_unit_ids(0) {
        game.units.get_mut(&identifier).unwrap().pos = home;
    }
    controller.exit_the_drain(&mut game, 0);
    assert_eq!(game.player_unit_ids(0).len(), 4);
    let identifier = game.player_unit_ids(0)[0];
    game.units.get_mut(&identifier).unwrap().pos = (6, 6);
    controller.exit_the_drain(&mut game, 0);
    assert_eq!(game.player_unit_ids(0).len(), 3);
}

#[test]
fn conquest_campaign_bankruptcy_controller_clone_remembers_the_same_turn_batch() {
    let (mut game, controller) = bankruptcy_cleanup_campaign("horseman");
    controller.exit_the_drain(&mut game, 0);
    assert_eq!(game.player_unit_ids(0).len(), 3);
    let cloned = controller.clone();
    cloned.exit_the_drain(&mut game, 0);
    assert_eq!(game.player_unit_ids(0).len(), 3);
}

#[test]
fn conquest_campaign_bankruptcy_preserves_existing_no_op_controls() {
    for control in ["gene_off", "covered_deficit", "new_gold_card"] {
        let (mut game, mut controller) = bankruptcy_cleanup_campaign("horseman");
        match control {
            "gene_off" => controller.disable_bankruptcy_exits_the_drain(),
            "covered_deficit" => game.players[0].gold = 10.0,
            "new_gold_card" => controller.drain_card_turn.set(Some(game.turn)),
            _ => unreachable!(),
        }
        controller.exit_the_drain(&mut game, 0);
        assert_eq!(game.player_unit_ids(0).len(), 4, "{control}");
    }
}

#[test]
fn conquest_campaign_bankruptcy_uses_host_formation_bill_then_discount() {
    let (mut game, controller) = bankruptcy_cleanup_campaign("horseman");
    game.players[0].gold_per_turn = -1.5;
    game.players[0]
        .policies
        .insert(crate::name!("conscription"));
    for identifier in game.player_unit_ids(0) {
        game.units.get_mut(&identifier).unwrap().formation = 1;
        Arc::make_mut(&mut game.host_unit_facts).insert(
            identifier,
            crate::game::HostUnitFacts {
                maintenance: Some(2.0),
                ..Default::default()
            },
        );
    }
    assert_eq!(game.unit_gold_maintenance(0), 4.0);
    controller.exit_the_drain(&mut game, 0);
    assert_eq!(game.player_unit_ids(0).len(), 2);
}

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
