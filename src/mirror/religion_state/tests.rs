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
        plots: vec![
            plot,
            serde_json::from_str(r#"{"x":9,"y":5,"t":"TERRAIN_GRASS","o":1,"w":false,"i":false}"#)
                .unwrap(),
        ],
    }]);
    let state = serde_json::from_str(
        r#"{
        "turn":95,"founded_religion":"RELIGION_BUDDHISM",
        "holy_city":[5,5],"inquisition_launched":false,
        "cities":[{"id":65536,"name":"Bogotá","x":5,"y":5,"pop":6}]
    }"#,
    )
    .unwrap();
    (snapshot, state)
}

#[test]
fn holy_city_identity_maps_after_cities_exist_on_rebuild_and_sync() {
    let (snapshot, mut state) = fixture();
    // Native IDs repeat across owners. The Holy City must not resolve to the
    // rival's first city simply because it was inserted into a map last.
    state.rivals.push(StateRival {
        player: 1,
        cities: vec![StateCity {
            id: 65536,
            x: 9,
            y: 5,
            pop: 4,
            ..StateCity::default()
        }],
        ..StateRival::default()
    });
    let mut mirror = LiveMirror::new(&snapshot, &state, 2, 1, 300, 0);
    assert_eq!(
        mirror.game.players[0].holy_city,
        mirror.cid_of.get(&65536).copied()
    );
    assert!(mirror.game.players[0].holy_city.is_some());
    state.cities[0].id = 131072;
    state.holy_city = Some([5, 5]);
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(
        mirror.game.players[0].holy_city,
        mirror.cid_of.get(&131072).copied()
    );
    state.holy_city = Some([19, 19]);
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
    state.holy_city = None;
    state.inquisition_launched = None;
    let mirror = LiveMirror::new(&snapshot, &state, 2, 1, 300, 0);
    assert_eq!(mirror.game.players[0].holy_city, None);
    assert_eq!(mirror.game.players[0].counters.get("inquisition"), None);
}
