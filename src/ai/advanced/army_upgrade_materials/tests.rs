use super::*;
use crate::game::{HostUnitFacts, HostUnitUpgrade, HostUnitUpgradeResource};
use std::sync::Arc;

fn fixture() -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(2, 24, 16, 15_041, 250, 0, false);
    g.units.clear();
    g.current = 0;
    g.turn = 89;
    g.players[0].gold = 200.0;
    g.players[0].gold_per_turn = 24.0;
    g.players[0].techs.insert(crate::name!("stirrups"));
    let uid = g.spawn_test_unit("heavy_chariot", 0, (4, 4));
    Arc::make_mut(&mut g.host_unit_facts).insert(
        uid,
        HostUnitFacts {
            upgrade: Some(HostUnitUpgrade {
                to: Some(crate::name!("knight")),
                cost: Some(160.0),
                resources: Some(HostUnitUpgradeResource {
                    resource: Some(crate::name!("iron")),
                    cost: 10.0,
                }),
                blocked: Some("Insufficient Resources".into()),
            }),
            ..Default::default()
        },
    );
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_army_upgrade_materials();
    (g, ai, uid)
}

#[test]
fn the_native_knight_bill_is_requested_without_spending_its_upgrade_money() {
    let (g, ai, uid) = fixture();
    let want = &ai.army_upgrade_material_wants(&g, 0)[0];
    assert_eq!(want.uid, uid);
    assert_eq!(want.resource, "RESOURCE_IRON");
    assert_eq!(
        want.amount, 10,
        "use the host's ten, not the model's twenty"
    );
    assert_eq!(want.unit, "knight");
    assert_eq!(want.gold_reserve, 190.0);
    assert_eq!(g.players[0].gold, 200.0);
    assert_eq!(g.units[&uid].kind, "heavy_chariot");
}

#[test]
fn a_large_obsolete_army_requests_only_one_fundable_upgrade() {
    let (mut g, ai, first) = fixture();
    let quote = g.host_unit_facts[&first].clone();
    for _ in 0..9 {
        let uid = g.spawn_test_unit("heavy_chariot", 0, (5, 4));
        Arc::make_mut(&mut g.host_unit_facts).insert(uid, quote.clone());
    }
    let wants = ai.army_upgrade_material_wants(&g, 0);
    assert_eq!(wants.len(), 1);
    assert_eq!(wants[0].amount, 10);
}

#[test]
fn disabled_other_victory_unfunded_wounded_and_existing_stock_do_not_buy() {
    let (mut g, mut ai, uid) = fixture();
    ai.disable_army_upgrade_materials();
    assert!(ai.army_upgrade_material_wants(&g, 0).is_empty());
    ai.enable_army_upgrade_materials();
    g.players[0].gold = 189.0;
    assert!(ai.army_upgrade_material_wants(&g, 0).is_empty());
    g.players[0].gold = 200.0;
    g.units.get_mut(&uid).unwrap().hp = 10;
    assert!(ai.army_upgrade_material_wants(&g, 0).is_empty());
    g.units.get_mut(&uid).unwrap().hp = 100;
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 10.0);
    assert!(ai.army_upgrade_material_wants(&g, 0).is_empty());
    g.players[0].strategic_resources.clear();
    let mut culture = AdvancedAi::targeting(VictoryTarget::Culture);
    culture.enable_army_upgrade_materials();
    assert!(culture.army_upgrade_material_wants(&g, 0).is_empty());
}

#[test]
fn a_settler_guard_keeps_its_reservation() {
    let (mut g, mut ai, uid) = fixture();
    let settler = g.spawn_test_unit("settler", 0, (4, 5));
    ai.settler_guards.insert(settler, uid);
    assert!(ai.army_upgrade_material_wants(&g, 0).is_empty());
}

#[test]
fn an_unmodeled_host_successor_cannot_be_replaced_by_a_guessed_upgrade() {
    let (mut g, ai, uid) = fixture();
    Arc::make_mut(&mut g.host_unit_facts)
        .get_mut(&uid)
        .unwrap()
        .upgrade
        .as_mut()
        .unwrap()
        .to = None;
    assert!(ai.army_upgrade_material_wants(&g, 0).is_empty());
}

#[test]
fn stocked_material_keeps_one_native_bill_for_a_large_obsolete_army() {
    let (mut g, ai, first) = fixture();
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 25.0);
    let quote = g.host_unit_facts[&first].clone();
    for _ in 0..9 {
        let uid = g.spawn_test_unit("heavy_chariot", 0, (5, 4));
        Arc::make_mut(&mut g.host_unit_facts).insert(uid, quote.clone());
    }
    assert!(ai.army_upgrade_material_wants(&g, 0).is_empty());
    assert_eq!(
        ai.army_upgrade_material_reserves(&g, 0),
        BTreeMap::from([(crate::name!("iron"), 10.0)])
    );
}

#[test]
fn fractional_native_bills_round_only_the_missing_purchase() {
    let (mut g, ai, uid) = fixture();
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 1.5);
    Arc::make_mut(&mut g.host_unit_facts)
        .get_mut(&uid)
        .unwrap()
        .upgrade
        .as_mut()
        .unwrap()
        .resources
        .as_mut()
        .unwrap()
        .cost = 10.2;
    assert_eq!(ai.army_upgrade_material_wants(&g, 0)[0].amount, 9);
    assert_eq!(
        ai.army_upgrade_material_reserves(&g, 0)[&crate::name!("iron")],
        10.2
    );
}

