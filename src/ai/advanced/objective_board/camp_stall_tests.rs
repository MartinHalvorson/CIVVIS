//! `camp-stall-goes-home`: a ClearCamp party that strikes no blow for three
//! turns goes home for ten, and sites beside that camp lose value.

use super::tests::{at, conquest, flat_board, on};
use super::*;
use crate::ai::advanced::camp_stall_goes_home::{
    CAMP_STALL_CLOSE_TURNS, CAMP_STALL_HOME_WATCH, CAMP_STALL_SITE_PENALTY, CAMP_STALL_TURNS,
};
use crate::ai::Ai;
use crate::game::Action;

fn row(ai: &AdvancedAi, key: ObjectiveKey) -> Option<&Objective> {
    ai.objective_board().rows.iter().find(|row| row.key == key)
}

fn camp_force(ai: &AdvancedAi, camp: Pos) -> Option<&TaskForce> {
    ai.objective_board()
        .forces
        .iter()
        .find(|force| force.objective_key == ObjectiveKey::Camp(camp))
}

/// A new turn's movement and attack for `uid`.
fn fresh_turn(g: &mut Game, uid: u32) {
    let moves = g.unit_max_moves(uid);
    let unit = g.units.get_mut(&uid).unwrap();
    unit.moves_left = moves;
    unit.attacks_left = 1;
    unit.moved = false;
    unit.acted = false;
}

/// Seat p4's camp of `hotseat-20261128T031456Z` moved onto the test board:
/// Tyre's camp at civ6 (45,20) stood five tiles from the city and a
/// barbarian Spearman held it from turn 10; here a camp five tiles east of
/// our capital at (6,8), a barbarian Spearman on it unless `guarded` is
/// false, and two of our Warriors `apart` tiles from it. Returns the board,
/// the camp, our Warriors and the seat, gene on or off.
fn camp_board(gene: bool, guarded: bool, apart: i32) -> (Game, Pos, Vec<u32>, AdvancedAi) {
    let mut g = flat_board(17, &[at(6, 8), at(30, 8)], true);
    let camp = at(11, 8);
    g.barb_camps.insert(camp, g.turn);
    if guarded {
        let barb = g.barb_pid.expect("barbarians");
        g.spawn_test_unit("spearman", barb, camp);
    }
    let mut ring: Vec<Pos> = g
        .wdisk(camp, apart)
        .into_iter()
        .filter(|pos| {
            g.wdist(*pos, camp) == apart
                && g.city_at(*pos).is_none()
                && g.unit_ids_at(*pos).is_empty()
                && g.wdist(*pos, at(6, 8)) <= g.wdist(camp, at(6, 8))
        })
        .collect();
    ring.sort();
    let warriors: Vec<u32> = ring
        .iter()
        .take(2)
        .map(|pos| {
            let uid = g.spawn_test_unit("warrior", 0, *pos);
            fresh_turn(&mut g, uid);
            uid
        })
        .collect();
    assert_eq!(warriors.len(), 2, "two tiles for the party");
    let mut ai = on();
    if gene {
        ai.enable_camp_stall_goes_home();
    }
    ai.base.journal = crate::reasoning::Journal::recording();
    (g, camp, warriors, ai)
}

/// One turn of the board: drawn, then the end of our unit pass with a blow
/// at the camp when `strike` names the unit that struck it, then the next
/// turn. Whether the turn's board raised the camp's row.
fn a_turn(g: &mut Game, ai: &mut AdvancedAi, camp: Pos, strike: Option<u32>) -> bool {
    let plan = conquest(g, None);
    ai.rebuild_force_groups(g, 0, &plan);
    let raised = row(ai, ObjectiveKey::Camp(camp)).is_some();
    let begins = g.log.len();
    if let Some(unit) = strike {
        g.log.push(0, Action::Attack { unit, target: camp });
    }
    ai.observe_camp_stalls(g, 0, begins);
    g.turn += 1;
    raised
}

fn gene_lines(ai: &AdvancedAi) -> Vec<String> {
    ai.base
        .journal
        .since(u64::MAX)
        .thoughts
        .into_iter()
        .filter(|thought| thought.detail.contains("camp-stall-goes-home:"))
        .map(|thought| format!("{} | {}", thought.headline, thought.detail))
        .collect()
}

