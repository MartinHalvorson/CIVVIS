use super::*;

#[test]
fn conquest_campaign_frontline_respects_threats_and_recent_attacks() {
    let (game, controller, plan, cities) = frontline_campaign();
    let mut threatened = plan.clone();
    threatened.threatened_city = Some(cities[0]);
    assert_eq!(
        controller.claim_conquest_frontline(&mut game.clone(), 0, &threatened),
        None
    );
    let mut alarmed = controller.clone();
    alarmed.base.plan_threatened_city = Some(cities[0]);
    assert_eq!(
        alarmed.claim_conquest_frontline(&mut game.clone(), 0, &plan),
        None
    );
    let mut attacked = game;
    for city in cities {
        attacked.cities.get_mut(&city).unwrap().last_attacked = attacked.turn;
    }
    assert_eq!(
        controller.claim_conquest_frontline(&mut attacked, 0, &plan),
        None
    );
}

#[test]
fn conquest_campaign_frontline_budgets_the_military_upkeep_surcharge() {
    let (mut game, controller, plan, _) = frontline_campaign();
    game.players[0]
        .policies
        .insert(crate::name!("elite_forces"));
    assert_eq!(game.policy_effect(0, "unit_maintenance_surcharge"), 2.0);
    game.players[0].gold_per_turn = 13.9;
    assert_eq!(
        controller.claim_conquest_frontline(&mut game, 0, &plan),
        None
    );
    game.players[0].gold_per_turn = 14.0;
    assert!(controller
        .claim_conquest_frontline(&mut game, 0, &plan)
        .is_some());
}

#[test]
fn conquest_campaign_frontline_keeps_writing_and_other_victory_lanes_first() {
    let (game, controller, plan, _) = frontline_campaign();
    let mut unwritten = game.clone();
    unwritten.players[0].techs.remove(&crate::name!("writing"));
    assert_eq!(
        controller.claim_conquest_frontline(&mut unwritten, 0, &plan),
        None
    );
    let science = AdvancedAi::targeting(VictoryTarget::Science);
    assert_eq!(
        science.claim_conquest_frontline(&mut game.clone(), 0, &plan),
        None
    );
    let (mut small, _, _, _) = campaign();
    small.players[0].gold = 200.0;
    small.players[0].gold_per_turn = 20.0;
    small.players[0].techs.insert(crate::name!("writing"));
    small.players[0].techs.insert(crate::name!("metal_casting"));
    for _ in 0..3 {
        small.spawn_test_unit("heavy_chariot", 0, (30, 20));
    }
    assert_eq!(
        controller.claim_conquest_frontline(&mut small, 0, &plan),
        None
    );
}

fn frontline_campaign() -> (Game, AdvancedAi, StrategicPlan, Vec<u32>) {
    let (mut game, controller, _, nearby) = campaign();
    game.found_city_for(0, (25, 20), None);
    game.found_city_for(0, (20, 20), None);
    game.current = 0;
    game.turn = 100;
    game.players[0].gold = 200.0;
    game.players[0].gold_per_turn = 20.0;
    game.players[0].techs.insert(crate::name!("writing"));
    game.players[0].techs.insert(crate::name!("metal_casting"));
    for _ in 0..3 {
        game.spawn_test_unit("heavy_chariot", 0, (30, 20));
    }
    let cities = game.player_city_ids(0);
    for city in &cities {
        game.cities.get_mut(city).unwrap().queue.clear();
        game.cities
            .get_mut(city)
            .unwrap()
            .production_progress
            .clear();
    }
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(2),
        target_city: Some(nearby),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    assert!(game.can_produce(
        0,
        cities[1],
        &Item::Unit {
            unit: crate::name!("pike_and_shot")
        }
    ));
    (game, controller, plan, cities)
}

#[test]
fn conquest_campaign_frontline_claims_a_real_resource_free_upgrade() {
    let (mut game, controller, plan, _) = frontline_campaign();
    let (city, item) = controller
        .claim_conquest_frontline(&mut game, 0, &plan)
        .unwrap();
    assert!(!game.cities[&city].is_capital);
    assert_eq!(game.cities[&city].queue.first(), Some(&item));
    let Item::Unit { unit } = item else {
        panic!("expected unit")
    };
    let spec = &game.rules.units[unit];
    assert!(spec.strength >= 55.0);
    assert!(spec.requires_resource.is_none());
    assert!(!spec.has_ranged_attack());
}

#[test]
fn conquest_campaign_frontline_stops_after_two_queued_bodies_across_replans() {
    let (mut game, controller, plan, _) = frontline_campaign();
    assert!(controller
        .claim_conquest_frontline(&mut game, 0, &plan)
        .is_some());
    assert!(controller
        .claim_conquest_frontline(&mut game, 0, &plan)
        .is_some());
    assert_eq!(
        controller.claim_conquest_frontline(&mut game, 0, &plan),
        None
    );
}

