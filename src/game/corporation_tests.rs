use super::*;

fn industry_game() -> (Game, u32, Vec<Pos>) {
    let mut game = Game::new_full(2, 24, 16, 91_700, 200, 0, false);
    for tile in game.map.tiles.values_mut() {
        tile.resource = None;
        tile.improvement = None;
        tile.pillaged = false;
    }
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let center = game.units[&settler].pos;
    game.found_city_for(0, center, None);
    let city = game.city_at(center).unwrap();
    let positions: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != center)
        .take(3)
        .collect();
    assert_eq!(positions.len(), 3);
    for position in &positions {
        let tile = game.map.tiles.get_mut(position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = Some(crate::name!("silk"));
        tile.improvement = Some(crate::name!("plantation"));
    }
    game.players[0].techs.insert(crate::name!("currency"));
    (game, city, positions)
}

#[test]
fn industry_corporation_and_products_form_one_playable_save_stable_chain() {
    let (mut game, city, positions) = industry_game();
    install_test_district(&mut game, city, "commercial_hub");
    install_test_district(&mut game, city, "harbor");
    let builder = game.spawn_unit("builder", 0, positions[0]);
    assert_eq!(game.controlled_resource_count(0, "silk"), 3);
    game.map.tiles.get_mut(&positions[2]).unwrap().improvement = None;
    assert_eq!(game.controlled_resource_count(0, "silk"), 2);
    assert!(game
        .valid_improvements(0, positions[0])
        .contains(&crate::name!("industry")));
    assert!(
        !game
            .valid_improvements(0, positions[2])
            .contains(&crate::name!("industry")),
        "the Industry must replace an existing resource improvement"
    );
    game.players[0].era_score = 0;
    game.apply(
        0,
        &Action::Improve {
            unit: builder,
            improvement: crate::name!("industry"),
        },
    )
    .unwrap();
    assert_eq!(
        game.map.tiles[&positions[0]].improvement.as_deref(),
        Some("industry")
    );
    assert_eq!(
        game.players[0].era_score, 6,
        "the world's first Industry and first luxury Monopoly are +3 each"
    );
    for moment in [
        "MOMENT_FIRST_INDUSTRY_IN_WORLD",
        "MOMENT_FIRST_LUXURY_RESOURCE_MONOPOLY_IN_WORLD",
    ] {
        assert_eq!(
            game.players[0]
                .counters
                .get(&format!("historic_moment_awards:{moment}")),
            Some(&1)
        );
    }
    assert!(!game
        .valid_improvements(0, positions[1])
        .contains(&crate::name!("industry")));

    let merchant_before = game.players[0].gpp.get("merchant").copied().unwrap_or(0.0);
    game.process_great_people(0);
    assert_eq!(
        game.players[0].gpp["merchant"],
        merchant_before + 2.0,
        "the active Commercial Hub and the Industry each grant one Merchant point"
    );

    game.players[0].techs.insert(crate::name!("economics"));
    let merchant_cost = game.gp_cost(0, "merchant");
    game.players[0]
        .gpp
        .insert("merchant".to_string(), merchant_cost);
    assert!(
        !game.can_found_corporation(0, positions[0]),
        "a Corporation always requires three connected copies"
    );
    game.map.tiles.get_mut(&positions[2]).unwrap().improvement = Some(crate::name!("plantation"));
    assert!(game.can_found_corporation(0, positions[0]));
    let gold_before = game.players[0].gold;
    let score_before = game.players[0].era_score;
    game.apply(0, &Action::FoundCorporation { pos: positions[0] })
        .unwrap();
    assert_eq!(
        game.players[0].gold, gold_before,
        "the retired Merchant's named effect is forgone"
    );
    assert_eq!(
        game.players[0].era_score - score_before,
        4,
        "the world's first Corporation is +3 and recruiting its Merchant is +1"
    );
    assert_eq!(
        game.players[0]
            .counters
            .get("historic_moment_awards:MOMENT_FIRST_CORPORATION_IN_WORLD"),
        Some(&1)
    );
    assert_eq!(
        game.map.tiles[&positions[0]].improvement.as_deref(),
        Some("corporation")
    );

    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("stock_exchange"));
    assert_eq!(game.product_capacity(&game.cities[&city]), 3);
    let product = Item::Product {
        product: crate::name!("silk"),
    };
    assert!(game.can_produce(0, city, &product));
    game.apply(
        0,
        &Action::Produce {
            city,
            item: product.clone(),
        },
    )
    .unwrap();
    game.cities.get_mut(&city).unwrap().production = 500.0;
    let culture_before = game.city_yields(city).culture;
    game.process_city(0, city);
    assert_eq!(game.cities[&city].products, vec!["silk"]);
    assert!(game.city_yields(city).culture > culture_before);
    assert!(game.tourism_per_turn(0) >= 1.0);

    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("seaport"));
    for _ in 1..5 {
        assert!(game.can_produce(0, city, &product));
        assert!(game.complete_item(0, city, &product));
    }
    assert_eq!(game.cities[&city].products.len(), 5);
    assert!(!game.can_produce(0, city, &product));

    let encoded = serde_json::to_value(&game).unwrap();
    let restored: Game = serde_json::from_value(encoded).unwrap();
    assert_eq!(restored.cities[&city].products.len(), 5);
    assert_eq!(
        restored.map.tiles[&positions[0]].improvement.as_deref(),
        Some("corporation")
    );
    assert_eq!(restored.product_capacity(&restored.cities[&city]), 6);
}

