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

fn funding_campaign() -> (Game, AdvancedAi, StrategicPlan, Vec<u32>) {
    let (mut game, ranked) =
        crate::ai::commercial_hub::tests::emperor_empire("CAMPAIGNFUNDING", 20261017);
    game.players[0].gold = 0.0;
    game.players[0].gold_per_turn = 1.0;
    for technology in [
        "archery",
        "iron_working",
        "engineering",
        "machinery",
        "apprenticeship",
    ] {
        game.players[0].techs.insert(Name::new(technology));
    }
    Arc::make_mut(&mut game.observed_trade_capacity).insert(0, 0);
    let position = game.cities[&ranked[0]].pos;
    for _ in 0..12 {
        game.spawn_test_unit("warrior", 0, position);
    }
    for _ in 0..3 {
        game.spawn_test_unit("archer", 0, position);
    }
    let mut controller = AdvancedAi::targeting(VictoryTarget::Domination);
    controller.base.book_pos = 4;
    controller.base.w.city_target = 6.0;
    controller.enable_industrial_zone_in_the_producers();
    controller.enable_commercial_hub_in_the_strategic_queue();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: game.turn,
        rush: false,
    };
    assert!(controller.domination_upgrade_funding_shortfall(&game, 0));
    assert!(controller
        .base
        .industrial_zone_producer_item(&game, 0, ranked[0], 6, None)
        .is_some());
    (game, controller, plan, ranked)
}

#[test]
fn conquest_campaign_funds_its_existing_cohort_before_new_industry() {
    let (mut game, mut controller, plan, ranked) = funding_campaign();
    controller.advanced_production(&mut game, 0, &plan, false);
    assert!(
        matches!(game.cities[&ranked[0]].queue.first(), Some(Item::District { district, .. })
            if game.district_family(*district) == "commercial_hub"),
        "{:?}",
        game.cities[&ranked[0]].queue
    );
}

#[test]
fn conquest_campaign_funding_keeps_an_already_funded_industrial_step() {
    let (mut game, mut controller, plan, ranked) = funding_campaign();
    game.players[0].gold = 10000.0;
    assert!(!controller.domination_upgrade_funding_shortfall(&game, 0));
    controller.advanced_production(&mut game, 0, &plan, false);
    assert!(
        matches!(game.cities[&ranked[0]].queue.first(), Some(Item::District { district, .. })
            if game.district_family(*district) == "industrial_zone"),
        "{:?}",
        game.cities[&ranked[0]].queue
    );
}

#[test]
fn conquest_campaign_funding_follows_the_existing_army_through_expansion() {
    let (mut game, mut controller, mut plan, ranked) = funding_campaign();
    plan.strategy = GrandStrategy::Expansion;
    controller.advanced_production(&mut game, 0, &plan, false);
    assert!(
        matches!(game.cities[&ranked[0]].queue.first(), Some(Item::District { district, .. })
            if game.district_family(*district) == "commercial_hub")
    );
}

#[test]
fn conquest_campaign_funding_preserves_other_victory_lanes_and_recovery() {
    let (game, mut controller, mut plan, _) = funding_campaign();
    for target in [
        None,
        Some(VictoryTarget::Science),
        Some(VictoryTarget::Religion),
    ] {
        controller.victory_target = target;
        assert!(!controller.conquest_campaign_needs_funding(&game, 0, &plan));
    }
    controller.victory_target = Some(VictoryTarget::Domination);
    for strategy in [GrandStrategy::Science, GrandStrategy::Recovery] {
        plan.strategy = strategy;
        assert!(!controller.conquest_campaign_needs_funding(&game, 0, &plan));
    }
}

#[test]
fn conquest_campaign_funding_preserves_major_war_production() {
    let (mut game, controller, plan, _) = funding_campaign();
    let mut rival = game.players[0].clone();
    let rival_id = game.players.len();
    rival.id = rival_id;
    rival.team = None;
    game.players.push(rival);
    game.at_war.insert((0, rival_id));
    assert!(game.is_at_war(0, rival_id));
    assert!(!controller.conquest_campaign_needs_funding(&game, 0, &plan));
}

#[test]
fn conquest_campaign_funding_retains_already_committed_industry() {
    let (mut game, mut controller, plan, ranked) = funding_campaign();
    let item = controller
        .base
        .industrial_zone_producer_item(&game, 0, ranked[0], 6, None)
        .unwrap();
    game.apply(
        0,
        &Action::Produce {
            city: ranked[0],
            item: item.clone(),
        },
    )
    .unwrap();
    controller.advanced_production(&mut game, 0, &plan, false);
    assert_eq!(game.cities[&ranked[0]].queue.first(), Some(&item));
}

#[test]
fn conquest_campaign_funding_preserves_city_threats_and_restores_the_loan() {
    let (game, mut controller, mut plan, ranked) = funding_campaign();
    plan.threatened_city = Some(ranked[0]);
    controller.base.plan_threatened_city = Some(ranked[1]);
    let counts = controller.counts(&game, 0);
    assert!(controller
        .conquest_campaign_trade_item(&game, 0, ranked[0], &plan, &counts)
        .is_none());
    assert_eq!(controller.base.plan_threatened_city, Some(ranked[1]));
}

#[test]
fn conquest_campaign_funding_requires_the_existing_trade_gene() {
    let (mut game, mut controller, plan, ranked) = funding_campaign();
    controller.disable_commercial_hub_in_the_strategic_queue();
    assert!(!controller.conquest_campaign_needs_funding(&game, 0, &plan));
    controller.advanced_production(&mut game, 0, &plan, false);
    assert!(
        matches!(game.cities[&ranked[0]].queue.first(), Some(Item::District { district, .. })
            if game.district_family(*district) == "industrial_zone")
    );
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
