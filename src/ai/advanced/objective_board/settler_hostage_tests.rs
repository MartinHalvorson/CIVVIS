//! `raiders-holding-a-settler-are-hunted`: the hostiles that hold a Settler
//! of ours in the field get a Destroy row priced with the Settler.

use super::tests::{at, conquest, flat_board, on};
use super::*;
use crate::ai::advanced::raiders_holding_a_settler::SETTLER_HOLD_RADIUS;

fn destroy_rows(ai: &AdvancedAi) -> Vec<Objective> {
    ai.objective_board()
        .rows
        .iter()
        .filter(|row| row.kind == ObjectiveKind::Destroy)
        .cloned()
        .collect()
}

fn spawn_ready(g: &mut Game, kind: &str, pid: usize, pos: Pos) -> u32 {
    let uid = g.spawn_test_unit(kind, pid, pos);
    let moves = g.unit_max_moves(uid);
    let unit = g.units.get_mut(&uid).unwrap();
    unit.moves_left = moves;
    unit.attacks_left = 1;
    uid
}

fn assess(g: &Game, gene: bool) -> AdvancedAi {
    let mut ai = on();
    if gene {
        ai.enable_raiders_holding_a_settler_are_hunted();
    }
    let plan = conquest(g, None);
    ai.rebuild_force_groups(g, 0, &plan);
    ai
}

/// Our capital at (6, 8), a Settler of ours walking out at (16, 8), ten tiles
/// from it, and a barbarian Warrior two tiles beyond the Settler: far from
/// every city, so the plain board never sees it.
fn far_raider_board() -> (Game, u32) {
    let mut g = flat_board(23, &[at(6, 8), at(32, 16)], true);
    let barbarians = g.barb_pid.expect("a barbarian player");
    spawn_ready(&mut g, "warrior", 0, at(7, 8));
    spawn_ready(&mut g, "warrior", 0, at(7, 9));
    spawn_ready(&mut g, "settler", 0, at(16, 8));
    let raider = spawn_ready(&mut g, "warrior", barbarians, at(18, 8));
    assert!(g.wdist(at(6, 8), at(18, 8)) > THREAT_RELIEF_RADIUS);
    assert!(g.wdist(at(16, 8), at(18, 8)) <= SETTLER_HOLD_RADIUS);
    (g, raider)
}

#[test]
fn raiders_holding_a_settler_are_hunted_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(
        !ai.raiders_holding_a_settler_are_hunted,
        "an opt-in ships off"
    );
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "raiders_holding_a_settler_are_hunted"));
    super::super::test_support::opt_in_off_in_both_controllers(
        "raiders-holding-a-settler-are-hunted",
        |ai| ai.raiders_holding_a_settler_are_hunted,
    );
}

/// A barbarian far from every city but beside our Settler: no row without
/// the gene; with it, a Destroy row worth the Settler's Escort plus the
/// Warrior, which outranks the Warrior's own worth.
#[test]
fn a_raider_beside_a_settler_in_the_field_gets_a_destroy_row_worth_the_settler() {
    let (g, raider) = far_raider_board();
    let plain = assess(&g, false);
    assert!(
        destroy_rows(&plain).is_empty(),
        "the plain board ignores a barbarian far from our cities"
    );
    let hunted = assess(&g, true);
    let rows = destroy_rows(&hunted);
    let row = rows
        .iter()
        .find(|row| row.key == ObjectiveKey::Destroy(raider))
        .expect("a Destroy row against the raider");
    let settler = g.rules.units.get("settler").unwrap().cost + SETTLER_PREMIUM;
    let warrior = unit_value(&g, raider);
    assert!(
        (row.value - (settler + warrior)).abs() < 1e-6,
        "the row carries the Settler: {} vs {} + {}",
        row.value,
        settler,
        warrior
    );
    assert!(
        row.requirement.strength > 0.0,
        "the row keeps its strength need"
    );
}

/// The same raider with the Settler back inside the capital holds nothing:
/// the gene adds no row.
#[test]
fn a_settler_inside_a_city_holds_no_raider() {
    let mut g = flat_board(23, &[at(6, 8), at(32, 16)], true);
    let barbarians = g.barb_pid.expect("a barbarian player");
    spawn_ready(&mut g, "warrior", 0, at(7, 8));
    spawn_ready(&mut g, "settler", 0, at(6, 8));
    spawn_ready(&mut g, "warrior", barbarians, at(16, 8));
    assert!(destroy_rows(&assess(&g, true)).is_empty());
}

/// A raider beyond the hold radius of the Settler is left to the plain
/// rules.
#[test]
fn a_raider_beyond_the_hold_radius_is_left_alone() {
    let mut g = flat_board(23, &[at(6, 8), at(32, 16)], true);
    let barbarians = g.barb_pid.expect("a barbarian player");
    spawn_ready(&mut g, "warrior", 0, at(7, 8));
    spawn_ready(&mut g, "settler", 0, at(14, 8));
    spawn_ready(&mut g, "warrior", barbarians, at(20, 8));
    assert!(g.wdist(at(14, 8), at(20, 8)) > SETTLER_HOLD_RADIUS);
    assert!(destroy_rows(&assess(&g, true)).is_empty());
}
