use super::*;

fn order(kind: &'static str, subject: i64, verb: &str, pos: Option<(i32, i32)>) -> Order {
    Order {
        kind,
        subject: Some(subject),
        verb: Some(verb.to_string()),
        pos,
    }
}

fn shown(orders: &[Order]) -> Vec<String> {
    orders
        .iter()
        .map(|order| {
            format!(
                "{}:{}:{:?}:{:?}",
                order.kind,
                order.verb.as_deref().unwrap_or("-"),
                order.subject,
                order.pos
            )
        })
        .collect()
}

/// Seat p2 of `hotseat-20261128T031456Z`, turn 16: frame 0 sent the
/// delegation to seat 5, a move and a fortify; frame 1 the delegation again.
fn p2_turn_16_frame(frame: u32) -> Vec<Order> {
    let mut orders = vec![order("delegation", 5, "DIPLOMATIC_DELEGATION", None)];
    if frame == 0 {
        orders.push(order("unit", 131073, "MOVE_TO", Some((12, 34))));
        orders.push(order("unit", 131073, "FORTIFY", None));
    }
    orders
}

#[test]
fn the_replan_frame_does_not_send_the_delegation_again() {
    let mut asks = Asks::default();
    let mut first = p2_turn_16_frame(0);
    assert!(asks.keep_once(&mut first, 16).is_empty());
    assert_eq!(first.len(), 3, "the first frame sends everything");
    let mut second = p2_turn_16_frame(1);
    let dropped = asks.keep_once(&mut second, 16);
    assert!(second.is_empty(), "{:?}", shown(&second));
    assert_eq!(
        dropped,
        vec!["delegation:DIPLOMATIC_DELEGATION 5".to_string()]
    );
}

#[test]
fn a_new_turn_asks_again() {
    let mut asks = Asks::default();
    let mut first = p2_turn_16_frame(0);
    asks.keep_once(&mut first, 16);
    let mut next = p2_turn_16_frame(1);
    assert!(asks.keep_once(&mut next, 17).is_empty());
    assert_eq!(next.len(), 1, "turn 17 sends the delegation");
}

#[test]
fn moves_blows_envoys_war_and_peace_are_left_alone() {
    let mut asks = Asks::default();
    let frame = || {
        vec![
            order("unit", 7, "MOVE_TO", Some((3, 4))),
            order("unit", 7, "ATTACK", Some((3, 5))),
            order("envoy", 12, "ENVOY", None),
            order("envoy", 12, "ENVOY", None),
            order("war", 3, "DECLARE", None),
            order("peace", 3, "MAKE_PEACE", None),
            order("research", 0, "TECH_WRITING", None),
        ]
    };
    let mut first = frame();
    asks.keep_once(&mut first, 9);
    let mut second = frame();
    assert!(asks.keep_once(&mut second, 9).is_empty());
    assert_eq!(
        shown(&second),
        shown(&frame()),
        "nothing of these is dropped"
    );
}

#[test]
fn any_second_deal_with_one_rival_is_dropped_and_another_rival_is_not() {
    let mut asks = Asks::default();
    // p2 turn 15: the Great Work purchase from seat 5 in frame 0.
    let mut first = vec![Order {
        kind: "buy",
        subject: Some(5),
        verb: Some("GREATWORK_ANY;GPT_CAP=4".into()),
        pos: Some((100, 0)),
    }];
    asks.keep_once(&mut first, 15);
    // Frame 1: the same purchase at another price, a sale to the same rival
    // and a purchase from another one.
    let mut second = vec![
        Order {
            kind: "buy",
            subject: Some(5),
            verb: Some("GREATWORK_ANY;GPT_CAP=4".into()),
            pos: Some((120, 0)),
        },
        order("sell", 5, "RESOURCE_HORSES", Some((40, 0))),
        order("buy", 2, "OPEN_BORDERS", Some((30, 0))),
    ];
    let dropped = asks.keep_once(&mut second, 15);
    assert_eq!(dropped.len(), 2, "{dropped:?}");
    assert_eq!(
        shown(&second),
        vec!["buy:OPEN_BORDERS:Some(2):Some((30, 0))"]
    );
}

#[test]
fn a_trader_route_goes_once_and_another_destination_still_goes() {
    let mut asks = Asks::default();
    // p3 turn 12: the Trader at (14,13) sent to (12,21) in both frames.
    let mut first = vec![order("unit", 327680, "TRADE_ROUTE", Some((12, 21)))];
    asks.keep_once(&mut first, 12);
    let mut second = vec![
        order("unit", 327680, "TRADE_ROUTE", Some((12, 21))),
        order("unit", 327680, "TRADE_ROUTE", Some((9, 18))),
    ];
    let dropped = asks.keep_once(&mut second, 12);
    assert_eq!(dropped, vec!["unit:TRADE_ROUTE 327680".to_string()]);
    assert_eq!(
        shown(&second),
        vec!["unit:TRADE_ROUTE:Some(327680):Some((9, 18))"]
    );
}

#[test]
fn a_delegation_and_an_embassy_to_one_rival_are_two_asks() {
    let mut asks = Asks::default();
    let mut first = vec![order("delegation", 4, "DIPLOMATIC_DELEGATION", None)];
    asks.keep_once(&mut first, 30);
    let mut second = vec![
        order("delegation", 4, "RESIDENT_EMBASSY", None),
        order("friendship", 4, "DECLARE_FRIEND", None),
    ];
    assert!(asks.keep_once(&mut second, 30).is_empty());
    assert_eq!(second.len(), 2);
    let mut third = vec![order("friendship", 4, "DECLARE_FRIEND", None)];
    assert_eq!(
        asks.keep_once(&mut third, 30).len(),
        1,
        "the friendship went in frame 1"
    );
}

#[test]
fn the_switch_ships_off_and_is_registered() {
    let ai = civvis::ai::AdvancedAi::new();
    assert!(!ai.asks_go_once_a_turn_enabled());
    let gene = civvis::ai::gene("asks-go-once-a-turn").expect("published");
    let mut on = civvis::ai::AdvancedAi::new();
    (gene.enable)(&mut on);
    assert!(on.asks_go_once_a_turn_enabled());
}
