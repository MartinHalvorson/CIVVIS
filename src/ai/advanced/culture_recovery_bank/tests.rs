use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::install_test_district;

/// Seat 0 with two cities, the Cold War civic and a Theater Square in its
/// capital, so the capital sells a Rock Band for Faith; seat 1 a met rival.
fn culture_seat_in_the_cold_war() -> (Game, u32) {
    let mut g = Game::new_full(2, 32, 22, 41_007, 250, 0, false);
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("each fixture major begins with a Settler");
        let position = g.units[&settler].pos;
        g.found_city_for(pid, position, None);
        for uid in g.player_unit_ids(pid) {
            g.remove_unit(uid);
        }
    }
    g.current = 0;
    g.turn = 200;
    g.record_contact(0, 1);
    g.players[0].civics = g.rules.civics.keys().copied().collect();
    let capital = g.player_city_ids(0)[0];
    if g.unit_purchase_cost(0, capital, "rock_band", "faith")
        .is_none()
    {
        install_test_district(&mut g, capital, "theater_square");
    }
    assert!(
        g.unit_purchase_cost(0, capital, "rock_band", "faith")
            .is_some(),
        "fixture: the capital sells a Rock Band"
    );
    (g, capital)
}

fn recovery() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Recovery,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: 200,
        rush: false,
    }
}

#[test]
fn culture_recovery_spends_the_bank_is_opt_in() {
    opt_in_off_in_both_controllers("culture-recovery-spends-the-bank", |ai| {
        ai.culture_recovery_spends_the_bank
    });
}

/// 132518Z: a named Culture seat in Recovery with a bank many times a Band's
/// price. Stock, Recovery keeps the reserve (no host menu proves Bands are
/// the only Faith units); under the gene it spends.
#[test]
fn a_culture_seat_in_recovery_spends_an_overflowing_bank() {
    let (mut g, capital) = culture_seat_in_the_cold_war();
    let price = g
        .unit_purchase_cost(0, capital, "rock_band", "faith")
        .unwrap();
    g.players[0].faith = 4_384.0;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    assert!(
        !ai.culture_lane_spends(&g, 0, &recovery()),
        "stock: Recovery holds"
    );
    ai.enable_culture_recovery_spends_the_bank();
    assert!(ai.culture_recovery_bank_overflows(&g, 0));
    assert!(ai.culture_lane_spends(&g, 0, &recovery()));
    g.players[0].faith = CULTURE_RECOVERY_BANK_MULTIPLE * price - 1.0;
    assert!(
        !ai.culture_lane_spends(&g, 0, &recovery()),
        "under twice the price, Recovery keeps its reserve"
    );
}

/// Only a named Culture seat: an adaptive or Diplomatic seat in Recovery
/// keeps the reserve whatever the bank.
#[test]
fn only_a_named_culture_seat_lets_the_bank_go() {
    let (mut g, _) = culture_seat_in_the_cold_war();
    g.players[0].faith = 4_384.0;
    for mut ai in [
        AdvancedAi::targeting(VictoryTarget::Diplomacy),
        AdvancedAi::new(),
    ] {
        ai.enable_culture_recovery_spends_the_bank();
        assert!(!ai.culture_lane_spends(&g, 0, &recovery()));
    }
}

/// Nothing for sale: no overflow.
#[test]
fn no_band_on_sale_is_no_overflow() {
    let (mut g, _) = culture_seat_in_the_cold_war();
    g.players[0].faith = 4_384.0;
    g.players[0].civics.remove(&crate::name!("cold_war"));
    g.players[0].civics.remove(&crate::name!("conservation"));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_culture_recovery_spends_the_bank();
    let capital = g.player_city_ids(0)[0];
    if g.unit_purchase_cost(0, capital, "rock_band", "faith")
        .is_none()
        && g.unit_purchase_cost(0, capital, "naturalist", "faith")
            .is_none()
    {
        assert!(!ai.culture_recovery_bank_overflows(&g, 0));
    }
}
