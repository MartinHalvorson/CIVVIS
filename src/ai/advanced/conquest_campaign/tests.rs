use super::*;

fn conquest_campaign_campus_fixture() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let (mut game, mut controller, _, _) = campaign();
    let capital = game.player_city_ids(0)[0];
    let town = game.found_city_for(0, (20, 20), None);
    game.found_city_for(0, (30, 29), None);
    for tile in game.map.tiles.values_mut() {
        tile.hills = true;
    }
    for city in game.cities.values_mut().filter(|city| city.owner == 0) {
        city.pop = 6;
    }
    game.turn = 40;
    game.players[0].gold = 1000.0;
    game.players[0].gold_per_turn = 5.0;
    for technology in ["pottery", "writing", "mining"] {
        game.players[0].techs.insert(Name::new(technology));
    }
    for _ in 0..3 {
        game.spawn_test_unit("warrior", 0, (30, 20));
    }
    controller.victory_planning = true;
    controller.base.campus_before_the_army_2 = true;
    controller.capital_campus_before_the_plaza = true;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 10,
        assessed_turn: game.turn,
        rush: false,
    };
    assert!(BasicAi::first_campus_item(&game, 0, town).is_some());
    (game, controller, plan, capital, town)
}

fn conquest_campaign_noncapital_campus_queues(game: &Game) -> usize {
    game.player_city_ids(0)
        .iter()
        .filter(|city| {
            !game.cities[city].is_capital
                && matches!(game.cities[city].queue.first(), Some(Item::District { district, .. }) if game.district_family(*district) == "campus")
        })
        .count()
}

#[test]
fn conquest_campaign_campus_funds_one_town_without_taking_the_capital_settler() {
    let (mut game, controller, plan, capital, _) = conquest_campaign_campus_fixture();
    assert!(controller.base.settler_due(&game, 0, capital, 3, 0));
    assert!(controller
        .capital_campus_claim_item(&game, 0, &plan)
        .is_none());
    controller.claim_conquest_campaign_campus(&mut game, 0, &plan);
    assert_eq!(conquest_campaign_noncapital_campus_queues(&game), 1);
    assert!(game.cities[&capital].queue.is_empty());
    controller.claim_conquest_campaign_campus(&mut game, 0, &plan);
    assert_eq!(conquest_campaign_noncapital_campus_queues(&game), 1);
}

#[test]
fn conquest_campaign_campus_preserves_an_actual_capital_settler_queue() {
    let (mut game, controller, plan, capital, _) = conquest_campaign_campus_fixture();
    let settler = Item::Unit {
        unit: Name::new("settler"),
    };
    game.apply(
        0,
        &Action::Produce {
            city: capital,
            item: settler,
        },
    )
    .unwrap();
    let original = game.cities[&capital].queue.clone();
    controller.claim_conquest_campaign_campus(&mut game, 0, &plan);
    assert_eq!(game.cities[&capital].queue, original);
    assert_eq!(conquest_campaign_noncapital_campus_queues(&game), 1);
}

#[test]
fn conquest_campaign_campus_preserves_rushes_and_requires_writing() {
    for rush in [false, true] {
        let (mut game, controller, mut plan, _, _) = conquest_campaign_campus_fixture();
        if rush {
            plan.rush = true;
        } else {
            game.players[0].techs.remove(&crate::name!("writing"));
        }
        controller.claim_conquest_campaign_campus(&mut game, 0, &plan);
        assert_eq!(conquest_campaign_noncapital_campus_queues(&game), 0);
    }
}

#[test]
fn conquest_campaign_campus_preserves_existing_noncapital_campus_work() {
    let (mut game, controller, plan, _, town) = conquest_campaign_campus_fixture();
    let item = BasicAi::first_campus_item(&game, 0, town).unwrap();
    game.apply(0, &Action::Produce { city: town, item })
        .unwrap();
    let original = game.cities[&town].queue.clone();
    controller.claim_conquest_campaign_campus(&mut game, 0, &plan);
    assert_eq!(game.cities[&town].queue, original);
    assert_eq!(conquest_campaign_noncapital_campus_queues(&game), 1);
}

