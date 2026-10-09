use super::*;

fn fixture(
    health: Option<serde_json::Value>,
    complete: bool,
    pillaged: bool,
    minor: bool,
) -> (Snapshot, StateSnapshot) {
    let native_owner = if minor { 5 } else { 3 };
    let mut plots = Vec::new();
    for x in 0..20 {
        for y in 0..20 {
            let mut row = serde_json::json!({"x":x,"y":y,"t":"TERRAIN_GRASS","vis":true});
            if (x, y) == (10, 10) || (x, y) == (9, 10) {
                row["o"] = native_owner.into();
                row["d"] = if x == 10 {
                    "DISTRICT_CITY_CENTER"
                } else {
                    "DISTRICT_ENCAMPMENT"
                }
                .into();
                row["dc"] = complete.into();
                row["p"] = pillaged.into();
                if x == 9 {
                    if let Some(value) = &health {
                        row["dh"] = value.clone();
                    }
                }
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
    let city = |id, x, y| StateCity {
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
    let foreign = city(native_owner, 10, 10);
    let mut state = StateSnapshot {
        turn: 180,
        cities: vec![city(1, 2, 2)],
        ..StateSnapshot::default()
    };
    if minor {
        state.minors.push(StateMinor {
            player: native_owner as usize,
            civ: "CIVILIZATION_KABUL".into(),
            at_war: true,
            cities: vec![foreign],
            ..StateMinor::default()
        });
    } else {
        state.rivals.push(StateRival {
            player: native_owner as usize,
            civ: "CIVILIZATION_SCOTLAND".into(),
            at_war: true,
            cities: vec![foreign],
            ..StateRival::default()
        });
    }
    (snapshot, state)
}

fn health(
    damage: i32,
    max_damage: i32,
    wall_damage: i32,
    max_wall_damage: i32,
) -> serde_json::Value {
    serde_json::json!({
        "damage":damage,"max_damage":max_damage,
        "wall_damage":wall_damage,"max_wall_damage":max_wall_damage
    })
}

fn reconstructed(
    value: Option<serde_json::Value>,
    complete: bool,
    pillaged: bool,
    minor: bool,
) -> (Reconstruction, u32) {
    let (snapshot, state) = fixture(value, complete, pillaged, minor);
    let reconstruction = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    let position = crate::hex::offset_to_axial(10, 10);
    let cid = reconstruction
        .game
        .city_at(position)
        .expect("foreign city placed");
    assert_ne!(reconstruction.game.cities[&cid].owner, 0);
    (reconstruction, cid)
}

#[test]
fn a_healthy_foreign_encampment_keeps_its_independent_shot() {
    let (mut reconstruction, cid) = reconstructed(Some(health(0, 100, 0, 200)), true, false, false);
    let city = &reconstruction.game.cities[&cid];
    assert_eq!(
        city.districts.get(crate::name!("encampment")),
        Some(&crate::hex::offset_to_axial(9, 10))
    );
    assert_eq!(city.encampment_hp, 100);
    assert_eq!(city.encampment_wall_hp, 200);
    assert!(reconstruction.game.encampment_can_strike(city));
    let owner = city.owner;
    let target = crate::hex::offset_to_axial(8, 10);
    let uid = reconstruction.game.spawn_test_unit("warrior", 0, target);
    let action = crate::game::Action::EncampmentStrike { city: cid, target };
    assert!(reconstruction.game.is_at_war(0, owner));
    reconstruction.game.current = owner;
    assert!(reconstruction.game.legal_actions(owner).contains(&action));
    reconstruction
        .game
        .apply(owner, &action)
        .expect("actual Encampment shot");
    assert!(reconstruction
        .game
        .units
        .get(&uid)
        .is_none_or(|unit| unit.hp < 100));
}

#[test]
fn foreign_encampment_damage_is_rescaled_and_not_borrowed_from_the_city() {
    let (reconstruction, cid) = reconstructed(Some(health(120, 200, 150, 400)), true, false, false);
    let city = &reconstruction.game.cities[&cid];
    assert_eq!(city.encampment_hp, 40);
    assert_eq!(city.encampment_wall_hp, 250);
    assert!(reconstruction.game.encampment_can_strike(city));
}

#[test]
fn a_city_state_encampment_uses_the_same_observed_defenses() {
    let (reconstruction, cid) = reconstructed(Some(health(30, 100, 40, 200)), true, false, true);
    let city = &reconstruction.game.cities[&cid];
    assert!(reconstruction.game.players[city.owner].is_minor);
    assert_eq!(city.encampment_hp, 70);
    assert_eq!(city.encampment_wall_hp, 160);
    assert!(reconstruction.game.encampment_can_strike(city));
}

#[test]
fn observed_zero_outer_defenses_are_not_missing_health() {
    for value in [health(0, 100, 0, 0), health(0, 100, 200, 200)] {
        let (reconstruction, cid) = reconstructed(Some(value), true, false, false);
        let city = &reconstruction.game.cities[&cid];
        assert_eq!(city.encampment_hp, 100);
        assert_eq!(city.encampment_wall_hp, 0);
        assert!(!reconstruction.game.encampment_can_strike(city));
    }
}

#[test]
fn a_destroyed_or_pillaged_foreign_encampment_cannot_strike() {
    for (value, pillaged) in [
        (health(100, 100, 200, 200), false),
        (health(0, 100, 0, 200), true),
    ] {
        let (reconstruction, cid) = reconstructed(Some(value), true, pillaged, false);
        let city = &reconstruction.game.cities[&cid];
        assert!(city.districts.contains_key(crate::name!("encampment")));
        assert_eq!(city.encampment_hp, if pillaged { 100 } else { 0 });
        assert_eq!(city.encampment_pillaged, pillaged);
        assert!(!reconstruction.game.encampment_can_strike(city));
    }
}

#[test]
fn an_unbuilt_encampment_does_not_gain_a_defensive_district() {
    let (reconstruction, cid) = reconstructed(Some(health(0, 100, 0, 200)), false, false, false);
    let city = &reconstruction.game.cities[&cid];
    assert!(!city.districts.contains_key(crate::name!("encampment")));
    assert!(
        reconstruction.game.map.tiles[&crate::hex::offset_to_axial(9, 10)]
            .district
            .is_none()
    );
}

#[test]
fn legacy_or_failed_foreign_health_does_not_invent_a_destroyed_fort() {
    for value in [
        None,
        Some(serde_json::json!({})),
        Some(health(-1, -1, -1, -1)),
    ] {
        let (reconstruction, cid) = reconstructed(value, true, false, false);
        let city = &reconstruction.game.cities[&cid];
        assert_eq!(city.encampment_hp, 100);
        assert_eq!(
            city.encampment_wall_hp,
            reconstruction.game.city_max_wall_hp(city)
        );
    }
}

#[test]
fn an_observed_health_change_updates_the_existing_foreign_district() {
    let (mut reconstruction, cid) = reconstructed(Some(health(0, 100, 0, 200)), true, false, false);
    for (value, expected) in [
        (health(60, 100, 125, 200), (40, 75)),
        (health(100, 100, 200, 200), (0, 0)),
    ] {
        let (snapshot, _) = fixture(Some(value), true, false, false);
        apply_foreign_infrastructure(&mut reconstruction.game, &snapshot);
        let city = &reconstruction.game.cities[&cid];
        assert_eq!((city.encampment_hp, city.encampment_wall_hp), expected);
    }
}
