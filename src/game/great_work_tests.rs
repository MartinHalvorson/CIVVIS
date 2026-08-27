use super::*;

fn game_with_capital(seed: u64) -> (Game, u32) {
    let mut game = Game::new_full(1, 24, 16, seed, 300, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    install_test_district(&mut game, city, "theater_square");
    (game, city)
}

#[test]
fn great_works_obey_typed_and_universal_slots() {
    let (mut game, city) = game_with_capital(4_121);
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("archaeological_museum"));
    game.players[0]
        .counters
        .insert("great_work:relic".to_string(), 1);
    game.players[0]
        .counters
        .insert("great_work:writing".to_string(), 1);

    let housed = game.housed_great_works(0);
    assert_eq!(housed[&city].get("relic"), Some(&1));
    assert_eq!(
        housed[&city].get("writing"),
        None,
        "Writing cannot occupy an Artifact slot"
    );
    assert_eq!(game.religious_tourism_per_turn(0), 8.0);

    game.players[0].counters.remove("great_work:writing");
    assert!(!game.can_house_additional_great_work(0, "writing"));
    assert!(game.can_house_additional_great_work(0, "artifact"));
}

#[test]
fn archaeologists_extract_housed_artifacts_and_consume_sites() {
    let (mut game, city) = game_with_capital(4_122);
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("archaeological_museum"));
    // Occupy the Palace so the Museum's three Artifact slots define the
    // exact excavation capacity.
    game.players[0]
        .counters
        .insert("great_work:relic".to_string(), 1);
    game.players[0]
        .civics
        .insert(crate::name!("natural_history"));
    let sites: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| {
            *position != game.cities[&city].pos && game.map.tiles[position].district.is_none()
        })
        .take(4)
        .collect();
    assert_eq!(sites.len(), 4);
    for position in &sites {
        let tile = game.map.tiles.get_mut(position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = Some(crate::name!("antiquity_site"));
        tile.improvement = None;
        tile.pillaged = false;
    }

    for position in sites.iter().take(3).copied() {
        assert!(game
            .valid_improvements(0, position)
            .contains(&crate::name!("archaeological_dig")));
        let archaeologist = game.spawn_unit("archaeologist", 0, position);
        game.apply(
            0,
            &Action::Improve {
                unit: archaeologist,
                improvement: crate::name!("archaeological_dig"),
            },
        )
        .unwrap();
        assert!(game.map.tiles[&position].resource.is_none());
        assert!(game.map.tiles[&position].improvement.is_none());
    }
    assert_eq!(game.players[0].counters["great_work:artifact"], 3);
    assert_eq!(game.housed_great_works(0)[&city].get("artifact"), Some(&3));
    assert!(!game
        .valid_improvements(0, sites[3])
        .contains(&crate::name!("archaeological_dig")));
    assert_eq!(game.great_work_tourism(0, "artifact"), 3.0);

    let culture_with_artifacts = game.city_yields(city).culture;
    game.players[0]
        .counters
        .insert("great_work:artifact".to_string(), 0);
    let culture_without_artifacts = game.city_yields(city).culture;
    // Three Ancient-era digs share era zero; whether they theme the
    // museum depends on drawing three distinct origin civilizations.
    let artifacts: Vec<&GreatWorkPiece> = game.players[0]
        .great_work_pieces
        .iter()
        .filter(|piece| piece.kind == "artifact")
        .collect();
    let origins: BTreeSet<&str> = artifacts
        .iter()
        .map(|piece| piece.creator.as_str())
        .collect();
    let theming = if artifacts.len() >= 3 && origins.len() >= 3 {
        9.0
    } else {
        0.0
    };
    assert!((culture_with_artifacts - culture_without_artifacts - (9.0 + theming)).abs() < 1e-9);
    game.players[0]
        .counters
        .insert("great_work:artifact".to_string(), 3);

    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(
        restored.housed_great_works(0)[&city].get("artifact"),
        Some(&3)
    );
}

#[test]
fn foreign_excavation_requires_access_unless_terracotta_grants_it() {
    let mut game = Game::new_full(2, 24, 16, 4_1221, 300, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
    }
    let museum_city = game.player_city_ids(0)[0];
    install_test_district(&mut game, museum_city, "theater_square");
    game.cities
        .get_mut(&museum_city)
        .unwrap()
        .buildings
        .push(crate::name!("archaeological_museum"));
    game.players[0]
        .civics
        .insert(crate::name!("natural_history"));
    game.players[1].civics.insert(crate::name!("early_empire"));
    let foreign_city = game.player_city_ids(1)[0];
    let site = game.cities[&foreign_city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&foreign_city].pos)
        .unwrap();
    let tile = game.map.tiles.get_mut(&site).unwrap();
    tile.terrain = crate::name!("plains");
    tile.feature = None;
    tile.resource = Some(crate::name!("antiquity_site"));
    tile.improvement = None;
    tile.district = None;
    tile.wonder = None;
    assert!(game.valid_improvements(0, site).is_empty());

    game.players[1].open_borders_until.insert(0, 30);
    assert_eq!(
        game.valid_improvements(0, site),
        vec!["archaeological_dig".to_string()]
    );
    game.players[1].open_borders_until.clear();
    let wonder_position = game.cities[&museum_city].pos;
    game.cities
        .get_mut(&museum_city)
        .unwrap()
        .wonders
        .insert(crate::name!("terracotta_army"), wonder_position);
    assert_eq!(
        game.valid_improvements(0, site),
        vec!["archaeological_dig".to_string()]
    );
    game.at_war.insert(pair(0, 1));
    assert!(game.valid_improvements(0, site).is_empty());

    game.at_war.clear();
    game.cities
        .get_mut(&museum_city)
        .unwrap()
        .wonders
        .remove(&Name::new("terracotta_army"));
    let neutral = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| {
            game.map.tiles[position].owner_city.is_none()
                && !game.rules.is_water(&game.map.tiles[position])
        })
        .unwrap();
    let tile = game.map.tiles.get_mut(&neutral).unwrap();
    tile.terrain = crate::name!("plains");
    tile.feature = None;
    tile.resource = Some(crate::name!("antiquity_site"));
    tile.improvement = None;
    tile.district = None;
    tile.wonder = None;
    assert_eq!(
        game.valid_improvements(0, neutral),
        vec!["archaeological_dig".to_string()]
    );
}

#[test]
fn pillaged_cultural_buildings_suspend_their_great_work_slots() {
    let (mut game, city) = game_with_capital(4_123);
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("archaeological_museum"));
    game.players[0]
        .counters
        .insert("great_work:relic".to_string(), 1);
    game.players[0]
        .counters
        .insert("great_work:artifact".to_string(), 1);
    assert_eq!(game.housed_great_works(0)[&city].get("artifact"), Some(&1));

    game.cities
        .get_mut(&city)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("archaeological_museum"));
    assert_eq!(game.housed_great_works(0)[&city].get("artifact"), None);
    game.cities
        .get_mut(&city)
        .unwrap()
        .pillaged_buildings
        .remove(&Name::new("archaeological_museum"));
    assert_eq!(game.housed_great_works(0)[&city].get("artifact"), Some(&1));
}
