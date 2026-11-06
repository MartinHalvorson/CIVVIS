use super::*;

fn fixture() -> (Snapshot, StateSnapshot) {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 91,
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
    let state = StateSnapshot {
        turn: 91,
        rivals: vec![StateRival {
            player: 3,
            civ: "CIVILIZATION_MONGOLIA".into(),
            leader: "LEADER_KUBLAI_KHAN_MONGOLIA".into(),
            at_war: true,
            diplomatic_state: Some("DIPLO_STATE_WAR".into()),
            visibility: Some(0),
            their_visibility_on_us: Some(2),
            ..StateRival::default()
        }],
        ..StateSnapshot::default()
    };
    (snapshot, state)
}

fn assert_kublai_estimates(game: &mut crate::game::Game) {
    assert_eq!(game.players[1].civ, "Mongolia");
    assert_eq!(game.diplomatic_visibility(0, 1), 0.0);
    assert_eq!(game.diplomatic_visibility(1, 0), 2.0);
    let ours = game.spawn_test_unit("archer", 0, (4, 4));
    let theirs = game.spawn_test_unit("keshig", 1, (5, 4));
    let mut roman = game.clone();
    roman.players[1].civ = "Rome".into();
    assert_eq!(
        game.unit_strength(&game.units[&theirs], false),
        roman.unit_strength(&roman.units[&theirs], false)
    );
    assert_eq!(
        game.unit_ranged_strength(&game.units[&theirs]),
        roman.unit_ranged_strength(&roman.units[&theirs])
    );
    let baseline = roman.ranged_strike_strengths(theirs, ours, (4, 4)).unwrap();
    let observed = game.ranged_strike_strengths(theirs, ours, (4, 4)).unwrap();
    assert_eq!(
        observed,
        (baseline.0 + 6.0, baseline.1),
        "two levels: Mongolia +12 versus ordinary +6"
    );
    game.remove_unit(ours);
    game.remove_unit(theirs);
}

#[test]
fn mongolia_combat_host_identity_and_visibility_reach_rebuild_sync_and_clones() {
    let (snapshot, mut state) = fixture();
    let mut rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    assert_kublai_estimates(&mut rebuilt.game);
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    assert_kublai_estimates(&mut mirror.game);
    assert_kublai_estimates(&mut mirror.game.clone());

    state.turn += 1;
    state.rivals[0].leader = "LEADER_GENGHIS_KHAN".into();
    mirror.sync(&snapshot, &state, 0);
    let id = mirror.game.spawn_test_unit("keshig", 1, (5, 4));
    let genghis = mirror.game.unit_ranged_strength(&mirror.game.units[&id]);
    state.turn += 1;
    state.rivals[0].leader = "LEADER_KUBLAI_KHAN_MONGOLIA".into();
    mirror.sync(&snapshot, &state, 0);
    let id = mirror.game.spawn_test_unit("keshig", 1, (5, 4));
    assert_eq!(
        mirror.game.unit_ranged_strength(&mirror.game.units[&id]),
        genghis - 3.0
    );
}
