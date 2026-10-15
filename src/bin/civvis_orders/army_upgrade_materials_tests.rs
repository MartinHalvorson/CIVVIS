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
#[test]
fn the_production_decider_reserves_cash_after_earlier_purchases() {
    use civvis::mirror::{Snapshot, StateCity, StateUnit, TilesChunk};
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 89,
        width: 16,
        height: 12,
        chunk: 1,
        plots: (0..16)
            .flat_map(|x| {
                (0..12).map(move |y| {
                    serde_json::from_value(serde_json::json!({
                        "x": x, "y": y, "t": "TERRAIN_GRASS", "vis": true,
                        "o": if x < 8 { 0 } else { 1 },
                        "oc": if x < 8 { 7 } else { 70 }
                    }))
                    .unwrap()
                })
            })
            .collect(),
    }]);
    let mut state = state();
    state.gold = 600;
    state.gold_per_turn = Some(0.0);
    state.techs = vec!["TECH_IRON_WORKING".into(), "TECH_STIRRUPS".into()];
    state.luxury_counts = Some(BTreeMap::new());
    state.cities = vec![StateCity {
        id: 7,
        name: "Bogota".into(),
        x: 4,
        y: 6,
        pop: 4,
        capital: true,
        ..Default::default()
    }];
    state.rivals[0].can_declare = Some(false);
    state.rivals[0].cities = vec![StateCity {
        id: 70,
        name: "Berlin".into(),
        x: 12,
        y: 6,
        pop: 4,
        capital: true,
        ..Default::default()
    }];
    state.units = vec![
        StateUnit {
            id: 17,
            kind: "UNIT_WARRIOR".into(),
            x: 5,
            y: 6,
            hp: 100.0,
            moves: 2.0,
            upgrade_to: Some("UNIT_SWORDSMAN".into()),
            upgrade_cost: Some(400.0),
            upgrade_resource_cost: Some(0.0),
            upgrade_blocked_reason: Some("Unavailable this turn".into()),
            ..Default::default()
        },
        StateUnit {
            id: 18,
            kind: "UNIT_HEAVY_CHARIOT".into(),
            x: 6,
            y: 6,
            hp: 100.0,
            moves: 2.0,
            upgrade_to: Some("UNIT_KNIGHT".into()),
            upgrade_cost: Some(160.0),
            upgrade_resource: Some("RESOURCE_IRON".into()),
            upgrade_resource_cost: Some(10.0),
            upgrade_blocked_reason: Some("Insufficient Resources".into()),
            ..Default::default()
        },
    ];
    let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
    let mut ai = civvis::ai::AdvancedAi::targeting(civvis::ai::VictoryTarget::Domination);
    ai.enable_army_upgrade_materials();
    let initial = ai.army_upgrade_material_wants(&mirror.game, 0);
    assert_eq!(initial.len(), 1);
    assert_eq!(initial[0].gold_reserve, 190.0);
    let reply: serde_json::Value = serde_json::from_str(&decide(
        &mut mirror,
        &mut ai,
        &snapshot,
        &state,
        DecisionArm {
            war_from_plan: false,
            forced_on: &["army-upgrade-materials"],
            withheld: &[],
        },
        DecisionMemory {
            ours: &mut Default::default(),
            host_peace_retries: &mut HostPeaceRetries::default(),
            host_move_refusals: &mut HostMoveRefusals::default(),
            host_order_refusals: &mut HostOrderRefusals::default(),
            air_assault_continuation: &mut air_assault_continuation::Continuation::default(),
        },
    ))
    .unwrap();
    let actions = reply["decision"]["native_actions"].as_array().unwrap();
    assert!(actions
        .iter()
        .any(|a| a["type"] == "buy" && a["unit"] == "builder" && a["currency"] == "gold"));
    assert!(actions.iter().any(|a| a["type"] == "buy_building"
        && a["building"] == "monument"
        && a["currency"] == "gold"));
    let material_buy = reply["orders"].as_array().unwrap().iter().any(|o| {
        o["kind"] == "buy"
            && o["verb"]
                .as_str()
                .is_some_and(|v| v.starts_with("RESOURCE_IRON=10"))
    });
    assert!(!material_buy,
        "earlier Builder and Monument purchases must reduce the Iron budget and retain the Knight's 190-Gold reservation: {}", reply);
    assert_eq!(
        mirror.game.players[0].gold, 600.0,
        "the authoritative mirror stays untouched"
    );
}

#[test]
fn barter_cannot_hide_an_upgrade_reserve_already_spent_by_planning() {
    let mut orders = Vec::new();
    assert_eq!(
        append_army_upgrade_material_buy_order(
            &want(),
            &spares(),
            &state(),
            &mut orders,
            &|_, _| 35.0,
            &|_, _, _| 40.0,
            180,
        ),
        Some("treasury")
    );
    assert!(orders.is_empty());
}