#[test]
fn products_move_between_exact_slots_and_pillaged_hosts_stop_their_effects() {
    let (mut game, origin, positions) = industry_game();
    install_test_district(&mut game, origin, "commercial_hub");
    game.map.tiles.get_mut(&positions[0]).unwrap().improvement =
        Some(crate::name!("corporation"));
    game.cities
        .get_mut(&origin)
        .unwrap()
        .buildings
        .push(crate::name!("stock_exchange"));
    game.cities
        .get_mut(&origin)
        .unwrap()
        .products
        .push("silk".to_string());

    let second_position = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|position| game.map.tiles[position].owner_city.is_none())
        .filter(|position| {
            game.rules.is_passable(&game.map.tiles[position])
                && !game.rules.is_water(&game.map.tiles[position])
        })
        .max_by_key(|position| game.wdist(game.cities[&origin].pos, *position))
        .unwrap();
    game.found_city_for(0, second_position, Some("Product Host".to_string()));
    let target = game.city_at(second_position).unwrap();
    install_test_district(&mut game, target, "harbor");
    game.cities
        .get_mut(&target)
        .unwrap()
        .buildings
        .push(crate::name!("seaport"));
    game.apply(
        0,
        &Action::MoveProduct {
            from: origin,
            to: target,
            product: crate::name!("silk"),
        },
    )
    .unwrap();
    assert!(game.cities[&origin].products.is_empty());
    assert_eq!(game.cities[&target].products, vec!["silk"]);

    let active = game.city_yields(target).culture;
    game.cities
        .get_mut(&target)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("seaport"));
    assert_eq!(game.product_capacity(&game.cities[&target]), 0);
    assert!(game.city_yields(target).culture < active);
    assert!(
        game.do_move_product(0, target, origin, "silk").is_ok(),
        "an inactive Product remains movable into a valid open slot"
    );
}

#[test]
fn monopoly_thresholds_and_industry_multiplier_match_civilopedia_formula() {
    let mut game = Game::new_full(3, 24, 16, 91_701, 200, 0, false);
    for pid in 0..3 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
    }
    for tile in game.map.tiles.values_mut() {
        tile.resource = None;
        tile.improvement = None;
    }
    let owned: Vec<Pos> = game
        .cities
        .values()
        .filter(|city| city.owner == 0)
        .flat_map(|city| city.owned_tiles.iter().copied())
        .filter(|position| game.city_at(*position).is_none())
        .take(3)
        .collect();
    assert_eq!(owned.len(), 3);
    for position in &owned {
        let tile = game.map.tiles.get_mut(position).unwrap();
        tile.resource = Some(crate::name!("wine"));
        tile.improvement = Some(crate::name!("plantation"));
    }
    let rival_copy = game
        .cities
        .values()
        .find(|city| city.owner == 1)
        .unwrap()
        .owned_tiles
        .iter()
        .copied()
        .find(|position| game.city_at(*position).is_none())
        .unwrap();
    {
        let tile = game.map.tiles.get_mut(&rival_copy).unwrap();
        tile.resource = Some(crate::name!("wine"));
        tile.improvement = Some(crate::name!("plantation"));
    }
    let uncontrolled = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.map.tiles[position].owner_city.is_none())
        .unwrap();
    game.map.tiles.get_mut(&uncontrolled).unwrap().resource = Some(crate::name!("wine"));
    assert_eq!(game.monopoly_bonuses(0), (5.0, 3.0));
    game.map.tiles.get_mut(&owned[0]).unwrap().improvement = Some(crate::name!("industry"));
    assert_eq!(game.monopoly_bonuses(0), (5.0, 9.0));
    game.map.tiles.get_mut(&uncontrolled).unwrap().resource = None;
    assert_eq!(game.monopoly_bonuses(0).0, 10.0, "three of four is 75%");
    game.map.tiles.get_mut(&rival_copy).unwrap().resource = None;
    assert_eq!(game.monopoly_bonuses(0).0, 25.0, "three of three is 100%");
}
