use super::*;

fn fixture(pillaged: Option<bool>) -> (Snapshot, StateSnapshot) {
    let mut plots = Vec::new();
    for x in 0..20 {
        for y in 0..20 {
            let mut row = serde_json::json!({"x":x,"y":y,"t":"TERRAIN_GRASS","vis":true});
            if (x, y) == (10, 10) || (x, y) == (11, 10) {
                row["o"] = 3.into();
                row["d"] = if x == 10 {
                    "DISTRICT_CITY_CENTER"
                } else {
                    "DISTRICT_CAMPUS"
                }
                .into();
                row["dc"] = true.into();
                if let Some(value) = pillaged {
                    row["p"] = value.into();
                }
            }
            plots.push(serde_json::from_value(row).unwrap());
        }
    }
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 200,
        width: 20,
        height: 20,
        chunk: 1,
        plots,
    }]);
    let city = |id, x, y| StateCity {
        id,
        x,
        y,
        pop: 5,
        loyalty: 100.0,
        ..StateCity::default()
    };
    let state = StateSnapshot {
        turn: 200,
        cities: vec![city(1, 2, 2)],
        rivals: vec![StateRival {
            player: 3,
            civ: "CIVILIZATION_SCOTLAND".into(),
            at_war: true,
            cities: vec![city(3, 10, 10)],
            ..StateRival::default()
        }],
        ..StateSnapshot::default()
    };
    (snapshot, state)
}

#[test]
fn observed_enemy_district_pillage_removes_the_bomber_mission() {
    for pillaged in [false, true] {
        let (snapshot, state) = fixture(Some(pillaged));
        let mut mirror = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
        let target = crate::hex::offset_to_axial(11, 10);
        assert_eq!(mirror.game.map.tiles[&target].pillaged, pillaged);
        let unit = mirror
            .game
            .spawn_test_unit("jet_bomber", 0, crate::hex::offset_to_axial(8, 10));
        let action = crate::game::Action::AirPillage { unit, target };
        assert_eq!(
            mirror
                .game
                .legal_doctrine_actions(0, unit)
                .contains(&action),
            !pillaged
        );
    }
}

#[test]
fn enemy_district_repair_and_legacy_exports_restore_the_existing_state() {
    let (snapshot, state) = fixture(Some(true));
    let mut mirror = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    let target = crate::hex::offset_to_axial(11, 10);
    assert!(mirror.game.map.tiles[&target].pillaged);
    for value in [Some(false), None] {
        let (snapshot, _) = fixture(value);
        apply_foreign_infrastructure(&mut mirror.game, &snapshot);
        assert!(!mirror.game.map.tiles[&target].pillaged);
        assert_eq!(
            mirror.game.map.tiles[&target].district.as_deref(),
            Some("campus")
        );
    }
}
