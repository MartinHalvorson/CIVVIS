use super::*;

fn war_board(city_count: usize) -> Game {
    let mut game = Game::new_full(4, 64, 40, 91_105, 300, 0, false);
    assert!(game.cities.is_empty());
    for unit_id in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit_id);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
    }
    for position in [(8, 8), (14, 8), (8, 14), (14, 14)]
        .into_iter()
        .take(city_count)
    {
        game.found_city_for(0, crate::hex::offset_to_axial(position.0, position.1), None);
    }
    for (owner, position) in [(1, (38, 10)), (2, (45, 24)), (3, (25, 30))] {
        game.found_city_for(
            owner,
            crate::hex::offset_to_axial(position.0, position.1),
            None,
        );
        game.record_contact(0, owner);
    }
    for owner in 0..4 {
        let city_id = game.player_city_ids(owner)[0];
        let center = game.cities[&city_id].pos;
        let count = if owner == 0 { 5 } else { 4 };
        for position in game.wring(center, 2).into_iter().take(count) {
            game.spawn_test_unit("warrior", owner, position);
        }
    }
    game.current = 0;
    game.turn = 70;
    game.at_war.insert((0, 1));
    assert_eq!(game.player_city_ids(0).len(), city_count);
    let ours = game.military_power(0);
    let theirs = game.military_power(1);
    assert!(ours * 1.25 >= theirs);
    assert!(ours <= theirs * 1.80 + 20.0);
    game
}

fn controller(target: VictoryTarget, enabled: bool) -> AdvancedAi {
    let mut controller = AdvancedAi::targeting(target);
    controller.enable_rapid_city_expansion_2();
    controller.enable_expansion_scales_with_difficulty();
    controller.enable_domination_lane_hands_over();
    if enabled {
        controller.enable_domination_war_hands_over();
    }
    controller
}

fn bounded_raid(controller: &mut AdvancedAi, turn: u32) {
    controller.raid_war = Some(super::opportunistic_war::RaidWar {
        target: 1,
        declared: turn - 2,
        value: 150.0,
        settlers: 1,
        builders: 0,
        pillage_tiles: 0,
    });
}

#[test]
fn domination_war_handover_is_registered_and_off_in_both_controllers() {
    super::test_support::opt_in_off_in_both_controllers(
        "domination-war-hands-over",
        |controller| controller.domination_war_hands_over,
    );
}

#[test]
fn an_existing_major_war_hands_the_domination_lane_over_before_the_city_target() {
    let game = war_board(DOMINATION_HANDOVER_CITIES);
    let baseline = controller(VictoryTarget::Domination, false).assess(&game, 0);
    assert!(baseline.desired_cities > game.player_city_ids(0).len());
    assert_eq!(baseline.strategy, GrandStrategy::Expansion, "{baseline:?}");
    assert!(baseline.threatened_city.is_none());
    let treatment = controller(VictoryTarget::Domination, true).assess(&game, 0);
    assert_eq!(treatment.strategy, GrandStrategy::Conquest, "{treatment:?}");
    assert_eq!(treatment.desired_cities, baseline.desired_cities);
}

#[test]
fn a_second_major_war_is_not_hidden_by_an_existing_bounded_raid() {
    let mut game = war_board(DOMINATION_HANDOVER_CITIES);
    game.at_war.insert((0, 2));
    let mut baseline = controller(VictoryTarget::Domination, false);
    bounded_raid(&mut baseline, game.turn);
    assert!(!baseline.raid_only_war(&game, 0));
    assert_eq!(baseline.assess(&game, 0).strategy, GrandStrategy::Expansion);
    let mut treatment = controller(VictoryTarget::Domination, true);
    bounded_raid(&mut treatment, game.turn);
    assert_eq!(treatment.assess(&game, 0).strategy, GrandStrategy::Conquest);
}

#[test]
fn peace_keeps_the_existing_prewar_power_bar() {
    let mut game = war_board(DOMINATION_HANDOVER_CITIES);
    game.at_war.clear();
    let baseline = controller(VictoryTarget::Domination, false).assess(&game, 0);
    assert_eq!(baseline.strategy, GrandStrategy::Expansion);
    assert_eq!(
        controller(VictoryTarget::Domination, true).assess(&game, 0),
        baseline
    );
}

