use super::*;

fn farm_builder() -> (Game, u32, u32, Pos) {
    let mut game = Game::new_full(1, 24, 16, 914_3578, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|id| game.units[id].kind == "settler")
        .unwrap();
    let center = game.units[&settler].pos;
    let city = game.found_city_for(0, center, None);
    game.remove_unit(settler);
    let pos = *game.cities[&city]
        .owned_tiles
        .iter()
        .find(|pos| **pos != center)
        .unwrap();
    let tile = game.map.tiles.get_mut(&pos).unwrap();
    *tile = Tile::new(pos);
    tile.terrain = crate::name!("grassland");
    tile.owner_city = Some(city);
    let builder = game.spawn_unit("builder", 0, pos);
    (game, city, builder, pos)
}

#[test]
fn a_stale_city_handle_neither_panics_nor_spends_a_builder_charge() {
    let (mut game, city, builder, pos) = farm_builder();
    assert!(game
        .valid_improvements(0, pos)
        .contains(&crate::name!("farm")));
    game.cities.remove(&city);
    assert_eq!(game.map.tiles[&pos].owner_city, Some(city));
    assert!(game.valid_improvements(0, pos).is_empty());
    let charges = game.units[&builder].charges;
    assert!(game
        .apply(
            0,
            &Action::Improve {
                unit: builder,
                improvement: crate::name!("farm"),
            },
        )
        .is_err());
    assert_eq!(game.units[&builder].charges, charges);
    assert!(game.map.tiles[&pos].improvement.is_none());
}

#[test]
fn a_present_owner_still_allows_the_real_farm_action() {
    let (mut game, _, builder, pos) = farm_builder();
    let charges = game.units[&builder].charges;
    game.apply(
        0,
        &Action::Improve {
            unit: builder,
            improvement: crate::name!("farm"),
        },
    )
    .unwrap();
    assert_eq!(game.units[&builder].charges, charges - 1);
    assert_eq!(game.map.tiles[&pos].improvement, Some(crate::name!("farm")));
}