fn mark_native(g: &mut Game, uid: u32) {
    Arc::make_mut(&mut g.host_unit_facts)
        .get_mut(&uid)
        .unwrap()
        .civ6_id = Some(i64::from(uid));
    Arc::make_mut(&mut g.observed_strategic_income_adjustments)
        .insert(0, BTreeMap::from([(crate::name!("iron"), 0.0)]));
}

#[test]
fn host_cash_price_is_off_in_both_playing_controllers() {
    assert!(!AdvancedAi::new().land_army_asks_the_host_price);
    assert!(!AdvancedAi::targeting(VictoryTarget::Domination).land_army_asks_the_host_price);
}

#[test]
fn native_cash_price_keeps_the_exact_want_and_its_upgrade_reserve() {
    let (mut g, mut ai, uid) = fixture();
    mark_native(&mut g, uid);
    g.players[0].gold = 250.0;
    let (off, old_price) = ai.army_upgrade_material_wants_after_spending(&g, 0, 250.0);
    assert!(!old_price);
    ai.enable_land_army_asks_the_host_price();
    let (on, host_price) = ai.army_upgrade_material_wants_after_spending(&g, 0, 250.0);
    assert!(host_price);
    assert_eq!(off, on);
    assert_eq!(on[0].amount, 10);
    assert_eq!(on[0].gold_reserve, 190.0);
    assert_eq!(g.players[0].gold, 250.0);
    assert_eq!(g.units[&uid].kind, "heavy_chariot");
    ai.disable_land_army_asks_the_host_price();
    assert!(
        !ai.army_upgrade_material_wants_after_spending(&g, 0, 250.0)
            .1
    );
}

#[test]
fn model_units_and_incomplete_native_observations_keep_the_model_price_path() {
    for missing in ["native_id", "income", "gold", "resource_bill"] {
        let (mut g, mut ai, uid) = fixture();
        mark_native(&mut g, uid);
        g.players[0].gold = 250.0;
        ai.enable_land_army_asks_the_host_price();
        match missing {
            "native_id" => {
                Arc::make_mut(&mut g.host_unit_facts)
                    .get_mut(&uid)
                    .unwrap()
                    .civ6_id = None
            }
            "income" => Arc::make_mut(&mut g.observed_strategic_income_adjustments).clear(),
            "gold" => {
                Arc::make_mut(&mut g.host_unit_facts)
                    .get_mut(&uid)
                    .unwrap()
                    .upgrade
                    .as_mut()
                    .unwrap()
                    .cost = None
            }
            "resource_bill" => {
                Arc::make_mut(&mut g.host_unit_facts)
                    .get_mut(&uid)
                    .unwrap()
                    .upgrade
                    .as_mut()
                    .unwrap()
                    .resources = None
            }
            _ => unreachable!(),
        }
        assert!(
            !ai.army_upgrade_material_wants_after_spending(&g, 0, 250.0)
                .1,
            "{missing}"
        );
    }
}

#[test]
fn native_price_does_not_create_a_want_for_stocked_wounded_or_reserved_units() {
    for unavailable in ["stocked", "wounded", "guard", "unfunded", "disabled"] {
        let (mut g, mut ai, uid) = fixture();
        mark_native(&mut g, uid);
        ai.enable_land_army_asks_the_host_price();
        match unavailable {
            "stocked" => {
                g.players[0]
                    .strategic_resources
                    .insert(crate::name!("iron"), 10.0);
            }
            "wounded" => g.units.get_mut(&uid).unwrap().hp = 10,
            "guard" => {
                let settler = g.spawn_test_unit("settler", 0, (4, 5));
                ai.settler_guards.insert(settler, uid);
            }
            "unfunded" => g.players[0].gold = 189.0,
            "disabled" => ai.disable_army_upgrade_materials(),
            _ => unreachable!(),
        }
        let (wants, host_price) = ai.army_upgrade_material_wants_after_spending(&g, 0, 200.0);
        assert!(wants.is_empty(), "{unavailable}");
        assert!(!host_price, "{unavailable}");
    }
}

#[test]
fn a_cash_quote_below_the_transport_floor_keeps_the_existing_barter_want() {
    let (mut g, mut ai, uid) = fixture();
    mark_native(&mut g, uid);
    g.players[0].gold = 250.0;
    let baseline = ai
        .army_upgrade_material_wants_after_spending(&g, 0, 250.0)
        .0;
    assert_eq!(baseline[0].gold_reserve, 190.0);
    ai.enable_land_army_asks_the_host_price();
    for budget in [200.0, 219.0, 219.99] {
        let (wants, cash_price) = ai.army_upgrade_material_wants_after_spending(&g, 0, budget);
        assert_eq!(wants, baseline);
        assert!(!cash_price, "budget {budget}: keep the barter opportunity");
    }
    assert!(
        ai.army_upgrade_material_wants_after_spending(&g, 0, 220.0)
            .1
    );
    g.players[0].gold = 219.99;
    assert!(
        !ai.army_upgrade_material_wants_after_spending(&g, 0, 250.0)
            .1,
        "the host treasury caps the requested remaining budget too"
    );
}
