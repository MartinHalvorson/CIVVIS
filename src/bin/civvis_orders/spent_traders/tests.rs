use super::*;
use civvis::mirror::StateUnit;

fn route(subject: i64) -> Order {
    Order {
        kind: "unit",
        subject: Some(subject),
        verb: Some("TRADE_ROUTE".into()),
        pos: Some((19, 31)),
    }
}

fn trader(id: i64, moves: f64) -> StateUnit {
    StateUnit {
        id,
        kind: "UNIT_TRADER".into(),
        x: 13,
        y: 26,
        moves,
        ..StateUnit::default()
    }
}

fn state(units: Vec<StateUnit>, moves_at_turn_start: bool) -> StateSnapshot {
    let mut state = StateSnapshot {
        turn: 16,
        units,
        ..StateSnapshot::default()
    };
    state.seat.moves_at_turn_start = moves_at_turn_start;
    state
}

#[test]
fn a_trader_bought_this_turn_is_not_sent_on_a_route() {
    // Turn 16 of civvis-20261115T071444Z: Trader 393220 was bought in frame 0
    // and read `moves: 0` in frames 1 and 2, where its route was refused twice.
    let state = state(vec![trader(393220, 0.0)], true);
    let mut orders = vec![route(393220)];
    assert_eq!(hold_routes(&mut orders, &state), 1);
    assert!(
        orders.is_empty(),
        "the route waits for the Trader's movement"
    );
}

#[test]
fn a_trader_with_movement_is_sent_as_before() {
    // Turn 17 of the same game: `moves: 2`, and the host accepted the route.
    let state = state(vec![trader(393220, 2.0)], true);
    let mut orders = vec![route(393220)];
    assert_eq!(hold_routes(&mut orders, &state), 0);
    assert_eq!(orders.len(), 1);
}

#[test]
fn only_the_spent_traders_route_is_held() {
    let state = state(
        vec![
            trader(1, 0.0),
            trader(2, 2.0),
            StateUnit {
                id: 3,
                kind: "UNIT_BUILDER".into(),
                moves: 0.0,
                ..StateUnit::default()
            },
        ],
        true,
    );
    let mut orders = vec![
        route(1),
        route(2),
        Order {
            kind: "unit",
            subject: Some(1),
            verb: Some("TELEPORT_TO_CITY".into()),
            pos: Some((10, 15)),
        },
        Order {
            kind: "unit",
            subject: Some(3),
            verb: Some("IMPROVE:IMPROVEMENT_FARM".into()),
            pos: None,
        },
        Order {
            kind: "produce",
            subject: Some(1),
            verb: Some("UNIT_TRADER".into()),
            pos: None,
        },
    ];
    assert_eq!(hold_routes(&mut orders, &state), 1);
    let kept: Vec<_> = orders
        .iter()
        .map(|order| (order.kind, order.subject, order.verb.as_deref()))
        .collect();
    assert_eq!(
        kept,
        vec![
            ("unit", Some(2), Some("TRADE_ROUTE")),
            ("unit", Some(1), Some("TELEPORT_TO_CITY")),
            ("unit", Some(3), Some("IMPROVE:IMPROVEMENT_FARM")),
            ("produce", Some(1), Some("UNIT_TRADER")),
        ],
        "an origin change, another unit's order and a city's order with the same id all stand"
    );
}

#[test]
fn an_export_that_does_not_read_moves_at_turn_start_holds_nothing() {
    // Older exports reported `moves: 0` for every Trader; holding on that
    // reading would silence every route the seat ever sent.
    let state = state(vec![trader(42, 0.0)], false);
    let mut orders = vec![route(42)];
    assert_eq!(hold_routes(&mut orders, &state), 0);
    assert_eq!(orders.len(), 1);
}

#[test]
fn an_unknown_reading_or_an_unseen_trader_holds_nothing() {
    // `moves` deserialises to -1 when the export carried none.
    let state = state(vec![trader(42, -1.0)], true);
    let mut orders = vec![route(42), route(77)];
    assert_eq!(hold_routes(&mut orders, &state), 0);
    assert_eq!(orders.len(), 2);
}
