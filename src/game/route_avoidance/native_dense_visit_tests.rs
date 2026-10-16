use super::*;
use std::cell::RefCell;

fn plain_game(seed: u64, topology: crate::world::Topology) -> Game {
    let mut g = Game::new_full(2, 18, 12, seed, 200, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.cities.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.owner_city = None;
    }
    g.map.topology = topology;
    g
}

fn compare(g: &Game, uid: u32, goal: Pos, blocked: &BTreeSet<Pos>) -> bool {
    let expected_probes = RefCell::new(Vec::new());
    let expected = old_predicate_route(g, uid, goal, |pos| {
        expected_probes.borrow_mut().push(pos);
        blocked.contains(&pos)
    });
    let actual_probes = RefCell::new(Vec::new());
    let actual = g.route_step_avoiding_when(uid, goal, |pos| {
        actual_probes.borrow_mut().push(pos);
        blocked.contains(&pos)
    });
    assert_eq!(actual, expected, "uid {uid}, goal {goal:?}");
    assert_eq!(
        actual_probes.into_inner(),
        expected_probes.into_inner(),
        "same blocked-predicate probes in the same order for {goal:?}"
    );
    assert_eq!(
        actual,
        g.route_step_avoiding_tiles(uid, goal, blocked),
        "the independently unchanged set router agrees for {goal:?}"
    );
    expected.is_some()
}

#[test]
fn dense_visit_routes_and_predicate_order_match_the_frozen_tree_search() {
    let mut reachable = 0;
    for topology in [
        crate::world::Topology::Rectangle,
        crate::world::Topology::Cylinder,
    ] {
        for seed in [40800011, 40800012, 40800013] {
            let mut g = plain_game(seed, topology);
            let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
            let start = positions[positions.len() / 3];
            for (index, pos) in positions.iter().enumerate() {
                if *pos != start && index % 11 == 0 {
                    g.map.tiles.get_mut(pos).unwrap().terrain = crate::name!("mountain");
                }
            }
            let uid = g.spawn_unit("warrior", 0, start);
            for stride in [0, 13, 29] {
                let blocked: BTreeSet<Pos> = positions
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| stride > 0 && index % stride == 0)
                    .map(|(_, pos)| *pos)
                    .collect();
                for goal in positions.iter().step_by(9) {
                    reachable += usize::from(compare(&g, uid, *goal, &blocked));
                }
            }
        }
    }
    assert!(reachable > 100, "the matrix exercises real routes");
}

#[test]
fn dense_visit_sparse_grids_and_unindexable_starts_keep_the_old_result() {
    let mut g = plain_game(40800014, crate::world::Topology::Cylinder);
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
    let start = positions[positions.len() / 2];
    let uid = g.spawn_unit("warrior", 0, start);
    let blocked = BTreeSet::new();
    let mut reachable = 0;
    for goal in positions.iter().step_by(11) {
        reachable += usize::from(compare(&g, uid, *goal, &blocked));
    }
    assert!(reachable > 0, "the sparse grid contains usable roads");
    g.units.get_mut(&uid).unwrap().pos = (-100, -100);
    assert!(g.map.tiles.index_of((-100, -100)).is_none());
    for goal in positions.iter().step_by(11) {
        compare(&g, uid, *goal, &blocked);
    }
}

#[test]
fn dense_visit_start_target_zoc_and_ordinary_cache_gates_are_unchanged() {
    let mut g = plain_game(40800015, crate::world::Topology::Rectangle);
    let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
    let start = positions[positions.len() / 2];
    let uid = g.spawn_unit("warrior", 0, start);
    let goal = g.nbrs(start)[0];
    let empty = BTreeSet::new();
    assert!(compare(&g, uid, goal, &empty));
    let ordinary = g.route_step(uid, goal, 0);
    assert!(ordinary.is_some());
    for blocked in [
        BTreeSet::from([start]),
        BTreeSet::from([goal]),
        positions.iter().copied().collect(),
    ] {
        compare(&g, uid, goal, &blocked);
        assert_eq!(g.route_step(uid, goal, 0), ordinary);
    }
    compare(&g, uid, start, &empty);
    compare(&g, u32::MAX, goal, &empty);
    compare(&g, uid, (-100, -100), &empty);
    g.units.get_mut(&uid).unwrap().zoc_stopped = true;
    assert!(!compare(&g, uid, goal, &empty));
}

// Frozen pre-experiment predicate router; not a call into the new implementation.
fn old_predicate_route(
        g: &Game,
        uid: u32,
        target: Pos,
        blocked: impl Fn(Pos) -> bool,
    ) -> Option<Pos> {
        let unit = g.units.get(&uid)?;
        let start = unit.pos;
        if start == target || blocked(target) || g.formation_movement_locked_by_zoc(uid) {
            return None;
        }
        let access = g.unit_territory_access(unit);
        if !g.can_path_through_neighbor(uid, target, &access) {
            return None;
        }
        let _memo = g.query_memo();
        let mut seen = BTreeSet::new();
        seen.insert(start);
        let mut queue = VecDeque::from([(start, start)]);
        while let Some((current, first)) = queue.pop_front() {
            for next in g.nbrs(current) {
                if seen.contains(&next) || blocked(next) {
                    continue;
                }
                let allowed = if current == start {
                    g.can_enter_neighbor(uid, current, next)
                } else {
                    g.can_path_through_neighbor(uid, next, &access)
                };
                if !allowed {
                    continue;
                }
                seen.insert(next);
                let first = if current == start { next } else { first };
                if next == target {
                    return Some(first);
                }
                queue.push_back((next, first));
            }
        }
        None
    }
