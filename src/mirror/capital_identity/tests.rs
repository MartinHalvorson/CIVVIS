use super::*;
use crate::game::{Action, Game};

fn city(id: i64, name: &str, x: i32, original: bool, current: bool, founder: i64) -> StateCity {
    StateCity {
        id,
        name: name.into(),
        x,
        y: 8,
        pop: 8,
        loyalty: 100.0,
        loyalty_per_turn: 0.0,
        capital: current,
        original_capital: Some(original),
        original_owner: Some(founder),
        ..Default::default()
    }
}

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
        turn: 120,
        width: 26,
        height: 18,
        chunk: 1,
        plots,
    }]);
    let state = StateSnapshot {
        turn: 120,
        seat: Seat {
            players: 3,
            local_player: 0,
            ..Default::default()
        },
        cities: vec![
            city(1, "Home", 3, true, true, 0),
            city(2, "London", 8, true, false, 1),
        ],
        rivals: vec![
            StateRival {
                player: 1,
                cities: vec![city(3, "Plymouth", 13, false, true, 1)],
                ..Default::default()
            },
            StateRival {
                player: 2,
                cities: vec![city(4, "Next", 21, true, true, 2)],
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    (snapshot, state)
}

fn named(game: &Game, name: &str) -> u32 {
    game.cities
        .values()
        .find(|city| city.name == name)
        .unwrap()
        .id
}

#[test]
fn captured_original_capital_counts_without_an_extra_palace_or_raze_option() {
    let (snapshot, mut state) = fixture();
    state.cities[1].captured_from = Some(1);
    let game = rebuild_from_state(&snapshot, &state, 3, 364000, 500, 0).game;
    let london = named(&game, "London");
    let plymouth = named(&game, "Plymouth");
    assert!(game.cities[&london].is_capital);
    assert_eq!(game.cities[&london].original_owner, 1);
    assert!(!game.city_has_palace(&game.cities[&london]));
    assert!(!game.cities[&plymouth].is_capital);
    assert!(game.city_has_palace(&game.cities[&plymouth]));
    assert_eq!(game.victory_races(0, 0).controlled_capitals, 2);
    let actions = game.legal_city_disposition_actions(0);
    assert!(actions.contains(&Action::KeepCity { city: london }));
    assert!(!actions.contains(&Action::RazeCity { city: london }));
}

#[test]
fn rival_occupier_keeps_the_original_founder_and_sync_tracks_recapture() {
    let (snapshot, mut state) = fixture();
    let london = state.cities.pop().unwrap();
    state.rivals[1].cities.push(london);
    let mut mirror = LiveMirror::new(&snapshot, &state, 3, 364001, 500, 0);
    let id = named(&mirror.game, "London");
    assert_eq!(
        (
            mirror.game.cities[&id].owner,
            mirror.game.cities[&id].original_owner
        ),
        (2, 1)
    );
    assert!(mirror.game.cities[&id].is_capital);
    assert!(!mirror.game.city_has_palace(&mirror.game.cities[&id]));
    assert_eq!(mirror.game.victory_races(0, 0).controlled_capitals, 1);
    // Native Palace need not move back when the founder regains London.
    let london = state.rivals[1].cities.pop().unwrap();
    state.rivals[0].cities.push(london);
    mirror.sync(&snapshot, &state, 0);
    let id = named(&mirror.game, "London");
    assert_eq!(
        (
            mirror.game.cities[&id].owner,
            mirror.game.cities[&id].original_owner
        ),
        (1, 1)
    );
    assert!(mirror.game.cities[&id].is_capital);
    assert!(!mirror.game.city_has_palace(&mirror.game.cities[&id]));
    assert!(mirror
        .game
        .city_has_palace(&mirror.game.cities[&named(&mirror.game, "Plymouth")]));
}

#[test]
fn current_capital_alone_does_not_complete_domination() {
    let (snapshot, mut state) = fixture();
    let london = state.cities.pop().unwrap();
    let mut plymouth = state.rivals[0].cities.pop().unwrap();
    plymouth.capital = false;
    state.cities.push(plymouth);
    state.rivals[0].cities.push(london);
    let game = rebuild_from_state(&snapshot, &state, 3, 364002, 500, 0).game;
    assert_eq!(game.victory_races(0, 0).controlled_capitals, 1);
    assert!(!game.cities[&named(&game, "Plymouth")].is_capital);
}

#[test]
fn nonzero_host_seat_maps_founders_and_unknown_founder_gets_no_foreign_credit() {
    let (snapshot, mut state) = fixture();
    state.seat.local_player = 2;
    state.cities[0].original_owner = Some(2);
    state.rivals[1].player = 0;
    state.rivals[1].cities[0].original_owner = Some(0);
    let game = rebuild_from_state(&snapshot, &state, 3, 364003, 500, 0).game;
    assert_eq!(game.cities[&named(&game, "Home")].original_owner, 0);
    assert_eq!(game.cities[&named(&game, "London")].original_owner, 2);
    assert_eq!(game.cities[&named(&game, "Next")].original_owner, 1);
    state.cities[1].original_owner = Some(999);
    let game = rebuild_from_state(&snapshot, &state, 3, 364003, 500, 0).game;
    assert!(game.cities[&named(&game, "London")].is_capital);
    assert_eq!(game.cities[&named(&game, "London")].original_owner, 0);
    assert_eq!(
        game.victory_races(0, 0).controlled_capitals,
        1,
        "an unmapped founder must not invent credit for a foreign capital"
    );
}

#[test]
fn legacy_exports_and_saves_keep_their_fallback_and_do_not_retain_stale_palaces() {
    let (snapshot, mut state) = fixture();
    let mut mirror = LiveMirror::new(&snapshot, &state, 3, 364004, 500, 0);
    let decoded: Game =
        serde_json::from_str(&serde_json::to_string(&mirror.game).unwrap()).unwrap();
    assert!(!decoded.city_has_palace(&decoded.cities[&named(&decoded, "London")]));
    let mut legacy = serde_json::to_value(&decoded).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("observed_city_palaces");
    let decoded: Game = serde_json::from_value(legacy).unwrap();
    assert!(decoded.observed_city_palaces.is_empty());
    for city in state.cities.iter_mut().chain(
        state
            .rivals
            .iter_mut()
            .flat_map(|rival| rival.cities.iter_mut()),
    ) {
        city.original_capital = None;
    }
    mirror.sync(&snapshot, &state, 0);
    assert!(mirror.game.observed_city_palaces.is_empty());
    assert!(!mirror.game.cities[&named(&mirror.game, "London")].is_capital);
    assert!(mirror.game.cities[&named(&mirror.game, "Plymouth")].is_capital);
}

#[test]
fn hypothetical_liberation_releases_observed_palace_placement_for_affected_seats() {
    let (snapshot, mut state) = fixture();
    state.cities[1].captured_from = Some(2);
    let mut game = rebuild_from_state(&snapshot, &state, 3, 364005, 500, 0).game;
    let london = named(&game, "London");
    game.apply(0, &Action::LiberateCity { city: london })
        .unwrap();
    assert_eq!(game.cities[&london].owner, 1);
    assert!(game.city_has_palace(&game.cities[&london]));
    assert!(!game.city_has_palace(&game.cities[&named(&game, "Plymouth")]));
    assert_eq!(
        game.observed_city_palaces.get(&named(&game, "Next")),
        Some(&true)
    );
}

#[test]
fn palace_yields_and_loyalty_pressure_match_the_legacy_current_capital_board() {
    let (snapshot, mut state) = fixture();
    let mut modern = rebuild_from_state(&snapshot, &state, 3, 364007, 500, 0).game;
    for city in state.cities.iter_mut().chain(
        state
            .rivals
            .iter_mut()
            .flat_map(|rival| rival.cities.iter_mut()),
    ) {
        city.original_capital = None;
    }
    let mut legacy = rebuild_from_state(&snapshot, &state, 3, 364007, 500, 0).game;
    // Test the model itself rather than equality forced by host corrections.
    for game in [&mut modern, &mut legacy] {
        Arc::make_mut(&mut game.observed_city_loyalty_per_turn).clear();
        Arc::make_mut(&mut game.observed_city_yield_adjustments).clear();
        Arc::make_mut(&mut game.observed_yield_adjustments).clear();
    }
    for name in ["Home", "London", "Plymouth", "Next"] {
        let m = named(&modern, name);
        let l = named(&legacy, name);
        assert_eq!(
            modern.city_yields(m),
            legacy.city_yields(l),
            "{name} yields"
        );
        assert_eq!(
            modern.city_loyalty_per_turn(&modern.cities[&m]),
            legacy.city_loyalty_per_turn(&legacy.cities[&l]),
            "{name} loyalty pressure"
        );
    }
}
