use super::*;

/// One of our cities and one foreign city whose banner shows `religion`.
fn fixture(religion: Option<&str>, minor: bool) -> (Snapshot, StateSnapshot) {
    let native_owner = if minor { 5 } else { 3 };
    let mut plots = Vec::new();
    for x in 0..20 {
        for y in 0..20 {
            let mut row = serde_json::json!({"x":x,"y":y,"t":"TERRAIN_GRASS","vis":true});
            if (x, y) == (10, 10) {
                row["o"] = native_owner.into();
                row["d"] = "DISTRICT_CITY_CENTER".into();
                row["dc"] = true.into();
            }
            plots.push(serde_json::from_value(row).unwrap());
        }
    }
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 120,
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
    let mut foreign = city(native_owner, 10, 10);
    foreign.religion = religion.map(str::to_string);
    let mut own = city(1, 2, 2);
    own.religion = Some("RELIGION_HINDUISM".into());
    let mut state = StateSnapshot {
        turn: 120,
        cities: vec![own],
        ..StateSnapshot::default()
    };
    if minor {
        state.minors.push(StateMinor {
            player: native_owner as usize,
            civ: "CIVILIZATION_KABUL".into(),
            cities: vec![foreign],
            ..StateMinor::default()
        });
    } else {
        state.rivals.push(StateRival {
            player: native_owner as usize,
            civ: "CIVILIZATION_SCOTLAND".into(),
            cities: vec![foreign],
            ..StateRival::default()
        });
    }
    (snapshot, state)
}

fn foreign_city_faith(
    religion: Option<&str>,
    minor: bool,
) -> (Option<String>, usize, Option<String>) {
    let (snapshot, state) = fixture(religion, minor);
    let reconstruction = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
    let game = &reconstruction.game;
    let cid = game
        .city_at(crate::hex::offset_to_axial(10, 10))
        .expect("foreign city placed");
    (
        game.observed_foreign_city_religion.get(&cid).cloned(),
        game.observed_foreign_city_religion.len(),
        game.city_religion(&game.cities[&cid]).map(str::to_string),
    )
}

/// The banner of a rival's or a city-state's city reaches the board, and only
/// foreign cities are recorded. It stays out of the pressure model.
#[test]
fn a_foreign_banner_is_recorded_for_rivals_and_city_states() {
    for minor in [false, true] {
        let (faith, recorded, pressure_read) = foreign_city_faith(Some("RELIGION_BUDDHISM"), minor);
        assert_eq!(faith.as_deref(), Some("Buddhism"), "minor={minor}");
        assert_eq!(recorded, 1, "our own city is not recorded");
        assert_eq!(pressure_read, None, "no pressure is invented");
    }
}

/// A city with no majority records nothing.
#[test]
fn a_city_without_a_majority_records_nothing() {
    let (faith, recorded, _) = foreign_city_faith(None, false);
    assert_eq!(faith, None);
    assert_eq!(recorded, 0);
}

/// The agent's JSON field reaches `StateCity` for a foreign city.
#[test]
fn the_agents_foreign_city_religion_field_parses() {
    let city: StateCity = serde_json::from_value(serde_json::json!({
        "id": 3, "x": 10, "y": 10, "pop": 4, "religion": "RELIGION_BUDDHISM"
    }))
    .unwrap();
    assert_eq!(city.religion.as_deref(), Some("RELIGION_BUDDHISM"));
}
