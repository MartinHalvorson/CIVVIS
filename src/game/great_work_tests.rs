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

fn game_with_work_collection(seed: u64) -> (Game, u32) {
    let (mut game, city) = game_with_capital(seed);
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("archaeological_museum"));
    game.grant_great_work(0, "artifact", 1, "first dig");
    game.grant_great_work(0, "relic", 0, "shrine");
    game.grant_great_work(0, "artifact", 2, "second dig");
    (game, city)
}

#[test]
fn empty_great_work_snapshots_do_not_hide_the_first_work() {
    let (mut game, city) = game_with_capital(4_126);
    let (empty_counts, empty_pieces) = {
        let _memo = game.query_memo();
        (game.housed_great_works(0), game.housed_great_work_pieces(0))
    };
    assert!(empty_counts.is_empty());
    assert!(empty_pieces.is_empty());
    game.grant_great_work(0, "relic", 0, "first work");
    let _memo = game.query_memo();
    assert_eq!(game.housed_great_works(0)[&city].get("relic"), Some(&1));
    assert_eq!(game.housed_great_work_pieces(0)[&city].len(), 1);
    assert!(empty_counts.is_empty());
    assert!(empty_pieces.is_empty());
}

#[test]
fn great_work_queries_share_ordered_allocations_within_one_scope() {
    let (game, city) = game_with_work_collection(4_124);
    let _memo = game.query_memo();
    let counts = game.housed_great_works(0);
    let pieces = game.housed_great_work_pieces(0);
    assert_eq!(counts[&city].get("artifact"), Some(&2));
    assert_eq!(*counts, game.housed_great_works_uncached(0));
    assert_eq!(*pieces, game.housed_great_work_pieces_uncached(0));
    assert_eq!(
        pieces[&city]
            .iter()
            .map(|piece| (piece.creator.as_str(), piece.era))
            .collect::<Vec<_>>(),
        vec![("first dig", 1), ("second dig", 2), ("shrine", 0)],
        "city and kind ordering must preserve creation order within each kind"
    );
    {
        let _nested = game.query_memo();
        assert!(Arc::ptr_eq(&counts, &game.housed_great_works(0)));
        assert!(Arc::ptr_eq(&pieces, &game.housed_great_work_pieces(0)));
    }
    assert!(Arc::ptr_eq(&counts, &game.housed_great_works(0)));
    assert!(Arc::ptr_eq(&pieces, &game.housed_great_work_pieces(0)));
}

#[test]
fn retained_great_work_snapshots_do_not_hide_pillage_or_repair() {
    let (mut game, city) = game_with_work_collection(4_125);
    let (old_counts, old_pieces) = {
        let _memo = game.query_memo();
        (game.housed_great_works(0), game.housed_great_work_pieces(0))
    };
    assert!(game.query_memo.housed_works.borrow().is_none());
    assert!(game.query_memo.housed_pieces.borrow().is_none());
    game.cities
        .get_mut(&city)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("archaeological_museum"));
    assert_eq!(game.housed_great_works(0)[&city].get("artifact"), None);
    assert_eq!(game.housed_great_work_pieces(0)[&city].len(), 1);
    assert!(game.query_memo.housed_works.borrow().is_none());
    assert!(game.query_memo.housed_pieces.borrow().is_none());
    assert_eq!(old_counts[&city].get("artifact"), Some(&2));
    assert_eq!(old_pieces[&city].len(), 3);

    game.cities
        .get_mut(&city)
        .unwrap()
        .pillaged_buildings
        .remove(&crate::name!("archaeological_museum"));
    let _memo = game.query_memo();
    let repaired_counts = game.housed_great_works(0);
    let repaired_pieces = game.housed_great_work_pieces(0);
    assert_eq!(repaired_counts, old_counts);
    assert_eq!(repaired_pieces, old_pieces);
    assert!(!Arc::ptr_eq(&repaired_counts, &old_counts));
    assert!(!Arc::ptr_eq(&repaired_pieces, &old_pieces));
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

    // Natural History can reveal a site beneath an existing improvement.
    // Digging extracts the artifact without replacing or repairing that tile.
    game.map.tiles.get_mut(&sites[0]).unwrap().improvement = Some(crate::name!("mine"));
    game.map.tiles.get_mut(&sites[1]).unwrap().improvement = Some(crate::name!("farm"));
    game.map.tiles.get_mut(&sites[1]).unwrap().pillaged = true;
    for position in sites.iter().take(3).copied() {
        let improvement_before = game.map.tiles[&position].improvement;
        let pillaged_before = game.map.tiles[&position].pillaged;
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
        assert_eq!(game.map.tiles[&position].improvement, improvement_before);
        assert_eq!(game.map.tiles[&position].pillaged, pillaged_before);
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
