use super::super::*;
use civvis::mirror::{Seat, Snapshot, StateSnapshot, StateUnit};
use std::collections::BTreeSet;

fn state(turn: u32, frame: u32, pos: (i32, i32)) -> StateSnapshot {
    StateSnapshot {
        turn,
        frame,
        seat: Seat {
            local_player: 0,
            players: 4,
            civ: "CIVILIZATION_GRAN_COLOMBIA".into(),
            leader: "LEADER_SIMON_BOLIVAR".into(),
            ..Seat::default()
        },
        units: vec![StateUnit {
            id: 131073,
            kind: "UNIT_WARRIOR".into(),
            x: pos.0,
            y: pos.1,
            hp: 100.0,
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    }
}

fn fixture(start: (i32, i32), want: (i32, i32)) -> PendingOrders {
    PendingOrders {
        turn: 2,
        frame: 0,
        before: state(2, 0, start),
        orders: vec![IssuedOrder {
            kind: "unit".into(),
            subject: Some(131073),
            verb: Some("MOVE_TO".into()),
            pos: Some(want),
        }],
    }
}

fn checks(
    pending: &PendingOrders,
    after: &StateSnapshot,
    states: &[StateSnapshot],
) -> Vec<OrderCheck> {
    verify_orders_with_later_fortifications(
        pending,
        after,
        &Snapshot::from_chunks(&[]),
        &[],
        &pending.orders,
        LaterOrderEvidence {
            states,
            fortified: &BTreeSet::new(),
            moved: &BTreeSet::from([131073]),
            policy_deck: false,
        },
    )
}

#[test]
fn native_g66_completed_move_survives_later_replan() {
    // Actual t2/f0 -> t2/f1 -> t3/f0 positions from
    // civvis-20261004T164910Z; its native verdict was superseded_by_move.
    let pending = fixture((15, 7), (16, 6));
    assert_eq!(
        checks(&pending, &state(3, 0, (16, 7)), &[state(2, 1, (16, 6))])[0].verdict,
        Verdict::Verified
    );
}

#[test]
fn native_g73_completed_move_survives_later_replan() {
    // Independent native civvis-20261004T190740Z has the same failure.
    let pending = fixture((40, 24), (40, 23));
    assert_eq!(
        checks(&pending, &state(3, 0, (41, 23)), &[state(2, 1, (40, 23))])[0].verdict,
        Verdict::Verified
    );
}

#[test]
fn completed_move_survives_later_movement_away() {
    let pending = fixture((15, 7), (16, 6));
    assert_eq!(
        checks(&pending, &state(3, 0, (10, 10)), &[state(2, 1, (16, 6))])[0].verdict,
        Verdict::Verified
    );
}

#[test]
fn completed_move_is_not_erased_by_later_unit_loss() {
    let pending = fixture((15, 7), (16, 6));
    let mut after = state(3, 0, (16, 6));
    after.units.clear();
    assert_eq!(
        checks(&pending, &after, &[state(2, 1, (16, 6))])[0].verdict,
        Verdict::Verified
    );
}

#[test]
fn completed_move_clears_refusal_memory() {
    let pending = fixture((15, 7), (16, 6));
    let mut refusals = HostOrderRefusals::default();
    let failed = OrderCheck {
        order: pending.orders[0].clone(),
        verdict: Verdict::Failed("superseded_by_move".into()),
    };
    for turn in 0..ORDER_REFUSAL_STRIKES {
        refusals.observe(std::slice::from_ref(&failed), turn);
    }
    assert!(refusals.seen[&order_identity(&failed.order)]
        .until
        .is_some());
    refusals.observe(
        &checks(&pending, &state(3, 0, (16, 7)), &[state(2, 1, (16, 6))]),
        2,
    );
    assert!(!refusals.seen.contains_key(&order_identity(&failed.order)));
}

#[test]
fn live_settlement_reads_native_seat_and_intermediate_endpoint() {
    use std::io::Write;
    let initial = fixture((40, 24), (40, 23));
    let identity = order_identity(&initial.orders[0]);
    let mut refusals = HostOrderRefusals::default();
    let failed = OrderCheck {
        order: initial.orders[0].clone(),
        verdict: Verdict::Failed("superseded_by_move".into()),
    };
    for turn in 0..ORDER_REFUSAL_STRIKES {
        refusals.observe(std::slice::from_ref(&failed), turn);
    }
    let mut replan = fixture((40, 23), (41, 23));
    replan.frame = 1;
    replan.before.frame = 1;
    let mut pending = vec![initial, replan];
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "civvis-intermediate-move-{}-{stamp}.jsonl",
        std::process::id()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    // Native identity is a separate event without a requested turn; native
    // state rows do NOT carry `seat`. Match the actual relay's spaced JSON.
    writeln!(file, "{{\"kind\": \"seat\", \"local_player\": 0, \"players\": 4, \"civ\": \"CIVILIZATION_GRAN_COLOMBIA\", \"leader\": \"LEADER_SIMON_BOLIVAR\"}}").unwrap();
    writeln!(file, "{{\"kind\": \"state\", \"turn\": 2, \"frame\": 1, \"units\": [{{\"id\": 131073, \"kind\": \"UNIT_WARRIOR\", \"x\": 40, \"y\": 23, \"hp\": 100}}]}}").unwrap();
    drop(file);
    let rows = settle_pending_orders(
        &mut pending,
        &path,
        &state(3, 0, (41, 23)),
        &Snapshot::from_chunks(&[]),
        &mut refusals,
    );
    std::fs::remove_file(&path).unwrap();
    assert!(pending.is_empty());
    assert_eq!(
        rows.iter().filter(|r| r.kind == "order_verified").count(),
        2
    );
    assert!(!refusals.seen.contains_key(&identity));
}

#[test]
fn only_strictly_later_frames_of_the_same_turn_can_prove_completion() {
    let pending = fixture((15, 7), (16, 6));
    for (turn, frame) in [(2, 0), (1, 3), (3, 1)] {
        assert_eq!(
            checks(
                &pending,
                &state(3, 0, (16, 7)),
                &[state(turn, frame, (16, 6))]
            )[0]
            .verdict,
            Verdict::Failed("superseded_by_move".into())
        );
    }
    let mut pending = pending;
    pending.frame = 2;
    assert_eq!(
        checks(&pending, &state(3, 0, (16, 7)), &[state(2, 1, (16, 6))])[0].verdict,
        Verdict::Failed("superseded_by_move".into())
    );
}

#[test]
fn another_unit_or_foreign_unit_cannot_prove_completion() {
    let pending = fixture((15, 7), (16, 6));
    let mut other = state(2, 1, (16, 6));
    other.units[0].id += 1;
    let mut foreign = state(2, 1, (16, 6));
    foreign.hostiles = foreign.units.clone();
    foreign.units.clear();
    for later in [other, foreign] {
        assert_eq!(
            checks(&pending, &state(3, 0, (16, 7)), &[later])[0].verdict,
            Verdict::Failed("superseded_by_move".into())
        );
    }
}

#[test]
fn another_native_seat_cannot_prove_completion() {
    let pending = fixture((15, 7), (16, 6));
    for field in 0..4 {
        let mut other = state(2, 1, (16, 6));
        match field {
            0 => other.seat.local_player = 1,
            1 => other.seat.players = 5,
            2 => other.seat.civ = "CIVILIZATION_NUBIA".into(),
            _ => other.seat.leader = "LEADER_AMANITORE".into(),
        }
        assert_eq!(
            checks(&pending, &state(3, 0, (16, 7)), &[other])[0].verdict,
            Verdict::Failed("superseded_by_move".into())
        );
    }
}

#[test]
fn another_destination_cannot_prove_completion() {
    let pending = fixture((15, 7), (16, 6));
    assert_eq!(
        checks(&pending, &state(3, 0, (16, 7)), &[state(2, 1, (15, 6))])[0].verdict,
        Verdict::Failed("superseded_by_move".into())
    );
}

#[test]
fn actor_missing_from_decision_frame_stays_unverifiable() {
    let mut pending = fixture((15, 7), (16, 6));
    pending.before.units.clear();
    assert_eq!(
        checks(&pending, &state(3, 0, (16, 7)), &[state(2, 1, (16, 6))])[0].verdict,
        Verdict::Unverifiable
    );
}

#[test]
fn requested_replan_without_observed_endpoint_stays_failed() {
    let pending = fixture((15, 7), (16, 6));
    assert_eq!(
        checks(&pending, &state(3, 0, (16, 7)), &[])[0].verdict,
        Verdict::Failed("superseded_by_move".into())
    );
}

#[test]
fn movement_postcondition_does_not_verify_other_verbs() {
    let mut pending = fixture((15, 7), (16, 6));
    pending.orders[0].verb = Some("FORTIFY".into());
    assert_ne!(
        checks(&pending, &state(3, 0, (16, 7)), &[state(2, 1, (16, 6))])[0].verdict,
        Verdict::Verified
    );
}
