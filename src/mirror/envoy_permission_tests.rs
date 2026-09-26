use super::*;

fn fixture() -> (Snapshot, StateSnapshot) {
    let state = state_from_json(
        r#"{"turn":165,"envoys_free":1,
        "cities":[{"id":1,"name":"Bogota","x":1,"y":1,"pop":5,"capital":true}],
        "minors":[{"player":42,"civ":"CIVILIZATION_TARUGA","at_war":true,
          "can_send_envoy":true,"envoys":10,"most_envoys":11,"suzerain":-1,
          "cities":[{"id":2,"name":"Taruga","x":5,"y":5,"pop":5,"capital":true}]}]}"#,
    )
    .unwrap();
    assert!(state.schema_gaps.is_empty(), "{:?}", state.schema_gaps);
    let plots = [(1, 1), (5, 5)]
        .into_iter()
        .map(|(x, y)| {
            serde_json::from_value(serde_json::json!({
                "x":x,"y":y,"t":"TERRAIN_GRASS"
            }))
            .unwrap()
        })
        .collect();
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: state.turn,
        width: 8,
        height: 8,
        chunk: 1,
        plots,
    }]);
    (snapshot, state)
}

#[test]
fn host_envoy_permissions_rebuild_and_refresh_on_same_turn_sync() {
    let (snapshot, mut state) = fixture();
    let mut mirror = LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
    let minor = mirror
        .game
        .players
        .iter()
        .find(|p| p.civ == "Taruga")
        .unwrap()
        .id;
    assert_ne!(minor, 42, "permissions must translate host seats");
    assert!(mirror.game.is_at_war(0, minor));
    assert!(mirror.game.can_send_envoy(0, minor));
    state.minors[0].can_send_envoy = Some(false);
    state.minors[0].at_war = false;
    mirror.sync(&snapshot, &state, 0);
    assert!(!mirror.game.is_at_war(0, minor));
    assert!(!mirror.game.can_send_envoy(0, minor));
    state.minors[0].can_send_envoy = None;
    mirror.sync(&snapshot, &state, 0);
    assert!(mirror.game.can_send_envoy(0, minor));
    assert!(!mirror.game.host_envoy_permissions.contains_key(&(0, minor)));
}

#[test]
fn host_envoy_permissions_do_not_outlive_an_omitted_minor() {
    let (snapshot, mut state) = fixture();
    let mut mirror = LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
    assert_eq!(mirror.game.host_envoy_permissions.len(), 1);
    state.minors.clear();
    mirror.sync(&snapshot, &state, 0);
    assert!(mirror.game.host_envoy_permissions.is_empty());
}

#[test]
fn old_exports_keep_the_existing_wartime_rule() {
    let (snapshot, mut state) = fixture();
    state.minors[0].can_send_envoy = None;
    let mirror = LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
    let minor = mirror
        .game
        .players
        .iter()
        .find(|p| p.civ == "Taruga")
        .unwrap()
        .id;
    assert!(mirror.game.is_at_war(0, minor));
    assert!(!mirror.game.can_send_envoy(0, minor));
    assert!(mirror.game.host_envoy_permissions.is_empty());
}
