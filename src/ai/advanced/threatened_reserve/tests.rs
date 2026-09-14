use super::*;
use crate::game::{HostPurchaseEntry, Item};
use crate::setup::GameSpeed;
use std::collections::{BTreeMap, BTreeSet};

fn board() -> (Game, u32, u32, StrategicPlan) {
    let mut g = Game::new_full(2, 40, 24, 914_374_002, 250, 0, false);
    g.game_speed = GameSpeed::Online;
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    let first = g.found_city_for(0, (8, 8), None);
    let second = g.found_city_for(0, (16, 8), None);
    g.found_city_for(1, (24, 8), None);
    g.players[0]
        .techs
        .extend([crate::name!("archery"), crate::name!("engineering")]);
    assert!(g.unit_purchase_cost(0, first, "archer", "gold").is_some());
    let plan = StrategicPlan {
        strategy: super::super::GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: Some(first),
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, first, second, plan)
}

fn ai() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_threatened_city_reserve_2();
    ai
}

fn quotes(g: &mut Game, offers: &[(u32, &[(&str, f64)])]) {
    let menus = offers
        .iter()
        .map(|(cid, entries)| {
            (
                *cid,
                entries
                    .iter()
                    .map(|(unit, gold)| {
                        (
                            format!("unit:{unit}"),
                            HostPurchaseEntry {
                                gold: Some(*gold),
                                faith: None,
                            },
                        )
                    })
                    .collect(),
            )
        })
        .collect();
    g.replace_host_menus(BTreeMap::new(), menus, BTreeMap::new());
}

#[test]
fn reserve_versions_are_exclusive_and_off_by_default() {
    super::super::test_support::opt_in_off_in_both_controllers("threatened-city-reserve-2", |ai| {
        ai.threatened_city_reserve_2
    });
    let (g, _, _, plan) = board();
    let mut ai = ai();
    assert!(!ai.threatened_city_reserve);
    ai.enable_threatened_city_reserve();
    assert!(!ai.threatened_city_reserve_2);
    assert_eq!(ai.local_defender_gold_floor(&g, 0, &plan), 0.0);
    ai.enable_threatened_city_reserve_2();
    assert!(!ai.threatened_city_reserve);
    ai.disable_threatened_city_reserve_2();
    assert_eq!(ai.threatened_city_gold_floor(&g, 0, &plan), 0.0);
}

#[test]
fn local_reserve_replaces_a_catapult_bill_with_the_available_archer() {
    let (mut g, cid, _, plan) = board();
    g.players[0].gold = 0.0;
    let archer = g.unit_purchase_cost(0, cid, "archer", "gold").unwrap();
    assert!(g.can_produce(
        0,
        cid,
        &Item::Unit {
            unit: crate::name!("catapult")
        }
    ));
    let mut controller = ai();
    assert_eq!(controller.threatened_city_gold_floor(&g, 0, &plan), archer);
    assert_eq!(
        controller.reserve_for_the_threatened_city(&g, 0, &plan, 1.0),
        archer
    );
    assert_eq!(
        controller.reserve_for_the_threatened_city(&g, 0, &plan, archer + 100.0),
        archer + 100.0
    );
    controller.enable_threatened_city_reserve();
    assert!(controller.threatened_city_gold_floor(&g, 0, &plan) > archer);
    assert_eq!(
        g.players[0].gold, 0.0,
        "a quote neither requires nor spends the bank"
    );
}

#[test]
fn local_reserve_uses_the_engine_discount() {
    let (mut g, cid, _, plan) = board();
    let ordinary = ai().threatened_city_gold_floor(&g, 0, &plan);
    g.players[0].government = Some("democracy".to_string());
    let discounted = g.unit_purchase_cost(0, cid, "archer", "gold").unwrap();
    assert!(discounted < ordinary);
    assert_eq!(ai().threatened_city_gold_floor(&g, 0, &plan), discounted);
}

#[test]
fn local_reserve_reads_only_the_threatened_citys_host_offer_and_refusal() {
    let (mut g, first, second, mut plan) = board();
    quotes(
        &mut g,
        &[
            (first, &[("warrior", 42.0)]),
            (second, &[("archer", 900.0)]),
        ],
    );
    assert_eq!(ai().threatened_city_gold_floor(&g, 0, &plan), 42.0);
    g.replace_blocked_purchases(BTreeMap::from([(
        first,
        BTreeSet::from(["unit:warrior".to_string()]),
    )]));
    assert_eq!(ai().threatened_city_gold_floor(&g, 0, &plan), 0.0);
    assert_eq!(
        ai().reserve_for_the_threatened_city(&g, 0, &plan, 75.0),
        75.0
    );
    plan.threatened_city = Some(second);
    assert_eq!(ai().threatened_city_gold_floor(&g, 0, &plan), 900.0);
    plan.threatened_city = None;
    assert_eq!(ai().threatened_city_gold_floor(&g, 0, &plan), 0.0);
    plan.threatened_city = Some(g.player_city_ids(1)[0]);
    assert_eq!(ai().threatened_city_gold_floor(&g, 0, &plan), 0.0);
}

#[test]
fn local_reserve_does_not_buy_a_siege_or_recon_garrison() {
    let (mut g, cid, _, plan) = board();
    quotes(
        &mut g,
        &[(
            cid,
            &[("archer", 53.0), ("catapult", 900.0), ("ranger", 9_999.0)],
        )],
    );
    assert_eq!(ai().threatened_city_gold_floor(&g, 0, &plan), 53.0);
    // Even a cheap weaker body does not replace the quoted strongest defender.
    quotes(&mut g, &[(cid, &[("warrior", 1.0), ("archer", 53.0)])]);
    assert_eq!(ai().threatened_city_gold_floor(&g, 0, &plan), 53.0);
}

#[test]
fn local_reserve_releases_a_purchase_blocked_by_an_existing_or_queued_defender() {
    let (mut g, cid, _, plan) = board();
    let uid = g.spawn_unit("archer", 0, g.cities[&cid].pos);
    assert_eq!(ai().threatened_city_gold_floor(&g, 0, &plan), 0.0);
    g.remove_unit(uid);
    g.cities.get_mut(&cid).unwrap().queue.push(Item::Unit {
        unit: crate::name!("archer"),
    });
    assert_eq!(ai().threatened_city_gold_floor(&g, 0, &plan), 0.0);
}

#[test]
fn local_reserve_preserves_the_selected_native_emergency_signal() {
    let (mut g, cid, _, mut plan) = board();
    plan.threatened_city = None;
    g.turn = 50;
    g.cities.get_mut(&cid).unwrap().hp = 150;
    g.cities.get_mut(&cid).unwrap().last_attacked = 49;
    let mut controller = ai();
    assert_eq!(controller.threatened_city_gold_floor(&g, 0, &plan), 0.0);
    controller.enable_native_emergency_purchase();
    let quote = g.unit_purchase_cost(0, cid, "archer", "gold").unwrap();
    assert_eq!(controller.threatened_city_gold_floor(&g, 0, &plan), quote);
    g.turn += 10;
    assert_eq!(controller.threatened_city_gold_floor(&g, 0, &plan), 0.0);
}
