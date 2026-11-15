use super::*;
use civvis::mirror::{StateCity, StateGreatWork};

fn work(kind: &str, slot: i32, created: Option<i64>) -> StateGreatWork {
    StateGreatWork {
        kind: kind.into(),
        building: "BUILDING_MUSEUM_ART".into(),
        slot,
        created,
        ..StateGreatWork::default()
    }
}

fn city(id: i64, works: Vec<StateGreatWork>) -> StateCity {
    StateCity {
        id,
        great_works: Some(works),
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
    // Turn 142 of civvis-20261115T123222Z: the work was nine turns old.
    let board = state(
        142,
        vec![
            city(10, vec![work("ART_A", 0, Some(133))]),
            city(20, vec![work("ART_B", 0, Some(100))]),
        ],
    );
    assert!(locked(&board, 10, "ART_A", 20, 1));
    // ...and is the turn the lock ends.
    let later = state(
        143,
        vec![
            city(10, vec![work("ART_A", 0, Some(133))]),
            city(20, vec![work("ART_B", 0, Some(100))]),
        ],
    );
    assert!(!locked(&later, 10, "ART_A", 20, 1));
}

#[test]
fn the_work_in_the_slot_it_would_take_counts_too() {
    let board = state(
        142,
        vec![
            city(10, vec![work("ART_A", 0, Some(100))]),
            city(20, vec![work("ART_B", 1, Some(140))]),
        ],
    );
    assert!(locked(&board, 10, "ART_A", 20, 1));
    // Another slot of that museum is no obstacle.
    assert!(!locked(&board, 10, "ART_A", 20, 2));
}

#[test]
fn an_unknown_turn_holds_nothing_back() {
    // An older mod does not export the turn; the host's own refusal stands.
    let board = state(
        142,
        vec![
            city(10, vec![work("ART_A", 0, None)]),
            city(20, vec![work("ART_B", 1, None)]),
        ],
    );
    assert!(!locked(&board, 10, "ART_A", 20, 1));
    // A work elsewhere than the Art Museum, and a city the state does not
    // have, are not this lock's business.
    let mut elsewhere = work("ART_A", 0, Some(141));
    elsewhere.building = "BUILDING_PALACE".into();
    let board = state(142, vec![city(10, vec![elsewhere])]);
    assert!(!locked(&board, 10, "ART_A", 20, 1));
}