#[test]
fn camp_stall_goes_home_ships_off_and_is_registered() {
    super::super::test_support::opt_in_off_in_both_controllers("camp-stall-goes-home", |ai| {
        ai.camp_stall_goes_home
    });
}

#[test]
fn the_party_beside_the_camp_serves_its_row() {
    let (g, camp, warriors, mut ai) = camp_board(true, true, 1);
    let plan = conquest(&g, None);
    ai.rebuild_force_groups(&g, 0, &plan);
    assert!(
        row(&ai, ObjectiveKey::Camp(camp)).is_some(),
        "a ClearCamp row"
    );
    let force = camp_force(&ai, camp).expect("a party for the camp");
    for uid in &warriors {
        assert!(
            force.units.contains(uid),
            "{uid} serves the camp: {force:?}"
        );
    }
}

#[test]
fn three_turns_without_a_blow_close_the_row_for_ten_and_the_party_goes_home() {
    let (mut g, camp, warriors, mut ai) = camp_board(true, true, 1);
    let start = g.turn;
    for _ in 0..CAMP_STALL_TURNS {
        assert!(
            a_turn(&mut g, &mut ai, camp, None),
            "turn {} still raises it",
            g.turn
        );
    }
    let closed_on = g.turn;
    assert_eq!(closed_on, start + CAMP_STALL_TURNS);
    let plan = conquest(&g, None);
    ai.rebuild_force_groups(&g, 0, &plan);
    assert!(
        row(&ai, ObjectiveKey::Camp(camp)).is_none(),
        "the fourth turn raises no row"
    );
    assert!(camp_force(&ai, camp).is_none(), "and keeps no party there");
    let lines = gene_lines(&ai);
    assert!(
        lines.iter().any(|line| line.contains("goes home")
            && line.contains(&format!("turns {start}-{}", closed_on - 1))
            && line.contains(&format!(
                "until turn {}",
                closed_on + CAMP_STALL_CLOSE_TURNS
            ))),
        "{lines:#?}"
    );
    // The Warriors serve the board's other rows, here the Reserve.
    for uid in &warriors {
        assert!(
            ai.objective_board()
                .forces
                .iter()
                .any(|force| force.units.contains(uid)),
            "{uid} is in another force"
        );
    }
    // Closed for ten turns, whatever the party does meanwhile.
    for _ in 0..CAMP_STALL_CLOSE_TURNS {
        assert!(
            !a_turn(&mut g, &mut ai, camp, None),
            "turn {} is closed",
            g.turn
        );
    }
    assert_eq!(g.turn, closed_on + CAMP_STALL_CLOSE_TURNS);
    assert!(
        a_turn(&mut g, &mut ai, camp, None),
        "turn {} raises the row again",
        g.turn - 1
    );
    assert!(gene_lines(&ai)
        .iter()
        .any(|line| line.contains("a ClearCamp row again")));
    // A clean record: two more stalled turns do not close it.
    assert!(a_turn(&mut g, &mut ai, camp, None));
    assert!(a_turn(&mut g, &mut ai, camp, None));
}

#[test]
fn two_turns_without_a_blow_do_not_close_it() {
    let (mut g, camp, _, mut ai) = camp_board(true, true, 1);
    for _ in 0..CAMP_STALL_TURNS - 1 {
        assert!(a_turn(&mut g, &mut ai, camp, None));
    }
    assert!(
        a_turn(&mut g, &mut ai, camp, None),
        "the third turn is still open"
    );
}

#[test]
fn a_blow_starts_the_count_again() {
    let (mut g, camp, warriors, mut ai) = camp_board(true, true, 1);
    assert!(a_turn(&mut g, &mut ai, camp, None));
    assert!(a_turn(&mut g, &mut ai, camp, None));
    // The third turn a Warrior strikes the camp.
    assert!(a_turn(&mut g, &mut ai, camp, Some(warriors[0])));
    assert!(
        a_turn(&mut g, &mut ai, camp, None),
        "the blow keeps it open"
    );
    assert!(a_turn(&mut g, &mut ai, camp, None));
    assert!(
        a_turn(&mut g, &mut ai, camp, None),
        "two quiet turns since the blow are not three"
    );
    assert!(
        !a_turn(&mut g, &mut ai, camp, None),
        "three quiet turns since the blow close it"
    );
}

