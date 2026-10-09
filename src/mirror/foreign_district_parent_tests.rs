use super::*;

const CENTRES: [(i32, i32); 2] = [(9, 10), (12, 10)];
const FORTS: [(i32, i32); 2] = [(11, 10), (13, 10)];

fn fixture(minor: bool, parents: [Option<i64>; 2]) -> (Snapshot, StateSnapshot) {
    let owner = if minor { 5 } else { 3 };
    let mut plots = Vec::new();
    for x in 0..20 {
        for y in 0..20 {
            let mut row = serde_json::json!({"x":x,"y":y,"t":"TERRAIN_GRASS","vis":true});
            if CENTRES.contains(&(x, y)) {
                row["o"] = owner.into();
                row["d"] = "DISTRICT_CITY_CENTER".into();
            }
            if let Some(index) = FORTS.iter().position(|pos| *pos == (x, y)) {
                row["o"] = owner.into();
                row["d"] = "DISTRICT_ENCAMPMENT".into();
                row["dc"] = true.into();
                row["oc"] = parents[index].into();
                row["dh"] = serde_json::json!({"damage":index * 20,"max_damage":100,
                    "wall_damage":index * 40,"max_wall_damage":200});
            }
            plots.push(serde_json::from_value(row).unwrap());
        }
    }
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 180,
        width: 20,
        height: 20,
        chunk: 1,
        plots,
    }]);
    let city = |id, (x, y)| StateCity {
        id,
        x,
        y,
        pop: 5,
        loyalty: 100.0,
        damage: 150.0,
        max_damage: 200.0,
        wall_damage: 30.0,
        max_wall_damage: 200.0,
        ..StateCity::default()
    };
    let cities = vec![city(31, CENTRES[0]), city(32, CENTRES[1])];
    // City IDs are scoped to native players: our city deliberately shares 31.
    let mut state = StateSnapshot {
        turn: 180,
        cities: vec![city(31, (2, 2))],
        ..StateSnapshot::default()
    };
    if minor {
        state.minors.push(StateMinor {
            player: owner as usize,
            civ: "CIVILIZATION_KABUL".into(),
            at_war: true,
            cities,
            ..StateMinor::default()
        });
    } else {
        state.rivals.push(StateRival {
            player: owner as usize,
            civ: "CIVILIZATION_SCOTLAND".into(),
            at_war: true,
            cities,
            ..StateRival::default()
        });
    }
    (snapshot, state)
}

fn city_at(game: &crate::game::Game, index: usize) -> u32 {
    game.city_at(crate::hex::offset_to_axial(
        CENTRES[index].0,
        CENTRES[index].1,
    ))
    .expect("observed foreign city")
}

fn assert_parent(game: &crate::game::Game, fort: usize, parent: usize, kind: Name) {
    let cid = city_at(game, parent);
    let pos = crate::hex::offset_to_axial(FORTS[fort].0, FORTS[fort].1);
    assert_eq!(
        game.map.tiles[&pos].owner_city,
        Some(cid),
        "physical district parent"
    );
    assert_eq!(
        game.cities[&cid].districts.get(kind),
        Some(&pos),
        "city district roster"
    );
}

#[test]
fn two_observed_encampments_keep_both_parents_and_actual_shots() {
    let (snapshot, state) = fixture(false, [Some(31), Some(32)]);
    let reconstruction = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    for index in 0..2 {
        let mut game = reconstruction.game.clone();
        assert_parent(&game, index, index, crate::name!("encampment"));
        let cid = city_at(&game, index);
        let city = &game.cities[&cid];
        assert_eq!(
            (city.encampment_hp, city.encampment_wall_hp),
            (100 - index as i32 * 20, 200 - index as i32 * 40)
        );
        let owner = city.owner;
        let target = crate::hex::offset_to_axial(12, 11);
        let uid = game.spawn_test_unit("warrior", 0, target);
        let action = crate::game::Action::EncampmentStrike { city: cid, target };
        assert!(game.is_at_war(0, owner));
        game.current = owner;
        assert!(
            game.legal_actions(owner).contains(&action),
            "each fort offers its own shot"
        );
        game.apply(owner, &action)
            .expect("actual independent fort shot");
        assert!(game.units.get(&uid).is_none_or(|unit| unit.hp < 100));
    }
}

