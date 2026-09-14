use super::*;

fn resource_city(resource: &str) -> (Game, u32, Pos) {
    let mut game = Game::new_full(1, 24, 16, 91_986, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let center = game.units[&settler].pos;
    let city = game.found_city_for(0, center, None);
    game.remove_unit(settler);
    game.cities.get_mut(&city).unwrap().pop = 3;
    let pos = *game.cities[&city]
        .owned_tiles
        .iter()
        .find(|pos| **pos != center)
        .unwrap();
    let tile = game.map.tiles.get_mut(&pos).unwrap();
    *tile = Tile::new(pos);
    tile.terrain = crate::name!("plains");
    tile.resource = Some(Name::new(resource));
    tile.owner_city = Some(city);
    game.players[0].techs.insert(crate::name!("writing"));
    (game, city, pos)
}

#[test]
fn hidden_deposits_allow_the_observed_district_order() {
    for resource in ["coal", "oil", "niter"] {
        let (mut game, city, pos) = resource_city(resource);
        assert!(!game.resource_visible_to(0, resource));
        let view = game.player_decision_view(0);
        assert_eq!(view.map.tiles[&pos].resource, None);
        let item = Item::District {
            district: crate::name!("campus"),
            pos,
        };
        assert!(view.can_produce(0, city, &item));
        assert!(game.can_produce(0, city, &item), "hidden {resource}");
        game.apply(0, &Action::Produce { city, item }).unwrap();
        assert_eq!(
            game.map.tiles[&pos]
                .district_foundation
                .as_ref()
                .unwrap()
                .district,
            crate::name!("campus")
        );
        assert_eq!(game.map.tiles[&pos].resource, Some(Name::new(resource)));
    }
}

#[test]
fn a_hidden_deposit_survives_district_completion_after_discovery() {
    let (mut game, city, pos) = resource_city("coal");
    let item = Item::District {
        district: crate::name!("campus"),
        pos,
    };
    game.apply(
        0,
        &Action::Produce {
            city,
            item: item.clone(),
        },
    )
    .unwrap();
    game.players[0]
        .techs
        .insert(crate::name!("industrialization"));
    assert!(game.resource_visible_to(0, "coal"));
    assert!(game.complete_item(0, city, &item));
    assert_eq!(game.map.tiles[&pos].district, Some(crate::name!("campus")));
    assert_eq!(game.map.tiles[&pos].resource, Some(crate::name!("coal")));
}

#[test]
fn a_revealed_strategic_deposit_still_blocks_a_new_district() {
    let (mut game, city, pos) = resource_city("coal");
    game.players[0]
        .techs
        .insert(crate::name!("industrialization"));
    assert!(game.resource_visible_to(0, "coal"));
    assert!(!game
        .district_sites(city, crate::name!("campus"))
        .contains(&pos));
}

#[test]
fn a_visible_bonus_resource_still_requires_its_removal_technology() {
    let (mut game, city, pos) = resource_city("bananas");
    assert!(game.resource_visible_to(0, "bananas"));
    let technology = game.rules.improvements["plantation"].tech.unwrap();
    assert!(!game.players[0].techs.contains(&technology));
    assert!(!game
        .district_sites(city, crate::name!("campus"))
        .contains(&pos));
    game.players[0].techs.insert(technology);
    assert!(game
        .district_sites(city, crate::name!("campus"))
        .contains(&pos));
}
