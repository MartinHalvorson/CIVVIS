use super::*;

fn city_state_start(difficulty: &str, seed: u64) -> Game {
    let mut options = GameOptions::new(1, 32, 20, seed, 120, 1);
    options.barbarians = false;
    options.difficulty = difficulty.to_string();
    Game::new_with(options)
}

fn settled_two_player_game(seed: u64) -> Game {
    let mut game = Game::new_full(2, 24, 16, seed, 120, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
    }
    game
}

#[test]
fn city_state_starting_defenses_follow_difficulty() {
    for (difficulty, warriors, walls) in [
        ("prince", 2_usize, false),
        ("emperor", 3_usize, false),
        ("immortal", 4_usize, true),
        ("deity", 5_usize, true),
    ] {
        let game = city_state_start(difficulty, 551_000 + warriors as u64);
        let minor = game
            .players
            .iter()
            .find(|player| player.is_minor && !player.is_barbarian)
            .unwrap()
            .id;
        let city = game.player_city_ids(minor)[0];
        let army = game
            .units
            .values()
            .filter(|unit| unit.owner == minor)
            .collect::<Vec<_>>();

        assert_eq!(army.len(), warriors, "{difficulty} city-state army");
        assert!(army.iter().all(|unit| unit.kind == "warrior"));
        assert_eq!(
            game.cities[&city]
                .buildings
                .contains(&crate::name!("walls")),
            walls,
            "{difficulty} starting Walls"
        );
        assert_eq!(game.cities[&city].wall_hp, if walls { 100 } else { 0 });
    }
}

#[test]
fn city_states_cannot_settle_seek_wonders_run_projects_or_declare_war() {
    let mut game = settled_two_player_game(551_010);
    let city = game.player_city_ids(0)[0];
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.players[0].is_minor = true;

    assert!(!game.can_produce(
        0,
        city,
        &Item::Unit {
            unit: crate::name!("settler")
        }
    ));
    assert!(!game.can_found_city(settler));
    assert_eq!(
        game.do_found_city(0, settler).unwrap_err(),
        "city-states do not found cities"
    );
    assert!(!game.can_produce(
        0,
        city,
        &Item::Wonder {
            wonder: crate::name!("pyramids"),
            pos: game.cities[&city].pos,
        }
    ));
    assert!(!game.can_produce(
        0,
        city,
        &Item::Project {
            project: crate::name!("campus_research_grants")
        }
    ));
    assert!(!game
        .legal_actions(0)
        .iter()
        .any(|action| matches!(action, Action::DeclareWar { .. } | Action::FoundCity { .. })));
    assert_eq!(
        game.apply(0, &Action::DeclareWar { player: 1 })
            .unwrap_err(),
        "city-states do not declare war independently"
    );

    game.turn = 4;
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    game.cities.get_mut(&city).unwrap().wall_hp = 50;
    assert!(game.can_produce(
        0,
        city,
        &Item::Project {
            project: crate::name!("repair_outer_defenses")
        }
    ));
}

#[test]
fn city_states_must_raze_captures_unless_the_city_is_unrazable() {
    let mut raze = settled_two_player_game(551_020);
    raze.players[0].is_minor = true;
    let captured = raze.player_city_ids(1)[0];
    {
        let city = raze.cities.get_mut(&captured).unwrap();
        city.owner = 0;
        city.captured_from = Some(1);
        city.is_capital = false;
    }
    assert_eq!(
        raze.legal_city_disposition_actions(0),
        vec![Action::RazeCity { city: captured }]
    );
    assert_eq!(
        raze.do_keep_city(0, captured).unwrap_err(),
        "city-states must raze captured cities when possible"
    );
    raze.apply(0, &Action::RazeCity { city: captured }).unwrap();
    assert!(!raze.cities.contains_key(&captured));

    let mut keep = settled_two_player_game(551_021);
    keep.players[0].is_minor = true;
    let capital = keep.player_city_ids(1)[0];
    {
        let city = keep.cities.get_mut(&capital).unwrap();
        city.owner = 0;
        city.captured_from = Some(1);
    }
    assert_eq!(
        keep.legal_city_disposition_actions(0),
        vec![Action::KeepCity { city: capital }]
    );
    keep.apply(0, &Action::KeepCity { city: capital }).unwrap();
    assert_eq!(keep.cities[&capital].captured_from, None);
}
