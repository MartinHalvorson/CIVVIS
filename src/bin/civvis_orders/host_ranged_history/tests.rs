use super::*;
use civvis::mirror::{Seat, Snapshot, StateCity, StateRival, StateUnit, TilesChunk};

fn fixture() -> (Snapshot, StateSnapshot) {
    let plots = (0..26)
        .flat_map(|x| {
            (0..18).map(move |y| {
                serde_json::from_value(serde_json::json!({"x":x,"y":y,"t":"TERRAIN_GRASS","o":-1}))
                    .unwrap()
            })
        })
        .collect();
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 133,
        width: 26,
        height: 18,
        chunk: 1,
        plots,
    }]);
    let state = StateSnapshot {
        turn: 133,
        seat: Seat {
            players: 4,
            local_player: 0,
            civ: "CIVILIZATION_GRAN_COLOMBIA".into(),
            ..Default::default()
        },
        rivals: vec![StateRival {
            player: 2,
            civ: "CIVILIZATION_NUBIA".into(),
            cities: vec![StateCity {
                id: 720906,
                name: "Panamá".into(),
                x: 22,
                y: 4,
                pop: 2,
                defense: 68.0,
                max_damage: 200.0,
                max_wall_damage: 400.0,
                ..Default::default()
            }],
            units: vec![StateUnit {
                id: 1001,
                kind: "UNIT_FIELD_CANNON".into(),
                x: 20,
                y: 4,
                hp: 100.0,
                ranged: 60.0,
                moves: 2.0,
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    (snapshot, state)
}

fn board(snapshot: &Snapshot, state: &StateSnapshot) -> LiveMirror {
    LiveMirror::new(snapshot, state, 4, 3922, 650, 0)
}

fn fire(board: &LiveMirror) -> f64 {
    let city = board
        .game
        .cities
        .values()
        .find(|c| c.name == "Panamá")
        .unwrap();
    board.game.city_ranged_strength(city.id)
}

#[test]
fn native_field_cannon_sighting_survives_fresh_board_fog_without_exact_reading() {
    let (snapshot, mut state) = fixture();
    let mut history = History::default();
    let mut seen = board(&snapshot, &state);
    history.observe_and_apply(&mut seen, &state);
    assert_eq!(fire(&seen), 60.0);
    state.turn = 137;
    state.rivals[0].units.clear();
    let mut fogged = board(&snapshot, &state);
    assert_eq!(
        fire(&fogged),
        3.0,
        "regression: a fresh board loses history"
    );
    history.observe_and_apply(&mut fogged, &state);
    assert_eq!(fire(&fogged), 60.0);
    assert!(fogged.game.observed_city_ranged_strength.is_empty());
    assert!(fogged.foreign_uid_of.is_empty(), "no phantom hidden cannon");
    let old_damage = civvis::game::expected_damage(3.0, 30.0);
    let remembered_damage = civvis::game::expected_damage(fire(&fogged), 30.0);
    assert!(old_damage < 12.0);
    assert!(
        remembered_damage > 99.0,
        "price the observed weapon against a Crossbow"
    );
}

#[test]
fn current_exact_city_values_including_zero_override_history() {
    let (snapshot, mut state) = fixture();
    let mut history = History::default();
    history.observe_and_apply(&mut board(&snapshot, &state), &state);
    state.rivals[0].units.clear();
    for strength in [70.0, 0.0, 20.0] {
        state.rivals[0].cities[0].ranged_strength = Some(strength);
        let mut current = board(&snapshot, &state);
        history.observe_and_apply(&mut current, &state);
        assert_eq!(fire(&current), strength);
    }
    state.rivals[0].cities[0].ranged_strength = None;
    let mut current = board(&snapshot, &state);
    history.observe_and_apply(&mut current, &state);
    assert_eq!(
        fire(&current),
        60.0,
        "exact city values never enter history"
    );
}

#[test]
fn history_uses_nominal_type_not_formation_or_simulated_counter() {
    let (snapshot, mut state) = fixture();
    state.rivals[0].units[0].formation = 2;
    state.rivals[0].units[0].ranged = 102.0;
    let mut history = History::default();
    let mut seen = board(&snapshot, &state);
    seen.game.players[2]
        .counters
        .insert("strongest_ranged_built".into(), 150);
    history.observe_and_apply(&mut seen, &state);
    state.rivals[0].units.clear();
    let mut current = board(&snapshot, &state);
    history.observe_and_apply(&mut current, &state);
    assert_eq!(fire(&current), 60.0);
}

#[test]
fn native_identity_survives_met_roster_reordering_and_nonzero_local_seat() {
    let (snapshot, mut state) = fixture();
    state.seat.local_player = 3;
    let mut history = History::default();
    history.observe_and_apply(&mut board(&snapshot, &state), &state);
    state.rivals[0].units.clear();
    state.rivals.insert(
        0,
        StateRival {
            player: 0,
            civ: "CIVILIZATION_PERSIA".into(),
            ..Default::default()
        },
    );
    let mut current = board(&snapshot, &state);
    history.observe_and_apply(&mut current, &state);
    assert_eq!(fire(&current), 60.0);
    assert!(!current.game.players[1]
        .counters
        .contains_key("strongest_ranged_built"));
}

#[test]
fn owner_identity_changes_and_turn_rewind_do_not_inherit_history() {
    let (snapshot, mut state) = fixture();
    let mut history = History::default();
    history.observe_and_apply(&mut board(&snapshot, &state), &state);
    state.rivals[0].units.clear();
    state.rivals[0].civ = "CIVILIZATION_PERSIA".into();
    let mut current = board(&snapshot, &state);
    history.observe_and_apply(&mut current, &state);
    assert_eq!(fire(&current), 3.0);
    state.rivals[0].civ = "CIVILIZATION_NUBIA".into();
    state.turn = 1;
    let mut current = board(&snapshot, &state);
    history.observe_and_apply(&mut current, &state);
    assert_eq!(fire(&current), 3.0);
}

#[test]
fn unseen_unlocks_and_exact_city_only_sightings_do_not_create_unit_history() {
    let (snapshot, mut state) = fixture();
    state.rivals[0].units.clear();
    state.rivals[0].tech_names = vec!["TECH_ADVANCED_BALLISTICS".into()];
    state.rivals[0].cities[0].ranged_strength = Some(102.0);
    let mut history = History::default();
    history.observe_and_apply(&mut board(&snapshot, &state), &state);
    state.rivals[0].cities[0].ranged_strength = None;
    let mut current = board(&snapshot, &state);
    history.observe_and_apply(&mut current, &state);
    assert_eq!(fire(&current), 3.0);
}

#[test]
fn weaker_later_sightings_do_not_erase_history_and_current_policy_is_not_cached() {
    let (snapshot, mut state) = fixture();
    let mut history = History::default();
    history.observe_and_apply(&mut board(&snapshot, &state), &state);
    state.rivals[0].units[0].kind = "UNIT_ARCHER".into();
    state.rivals[0].units[0].ranged = 25.0;
    let mut current = board(&snapshot, &state);
    current.game.players[2]
        .policies
        .insert(civvis::name!("bastions"));
    history.observe_and_apply(&mut current, &state);
    assert_eq!(fire(&current), 65.0);
    state.rivals[0].units.clear();
    let mut current = board(&snapshot, &state);
    history.observe_and_apply(&mut current, &state);
    assert_eq!(fire(&current), 60.0);
}

#[test]
fn own_units_are_remembered_but_unmapped_foreign_units_are_not() {
    let (snapshot, mut state) = fixture();
    state.units = state.rivals[0].units.clone();
    state.rivals[0].units[0].kind = "UNIT_DOES_NOT_EXIST".into();
    let mut history = History::default();
    history.observe_and_apply(&mut board(&snapshot, &state), &state);
    state.units.clear();
    state.rivals[0].units.clear();
    let mut current = board(&snapshot, &state);
    history.observe_and_apply(&mut current, &state);
    assert_eq!(
        current.game.players[0]
            .counters
            .get("strongest_ranged_built"),
        Some(&60)
    );
    assert_eq!(fire(&current), 3.0);
}
