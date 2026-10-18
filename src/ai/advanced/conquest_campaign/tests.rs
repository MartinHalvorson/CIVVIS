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

fn preparation_campaign() -> (Game, AdvancedAi, StrategicPlan, Vec<u32>) {
    let (mut game, controller, _, nearby) = campaign();
    game.players[0].civ = "GranColombia".to_string();
    game.found_city_for(0, (25, 20), None);
    game.current = 0;
    game.turn = 114;
    game.game_speed = crate::setup::GameSpeed::Online;
    game.players[0]
        .techs
        .extend([crate::name!("writing"), crate::name!("stirrups")]);
    game.players[0]
        .policies
        .insert(crate::name!("professional_army"));
    game.players[0].gold = 1000.0;
    game.players[0].gold_per_turn = 30.0;
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 50.0);
    let units: Vec<u32> = (0..3)
        .map(|_| game.spawn_test_unit("heavy_chariot", 0, (30, 20)))
        .collect();
    assert_eq!(
        game.unit_upgrade_target(0, crate::name!("heavy_chariot")),
        Some(crate::name!("knight"))
    );
    assert_eq!(
        game.unit_gold_upgrade_offer(0, units[0]),
        Some((crate::name!("knight"), 80.0, 20.0))
    );
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: Some(2),
        target_city: Some(nearby),
        threatened_city: Some(game.player_city_ids(0)[0]),
        desired_cities: 10,
        assessed_turn: game.turn,
        rush: false,
    };
    (game, controller, plan, units)
}

#[test]
fn conquest_campaign_preparation_upgrades_only_two_current_frontliners() {
    let (mut game, controller, plan, units) = preparation_campaign();
    game.players[0].policies.insert(crate::name!("retinues"));
    assert_eq!(game.unit_gold_upgrade_offer(0, units[0]).unwrap().2, 10.0);
    controller.conquest_campaign_prepare_frontline(&mut game, 0, &plan);
    assert_eq!(
        units
            .iter()
            .filter(|unit| game.units[unit].kind == "knight")
            .count(),
        2
    );
    let remaining = *units
        .iter()
        .find(|unit| game.units[unit].kind == "heavy_chariot")
        .unwrap();
    assert!(game.unit_gold_upgrade_offer(0, remaining).is_some());
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 30.0);
    assert_eq!(game.players[0].gold, 840.0);
}

#[test]
fn conquest_campaign_preparation_routes_before_the_existing_funder_gate() {
    let (mut game, controller, plan, units) = preparation_campaign();
    game.players[0].gold = 111.0;
    controller.fund_domination_upgrades(&mut game, 0, &plan);
    assert_eq!(game.units[&units[0]].kind, "knight");
    assert_eq!(game.players[0].gold, 31.0);
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 30.0);
}

#[test]
fn conquest_campaign_preparation_keeps_the_emergency_bank() {
    let (mut game, controller, plan, units) = preparation_campaign();
    game.players[0].gold = 109.0;
    controller.conquest_campaign_prepare_frontline(&mut game, 0, &plan);
    assert!(units
        .iter()
        .all(|unit| game.units[unit].kind == "heavy_chariot"));
    assert_eq!(game.players[0].gold, 109.0);
}

#[test]
fn conquest_campaign_preparation_keeps_current_material_authoritative() {
    let (mut game, controller, plan, units) = preparation_campaign();
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 19.0);
    assert!(game.unit_gold_upgrade_offer(0, units[0]).is_none());
    controller.conquest_campaign_prepare_frontline(&mut game, 0, &plan);
    assert!(units
        .iter()
        .all(|unit| game.units[unit].kind == "heavy_chariot"));
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 19.0);
}

#[test]
fn conquest_campaign_preparation_respects_spent_and_foreign_offers() {
    let (mut game, controller, plan, units) = preparation_campaign();
    game.units.get_mut(&units[0]).unwrap().acted = true;
    game.units.get_mut(&units[1]).unwrap().moves_left = 0.0;
    game.units.get_mut(&units[2]).unwrap().pos = (35, 20);
    assert!(units
        .iter()
        .all(|unit| game.unit_gold_upgrade_offer(0, *unit).is_none()));
    controller.conquest_campaign_prepare_frontline(&mut game, 0, &plan);
    assert!(units
        .iter()
        .all(|unit| game.units[unit].kind == "heavy_chariot"));
}

#[test]
fn conquest_campaign_preparation_does_not_fund_badly_damaged_bodies() {
    let (mut game, controller, plan, units) = preparation_campaign();
    for unit in &units {
        game.units.get_mut(unit).unwrap().hp = 74;
    }
    controller.conquest_campaign_prepare_frontline(&mut game, 0, &plan);
    assert!(units
        .iter()
        .all(|unit| game.units[unit].kind == "heavy_chariot"));
}

