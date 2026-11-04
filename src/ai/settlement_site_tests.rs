use super::*;

#[test]
fn settlement_requires_known_friendly_ownership_or_unclaimed_land() {
    let mut game = Game::new_full(2, 24, 18, 91_773, 120, 0, false);
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.owner_city = None;
    }
    let own = game.found_city_for(0, (3, 3), None);
    let enemy = game.found_city_for(1, (3, 12), None);
    let site = (12, 8);
    let ai = BasicAi::new();
    game.map.tiles.get_mut(&site).unwrap().owner_city = None;
    assert!(ai.valid_settle_site(&game, 0, site));
    game.map.tiles.get_mut(&site).unwrap().owner_city = Some(own);
    assert!(ai.valid_settle_site(&game, 0, site));
    game.map.tiles.get_mut(&site).unwrap().owner_city = Some(enemy);
    assert!(!ai.valid_settle_site(&game, 0, site));
    game.cities.remove(&enemy);
    assert!(game.map.tiles[&site].owner_city.is_some());
    assert!(!ai.valid_settle_site(&game, 0, site));
}

fn one_usable_site(site: Pos) -> (Game, BasicAi, u32) {
    let mut game = Game::new_full(2, 24, 18, 91_774, 120, 0, false);
    game.map.topology = crate::world::Topology::Rectangle;
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
        tile.owner_city = None;
    }
    let capital = game.found_city_for(0, (3, 3), None);
    let enemy = game.found_city_for(1, (15, 12), None);
    for tile in game.map.tiles.values_mut() {
        tile.owner_city = Some(enemy);
    }
    for position in game.wdisk((3, 3), 3) {
        game.map.tiles.get_mut(&position).unwrap().owner_city = Some(capital);
    }
    game.map.tiles.get_mut(&site).unwrap().owner_city = None;
    game.host_observed = std::sync::Arc::new(game.map.tiles.keys().copied().collect());
    let mut ai = BasicAi::new();
    ai.w.min_city_dist = 4.0;
    let usable: Vec<_> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|position| ai.valid_settle_site(&game, 0, *position))
        .collect();
    assert_eq!(usable, vec![site], "the fixture has only one usable site");
    (game, ai, capital)
}

#[test]
fn practical_site_search_keeps_a_short_route_after_a_long_first_visit() {
    let (game, ai, _) = one_usable_site((9, 3));
    let route = [(3, 3), (4, 3), (5, 3), (6, 3), (7, 3), (8, 3), (9, 3)];
    for step in route.windows(2) {
        assert!(game.nbrs(step[0]).contains(&step[1]));
        assert!(game.rules.is_passable(&game.map.tiles[&step[1]]));
        assert!(!game.rules.is_water(&game.map.tiles[&step[1]]));
        assert!(game.city_at(step[1]).is_none());
    }
    assert_eq!(route.len() - 1, 6);
    assert!(
        ai.has_practical_settle_site(&game, 0),
        "a six-step route remains within the eight-step settlement budget"
    );
}

#[test]
fn practical_site_search_keeps_the_short_route_from_each_city() {
    let (mut game, ai, _) = one_usable_site((9, 3));
    game.found_city_for(0, (3, 12), None);
    assert!(ai.valid_settle_site(&game, 0, (9, 3)));
    assert!(
        ai.has_practical_settle_site(&game, 0),
        "searching the second city's approaches cannot hide the capital's nearby site"
    );
}

#[test]
fn practical_site_search_keeps_the_walking_distance_limit() {
    let (mut game, ai, _) = one_usable_site((3, 12));
    assert_eq!(game.wdist((3, 3), (3, 12)), 9);
    assert!(!ai.has_practical_settle_site(&game, 0));
    game.players[0].techs.insert(crate::name!("shipbuilding"));
    assert!(ai.has_practical_settle_site(&game, 0));
}

#[test]
fn practical_site_search_still_respects_impassable_approaches() {
    let (mut game, ai, _) = one_usable_site((9, 3));
    for position in game.nbrs((3, 3)) {
        game.map.tiles.get_mut(&position).unwrap().terrain = crate::name!("mountain");
    }
    assert!(ai.valid_settle_site(&game, 0, (9, 3)));
    assert!(!ai.has_practical_settle_site(&game, 0));
}

#[test]
fn a_paid_settler_keeps_its_queue_when_a_short_route_still_exists() {
    let (mut game, mut ai, capital) = one_usable_site((9, 3));
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    game.current = 0;
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    let city = game.cities.get_mut(&capital).unwrap();
    city.pop = 3;
    city.queue = vec![settler.clone()];
    city.production = 17.0;
    ai.book_pos = 4;
    ai.w.city_target = 7.0;
    ai.cities(&mut game, 0);
    assert_eq!(game.cities[&capital].queue, vec![settler]);
    assert_eq!(game.cities[&capital].production, 17.0);
}

#[test]
fn frozen_headless_search_keeps_its_existing_route_order() {
    let (mut game, ai, _) = one_usable_site((9, 3));
    game.host_observed = std::sync::Arc::new(Default::default());
    assert!(ai.valid_settle_site(&game, 0, (9, 3)));
    assert!(
        !ai.has_practical_settle_site(&game, 0),
        "headless reference controllers keep their recorded route traversal"
    );
}
