//! A route walks its path for a Gold-multiplying wonder only once one stands.
//! See `Game::route_gold_wonder_stands`.

use super::*;

#[test]
fn the_route_multiplier_follows_the_wonders_on_the_map() {
    let mut game = Game::new_full(2, 24, 16, 91_171, 100, 0, false);
    let land: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .collect();
    let from = land[0];
    let to = *land
        .iter()
        .find(|pos| game.wdist(from, **pos) >= 4)
        .expect("land four tiles away");
    assert!(!game.route_gold_wonder_stands());
    assert_eq!(game.route_district_gold_multiplier(from, to), 1.0);

    // The walk starts where the route does: a Panama Canal there doubles it,
    // and the tile write is what the kept answer notices.
    game.map.tiles.get_mut(&from).unwrap().wonder = Some(crate::name!("panama_canal"));
    assert!(game.route_gold_wonder_stands());
    assert_eq!(game.route_district_gold_multiplier(from, to), 2.0);

    // A wonder that multiplies nothing leaves the walk skipped.
    game.map.tiles.get_mut(&from).unwrap().wonder = Some(crate::name!("oracle"));
    assert!(!game.route_gold_wonder_stands());
    assert_eq!(game.route_district_gold_multiplier(from, to), 1.0);

    // A copy keeps no answer of its own: it reads its own map.
    game.map.tiles.get_mut(&from).unwrap().wonder = Some(crate::name!("golden_gate_bridge"));
    let copy = game.clone();
    assert!(copy.route_gold_wonder_stands());
    assert_eq!(copy.route_district_gold_multiplier(from, to), 2.0);
}

#[test]
fn a_kept_route_walk_is_the_walk_on_the_map_as_it_now_stands() {
    let mut game = Game::new_full(2, 24, 16, 91_171, 100, 0, false);
    let land: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .collect();
    let from = land[0];
    let to = *land
        .iter()
        .find(|pos| game.wdist(from, **pos) >= 4)
        .expect("land four tiles away");
    let walk = game.route_walk(from, to);
    assert_eq!(&walk[..], &game.route_walk_uncached(from, to)[..]);
    assert!(walk.len() >= 2, "the walk takes a few steps: {walk:?}");
    // Asked again on the same map, the kept walk is handed back.
    assert!(std::sync::Arc::ptr_eq(&walk, &game.route_walk(from, to)));

    // A mountain on the first step turns the walk; the kept one is dropped.
    game.map.tiles.get_mut(&walk[0]).unwrap().terrain = crate::name!("mountain");
    let turned = game.route_walk(from, to);
    assert_eq!(&turned[..], &game.route_walk_uncached(from, to)[..]);
    assert_ne!(
        turned.first(),
        walk.first(),
        "the walk steps around the mountain"
    );

    // A copy walks its own map.
    let copy = game.clone();
    assert_eq!(&copy.route_walk(from, to)[..], &turned[..]);
}