#[test]
fn an_outgunned_empire_keeps_recovery_ahead_of_the_new_handover() {
    let mut game = war_board(DOMINATION_HANDOVER_CITIES);
    let city_id = game.player_city_ids(1)[0];
    let center = game.cities[&city_id].pos;
    for position in game.wring(center, 3).into_iter().take(8) {
        game.spawn_test_unit("warrior", 1, position);
    }
    assert!(game.military_power(0) * 1.25 < game.military_power(1));
    let baseline = controller(VictoryTarget::Domination, false).assess(&game, 0);
    assert_eq!(baseline.strategy, GrandStrategy::Recovery);
    assert_eq!(
        controller(VictoryTarget::Domination, true).assess(&game, 0),
        baseline
    );
}

#[test]
fn a_threatened_city_keeps_recovery_ahead_of_campaign_handover() {
    let mut game = war_board(DOMINATION_HANDOVER_CITIES);
    let city_id = game.player_city_ids(0)[0];
    let center = game.cities[&city_id].pos;
    game.cities.get_mut(&city_id).unwrap().hp = 10;
    let positions = game
        .wring(center, 1)
        .into_iter()
        .filter(|position| game.unit_ids_at(*position).is_empty())
        .take(3)
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 3);
    for position in positions {
        assert!(game.player_can_see(0, position));
        game.spawn_test_unit("swordsman", 1, position);
    }
    let reserve = crate::hex::offset_to_axial(22, 20);
    for position in game.wring(reserve, 2).into_iter().take(4) {
        assert!(game.wdist(center, position) > 6);
        game.spawn_test_unit("warrior", 0, position);
    }
    assert!(game.military_power(0) * 1.25 >= game.military_power(1));
    assert!(AdvancedAi::city_pressure(&game, 0, city_id) >= BASTION_PRESSURE);
    let baseline = controller(VictoryTarget::Domination, false).assess(&game, 0);
    assert!(baseline.threatened_city.is_some());
    assert_eq!(baseline.strategy, GrandStrategy::Recovery);
    assert_eq!(
        controller(VictoryTarget::Domination, true).assess(&game, 0),
        baseline
    );
}

#[test]
fn a_bounded_raid_does_not_expand_into_a_conquest_campaign() {
    let game = war_board(DOMINATION_HANDOVER_CITIES);
    let mut baseline = controller(VictoryTarget::Domination, false);
    bounded_raid(&mut baseline, game.turn);
    assert!(baseline.raid_only_war(&game, 0));
    let mut treatment = controller(VictoryTarget::Domination, true);
    bounded_raid(&mut treatment, game.turn);
    assert_eq!(treatment.assess(&game, 0), baseline.assess(&game, 0));
}

#[test]
fn the_opening_city_band_is_still_required() {
    for city_count in [1, DOMINATION_HANDOVER_CITIES - 1] {
        let game = war_board(city_count);
        assert_eq!(
            controller(VictoryTarget::Domination, true).assess(&game, 0),
            controller(VictoryTarget::Domination, false).assess(&game, 0)
        );
    }
}

#[test]
fn a_minor_barbarian_or_defeated_rival_is_not_an_active_major_campaign() {
    for classification in ["minor", "barbarian", "defeated"] {
        let mut game = war_board(DOMINATION_HANDOVER_CITIES);
        match classification {
            "minor" => game.players[1].is_minor = true,
            "barbarian" => game.players[1].is_barbarian = true,
            "defeated" => game.players[1].alive = false,
            _ => unreachable!(),
        }
        assert_eq!(
            controller(VictoryTarget::Domination, true).assess(&game, 0),
            controller(VictoryTarget::Domination, false).assess(&game, 0),
            "{classification}"
        );
    }
}

#[test]
fn assigned_non_domination_lanes_keep_their_existing_strategy() {
    let game = war_board(DOMINATION_HANDOVER_CITIES);
    for target in [
        VictoryTarget::Science,
        VictoryTarget::Culture,
        VictoryTarget::Religion,
    ] {
        assert_eq!(
            controller(target, true).assess(&game, 0),
            controller(target, false).assess(&game, 0),
            "{target:?}"
        );
    }
}