#[test]
fn off_the_party_stays_however_long_it_stands() {
    let (mut g, camp, warriors, mut ai) = camp_board(false, true, 1);
    for _ in 0..(CAMP_STALL_TURNS * 3) {
        assert!(a_turn(&mut g, &mut ai, camp, None), "turn {}", g.turn);
    }
    let plan = conquest(&g, None);
    ai.rebuild_force_groups(&g, 0, &plan);
    let force = camp_force(&ai, camp).expect("the party is still there");
    assert!(warriors.iter().all(|uid| force.units.contains(uid)));
    assert!(ai.camp_stalls.is_empty(), "nothing is written off");
    assert!(gene_lines(&ai).is_empty());
}

#[test]
fn an_unguarded_camp_or_a_party_out_of_reach_is_no_stall() {
    // Nobody on the camp tile: there was no blow to decline.
    let (mut g, camp, _, mut ai) = camp_board(true, false, 1);
    for _ in 0..(CAMP_STALL_TURNS + 2) {
        assert!(
            a_turn(&mut g, &mut ai, camp, None),
            "unguarded, turn {}",
            g.turn
        );
    }
    // A party still four tiles out is walking in, not standing.
    let (mut g, camp, _, mut ai) = camp_board(true, true, 4);
    for _ in 0..(CAMP_STALL_TURNS + 2) {
        assert!(a_turn(&mut g, &mut ai, camp, None), "far, turn {}", g.turn);
    }
}

#[test]
fn a_site_beside_a_closed_camp_loses_value_only_while_it_is_closed() {
    let (mut g, camp, _, mut ai) = camp_board(true, true, 1);
    let three = at(14, 8);
    let four = at(15, 8);
    assert_eq!(g.wdist(three, camp), 3);
    assert_eq!(g.wdist(four, camp), 4);
    assert_eq!(ai.camp_stall_site_penalty(&g, three), 0.0, "an open row");
    for _ in 0..CAMP_STALL_TURNS {
        a_turn(&mut g, &mut ai, camp, None);
    }
    assert!(!a_turn(&mut g, &mut ai, camp, None), "closed");
    assert_eq!(
        ai.camp_stall_site_penalty(&g, three),
        CAMP_STALL_SITE_PENALTY
    );
    assert_eq!(ai.camp_stall_site_penalty(&g, four), 0.0, "four tiles out");
    // The penalty is in the site value the Settler reads.
    let visible = ai.battlefront_visibility(&g, 0);
    let mut open = ai.clone();
    open.camp_stalls.clear();
    let closed_value = ai.settle_value_visible(&g, 0, three, &visible);
    let open_value = open.settle_value_visible(&g, 0, three, &visible);
    assert!(
        (open_value - closed_value - CAMP_STALL_SITE_PENALTY).abs() < 1e-9,
        "{open_value} open, {closed_value} closed"
    );
    // Gone with the camp, and with the gene off.
    let mut cleared = g.clone();
    cleared.barb_camps.remove(&camp);
    assert_eq!(ai.camp_stall_site_penalty(&cleared, three), 0.0);
    let mut off = ai.clone();
    off.disable_camp_stall_goes_home();
    assert_eq!(off.camp_stall_site_penalty(&g, three), 0.0);
    // And after the row reopens.
    g.turn += CAMP_STALL_CLOSE_TURNS;
    assert_eq!(ai.camp_stall_site_penalty(&g, three), 0.0);
}

/// The watch is wired into the unit pass: a whole turn of the seat with the
/// party beside the camp leaves the camp's record.
#[test]
fn a_whole_turn_writes_the_camps_record_and_off_writes_nothing() {
    for gene in [false, true] {
        let (mut g, camp, _, mut ai) = camp_board(gene, true, 1);
        ai.take_turn(&mut g, 0);
        assert_eq!(
            ai.camp_stalls.contains_key(&camp),
            gene,
            "gene {gene}: {:?}",
            ai.camp_stalls
        );
    }
}

