//! `taken-settler-is-retaken` on the objective board: the force standing on
//! a civilian taken from us gets a Destroy row priced with the civilian.

use super::tests::{at, conquest, flat_board, on, war};
use super::*;

fn destroy_row(ai: &AdvancedAi, hostile: u32) -> Option<Objective> {
    ai.objective_board()
        .rows
        .iter()
        .find(|row| row.key == ObjectiveKey::Destroy(hostile))
        .cloned()
}

fn spawn_ready(g: &mut Game, kind: &str, pid: usize, pos: Pos) -> u32 {
    let uid = g.spawn_test_unit(kind, pid, pos);
    let moves = g.unit_max_moves(uid);
    let unit = g.units.get_mut(&uid).unwrap();
    unit.moves_left = moves;
    unit.attacks_left = 1;
    uid
}

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = on();
    if gene {
        ai.enable_taken_settler_is_retaken();
    }
    ai
}

/// Our Settler at `settler` beside our Warrior at `guard`; next turn the
/// rebuilt board shows `captor`'s `kind` and a Settler of theirs on that
/// tile instead. Returns the captor's soldier.
fn take(
    g: &mut Game,
    ai: &mut AdvancedAi,
    guard: Pos,
    settler: Pos,
    captor: usize,
    kind: &str,
) -> u32 {
    spawn_ready(g, "warrior", 0, guard);
    let ours = spawn_ready(g, "settler", 0, settler);
    ai.observe_taken_civilians(g, 0);
    g.remove_unit(ours);
    let soldier = spawn_ready(g, kind, captor, settler);
    spawn_ready(g, "settler", captor, settler);
    g.turn += 1;
    ai.observe_taken_civilians(g, 0);
    soldier
}

/// The person's Scout three tiles from our capital, on the Settler it took:
/// the Destroy row it already had now carries the Settler too.
#[test]
fn the_captors_destroy_row_carries_the_settler() {
    let settler = settler_worth();
    for gene in [false, true] {
        let mut g = flat_board(23, &[at(6, 8), at(32, 16)], false);
        war(&mut g, 0, 1);
        let mut ai = seat(gene);
        let scout = take(&mut g, &mut ai, at(7, 8), at(9, 8), 1, "scout");
        let plan = conquest(&g, None);
        ai.rebuild_force_groups(&g, 0, &plan);
        let own = unit_value(&g, scout);
        match destroy_row(&ai, scout) {
            Some(row) => {
                let expected = if gene { own + settler } else { own };
                assert!(
                    (row.value - expected).abs() < 1e-6,
                    "gene {gene}: the row is worth {} (want {expected})",
                    row.value
                );
                assert!(row.requirement.strength > 0.0, "the row keeps its need");
            }
            // Off, a Defend row on the capital may cover the Scout; on, a
            // Defend row covers nothing that holds our Settler.
            None => assert!(!gene, "with the gene the captor always has its row"),
        }
    }
}

/// A barbarian far from every city of ours is not on the board; standing on
/// a Settler it took from us, it is, worth the Settler.
#[test]
fn a_far_barbarian_holding_our_settler_is_on_the_board() {
    let settler = settler_worth();
    for gene in [false, true] {
        let mut g = flat_board(23, &[at(6, 8), at(32, 16)], true);
        let barbarians = g.barb_pid.expect("a barbarian player");
        let mut ai = seat(gene);
        let raider = take(&mut g, &mut ai, at(16, 8), at(18, 8), barbarians, "warrior");
        assert!(g.wdist(at(6, 8), at(18, 8)) > THREAT_RELIEF_RADIUS);
        let plan = conquest(&g, None);
        ai.rebuild_force_groups(&g, 0, &plan);
        let row = destroy_row(&ai, raider);
        if gene {
            let row = row.expect("the captor is on the board");
            assert!((row.value - (unit_value(&g, raider) + settler)).abs() < 1e-6);
        } else {
            assert!(row.is_none(), "the plain board ignores a far barbarian");
        }
    }
}

fn settler_worth() -> f64 {
    let g = flat_board(23, &[at(6, 8), at(32, 16)], false);
    g.rules.units.get("settler").unwrap().cost + SETTLER_PREMIUM
}
