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
    for strategy in [
        GrandStrategy::Expansion,
        GrandStrategy::Recovery,
        GrandStrategy::Science,
    ] {
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
