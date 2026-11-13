//! Batch ZOC reuse preserves religious distinctions, scope expiry and flood parents.

use super::*;

fn board(planet: bool) -> (Game, Pos, Pos, Pos) {
    let mut game = if planet {
        let size = crate::setup::MapSize::for_players(2);
        let (width, height) = size.dimensions(crate::setup::MapTopology::Planet);
        Game::new_with(GameOptions {
            map_topology: crate::setup::MapTopology::Planet,
            ..GameOptions::new(2, width, height, 6117, 40, 0)
        })
    } else {
        Game::new_full(2, 20, 14, 6117, 40, 0, false)
    };
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    game.map.clear_rivers();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.owner_city = None;
        tile.road = 0;
    }
    let start = *game.map.tiles.keys().next().unwrap();
    let enemy = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| game.wdist(start, *pos) >= 5)
        .unwrap();
    let target = game
        .nbrs(enemy)
        .into_iter()
        .find(|pos| game.wdist(start, *pos) >= 4)
        .unwrap();
    (game, start, enemy, target)
}

fn calculations() -> u64 {
    ZOC_CALCULATIONS.with(std::cell::Cell::get)
}

#[test]
fn different_unit_kinds_share_one_positive_calculation_and_then_expire() {
    let (mut game, start, enemy, target) = board(false);
    let first = game.spawn_unit("warrior", 0, start);
    let second = game.spawn_unit("slinger", 0, start);
    game.spawn_unit("warrior", 1, enemy);
    game.at_war.insert(pair(0, 1));
    let before = calculations();
    {
        let _batch = game.zoc_batch_memo();
        assert!(game.in_enemy_zoc_for(first, target));
        assert!(game.in_enemy_zoc_for(second, target));
        assert_eq!(calculations() - before, 1);
    }
    assert!(game.in_enemy_zoc_for(first, target));
    assert!(game.in_enemy_zoc_for(second, target));
    assert_eq!(calculations() - before, 3);
    game.at_war.clear();
    let _batch = game.zoc_batch_memo();
    assert!(!game.in_enemy_zoc_for(second, target));
    assert_eq!(calculations() - before, 4);
}

#[test]
fn negative_answers_are_shared_but_different_owners_are_not() {
    let (mut game, start, enemy, target) = board(false);
    let first = game.spawn_unit("warrior", 0, start);
    let second = game.spawn_unit("slinger", 0, start);
    let projector = game.spawn_unit("warrior", 1, enemy);
    {
        let _batch = game.zoc_batch_memo();
        let before = calculations();
        assert!(!game.in_enemy_zoc_for(first, target));
        assert!(!game.in_enemy_zoc_for(second, target));
        assert_eq!(calculations() - before, 1);
    }
    game.at_war.insert(pair(0, 1));
    let _batch = game.zoc_batch_memo();
    let before = calculations();
    assert!(game.in_enemy_zoc_for(first, target));
    assert!(!game.in_enemy_zoc_for(projector, target));
    assert!(game.in_enemy_zoc_for(second, target));
    assert_eq!(calculations() - before, 2);
}

#[test]
fn religious_context_includes_actual_class_and_religion() {
    let (mut game, start, enemy, target) = board(false);
    let same = game.spawn_unit("apostle", 0, start);
    let different = game.spawn_unit("apostle", 0, start);
    let ordinary = game.spawn_unit("warrior", 0, start);
    let tagged_military = game.spawn_unit("slinger", 0, start);
    let projector = game.spawn_unit("apostle", 1, enemy);
    for (uid, religion) in [
        (same, "alpha"),
        (different, "beta"),
        (tagged_military, "beta"),
        (projector, "alpha"),
    ] {
        game.units.get_mut(&uid).unwrap().religion = Some(religion.to_owned());
    }
    let _batch = game.zoc_batch_memo();
    let before = calculations();
    assert!(!game.in_enemy_zoc_for(same, target));
    assert!(game.in_enemy_zoc_for(different, target));
    assert!(!game.in_enemy_zoc_for(ordinary, target));
    assert!(!game.in_enemy_zoc_for(tagged_military, target));
    assert_eq!(calculations() - before, 3);
}

#[test]
fn nested_batches_and_ordinary_query_scopes_keep_the_outer_batch() {
    let (mut game, start, enemy, target) = board(false);
    let first = game.spawn_unit("warrior", 0, start);
    let second = game.spawn_unit("slinger", 0, start);
    game.spawn_unit("warrior", 1, enemy);
    game.at_war.insert(pair(0, 1));
    let _batch = game.zoc_batch_memo();
    let before = calculations();
    assert!(game.in_enemy_zoc_for(first, target));
    {
        let _inner = game.zoc_batch_memo();
        let _queries = game.query_memo();
        assert!(game.in_enemy_zoc_for(second, target));
    }
    assert!(game.in_enemy_zoc_for(first, target));
    assert_eq!(calculations() - before, 1);
}

