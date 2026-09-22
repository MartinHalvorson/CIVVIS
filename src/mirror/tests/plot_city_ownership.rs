use super::*;

fn fixture(city: Option<i64>) -> (Snapshot, StateSnapshot) {
    let mut plots = Vec::new();
    for x in 0..12 {
        for y in 0..10 {
            let mut tile = plot(x, y, "TERRAIN_GRASS");
            if [(3, 4), (7, 4), (4, 4)].contains(&(x, y)) {
                tile.o = 0;
            }
            if (x, y) == (4, 4) {
                tile.oc = city;
                tile.f = Some("FEATURE_FOREST".into());
            }
            plots.push(tile);
        }
    }
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 94,
        width: 12,
        height: 10,
        chunk: 1,
        plots,
    }]);
    let state = StateSnapshot {
        turn: 94,
        cities: vec![
            StateCity {
                id: 10,
                name: "Near".into(),
                x: 3,
                y: 4,
                pop: 4,
                ..Default::default()
            },
            StateCity {
                id: 20,
                name: "Sanctuary".into(),
                x: 7,
                y: 4,
                pop: 4,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    (snapshot, state)
}

#[test]
fn native_plot_city_ownership_pays_the_assigned_production_queue() {
    let (snapshot, state) = fixture(Some(20));
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let game = &mut mirror.game;
    let target = crate::hex::offset_to_axial(4, 4);
    let near = game.city_at(crate::hex::offset_to_axial(3, 4)).unwrap();
    let far = game.city_at(crate::hex::offset_to_axial(7, 4)).unwrap();
    assert_eq!(game.map.tiles[&target].owner_city, Some(far));
    assert!(game.cities[&far].owned_tiles.contains(&target));
    assert!(!game.cities[&near].owned_tiles.contains(&target));
    game.players[0].techs.insert(crate::name!("mining"));
    let builder = game.spawn_test_unit("builder", 0, target);
    let before_near = game.cities[&near].production;
    let before_far = game.cities[&far].production;
    game.apply(
        0,
        &crate::game::Action::Improve {
            unit: builder,
            improvement: crate::name!("chop_woods"),
        },
    )
    .unwrap();
    assert_eq!(game.cities[&near].production, before_near);
    assert!(game.cities[&far].production > before_far);
}

#[test]
fn native_plot_city_ownership_refreshes_same_player_swaps() {
    let (snapshot, mut state) = fixture(Some(20));
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let (mut later, _) = fixture(Some(10));
    later.turn = 95;
    state.turn = 95;
    mirror.sync(&later, &state, 0);
    let target = crate::hex::offset_to_axial(4, 4);
    let near = mirror
        .game
        .city_at(crate::hex::offset_to_axial(3, 4))
        .unwrap();
    let far = mirror
        .game
        .city_at(crate::hex::offset_to_axial(7, 4))
        .unwrap();
    assert_eq!(mirror.game.map.tiles[&target].owner_city, Some(near));
    assert!(mirror.game.cities[&near].owned_tiles.contains(&target));
    assert!(!mirror.game.cities[&far].owned_tiles.contains(&target));
}

#[test]
fn native_plot_city_ownership_distinguishes_unknown_from_legacy() {
    for (native, expected) in [(None, Some("Near")), (Some(999), None)] {
        let (snapshot, state) = fixture(native);
        let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
        let target = crate::hex::offset_to_axial(4, 4);
        let name = mirror.game.map.tiles[&target]
            .owner_city
            .map(|id| mirror.game.cities[&id].name.as_str());
        assert_eq!(name, expected);
    }
    let parsed: Plot = serde_json::from_str(r#"{"x":4,"y":4,"o":0,"oc":0}"#).unwrap();
    assert_eq!(parsed.oc, Some(0), "city zero is a valid observation");
}

#[test]
fn native_plot_city_ownership_does_not_claim_foreign_ground() {
    let (mut snapshot, state) = fixture(Some(20));
    snapshot.revealed.get_mut(&(4, 4)).unwrap().o = 9;
    let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let target = crate::hex::offset_to_axial(4, 4);
    assert_eq!(mirror.game.map.tiles[&target].owner_city, None);
    assert!(mirror.game.closed_borders.contains(&target));
}

#[test]
fn native_plot_city_ownership_clears_an_unresolved_assignment_on_sync() {
    let (snapshot, mut state) = fixture(Some(20));
    let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let (mut later, _) = fixture(Some(999));
    later.turn = 95;
    state.turn = 95;
    mirror.sync(&later, &state, 0);
    let target = crate::hex::offset_to_axial(4, 4);
    assert_eq!(mirror.game.map.tiles[&target].owner_city, None);
    assert!(mirror
        .game
        .cities
        .values()
        .all(|city| !city.owned_tiles.contains(&target)));
}
