use super::*;
use crate::ai::AdvancedAi;
use crate::game::Game;

fn fixture() -> Game {
    let mut game = Game::new_full(2, 24, 18, 363800, 500, 0, false);
    game.clear_mirror_cities();
    for tile in game.map.tiles.values_mut() {
        tile.resource = None;
        tile.improvement = None;
    }
    for pid in 0..2 {
        game.players[pid].techs.insert(crate::name!("radio"));
        game.players[pid].techs.insert(crate::name!("refining"));
    }
    game.found_city_for(0, (6, 9), None);
    game.players[0].strategic_resources.clear();
    game
}

fn state(amount: f64) -> StateSnapshot {
    StateSnapshot {
        strategic_resource_income: Some(BTreeMap::from([("RESOURCE_ALUMINUM".into(), amount)])),
        ..Default::default()
    }
}

#[test]
fn imported_income_sustains_a_wing_without_a_visible_deposit() {
    let mut game = fixture();
    assert_eq!(AdvancedAi::air_surge_bomber_goal(&game, 0), 0);
    apply(&mut game, &state(4.0), &mut Vec::new());
    assert_eq!(game.strategic_resource_rate(0, "aluminum"), 4.0);
    assert_eq!(AdvancedAi::air_surge_bomber_goal(&game, 0), 4);
    assert_eq!(game.strategic_resource_rate(1, "aluminum"), 0.0);
    game.spawn_test_unit("jet_fighter", 0, (6, 9));
    assert_eq!(
        AdvancedAi::air_surge_bomber_goal(&game, 0),
        3,
        "gross imports must still pay for the existing fighter"
    );
}

#[test]
fn explicit_zero_overrides_a_deposit_and_refresh_does_not_accumulate() {
    let mut game = fixture();
    game.map.tiles.get_mut(&(6, 9)).unwrap().resource = Some(crate::name!("aluminum"));
    let modeled = game.strategic_resource_rate(0, "aluminum");
    assert!(modeled > 0.0);
    apply(&mut game, &state(0.0), &mut Vec::new());
    assert_eq!(game.strategic_resource_rate(0, "aluminum"), 0.0);
    assert_eq!(AdvancedAi::air_surge_bomber_goal(&game, 0), 0);
    for _ in 0..3 {
        apply(&mut game, &state(3.0), &mut Vec::new());
        assert_eq!(game.strategic_resource_rate(0, "aluminum"), 3.0);
    }
    apply(&mut game, &StateSnapshot::default(), &mut Vec::new());
    assert_eq!(game.strategic_resource_rate(0, "aluminum"), modeled);
}

#[test]
fn correction_preserves_counterfactual_resource_gains_and_serialization() {
    let mut game = fixture();
    apply(&mut game, &state(2.0), &mut Vec::new());
    let mut counterfactual = game.clone();
    counterfactual.map.tiles.get_mut(&(6, 9)).unwrap().resource = Some(crate::name!("aluminum"));
    assert!(counterfactual.strategic_resource_rate(0, "aluminum") > 2.0);
    assert_eq!(game.strategic_resource_rate(0, "aluminum"), 2.0);
    let encoded = serde_json::to_string(&game).unwrap();
    let decoded: Game = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.strategic_resource_rate(0, "aluminum"), 2.0);
    let mut legacy = serde_json::to_value(&game).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("observed_strategic_income_adjustments");
    let decoded: Game = serde_json::from_value(legacy).unwrap();
    assert_eq!(decoded.strategic_resource_rate(0, "aluminum"), 0.0);
}

#[test]
fn invalid_entries_are_reported_and_absent_entries_remain_modeled() {
    let mut game = fixture();
    game.players[0]
        .counters
        .insert("great_person:oil_per_turn".into(), 3);
    let mut snapshot = state(4.0);
    snapshot
        .strategic_resource_income
        .as_mut()
        .unwrap()
        .extend([
            ("RESOURCE_OIL".into(), f64::NAN),
            ("RESOURCE_IRON".into(), -1.0),
            ("RESOURCE_UNOBTAINIUM".into(), 5.0),
        ]);
    let mut unmapped = Vec::new();
    for _ in 0..2 {
        apply(&mut game, &snapshot, &mut unmapped);
    }
    assert_eq!(unmapped.len(), 3);
    assert_eq!(game.strategic_resource_rate(0, "oil"), 3.0);
    for json in [
        r#"{"turn":1}"#,
        r#"{"turn":1,"strategic_resource_income":[]}"#,
    ] {
        let parsed: StateSnapshot = serde_json::from_str(json).unwrap();
        apply(&mut game, &parsed, &mut unmapped);
        assert_eq!(game.strategic_resource_rate(0, "aluminum"), 0.0);
    }
}

#[test]
fn full_mirror_and_sync_reconcile_the_finished_board() {
    let plots = (3..=9)
        .flat_map(|x| {
            (3..=9).map(move |y| {
                serde_json::from_value(serde_json::json!({"x":x,"y":y,"t":"TERRAIN_GRASS","o":-1}))
                    .unwrap()
            })
        })
        .collect();
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 120,
        width: 12,
        height: 12,
        chunk: 1,
        plots,
    }]);
    let mut observed = state(4.0);
    observed.turn = 120;
    observed.cities.push(StateCity {
        id: 1,
        name: "Bogota".into(),
        x: 5,
        y: 5,
        pop: 8,
        capital: true,
        ..Default::default()
    });
    let mut mirror = LiveMirror::new(&snapshot, &observed, 4, 1, 500, 0);
    assert_eq!(mirror.game.strategic_resource_rate(0, "aluminum"), 4.0);
    observed.strategic_resource_income = state(0.0).strategic_resource_income;
    mirror.sync(&snapshot, &observed, 4);
    assert_eq!(mirror.game.strategic_resource_rate(0, "aluminum"), 0.0);
    observed.strategic_resource_income = state(2.0).strategic_resource_income;
    mirror.sync(&snapshot, &observed, 4);
    assert_eq!(mirror.game.strategic_resource_rate(0, "aluminum"), 2.0);
}
