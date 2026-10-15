use super::*;

fn fixture(owner: Option<i32>, turn: u32) -> (Snapshot, StateSnapshot) {
    let mut state = state_from_json(
        r#"{
            "turn":155,
            "seat":{"local_player":1,"players":3,"speed":"GAMESPEED_ONLINE"},
            "cities":[{"id":77,"name":"Base","x":2,"y":2,"pop":8,"capital":true}],
            "units":[{"id":900,"kind":"UNIT_BOMBER","x":2,"y":2,"hp":100,"moves":11,"max_moves":11,"attacks_remaining":1}],
            "rivals":[{"player":0,"civ":"CIVILIZATION_ROME","cities":[{"id":88,"name":"Other","x":17,"y":8,"pop":4}]}]
        }"#,
    )
    .unwrap();
    state.turn = turn;
    assert_eq!(host_major_seat_map(&state, 3)[&1], 0);
    let mut plots = Vec::new();
    for y in 0..12 {
        for x in 0..20 {
            let mut value = serde_json::json!({
                "x":x,"y":y,"t":"TERRAIN_GRASS","o":-1,"vis":true
            });
            if (x, y) == (2, 2) {
                value["o"] = serde_json::json!(1);
            }
            if (x, y) == (17, 8) {
                value["o"] = serde_json::json!(0);
            }
            if (x, y) == (10, 2) {
                value["im"] = serde_json::json!("IMPROVEMENT_AIRSTRIP");
                if let Some(owner) = owner {
                    value["io"] = serde_json::json!(owner);
                }
            }
            plots.push(serde_json::from_value::<Plot>(value).unwrap());
        }
    }
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn,
        width: 20,
        height: 12,
        chunk: 1,
        plots,
    }]);
    (snapshot, state)
}

#[test]
fn a_native_airstrip_owner_does_not_need_a_city_to_own_its_tile() {
    let (snapshot, state) = fixture(Some(1), 155);
    let live = LiveMirror::new(&snapshot, &state, 3, 4064, 650, 0);
    let target = crate::hex::offset_to_axial(10, 2);
    assert_eq!(
        live.game.map.tiles[&target].improvement.as_deref(),
        Some("airstrip")
    );
    assert!(live.game.map.tiles[&target].owner_city.is_none());
    assert_eq!(
        live.game.air_capacity_at(0, target),
        3,
        "native player 1 is our modeled seat 0; improvement ownership is separate"
    );
}

#[test]
fn native_foreign_owner_is_translated_instead_of_used_as_a_model_index() {
    let (snapshot, state) = fixture(Some(0), 155);
    let live = LiveMirror::new(&snapshot, &state, 3, 4064, 650, 0);
    let target = crate::hex::offset_to_axial(10, 2);
    assert_eq!(live.game.map.tiles[&target].improvement_owner, Some(1));
    assert_eq!(live.game.air_capacity_at(0, target), 0);
    assert_eq!(live.game.air_capacity_at(1, target), 3);
}

#[test]
fn legacy_negative_and_unmapped_native_owners_cannot_invent_a_neutral_base() {
    for owner in [None, Some(-1), Some(99)] {
        let (snapshot, state) = fixture(owner, 155);
        let live = LiveMirror::new(&snapshot, &state, 3, 4064, 650, 0);
        let target = crate::hex::offset_to_axial(10, 2);
        assert_eq!(live.game.map.tiles[&target].improvement_owner, None);
        assert_eq!(live.game.air_capacity_at(0, target), 0);
        assert_eq!(live.game.air_capacity_at(1, target), 0);
    }
}

#[test]
fn native_owner_changes_pillage_and_removal_replace_prior_base_capability() {
    let (snapshot, state) = fixture(Some(1), 155);
    let mut live = LiveMirror::new(&snapshot, &state, 3, 4064, 650, 0);
    let target = crate::hex::offset_to_axial(10, 2);
    assert_eq!(live.game.air_capacity_at(0, target), 3);
    let (changed, state) = fixture(Some(0), 156);
    live.sync(&changed, &state, 0);
    assert_eq!(live.game.air_capacity_at(0, target), 0);
    assert_eq!(live.game.air_capacity_at(1, target), 3);
    let (mut pillaged, state) = fixture(Some(0), 157);
    pillaged.revealed.get_mut(&(10, 2)).unwrap().p = true;
    live.sync(&pillaged, &state, 0);
    assert_eq!(live.game.air_capacity_at(1, target), 0);
    let (mut removed, state) = fixture(Some(0), 158);
    removed.revealed.get_mut(&(10, 2)).unwrap().im = None;
    live.sync(&removed, &state, 0);
    assert_eq!(live.game.map.tiles[&target].improvement_owner, None);
    assert_eq!(live.game.air_capacity_at(1, target), 0);
}

#[test]
fn an_improvement_request_cannot_transfer_its_predecessors_observed_owner() {
    let (mut snapshot, _) = fixture(Some(1), 155);
    assert_eq!(snapshot.plot((10, 2)).unwrap().io, Some(1));
    assert!(snapshot.set_improvement((10, 2), "IMPROVEMENT_AIRSTRIP"));
    assert_eq!(snapshot.plot((10, 2)).unwrap().io, Some(1));
    assert!(snapshot.set_improvement((10, 2), "IMPROVEMENT_FARM"));
    assert_eq!(snapshot.plot((10, 2)).unwrap().io, None);
}
