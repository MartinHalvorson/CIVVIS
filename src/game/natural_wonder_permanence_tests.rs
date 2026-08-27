//! A natural wonder is permanent terrain in Civ VI. The generator draws a
//! fixed number of them per map size, and nothing a player does afterwards
//! removes one — so the count a map starts with is the count it ends with.
use super::*;

/// A capital with a natural wonder on a workable neighbour, plus a Builder
/// standing on it. The wonder sits on grassland so that a Farm would be a
/// legal siting were the tile ordinary — that is exactly the case that was
/// erasing wonders mid-game.
fn wonder_beside_capital() -> (Game, u32, Pos) {
    let mut game = Game::new_full(1, 24, 16, 51_207, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    let site = game
        .nbrs(game.cities[&city].pos)
        .into_iter()
        .find(|position| {
            game.map.tiles[position].owner_city == Some(city)
                && !game.rules.is_water(&game.map.tiles[position])
        })
        .expect("the capital works at least one land neighbour");
    {
        let tile = game.map.tiles.get_mut(&site).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.hills = false;
        tile.feature = Some(crate::name!("pantanal"));
        tile.resource = None;
        tile.improvement = None;
    }
    (game, city, site)
}

#[test]
fn no_builder_improvement_may_cover_a_natural_wonder() {
    let (mut game, _city, site) = wonder_beside_capital();
    game.players[0].techs.insert(crate::name!("irrigation"));
    let builder = game.spawn_unit("builder", 0, site);

    // The same tile without its wonder takes a Farm, so the refusal below
    // is about the wonder and not about the terrain or the tech.
    game.map.tiles.get_mut(&site).unwrap().feature = None;
    assert!(game
        .valid_improvements(0, site)
        .contains(&crate::name!("farm")));

    game.map.tiles.get_mut(&site).unwrap().feature = Some(crate::name!("pantanal"));
    assert_eq!(
        game.valid_improvements(0, site),
        Vec::<String>::new(),
        "a natural wonder tile offers a Builder nothing"
    );
    assert!(game
        .apply(
            0,
            &Action::Improve {
                unit: builder,
                improvement: crate::name!("farm"),
            },
        )
        .is_err());
    assert_eq!(
        game.map.tiles[&site].feature.as_deref(),
        Some("pantanal"),
        "the wonder is still standing"
    );
}

#[test]
fn a_national_park_still_encloses_a_natural_wonder_without_clearing_it() {
    // The park fixture already builds four owned, park-legal tiles.
    let (mut game, city, positions) = super::national_park_tests::controlled_park_game();
    let wonder_tile = positions[1];
    game.map.tiles.get_mut(&wonder_tile).unwrap().feature = Some(crate::name!("crater_lake"));
    assert!(
        game.valid_national_park_site(0, &positions),
        "Civ VI parks are the one improvement allowed over a natural wonder"
    );
    assert!(game
        .valid_improvements(0, positions[0])
        .contains(&crate::name!("national_park")));

    let naturalist = game.spawn_unit("naturalist", 0, positions[0]);
    assert!(game
        .apply(
            0,
            &Action::Improve {
                unit: naturalist,
                improvement: crate::name!("national_park"),
            },
        )
        .is_ok());
    assert_eq!(
        game.map.tiles[&wonder_tile].improvement.as_deref(),
        Some("national_park")
    );
    assert_eq!(
        game.map.tiles[&wonder_tile].feature.as_deref(),
        Some("crater_lake"),
        "the park leaves the wonder in place"
    );
    assert!(game.cities.contains_key(&city));
}

#[test]
fn no_city_may_be_founded_on_a_natural_wonder_or_oasis() {
    let (mut game, _city, site) = wonder_beside_capital();
    // Far enough from the capital that only the wonder can refuse it.
    let distant = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| {
            game.wdist(*position, game.cities.values().next().unwrap().pos) >= 5
                && !game.rules.is_water(&game.map.tiles[position])
                && game.rules.is_passable(&game.map.tiles[position])
        })
        .expect("the map has room for a second city");
    {
        let tile = game.map.tiles.get_mut(&distant).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    let settler = game.spawn_unit("settler", 0, distant);
    assert!(game.can_found_city(settler), "an ordinary site is legal");

    game.map.tiles.get_mut(&distant).unwrap().feature = Some(crate::name!("uluru"));
    assert!(!game.can_found_city(settler));
    assert!(game.apply(0, &Action::FoundCity { unit: settler }).is_err());
    assert_eq!(game.map.tiles[&distant].feature.as_deref(), Some("uluru"));

    game.map.tiles.get_mut(&distant).unwrap().feature = Some(crate::name!("oasis"));
    assert!(!game.can_found_city(settler));
    assert!(game.apply(0, &Action::FoundCity { unit: settler }).is_err());
    assert_eq!(game.map.tiles[&distant].feature.as_deref(), Some("oasis"));
    assert_eq!(game.map.tiles[&site].feature.as_deref(), Some("pantanal"));
}

/// Civilization VI refuses a district on an Oasis and a Builder cannot
/// clear one. CIVVIS knew this for city founding (above) and not for
/// district siting, so run civvis-20260811T230324Z asked the host for a
/// Campus on one oasis tile 40 times and was refused every time — the
/// re-ask loop #1577 bounded but could not close, because the belief that
/// the plot was legal came from here.
#[test]
fn no_district_may_be_sited_on_an_oasis() {
    let (mut game, city, site) = wonder_beside_capital();
    {
        let tile = game.map.tiles.get_mut(&site).unwrap();
        tile.terrain = crate::name!("desert");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.flooded = false;
    }
    game.cities.get_mut(&city).unwrap().pop = 10;
    assert!(
        game.district_sites(city, crate::name!("campus"))
            .contains(&site),
        "a bare desert tile is a legal Campus site"
    );
    game.map.tiles.get_mut(&site).unwrap().feature = Some(crate::name!("oasis"));
    assert!(
        !game
            .district_sites(city, crate::name!("campus"))
            .contains(&site),
        "and the same tile with an Oasis is not"
    );
}

#[test]
fn oasis_keeps_its_yields_appeal_fresh_water_and_permanence() {
    let (mut game, city, site) = wonder_beside_capital();
    let center = game.cities[&city].pos;
    game.map.tiles.get_mut(&center).unwrap().river_edges = [false; 6];
    for neighbor in game.nbrs(center) {
        let tile = game.map.tiles.get_mut(&neighbor).unwrap();
        tile.terrain = crate::name!("plains");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
    }
    game.map.tiles.get_mut(&site).unwrap().terrain = crate::name!("desert");

    let bare_appeal = game.tile_appeal(center);
    let dry_housing = game.city_housing(&game.cities[&city]);
    game.map.tiles.get_mut(&site).unwrap().feature = Some(crate::name!("oasis"));

    let yields = game.rules.tile_yields(&game.map.tiles[&site]);
    assert_eq!(yields.food, 3.0);
    assert_eq!(yields.gold, 1.0);
    assert_eq!(yields.total(), 4.0);
    assert_eq!(game.tile_appeal(center), bare_appeal + 1);
    assert_eq!(game.city_housing(&game.cities[&city]), dry_housing + 3.0);
    assert!(game.valid_improvements(0, site).is_empty());
    assert!(game.builder_operations(0, site).is_empty());
}
