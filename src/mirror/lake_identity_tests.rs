use super::*;

fn fixture(lake: Option<bool>) -> (Snapshot, StateSnapshot) {
    let plots = vec![
        serde_json::from_str(r#"{"x":6,"y":6,"t":"TERRAIN_GRASS","o":0}"#).unwrap(),
        serde_json::from_value(serde_json::json!({
            "x":7,"y":6,"t":"TERRAIN_COAST","w":true,"lk":lake
        }))
        .unwrap(),
    ];
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 133,
        width: 20,
        height: 20,
        chunk: 1,
        plots,
    }]);
    let state = StateSnapshot {
        turn: 133,
        techs: vec!["TECH_SAILING".into()],
        cities: vec![StateCity {
            id: 1,
            name: "Panamá".into(),
            x: 6,
            y: 6,
            pop: 5,
            capital: true,
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    };
    (snapshot, state)
}

fn assert_launch(game: &crate::game::Game, terrain: &str, launch: bool) {
    assert_eq!(
        game.map
            .get(crate::hex::offset_to_axial(7, 6))
            .unwrap()
            .terrain,
        terrain
    );
    let mut ai = crate::ai::BasicAi::new();
    ai.open_water_navy = true;
    let city = game.player_city_ids(0)[0];
    assert_eq!(ai.naval_city_can_launch(game, city), launch);
    assert_eq!(ai.desired_navy(game, 0), usize::from(launch));
}

#[test]
fn native_lake_identity_reaches_rebuild_and_same_turn_delta_sync() {
    let (mut snapshot, state) = fixture(Some(true));
    let rebuilt = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
    assert_launch(&rebuilt.game, "lake", false);
    let mut live = LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
    assert_launch(&live.game, "lake", false);

    let mut sea = snapshot.revealed[&(7, 6)].clone();
    sea.lk = Some(false);
    snapshot.merge_delta(&TilesChunk {
        turn: state.turn,
        width: 20,
        height: 20,
        chunk: 1,
        plots: vec![sea],
    });
    live.sync(&snapshot, &state, 0);
    assert_launch(&live.game, "coast", true);
}

#[test]
fn absent_lake_metadata_preserves_legacy_terrain_and_does_not_invent_land_water() {
    for lake in [None, Some(false)] {
        let (snapshot, state) = fixture(lake);
        assert_launch(
            &rebuild_from_state(&snapshot, &state, 2, 1, 250, 0).game,
            "coast",
            true,
        );
    }
    let (mut snapshot, state) = fixture(Some(true));
    let tile = snapshot.revealed.get_mut(&(7, 6)).unwrap();
    tile.t = Some("TERRAIN_GRASS".into());
    tile.w = false;
    assert_launch(
        &rebuild_from_state(&snapshot, &state, 2, 1, 250, 0).game,
        "grassland",
        false,
    );
}
