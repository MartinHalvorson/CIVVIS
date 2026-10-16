use super::*;
use crate::game::Game;

fn uncached(players: usize, width: i32, height: i32, seed: u64, minors: usize) -> Game {
    let mut game = Game::new(
        players.max(2),
        width.max(1),
        height.max(1),
        seed,
        500,
        minors,
    );
    Arc::make_mut(&mut game.rules).enable_unknown_terrain();
    game
}

fn assert_same(actual: &Game, expected: &Game) {
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    assert_eq!(actual.rng, expected.rng);
    let mut actual_rng = actual.rng.clone();
    let mut expected_rng = expected.rng.clone();
    for _ in 0..16 {
        assert_eq!(actual_rng.next_u64(), expected_rng.next_u64());
    }
}

#[test]
fn cold_and_warm_constructor_states_match_for_every_key_input() {
    NATIVE_BASE_GAME.with(|cached| *cached.borrow_mut() = None);
    for (players, width, height, seed, minors) in [
        (4, 20, 16, 71, 2),
        (6, 20, 16, 71, 2),
        (4, 22, 16, 71, 2),
        (4, 20, 18, 71, 2),
        (4, 20, 16, 72, 2),
        (4, 20, 16, 71, 3),
        (1, 0, -1, 73, 0),
    ] {
        let expected = uncached(players, width, height, seed, minors);
        let cold = native_base_game(players, width, height, seed, minors);
        let warm = native_base_game(players, width, height, seed, minors);
        assert_same(&cold, &expected);
        assert_same(&warm, &expected);
        assert!(Arc::ptr_eq(&cold.rules, &warm.rules));
    }
}

#[test]
fn mutated_returned_board_cannot_change_the_untouched_template() {
    NATIVE_BASE_GAME.with(|cached| *cached.borrow_mut() = None);
    let expected = uncached(4, 20, 16, 74, 2);
    let mut observed = native_base_game(4, 20, 16, 74, 2);
    let _ = observed.rng.next_u64();
    observed.turn = 81;
    observed.players[0].gold = 1234.0;
    let first_unit = observed.units.keys().next().copied();
    if let Some(uid) = first_unit {
        observed.remove_unit(uid);
    }
    observed.clear_mirror_cities();
    Arc::make_mut(&mut observed.rules)
        .terrains
        .remove("unknown");
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 81,
        width: 20,
        height: 16,
        chunk: 1,
        plots: vec![serde_json::from_str(r#"{"x":6,"y":6,"t":"TERRAIN_GRASS","o":0}"#).unwrap()],
    }]);
    // Restore the mirror marker only on this private rules snapshot before
    // applying observations, then verify the cached constructor is untouched.
    Arc::make_mut(&mut observed.rules).enable_unknown_terrain();
    apply_terrain(&mut observed, &snapshot);
    let warm = native_base_game(4, 20, 16, 74, 2);
    assert_same(&warm, &expected);
    assert!(!Arc::ptr_eq(&observed.rules, &warm.rules));
}

#[test]
fn alternating_setups_replace_the_single_slot_without_carrying_state() {
    let first = native_base_game(4, 20, 16, 75, 2);
    let other = native_base_game(4, 22, 18, 76, 3);
    let first_again = native_base_game(4, 20, 16, 75, 2);
    assert_same(&other, &uncached(4, 22, 18, 76, 3));
    assert_same(&first_again, &first);
    assert!(!Arc::ptr_eq(&first.rules, &first_again.rules));
    NATIVE_BASE_GAME.with(|cached| {
        let cached = cached.borrow();
        let (key, _) = cached.as_ref().unwrap();
        assert_eq!(
            (
                key.players,
                key.width,
                key.height,
                key.seed,
                key.city_states
            ),
            (4, 20, 16, 75, 2)
        );
    });
}

#[test]
fn successive_observed_boards_reapply_all_terrain_without_leaking_revelation() {
    let known = Snapshot::from_chunks(&[TilesChunk {
        turn: 82,
        width: 20,
        height: 16,
        chunk: 1,
        plots: vec![serde_json::from_str(r#"{"x":6,"y":6,"t":"TERRAIN_GRASS","o":0}"#).unwrap()],
    }]);
    let hidden = Snapshot {
        width: 20,
        height: 16,
        ..Snapshot::default()
    };
    let observed = rebuild_game_with_city_states(&known, 4, 77, 2);
    let concealed = rebuild_game_with_city_states(&hidden, 4, 77, 2);
    let pos = crate::hex::offset_to_axial(6, 6);
    assert_eq!(observed.map.get(pos).unwrap().terrain, "grassland");
    assert_eq!(concealed.map.get(pos).unwrap().terrain, "unknown");
    let mut expected = uncached(4, 20, 16, 77, 2);
    apply_terrain(&mut expected, &hidden);
    assert_same(&concealed, &expected);
}
