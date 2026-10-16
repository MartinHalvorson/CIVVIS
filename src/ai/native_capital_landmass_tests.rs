use super::*;

fn empty_game(map: crate::world::WorldMap) -> Game {
    let mut g = Game::new_full(2, 18, 12, 40810011, 200, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.cities.clear();
    g.map = map;
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.owner_city = None;
    }
    g
}

fn compare(g: &Game, pid: usize) -> BTreeSet<Pos> {
    let expected = old_capital_landmass(g, pid);
    let actual = BasicAi::capital_landmass(g, pid);
    assert_eq!(actual, expected, "pid {pid}");
    actual
}

#[test]
fn capital_landmass_complete_sets_match_the_old_flood_on_all_topologies() {
    let mut nontrivial = 0;
    for shape in 0..3 {
        for variant in 0..3 {
            let map = match shape {
                0 => crate::world::WorldMap::arena(18, 12),
                1 => crate::world::WorldMap::new(18, 12),
                _ => crate::world::WorldMap::globe(2),
            };
            let mut g = empty_game(map);
            let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
            let home = positions[positions.len() / 2];
            for (index, pos) in positions.iter().enumerate() {
                if *pos == home {
                    continue;
                }
                let tile = g.map.tiles.get_mut(pos).unwrap();
                tile.terrain = if variant > 0 && index % 7 == 0 {
                    crate::name!("ocean")
                } else if variant > 1 && index % 11 == 0 {
                    "unknown".into()
                } else if index % 13 == 0 {
                    crate::name!("mountain")
                } else {
                    crate::name!("grassland")
                };
            }
            g.found_city_for(0, home, None);
            let answer = compare(&g, 0);
            assert!(answer.contains(&home));
            nontrivial += usize::from(answer.len() > 2);
            if variant == 0 {
                assert_eq!(
                    answer.len(),
                    positions.len(),
                    "mountains are dry ground even though they are impassable"
                );
            }
        }
    }
    assert!(nontrivial >= 6, "the matrices exercise real components");
}

#[test]
fn capital_landmass_keeps_palace_preference_and_lowest_city_id_ties() {
    let mut g = empty_game(crate::world::WorldMap::arena(18, 12));
    assert!(compare(&g, 0).is_empty(), "no owned city means no home");
    let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
    let first = positions[positions.len() / 4];
    let second = *positions
        .iter()
        .filter(|pos| g.wdist(first, **pos) > 8)
        .max()
        .unwrap();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("ocean");
    }
    for center in [first, second] {
        for pos in g.wdisk(center, 1) {
            g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("grassland");
        }
    }
    let a = g.found_city_for(0, first, None);
    let b = g.found_city_for(0, second, None);
    assert!(a < b, "fixture controls the lowest city id");
    std::sync::Arc::make_mut(&mut g.observed_city_palaces).insert(a, false);
    std::sync::Arc::make_mut(&mut g.observed_city_palaces).insert(b, false);
    let answer = compare(&g, 0);
    assert!(answer.contains(&first) && !answer.contains(&second));
    std::sync::Arc::make_mut(&mut g.observed_city_palaces).insert(b, true);
    assert!(g.city_has_palace(&g.cities[&b]));
    let answer = compare(&g, 0);
    assert!(answer.contains(&second) && !answer.contains(&first));
    std::sync::Arc::make_mut(&mut g.observed_city_palaces).insert(a, true);
    let answer = compare(&g, 0);
    assert!(answer.contains(&first) && !answer.contains(&second));
    assert!(
        compare(&g, 1).is_empty(),
        "another player's cities are ignored"
    );
}

#[test]
fn capital_landmass_keeps_sparse_and_unknown_water_or_unindexable_homes() {
    let mut g = empty_game(crate::world::WorldMap::new(18, 12));
    let mut serialized = serde_json::to_value(&g.map).unwrap();
    let tiles = serialized["tiles"].as_array_mut().unwrap();
    let mut index = 0;
    tiles.retain(|_| {
        index += 1;
        index % 7 != 0
    });
    g.map = serde_json::from_value(serialized).unwrap();
    assert!(g.map.tiles.len() < (g.map.width * g.map.height) as usize);
    let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
    let home = positions[positions.len() / 2];
    let cid = g.found_city_for(0, home, None);
    let answer = compare(&g, 0);
    assert!(answer.contains(&home) && answer.len() > 2);
    for terrain in ["unknown", "ocean"] {
        g.map.tiles.get_mut(&home).unwrap().terrain = terrain.into();
        let answer = compare(&g, 0);
        assert!(
            answer.contains(&home),
            "the initial home is included even on {terrain}"
        );
    }
    let outside = (-100, -100);
    assert!(g.map.tiles.index_of(outside).is_none());
    g.cities.get_mut(&cid).unwrap().pos = outside;
    assert!(compare(&g, 0).contains(&outside));
}

// Frozen pre-experiment flood; the new implementation is not its oracle.
fn old_capital_landmass(g: &Game, pid: usize) -> BTreeSet<Pos> {
    let Some(home) = g
        .player_city_ids(pid)
        .into_iter()
        .filter_map(|cid| g.cities.get(&cid))
        .min_by_key(|city| (std::cmp::Reverse(g.city_has_palace(city)), city.id))
        .map(|city| city.pos)
    else {
        return BTreeSet::new();
    };
    let mut landmass = BTreeSet::new();
    // The set is the answer, but the flood asked it about every
    // neighbour of every plot — six ordered-set probes a plot, for every
    // naval explorer, every turn (1.5% of a ladder game's CPU in the
    // 2026-09-26 profile). A dense table in tile-vector order (see
    // `first_visit` in the settle-site sweep) answers the same question
    // with one index; it is marked exactly when the set is, so the two
    // never disagree, and a plot the grid cannot index (none in practice
    // — every neighbour is on the map) still asks the set.
    let mut seen = vec![false; g.map.tiles.len()];
    let mut frontier = VecDeque::from([home]);
    while let Some(pos) = frontier.pop_front() {
        if !landmass.insert(pos) {
            continue;
        }
        if let Some(index) = g.map.tiles.index_of(pos) {
            seen[index] = true;
        }
        for next in g.nbrs(pos) {
            let visited = match g.map.tiles.index_of(next) {
                Some(index) => seen[index],
                None => landmass.contains(&next),
            };
            if visited {
                continue;
            }
            if g.map
                .get(next)
                .is_some_and(|tile| !g.rules.is_unknown(tile) && !g.rules.is_water(tile))
            {
                frontier.push_back(next);
            }
        }
    }
    landmass
}
