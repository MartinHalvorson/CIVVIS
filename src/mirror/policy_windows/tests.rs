use super::*;

fn snapshot() -> Snapshot {
    Snapshot::from_chunks(&[TilesChunk {
        turn: 40,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![],
    }])
}

#[test]
fn native_windows_reach_fresh_and_persistent_mirrors_and_clear_on_missing_readings() {
    let snapshot = snapshot();
    let mut state: StateSnapshot = serde_json::from_value(serde_json::json!({
        "turn":40, "policy_change":{"currency":"gold","made":false,"unlock_cost":25}
    }))
    .unwrap();
    let mut mirror = LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
    assert!(mirror.game.host_policy_change_blocked(0));
    assert!(!mirror.game.host_policy_change_blocked(1));
    state.policy_change.as_mut().unwrap().unlock_cost = Some(0.0);
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.host_policy_change_window(0), Some(true));
    state.turn += 1;
    state.policy_change.as_mut().unwrap().made = Some(true);
    mirror.sync(&snapshot, &state, 0);
    assert!(mirror.game.host_policy_change_blocked(0));
    state.policy_change = None;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.host_policy_change_window(0), None);
}

#[test]
fn partial_negative_and_nonfinite_observations_never_confirm_a_free_window() {
    let mut game = crate::game::Game::new_full(2, 14, 14, 10_033_878, 500, 0, false);
    let mut state = StateSnapshot {
        turn: game.turn,
        ..Default::default()
    };
    for (cost, made) in [
        (None, None),
        (None, Some(false)),
        (Some(0.0), None),
        (Some(-1.0), Some(false)),
        (Some(f64::NAN), Some(false)),
        (Some(f64::INFINITY), Some(false)),
        (Some(0.0), Some(true)),
        (Some(115.0), Some(false)),
    ] {
        state.policy_change = Some(StatePolicyChange {
            unlock_cost: cost,
            made,
        });
        apply(&mut game, &state);
        assert!(
            game.host_policy_change_blocked(0),
            "cost={cost:?}, made={made:?}"
        );
    }
    state.policy_change = Some(StatePolicyChange {
        unlock_cost: Some(0.0),
        made: Some(false),
    });
    apply(&mut game, &state);
    assert_eq!(game.host_policy_change_window(0), Some(true));
}
