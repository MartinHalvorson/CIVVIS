use super::*;
use civvis::mirror::{StateCity, StateGreatWork};

/// A city whose Art Museum holds `works`: (type, slot, turn created).
fn city(id: i64, works: &[(&str, i32, Option<i64>)]) -> StateCity {
    StateCity {
        id,
        great_works: Some(
            works
                .iter()
                .map(|(kind, slot, _)| StateGreatWork {
                    kind: (*kind).into(),
                    building: "BUILDING_MUSEUM_ART".into(),
                    slot: *slot,
                    ..StateGreatWork::default()
                })
                .collect(),
        ),
        great_work_turns: Some(
            works
                .iter()
                .filter_map(|(kind, _, made)| made.map(|turn| ((*kind).to_string(), turn)))
                .collect(),
        ),
        ..StateCity::default()
    }
}

fn state(turn: u32, cities: Vec<StateCity>) -> StateSnapshot {
    StateSnapshot {
        turn,
        cities,
        ..StateSnapshot::default()
    }
}

#[test]
fn a_work_inside_its_lock_is_not_proposed() {
    let cities = || {
        vec![
            city(10, &[("ART_A", 0, Some(133))]),
            city(20, &[("ART_B", 0, Some(100))]),
        ]
    };
    // Nine turns old: held.
    assert!(locked(&state(142, cities()), 10, "ART_A", 20, 1));
    // The turn the lock ends, which is the turn the host accepts it.
    assert!(!locked(&state(143, cities()), 10, "ART_A", 20, 1));
}

#[test]
fn the_work_in_the_slot_it_would_take_counts_too() {
    let board = state(
        142,
        vec![
            city(10, &[("ART_A", 0, Some(100))]),
            city(20, &[("ART_B", 1, Some(140))]),
        ],
    );
    assert!(locked(&board, 10, "ART_A", 20, 1));
    // Another slot of that museum is no obstacle.
    assert!(!locked(&board, 10, "ART_A", 20, 2));
}

#[test]
fn an_unknown_turn_holds_nothing_back() {
    // An older mod does not export the turns; the host's own refusal stands.
    let board = state(
        142,
        vec![
            city(10, &[("ART_A", 0, None)]),
            city(20, &[("ART_B", 1, None)]),
        ],
    );
    assert!(!locked(&board, 10, "ART_A", 20, 1));
    // A city the state does not have is not this lock's business.
    let alone = state(142, vec![city(10, &[("ART_A", 0, Some(100))])]);
    assert!(!locked(&alone, 10, "ART_A", 20, 1));
}
