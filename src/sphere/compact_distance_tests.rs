//! Compact exact globe distances retain BFS answers, caps and bounded fallback.

use super::*;

/// Independent unweighted graph traversal, without the table, landmarks,
/// lazy rows, ring search or any public distance reader.
fn graph_distances(globe: &Sphere, source: usize) -> Vec<i32> {
    let mut distances = vec![-1; globe.len()];
    distances[source] = 0;
    let mut queue = vec![source];
    let mut cursor = 0;
    while cursor < queue.len() {
        let here = queue[cursor];
        cursor += 1;
        for &next in &globe.adjacent[here][..globe.degree[here] as usize] {
            let next = next as usize;
            if distances[next] == -1 {
                distances[next] = distances[here] + 1;
                queue.push(next);
            }
        }
    }
    assert!(distances.iter().all(|distance| *distance >= 0));
    distances
}

#[test]
fn every_pair_matches_independent_bfs_across_hexagons_and_pentagons() {
    for frequency in [1, 2, 3, 4, 6] {
        let globe = build(frequency);
        assert!(globe.compact_distances.is_some());
        assert!(globe.degree.contains(&5));
        if frequency > 1 {
            assert!(globe.degree.contains(&6));
        }
        for source in 0..globe.len() {
            let expected = graph_distances(&globe, source);
            for (target, distance) in expected.into_iter().enumerate() {
                assert_eq!(
                    globe.distance(globe.pos[source], globe.pos[target]),
                    distance,
                    "frequency {frequency}: {source} -> {target}"
                );
            }
        }
    }
}

#[test]
fn negative_saturated_and_ring_boundary_caps_keep_exact_minimums() {
    let globe = build(6);
    for source in [0, globe.len() / 3, globe.len() - 1] {
        let expected = graph_distances(&globe, source);
        for (target, distance) in expected.into_iter().enumerate() {
            for cap in [i32::MIN, -3, -1, 0, 1, 2, 5, 6, 7, 9, 40, i32::MAX] {
                assert_eq!(
                    globe.distance_capped(globe.pos[source], globe.pos[target], cap),
                    distance.min(cap.saturating_add(1)),
                    "{source} -> {target} at cap {cap}"
                );
            }
        }
    }
}

#[test]
fn invalid_positions_keep_the_original_cylindrical_fallback() {
    let globe = build(3);
    let invalid = [(-50, -50), (500, -20)];
    assert!(invalid.iter().all(|pos| globe.index_of(*pos).is_none()));
    for (from, to) in [
        (invalid[0], globe.pos[0]),
        (globe.pos[0], invalid[1]),
        (invalid[0], invalid[1]),
        (invalid[0], invalid[0]),
    ] {
        let expected = hex::wdistance(from, to, globe.width);
        assert_eq!(globe.distance(from, to), expected);
        for cap in [i32::MIN, -1, 0, 1, 6, 100, i32::MAX] {
            assert_eq!(
                globe.distance_capped(from, to, cap),
                expected.min(cap.saturating_add(1))
            );
        }
    }
}

#[test]
fn byte_budget_rejects_small_budgets_and_large_globes_without_partial_tables() {
    let globe = build(3);
    let exact_bytes = globe.len() * globe.len();
    assert!(globe.build_compact_distances(0).is_none());
    assert!(globe.build_compact_distances(exact_bytes - 1).is_none());
    assert_eq!(
        globe.build_compact_distances(exact_bytes).unwrap().len(),
        exact_bytes
    );
    assert_eq!(COMPACT_DISTANCE_TABLE_BYTES, 16 * 1024 * 1024);
    let large = build(21);
    assert!(large.len() * large.len() > COMPACT_DISTANCE_TABLE_BYTES);
    assert!(large.compact_distances.is_none());
    let expected = graph_distances(&large, 0);
    for target in [0, large.len() / 2, large.len() - 1] {
        assert_eq!(
            large.distance(large.pos[0], large.pos[target]),
            expected[target]
        );
    }
}

#[test]
fn normal_roster_topologies_use_one_byte_per_pair_and_exact_answers() {
    for (frequency, tiles) in [(15, 2252), (18, 3242)] {
        let globe = build(frequency);
        assert_eq!(globe.len(), tiles);
        assert_eq!(
            globe.compact_distances.as_ref().unwrap().len(),
            tiles * tiles
        );
        assert!(tiles * tiles <= COMPACT_DISTANCE_TABLE_BYTES);
        for source in [0, tiles / 3, tiles / 2, tiles - 1] {
            let expected = graph_distances(&globe, source);
            for (target, distance) in expected.into_iter().enumerate() {
                assert_eq!(
                    globe.distance(globe.pos[source], globe.pos[target]),
                    distance
                );
            }
        }
        assert!(globe.distance_rows.iter().all(|row| row.get().is_none()));
    }
}

#[test]
fn disks_rings_and_caller_owned_rows_do_not_depend_on_the_optional_table() {
    let mut globe = build(4);
    let from = globe.pos[globe.len() / 3];
    let disks: Vec<_> = (0..12).map(|radius| globe.disk(from, radius)).collect();
    let rings: Vec<_> = (0..12).map(|radius| globe.ring(from, radius)).collect();
    let row = globe.distance_row(from);
    globe.compact_distances = None;
    for radius in 0..12 {
        assert_eq!(globe.disk(from, radius), disks[radius as usize]);
        assert_eq!(globe.ring(from, radius), rings[radius as usize]);
    }
    assert_eq!(globe.distance_row(from), row);
}

#[test]
fn disabled_tables_keep_original_lazy_row_admission_and_all_exact_distances() {
    let mut globe = build(6);
    globe.compact_distances = None;
    let expected = graph_distances(&globe, 0);
    let distant: Vec<_> = expected
        .iter()
        .enumerate()
        .filter(|(_, distance)| **distance > RING_RADIUS)
        .map(|(index, _)| index)
        .take(DISTANCE_ROW_ADMISSION_HITS as usize)
        .collect();
    assert_eq!(distant.len(), DISTANCE_ROW_ADMISSION_HITS as usize);
    assert!(globe.distance_rows[0].get().is_none());
    for target in distant {
        assert_eq!(
            globe.distance(globe.pos[0], globe.pos[target]),
            expected[target]
        );
    }
    assert!(globe.distance_rows[0].get().is_some());
    for (target, distance) in expected.into_iter().enumerate() {
        assert_eq!(globe.distance(globe.pos[0], globe.pos[target]), distance);
        for cap in [-1i32, 0, 1, 6, 7, 30] {
            assert_eq!(
                globe.distance_capped(globe.pos[0], globe.pos[target], cap),
                distance.min(cap.saturating_add(1))
            );
        }
    }
}
