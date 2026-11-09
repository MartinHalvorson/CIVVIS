//! Occupied policy seats during a measured Loyalty emergency.

use super::*;

/// The native t132 policy shape: every Monarchy slot filled, an established
/// governor, and a city above the old level alarm but under nine turns from
/// revolt. The Campus makes Natural Philosophy a wanted card too.
fn emergency() -> (Game, AdvancedAi, u32) {
    let mut g = Game::new(2, 24, 16, 132_009, 200, 0);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = g.player_city_ids(0)[0];
    g.turn = 132;
    g.players[0].government = Some("monarchy".into());
    g.players[0].civics.extend([
        crate::name!("code_of_laws"),
        crate::name!("craftsmanship"),
        crate::name!("early_empire"),
        crate::name!("political_philosophy"),
        crate::name!("recorded_history"),
        crate::name!("theology"),
        crate::name!("feudalism"),
        crate::name!("divine_right"),
        crate::name!("medieval_faires"),
    ]);
    g.players[0].policies.extend([
        crate::name!("discipline"),
        crate::name!("feudal_contract"),
        crate::name!("scripture"),
        crate::name!("natural_philosophy"),
        crate::name!("urban_planning"),
        crate::name!("merchant_confederation"),
    ]);
    g.players[0].envoys = vec![(5, 15)];
    g.players[0].gold = 155.0;
    g.players[0].gold_per_turn = 18.0;
    g.players[0].governors = vec![cid];
    g.players[0].governor_roster.insert(
        "magnus".into(),
        GovernorState {
            city: Some(cid),
            assigned_turn: 100,
            disabled_until: 0,
            promotions: BTreeSet::new(),
        },
    );
    let city = g.cities.get_mut(&cid).unwrap();
    city.loyalty = 80.1875;
    city.districts.insert(crate::name!("campus"), city.pos);
    std::sync::Arc::make_mut(&mut g.observed_city_loyalty_per_turn).insert(cid, -9.125);
    g.host_policy_windows.insert(
        0,
        crate::game::HostPolicyWindow {
            turn: g.turn,
            open: true,
        },
    );
    let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
    ai.base.loyalty_rate_alarm = true;
    ai.research_economy = true;
    ai.merchant_confederation_earns_its_slot_2 = true;
    ai.urban_planning_fills_the_slot = true;
    (g, ai, cid)
}

fn has(g: &Game, card: &str) -> bool {
    g.players[0].policies.contains(&Name::new(card))
}

#[test]
fn loyalty_repair_takes_the_wanted_diplomatic_seat_and_stays_there() {
    let (mut g, ai, _) = emergency();
    assert!(ai.loyalty_policy_needed(&g, 0));
    let before = g.players[0].policies.clone();
    ai.strategic_policies(&mut g, 0, GrandStrategy::Religion);
    assert!(has(&g, "praetorium"));
    assert!(!has(&g, "merchant_confederation"));
    let mut expected = before;
    expected.remove(&crate::name!("merchant_confederation"));
    expected.insert(crate::name!("praetorium"));
    assert_eq!(g.players[0].policies, expected);
    ai.strategic_policies(&mut g, 0, GrandStrategy::Religion);
    assert_eq!(
        g.players[0].policies, expected,
        "the paying envoy card must not undo the repair"
    );
}

#[test]
fn frozen_loyalty_repair_keeps_the_wanted_envoy_card() {
    let (mut g, mut ai, _) = emergency();
    ai.base.loyalty_rate_alarm = false;
    ai.strategic_policies(&mut g, 0, GrandStrategy::Religion);
    assert!(has(&g, "merchant_confederation"));
    assert!(!has(&g, "praetorium"));
}

#[test]
fn stable_or_distant_decline_does_not_displace_the_envoy_card() {
    for rate in [0.0, 1.0, -0.25] {
        let (mut g, ai, cid) = emergency();
        std::sync::Arc::make_mut(&mut g.observed_city_loyalty_per_turn).insert(cid, rate);
        assert!(!ai.loyalty_policy_needed(&g, 0));
        ai.strategic_policies(&mut g, 0, GrandStrategy::Religion);
        assert!(has(&g, "merchant_confederation"));
        assert!(!has(&g, "praetorium"));
    }
}

#[test]
fn an_ungoverned_city_cannot_spend_a_slot_on_governor_loyalty() {
    let (mut g, ai, _) = emergency();
    g.players[0].governors.clear();
    g.players[0].governor_roster.clear();
    assert!(!ai.loyalty_policy_needed(&g, 0));
    ai.strategic_policies(&mut g, 0, GrandStrategy::Religion);
    assert!(has(&g, "merchant_confederation"));
    assert!(!has(&g, "praetorium"));
}

#[test]
fn the_repair_cannot_change_a_closed_host_policy_window() {
    let (mut g, ai, _) = emergency();
    g.host_policy_windows.get_mut(&0).unwrap().open = false;
    let before = g.players[0].policies.clone();
    let gold = g.players[0].gold;
    ai.strategic_policies(&mut g, 0, GrandStrategy::Religion);
    assert_eq!(g.players[0].policies, before);
    assert_eq!(g.players[0].gold, gold);
}

#[test]
fn the_repair_requires_an_available_card() {
    let (mut g, ai, _) = emergency();
    g.players[0]
        .civics
        .remove(&crate::name!("recorded_history"));
    assert!(!g
        .available_policies(0)
        .contains(&crate::name!("praetorium")));
    ai.strategic_policies(&mut g, 0, GrandStrategy::Religion);
    assert!(has(&g, "merchant_confederation"));
    assert!(!has(&g, "praetorium"));
}

#[test]
fn the_repair_does_not_borrow_an_ordinary_lanes_wildcard_seat() {
    let (mut g, ai, _) = emergency();
    g.players[0].government = Some("oligarchy".into());
    g.players[0].policies = [
        "discipline",
        "feudal_contract",
        "scripture",
        "urban_planning",
    ]
    .into_iter()
    .map(Name::new)
    .collect();
    assert!(AdvancedAi::policy_swap_fits(
        &g,
        0,
        &crate::name!("urban_planning"),
        "praetorium"
    ));
    ai.strategic_policies(&mut g, 0, GrandStrategy::Religion);
    assert!(!has(&g, "praetorium"));
}

#[test]
fn an_active_bankruptcy_drain_keeps_its_paying_envoy_card() {
    let (mut g, mut ai, _) = emergency();
    ai.bankruptcy_exits_the_drain = true;
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = -20.0;
    ai.strategic_policies(&mut g, 0, GrandStrategy::Religion);
    assert!(has(&g, "merchant_confederation"));
    assert!(!has(&g, "praetorium"));
}
