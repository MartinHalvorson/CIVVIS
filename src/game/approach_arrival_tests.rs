use super::*;

fn board(seed: u64) -> Game {
    let mut g = Game::new_full(2, 20, 14, seed, 80, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    g.map.clear_rivers();
    for player in &mut g.players {
        player.civ = "Rome".to_string();
        player.government = None;
        player.policies.clear();
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.owner_city = None;
        tile.hills = false;
        tile.road = 0;
    }
    g.current = 0;
    g.record_contact(0, 1);
    g.at_war.extend([(0, 1), (1, 0)]);
    g
}

fn assert_same_arrivals(g: &Game, uid: u32) {
    let expected: BTreeMap<_, _> = g
        .approach_reach(uid)
        .into_iter()
        .map(|(pos, (moves, _))| (pos, moves.to_bits()))
        .collect();
    let arrivals = g.approach_arrivals(uid);
    let actual: BTreeMap<_, _> = arrivals
        .iter()
        .map(|(pos, moves)| (*pos, moves.to_bits()))
        .collect();
    assert_eq!(arrivals.len(), actual.len(), "destinations are distinct");
    assert_eq!(actual, expected, "every movement value is bit-identical");
}

#[test]
fn arrivals_match_paths_across_partial_moves_rough_ground_roads_and_zoc() {
    for seed in [9_613, 9_614, 9_615] {
        for kind in ["warrior", "scout", "archer", "knight", "catapult"] {
            let mut g = board(seed);
            for tile in g.map.tiles.values_mut() {
                tile.hills = (tile.pos.0 + tile.pos.1).rem_euclid(4) == 0;
                if (tile.pos.0 - tile.pos.1).rem_euclid(5) == 0 {
                    tile.feature = Some(crate::name!("woods"));
                }
                if tile.pos.1 == 7 {
                    tile.road = 1;
                }
            }
            let uid = g.spawn_test_unit(kind, 0, (9, 7));
            g.spawn_test_unit("warrior", 0, (10, 7));
            g.spawn_test_unit("warrior", 1, (12, 7));
            for moves in [0.25, 1.0, 2.5, 5.0] {
                g.units.get_mut(&uid).unwrap().moves_left = moves;
                let before = format!("{:?}", g.units);
                assert_same_arrivals(&g, uid);
                assert_eq!(format!("{:?}", g.units), before);
            }
        }
    }
}

#[test]
fn an_arrival_can_cross_a_friendly_unit_without_ending_on_it() {
    let mut g = board(9_616);
    let uid = g.spawn_test_unit("knight", 0, (9, 7));
    let friend = (10, 7);
    g.spawn_test_unit("warrior", 0, friend);
    let paths = g.approach_reach(uid);
    assert!(
        paths.values().any(|(_, path)| path.contains(&friend)),
        "fixture must exercise an actual crossing"
    );
    assert!(!paths.contains_key(&friend));
    assert_same_arrivals(&g, uid);
}

#[test]
fn naval_arrivals_and_empty_readings_keep_the_path_readers_contract() {
    let mut g = board(9_617);
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("coast");
    }
    let uid = g.spawn_test_unit("galley", 0, (9, 7));
    g.spawn_test_unit("galley", 0, (10, 7));
    g.spawn_test_unit("galley", 1, (12, 7));
    assert!(!g.approach_arrivals(uid).is_empty());
    assert_same_arrivals(&g, uid);
    g.units.get_mut(&uid).unwrap().moves_left = 0.0;
    assert!(g.approach_arrivals(uid).is_empty());
    assert_same_arrivals(&g, uid);
    assert!(g.approach_arrivals(u32::MAX).is_empty());
    assert_same_arrivals(&g, u32::MAX);
}

/// A manual kernel measurement, separate from the correctness gate. Alternate
/// the reader order to reduce ordering bias; print all samples, never assert
/// a wall-clock threshold on a shared host.
#[test]
#[ignore = "manual release-profile performance measurement"]
fn measure_arrivals_without_discarded_paths() {
    let mut g = board(9_618);
    for tile in g.map.tiles.values_mut() {
        tile.road = 1;
    }
    let uid = g.spawn_test_unit("knight", 0, (9, 7));
    g.spawn_test_unit("warrior", 0, (10, 7));
    g.spawn_test_unit("warrior", 1, (14, 7));
    g.units.get_mut(&uid).unwrap().moves_left = 5.0;
    assert_same_arrivals(&g, uid);
    for sample in 0..6 {
        let mut times = [0; 2];
        let mut checksum = [0; 2];
        for reader in if sample % 2 == 0 { [0, 1] } else { [1, 0] } {
            let started = std::time::Instant::now();
            for _ in 0..300 {
                checksum[reader] += if reader == 0 {
                    std::hint::black_box(g.approach_reach(uid)).len()
                } else {
                    std::hint::black_box(g.approach_arrivals(uid)).len()
                };
            }
            times[reader] = started.elapsed().as_nanos();
        }
        assert_eq!(checksum[0], checksum[1]);
        println!(
            "arrival_kernel sample={sample} path_ns={} arrival_ns={} destinations={}",
            times[0], times[1], checksum[0]
        );
    }
}
