//! Exact empty-goal detection without changing exploration commitments or saves.

use super::*;
use crate::game::GameOptions;
use crate::setup::MapTopology;

fn fixture(shape: MapTopology) -> (Game, u32, Pos) {
    let mut options = GameOptions::new(2, 24, 16, 91_827, 250, 0);
    options.barbarians = false;
    options.map_topology = shape;
    let mut game = Game::new_with(options);
    for id in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(id);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.improvement = None;
    }
    let origin = *game.map.tiles.keys().next().unwrap();
    let next = game.nbrs(origin)[0];
    let scout = game.spawn_unit("scout", 0, origin);
    game.players[0].explored = game.map.tiles.keys().copied().collect();
    (game, scout, next)
}

#[test]
fn completely_charted_planet_and_flat_world_have_no_frontier_goal() {
    for shape in [MapTopology::Flat, MapTopology::Planet] {
        let (game, scout, _) = fixture(shape);
        for committed in [false, true] {
            let mut ai = BasicAi::new();
            if committed {
                ai.enable_explore_commit();
            }
            let before = serde_json::to_vec(&game).unwrap();
            for dry_only in [false, true] {
                assert_eq!(ai.exploration_goal(&game, 0, scout, dry_only), None);
                assert!(ai.explore_goal.borrow().is_empty());
                assert_eq!(serde_json::to_vec(&game).unwrap(), before);
            }
        }
    }
}

#[test]
fn a_single_unknown_tile_is_still_selected_on_both_shapes() {
    for shape in [MapTopology::Flat, MapTopology::Planet] {
        let (mut game, scout, unknown) = fixture(shape);
        game.players[0].explored.remove(&unknown);
        let before = serde_json::to_vec(&game).unwrap();
        let ai = BasicAi::new();
        assert_eq!(ai.exploration_goal(&game, 0, scout, false), Some(unknown));
        assert_eq!(ai.exploration_goal(&game, 0, scout, true), Some(unknown));
        assert_eq!(serde_json::to_vec(&game).unwrap(), before);
    }
}

#[test]
fn equal_counts_with_an_off_map_entry_do_not_hide_a_real_unknown_tile() {
    for shape in [MapTopology::Flat, MapTopology::Planet] {
        let (mut game, scout, unknown) = fixture(shape);
        game.players[0].explored.remove(&unknown);
        let off_map = (999_999, -999_999);
        assert!(!game.map.tiles.contains_key(&off_map));
        game.players[0].explored.insert(off_map);
        assert_eq!(game.players[0].explored.len(), game.map.tiles.len());
        assert_eq!(
            BasicAi::new().exploration_goal(&game, 0, scout, false),
            Some(unknown)
        );
    }
}

#[test]
fn extra_off_map_entries_keep_the_existing_empty_search_result() {
    let (mut game, scout, _) = fixture(MapTopology::Planet);
    game.players[0].explored.insert((999_999, -999_999));
    assert!(game.players[0].explored.len() > game.map.tiles.len());
    let before = serde_json::to_vec(&game).unwrap();
    assert_eq!(
        BasicAi::new().exploration_goal(&game, 0, scout, false),
        None
    );
    assert_eq!(serde_json::to_vec(&game).unwrap(), before);
}

#[test]
fn charting_a_held_goal_still_retires_only_that_units_commitment() {
    let (mut game, scout, goal) = fixture(MapTopology::Planet);
    let other = game.spawn_unit("scout", 0, goal);
    let mut ai = BasicAi::new();
    ai.enable_explore_commit();
    ai.explore_goal
        .borrow_mut()
        .insert(scout, (goal, game.turn));
    ai.explore_goal
        .borrow_mut()
        .insert(other, (goal, game.turn));
    let before = serde_json::to_vec(&game).unwrap();
    assert_eq!(ai.exploration_goal(&game, 0, scout, false), None);
    assert!(!ai.explore_goal.borrow().contains_key(&scout));
    assert_eq!(
        ai.explore_goal.borrow().get(&other),
        Some(&(goal, game.turn))
    );
    assert_eq!(serde_json::to_vec(&game).unwrap(), before);
}

#[test]
fn a_still_unknown_held_goal_keeps_its_original_commitment_stamp() {
    let (mut game, scout, goal) = fixture(MapTopology::Planet);
    game.players[0].explored.remove(&goal);
    let mut ai = BasicAi::new();
    ai.enable_explore_commit();
    ai.explore_goal
        .borrow_mut()
        .insert(scout, (goal, game.turn));
    game.turn += 1;
    assert_eq!(ai.exploration_goal(&game, 0, scout, false), Some(goal));
    assert_eq!(
        ai.explore_goal.borrow().get(&scout),
        Some(&(goal, game.turn - 1))
    );
}

#[test]
fn partial_coverage_with_only_impassable_unknown_ground_still_returns_none() {
    let (mut game, scout, unknown) = fixture(MapTopology::Planet);
    game.players[0].explored.remove(&unknown);
    game.map.tiles.get_mut(&unknown).unwrap().terrain = crate::name!("mountain");
    assert!(!game.unit_can_traverse(scout, unknown));
    assert_eq!(
        BasicAi::new().exploration_goal(&game, 0, scout, false),
        None
    );
}

#[test]
fn the_fast_exit_matches_the_original_ring_path_bookkeeping_and_policy_reads() {
    let (game, scout, goal) = fixture(MapTopology::Planet);
    // This extra position cannot be a traversal candidate, but makes the
    // exact-coverage guard decline so the complete original ring path runs.
    let mut fallback = game.clone();
    fallback.players[0].explored.insert((999_999, -999_999));
    let mut fast_ai = BasicAi::new();
    fast_ai.enable_explore_dead_targets();
    fast_ai.enable_explore_commit();
    fast_ai
        .explore_goal
        .borrow_mut()
        .insert(scout, (goal, game.turn));
    let fallback_ai = fast_ai.clone();
    let fast_before = serde_json::to_vec(&game).unwrap();
    let fallback_before = serde_json::to_vec(&fallback).unwrap();
    let _fast_memo = game.query_memo();
    let _fallback_memo = fallback.query_memo();
    let fast_trace = game.trace_policy_reads(0);
    let fallback_trace = fallback.trace_policy_reads(0);
    assert_eq!(fast_ai.exploration_goal(&game, 0, scout, false), None);
    assert_eq!(
        fallback_ai.exploration_goal(&fallback, 0, scout, false),
        None
    );
    assert_eq!(
        *fast_ai.explore_goal.borrow(),
        *fallback_ai.explore_goal.borrow()
    );
    assert_eq!(
        *fast_ai.explore_dead.borrow(),
        *fallback_ai.explore_dead.borrow()
    );
    let fast_reads = fast_trace.finish();
    let fallback_reads = fallback_trace.finish();
    assert_eq!(format!("{fast_reads:?}"), format!("{fallback_reads:?}"));
    assert_eq!(fast_reads.is_opaque(), fallback_reads.is_opaque());
    for &card in game.rules.policies.keys() {
        assert_eq!(
            fast_reads.card_is_inert(&game.rules, card),
            fallback_reads.card_is_inert(&fallback.rules, card)
        );
    }
    assert_eq!(serde_json::to_vec(&game).unwrap(), fast_before);
    assert_eq!(serde_json::to_vec(&fallback).unwrap(), fallback_before);
}
