use super::tests::{aim, conquest_fixture};
use super::*;

fn present_siege() -> (Game, AdvancedAi, u32) {
    let (mut g, target) = conquest_fixture();
    g.turn = 50;
    let at = g.cities[&target].pos;
    let beside = g
        .wdisk(at, 1)
        .into_iter()
        .find(|pos| {
            *pos != at && !g.rules.is_water(&g.map.tiles[pos]) && g.units_at(*pos).is_empty()
        })
        .unwrap();
    g.spawn_test_unit("warrior", 0, beside);
    let mut ai = AdvancedAi::new();
    ai.enable_capture_go_or_stand_down_2();
    aim(&mut ai, &g, target);
    ai.reconcile_commitments(&mut g, 0);
    (g, ai, target)
}

#[test]
fn observation_replans_do_not_age_a_siege_three_times_per_turn() {
    let (mut g, mut ai, target) = present_siege();
    for turn in 51..=54 {
        g.turn = turn;
        for _ in 0..=crate::ai::player::REPLAN_FRAMES {
            ai.reconcile_commitments(&mut g, 0);
        }
    }
    let c = ai
        .commitments
        .open_for(Kind::Capture, Owner::Empire)
        .unwrap();
    assert_eq!(
        c.turns_open, 4,
        "four game turns elapsed, regardless of planning frames"
    );
    assert_eq!(c.stalled_streak, 2);
    assert!(!ai.capture_stood_down.contains_key(&target));
    assert_eq!(ai.commitments.census.capture.open_turns, 4);
}

#[test]
fn same_turn_replans_cannot_exhaust_a_missing_armys_patience() {
    let (mut g, target) = conquest_fixture();
    let mut ai = AdvancedAi::new();
    ai.enable_capture_go_or_stand_down();
    aim(&mut ai, &g, target);
    for _ in 0..=CAPTURE_GO_TURNS {
        ai.reconcile_commitments(&mut g, 0);
    }
    assert!(!ai.capture_stood_down.contains_key(&target));
    for _ in 0..CAPTURE_GO_TURNS {
        g.turn += 1;
        ai.reconcile_commitments(&mut g, 0);
    }
    assert!(
        ai.capture_stood_down.contains_key(&target),
        "real forgotten turns still exhaust patience"
    );
}

#[test]
fn a_replan_still_observes_damage_and_capture_in_the_same_turn() {
    let (mut g, mut ai, target) = present_siege();
    g.turn = 54;
    ai.reconcile_commitments(&mut g, 0);
    let before = ai
        .commitments
        .open_for(Kind::Capture, Owner::Empire)
        .unwrap()
        .clone();
    g.cities.get_mut(&target).unwrap().hp -= 40;
    ai.reconcile_commitments(&mut g, 0);
    let after = ai
        .commitments
        .open_for(Kind::Capture, Owner::Empire)
        .unwrap();
    assert_eq!(after.best, before.best - 40);
    assert_eq!(after.stalled_streak, 0);
    assert_eq!(after.turns_open, before.turns_open);
    g.cities.get_mut(&target).unwrap().owner = 0;
    ai.reconcile_commitments(&mut g, 0);
    assert!(ai
        .commitments
        .open_for(Kind::Capture, Owner::Empire)
        .is_none());
    assert_eq!(ai.commitments.census.capture.completed, 1);
}
