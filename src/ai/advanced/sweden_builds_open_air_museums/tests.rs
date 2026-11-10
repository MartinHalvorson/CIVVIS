//! `sweden-builds-open-air-museums`: the museum is the Builder's next job.
use super::*;

/// One founded Swedish capital on Plains that knows Nationalism; returns the
/// game and a bare, flat, first-ring Plains plot of it.
fn sweden_after_nationalism(seed: u64) -> (Game, Pos) {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("SWEDENMUSEUM", seed),
        250,
        0,
        false,
    );
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    game.players[0].civ = "Sweden".to_string();
    game.players[0].civics.insert(crate::name!("nationalism"));
    let cid = game.player_city_ids(0)[0];
    let center = game.cities[&cid].pos;
    game.map.tiles.get_mut(&center).unwrap().terrain = crate::name!("plains");
    let site = game.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != center && game.wdist(center, *pos) == 1)
        .expect("a first-ring plot");
    let tile = game.map.tiles.get_mut(&site).unwrap();
    tile.terrain = crate::name!("plains");
    tile.hills = false;
    tile.feature = None;
    tile.resource = None;
    tile.improvement = None;
    (game, site)
}

#[test]
fn sweden_prices_the_museum_above_any_ordinary_tile() {
    let (game, site) = sweden_after_nationalism(41_101);
    assert!(
        game.valid_improvements(0, site)
            .contains(&crate::name!("open_air_museum")),
        "the engine lets Sweden build it here"
    );
    let off = AdvancedAi::new();
    let mut on = AdvancedAi::new();
    on.enable_sweden_builds_open_air_museums();
    assert_eq!(
        off.open_air_museum_value(&game, site, "open_air_museum", GrandStrategy::Culture),
        0.0,
        "off, nothing changes"
    );
    assert_eq!(
        on.open_air_museum_value(&game, site, "farm", GrandStrategy::Culture),
        0.0,
        "only the museum"
    );
    assert!(
        !off.worthwhile_improvements(&game, 0, site, GrandStrategy::Culture)
            .first()
            .is_some_and(|best| best == "open_air_museum"),
        "off, the museum is not the plot's best job"
    );
    for strategy in [
        GrandStrategy::Culture,
        GrandStrategy::Science,
        GrandStrategy::Recovery,
    ] {
        let best = on.worthwhile_improvements(&game, 0, site, strategy);
        assert_eq!(
            best.first().map(|name| name.as_str()),
            Some("open_air_museum"),
            "on, the museum is the plot's best job under {strategy:?}: {best:?}"
        );
    }
}

#[test]
fn the_museum_scales_with_the_terrain_classes_of_swedens_cities() {
    let (mut game, site) = sweden_after_nationalism(41_102);
    let mut on = AdvancedAi::new();
    on.enable_sweden_builds_open_air_museums();
    assert_eq!(AdvancedAi::open_air_museum_terrain_classes(&game, 0), 1);
    let one = on.open_air_museum_value(&game, site, "open_air_museum", GrandStrategy::Culture);
    // A second city centre on Tundra Hills adds the Tundra class; a third on
    // Plains adds nothing.
    for (terrain, hills) in [("tundra", true), ("plains", false)] {
        let pos = *game
            .map
            .tiles
            .keys()
            .filter(|pos| {
                !game.rules.is_water(&game.map.tiles[pos])
                    && game
                        .cities
                        .values()
                        .all(|city| game.wdist(city.pos, **pos) >= 4)
            })
            .min()
            .expect("room for another city");
        let tile = game.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = Name::new(terrain);
        tile.hills = hills;
        game.place_city(0, pos, None);
    }
    assert_eq!(game.player_city_ids(0).len(), 3);
    assert_eq!(AdvancedAi::open_air_museum_terrain_classes(&game, 0), 2);
    let two = on.open_air_museum_value(&game, site, "open_air_museum", GrandStrategy::Culture);
    // 2 Culture at the Culture lane's 4.2 plus 2 Tourism at 35, per class.
    assert!(
        (two - one - (2.0 * 4.2 + 2.0 * 35.0)).abs() < 1e-9,
        "{one} -> {two}"
    );
}

#[test]
fn another_civilization_and_foreign_ground_pay_nothing() {
    let (mut game, site) = sweden_after_nationalism(41_103);
    let mut on = AdvancedAi::new();
    on.enable_sweden_builds_open_air_museums();
    assert!(on.open_air_museum_value(&game, site, "open_air_museum", GrandStrategy::Culture) > 0.0);
    game.players[0].civ = "Rome".to_string();
    assert_eq!(
        on.open_air_museum_value(&game, site, "open_air_museum", GrandStrategy::Culture),
        0.0
    );
    let (mut unowned, site) = sweden_after_nationalism(41_104);
    unowned.map.tiles.get_mut(&site).unwrap().owner_city = None;
    assert_eq!(
        on.open_air_museum_value(&unowned, site, "open_air_museum", GrandStrategy::Culture),
        0.0
    );
}
