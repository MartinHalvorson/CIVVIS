use super::*;

// The original ranked allocation is an independent oracle for the shortcut.
fn ranked_allocations(game: &Game, pid: usize) -> BTreeMap<u32, i64> {
    let cities: Vec<&City> = game
        .cities
        .values()
        .filter(|city| city.owner == pid)
        .collect();
    let mut allocations: BTreeMap<u32, i64> = cities.iter().map(|city| (city.id, 0)).collect();
    let mut surplus: BTreeMap<u32, i64> = cities
        .iter()
        .map(|city| {
            (
                city.id,
                game.city_local_amenities(city) - Game::city_amenities_required(city),
            )
        })
        .collect();
    let mut supplied_luxuries = Vec::new();
    for luxury in game.empire_luxury_names(pid) {
        if game.congress_effect_active("luxury_policy", "B", &luxury) {
            continue;
        }
        let copies = if game.congress_effect_active("luxury_policy", "A", &luxury) {
            game.resource_access_count(pid, &luxury).max(1) as usize
        } else {
            1_usize
        };
        supplied_luxuries.extend(std::iter::repeat_n(luxury, copies));
    }
    for luxury in supplied_luxuries {
        // Cinnamon and Cloves each provide six Amenities. The Aztec
        // ability independently raises every ordinary Luxury to the same
        // six-city reach.
        let reach = if matches!(luxury.as_str(), "cinnamon" | "cloves")
            || game.has_ability(pid, "gifts_for_the_tlatoani")
        {
            6
        } else {
            4
        };
        let mut neediest: Vec<u32> = cities.iter().map(|city| city.id).collect();
        neediest.sort_by_key(|cid| (surplus[cid], *cid));
        for cid in neediest.into_iter().take(reach) {
            *allocations.get_mut(&cid).unwrap() += 1;
            *surplus.get_mut(&cid).unwrap() += 1;
        }
    }
    allocations
}

fn board(count: usize) -> Game {
    let mut game = Game::new_full(1, 48, 32, 913_3549, 300, 0, false);
    game.players[0].civ = "Rome".to_string();
    for tile in game.map.tiles.values_mut() {
        tile.resource = None;
    }
    for index in 0..count {
        let position = game
            .map
            .tiles
            .iter()
            .find_map(|(pos, tile)| {
                (tile.owner_city.is_none()
                    && game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && game
                        .cities
                        .values()
                        .all(|city| game.wdist(city.pos, *pos) >= 4))
                .then_some(*pos)
            })
            .expect("fixture has room for cities");
        let cid = game.found_city_for(0, position, None);
        game.cities.get_mut(&cid).unwrap().pop = 1 + (index * 5 % 17) as i32;
    }
    game
}

fn give_luxuries(game: &mut Game) {
    let positions: Vec<_> = game.cities.values().map(|city| city.pos).collect();
    for (pos, luxury) in positions
        .into_iter()
        .zip(["silk", "silk", "spices", "diamonds"])
    {
        game.map.tiles.get_mut(&pos).unwrap().resource = Some(Name::new(luxury));
    }
}

fn assert_allocation(game: &Game) {
    let expected = ranked_allocations(game, 0);
    assert_eq!(game.luxury_amenity_allocations_uncached(0), expected);
    let memo = game.query_memo();
    for _ in 0..2 {
        for city in game.cities.values() {
            assert_eq!(game.city_luxury_amenities(city), expected[&city.id]);
        }
    }
    drop(memo);
    assert!(game.query_memo.lux_alloc.borrow().is_none());
}

#[test]
fn luxury_allocation_matches_ranked_oracle_across_reach_and_congress_boundaries() {
    for cities in [0, 1, 4, 5, 6, 7, 10] {
        let mut game = board(cities);
        assert_allocation(&game);
        give_luxuries(&mut game);
        for civilization in ["Rome", "Aztec"] {
            game.players[0].civ = civilization.to_string();
            assert_eq!(
                game.has_ability(0, "gifts_for_the_tlatoani"),
                civilization == "Aztec"
            );
            for outcome in [None, Some("A"), Some("B")] {
                game.active_congress_effects.clear();
                if let Some(outcome) = outcome {
                    game.active_congress_effects.push(CongressEffect {
                        resolution: "luxury_policy".to_string(),
                        outcome: outcome.to_string(),
                        target: "silk".to_string(),
                        expires: game.turn + 10,
                    });
                }
                assert_allocation(&game);
            }
        }
        game.active_congress_effects.clear();
        game.players[0].civ = "Rome".to_string();
        let minor = game.players.len();
        game.players.push(Player::new(minor, "Zanzibar", true));
        game.players[0].envoys.push((minor, 3));
        assert!(game
            .empire_luxury_names(0)
            .contains(&crate::name!("cinnamon")));
        assert_allocation(&game);
    }
}

#[test]
fn universal_luxuries_skip_city_valuations_and_memo_expires_before_mutation() {
    for count in [1, 4, 7] {
        let mut game = board(count);
        {
            let _memo = game.query_memo();
            let city = game.cities.values().next().unwrap();
            assert_eq!(game.city_luxury_amenities(city), 0);
            assert!(game
                .query_memo
                .amenities
                .borrow()
                .as_ref()
                .unwrap()
                .is_empty());
        }
        give_luxuries(&mut game);
        {
            let _memo = game.query_memo();
            let values: Vec<_> = game
                .cities
                .values()
                .map(|city| game.city_luxury_amenities(city))
                .collect();
            assert!(
                values.iter().sum::<i64>() > 0,
                "new scope must see newly connected luxuries"
            );
            if count <= 4 {
                assert!(game
                    .query_memo
                    .amenities
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .is_empty());
                assert!(values
                    .iter()
                    .all(|value| *value == game.empire_luxury_names(0).len() as i64));
            } else {
                assert_eq!(
                    game.query_memo.amenities.borrow().as_ref().unwrap().len(),
                    count
                );
            }
        }
    }
}
