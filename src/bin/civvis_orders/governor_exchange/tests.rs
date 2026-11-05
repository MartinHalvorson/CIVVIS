use super::*;
use civvis::mirror::StateCity;

const PIN: &str = "GOVERNOR_THE_EDUCATOR";
const MAG: &str = "GOVERNOR_THE_RESOURCE_MANAGER";
const A: i64 = 131_073;
const B: i64 = 65_536;

fn governor(kind: &str, city: i64, x: i32, y: i32) -> StateGovernor {
    StateGovernor {
        kind: kind.into(),
        city,
        city_player: 0,
        x,
        y,
        ..StateGovernor::default()
    }
}

fn state() -> StateSnapshot {
    let mut state = StateSnapshot {
        turn: 154,
        frame: 0,
        cities: vec![
            StateCity {
                id: A,
                x: 57,
                y: 12,
                ..StateCity::default()
            },
            StateCity {
                id: B,
                x: 61,
                y: 10,
                ..StateCity::default()
            },
        ],
        governors: Some(vec![governor(PIN, A, 57, 12), governor(MAG, B, 61, 10)]),
        ..StateSnapshot::default()
    };
    state.seat.local_player = 0;
    state
}

fn assign(name: &str, city: i64) -> Order {
    Order {
        kind: "governor_assign",
        subject: Some(city),
        verb: Some(name.into()),
        pos: Some((0, -1)),
    }
}

fn research() -> Order {
    Order {
        kind: "research",
        subject: None,
        verb: Some("TECH_SATELLITES".into()),
        pos: None,
    }
}

fn pair() -> Vec<Order> {
    vec![research(), assign(PIN, B), assign(MAG, A)]
}

fn first_observed(state: &mut StateSnapshot) {
    state.frame += 1;
    state.governors = Some(vec![governor(PIN, B, 61, 10), governor(MAG, -1, -1, -1)]);
}

fn prepare(
    continuation: &mut Continuation,
    state: &StateSnapshot,
    mut orders: Vec<Order>,
) -> (Option<&'static str>, Vec<Order>) {
    let note = continuation.prepare(state, &mut orders);
    (note, super::super::defer_governor_followups(orders).0)
}

#[test]
fn reciprocal_exchange_releases_return_only_after_native_first_assignment() {
    let mut state = state();
    let mut continuation = Continuation::default();
    let (note, orders) = prepare(&mut continuation, &state, pair());
    assert_eq!(note, Some("return_retained"));
    assert_eq!(orders.len(), 2);
    assert_eq!(orders[1].verb.as_deref(), Some(PIN));
    first_observed(&mut state);
    let (note, orders) = prepare(&mut continuation, &state, vec![research(), assign(PIN, A)]);
    assert_eq!(note, Some("return_released_after_first_readback"));
    assert_eq!(orders[0].kind, "research");
    assert_eq!(orders[1].verb.as_deref(), Some(MAG));
    assert_eq!(orders[1].subject, Some(A));
    assert_eq!(orders[1].pos, Some((0, -1)));
    state.frame += 1;
    state.governors.as_mut().unwrap()[1] = governor(MAG, A, 57, 12);
    assert_eq!(
        prepare(&mut continuation, &state, vec![research()]).0,
        Some("both_posts_observed")
    );
    assert!(continuation.pending.is_none());
}

#[test]
fn unobserved_first_assignment_and_same_frame_cannot_release_return() {
    let mut state = state();
    let mut continuation = Continuation::default();
    prepare(&mut continuation, &state, pair());
    state.frame += 1;
    let (note, orders) = prepare(&mut continuation, &state, pair());
    assert_eq!(note, Some("waiting_for_first_assignment"));
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].kind, "research");

    let mut fresh_state = self::state();
    let mut continuation = Continuation::default();
    prepare(&mut continuation, &fresh_state, pair());
    first_observed(&mut fresh_state);
    fresh_state.frame = 0;
    let (note, orders) = prepare(&mut continuation, &fresh_state, pair());
    assert_eq!(note, Some("waiting_for_fresh_return_readback"));
    assert_eq!(orders.len(), 1);
}

#[test]
fn return_retries_once_per_turn_and_expires() {
    let mut state = state();
    let mut continuation = Continuation::default();
    prepare(&mut continuation, &state, pair());
    first_observed(&mut state);
    assert_eq!(prepare(&mut continuation, &state, vec![]).1.len(), 1);
    state.frame += 1;
    assert!(prepare(&mut continuation, &state, vec![]).1.is_empty());
    state.turn += 1;
    state.frame = 0;
    assert_eq!(prepare(&mut continuation, &state, vec![]).1.len(), 1);
    state.turn = 158;
    assert_eq!(
        prepare(&mut continuation, &state, vec![]).0,
        Some("cancelled_stale_city_or_frame")
    );
    assert!(continuation.pending.is_none());
}

#[test]
fn changed_native_identity_or_governor_cancels_without_injecting_an_order() {
    for case in 0..9 {
        let mut state = state();
        let mut continuation = Continuation::default();
        prepare(&mut continuation, &state, pair());
        first_observed(&mut state);
        match case {
            0 => {
                state.seat.local_player = 1;
            }
            1 => {
                state.cities[0].x += 1;
            }
            2 => {
                state.cities.remove(0);
            }
            3 => {
                state.governors = None;
            }
            4 => {
                state.governors.as_mut().unwrap().remove(1);
            }
            5 => {
                state.governors.as_mut().unwrap()[0].neutralized_turns = 2;
            }
            6 => {
                state.governors.as_mut().unwrap().push(governor(
                    "GOVERNOR_THE_DEFENDER",
                    A,
                    57,
                    12,
                ));
            }
            7 => {
                state.governors.as_mut().unwrap()[1] = governor(MAG, 999, 1, 1);
            }
            8 => {
                state.turn -= 1;
            }
            _ => unreachable!(),
        };
        let (note, orders) = prepare(&mut continuation, &state, vec![research()]);
        assert!(note.unwrap().starts_with("cancelled_"), "case {case}");
        assert!(continuation.pending.is_none(), "case {case}");
        assert_eq!(orders.len(), 1, "case {case}");
        assert_eq!(orders[0].kind, "research");
    }
}

#[test]
fn appointment_or_promotion_before_pair_keeps_existing_replan_guard() {
    for kind in ["governor_appoint", "governor_promote"] {
        let mut orders = pair();
        orders.insert(
            0,
            Order {
                kind,
                subject: None,
                verb: Some(PIN.into()),
                pos: None,
            },
        );
        let mut continuation = Continuation::default();
        let (note, orders) = prepare(&mut continuation, &state(), orders);
        assert_eq!(note, None);
        assert!(continuation.pending.is_none());
        assert_eq!(orders.len(), 2);
        assert_eq!(orders[0].kind, kind);
        assert_eq!(orders[1].kind, "research");
    }
}

#[test]
fn nonreciprocal_duplicate_and_foreign_assignments_are_not_retained() {
    for orders in [
        vec![assign(PIN, B), assign(MAG, B)],
        vec![assign(PIN, B), assign(PIN, A)],
        vec![assign("UNKNOWN", B), assign(MAG, A)],
        vec![
            assign(PIN, B),
            Order {
                pos: Some((1, -1)),
                ..assign(MAG, A)
            },
        ],
    ] {
        let mut continuation = Continuation::default();
        assert_eq!(prepare(&mut continuation, &state(), orders).0, None);
        assert!(continuation.pending.is_none());
    }
}
