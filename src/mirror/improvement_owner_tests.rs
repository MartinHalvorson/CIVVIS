use super::*;

#[test]
fn a_native_airstrip_owner_does_not_need_a_city_to_own_its_tile() {
    let state = state_from_json(
        r#"{
            "turn":155,
            "seat":{"local_player":1,"players":3,"speed":"GAMESPEED_ONLINE"},
            "cities":[{"id":77,"name":"Base","x":2,"y":2,"pop":8,"capital":true}],
            "units":[{"id":900,"kind":"UNIT_BOMBER","x":2,"y":2,"hp":100,"moves":11,"max_moves":11,"attacks_remaining":1}],
            "rivals":[{"player":0,"civ":"CIVILIZATION_ROME","cities":[{"id":88,"name":"Other","x":17,"y":8,"pop":4}]}]
        }"#,
    )
    .unwrap();
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
                value["io"] = serde_json::json!(1);
            }
            plots.push(serde_json::from_value::<Plot>(value).unwrap());
        }
    }
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 155,
        width: 20,
        height: 12,
        chunk: 1,
        plots,
    }]);
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
