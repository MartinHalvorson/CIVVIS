use super::*;

fn direct_funding_campaign() -> (Game, AdvancedAi, StrategicPlan, Vec<u32>) {
    let (mut game, ranked) =
        crate::ai::commercial_hub::tests::emperor_empire("CAMPAIGNDIRECTFUNDING", 20_261_017);
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
    controller.base.w.city_target = 10.0;
    controller.enable_commercial_hub_in_the_strategic_queue();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 10,
        assessed_turn: game.turn,
        rush: false,
    };
    assert!(controller.domination_upgrade_funding_shortfall(&game, 0));
    assert!(controller.base.commercial_hub_window_open(&game));
    assert!(controller
        .base
        .commercial_hub_step(&game, 0, ranked[1], 6, 0)
        .is_some());
    (game, controller, plan, ranked)
}

fn direct_funding_queues(game: &Game) -> usize {
    game.player_city_ids(0)
        .iter()
        .filter(|city| {
            game.cities[city].queue.iter().any(|item| match item {
                Item::District { district, .. } => {
                    game.district_family(*district) == "commercial_hub"
                }
                Item::Building { building } => {
                    game.rules
                        .buildings
                        .get(building)
                        .is_some_and(|specification| {
                            specification.district.is_some_and(|district| {
                                game.district_family(district) == "commercial_hub"
                            })
                        })
                }
                Item::Unit { unit } => unit == "trader",
                _ => false,
            })
        })
        .count()
}

#[test]
fn conquest_campaign_direct_funding_claims_one_real_queue_before_the_settler_promise() {
    let (mut game, mut controller, plan, ranked) = direct_funding_campaign();
    assert!(controller.base.settler_due(&game, 0, ranked[1], 6, 0));
    let capital = game
        .player_city_ids(0)
        .into_iter()
        .find(|city| game.cities[city].is_capital)
        .unwrap();
    let original_capital = game.cities[&capital].queue.clone();
    controller.claim_conquest_campaign_funding(&mut game, 0, &plan);
    assert_eq!(direct_funding_queues(&game), 1);
    assert!(
        matches!(game.cities[&ranked[1]].queue.first(), Some(Item::District { district, .. }) if game.district_family(*district) == "commercial_hub")
    );
    assert_eq!(game.cities[&capital].queue, original_capital);
    controller.claim_conquest_campaign_funding(&mut game, 0, &plan);
    assert_eq!(direct_funding_queues(&game), 1);
}

#[test]
fn conquest_campaign_direct_funding_preserves_an_actual_capital_settler() {
    let (mut game, mut controller, plan, _) = direct_funding_campaign();
    let capital = game
        .player_city_ids(0)
        .into_iter()
        .find(|city| game.cities[city].is_capital)
        .unwrap();
    game.apply(
        0,
        &Action::Produce {
            city: capital,
            item: Item::Unit {
                unit: Name::new("settler"),
            },
        },
    )
    .unwrap();
    let original = game.cities[&capital].queue.clone();
    controller.claim_conquest_campaign_funding(&mut game, 0, &plan);
    assert_eq!(game.cities[&capital].queue, original);
    assert_eq!(direct_funding_queues(&game), 1);
}

#[test]
fn conquest_campaign_direct_funding_stops_for_an_existing_trade_commitment() {
    let (mut game, mut controller, plan, ranked) = direct_funding_campaign();
    let item = controller
        .base
        .commercial_hub_step(&game, 0, ranked[1], 6, 0)
        .unwrap();
    game.apply(
        0,
        &Action::Produce {
            city: ranked[1],
            item,
        },
    )
    .unwrap();
    let original = game.cities[&ranked[1]].queue.clone();
    controller.claim_conquest_campaign_funding(&mut game, 0, &plan);
    assert_eq!(game.cities[&ranked[1]].queue, original);
    assert_eq!(direct_funding_queues(&game), 1);
}

#[test]
fn conquest_campaign_direct_funding_preserves_paused_settler_progress() {
    let (mut game, mut controller, plan, _) = direct_funding_campaign();
    for city in game.player_city_ids(0) {
        if !game.cities[&city].is_capital {
            game.cities
                .get_mut(&city)
                .unwrap()
                .production_progress
                .insert("unit:settler".into(), 20.0);
        }
    }
    controller.claim_conquest_campaign_funding(&mut game, 0, &plan);
    assert_eq!(direct_funding_queues(&game), 0);
    for city in game.player_city_ids(0) {
        if !game.cities[&city].is_capital {
            assert_eq!(
                game.cities[&city].production_progress.get("unit:settler"),
                Some(&20.0)
            );
        }
    }
}

#[test]
fn conquest_campaign_direct_funding_preserves_an_inflight_market() {
    let (mut game, mut controller, plan, ranked) = direct_funding_campaign();
    let Item::District { district, pos } = controller
        .base
        .commercial_hub_step(&game, 0, ranked[1], 6, 0)
        .unwrap()
    else {
        panic!("expected a legal Commercial Hub site");
    };
    game.cities
        .get_mut(&ranked[1])
        .unwrap()
        .districts
        .insert(district, pos);
    let item = controller
        .base
        .commercial_hub_step(&game, 0, ranked[1], 6, 0)
        .unwrap();
    assert!(matches!(&item, Item::Building { .. }));
    game.apply(
        0,
        &Action::Produce {
            city: ranked[1],
            item,
        },
    )
    .unwrap();
    let original = game.cities[&ranked[1]].queue.clone();
    controller.claim_conquest_campaign_funding(&mut game, 0, &plan);
    assert_eq!(game.cities[&ranked[1]].queue, original);
    assert_eq!(direct_funding_queues(&game), 1);
}

