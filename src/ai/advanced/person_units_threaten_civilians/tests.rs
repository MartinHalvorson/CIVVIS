//! `person-units-threaten-civilians`: the host's `person` mark crosses into
//! the board, and a person's soldier at peace threatens our civilians in
//! every reader the gene names, only with the gene on and the mark present.
//!
//! The boards copy the live one of `hotseat-20261128T031456Z` p1 turns
//! 10-11: our Settler two tiles from the person's soldier, at peace.

use super::*;
use crate::game::Game;
use crate::mirror::{apply_person_rivals, state_from_json, StateRival, StateSnapshot};
use crate::Pos;
use std::collections::BTreeMap;
use std::sync::Arc;

/// A flat grassland world of two empires at peace, every starting unit
/// cleared, turn 60.
fn world() -> Game {
    let mut g = Game::new_full(2, 36, 22, 7_411, 1_000, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    g.at_war.clear();
    g.turn = 60;
    g
}

fn at(col: i32, row: i32) -> Pos {
    crate::hex::offset_to_axial(col, row)
}

fn spawn_ready(g: &mut Game, kind: &str, pid: usize, pos: Pos) -> u32 {
    let uid = g.spawn_test_unit(kind, pid, pos);
    let moves = g.unit_max_moves(uid);
    let unit = g.units.get_mut(&uid).unwrap();
    unit.moves_left = moves;
    unit.attacks_left = 1;
    uid
}

/// What the hotseat mod exports for seat 1: a person plays it.
fn mark_person(g: &mut Game, seat: usize) {
    Arc::make_mut(&mut g.observed_person_rivals).insert(seat);
}

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    if gene {
        ai.enable_person_units_threaten_civilians();
    }
    ai
}

fn risk(ai: &AdvancedAi, g: &Game, uid: Option<u32>, pos: Pos) -> f64 {
    let visible = ai.battlefront_visibility(g, 0);
    ai.settlement_tile_risk(g, 0, uid, pos, &visible)
}

#[test]
fn person_units_threaten_civilians_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(!ai.person_units_threaten_civilians, "an opt-in ships off");
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "person_units_threaten_civilians"));
    super::super::test_support::opt_in_off_in_both_controllers(
        "person-units-threaten-civilians",
        |ai| ai.person_units_threaten_civilians,
    );
}

