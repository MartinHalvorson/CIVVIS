use super::*;

fn fixture(maximum: serde_json::Value) -> (Snapshot, StateSnapshot) {
    let state = state_from_json(
        &serde_json::json!({
            "turn":158,
            "cities":[{"id":1,"name":"Bogotá","x":3,"y":3,"pop":5,"capital":true}],
            "rivals":[{
                "player":42,"civ":"CIVILIZATION_SUMERIA","at_war":true,
                "cities":[{"id":1,"name":"Uruk","x":8,"y":8,"pop":5,"capital":true}],
                "units":[{"id":3997696,"kind":"UNIT_CUIRASSIER","x":7,"y":8,
                    "hp":100,"moves":0,"max_moves":maximum}]
            }],
            "minors":[{
                "player":6,"civ":"CIVILIZATION_ZANZIBAR",
                "cities":[{"id":1,"name":"Zanzibar","x":12,"y":12,"pop":3}],
                "units":[{"id":131072,"kind":"UNIT_CUIRASSIER","x":11,"y":12,
                    "hp":100,"moves":0,"max_moves":maximum}]
            }]
        })
        .to_string(),
    )
    .unwrap();
    assert!(state.schema_gaps.is_empty(), "{:?}", state.schema_gaps);
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 158,
        width: 20,
        height: 20,
        chunk: 1,
        plots: [(3, 3), (8, 8), (7, 8), (12, 12), (11, 12)]
            .into_iter()
            .map(|(x, y)| plot(x, y, "TERRAIN_GRASS"))
            .collect(),
    }]);
    (snapshot, state)
}

fn assert_allowance(live: &LiveMirror, expected: f64) {
    for host in [3997696, 131072] {
        let uid = live.foreign_uid_of[&host];
        assert_ne!(live.game.units[&uid].owner, 0);
        assert_eq!(live.game.unit_max_moves(uid), expected, "host {host}");
    }
}

#[test]
fn visible_foreign_allowance_rebuilds_and_refreshes_both_rosters() {
    // Six is synthetic API data, not a claim about the archived Cuirassier.
    let (snapshot, state) = fixture(serde_json::json!(6.0));
    assert_eq!(state.rivals[0].units[0].moves, 0.0);
    assert_eq!(state.minors[0].units[0].moves, 0.0);
    let mut live = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    assert_allowance(&live, 6.0);
    let (_, changed) = fixture(serde_json::json!(2.5));
    live.sync(&snapshot, &changed, 0);
    assert_allowance(&live, 2.5);
    let mut next_turn = state;
    next_turn.turn += 1;
    live.sync(&snapshot, &next_turn, 0);
    assert_allowance(&live, 6.0);
}

#[test]
fn absent_or_invalid_foreign_allowance_clears_stale_host_values() {
    let (snapshot, known) = fixture(serde_json::json!(6.0));
    let mut live = LiveMirror::new(&snapshot, &known, 4, 1, 250, 0);
    let (_, missing) = fixture(serde_json::Value::Null);
    let fallback = LiveMirror::new(&snapshot, &missing, 4, 1, 250, 0);
    let normal = fallback
        .game
        .unit_max_moves(fallback.foreign_uid_of[&3997696]);
    assert!(normal > 0.0 && normal != 6.0);
    for value in [
        serde_json::Value::Null,
        serde_json::json!(0),
        serde_json::json!(-1),
    ] {
        live.sync(&snapshot, &known, 0);
        assert_allowance(&live, 6.0);
        let (_, absent) = fixture(value);
        live.sync(&snapshot, &absent, 0);
        assert_allowance(&live, normal);
    }
}
