use super::*;

#[test]
fn ordinary_buildings_cost_four_times_production_and_finish_immediately() {
    let mut game = Game::new_full(1, 20, 14, 88_201, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.found_city_for(0, game.units[&settler].pos, None);
    let city = game.player_city_ids(0)[0];
    game.players[0].techs.insert(crate::name!("pottery"));
    game.players[0].gold = 1_000.0;
    let granary = Item::Building {
        building: crate::name!("granary"),
    };
    game.cities.get_mut(&city).unwrap().queue = vec![granary.clone()];
    game.cities.get_mut(&city).unwrap().production = 20.0;

    assert_eq!(
        game.building_gold_purchase_cost(0, city, "granary"),
        Some(260.0)
    );
    let purchase = Action::BuyBuilding {
        city,
        building: crate::name!("granary"),
        currency: "gold".to_string(),
    };
    assert_eq!(
        game.legal_actions(0)
            .iter()
            .filter(|action| **action == purchase)
            .count(),
        1,
        "the purchase action should not be duplicated"
    );
    game.apply(0, &purchase).unwrap();

    assert_eq!(game.players[0].gold, 740.0);
    assert!(game.cities[&city]
        .buildings
        .contains(&crate::name!("granary")));
    assert!(game.cities[&city].queue.is_empty());
    assert_eq!(game.cities[&city].production, 0.0);
}

#[test]
fn defenses_and_government_plaza_buildings_remain_production_only() {
    let mut game = Game::new_full(1, 20, 14, 88_202, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.found_city_for(0, game.units[&settler].pos, None);
    let city = game.player_city_ids(0)[0];
    game.players[0].techs.insert(crate::name!("masonry"));
    game.players[0].gold = 10_000.0;
    assert!(game.can_produce(
        0,
        city,
        &Item::Building {
            building: crate::name!("walls"),
        }
    ));
    assert_eq!(game.building_gold_purchase_cost(0, city, "walls"), None);
    assert!(game
        .apply(
            0,
            &Action::BuyBuilding {
                city,
                building: crate::name!("walls"),
                currency: "gold".to_string(),
            },
        )
        .is_err());

    let plaza = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    game.map.tiles.get_mut(&plaza).unwrap().district = Some(crate::name!("government_plaza"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("government_plaza"), plaza);
    assert!(game.can_produce(
        0,
        city,
        &Item::Building {
            building: crate::name!("ancestral_hall"),
        }
    ));
    assert_eq!(
        game.building_gold_purchase_cost(0, city, "ancestral_hall"),
        None
    );
}

#[test]
fn pillage_and_enemy_occupation_block_district_buildings_and_repairs() {
    let mut game = Game::new_full(2, 20, 14, 88_203, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.found_city_for(0, game.units[&settler].pos, None);
    let city = game.player_city_ids(0)[0];
    let campus = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    game.map.tiles.get_mut(&campus).unwrap().district = Some(crate::name!("campus"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), campus);
    game.players[0].techs.insert(crate::name!("writing"));
    let library = Item::Building {
        building: crate::name!("library"),
    };
    assert!(game.can_produce(0, city, &library));

    game.map.tiles.get_mut(&campus).unwrap().pillaged = true;
    assert!(!game.can_produce(0, city, &library));
    assert_eq!(game.building_gold_purchase_cost(0, city, "library"), None);

    game.at_war.insert(pair(0, 1));
    game.spawn_test_unit("warrior", 1, campus);
    assert!(!game.can_produce(
        0,
        city,
        &Item::Repair {
            repair: crate::name!("district"),
            pos: campus,
        }
    ));
    game.map.tiles.get_mut(&campus).unwrap().pillaged = false;
    assert!(!game.can_produce(0, city, &library));
}
