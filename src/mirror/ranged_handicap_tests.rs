use super::*;

fn fixture() -> (Snapshot, StateSnapshot) {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 100,
        width: 20,
        height: 20,
        chunk: 1,
        plots: (0..20)
            .flat_map(|x| {
                (0..20).map(move |y| {
                    serde_json::from_value(
                        serde_json::json!({"x":x,"y":y,"t":"TERRAIN_GRASS","vis":true}),
                    )
                    .unwrap()
                })
            })
            .collect(),
    }]);
    let mut state: StateSnapshot = serde_json::from_value(serde_json::json!({
        "turn":100,
        "units":[{"id":10,"kind":"UNIT_ARCHER","combat":15,"ranged":25,"hp":100,"x":4,"y":4}],
        "rivals":[{"player":3,"civ":"CIVILIZATION_ROME","leader":"LEADER_TRAJAN",
            "at_war":true,"diplomatic_state":"DIPLO_STATE_WAR","visibility":0,"their_visibility_on_us":0,
            "units":[{"id":30,"kind":"UNIT_FIELD_CANNON","combat":50,"ranged":60,"hp":100,"x":5,"y":4}]}]
    })).unwrap();
    state.seat.civ = "CIVILIZATION_INDONESIA".into();
    state.seat.leader = "LEADER_GITARJA".into();
    state.seat.difficulty = "DIFFICULTY_DEITY".into();
    state.seat.speed = "GAMESPEED_ONLINE".into();
    (snapshot, state)
}

fn assert_native_base_is_not_a_total(game: &crate::game::Game, bonus: f64) {
    let ours = game
        .units
        .values()
        .find(|u| u.owner == 0 && u.kind == "archer")
        .unwrap();
    let enemy = game
        .units
        .values()
        .find(|u| u.owner == 1 && u.kind == "field_cannon")
        .unwrap();
    assert!(game.is_human_seat(0));
    assert_eq!(game.rules.units[enemy.kind].ranged_strength, 60.0);
    assert_eq!(game.unit_ranged_strength(ours), 25.0);
    assert_eq!(game.unit_ranged_strength(enemy), 60.0 + bonus);
    assert_eq!(
        game.ranged_strike_strengths(enemy.id, ours.id, ours.pos)
            .unwrap()
            .0,
        60.0 + bonus
    );
}

#[test]
fn ranged_handicap_native_base_stats_and_difficulty_cross_rebuild_sync_and_clones() {
    let (snapshot, mut state) = fixture();
    let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    assert_native_base_is_not_a_total(&rebuilt.game, 4.0);
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    assert_native_base_is_not_a_total(&mirror.game, 4.0);
    assert_native_base_is_not_a_total(&mirror.game.clone(), 4.0);
    state.turn += 1;
    mirror.sync(&snapshot, &state, 0);
    assert_native_base_is_not_a_total(&mirror.game, 4.0);
    // Difficulty is a REBUILD-only game constant, not a per-turn reading.
    state.seat.difficulty = "DIFFICULTY_PRINCE".into();
    let prince = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    assert_native_base_is_not_a_total(&prince.game, 0.0);
}
