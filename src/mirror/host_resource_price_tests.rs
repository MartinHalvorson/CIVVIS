use super::*;

fn native_state() -> StateSnapshot {
    state_from_json(
        r#"{
        "turn":144,
        "techs":["TECH_METAL_CASTING"],"strategic_resources":{"RESOURCE_NITER":10},
        "cities":[{"id":77,"name":"Bogotá","x":3,"y":3,"pop":5,"capital":true,
            "buildable":[{"t":"UNIT_BOMBARD","c":140,"p":4,"r":10}]}]
    }"#,
    )
    .unwrap()
}

#[test]
fn rebuild_and_refresh_use_the_current_native_price_and_clear_cached_choices() {
    let mut state = native_state();
    assert!(state.schema_gaps.is_empty(), "{:?}", state.schema_gaps);
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: state.turn,
        width: 52,
        height: 42,
        chunk: 1,
        plots: vec![serde_json::from_str(r#"{"x":3,"y":3,"t":"TERRAIN_GRASS","o":0}"#).unwrap()],
    }]);
    let item = crate::game::Item::Unit {
        unit: crate::name!("bombard"),
    };
    let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
    let city = rebuilt.game.player_city_ids(0)[0];
    assert!(rebuilt.game.can_produce(0, city, &item));
    assert!(rebuilt.game.producible_items(0, city).contains(&item));
    let mut live = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let city = live.game.player_city_ids(0)[0];
    assert!(live.game.producible_items(0, city).contains(&item));
    state.turn += 1;
    state.cities[0].buildable.as_mut().unwrap()[0].r = Some(12.0);
    live.sync(&snapshot, &state, 0);
    assert!(!live.game.can_produce(0, city, &item));
    assert!(!live.game.producible_items(0, city).contains(&item));
    state.turn += 1;
    state.cities[0].buildable.as_mut().unwrap()[0].r = Some(0.0);
    live.sync(&snapshot, &state, 0);
    assert!(live.game.can_produce(0, city, &item));
    state.turn += 1;
    state.cities[0].buildable.as_mut().unwrap()[0].r = None;
    live.sync(&snapshot, &state, 0);
    assert!(live.game.host_unit_resource_prices.is_empty());
    assert!(
        !live.game.can_produce(0, city, &item),
        "an old export must not retain a previous native price"
    );
}

#[test]
fn exact_tiers_survive_translation_and_invalid_or_nonunit_prices_do_not() {
    let mut state = native_state();
    let menu = state.cities[0].buildable.as_mut().unwrap();
    menu.extend([
        StateMenuItem {
            t: "UNIT_BOMBARD".to_string(),
            f: Some(1),
            r: Some(18.0),
            ..StateMenuItem::default()
        },
        StateMenuItem {
            t: "UNIT_BOMBARD".to_string(),
            f: Some(2),
            r: Some(25.0),
            ..StateMenuItem::default()
        },
        StateMenuItem {
            t: "BUILDING_GRANARY".to_string(),
            r: Some(1.0),
            ..StateMenuItem::default()
        },
        StateMenuItem {
            t: "UNIT_DOES_NOT_EXIST".to_string(),
            r: Some(1.0),
            ..StateMenuItem::default()
        },
    ]);
    let rules = crate::rules::Rules::embedded();
    let menus = host_menus_from(&state.cities, &[(42, 77)].into(), &rules);
    let prices = &menus.unit_resource_prices[&42];
    assert_eq!(prices.len(), 3);
    assert_eq!(prices["unit:bombard"], 10.0);
    assert_eq!(prices["formation:bombard:1"], 18.0);
    assert_eq!(prices["formation:bombard:2"], 25.0);
    for invalid in [None, Some(-1.0), Some(f64::NAN), Some(f64::INFINITY)] {
        state.cities[0].buildable.as_mut().unwrap().truncate(1);
        state.cities[0].buildable.as_mut().unwrap()[0].r = invalid;
        let menus = host_menus_from(&state.cities, &[(42, 77)].into(), &rules);
        assert!(menus.unit_resource_prices.is_empty());
    }
}
