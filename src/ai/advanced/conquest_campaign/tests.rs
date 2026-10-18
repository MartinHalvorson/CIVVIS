use super::*;

#[test]
fn conquest_campaign_material_respects_the_unique_successor_resource_bill() {
    let (mut game, controller, _, units) = material_campaign();
    game.players[0].civ = "Rome".to_string();
    assert_eq!(
        game.unit_upgrade_target(0, crate::name!("warrior")),
        Some(crate::name!("legion"))
    );
    assert_eq!(
        game.unit_upgrade_resource_price(0, units[0], crate::name!("legion")),
        Some(10.0)
    );
    assert_eq!(
        controller.conquest_campaign_material_upgrade_policy(&game, 0),
        None
    );
}

fn material_campaign() -> (Game, AdvancedAi, StrategicPlan, Vec<u32>) {
    let (mut game, mut controller, _, nearby) = campaign();
    game.players[0].civ = "GranColombia".to_string();
    game.found_city_for(0, (25, 20), None);
    game.found_city_for(0, (20, 20), None);
    game.current = 0;
    game.turn = 125;
    game.players[0].gold = 100.0;
    game.players[0].gold_per_turn = 20.0;
    game.players[0].techs.insert(crate::name!("iron_working"));
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 12.0);
    game.players[0].civics.extend(
        [
            "code_of_laws",
            "craftsmanship",
            "foreign_trade",
            "early_empire",
            "state_workforce",
            "political_philosophy",
            "military_tradition",
            "games_recreation",
            "defensive_tactics",
            "feudalism",
            "mercenaries",
            "divine_right",
        ]
        .into_iter()
        .map(Name::new),
    );
    game.players[0].government = Some("monarchy".into());
    game.players[0].policies = ["conscription", "professional_army"]
        .into_iter()
        .map(Name::new)
        .collect();
    let units: Vec<u32> = (0..3)
        .map(|_| game.spawn_test_unit("warrior", 0, (30, 20)))
        .collect();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(2),
        target_city: Some(nearby),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    controller.policy_deck_hysteresis_2 = true;
    controller.plan = Some(plan.clone());
    assert!(game
        .available_policies(0)
        .contains(&crate::name!("retinues")));
    assert_eq!(
        game.unit_upgrade_target(0, crate::name!("warrior")),
        Some(crate::name!("swordsman"))
    );
    assert_eq!(
        game.unit_upgrade_resource_price(0, units[0], crate::name!("swordsman")),
        Some(20.0)
    );
    (game, controller, plan, units)
}

#[test]
fn conquest_campaign_material_requests_an_affordable_resource_discount() {
    let (game, controller, _, _) = material_campaign();
    let policies = game.players[0].policies.clone();
    assert_eq!(
        controller.conquest_campaign_material_upgrade_policy(&game, 0),
        Some("retinues")
    );
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 12.0);
    assert_eq!(game.players[0].policies, policies);
}

#[test]
fn conquest_campaign_material_retains_the_card_while_its_stock_gap_persists() {
    let (mut game, controller, _, _) = material_campaign();
    game.players[0].policies.insert(crate::name!("retinues"));
    assert_eq!(
        controller.conquest_campaign_material_upgrade_policy(&game, 0),
        Some("retinues")
    );
}

#[test]
fn conquest_campaign_material_rejects_sufficient_or_insufficient_stock() {
    let (game, controller, _, _) = material_campaign();
    for stock in [0.0, 9.9, 20.0, 100.0] {
        let mut changed = game.clone();
        changed.players[0]
            .strategic_resources
            .insert(crate::name!("iron"), stock);
        assert_eq!(
            controller.conquest_campaign_material_upgrade_policy(&changed, 0),
            None
        );
    }
}

#[test]
fn conquest_campaign_material_requires_current_cash_above_the_existing_reserve() {
    let (mut game, controller, _, units) = material_campaign();
    let quote = game
        .unit_upgrade_price_in_formation(
            0,
            game.units[&units[0]].kind,
            crate::name!("swordsman"),
            0,
        )
        .unwrap()
        .0;
    game.players[0].gold = quote + 29.9;
    assert_eq!(
        controller.conquest_campaign_material_upgrade_policy(&game, 0),
        None
    );
    game.players[0].gold = quote + 30.0;
    assert_eq!(
        controller.conquest_campaign_material_upgrade_policy(&game, 0),
        Some("retinues")
    );
}

#[test]
fn conquest_campaign_material_requires_researched_targets_available_card_and_a_cohort() {
    let (game, controller, _, units) = material_campaign();
    let mut unresearched = game.clone();
    unresearched.players[0]
        .techs
        .remove(&crate::name!("iron_working"));
    assert_eq!(
        controller.conquest_campaign_material_upgrade_policy(&unresearched, 0),
        None
    );
    let mut locked = game.clone();
    locked.players[0]
        .civics
        .remove(&crate::name!("mercenaries"));
    assert_eq!(
        controller.conquest_campaign_material_upgrade_policy(&locked, 0),
        None
    );
    let mut single = game;
    for unit in &units[..2] {
        single.remove_unit(*unit);
    }
    assert_eq!(
        controller.conquest_campaign_material_upgrade_policy(&single, 0),
        None
    );
}

