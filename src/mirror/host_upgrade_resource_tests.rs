use super::*;

fn observation() -> (Snapshot, StateSnapshot) {
    let state = state_from_json(
        r#"{
        "turn":64,"techs":["TECH_IRON_WORKING"],"gold":1000,
        "strategic_resources":{"RESOURCE_IRON":20},
        "cities":[{"id":77,"name":"Bogotá","x":3,"y":3,"pop":5,"capital":true}],
        "units":[{"id":83,"kind":"UNIT_WARRIOR","x":3,"y":3,"hp":100,"moves":3,
            "upgrade_to":"UNIT_SWORDSMAN","upgrade_cost":25,
            "upgrade_resource":"RESOURCE_IRON","upgrade_resource_cost":10}]
        }"#,
    )
    .unwrap();
    assert!(state.schema_gaps.is_empty(), "{:?}", state.schema_gaps);
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: state.turn,
        width: 52,
        height: 42,
        chunk: 1,
        plots: vec![serde_json::from_str(r#"{"x":3,"y":3,"t":"TERRAIN_GRASS","o":0}"#).unwrap()],
    }]);
    (snapshot, state)
}

#[test]
fn rebuild_and_sync_replace_native_upgrade_material_prices() {
    let (snapshot, mut state) = observation();
    let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    let uid = rebuilt
        .unit_ids
        .iter()
        .find_map(|(uid, native)| (*native == 83).then_some(*uid))
        .unwrap();
    assert_eq!(
        rebuilt.game.unit_gold_upgrade_offer(0, uid).unwrap().2,
        10.0
    );
    let mut live = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let uid = *live.uid_of.get(&83).unwrap();
    state.turn += 1;
    state.units[0].upgrade_resource_cost = Some(7.0);
    live.sync(&snapshot, &state, 0);
    assert_eq!(live.game.unit_gold_upgrade_offer(0, uid).unwrap().2, 7.0);
    state.turn += 1;
    state.units[0].upgrade_resource_cost = Some(0.0);
    state.units[0].upgrade_resource = None;
    live.sync(&snapshot, &state, 0);
    assert_eq!(live.game.unit_gold_upgrade_offer(0, uid).unwrap().2, 0.0);
    state.turn += 1;
    state.units[0].upgrade_resource_cost = None;
    live.sync(&snapshot, &state, 0);
    assert!(live.game.host_unit_facts[&uid]
        .upgrade
        .as_ref()
        .unwrap()
        .resources
        .is_none());
    let unit = &live.game.units[&uid];
    let modeled = live
        .game
        .unit_upgrade_price_in_formation(0, unit.kind, crate::name!("swordsman"), unit.formation)
        .map_or(0.0, |(_, resources)| resources);
    assert_eq!(
        live.game.unit_gold_upgrade_offer(0, uid).unwrap().2,
        modeled,
        "a missing new price must clear the old observation"
    );
}

#[test]
fn invalid_or_unresolved_upgrade_materials_remain_unknown() {
    let (snapshot, mut state) = observation();
    for invalid in [None, Some(-1.0), Some(f64::NAN), Some(f64::INFINITY)] {
        state.units[0].upgrade_resource_cost = invalid;
        let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        let uid = rebuilt
            .unit_ids
            .iter()
            .find_map(|(uid, native)| (*native == 83).then_some(*uid))
            .unwrap();
        assert!(rebuilt.game.host_unit_facts[&uid]
            .upgrade
            .as_ref()
            .unwrap()
            .resources
            .is_none());
    }
    for cost in [0.0, 10.0] {
        state.units[0].upgrade_resource_cost = Some(cost);
        state.units[0].upgrade_resource = Some("RESOURCE_UNKNOWN".into());
        let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        let uid = rebuilt
            .unit_ids
            .iter()
            .find_map(|(uid, native)| (*native == 83).then_some(*uid))
            .unwrap();
        assert!(rebuilt.game.host_unit_facts[&uid]
            .upgrade
            .as_ref()
            .unwrap()
            .resources
            .is_none());
    }
}
