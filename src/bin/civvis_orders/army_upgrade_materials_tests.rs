use super::*;
use civvis::ai::{ArmyUpgradeMaterialWant, BarterSpare};
use civvis::mirror::{StateRival, StateSnapshot};
use std::collections::BTreeMap;

fn want() -> Vec<ArmyUpgradeMaterialWant> {
    vec![ArmyUpgradeMaterialWant {
        resource: "RESOURCE_IRON".into(),
        resource_id: "iron".into(),
        amount: 10,
        unit: "knight".into(),
        uid: 7,
        gold_reserve: 190.0,
        gain: 20.0,
    }]
}

fn state() -> StateSnapshot {
    StateSnapshot {
        turn: 89,
        gold: 200,
        gold_per_turn: Some(24.0),
        luxury_counts: Some(BTreeMap::from([("RESOURCE_OLIVES".into(), 4.0)])),
        rivals: vec![StateRival {
            player: 1,
            tradeable_strategics: Some(BTreeMap::from([("RESOURCE_IRON".into(), 25)])),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn spares() -> Vec<BarterSpare> {
    vec![BarterSpare {
        resource: "RESOURCE_OLIVES".into(),
        resource_id: "olives".into(),
        amount: 3,
        luxury: true,
    }]
}

#[test]
fn surplus_barter_can_buy_the_knights_iron_without_spending_upgrade_gold() {
    let mut orders = Vec::new();
    assert_eq!(
        append_army_upgrade_material_buy_order(
            &want(),
            &spares(),
            &state(),
            &mut orders,
            &|_, _| 35.0,
            &|_, _, _| 40.0,
            200,
        ),
        None
    );
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].subject, Some(1));
    assert_eq!(
        orders[0].verb.as_deref(),
        Some("RESOURCE_IRON=10;RESOURCE_OLIVES=1")
    );
    assert_eq!(orders[0].pos, Some((0, 0)));
}

#[test]
fn an_income_rich_seat_cannot_borrow_the_upgrade_bill_for_material() {
    let mut state = state();
    state.gold_per_turn = Some(1_000.0);
    let mut orders = Vec::new();
    assert_eq!(
        append_army_upgrade_material_buy_order(
            &want(),
            &[],
            &state,
            &mut orders,
            &|_, _| 35.0,
            &|_, _, _| 0.0,
            200,
        ),
        Some("treasury")
    );
    assert!(orders.is_empty());
}

#[test]
fn earlier_planning_spends_limit_the_cash_ceiling() {
    let mut state = state();
    state.gold = 500;
    let mut orders = Vec::new();
    assert_eq!(
        append_army_upgrade_material_buy_order(
            &want(),
            &[],
            &state,
            &mut orders,
            &|_, _| 35.0,
            &|_, _, _| 0.0,
            220,
        ),
        Some("treasury")
    );
    assert!(orders.is_empty());
    assert_eq!(
        append_army_upgrade_material_buy_order(
            &want(),
            &[],
            &state,
            &mut orders,
            &|_, _| 35.0,
            &|_, _, _| 0.0,
            300,
        ),
        None
    );
    assert_eq!(orders[0].pos, Some((35, 0)));
}

#[test]
fn wartime_missing_short_and_busy_sellers_do_not_receive_an_ask() {
    for bad in ["war", "missing", "short", "busy"] {
        let mut state = state();
        let mut orders = Vec::new();
        match bad {
            "war" => state.rivals[0].at_war = true,
            "missing" => state.rivals.clear(),
            "short" => {
                state.rivals[0]
                    .tradeable_strategics
                    .as_mut()
                    .unwrap()
                    .insert("RESOURCE_IRON".into(), 9);
            }
            "busy" => orders.push(Order {
                kind: "buy",
                subject: Some(1),
                verb: Some("open_borders".into()),
                pos: Some((30, 0)),
            }),
            _ => unreachable!(),
        }
        let before = orders.len();
        assert!(append_army_upgrade_material_buy_order(
            &want(),
            &spares(),
            &state,
            &mut orders,
            &|_, _| 35.0,
            &|_, _, _| 40.0,
            200,
        )
        .is_some());
        assert_eq!(orders.len(), before);
    }
}

#[test]
fn the_last_luxury_copy_and_an_already_committed_sale_cannot_be_bartered() {
    for last in [true, false] {
        let mut state = state();
        let mut orders = Vec::new();
        if last {
            state.luxury_counts = Some(BTreeMap::from([("RESOURCE_OLIVES".into(), 1.0)]));
        } else {
            orders.push(Order {
                kind: "sell",
                subject: Some(2),
                verb: Some("RESOURCE_OLIVES=1".into()),
                pos: None,
            });
        }
        let before = orders.len();
        assert_eq!(
            append_army_upgrade_material_buy_order(
                &want(),
                &spares(),
                &state,
                &mut orders,
                &|_, _| 35.0,
                &|_, _, _| 40.0,
                200,
            ),
            Some("treasury")
        );
        assert_eq!(orders.len(), before);
    }
}
