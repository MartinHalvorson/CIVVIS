use super::*;

fn fixture() -> (Game, u32, u32, Pos, Pos) {
    let mut game = Game::new_full(2, 40, 24, 388200, 300, 0, false);
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    game.found_city_for(0, (6, 12), None);
    let enemy = game.found_city_for(1, (18, 12), None);
    let denied = (8, 12);
    let alternative = (7, 13);
    for target in [denied, alternative] {
        let tile = game.map.tiles.get_mut(&target).unwrap();
        tile.owner_city = Some(enemy);
        tile.improvement = Some(crate::name!("mine"));
        tile.pillaged = false;
    }
    let bomber = game.spawn_test_unit("bomber", 0, (6, 12));
    let other = game.spawn_test_unit("bomber", 0, (6, 12));
    game.at_war.clear();
    game.at_war.insert((0, 1));
    game.current = 0;
    game.turn = 200;
    game.players[0]
        .explored
        .extend(game.map.tiles.keys().copied());
    (game, bomber, other, denied, alternative)
}

#[test]
fn a_blocked_air_pillage_cannot_spend_movement_or_damage_a_layer() {
    let (mut game, bomber, _, denied, _) = fixture();
    let action = Action::AirPillage {
        unit: bomber,
        target: denied,
    };
    assert!(game.legal_doctrine_actions(0, bomber).contains(&action));
    let before = (
        game.units[&bomber].moves_left,
        game.units[&bomber].attacks_left,
        game.log.len(),
    );
    std::sync::Arc::make_mut(&mut game.blocked_strikes).insert((bomber, denied));
    assert!(game.apply(0, &action).is_err());
    assert_eq!(
        (
            game.units[&bomber].moves_left,
            game.units[&bomber].attacks_left,
            game.log.len()
        ),
        before
    );
    assert!(!game.map.tiles[&denied].pillaged);
}

#[test]
fn both_air_action_enumerators_offer_an_alternative_not_the_blocked_pillage() {
    let (mut game, bomber, other, denied, alternative) = fixture();
    std::sync::Arc::make_mut(&mut game.blocked_strikes).insert((bomber, denied));
    let denied_action = Action::AirPillage {
        unit: bomber,
        target: denied,
    };
    let alternative_action = Action::AirPillage {
        unit: bomber,
        target: alternative,
    };
    for actions in [
        game.legal_doctrine_actions(0, bomber),
        game.legal_actions(0),
    ] {
        assert!(!actions.contains(&denied_action));
        assert!(actions.contains(&alternative_action));
    }
    assert!(game
        .legal_doctrine_actions(0, other)
        .contains(&Action::AirPillage {
            unit: other,
            target: denied
        }));
    game.apply(0, &alternative_action).unwrap();
    assert!(game.map.tiles[&alternative].pillaged);
    assert!(!game.map.tiles[&denied].pillaged);
}

#[test]
fn clearing_host_refusal_facts_restores_the_exact_pillage() {
    let (mut game, bomber, _, denied, _) = fixture();
    std::sync::Arc::make_mut(&mut game.blocked_strikes).insert((bomber, denied));
    game.blocked_strikes = Default::default();
    let action = Action::AirPillage {
        unit: bomber,
        target: denied,
    };
    assert!(game.legal_doctrine_actions(0, bomber).contains(&action));
    game.apply(0, &action).unwrap();
    assert!(game.map.tiles[&denied].pillaged);
}

#[test]
fn a_blocked_air_strike_cannot_spend_the_sortie_but_another_target_can() {
    let (mut game, bomber, other, denied, alternative) = fixture();
    for target in [denied, alternative] {
        game.spawn_test_unit("warrior", 1, target);
    }
    let denied_action = Action::AirStrike {
        unit: bomber,
        target: denied,
    };
    let alternative_action = Action::AirStrike {
        unit: bomber,
        target: alternative,
    };
    assert!(game
        .legal_doctrine_actions(0, bomber)
        .contains(&denied_action));
    let before = (
        game.units[&bomber].moves_left,
        game.units[&bomber].attacks_left,
        game.log.len(),
    );
    std::sync::Arc::make_mut(&mut game.blocked_strikes).insert((bomber, denied));
    assert!(game.apply(0, &denied_action).is_err());
    assert_eq!(
        (
            game.units[&bomber].moves_left,
            game.units[&bomber].attacks_left,
            game.log.len()
        ),
        before
    );
    for actions in [
        game.legal_doctrine_actions(0, bomber),
        game.legal_actions(0),
    ] {
        assert!(!actions.contains(&denied_action));
        assert!(actions.contains(&alternative_action));
    }
    assert!(game
        .legal_doctrine_actions(0, other)
        .contains(&Action::AirStrike {
            unit: other,
            target: denied
        }));
    game.apply(0, &alternative_action).unwrap();
}
