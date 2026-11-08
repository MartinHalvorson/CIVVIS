use super::*;
use civvis::mirror::{Seat, Snapshot, StateCity, StateRival, TilesChunk};

fn fixture() -> (Snapshot, StateSnapshot) {
    let plots = (0..16)
        .flat_map(|x| {
            (0..16).map(move |y| {
                serde_json::from_value(serde_json::json!({"x":x,"y":y,"t":"TERRAIN_GRASS","o":-1}))
                    .unwrap()
            })
        })
        .collect();
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 161,
        width: 16,
        height: 16,
        chunk: 1,
        plots,
    }]);
    let city = |id, x, y| StateCity {
        id,
        x,
        y,
        pop: 4,
        ..Default::default()
    };
    let state = StateSnapshot {
        turn: 161,
        seat: Seat {
            players: 4,
            local_player: 2,
            speed: "GAMESPEED_ONLINE".into(),
            ..Default::default()
        },
        cities: vec![city(10, 3, 3)],
        rivals: vec![StateRival {
            player: 3,
            at_war: true,
            cities: vec![city(20, 12, 12)],
            ..Default::default()
        }],
        ..Default::default()
    };
    (snapshot, state)
}

fn board(snapshot: &Snapshot, state: &StateSnapshot) -> LiveMirror {
    LiveMirror::new(snapshot, state, 4, 47_149, 500, 0)
}

#[test]
fn a_rebuilt_board_keeps_the_observed_war_old_enough_for_peace() {
    let (snapshot, mut state) = fixture();
    let mut history = History::default();
    history.observe_and_apply(&mut board(&snapshot, &state), &state);
    state.turn = 180;
    let mut rebuilt = board(&snapshot, &state);
    rebuilt.game.sync_war_log();
    assert!(rebuilt.game.peace_available_at(0, 3).is_some());
    history.observe_and_apply(&mut rebuilt, &state);
    assert_eq!(rebuilt.game.wars[&(0, 3)].started, 161);
    assert_eq!(rebuilt.game.peace_available_at(0, 3), None);
}

#[test]
fn peace_then_a_new_war_restarts_the_observed_age() {
    let (snapshot, mut state) = fixture();
    let mut history = History::default();
    history.observe_and_apply(&mut board(&snapshot, &state), &state);
    state.turn = 180;
    state.rivals[0].at_war = false;
    history.observe_and_apply(&mut board(&snapshot, &state), &state);
    state.turn = 190;
    state.rivals[0].at_war = true;
    let mut restarted = board(&snapshot, &state);
    history.observe_and_apply(&mut restarted, &state);
    assert_eq!(restarted.game.wars[&(0, 3)].started, 190);
    assert!(restarted.game.peace_available_at(0, 3).is_some());
}

#[test]
fn joining_an_existing_war_does_not_invent_an_earlier_declaration() {
    let (snapshot, mut state) = fixture();
    state.turn = 190;
    let mut history = History::default();
    let mut joined = board(&snapshot, &state);
    history.observe_and_apply(&mut joined, &state);
    assert_eq!(joined.game.wars[&(0, 3)].started, 190);
    assert!(joined.game.peace_available_at(0, 3).is_some());
}