#[test]
fn conquest_campaign_direct_funding_preserves_an_inflight_trader() {
    let (mut game, mut controller, plan, _) = direct_funding_campaign();
    Arc::make_mut(&mut game.observed_trade_capacity).insert(0, 1);
    let capital = game
        .player_city_ids(0)
        .into_iter()
        .find(|city| game.cities[city].is_capital)
        .unwrap();
    game.apply(
        0,
        &Action::Produce {
            city: capital,
            item: Item::Unit {
                unit: Name::new("trader"),
            },
        },
    )
    .unwrap();
    let original = game.cities[&capital].queue.clone();
    controller.claim_conquest_campaign_funding(&mut game, 0, &plan);
    assert_eq!(game.cities[&capital].queue, original);
    assert_eq!(direct_funding_queues(&game), 1);
}

#[test]
fn conquest_campaign_direct_funding_preserves_the_funded_army() {
    let (mut game, mut controller, plan, _) = direct_funding_campaign();
    game.players[0].gold = 100_000.0;
    assert!(!controller.domination_upgrade_funding_shortfall(&game, 0));
    controller.claim_conquest_campaign_funding(&mut game, 0, &plan);
    assert_eq!(direct_funding_queues(&game), 0);
}

#[test]
fn conquest_campaign_direct_funding_preserves_other_victory_lanes() {
    for target in [
        None,
        Some(VictoryTarget::Science),
        Some(VictoryTarget::Religion),
    ] {
        let (mut game, mut controller, plan, _) = direct_funding_campaign();
        controller.victory_target = target;
        controller.claim_conquest_campaign_funding(&mut game, 0, &plan);
        assert_eq!(direct_funding_queues(&game), 0);
    }
}

#[test]
fn conquest_campaign_direct_funding_requires_the_existing_trade_and_victory_genes() {
    for disable_victory in [false, true] {
        let (mut game, mut controller, plan, _) = direct_funding_campaign();
        if disable_victory {
            controller.victory_planning = false;
        } else {
            controller.commercial_hub_in_the_strategic_queue = false;
        }
        controller.claim_conquest_campaign_funding(&mut game, 0, &plan);
        assert_eq!(direct_funding_queues(&game), 0);
    }
}

#[test]
fn conquest_campaign_direct_funding_preserves_war_production() {
    let (mut game, mut controller, plan, _) = direct_funding_campaign();
    let mut rival = game.players[0].clone();
    rival.id = game.players.len();
    rival.team = None;
    game.at_war.insert((0, rival.id));
    game.players.push(rival);
    controller.claim_conquest_campaign_funding(&mut game, 0, &plan);
    assert_eq!(direct_funding_queues(&game), 0);
}

#[test]
fn conquest_campaign_direct_funding_preserves_rushes_and_recovery() {
    for rush in [false, true] {
        let (mut game, mut controller, mut plan, _) = direct_funding_campaign();
        if rush {
            plan.rush = true;
        } else {
            plan.strategy = GrandStrategy::Recovery;
        }
        controller.claim_conquest_campaign_funding(&mut game, 0, &plan);
        assert_eq!(direct_funding_queues(&game), 0);
    }
}

#[test]
fn conquest_campaign_direct_funding_preserves_the_actual_industrial_commitment() {
    let (mut game, mut controller, plan, ranked) = direct_funding_campaign();
    controller.enable_industrial_zone_in_the_producers();
    let item = controller
        .base
        .industrial_zone_producer_item(&game, 0, ranked[1], 6, None)
        .unwrap();
    game.apply(
        0,
        &Action::Produce {
            city: ranked[1],
            item,
        },
    )
    .unwrap();
    let original = game.cities[&ranked[1]].queue.clone();
    controller.claim_conquest_campaign_funding(&mut game, 0, &plan);
    assert_eq!(game.cities[&ranked[1]].queue, original);
}

#[test]
fn conquest_campaign_direct_funding_preserves_local_threats_and_the_plan_loan() {
    let (mut game, mut controller, mut plan, ranked) = direct_funding_campaign();
    controller.base.plan_threatened_city = Some(ranked[0]);
    plan.threatened_city = Some(ranked[1]);
    for city in ranked.iter().skip(2) {
        game.cities.get_mut(city).unwrap().last_attacked = game.turn;
    }
    controller.claim_conquest_campaign_funding(&mut game, 0, &plan);
    assert_eq!(direct_funding_queues(&game), 0);
    assert_eq!(controller.base.plan_threatened_city, Some(ranked[0]));
}

#[test]
fn conquest_campaign_direct_funding_waits_for_the_standing_defense_floor() {
    let (mut game, mut controller, plan, _) = direct_funding_campaign();
    for unit in game.player_unit_ids(0) {
        if game.units[&unit].kind == "warrior" {
            game.remove_unit(unit);
        }
    }
    assert!(controller.domination_upgrade_funding_shortfall(&game, 0));
    controller.claim_conquest_campaign_funding(&mut game, 0, &plan);
    assert_eq!(direct_funding_queues(&game), 0);
}

#[test]
fn conquest_campaign_direct_funding_preserves_economic_recovery() {
    let (mut game, mut controller, plan, _) = direct_funding_campaign();
    game.players[0].gold_per_turn = -10.0;
    controller.claim_conquest_campaign_funding(&mut game, 0, &plan);
    assert_eq!(direct_funding_queues(&game), 0);
}

#[test]
fn conquest_campaign_direct_funding_keeps_the_existing_trade_window() {
    let (mut game, mut controller, plan, _) = direct_funding_campaign();
    game.turn = 1;
    assert!(!controller.base.commercial_hub_window_open(&game));
    controller.claim_conquest_campaign_funding(&mut game, 0, &plan);
    assert_eq!(direct_funding_queues(&game), 0);
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
