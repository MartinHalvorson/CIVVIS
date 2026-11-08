use super::*;
use crate::game::{HostUnitFacts, HostUnitUpgrade, HostUnitUpgradeResource};
use std::sync::Arc;

fn fixture(count: usize) -> (Game, AdvancedAi, Vec<u32>) {
    let mut g = Game::new_full(2, 24, 16, 15_041, 500, 0, false);
    g.units.clear();
    g.current = 0;
    g.turn = 197;
    g.record_contact(0, 1);
    g.at_war.insert((0, 1));
    g.players[0].gold = 1048.0;
    g.players[0].gold_per_turn = -39.0;
    g.players[0]
        .strategic_resources
        .insert(crate::name!("oil"), 0.0);
    Arc::make_mut(&mut g.observed_strategic_income_adjustments)
        .insert(0, [(crate::name!("oil"), 0.0)].into_iter().collect());
    let ids = (0..count)
        .map(|_| {
            let uid = g.spawn_test_unit("bombard", 0, (4, 4));
            Arc::make_mut(&mut g.host_unit_facts).insert(
                uid,
                HostUnitFacts {
                    civ6_id: Some(i64::from(uid)),
                    upgrade: Some(HostUnitUpgrade {
                        to: Some(crate::name!("artillery")),
                        cost: Some(155.0),
                        resources: Some(HostUnitUpgradeResource {
                            resource: Some(crate::name!("oil")),
                            cost: 1.0,
                        }),
                        blocked: Some("Insufficient Resources".into()),
                    }),
                    ..Default::default()
                },
            );
            uid
        })
        .collect();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_army_upgrade_materials();
    (g, ai, ids)
}

#[test]
fn native_war_buys_the_four_upgrade_bills_and_a_unit_fuel_buffer() {
    let (g, ai, _) = fixture(4);
    let wants = ai.army_upgrade_material_wants(&g, 0);
    assert_eq!(wants.len(), 1);
    assert_eq!(wants[0].resource, "RESOURCE_OIL");
    assert_eq!(wants[0].amount, 44);
    assert_eq!(wants[0].gold_reserve, 759.0);
    assert_eq!(
        ai.army_upgrade_material_reserves(&g, 0)[&crate::name!("oil")],
        16.0
    );
}

#[test]
fn only_affordable_upgrades_are_funded() {
    let (mut g, ai, _) = fixture(4);
    g.players[0].gold = 500.0;
    let wants = ai.army_upgrade_material_wants(&g, 0);
    assert_eq!(wants[0].amount, 22);
    assert_eq!(wants[0].gold_reserve, 449.0);
}

#[test]
fn remaining_frame_budget_funds_a_smaller_group_instead_of_holding_every_upgrade() {
    let (g, ai, _) = fixture(4);
    let (wants, native_defense) = ai.army_upgrade_material_wants_after_spending(&g, 0, 500.0);
    assert!(native_defense);
    assert_eq!(wants[0].amount, 22);
    assert_eq!(wants[0].gold_reserve, 449.0);
    assert!(ai
        .army_upgrade_material_wants_after_spending(&g, 0, 100.0)
        .0
        .is_empty());
    assert!(ai
        .army_upgrade_material_wants_after_spending(&g, 0, f64::NAN)
        .0
        .is_empty());
}

#[test]
fn a_fielded_native_artillery_renews_before_its_buffer_runs_out() {
    let (mut g, ai, ids) = fixture(1);
    g.units.get_mut(&ids[0]).unwrap().kind = crate::name!("artillery");
    Arc::make_mut(&mut g.host_unit_facts)
        .get_mut(&ids[0])
        .unwrap()
        .upgrade = None;
    g.players[0]
        .strategic_resources
        .insert(crate::name!("oil"), 2.0);
    let wants = ai.army_upgrade_material_wants(&g, 0);
    assert_eq!(wants[0].amount, 8);
    assert_eq!(wants[0].gold_reserve, 139.0);
    g.players[0]
        .strategic_resources
        .insert(crate::name!("oil"), 10.0);
    assert!(ai.army_upgrade_material_wants(&g, 0).is_empty());
    g.players[0]
        .strategic_resources
        .insert(crate::name!("oil"), 0.0);
    Arc::make_mut(&mut g.observed_strategic_income_adjustments)
        .get_mut(&0)
        .unwrap()
        .insert(crate::name!("oil"), 1.0);
    assert!(ai.army_upgrade_material_wants(&g, 0).is_empty());
}

#[test]
fn missing_native_observations_and_disabled_policy_do_not_buy() {
    let (mut g, mut ai, _) = fixture(1);
    ai.disable_army_upgrade_materials();
    assert!(ai.army_upgrade_material_wants(&g, 0).is_empty());
    ai.enable_army_upgrade_materials();
    let facts = g.host_unit_facts.clone();
    Arc::make_mut(&mut g.host_unit_facts).clear();
    assert!(ai.army_upgrade_material_wants(&g, 0).is_empty());
    g.host_unit_facts = facts;
    Arc::make_mut(&mut g.observed_strategic_income_adjustments).clear();
    assert!(ai.army_upgrade_material_wants(&g, 0).is_empty());
}

#[test]
fn bad_quotes_wounded_units_and_insufficient_cash_do_not_buy() {
    let (mut g, ai, ids) = fixture(1);
    let uid = ids[0];
    g.units.get_mut(&uid).unwrap().hp = 10;
    assert!(ai.army_upgrade_material_wants(&g, 0).is_empty());
    g.units.get_mut(&uid).unwrap().hp = 100;
    let quote = Arc::make_mut(&mut g.host_unit_facts)
        .get_mut(&uid)
        .unwrap()
        .upgrade
        .as_mut()
        .unwrap();
    quote.cost = Some(f64::NAN);
    assert!(ai.army_upgrade_material_wants(&g, 0).is_empty());
    Arc::make_mut(&mut g.host_unit_facts)
        .get_mut(&uid)
        .unwrap()
        .upgrade
        .as_mut()
        .unwrap()
        .cost = Some(155.0);
    g.players[0].gold = 200.0;
    assert!(ai.army_upgrade_material_wants(&g, 0).is_empty());
}

#[test]
fn peace_and_other_victory_targets_keep_their_existing_policy() {
    let (mut g, ai, _) = fixture(1);
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_army_upgrade_materials();
    assert!(science.army_upgrade_material_wants(&g, 0).is_empty());
    g.at_war.clear();
    assert!(ai.army_upgrade_material_wants(&g, 0).is_empty());
}