#[test]
fn conquest_campaign_material_preserves_other_lanes_and_deficit_controls() {
    let (game, controller, _, _) = material_campaign();
    let mut off = controller.clone();
    off.victory_planning = false;
    assert_eq!(
        off.conquest_campaign_material_upgrade_policy(&game, 0),
        None
    );
    off = controller.clone();
    off.victory_target = None;
    assert_eq!(
        off.conquest_campaign_material_upgrade_policy(&game, 0),
        None
    );
    let science = AdvancedAi::targeting(VictoryTarget::Science);
    assert_eq!(
        science.conquest_campaign_material_upgrade_policy(&game, 0),
        None
    );
    let mut drained = game;
    drained.players[0].gold_per_turn = -0.1;
    assert_eq!(
        controller.conquest_campaign_material_upgrade_policy(&drained, 0),
        None
    );
}

#[test]
fn conquest_campaign_material_counts_canonical_formation_bodies() {
    let (mut game, controller, _, units) = material_campaign();
    for unit in units {
        game.units.get_mut(&unit).unwrap().formation = 1;
    }
    game.players[0].gold = 1000.0;
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 19.9);
    assert_eq!(
        controller.conquest_campaign_material_upgrade_policy(&game, 0),
        None
    );
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 20.0);
    assert_eq!(
        controller.conquest_campaign_material_upgrade_policy(&game, 0),
        Some("retinues")
    );
}

#[test]
fn conquest_campaign_material_rejects_a_redundant_card_or_resource_free_cohort() {
    let (game, controller, _, units) = material_campaign();
    let mut modern = game.clone();
    modern.players[0]
        .policies
        .insert(crate::name!("force_modernization"));
    assert_eq!(
        controller.conquest_campaign_material_upgrade_policy(&modern, 0),
        None
    );
    for kind in ["archer", "scout", "galley"] {
        let mut other = game.clone();
        other.players[0]
            .techs
            .extend(["machinery", "cartography"].into_iter().map(Name::new));
        for unit in &units {
            other.units.get_mut(unit).unwrap().kind = Name::new(kind);
        }
        assert_eq!(
            controller.conquest_campaign_material_upgrade_policy(&other, 0),
            None
        );
    }
}

#[test]
fn conquest_campaign_material_policy_deck_keeps_both_discounts_and_maintenance() {
    let (mut game, controller, _, _) = material_campaign();
    for _ in 0..3 {
        controller.strategic_policies(&mut game, 0, GrandStrategy::Conquest);
        for card in ["conscription", "professional_army", "retinues"] {
            assert!(
                game.players[0].policies.contains(&Name::new(card)),
                "missing {card}"
            );
        }
    }
}

#[test]
fn conquest_campaign_material_funding_spends_only_the_shared_observed_bank() {
    let (mut game, controller, plan, units) = material_campaign();
    controller.fund_domination_upgrades(&mut game, 0, &plan);
    assert_eq!(
        units
            .iter()
            .filter(|unit| game.units[unit].kind == "swordsman")
            .count(),
        1
    );
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 2.0);
    assert!(game.players[0].gold >= 30.0);
}

#[test]
fn conquest_campaign_material_waits_for_fresh_authoritative_resource_quotes() {
    let (mut game, controller, plan, units) = material_campaign();
    for unit in &units {
        Arc::make_mut(&mut game.host_unit_facts).insert(
            *unit,
            crate::game::HostUnitFacts {
                upgrade: Some(crate::game::HostUnitUpgrade {
                    to: Some(crate::name!("swordsman")),
                    cost: Some(25.0),
                    resources: Some(crate::game::HostUnitUpgradeResource {
                        resource: Some(crate::name!("iron")),
                        cost: 20.0,
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
    }
    controller.fund_domination_upgrades(&mut game, 0, &plan);
    assert!(game.players[0].policies.contains(&crate::name!("retinues")));
    assert!(units.iter().all(|unit| game.units[unit].kind == "warrior"));
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 12.0);
    assert!(units.iter().all(|unit| game.unit_upgrade_resource_price(
        0,
        *unit,
        crate::name!("swordsman")
    ) == Some(20.0)));
    game.turn += 1;
    for unit in &units {
        Arc::make_mut(&mut game.host_unit_facts)
            .get_mut(unit)
            .unwrap()
            .upgrade
            .as_mut()
            .unwrap()
            .resources
            .as_mut()
            .unwrap()
            .cost = 10.0;
    }
    controller.fund_domination_upgrades(&mut game, 0, &plan);
    assert_eq!(
        units
            .iter()
            .filter(|unit| game.units[unit].kind == "swordsman")
            .count(),
        1
    );
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 2.0);
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
