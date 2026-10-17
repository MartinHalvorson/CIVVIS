use super::*;

fn cash_flow_campaign() -> (Game, AdvancedAi, StrategicPlan, Vec<u32>) {
    let (mut game, ranked) =
        crate::ai::commercial_hub::tests::emperor_empire("CAMPAIGNCASHFLOW", 20_261_017);
    game.players[0].gold = 0.0;
    game.players[0].gold_per_turn = -4.0;
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

fn cash_flow_install_hub(game: &mut Game, controller: &AdvancedAi, city: u32) {
    let Item::District { district, pos } = controller
        .base
        .commercial_hub_step(game, 0, city, game.player_city_ids(0).len(), 0)
        .unwrap()
    else {
        panic!("expected canonical Hub proposal");
    };
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(district, pos);
}

fn cash_flow_queues(game: &Game) -> Vec<(u32, Vec<Item>)> {
    game.player_city_ids(0)
        .into_iter()
        .map(|city| (city, game.cities[&city].queue.iter().cloned().collect()))
        .collect()
}

#[test]
fn conquest_campaign_cash_flow_counts_active_routes_without_standing_traders() {
    let (mut game, mut controller, plan, ranked) = cash_flow_campaign();
    Arc::make_mut(&mut game.observed_trade_capacity).insert(0, 1);
    game.routes.push(crate::game::TradeRoute {
        origin: ranked[0],
        dest: ranked[1],
        owner: 0,
        ends: game.turn + 20,
    });
    assert_eq!(controller.counts(&game, 0).traders, 0);
    assert_eq!(game.active_routes(0), game.trade_capacity(0));
    controller.claim_conquest_campaign_cash_flow(&mut game, 0, &plan);
    assert!(cash_flow_queues(&game)
        .iter()
        .all(|(_, queue)| queue.is_empty()));
}

#[test]
fn conquest_campaign_cash_flow_preserves_local_defense_emergencies() {
    for emergency in ["recent_attack", "barbarian_raider"] {
        let (mut game, mut controller, plan, ranked) = cash_flow_campaign();
        cash_flow_install_hub(&mut game, &controller, ranked[1]);
        if emergency == "recent_attack" {
            game.cities.get_mut(&ranked[1]).unwrap().last_attacked = game.turn;
        } else {
            game.barb_pid = Some(1);
            let position = game.cities[&ranked[1]].pos;
            game.spawn_test_unit("warrior", 1, position);
            assert!(controller
                .base
                .barbarian_local_alarm_for_controller(&game, 0, ranked[1]));
        }
        controller.claim_conquest_campaign_cash_flow(&mut game, 0, &plan);
        assert!(
            cash_flow_queues(&game)
                .iter()
                .all(|(_, queue)| queue.is_empty()),
            "{emergency}"
        );
    }
}

#[test]
fn conquest_campaign_cash_flow_preserves_a_preexisting_threat_hint() {
    let (mut game, mut controller, plan, ranked) = cash_flow_campaign();
    cash_flow_install_hub(&mut game, &controller, ranked[1]);
    controller.base.plan_threatened_city = Some(ranked[0]);
    controller.claim_conquest_campaign_cash_flow(&mut game, 0, &plan);
    assert_eq!(controller.base.plan_threatened_city, Some(ranked[0]));
    assert!(cash_flow_queues(&game)
        .iter()
        .all(|(_, queue)| queue.is_empty()));
}

#[test]
fn conquest_campaign_cash_flow_preserves_canonical_housing_priority() {
    let (mut game, mut controller, plan, ranked) = cash_flow_campaign();
    cash_flow_install_hub(&mut game, &controller, ranked[1]);
    game.cities
        .get_mut(&ranked[1])
        .unwrap()
        .buildings
        .retain(|building| *building != "granary");
    let housing = game.city_housing(&game.cities[&ranked[1]]);
    let population = game.cities[&ranked[1]].pop as f64;
    *Arc::make_mut(&mut game.observed_city_housing_adjustments)
        .entry(ranked[1])
        .or_insert(0.0) += population - housing;
    assert_eq!(game.city_housing(&game.cities[&ranked[1]]), population);
    assert!(controller
        .base
        .commercial_hub_step(&game, 0, ranked[1], 6, 0)
        .is_none());
    controller.claim_conquest_campaign_cash_flow(&mut game, 0, &plan);
    assert!(cash_flow_queues(&game)
        .iter()
        .all(|(_, queue)| queue.is_empty()));
}

#[test]
fn conquest_campaign_cash_flow_claims_a_real_market_in_a_deficit() {
    let (mut game, mut controller, plan, ranked) = cash_flow_campaign();
    cash_flow_install_hub(&mut game, &controller, ranked[1]);
    let expected = controller
        .base
        .commercial_hub_step(&game, 0, ranked[1], 6, 0)
        .unwrap();
    assert!(matches!(expected, Item::Building { .. }));
    assert!(game.players[0].gold_per_turn < -0.5);
    controller.claim_conquest_campaign_cash_flow(&mut game, 0, &plan);
    assert_eq!(game.cities[&ranked[1]].queue.iter().next(), Some(&expected));
}

#[test]
fn conquest_campaign_cash_flow_claims_a_canonical_spare_capacity_trader() {
    let (mut game, mut controller, plan, ranked) = cash_flow_campaign();
    Arc::make_mut(&mut game.observed_trade_capacity).insert(0, 1);
    let expected = controller
        .base
        .commercial_hub_step(&game, 0, ranked[1], 6, 0)
        .unwrap();
    assert!(matches!(&expected, Item::Unit { unit } if unit == "trader"));
    controller.claim_conquest_campaign_cash_flow(&mut game, 0, &plan);
    assert_eq!(game.cities[&ranked[1]].queue.iter().next(), Some(&expected));
}

#[test]
fn conquest_campaign_cash_flow_never_starts_a_new_hub() {
    let (mut game, mut controller, plan, ranked) = cash_flow_campaign();
    assert!(matches!(
        controller
            .base
            .commercial_hub_step(&game, 0, ranked[1], 6, 0),
        Some(Item::District { .. })
    ));
    controller.claim_conquest_campaign_cash_flow(&mut game, 0, &plan);
    assert!(cash_flow_queues(&game)
        .iter()
        .all(|(_, queue)| queue.is_empty()));
}

#[test]
fn conquest_campaign_cash_flow_utilizes_a_hub_while_another_is_committed() {
    let (mut game, mut controller, plan, ranked) = cash_flow_campaign();
    cash_flow_install_hub(&mut game, &controller, ranked[1]);
    let hub = controller
        .base
        .commercial_hub_step(&game, 0, ranked[2], 6, 0)
        .unwrap();
    assert!(matches!(hub, Item::District { .. }));
    game.apply(
        0,
        &Action::Produce {
            city: ranked[2],
            item: hub,
        },
    )
    .unwrap();
    let original = game.cities[&ranked[2]].queue.clone();
    let expected = controller
        .base
        .commercial_hub_step(&game, 0, ranked[1], 6, 0)
        .unwrap();
    controller.claim_conquest_campaign_cash_flow(&mut game, 0, &plan);
    assert_eq!(game.cities[&ranked[1]].queue.iter().next(), Some(&expected));
    assert_eq!(game.cities[&ranked[2]].queue, original);
}

#[test]
fn conquest_campaign_cash_flow_preserves_an_existing_market_or_trader_queue() {
    for kind in ["market", "trader"] {
        let (mut game, mut controller, plan, ranked) = cash_flow_campaign();
        cash_flow_install_hub(&mut game, &controller, ranked[1]);
        if kind == "market" {
            cash_flow_install_hub(&mut game, &controller, ranked[2]);
        } else {
            Arc::make_mut(&mut game.observed_trade_capacity).insert(0, 1);
        }
        let item = controller
            .base
            .commercial_hub_step(&game, 0, ranked[2], 6, 0)
            .unwrap();
        assert!(matches!(&item, Item::Building { .. } | Item::Unit { .. }));
        game.apply(
            0,
            &Action::Produce {
                city: ranked[2],
                item,
            },
        )
        .unwrap();
        let original = cash_flow_queues(&game);
        controller.claim_conquest_campaign_cash_flow(&mut game, 0, &plan);
        assert_eq!(cash_flow_queues(&game), original, "{kind}");
    }
}

#[test]
fn conquest_campaign_cash_flow_preserves_actual_and_paused_commitments() {
    for kind in ["actual", "paused"] {
        let (mut game, mut controller, plan, ranked) = cash_flow_campaign();
        cash_flow_install_hub(&mut game, &controller, ranked[1]);
        for city in game.player_city_ids(0) {
            if kind == "actual" {
                game.apply(
                    0,
                    &Action::Produce {
                        city,
                        item: Item::Unit {
                            unit: crate::name!("builder"),
                        },
                    },
                )
                .unwrap();
            } else {
                game.cities
                    .get_mut(&city)
                    .unwrap()
                    .production_progress
                    .insert("unit:settler".to_string(), 20.0);
            }
        }
        let original = cash_flow_queues(&game);
        controller.claim_conquest_campaign_cash_flow(&mut game, 0, &plan);
        assert_eq!(cash_flow_queues(&game), original, "{kind}");
        if kind == "paused" {
            assert!(game
                .player_city_ids(0)
                .iter()
                .all(
                    |city| game.cities[city].production_progress.get("unit:settler") == Some(&20.0)
                ));
        }
    }
}

#[test]
fn conquest_campaign_cash_flow_preserves_war_and_global_emergencies() {
    for blocked in ["war", "rush", "recovery", "threatened_city"] {
        let (mut game, mut controller, mut plan, ranked) = cash_flow_campaign();
        cash_flow_install_hub(&mut game, &controller, ranked[1]);
        match blocked {
            "war" => {
                let mut rival = game.players[0].clone();
                rival.id = game.players.len();
                rival.team = None;
                game.at_war.insert((0, rival.id));
                game.players.push(rival);
            }
            "rush" => plan.rush = true,
            "recovery" => plan.strategy = GrandStrategy::Recovery,
            "threatened_city" => plan.threatened_city = Some(ranked[0]),
            _ => unreachable!(),
        }
        controller.claim_conquest_campaign_cash_flow(&mut game, 0, &plan);
        assert!(
            cash_flow_queues(&game)
                .iter()
                .all(|(_, queue)| queue.is_empty()),
            "{blocked}"
        );
    }
}

#[test]
fn conquest_campaign_cash_flow_leaves_an_already_funded_army_unchanged() {
    let (mut game, mut controller, plan, ranked) = cash_flow_campaign();
    cash_flow_install_hub(&mut game, &controller, ranked[1]);
    game.players[0].gold = 10_000.0;
    assert!(!controller.domination_upgrade_funding_shortfall(&game, 0));
    controller.claim_conquest_campaign_cash_flow(&mut game, 0, &plan);
    assert!(cash_flow_queues(&game)
        .iter()
        .all(|(_, queue)| queue.is_empty()));
}

#[test]
fn conquest_campaign_cash_flow_preserves_other_victory_lanes() {
    for target in [
        VictoryTarget::Science,
        VictoryTarget::Culture,
        VictoryTarget::Religion,
        VictoryTarget::Diplomacy,
        VictoryTarget::Score,
    ] {
        let (mut game, controller, plan, ranked) = cash_flow_campaign();
        cash_flow_install_hub(&mut game, &controller, ranked[1]);
        let mut other = AdvancedAi::targeting(target);
        other.enable_commercial_hub_in_the_strategic_queue();
        other.claim_conquest_campaign_cash_flow(&mut game, 0, &plan);
        assert!(
            cash_flow_queues(&game)
                .iter()
                .all(|(_, queue)| queue.is_empty()),
            "{target:?}"
        );
    }
}

#[test]
fn conquest_campaign_cash_flow_keeps_the_existing_genes_window_and_defense_floor() {
    for blocked in ["gene", "victory_planning", "window", "army"] {
        let (mut game, mut controller, plan, ranked) = cash_flow_campaign();
        cash_flow_install_hub(&mut game, &controller, ranked[1]);
        match blocked {
            "gene" => controller.commercial_hub_in_the_strategic_queue = false,
            "victory_planning" => controller.victory_planning = false,
            "window" => game.turn = 1,
            "army" => {
                for unit in game.player_unit_ids(0) {
                    game.remove_unit(unit);
                }
            }
            _ => unreachable!(),
        }
        controller.claim_conquest_campaign_cash_flow(&mut game, 0, &plan);
        assert!(
            cash_flow_queues(&game)
                .iter()
                .all(|(_, queue)| queue.is_empty()),
            "{blocked}"
        );
    }
}

#[test]
fn conquest_campaign_cash_flow_does_not_take_the_capitals_idle_queue() {
    let (mut game, mut controller, plan, ranked) = cash_flow_campaign();
    assert!(game.cities[&ranked[0]].is_capital);
    cash_flow_install_hub(&mut game, &controller, ranked[0]);
    controller.claim_conquest_campaign_cash_flow(&mut game, 0, &plan);
    assert!(cash_flow_queues(&game)
        .iter()
        .all(|(_, queue)| queue.is_empty()));
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
