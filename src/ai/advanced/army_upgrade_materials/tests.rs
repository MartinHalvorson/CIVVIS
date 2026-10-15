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
