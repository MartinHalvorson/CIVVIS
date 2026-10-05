use super::*;
use crate::mirror::{LiveMirror, Snapshot, StateCity, StateRival, StateUnit, TilesChunk};

fn fixture() -> (Snapshot, StateSnapshot) {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 120,
        width: 12,
        height: 12,
        chunk: 1,
        plots: [(3, 3), (5, 5), (6, 5), (7, 5)]
            .into_iter()
            .map(|(x, y)| {
                serde_json::from_value(serde_json::json!({"x":x,"y":y,"t":"TERRAIN_GRASS"}))
                    .unwrap()
            })
            .collect(),
    }]);
    let city = |id, x, y| StateCity {
        id,
        x,
        y,
        pop: 6,
        ..StateCity::default()
    };
    let state = StateSnapshot {
        turn: 120,
        spy_capacity: Some(3),
        cities: vec![city(9, 3, 3)],
        rivals: vec![StateRival {
            player: 1,
            cities: vec![city(9, 5, 5), city(10, 7, 5)],
            ..StateRival::default()
        }],
        units: vec![StateUnit {
            id: 78,
            kind: "UNIT_SPY".to_string(),
            x: 6,
            y: 5,
            level: Some(1),
            maintenance: Some(4.0),
            spy_operation: Some("UNITOPERATION_SPY_DISRUPT_ROCKETRY".to_string()),
            spy_operation_end_turn: Some(128),
            spy_city: Some(StateSpyCity {
                id: 10,
                player: 1,
                x: 7,
                y: 5,
            }),
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    };
    (snapshot, state)
}

#[test]
fn district_operation_keeps_its_host_posting_busy_on_rebuild_and_sync() {
    let (snapshot, state) = fixture();
    let mut live = LiveMirror::new(&snapshot, &state, 2, 1, 650, 0);
    let check = |live: &LiveMirror| {
        let uid = live.uid_of[&78];
        let spy = &live.game.spies[&uid];
        let city = live.game.city_at(offset_to_axial(7, 5)).unwrap();
        assert_eq!(spy.city, Some(city));
        let mission = spy.mission.as_ref().unwrap();
        assert_eq!(
            (mission.kind.as_str(), mission.city, mission.ends),
            ("disrupt_rocketry", city, 128)
        );
        assert_eq!(mission.target, offset_to_axial(6, 5));
        assert!(live.game.legal_spy_actions(0, uid).is_empty());
        // The original exact-center resolver cannot represent this posting.
        assert!(live
            .game
            .cities
            .values()
            .all(|city| city.pos != live.game.units[&uid].pos));
    };
    check(&live);
    let mut next = state.clone();
    next.turn = 121;
    live.sync(&snapshot, &next, 0);
    check(&live);
}

#[test]
fn native_city_owner_and_center_prevent_id_aliases_and_unresolved_fallbacks() {
    let (snapshot, mut state) = fixture();
    state.units[0].spy_city = Some(StateSpyCity {
        id: 9,
        player: 1,
        x: 5,
        y: 5,
    });
    let live = LiveMirror::new(&snapshot, &state, 2, 1, 650, 0);
    let uid = live.uid_of[&78];
    let city = live.game.spies[&uid].city.unwrap();
    assert_eq!(
        live.game.cities[&city].owner, 1,
        "same native id9 as our city must not alias"
    );
    for (id, player, x, y) in [(99, 1, 5, 5), (9, 3, 5, 5), (9, 0, 5, 5), (9, 1, 7, 5)] {
        let mut bad = state.clone();
        bad.units[0].x = 3;
        bad.units[0].y = 3;
        bad.units[0].spy_city = Some(StateSpyCity { id, player, x, y });
        let invalid = LiveMirror::new(&snapshot, &bad, 2, 1, 650, 0);
        let uid = invalid.uid_of[&78];
        let spy = &invalid.game.spies[&uid];
        assert!(
            spy.city.is_none(),
            "explicit unresolved native city must not fall back to our center"
        );
        assert_eq!(spy.ready_turn, 128);
        assert!(invalid.game.legal_spy_actions(0, uid).is_empty());
    }
}

#[test]
fn an_idle_spy_resumes_only_the_host_menu_and_a_new_foreign_city_seats_on_sync() {
    let (snapshot, mut state) = fixture();
    let mut initial = state.clone();
    initial.units.clear();
    initial.rivals[0].cities.pop();
    let mut live = LiveMirror::new(&snapshot, &initial, 2, 1, 650, 0);
    state.turn = 121;
    state.units[0].spy_operation = None;
    state.units[0].spy_operation_end_turn = None;
    state.units[0].spy_missions_available =
        Some(vec!["UNITOPERATION_SPY_LISTENING_POST".to_string()]);
    live.sync(&snapshot, &state, 0);
    let uid = live.uid_of[&78];
    let spy = &live.game.spies[&uid];
    assert_eq!(spy.city, live.game.city_at(offset_to_axial(7, 5)));
    assert!(spy.city.is_some() && spy.mission.is_none());
    let missions: Vec<_> = live
        .game
        .legal_spy_actions(0, uid)
        .into_iter()
        .filter_map(|action| {
            if let crate::game::Action::SpyMission { mission, .. } = action {
                Some(mission)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(missions, ["listening_post"]);
}

#[test]
fn older_archives_keep_center_postings_and_unknown_districts_stay_busy() {
    let (snapshot, mut state) = fixture();
    state.units[0].spy_city = None;
    let unknown = LiveMirror::new(&snapshot, &state, 2, 1, 650, 0);
    let uid = unknown.uid_of[&78];
    assert_eq!(unknown.game.spies[&uid].city, None);
    assert_eq!(unknown.game.spies[&uid].ready_turn, 128);
    state.units[0].x = 5;
    let centered = LiveMirror::new(&snapshot, &state, 2, 1, 650, 0);
    let uid = centered.uid_of[&78];
    assert_eq!(
        centered.game.spies[&uid].city,
        centered.game.city_at(offset_to_axial(5, 5))
    );
    assert!(centered.game.legal_spy_actions(0, uid).is_empty());
}

#[test]
fn the_native_spy_city_payload_survives_the_unit_prefilter() {
    let path = std::env::temp_dir().join(format!("civvis-spy-city-{}.jsonl", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    use std::io::Write;
    writeln!(file, "{{\"kind\":\"state\",\"turn\":120,\"units\":[{{\"id\":78,\"kind\":\"UNIT_SPY\",\"x\":6,\"y\":5,\"spy_city\":{{\"id\":10,\"player\":1,\"x\":7,\"y\":5}}}}]}}").unwrap();
    drop(file);
    let parsed = crate::mirror::state_from_events(&path, None).unwrap();
    let city = parsed.units[0].spy_city.as_ref().unwrap();
    assert_eq!((city.id, city.player, city.x, city.y), (10, 1, 7, 5));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn native_player_numbering_is_not_the_mirror_seat_numbering() {
    let (snapshot, mut state) = fixture();
    state.seat.local_player = 1;
    state.seat.players = 2;
    state.rivals[0].player = 0;
    state.units[0].spy_city.as_mut().unwrap().player = 0;
    let live = LiveMirror::new(&snapshot, &state, 2, 1, 650, 0);
    let uid = live.uid_of[&78];
    let cid = live.game.spies[&uid].city.unwrap();
    assert_eq!(
        live.game.cities[&cid].owner, 1,
        "native rival0 is mirrored1 when native local1 is mirrored0"
    );
}

#[test]
#[ignore = "read-only frozen native board; supplied posting is retrospective last-center evidence, not new host payload readback"]
fn inspect_recorded_native_spy_posting() {
    let path = std::env::var("CIVVIS_SPY_POSTING_MANIFEST").unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let prefix = std::path::Path::new(manifest["prefix"].as_str().unwrap());
    let snapshot = crate::mirror::snapshot_from_events(prefix).unwrap();
    let mut state = crate::mirror::state_from_events(prefix, None).unwrap();
    assert_eq!((state.turn, state.frame), (207, 0));
    assert_eq!((state.seat.players, state.seat.max_turns), (4, 650));
    let native = manifest["native_spy"].as_i64().unwrap();
    let idle_native = manifest["native_idle_spy"].as_i64().unwrap();
    let before = LiveMirror::new(&snapshot, &state, 4, 1, 650, 6);
    let uid = before.uid_of[&native];
    let idle = before.uid_of[&idle_native];
    let city = before.game.city_at(offset_to_axial(29, 11)).unwrap();
    let pads = [city].into_iter().collect();
    assert_eq!(before.game.units[&uid].pos, offset_to_axial(31, 8));
    assert!(before.game.spies[&uid].city.is_none());
    assert!(before.game.legal_spy_actions(0, uid).is_empty());
    let bonus_before = crate::ai::AdvancedAi::science_denial_spy_assignment_bonus(
        &before.game,
        0,
        idle,
        &pads,
        city,
    );
    let unit = state
        .units
        .iter_mut()
        .find(|unit| unit.id == native)
        .unwrap();
    assert!(
        unit.spy_city.is_none(),
        "archive must not be relabeled as new host readback"
    );
    unit.spy_city =
        Some(serde_json::from_value(manifest["last_observed_posting"].clone()).unwrap());
    let after = LiveMirror::new(&snapshot, &state, 4, 1, 650, 6);
    let uid_after = after.uid_of[&native];
    let idle_after = after.uid_of[&idle_native];
    let city_after = after.game.city_at(offset_to_axial(29, 11)).unwrap();
    let pads_after = [city_after].into_iter().collect();
    assert_eq!(after.game.spies[&uid_after].city, Some(city_after));
    assert_eq!(
        after.game.spies[&uid_after].mission.as_ref().unwrap().kind,
        "disrupt_rocketry"
    );
    assert!(after.game.legal_spy_actions(0, uid_after).is_empty());
    let bonus_after = crate::ai::AdvancedAi::science_denial_spy_assignment_bonus(
        &after.game,
        0,
        idle_after,
        &pads_after,
        city_after,
    );
    assert!(bonus_before > 0);
    assert_eq!(bonus_after, 0);
    let mut board_before = serde_json::to_value(&before.game).unwrap();
    let mut board_after = serde_json::to_value(&after.game).unwrap();
    board_before.as_object_mut().unwrap().remove("spies");
    board_after.as_object_mut().unwrap().remove("spies");
    assert!(
        board_before == board_after,
        "only the spy posting changes on this frozen reconstruction"
    );
    let result = serde_json::json!({"manifest":manifest,"bonus_before":bonus_before,"bonus_after":bonus_after,
        "spy_before":before.game.spies[&uid],"spy_after":after.game.spies[&uid_after],
        "scope":"Retrospective last native city-center evidence supplied as proposed host payload. This proves mirrored posting/accounting path only; no new native payload, changed complete decider, survival or win-rate proof.",
        "all_non_spy_serialized_game_equal":true,"busy_host_mission_not_retasked":true});
    std::fs::write(
        std::env::var("CIVVIS_SPY_POSTING_OUTPUT").unwrap(),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
}
