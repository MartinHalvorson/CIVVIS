//! Builder census reuse preserves city assignments, scope expiry and traces.

use super::*;

fn cities() -> (Game, [u32; 2]) {
    let mut game = Game::new_full(2, 32, 20, 92_201, 120, 0, false);
    let mut cities = [0; 2];
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| game.units[uid].kind == "settler")
            .unwrap();
        let cid = game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
        game.cities.get_mut(&cid).unwrap().pop = if pid == 0 { 4 } else { 1 };
        game.players[pid].techs.insert(crate::name!("mining"));
        let center = game.cities[&cid].pos;
        for pos in game.cities[&cid].owned_tiles.clone() {
            if pos == center {
                continue;
            }
            let tile = game.map.tiles.get_mut(&pos).unwrap();
            tile.terrain = crate::name!("plains");
            tile.feature = None;
            tile.resource = None;
            tile.hills = false;
            tile.improvement = None;
            tile.district = None;
            tile.district_foundation = None;
            tile.wonder = None;
            tile.pillaged = false;
        }
        cities[pid] = cid;
    }
    (game, cities)
}

// The original builder census, independent of the new count memo.
fn reference(game: &Game, pid: usize) -> usize {
    let _memo = game.query_memo();
    game.player_city_ids(pid)
        .into_iter()
        .map(|cid| {
            game.city_citizen_plan(cid)
                .worked_tiles
                .into_iter()
                .filter(|pos| {
                    game.map.get(*pos).is_some_and(|tile| {
                        tile.district.is_none() && (tile.improvement.is_none() || tile.pillaged)
                    }) && game
                        .valid_improvements(pid, *pos)
                        .iter()
                        .any(|kind| game.rules.improvements[kind].builder_buildable)
                })
                .count()
        })
        .sum()
}

fn improve(game: &mut Game, cid: u32) {
    let center = game.cities[&cid].pos;
    for pos in game.cities[&cid].owned_tiles.clone() {
        if pos != center {
            let tile = game.map.tiles.get_mut(&pos).unwrap();
            tile.improvement = Some(crate::name!("farm"));
            tile.pillaged = false;
        }
    }
}

#[test]
fn nested_scopes_reuse_separate_player_counts_and_zero() {
    let (game, _) = cities();
    let expected = [reference(&game, 0), reference(&game, 1)];
    assert!(expected[0] > expected[1]);
    assert!(game.query_memo.worked_backlog.borrow().is_none());
    {
        let _outer = game.query_memo();
        for _ in 0..3 {
            let _inner = game.query_memo();
            for pid in 0..2 {
                assert_eq!(
                    crate::ai::BasicAi::unimproved_worked_tiles(&game, pid),
                    expected[pid]
                );
            }
            assert_eq!(game.unimproved_worked_tiles(99), 0);
        }
        let memo = game.query_memo.worked_backlog.borrow();
        assert_eq!(memo.as_ref().unwrap().len(), 3);
        assert_eq!(memo.as_ref().unwrap().get(&99), Some(&0));
    }
    assert!(game.query_memo.worked_backlog.borrow().is_none());
}

#[test]
fn a_new_scope_reads_improvements_and_authoritative_work_assignments_again() {
    let (mut game, ids) = cities();
    let old = game.unimproved_worked_tiles(0);
    assert!(old > 0);
    improve(&mut game, ids[0]);
    assert_eq!(game.unimproved_worked_tiles(0), 0);
    let pos = game.cities[&ids[0]]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != game.cities[&ids[0]].pos)
        .unwrap();
    game.map.tiles.get_mut(&pos).unwrap().improvement = None;
    Arc::make_mut(&mut game.observed_city_worked_tiles).insert(ids[0], vec![pos]);
    assert_eq!(game.unimproved_worked_tiles(0), reference(&game, 0));
    assert_eq!(game.unimproved_worked_tiles(0), 1);
}

#[test]
fn a_changed_search_clone_does_not_inherit_the_parent_census() {
    let (game, ids) = cities();
    let _memo = game.query_memo();
    let parent = game.unimproved_worked_tiles(0);
    assert!(parent > 0);
    let mut branch = game.clone();
    assert!(branch.query_memo.worked_backlog.borrow().is_none());
    improve(&mut branch, ids[0]);
    assert_eq!(branch.unimproved_worked_tiles(0), 0);
    assert_eq!(game.unimproved_worked_tiles(0), parent);
}

#[test]
fn a_policy_trace_bypasses_a_primed_count_and_keeps_the_original_reads() {
    let (game, _) = cities();
    let oracle = game.clone();
    let _actual_scope = game.query_memo();
    let _oracle_scope = oracle.query_memo();
    let count = game.unimproved_worked_tiles(0);
    assert_eq!(reference(&oracle, 0), count);
    let expected_trace = oracle.trace_policy_reads(0);
    let expected = reference(&oracle, 0);
    let expected_reads = expected_trace.finish();
    let trace = game.trace_policy_reads(0);
    assert_eq!(game.unimproved_worked_tiles(0), expected);
    let reads = trace.finish();
    assert!(!expected_reads.effects.is_empty());
    assert_eq!(reads.effects, expected_reads.effects);
    assert_eq!(reads.cards, expected_reads.cards);
    assert_eq!(reads.opaque, expected_reads.opaque);
}