#[test]
fn a_known_parent_takes_precedence_over_the_nearest_city() {
    let (mut snapshot, state) = fixture(false, [Some(31), Some(32)]);
    snapshot.revealed.get_mut(&FORTS[1]).unwrap().d = None;
    let reconstruction = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    assert_parent(&reconstruction.game, 0, 0, crate::name!("encampment"));
    assert!(
        !reconstruction.game.cities[&city_at(&reconstruction.game, 1)]
            .districts
            .contains_key(crate::name!("encampment"))
    );
}

#[test]
fn legacy_foreign_districts_keep_the_nearest_city_fallback() {
    let (mut snapshot, state) = fixture(false, [None, None]);
    snapshot.revealed.get_mut(&FORTS[1]).unwrap().d = None;
    let reconstruction = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    assert_parent(&reconstruction.game, 0, 1, crate::name!("encampment"));
}

#[test]
fn an_explicit_unknown_parent_is_not_redirected_to_another_city() {
    let (mut snapshot, state) = fixture(false, [Some(99), Some(32)]);
    snapshot.revealed.get_mut(&FORTS[1]).unwrap().d = None;
    let reconstruction = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    let game = &reconstruction.game;
    let pos = crate::hex::offset_to_axial(FORTS[0].0, FORTS[0].1);
    assert_eq!(game.map.tiles[&pos].owner_city, None);
    assert!(game
        .cities
        .values()
        .all(|city| !city.districts.contains_key(crate::name!("encampment"))));
}

#[test]
fn city_state_districts_resolve_native_parent_ids() {
    let (snapshot, state) = fixture(true, [Some(31), Some(32)]);
    let reconstruction = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    for index in 0..2 {
        assert_parent(
            &reconstruction.game,
            index,
            index,
            crate::name!("encampment"),
        );
        assert!(
            reconstruction.game.players
                [reconstruction.game.cities[&city_at(&reconstruction.game, index)].owner]
                .is_minor
        );
    }
}

#[test]
fn native_parent_ids_are_scoped_to_the_observed_owner() {
    let (snapshot, mut state) = fixture(false, [Some(31), Some(32)]);
    state.rivals.insert(
        0,
        StateRival {
            player: 2,
            civ: "CIVILIZATION_AUSTRALIA".into(),
            cities: vec![StateCity {
                id: 31,
                x: 5,
                y: 5,
                pop: 5,
                ..StateCity::default()
            }],
            ..StateRival::default()
        },
    );
    let reconstruction = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    assert_parent(&reconstruction.game, 0, 0, crate::name!("encampment"));
    let cid = city_at(&reconstruction.game, 0);
    assert_ne!(reconstruction.game.cities[&cid].owner, 0);
    let other = reconstruction
        .game
        .city_at(crate::hex::offset_to_axial(5, 5))
        .unwrap();
    assert_ne!(
        reconstruction.game.cities[&cid].owner,
        reconstruction.game.cities[&other].owner
    );
    assert!(!reconstruction.game.cities[&other]
        .districts
        .contains_key(crate::name!("encampment")));
}

#[test]
fn persistent_sync_moves_a_changed_parent_and_clears_the_old_roster() {
    let (mut snapshot, state) = fixture(false, [Some(32), Some(32)]);
    snapshot.revealed.get_mut(&FORTS[1]).unwrap().d = None;
    let mut live = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    assert_parent(&live.game, 0, 1, crate::name!("encampment"));
    snapshot.revealed.get_mut(&FORTS[0]).unwrap().oc = Some(31);
    live.sync(&snapshot, &state, 0);
    assert_parent(&live.game, 0, 0, crate::name!("encampment"));
    assert!(!live.game.cities[&city_at(&live.game, 1)]
        .districts
        .contains_key(crate::name!("encampment")));
}

#[test]
fn nondefending_districts_use_the_same_observed_parent() {
    let (mut snapshot, state) = fixture(false, [Some(31), Some(32)]);
    snapshot.revealed.get_mut(&FORTS[0]).unwrap().d = Some("DISTRICT_CAMPUS".into());
    snapshot.revealed.get_mut(&FORTS[1]).unwrap().d = None;
    let reconstruction = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    assert_parent(&reconstruction.game, 0, 0, crate::name!("campus"));
}
