use super::*;

fn fixture(flooded: Option<bool>, submerged: Option<bool>) -> (Snapshot, StateSnapshot) {
    let plots = vec![
        serde_json::from_value(serde_json::json!({
            "x": 6, "y": 6, "t": "TERRAIN_GRASS", "o": 0
        }))
        .unwrap(),
        serde_json::from_value(serde_json::json!({
            "x": 7, "y": 6, "t": "TERRAIN_GRASS", "o": 0,
            "cl": 1, "d": "DISTRICT_CAMPUS", "p": true,
            "flooded": flooded, "submerged": submerged
        }))
        .unwrap(),
    ];
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 203,
        width: 20,
        height: 20,
        chunk: 1,
        plots,
    }]);
    let state = StateSnapshot {
        turn: 203,
        climate: Some(StateClimate {
            level: 6,
            ..StateClimate::default()
        }),
        cities: vec![StateCity {
            id: 1,
            name: "Bogotá".to_string(),
            x: 6,
            y: 6,
            pop: 5,
            capital: true,
            districts: vec![StateDistrict {
                kind: "DISTRICT_CAMPUS".to_string(),
                x: 7,
                y: 6,
                pillaged: true,
                ..StateDistrict::default()
            }],
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    (snapshot, state)
}

fn assert_repair(game: &crate::game::Game, flooded: bool, submerged: bool) {
    let pos = crate::hex::offset_to_axial(7, 6);
    let tile = game.map.get(pos).unwrap();
    assert_eq!((tile.flooded, tile.submerged), (flooded, submerged));
    let city = game.player_city_ids(0)[0];
    let repair = crate::game::Item::Repair {
        repair: crate::name!("district"),
        pos,
    };
    assert_eq!(game.can_produce(0, city, &repair), !flooded && !submerged);
    assert_eq!(
        game.producible_items(0, city).contains(&repair),
        !flooded && !submerged,
        "the production chooser must not offer an impossible repair"
    );
}

#[test]
fn native_flood_flags_override_climate_on_rebuild_and_same_turn_delta_sync() {
    let (mut snapshot, mut state) = fixture(Some(true), Some(false));
    let rebuilt = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    assert_repair(&rebuilt.game, true, false);
    let mut live = LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
    assert_repair(&live.game, true, false);

    // A host-observed dry tile (e.g. after a barrier) wins over level 6.
    // Change only a delta, without advancing the turn or changing pillage.
    let mut dry = snapshot.revealed[&(7, 6)].clone();
    dry.flooded = Some(false);
    snapshot.merge_delta(&TilesChunk {
        turn: state.turn,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![dry],
    });
    live.sync(&snapshot, &state, 0);
    assert_repair(&live.game, false, false);
    let rebuilt = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    assert_repair(&rebuilt.game, false, false);

    // The legacy-climate fallback path must not overwrite an exact dry flag.
    state.climate = None;
    live.sync(&snapshot, &state, 0);
    assert_repair(&live.game, false, false);

    snapshot.revealed.get_mut(&(7, 6)).unwrap().submerged = Some(true);
    live.sync(&snapshot, &state, 0);
    assert_repair(&live.game, false, true);
}

#[test]
fn native_flood_flags_work_without_climate_and_legacy_keeps_phase_fallback() {
    let (snapshot, mut state) = fixture(Some(true), Some(false));
    state.climate = None;
    assert_repair(
        &rebuild_from_state(&snapshot, &state, 2, 1, 250, 0).game,
        true,
        false,
    );
    let (snapshot, state) = fixture(None, None);
    assert_repair(
        &rebuild_from_state(&snapshot, &state, 2, 1, 250, 0).game,
        true,
        false,
    );
    let legacy: Plot = serde_json::from_str(r#"{"x":7,"y":6}"#).unwrap();
    assert_eq!((legacy.flooded, legacy.submerged), (None, None));
}
