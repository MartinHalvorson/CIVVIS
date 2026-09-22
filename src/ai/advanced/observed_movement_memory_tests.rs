use super::*;

fn fixture() -> (AdvancedAi, Game, u32) {
    let mut g = Game::new_full(2, 28, 18, 41, 30, 0, false);
    for tile in g.map.tiles.values_mut() {
        tile.terrain = "grassland".into();
        tile.feature = None;
        tile.hills = false;
    }
    g.units.clear();
    let uid = g.spawn_unit("warrior", 0, (5, 5));
    g.units.get_mut(&uid).unwrap().moves_left = 8.0;
    let mut ai = AdvancedAi::new();
    ai.base.recorded_tactical_step = true;
    ai.base.whole_turn_backtrack_guard = true;
    ai.base.move_refusal_break = true;
    (ai, g, uid)
}

#[test]
fn discarded_projected_walk_does_not_block_the_real_first_step() {
    let (mut ai, mut g, uid) = fixture();
    let before = ai.observed_movement_memory();
    let mut view = g.speculative_clone();
    assert!(ai.base.path_move(&mut view, 0, uid, (6, 5)));
    assert!(ai.base.path_move(&mut view, 0, uid, (7, 5)));
    ai.reconcile_observed_movement(&g, before, &[]);
    assert!(
        ai.base.path_move(&mut g, 0, uid, (6, 5)),
        "a projected step was mistaken for an executed step"
    );
}

#[test]
fn partial_walk_preserves_real_steps_but_releases_the_unexecuted_tail() {
    let (mut ai, mut g, uid) = fixture();
    let before = ai.observed_movement_memory();
    let mut view = g.speculative_clone();
    for to in [(6, 5), (7, 5), (8, 5)] {
        assert!(ai.base.path_move(&mut view, 0, uid, to));
    }
    g.apply(
        0,
        &Action::Move {
            unit: uid,
            to: (6, 5),
        },
    )
    .unwrap();
    ai.reconcile_observed_movement(&g, before, &[(uid, (5, 5), (6, 5))]);
    assert!(
        !ai.base.path_move(&mut g, 0, uid, (5, 5)),
        "the real reversal guard must remain"
    );
    assert!(
        ai.base.path_move(&mut g, 0, uid, (7, 5)),
        "an unexecuted tail blocked continuation"
    );
}

#[test]
fn unissued_walks_cannot_accumulate_host_refusal_strikes() {
    let (mut ai, mut g, uid) = fixture();
    for _ in 0..3 {
        ai.base.begin_movement_turn(&g, 0);
        let before = ai.observed_movement_memory();
        let mut view = g.speculative_clone();
        assert!(ai.base.path_move(&mut view, 0, uid, (6, 5)));
        ai.reconcile_observed_movement(&g, before, &[]);
        g.turn += 1;
    }
    ai.base.begin_movement_turn(&g, 0);
    assert!(
        ai.base.path_move(&mut g, 0, uid, (6, 5)),
        "never-issued movement acquired a refusal block"
    );
}

#[test]
fn a_later_cancelled_frame_keeps_the_earlier_executed_history() {
    let (mut ai, mut g, uid) = fixture();
    assert!(ai.base.path_move(&mut g, 0, uid, (6, 5)));
    let before = ai.observed_movement_memory();
    let mut view = g.speculative_clone();
    assert!(ai.base.path_move(&mut view, 0, uid, (7, 5)));
    assert!(ai.base.path_move(&mut view, 0, uid, (8, 5)));
    ai.reconcile_observed_movement(&g, before, &[]);
    assert!(!ai.base.path_move(&mut g, 0, uid, (5, 5)));
    assert!(ai.base.path_move(&mut g, 0, uid, (7, 5)));
}
