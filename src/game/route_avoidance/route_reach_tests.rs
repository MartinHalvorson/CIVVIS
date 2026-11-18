use super::*;

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

/// Every goal, asked in a scrambled order of one shared search, answers what
/// its own per-target search answers, including goals the per-target
/// predicate frees from the shared blocked set (the siege train's ring tile).
#[test]
fn one_shared_search_answers_every_goal_as_its_own_search_does() {
    let mut reachable = 0;
    let mut unreachable = 0;
    for topology in [
        crate::world::Topology::Rectangle,
        crate::world::Topology::Cylinder,
    ] {
        for seed in [40810011, 40810012, 40810013] {
            let mut g = plain_game(seed, topology);
            let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
            let start = positions[positions.len() / 3];
            for (index, pos) in positions.iter().enumerate() {
                if *pos != start && index % 7 == 3 {
                    g.map.tiles.get_mut(pos).unwrap().terrain = crate::name!("mountain");
                }
            }
            let uid = g.spawn_unit("warrior", 0, start);
            // A foreign unit beside the start: the first edge's own rule.
            if let Some(beside) = g.nbrs(start).into_iter().find(|pos| {
                g.map
                    .get(*pos)
                    .is_some_and(|tile| tile.terrain != crate::name!("mountain"))
            }) {
                g.spawn_unit("warrior", 1, beside);
            }
            for stride in [0usize, 5, 13] {
                let shared: BTreeSet<Pos> = positions
                    .iter()
                    .enumerate()
                    .filter(|(index, pos)| stride > 0 && index % stride == 0 && **pos != start)
                    .map(|(_, pos)| *pos)
                    .collect();
                let mut reach = g.route_reach(uid).expect("the unit stands");
                let mut goals: Vec<(usize, Pos)> = positions.iter().copied().enumerate().collect();
                // Far goals first, then near ones, then the rest: the search
                // is grown and re-read out of order.
                goals.sort_by_key(|(index, _)| (index * 7919) % positions.len());
                for (index, goal) in goals {
                    // One goal in three is freed from the shared set, as the
                    // siege train frees the ring tile it asks about.
                    let freed = index % 3 == 0;
                    let target_blocked = shared.contains(&goal) && !freed;
                    let expected = g
                        .route_step_avoiding_when(uid, goal, |pos| {
                            shared.contains(&pos) && !(freed && pos == goal)
                        })
                        .is_some();
                    let actual =
                        reach.reaches(&g, goal, target_blocked, |pos| shared.contains(&pos));
                    assert_eq!(
                        actual, expected,
                        "goal {goal:?}, stride {stride}, seed {seed}"
                    );
                    if expected {
                        reachable += 1;
                    } else {
                        unreachable += 1;
                    }
                }
            }
        }
    }
    assert!(
        reachable > 500,
        "the matrix exercises real routes: {reachable}"
    );
    assert!(unreachable > 100, "and real refusals: {unreachable}");
}

#[test]
fn a_missing_unit_has_no_search() {
    let g = plain_game(40810014, crate::world::Topology::Rectangle);
    assert!(g.route_reach(987_654).is_none());
}