#[test]
fn conquest_campaign_campus_does_not_add_a_second_completed_town_campus() {
    let (mut game, controller, plan, _, town) = conquest_campaign_campus_fixture();
    let Item::District { district, pos } = BasicAi::first_campus_item(&game, 0, town).unwrap()
    else {
        panic!("expected a legal Campus site");
    };
    game.cities
        .get_mut(&town)
        .unwrap()
        .districts
        .insert(district, pos);
    controller.claim_conquest_campaign_campus(&mut game, 0, &plan);
    assert_eq!(conquest_campaign_noncapital_campus_queues(&game), 0);
}

#[test]
fn conquest_campaign_campus_keeps_other_victory_lanes_unchanged() {
    for target in [
        None,
        Some(VictoryTarget::Science),
        Some(VictoryTarget::Religion),
    ] {
        let (mut game, mut controller, plan, _, _) = conquest_campaign_campus_fixture();
        controller.victory_target = target;
        controller.claim_conquest_campaign_campus(&mut game, 0, &plan);
        assert_eq!(conquest_campaign_noncapital_campus_queues(&game), 0);
    }
}

#[test]
fn conquest_campaign_campus_requires_the_existing_science_and_victory_genes() {
    for disable_victory in [false, true] {
        let (mut game, mut controller, plan, _, _) = conquest_campaign_campus_fixture();
        if disable_victory {
            controller.victory_planning = false;
        } else {
            controller.base.campus_before_the_army_2 = false;
            controller.base.campus_before_the_army_3 = false;
        }
        controller.claim_conquest_campaign_campus(&mut game, 0, &plan);
        assert_eq!(conquest_campaign_noncapital_campus_queues(&game), 0);
    }
}

#[test]
fn conquest_campaign_campus_preserves_war_and_recovery_production() {
    for war in [false, true] {
        let (mut game, controller, mut plan, _, _) = conquest_campaign_campus_fixture();
        if war {
            game.at_war.insert((0, 1));
        } else {
            plan.strategy = GrandStrategy::Recovery;
        }
        controller.claim_conquest_campaign_campus(&mut game, 0, &plan);
        assert_eq!(conquest_campaign_noncapital_campus_queues(&game), 0);
    }
}

#[test]
fn conquest_campaign_campus_keeps_actual_queues_and_recently_attacked_towns() {
    let (mut game, controller, mut plan, capital, town) = conquest_campaign_campus_fixture();
    for city in game.player_city_ids(0) {
        if city != capital && city != town {
            game.cities.get_mut(&city).unwrap().last_attacked = game.turn;
        }
    }
    plan.threatened_city = Some(town);
    controller.claim_conquest_campaign_campus(&mut game, 0, &plan);
    assert_eq!(conquest_campaign_noncapital_campus_queues(&game), 0);
    plan.threatened_city = None;
    game.cities
        .get_mut(&town)
        .unwrap()
        .queue
        .push(Item::Building {
            building: Name::new("monument"),
        });
    controller.claim_conquest_campaign_campus(&mut game, 0, &plan);
    assert_eq!(conquest_campaign_noncapital_campus_queues(&game), 0);
    assert!(
        matches!(game.cities[&town].queue.first(), Some(Item::Building { building }) if building == "monument")
    );
}

#[test]
fn conquest_campaign_campus_waits_for_a_standing_army() {
    let (mut game, controller, plan, _, _) = conquest_campaign_campus_fixture();
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    controller.claim_conquest_campaign_campus(&mut game, 0, &plan);
    assert_eq!(conquest_campaign_noncapital_campus_queues(&game), 0);
}

#[test]
fn conquest_campaign_campus_does_not_spend_the_economic_recovery_queue() {
    let (mut game, controller, plan, _, _) = conquest_campaign_campus_fixture();
    game.players[0].gold = 0.0;
    game.players[0].gold_per_turn = -1.0;
    controller.claim_conquest_campaign_campus(&mut game, 0, &plan);
    assert_eq!(conquest_campaign_noncapital_campus_queues(&game), 0);
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
