use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{GrandStrategy, VictoryTarget};

/// Our one city at turn 20 on a fresh board, no religion founded anywhere.
fn opening() -> Game {
    let mut g = Game::new_full(2, 40, 24, 936_711, 650, 0, false);
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| g.units[unit].kind == "settler")
            .unwrap();
        g.current = pid;
        g.apply(pid, &Action::FoundCity { unit: settler }).unwrap();
    }
    g.current = 0;
    g.turn = 20;
    g
}

#[test]
fn religion_race_keeps_expanding_is_opt_in() {
    opt_in_off_in_both_controllers("religion-race-keeps-expanding", |ai| {
        ai.religion_race_keeps_expanding
    });
}

/// Off, the racing Religion seat plans Religion; on, with one city of the
/// several it wants, it plans Expansion.
#[test]
fn the_racing_religion_seat_expands_first() {
    let g = opening();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
    assert_eq!(
        ai.assess(&g, 0).strategy,
        GrandStrategy::Religion,
        "off: the race takes the plan"
    );
    ai.enable_religion_race_keeps_expanding();
    assert_eq!(
        ai.assess(&g, 0).strategy,
        GrandStrategy::Expansion,
        "on: the city target comes first"
    );
}

#[test]
fn no_room_to_expand_keeps_the_race() {
    let mut ai = AdvancedAi::new();
    ai.enable_religion_race_keeps_expanding();
    assert!(ai.religion_race_expands_first(true));
    assert!(!ai.religion_race_expands_first(false));
    assert!(!AdvancedAi::new().religion_race_expands_first(true), "off");
}
