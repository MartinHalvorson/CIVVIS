use super::super::*;

fn fixture() -> (Snapshot, StateSnapshot) {
    let plot =
        serde_json::from_str(r#"{"x":5,"y":5,"t":"TERRAIN_GRASS","o":0,"w":false,"i":false}"#)
            .unwrap();
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 95,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![plot],
    }]);
    let state = serde_json::from_str(
        r#"{
        "turn":95,"founded_religion":"RELIGION_BUDDHISM",
        "holy_city_id":65536,"inquisition_launched":false,
        "cities":[{"id":65536,"name":"Bogotá","x":5,"y":5,"pop":6}]
    }"#,
    )
    .unwrap();
    (snapshot, state)
}

#[test]
fn holy_city_identity_maps_after_cities_exist_on_rebuild_and_sync() {
    let (snapshot, mut state) = fixture();
    let mut mirror = LiveMirror::new(&snapshot, &state, 2, 1, 300, 0);
    assert_eq!(
        mirror.game.players[0].holy_city,
        mirror.cid_of.get(&65536).copied()
    );
    assert!(mirror.game.players[0].holy_city.is_some());
    state.cities[0].id = 131072;
    state.holy_city_id = Some(131072);
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(
        mirror.game.players[0].holy_city,
        mirror.cid_of.get(&131072).copied()
    );
    state.holy_city_id = Some(999999);
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.players[0].holy_city, None);
}

#[test]
fn observed_inquisition_overrides_planning_memory_in_both_directions() {
    let (snapshot, mut state) = fixture();
    let mut mirror = LiveMirror::new(&snapshot, &state, 2, 1, 300, 0);
    assert_eq!(mirror.game.players[0].counters.get("inquisition"), None);
    state.inquisition_launched = Some(true);
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.players[0].counters.get("inquisition"), Some(&1));
    let rebuilt = LiveMirror::new(&snapshot, &state, 2, 1, 300, 0);
    assert_eq!(
        rebuilt.game.players[0].counters.get("inquisition"),
        Some(&1)
    );
    state.inquisition_launched = Some(false);
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.players[0].counters.get("inquisition"), None);
}

#[test]
fn older_observations_do_not_invent_a_holy_city_or_an_inquisition() {
    let (snapshot, mut state) = fixture();
    state.holy_city_id = None;
    state.inquisition_launched = None;
    let mirror = LiveMirror::new(&snapshot, &state, 2, 1, 300, 0);
    assert_eq!(mirror.game.players[0].holy_city, None);
    assert_eq!(mirror.game.players[0].counters.get("inquisition"), None);
}
