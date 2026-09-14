use super::*;

fn coal_builder() -> (Game, u32, Pos) {
    let mut game = Game::new_full(1, 24, 16, 91_986, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
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
    tile.terrain = crate::name!("plains");
    tile.hills = true;
    tile.feature = Some(crate::name!("forest"));
    tile.resource = Some(crate::name!("coal"));
    tile.owner_city = Some(city);
    game.players[0].techs.insert(crate::name!("construction"));
    game.players[0].techs.insert(crate::name!("mining"));
    let builder = game.spawn_unit("builder", 0, pos);
    (game, builder, pos)
}

#[test]
fn hidden_coal_allows_the_observed_lumber_mill_to_execute() {
    let (mut game, builder, pos) = coal_builder();
    assert!(!game.resource_visible_to(0, "coal"));
    let view = game.player_decision_view(0);
    assert_eq!(view.map.tiles[&pos].resource, None);
    assert!(view
        .valid_improvements(0, pos)
        .contains(&crate::name!("lumber_mill")));
    assert_eq!(
        game.valid_improvements(0, pos),
        view.valid_improvements(0, pos)
    );
    let charges = game.units[&builder].charges;
    game.apply(
        0,
        &Action::Improve {
            unit: builder,
            improvement: crate::name!("lumber_mill"),
        },
    )
    .unwrap();
    assert_eq!(
        game.map.tiles[&pos].improvement,
        Some(crate::name!("lumber_mill"))
    );
    assert_eq!(game.map.tiles[&pos].resource, Some(crate::name!("coal")));
    assert_eq!(game.units[&builder].charges, charges - 1);
}

#[test]
fn revealed_coal_still_requires_a_compatible_improvement() {
    let (mut game, _, pos) = coal_builder();
    game.players[0]
        .techs
        .insert(crate::name!("industrialization"));
    assert!(game.resource_visible_to(0, "coal"));
    let options = game.valid_improvements(0, pos);
    assert!(options.contains(&crate::name!("mine")));
    assert!(!options.contains(&crate::name!("lumber_mill")));
}

#[test]
fn hidden_coal_does_not_replace_the_lumber_mills_feature_requirement() {
    let (mut game, _, pos) = coal_builder();
    game.map.tiles.get_mut(&pos).unwrap().feature = None;
    assert!(!game
        .valid_improvements(0, pos)
        .contains(&crate::name!("lumber_mill")));
}