#[test]
fn conquest_campaign_frontline_counts_already_fielded_modern_bodies() {
    let (mut game, controller, plan, _) = frontline_campaign();
    game.spawn_test_unit("pike_and_shot", 0, (30, 20));
    assert!(controller
        .claim_conquest_frontline(&mut game, 0, &plan)
        .is_some());
    assert_eq!(
        controller.claim_conquest_frontline(&mut game, 0, &plan),
        None
    );
}

#[test]
fn conquest_campaign_frontline_leaves_a_sufficient_modern_line_alone() {
    let (mut game, controller, plan, _) = frontline_campaign();
    for _ in 0..2 {
        game.spawn_test_unit("pike_and_shot", 0, (30, 20));
    }
    assert_eq!(
        controller.claim_conquest_frontline(&mut game, 0, &plan),
        None
    );
}

#[test]
fn conquest_campaign_frontline_requires_three_obsolete_melee_bodies() {
    let (game, controller, plan, _) = frontline_campaign();
    for replacement in [None, Some("archer"), Some("scout")] {
        let mut reduced = game.clone();
        reduced.remove_unit(reduced.player_unit_ids(0)[0]);
        if let Some(kind) = replacement {
            reduced.spawn_test_unit(kind, 0, (30, 20));
        }
        assert_eq!(
            controller.claim_conquest_frontline(&mut reduced, 0, &plan),
            None
        );
    }
}

#[test]
fn conquest_campaign_frontline_reserves_sustainable_un_discounted_upkeep() {
    let (game, controller, plan, _) = frontline_campaign();
    for (gold, income) in [(29.0, 20.0), (200.0, 9.9), (200.0, -1.0)] {
        let mut poor = game.clone();
        poor.players[0].gold = gold;
        poor.players[0].gold_per_turn = income;
        assert_eq!(
            controller.claim_conquest_frontline(&mut poor, 0, &plan),
            None
        );
    }
    let mut funded = game;
    funded.players[0].gold_per_turn = 10.0;
    assert!(controller
        .claim_conquest_frontline(&mut funded, 0, &plan)
        .is_some());
}

#[test]
fn conquest_campaign_frontline_respects_lane_war_and_plan_controls() {
    let (game, controller, plan, _) = frontline_campaign();
    for strategy in [GrandStrategy::Recovery, GrandStrategy::Science] {
        let mut changed = plan.clone();
        changed.strategy = strategy;
        assert_eq!(
            controller.claim_conquest_frontline(&mut game.clone(), 0, &changed),
            None
        );
    }
    let mut rush = plan.clone();
    rush.rush = true;
    assert_eq!(
        controller.claim_conquest_frontline(&mut game.clone(), 0, &rush),
        None
    );
    let mut off = controller.clone();
    off.victory_planning = false;
    assert_eq!(
        off.claim_conquest_frontline(&mut game.clone(), 0, &plan),
        None
    );
    off = controller.clone();
    off.victory_target = None;
    assert_eq!(
        off.claim_conquest_frontline(&mut game.clone(), 0, &plan),
        None
    );
    let mut war = game;
    war.at_war.insert((0, 2));
    assert_eq!(
        controller.claim_conquest_frontline(&mut war, 0, &plan),
        None
    );
}

#[test]
fn conquest_campaign_frontline_preserves_queued_and_paused_work() {
    let (game, controller, plan, cities) = frontline_campaign();
    for paused in [false, true] {
        let mut busy = game.clone();
        for city in &cities {
            let held = busy.cities.get_mut(city).unwrap();
            let item = Item::Unit {
                unit: crate::name!("builder"),
            };
            if paused {
                held.production_progress
                    .insert("unit:builder".to_string(), 1.0);
            } else {
                held.queue = vec![item];
            }
        }
        assert_eq!(
            controller.claim_conquest_frontline(&mut busy, 0, &plan),
            None
        );
    }
}

#[test]
fn conquest_campaign_frontline_requires_tech_and_a_reachable_frontier() {
    let (game, controller, plan, _) = frontline_campaign();
    let mut unready = game.clone();
    unready.players[0]
        .techs
        .remove(&crate::name!("metal_casting"));
    assert_eq!(
        controller.claim_conquest_frontline(&mut unready, 0, &plan),
        None
    );
    let mut isolated = game;
    for rival in 1..4 {
        isolated.players[rival].alive = false;
    }
    assert_eq!(
        controller.claim_conquest_frontline(&mut isolated, 0, &plan),
        None
    );
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
