use super::*;

pub(super) fn controlled_park_game() -> (Game, u32, [Pos; 4]) {
    let mut game = Game::new_full(1, 24, 16, 91_741, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    let center = game.cities[&city].pos;
    let positions = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|top| game.wdist(center, *top) > 4)
        .find_map(|top| game.national_park_diamond(top))
        .expect("map has room for a four-tile park away from the capital");

    let nearby: BTreeSet<Pos> = positions
        .iter()
        .flat_map(|position| game.nbrs(*position))
        .chain(positions)
        .collect();
    for position in nearby {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.pillaged = false;
        tile.district = None;
        tile.wonder = None;
        tile.flooded = false;
        tile.submerged = false;
    }
    for position in positions {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("mountain");
        tile.owner_city = Some(city);
        if !game.cities[&city].owned_tiles.contains(&position) {
            game.cities
                .get_mut(&city)
                .unwrap()
                .owned_tiles
                .push(position);
        }
    }
    // The top tile is the one traversable tile. Its two park neighbors are
    // Mountains, so it is Charming and the complete diamond is legal.
    game.map.tiles.get_mut(&positions[0]).unwrap().terrain = crate::name!("grassland");
    game.players[0].civics.insert(crate::name!("conservation"));
    assert!(game.valid_national_park_site(0, &positions));
    (game, city, positions)
}

#[test]
fn naturalists_are_progressive_faith_only_purchases() {
    let (mut game, city, _) = controlled_park_game();
    let item = Item::Unit {
        unit: crate::name!("naturalist"),
    };
    assert!(!game.can_produce(0, city, &item));
    assert_eq!(game.naturalist_purchase_cost(0), 600.0);

    game.players[0].gold = 10_000.0;
    game.players[0].faith = 599.0;
    assert!(game
        .apply(
            0,
            &Action::Buy {
                city,
                unit: crate::name!("naturalist"),
                formation: 0,
                currency: "gold".to_string(),
            },
        )
        .is_err());
    assert!(game
        .apply(
            0,
            &Action::Buy {
                city,
                unit: crate::name!("naturalist"),
                formation: 0,
                currency: "faith".to_string(),
            },
        )
        .is_err());

    game.players[0].faith = 600.0;
    game.apply(
        0,
        &Action::Buy {
            city,
            unit: crate::name!("naturalist"),
            formation: 0,
            currency: "faith".to_string(),
        },
    )
    .unwrap();
    assert_eq!(game.players[0].faith, 0.0);
    assert_eq!(game.players[0].counters["purchased:naturalist"], 1);
    assert_eq!(game.naturalist_purchase_cost(0), 700.0);

    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(restored.naturalist_purchase_cost(0), 700.0);
}

#[test]
fn national_parks_use_four_tiles_live_appeal_and_exact_city_amenities() {
    let (mut game, city, positions) = controlled_park_game();

    let mut other_cities = Vec::new();
    for position in game.map.tiles.keys().copied().collect::<Vec<_>>() {
        if other_cities.len() == 5 {
            break;
        }
        if positions.contains(&position)
            || game.map.tiles[&position].owner_city.is_some()
            || game
                .cities
                .values()
                .any(|candidate| game.wdist(candidate.pos, position) < 3)
        {
            continue;
        }
        game.map.tiles.get_mut(&position).unwrap().terrain = crate::name!("plains");
        other_cities.push(game.found_city_for(0, position, None));
    }
    assert_eq!(other_cities.len(), 5);

    let city_ids: Vec<u32> = std::iter::once(city)
        .chain(other_cities.iter().copied())
        .collect();
    let amenity_before: BTreeMap<u32, i64> = city_ids
        .iter()
        .map(|city_id| (*city_id, game.city_local_amenities(&game.cities[city_id])))
        .collect();
    let tourism_before = game.tourism_per_turn(0);
    let culture_before = game
        .player_city_ids(0)
        .into_iter()
        .map(|city_id| game.city_yields(city_id).culture)
        .sum::<f64>();
    let global_multiplier =
        1.0 + (game.tree_effect(0, "tourism_pct") + game.monopoly_bonuses(0).1) / 100.0;
    let initial_appeal: i32 = positions
        .iter()
        .map(|position| game.tile_appeal(*position))
        .sum();
    let naturalist = game.spawn_unit("naturalist", 0, positions[0]);
    assert!(game
        .valid_improvements(0, positions[0])
        .contains(&crate::name!("national_park")));
    game.apply(
        0,
        &Action::Improve {
            unit: naturalist,
            improvement: crate::name!("national_park"),
        },
    )
    .unwrap();
    assert!(!game.units.contains_key(&naturalist));
    assert!(positions.iter().all(|position| {
        game.map.tiles[position].improvement.as_deref() == Some("national_park")
    }));
    let culture_with_park = game
        .player_city_ids(0)
        .into_iter()
        .map(|city_id| game.city_yields(city_id).culture)
        .sum::<f64>();
    let tourism_with_park = game.tourism_per_turn(0);
    let expected_gain = (initial_appeal as f64 + 0.15 * (culture_with_park - culture_before))
        * global_multiplier;
    assert!((tourism_with_park - tourism_before - expected_gain).abs() < 1e-9);

    let mut nearest = other_cities.clone();
    nearest.sort_by_key(|city_id| {
        (
            positions
                .iter()
                .map(|position| game.wdist(game.cities[city_id].pos, *position))
                .min()
                .unwrap(),
            *city_id,
        )
    });
    let nearest: BTreeSet<u32> = nearest.into_iter().take(4).collect();
    for city_id in city_ids {
        let expected = if city_id == city {
            2
        } else if nearest.contains(&city_id) {
            1
        } else {
            0
        };
        assert_eq!(
            game.city_local_amenities(&game.cities[&city_id]) - amenity_before[&city_id],
            expected
        );
    }

    // Degrading the surroundings changes the already-established park's
    // Tourism immediately, including negative Appeal rather than a clamp.
    let outside: BTreeSet<Pos> = positions
        .iter()
        .flat_map(|position| game.nbrs(*position))
        .filter(|position| !positions.contains(position))
        .collect();
    for position in outside {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.improvement = Some(crate::name!("mine"));
        tile.pillaged = false;
    }
    let degraded_appeal: i32 = positions
        .iter()
        .map(|position| game.tile_appeal(*position))
        .sum();
    assert!(degraded_appeal < initial_appeal);
    let degraded_culture = game
        .player_city_ids(0)
        .into_iter()
        .map(|city_id| game.city_yields(city_id).culture)
        .sum::<f64>();
    let expected_change = ((degraded_appeal - initial_appeal) as f64
        + 0.15 * (degraded_culture - culture_with_park))
        * global_multiplier;
    assert!((game.tourism_per_turn(0) - tourism_with_park - expected_change).abs() < 1e-9);

    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(
        restored.established_national_parks(0),
        vec![(city, positions)]
    );
    assert!((restored.tourism_per_turn(0) - game.tourism_per_turn(0)).abs() < 1e-9);
}