#[test]
fn a_clone_does_not_inherit_the_batch() {
    let (mut game, start, enemy, target) = board(false);
    let mover = game.spawn_unit("warrior", 0, start);
    game.spawn_unit("warrior", 1, enemy);
    game.at_war.insert(pair(0, 1));
    let _batch = game.zoc_batch_memo();
    assert!(game.in_enemy_zoc_for(mover, target));
    let mut branch = game.clone();
    branch.at_war.clear();
    assert!(!branch.in_enemy_zoc_for(mover, target));
    assert!(game.in_enemy_zoc_for(mover, target));
}

#[test]
fn policy_traces_repeat_original_reads_even_with_a_cached_answer() {
    let (mut game, start, enemy, target) = board(false);
    let first = game.spawn_unit("warrior", 0, start);
    let second = game.spawn_unit("slinger", 0, start);
    game.spawn_unit("warrior", 1, enemy);
    game.at_war.insert(pair(0, 1));
    let _batch = game.zoc_batch_memo();
    assert!(game.in_enemy_zoc_for(first, target));
    let trace = game.trace_policy_reads(0);
    let before = calculations();
    assert!(game.in_enemy_zoc_for(first, target));
    assert!(game.in_enemy_zoc_for(second, target));
    assert_eq!(calculations() - before, 2);
    drop(trace);
    assert!(game.in_enemy_zoc_for(second, target));
    assert_eq!(calculations() - before, 2);
}

#[test]
fn different_kinds_and_statuses_match_the_original_predicate() {
    let (mut game, start, enemy, target) = board(false);
    let projector = game.spawn_unit("apostle", 1, enemy);
    game.units.get_mut(&projector).unwrap().religion = Some("alpha".to_owned());
    let mut movers = Vec::new();
    for owner in 0..2 {
        for kind in [
            "warrior", "slinger", "horseman", "galley", "settler", "apostle",
        ] {
            for religion in [None, Some("alpha"), Some("beta")] {
                let uid = game.spawn_unit(kind, owner, start);
                let unit = game.units.get_mut(&uid).unwrap();
                unit.religion = religion.map(str::to_owned);
                unit.hp = 37;
                unit.acted = true;
                unit.moved = true;
                movers.push(uid);
            }
        }
    }
    let targets = [target, start, enemy];
    let expected: Vec<_> = movers
        .iter()
        .map(|uid| {
            targets
                .iter()
                .map(|pos| game.in_enemy_zoc_for_uncached(*uid, *pos))
                .collect::<Vec<_>>()
        })
        .collect();
    let _batch = game.zoc_batch_memo();
    for (uid, expected) in movers.iter().zip(expected) {
        for (pos, expected) in targets.iter().zip(expected) {
            assert_eq!(
                game.in_enemy_zoc_for(*uid, *pos),
                expected,
                "{uid} at {pos:?}"
            );
        }
    }
}

fn flood<V: FloodArrival>(game: &Game, uid: u32) -> (BTreeMap<Pos, V>, BTreeMap<Pos, Pos>) {
    let _queries = game.query_memo();
    let mut parents = BTreeMap::new();
    let values = game.relax_movement(
        uid,
        game.units[&uid].pos,
        5.0,
        |from, to| game.can_pass_neighbor(uid, from, to),
        |to, from| {
            parents.insert(to, from);
        },
    );
    (values, parents)
}

#[test]
fn both_flood_modes_keep_every_score_and_parent_across_units() {
    for planet in [false, true] {
        let (mut game, start, enemy, _) = board(planet);
        let movers: Vec<_> = ["warrior", "slinger", "horseman"]
            .into_iter()
            .map(|kind| game.spawn_unit(kind, 0, start))
            .collect();
        game.spawn_unit("warrior", 1, enemy);
        game.at_war.insert(pair(0, 1));
        let fresh = game.clone();
        let expected: Vec<_> = movers
            .iter()
            .map(|uid| {
                (
                    flood::<f64>(&fresh, *uid),
                    flood::<(f64, bool)>(&fresh, *uid),
                )
            })
            .collect();
        let _batch = game.zoc_batch_memo();
        for (uid, (expected, expected_approach)) in movers.iter().zip(expected) {
            let (values, parents) = flood::<f64>(&game, *uid);
            let bits = |values: BTreeMap<Pos, f64>| {
                values
                    .into_iter()
                    .map(|(pos, value)| (pos, value.to_bits()))
                    .collect::<Vec<_>>()
            };
            assert_eq!(bits(values), bits(expected.0));
            assert_eq!(parents, expected.1);
            let (values, parents) = flood::<(f64, bool)>(&game, *uid);
            let bits = |values: BTreeMap<Pos, (f64, bool)>| {
                values
                    .into_iter()
                    .map(|(pos, (value, stopped))| (pos, (value.to_bits(), stopped)))
                    .collect::<Vec<_>>()
            };
            assert_eq!(bits(values), bits(expected_approach.0));
            assert_eq!(parents, expected_approach.1);
        }
    }
}