/// The board on the turn a stall closes the camp's row: the party released
/// and reassigned, its force groups drawn.
fn closed_camp_board() -> (Game, Pos, Vec<u32>, AdvancedAi) {
    let (mut g, camp, warriors, mut ai) = camp_board(true, true, 1);
    for _ in 0..CAMP_STALL_TURNS {
        a_turn(&mut g, &mut ai, camp, None);
    }
    let plan = conquest(&g, None);
    ai.rebuild_force_groups(&g, 0, &plan);
    assert!(row(&ai, ObjectiveKey::Camp(camp)).is_none(), "closed");
    (g, camp, warriors, ai)
}

fn anchor_of(ai: &AdvancedAi, uid: u32) -> Pos {
    ai.force_groups
        .iter()
        .find(|group| group.units.contains(&uid))
        .map(|group| group.anchor)
        .expect("the released soldier is in a force")
}

#[test]
fn a_released_warrior_heads_for_its_new_force_and_holds_there() {
    let (mut g, camp, warriors, mut ai) = closed_camp_board();
    let mut walked = false;
    for uid in &warriors {
        let anchor = anchor_of(&ai, *uid);
        assert!(g.wdist(anchor, camp) > 1, "the new anchor is not the camp");
        // Three frames of its turns: the gene takes every one, and the
        // soldier never steps back toward the camp.
        for _ in 0..3 {
            let before = g.units[uid].pos;
            fresh_turn(&mut g, *uid);
            assert!(
                ai.camp_stall_home_step(&mut g, 0, *uid).is_some(),
                "the gene takes the released {uid}"
            );
            let after = g.units[uid].pos;
            assert!(
                g.wdist(after, camp) >= g.wdist(before, camp)
                    && g.wdist(after, anchor) <= g.wdist(before, anchor),
                "{uid}: {before:?} -> {after:?} for {anchor:?}, camp {camp:?}"
            );
            walked |= g.wdist(after, anchor) < g.wdist(before, anchor);
        }
    }
    assert!(
        walked,
        "a released Warrior walked toward its anchor: {:#?}",
        gene_lines(&ai)
    );
    assert!(gene_lines(&ai)
        .iter()
        .any(|line| line.contains("walks home from the camp")));
    // At its anchor it holds rather than handing itself back to the ladder.
    let uid = warriors[0];
    let anchor = anchor_of(&ai, uid);
    for _ in 0..12 {
        if g.wdist(g.units[&uid].pos, anchor) <= 1 {
            break;
        }
        fresh_turn(&mut g, uid);
        ai.camp_stall_home_step(&mut g, 0, uid);
    }
    if g.wdist(g.units[&uid].pos, anchor) <= 1 {
        let home = g.units[&uid].pos;
        fresh_turn(&mut g, uid);
        assert!(ai.camp_stall_home_step(&mut g, 0, uid).is_some());
        assert_eq!(g.units[&uid].pos, home, "it stays home");
    }
}

#[test]
fn another_enemy_near_or_a_soldier_never_in_the_party_is_the_ladders() {
    let (mut g, camp, warriors, mut ai) = closed_camp_board();
    // A soldier of ours the party never held.
    let stranger = g.spawn_test_unit("warrior", 0, at(8, 9));
    fresh_turn(&mut g, stranger);
    assert_eq!(ai.camp_stall_home_step(&mut g, 0, stranger), None);
    // A raider off the camp within reach of a released Warrior: a fight.
    let uid = warriors[0];
    let at_now = g.units[&uid].pos;
    let raider_at = g
        .wdisk(at_now, CAMP_STALL_HOME_WATCH)
        .into_iter()
        .find(|pos| {
            *pos != camp
                && g.wdist(*pos, at_now) == 2
                && g.unit_ids_at(*pos).is_empty()
                && g.city_at(*pos).is_none()
        })
        .expect("a tile two from the Warrior");
    let barb = g.barb_pid.expect("barbarians");
    g.spawn_test_unit("warrior", barb, raider_at);
    let before = g.units[&uid].pos;
    assert_eq!(ai.camp_stall_home_step(&mut g, 0, uid), None);
    assert_eq!(g.units[&uid].pos, before);
    // And nothing with the gene off.
    let (mut g, _, warriors, mut ai) = closed_camp_board();
    ai.disable_camp_stall_goes_home();
    assert_eq!(ai.camp_stall_home_step(&mut g, 0, warriors[0]), None);
}
