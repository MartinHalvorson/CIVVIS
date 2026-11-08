use super::*;

fn order(kind: &'static str) -> Order {
    Order {
        kind,
        subject: Some(4),
        verb: None,
        pos: None,
    }
}

#[test]
fn a_native_defensive_material_purchase_survives_alliance_serialization() {
    let mut orders = vec![order("alliance"), order("buy"), order("research")];
    assert_eq!(defer_alliances_for_defensive_material(&mut orders, true), 1);
    assert_eq!(hold_deals_beside_an_alliance(&mut orders), 0);
    assert_eq!(
        orders.iter().map(|order| order.kind).collect::<Vec<_>>(),
        vec!["buy", "research"]
    );
}

#[test]
fn ordinary_purchases_and_failed_material_requests_keep_alliance_priority() {
    let mut orders = vec![order("alliance"), order("buy")];
    assert_eq!(
        defer_alliances_for_defensive_material(&mut orders, false),
        0
    );
    assert_eq!(hold_deals_beside_an_alliance(&mut orders), 1);
    assert_eq!(orders[0].kind, "alliance");
    assert_eq!(defer_alliances_for_defensive_material(&mut orders, true), 0);
    assert_eq!(orders[0].kind, "alliance");
}

#[test]
fn war_or_peace_precedence_still_removes_the_material_purchase() {
    let mut orders = vec![order("peace"), order("buy")];
    assert_eq!(drop_deals_beside_war_or_peace(&mut orders), 1);
    assert_eq!(defer_alliances_for_defensive_material(&mut orders, true), 0);
    assert!(orders.iter().all(|order| order.kind != "buy"));
}
