use super::*;

fn resource_city(resource: &str) -> (Game, u32, Pos, Item) {
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
    tile.terrain = crate::name!("desert");
    tile.resource = Some(Name::new(resource));
    tile.owner_city = Some(city);
    game.players[0].techs.insert(crate::name!("mathematics"));
    game.current = 0;
    let item = Item::Wonder {
        wonder: crate::name!("petra"),
        pos,
    };
    (game, city, pos, item)
}

#[test]
fn an_undiscovered_deposit_does_not_refuse_the_observed_wonder_order() {
    for resource in ["coal", "oil", "niter", "aluminum"] {
        let (mut game, city, pos, item) = resource_city(resource);
        assert!(!game.resource_visible_to(0, resource));
        let view = game.player_decision_view(0);
        assert_eq!(view.map.tiles[&pos].resource, None);
        assert!(view.can_produce(0, city, &item));
        assert!(game.can_produce(0, city, &item), "hidden {resource}");
        game.apply(0, &Action::Produce { city, item }).unwrap();
        assert_eq!(game.map.tiles[&pos].resource, Some(Name::new(resource)));
    }
}

#[test]
fn a_revealed_deposit_still_blocks_a_new_wonder() {
    let (mut game, city, pos, item) = resource_city("aluminum");
    game.players[0].techs.insert(crate::name!("radio"));
    assert!(game.resource_visible_to(0, "aluminum"));
    assert!(!game.wonder_sites(city, "petra").contains(&pos));
    assert!(!game.can_produce(0, city, &item));
    assert!(game.apply(0, &Action::Produce { city, item }).is_err());
}

#[test]
fn discovery_does_not_cancel_a_placed_wonder_or_destroy_its_deposit() {
    for paused in [false, true] {
        let (mut game, city, pos, item) = resource_city("aluminum");
        game.apply(
            0,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .unwrap();
        if paused {
            game.cities.get_mut(&city).unwrap().production = 10.0;
            game.apply(
                0,
                &Action::Produce {
                    city,
                    item: Item::Unit {
                        unit: crate::name!("builder"),
                    },
                },
            )
            .unwrap();
        }
        game.players[0].techs.insert(crate::name!("radio"));
        assert!(game.resource_visible_to(0, "aluminum"));
        assert!(game.can_produce(0, city, &item));
        game.apply(
            0,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .unwrap();
        assert!(game.complete_item(0, city, &item));
        assert_eq!(game.map.tiles[&pos].wonder, Some(crate::name!("petra")));
        assert_eq!(
            game.map.tiles[&pos].resource,
            Some(crate::name!("aluminum"))
        );
    }
}

#[test]
fn a_visible_luxury_still_blocks_a_new_wonder() {
    let (game, city, pos, item) = resource_city("silk");
    assert!(game.resource_visible_to(0, "silk"));
    assert!(!game.wonder_sites(city, "petra").contains(&pos));
    assert!(!game.can_produce(0, city, &item));
}