/// `rivals[].person` crosses without a schema gap; a rival the export marks
/// is a person seat, our own seat and an unmapped player never are, and the
/// set is read afresh from every export.
#[test]
fn the_person_mark_crosses_into_its_own_set() {
    for (raw, expected) in [
        (r#"{}"#, None),
        (r#"{"person":true}"#, Some(true)),
        (r#"{"person":false}"#, Some(false)),
    ] {
        let state = state_from_json(&format!(r#"{{"turn":12,"rivals":[{raw}]}}"#)).unwrap();
        assert!(state.schema_gaps.is_empty(), "{:?}", state.schema_gaps);
        assert_eq!(state.rivals[0].person, expected);
    }
    let mut game = Game::new_full(4, 24, 16, 7_923, 300, 0, false);
    // Host ids 10, 11, 12 are seats 1, 2, 3; our host id 9 is seat 0.
    let seat_of_host: BTreeMap<usize, usize> =
        [(9, 0), (10, 1), (11, 2), (12, 3)].into_iter().collect();
    let rival = |player: usize, person: Option<bool>| StateRival {
        player,
        person,
        ..StateRival::default()
    };
    let mut state = StateSnapshot {
        rivals: vec![
            rival(10, Some(true)),
            rival(11, None),
            rival(12, Some(false)),
            rival(40, Some(true)),
        ],
        ..StateSnapshot::default()
    };
    apply_person_rivals(&mut game, &state, &seat_of_host);
    assert_eq!(
        game.observed_person_rivals
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![1]
    );
    state.rivals = vec![rival(9, Some(true)), rival(11, Some(true))];
    apply_person_rivals(&mut game, &state, &seat_of_host);
    assert_eq!(
        game.observed_person_rivals
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![2],
        "our own seat is never a person rival, and the set is rebuilt"
    );
    state.rivals.clear();
    apply_person_rivals(&mut game, &state, &seat_of_host);
    assert!(game.observed_person_rivals.is_empty());
}

/// Our Settler two tiles from a person's Warrior at peace: no risk without
/// the gene, none with it on a board that marks no person (a ladder board),
/// capture-tier risk with both, for the Settler's step and for the site.
#[test]
fn a_persons_warrior_at_peace_puts_our_settler_at_risk() {
    let mut g = world();
    let settler = spawn_ready(&mut g, "settler", 0, at(10, 8));
    spawn_ready(&mut g, "warrior", 1, at(12, 8));
    assert!(!g.is_at_war(0, 1));
    let on = seat(true);
    assert_eq!(risk(&seat(false), &g, Some(settler), at(10, 8)), 0.0);
    assert_eq!(
        risk(&on, &g, Some(settler), at(10, 8)),
        0.0,
        "no rival carries the person mark: the gene is a no-op"
    );
    mark_person(&mut g, 1);
    assert_eq!(risk(&seat(false), &g, Some(settler), at(10, 8)), 0.0);
    assert!(risk(&on, &g, Some(settler), at(10, 8)) > 0.0, "the step");
    assert!(risk(&on, &g, None, at(10, 8)) > 0.0, "the site");
    assert!(on.civilian_threat_owner(&g, 0, 1));
    assert!(!seat(false).civilian_threat_owner(&g, 0, 1));
    assert!(!on.civilian_threat_owner(&g, 0, 0), "never ourselves");

    // Beyond capture reach and the approach tier: no risk.
    let mut far = world();
    let settler = spawn_ready(&mut far, "settler", 0, at(10, 8));
    spawn_ready(&mut far, "warrior", 0, at(13, 8));
    spawn_ready(&mut far, "warrior", 1, at(15, 8));
    mark_person(&mut far, 1);
    assert_eq!(risk(&on, &far, Some(settler), at(10, 8)), 0.0);
}

/// The unstacked Settler's next step toward its site lies inside the
/// person's Warrior's moves: capturable only with the gene on and the mark.
#[test]
fn the_unstacked_settlers_step_is_capturable_by_a_person() {
    let mut g = world();
    let settler = spawn_ready(&mut g, "settler", 0, at(10, 8));
    spawn_ready(&mut g, "warrior", 1, at(13, 8));
    mark_person(&mut g, 1);
    let mut off = seat(false);
    let mut on = seat(true);
    off.settler_targets.insert(settler, at(16, 8));
    on.settler_targets.insert(settler, at(16, 8));
    assert!(!off.unstacked_settler_step_is_capturable(&g, 0, settler));
    assert!(on.unstacked_settler_step_is_capturable(&g, 0, settler));
    Arc::make_mut(&mut g.observed_person_rivals).clear();
    assert!(
        !on.unstacked_settler_step_is_capturable(&g, 0, settler),
        "a CIVVIS seat at peace is not a person"
    );
}

/// `settler-marches-alone-when-clear` reads a person's soldier beside the
/// road as a hostile, only with the gene on and the mark.
#[test]
fn a_persons_soldier_by_the_road_is_not_a_clear_road() {
    let mut g = world();
    spawn_ready(&mut g, "settler", 0, at(10, 8));
    spawn_ready(&mut g, "warrior", 0, at(12, 8));
    spawn_ready(&mut g, "scout", 1, at(13, 8));
    mark_person(&mut g, 1);
    let clear = |ai: &AdvancedAi| {
        let visible = ai.battlefront_visibility(&g, 0);
        ai.road_is_clear(&g, 0, at(10, 8), at(16, 8), 2, &visible)
    };
    assert!(clear(&seat(false)), "off: a rival at peace is no hostile");
    assert!(!clear(&seat(true)), "on: the person's Scout holds the road");
}
