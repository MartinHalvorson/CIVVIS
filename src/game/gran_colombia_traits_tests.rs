use super::*;

fn fixture(civ: &str) -> Game {
    let mut game = Game::new_full(2, 24, 16, 590_106, 250, 0, false);
    let units: Vec<_> = game.units.keys().copied().collect();
    for id in units {
        game.remove_unit(id);
    }
    game.barb_camps.clear();
    game.map.clear_rivers();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.owner_city = None;
        tile.road = 0;
    }
    for player in game.players.iter_mut() {
        player.civ = "Rome".to_string();
        player.government = None;
        player.policies.clear();
        player.techs.clear();
        player.civics.clear();
    }
    game.players[0].civ = civ.to_string();
    game.handicap_exempt.extend([0, 1]);
    game.found_city_for(0, (4, 4), None);
    game.found_city_for(1, (18, 10), None);
    game.current = 0;
    game
}

#[test]
fn the_movement_trait_reaches_military_civilian_religious_naval_and_air_units() {
    let mut game = fixture("Gran Colombia");
    for kind in [
        "warrior",
        "scout",
        "settler",
        "builder",
        "missionary",
        "galley",
        "bomber",
    ] {
        let pos = if kind == "galley" { (11, 6) } else { (10, 6) };
        if kind == "galley" {
            game.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("coast");
        }
        let id = game.spawn_test_unit(kind, 0, pos);
        assert_eq!(
            game.unit_max_moves(id),
            game.rules.units[kind].moves + 1.0,
            "{kind}"
        );
    }
}

#[test]
fn a_fresh_gran_colombia_unit_receives_the_extra_point_immediately() {
    let mut game = fixture("Gran Colombia");
    for kind in ["warrior", "settler"] {
        let id = game.spawn_test_unit(kind, 0, (10, 6));
        assert_eq!(game.units[&id].moves_left, 3.0, "{kind}");
    }
}

#[test]
fn turn_refresh_pays_the_trait_only_to_its_owner() {
    let mut game = fixture("Gran Colombia");
    let ours = game.spawn_test_unit("warrior", 0, (10, 6));
    let other = game.spawn_test_unit("warrior", 1, (14, 8));
    game.units.get_mut(&ours).unwrap().moves_left = 0.0;
    game.units.get_mut(&other).unwrap().moves_left = 0.0;
    game.begin_turn(0);
    game.begin_turn(1);
    assert_eq!(game.units[&other].moves_left, 2.0);
    assert_eq!(game.units[&ours].moves_left, 3.0);
}

#[test]
fn changing_owners_changes_the_modeled_allowance() {
    let mut game = fixture("Gran Colombia");
    let id = game.spawn_test_unit("warrior", 0, (10, 6));
    assert_eq!(game.unit_max_moves(id), 3.0);
    game.units.get_mut(&id).unwrap().owner = 1;
    assert_eq!(game.unit_max_moves(id), 2.0);
}

#[test]
fn host_maximum_is_authoritative_and_invalid_readings_fall_back_to_the_trait() {
    let mut game = fixture("Gran Colombia");
    let id = game.spawn_test_unit("warrior", 0, (10, 6));
    for (reading, expected) in [
        (Some(3.0), 3.0),
        (Some(7.5), 7.5),
        (Some(0.0), 3.0),
        (Some(-1.0), 3.0),
        (None, 3.0),
    ] {
        Arc::make_mut(&mut game.host_unit_facts).insert(
            id,
            HostUnitFacts {
                max_moves: reading,
                ..Default::default()
            },
        );
        assert_eq!(game.unit_max_moves(id), expected, "{reading:?}");
    }
}

#[test]
fn promotion_movement_stacks_once_with_the_civilization_trait() {
    let mut game = fixture("Gran Colombia");
    let id = game.spawn_test_unit("warrior", 0, (10, 6));
    game.units
        .get_mut(&id)
        .unwrap()
        .promotions
        .insert(crate::name!("commando"));
    assert_eq!(game.unit_max_moves(id), 4.0);
}

#[test]
fn an_embarked_land_unit_also_receives_the_movement_trait() {
    let mut game = fixture("Gran Colombia");
    let pos = (10, 6);
    game.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("coast");
    let id = game.spawn_test_unit("warrior", 0, pos);
    assert!(game.is_embarked(&game.units[&id]));
    assert_eq!(game.unit_max_moves(id), 3.0);
}

#[test]
fn the_trait_does_not_invent_an_unconditional_combat_strength_bonus() {
    let mut game = fixture("Gran Colombia");
    let id = game.spawn_test_unit("warrior", 0, (10, 6));
    assert_eq!(game.unit_strength(&game.units[&id], false), 20.0);
    game.players[0].civ = "Rome".to_string();
    assert_eq!(game.unit_strength(&game.units[&id], false), 20.0);
}

#[test]
fn the_trait_does_not_invent_combat_experience_but_nubia_keeps_its_ranged_bonus() {
    let mut game = fixture("Gran Colombia");
    let id = game.spawn_test_unit("archer", 0, (10, 6));
    assert_eq!(game.modified_xp(id, 3.0), 3);
    game.players[0].civ = "Rome".to_string();
    assert_eq!(game.modified_xp(id, 3.0), 3);
    game.players[0].civ = "Nubia".to_string();
    assert_eq!(game.modified_xp(id, 3.0), 5);
}

#[test]
fn the_additional_point_pays_for_a_third_plain_tile_move() {
    let mut game = fixture("Gran Colombia");
    let id = game.spawn_test_unit("warrior", 0, (10, 6));
    game.begin_turn(0);
    for to in [(11, 6), (12, 6), (13, 6)] {
        game.apply(0, &Action::Move { unit: id, to }).unwrap();
    }
    assert_eq!(game.units[&id].pos, (13, 6));
    assert_eq!(game.units[&id].moves_left, 0.0);
}

#[test]
fn an_ordinary_civilization_still_pays_its_normal_two_point_budget() {
    let mut game = fixture("Rome");
    let id = game.spawn_test_unit("warrior", 0, (10, 6));
    game.begin_turn(0);
    for to in [(11, 6), (12, 6)] {
        game.apply(0, &Action::Move { unit: id, to }).unwrap();
    }
    assert!(game
        .apply(
            0,
            &Action::Move {
                unit: id,
                to: (13, 6)
            }
        )
        .is_err());
    assert_eq!(game.units[&id].pos, (12, 6));
    assert_eq!(game.unit_max_moves(id), 2.0);
}