#[test]
fn conquest_campaign_preparation_respects_existing_contemporary_frontliners() {
    let (mut game, controller, plan, units) = preparation_campaign();
    game.spawn_test_unit("knight", 0, (30, 20));
    game.spawn_test_unit("knight", 0, (30, 20));
    controller.conquest_campaign_prepare_frontline(&mut game, 0, &plan);
    assert!(units
        .iter()
        .all(|unit| game.units[unit].kind == "heavy_chariot"));
}

#[test]
fn conquest_campaign_preparation_must_carry_the_new_upkeep() {
    let (mut game, controller, plan, units) = preparation_campaign();
    game.players[0].gold_per_turn = 4.0;
    assert_eq!(
        AdvancedAi::upgrade_upkeep_delta(&game, units[0], crate::name!("knight")),
        3.0
    );
    controller.conquest_campaign_prepare_frontline(&mut game, 0, &plan);
    assert!(units
        .iter()
        .all(|unit| game.units[unit].kind == "heavy_chariot"));
}

#[test]
fn conquest_campaign_preparation_preserves_other_lanes_and_strategies() {
    let (mut game, controller, mut plan, units) = preparation_campaign();
    AdvancedAi::targeting(VictoryTarget::Science)
        .conquest_campaign_prepare_frontline(&mut game, 0, &plan);
    assert!(units
        .iter()
        .all(|unit| game.units[unit].kind == "heavy_chariot"));
    plan.strategy = GrandStrategy::Conquest;
    controller.conquest_campaign_prepare_frontline(&mut game, 0, &plan);
    assert!(units
        .iter()
        .all(|unit| game.units[unit].kind == "heavy_chariot"));
}

#[test]
fn conquest_campaign_preparation_requires_a_reachable_named_major() {
    let (game, controller, plan, units) = preparation_campaign();
    for distant in [false, true] {
        let mut control = game.clone();
        let mut inactive = plan.clone();
        inactive.target_player = if distant { Some(1) } else { None };
        inactive.target_city = if distant {
            Some(game.player_city_ids(1)[0])
        } else {
            None
        };
        controller.conquest_campaign_prepare_frontline(&mut control, 0, &inactive);
        assert!(units
            .iter()
            .all(|unit| control.units[unit].kind == "heavy_chariot"));
    }
}

#[test]
fn conquest_campaign_preparation_does_not_expand_an_active_major_war_budget() {
    let (mut game, controller, plan, units) = preparation_campaign();
    game.at_war.extend([(0, 2), (2, 0)]);
    controller.conquest_campaign_prepare_frontline(&mut game, 0, &plan);
    assert!(units
        .iter()
        .all(|unit| game.units[unit].kind == "heavy_chariot"));
}

#[test]
fn conquest_campaign_preparation_requires_the_actual_researched_successor() {
    let (mut game, controller, plan, units) = preparation_campaign();
    game.players[0].techs.remove(&crate::name!("stirrups"));
    assert!(game
        .unit_upgrade_target(0, crate::name!("heavy_chariot"))
        .is_none());
    controller.conquest_campaign_prepare_frontline(&mut game, 0, &plan);
    assert!(units
        .iter()
        .all(|unit| game.units[unit].kind == "heavy_chariot"));
}

#[test]
fn conquest_campaign_preparation_honors_host_refusal_and_current_gold_quote() {
    let (mut game, controller, plan, units) = preparation_campaign();
    for (index, unit_id) in units.iter().enumerate() {
        std::sync::Arc::make_mut(&mut game.host_unit_facts).insert(
            *unit_id,
            crate::game::HostUnitFacts {
                upgrade: Some(crate::game::HostUnitUpgrade {
                    to: Some(crate::name!("knight")),
                    cost: Some(125.0),
                    blocked: (index != 1).then(|| "host refusal".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
    }
    game.players[0].gold = 156.0;
    controller.conquest_campaign_prepare_frontline(&mut game, 0, &plan);
    assert_eq!(game.units[&units[0]].kind, "heavy_chariot");
    assert_eq!(game.units[&units[1]].kind, "knight");
    assert_eq!(game.units[&units[2]].kind, "heavy_chariot");
    assert_eq!(game.players[0].gold, 31.0);
}

#[test]
fn conquest_campaign_preparation_tracks_upkeep_between_actual_upgrades() {
    let (mut game, controller, plan, units) = preparation_campaign();
    game.players[0].gold_per_turn = 7.0;
    controller.conquest_campaign_prepare_frontline(&mut game, 0, &plan);
    assert_eq!(
        units
            .iter()
            .filter(|unit| game.units[unit].kind == "knight")
            .count(),
        1
    );
    assert!(game.unit_gold_upgrade_offer(0, units[1]).is_some());
    assert_eq!(game.players[0].gold, 920.0);
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 30.0);
}
