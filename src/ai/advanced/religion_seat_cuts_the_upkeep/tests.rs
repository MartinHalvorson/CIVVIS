use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

fn board() -> (Game, AdvancedAi) {
    let mut g = Game::new(2, 30, 20, 90_948, 250, 0);
    let city = g.found_city_for(0, (6, 8), None);
    g.turn = 71;
    g.players[0].religion = Some("Catholicism".into());
    g.players[0].religion_beliefs = vec!["work_ethic".into()];
    g.players[0].government = Some("classical_republic".into());
    g.players[0].gold = 1.0;
    g.players[0].gold_per_turn = -8.0;
    g.players[0].civics.extend(
        [
            "code_of_laws",
            "craftsmanship",
            "foreign_trade",
            "state_workforce",
            "early_empire",
            "political_philosophy",
            "recorded_history",
            "theology",
        ]
        .into_iter()
        .map(Name::new),
    );
    g.players[0].policies = ["ilkum", "scripture", "charismatic_leader", "urban_planning"]
        .into_iter()
        .map(Name::new)
        .collect();
    let pos = g.cities[&city].pos;
    for (kind, count) in [
        ("archer", 6),
        ("heavy_chariot", 4),
        ("spearman", 2),
        ("catapult", 2),
    ] {
        for _ in 0..count {
            g.spawn_unit(kind, 0, pos);
        }
    }
    g.host_policy_windows.insert(
        0,
        crate::game::HostPolicyWindow {
            turn: 71,
            open: true,
        },
    );
    let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
    ai.solvent_or_hold = true;
    ai.bankruptcy_exits_the_drain_2 = true;
    ai.religion_seat_cuts_the_upkeep = true;
    (g, ai)
}

fn has(g: &Game, card: &str) -> bool {
    g.players[0].policies.contains(&Name::new(card))
}

#[test]
fn religion_upkeep_is_default_off_in_both_controllers() {
    opt_in_off_in_both_controllers("religion-seat-cuts-the-upkeep", |ai| {
        ai.religion_seat_cuts_the_upkeep
    });
}

#[test]
fn religion_upkeep_uses_the_free_window_in_a_full_classical_deck() {
    let (mut g, mut ai) = board();
    let mut off = g.clone();
    ai.disable_religion_seat_cuts_the_upkeep();
    ai.strategic_policies(&mut off, 0, GrandStrategy::Religion);
    assert!(!has(&off, "conscription"));
    ai.enable_religion_seat_cuts_the_upkeep();
    assert_eq!(ai.religion_upkeep_card(&g, 0), Some(("conscription", 14.0)));
    ai.strategic_policies(&mut g, 0, GrandStrategy::Religion);
    assert!(has(&g, "conscription"));
    assert!(
        has(&g, "scripture"),
        "Work Ethic keeps its Faith and Production"
    );
    assert!(
        has(&g, "charismatic_leader"),
        "a suitable ordinary economy card donates capacity first"
    );
    assert_eq!(g.players[0].policies.len(), 4);
    assert_eq!(ai.drain_card_turn.get(), Some(71));
    let army = g.player_unit_ids(0);
    ai.exit_the_drain(&mut g, 0);
    assert_eq!(
        g.player_unit_ids(0),
        army,
        "wait for the newly slotted card's income readback"
    );
}

#[test]
fn religion_upkeep_preserves_work_ethic_through_a_later_gold_card() {
    let (mut g, mut ai) = board();
    ai.urban_planning_fills_the_slot = true;
    g.players[0].civics.insert(crate::name!("civil_service"));
    // The base governor already put Civil Prestige over Ilkum, as in the
    // actual native turn. One route makes Caravansaries worth two Gold.
    g.players[0].policies.remove(&crate::name!("ilkum"));
    g.players[0].policies.insert(crate::name!("civil_prestige"));
    let origin = g.player_city_ids(0)[0];
    let dest = g.found_city_for(1, (20, 8), None);
    g.routes.push(crate::game::TradeRoute {
        origin,
        dest,
        owner: 0,
        ends: g.turn + 20,
    });
    let mut off = g.clone();
    ai.disable_religion_seat_cuts_the_upkeep();
    ai.strategic_policies(&mut off, 0, GrandStrategy::Religion);
    assert!(has(&off, "caravansaries") && has(&off, "scripture"));
    assert!(!has(&off, "conscription"));

    ai.enable_religion_seat_cuts_the_upkeep();
    ai.strategic_policies(&mut g, 0, GrandStrategy::Religion);
    assert!(has(&g, "conscription"));
    assert!(
        has(&g, "scripture"),
        "the later Gold pass must keep Work Ethic"
    );
    assert!(has(&g, "civil_prestige"));
    assert!(
        !has(&g, "caravansaries"),
        "skip two Gold when no ordinary donor remains"
    );
    assert_eq!(g.players[0].policies.len(), 4);
}

#[test]
fn religion_upkeep_prices_native_bills_with_a_floor_per_unit() {
    let (mut g, ai) = board();
    for uid in g.player_unit_ids(0) {
        Arc::make_mut(&mut g.host_unit_facts).insert(
            uid,
            crate::game::HostUnitFacts {
                maintenance: Some(0.0),
                ..Default::default()
            },
        );
    }
    let ids = g.player_unit_ids(0);
    for (uid, bill) in ids.iter().zip([0.5, 2.0, 9.0]) {
        Arc::make_mut(&mut g.host_unit_facts)
            .get_mut(uid)
            .unwrap()
            .maintenance = Some(bill);
    }
    assert_eq!(ai.religion_upkeep_card(&g, 0), Some(("conscription", 2.5)));
    Arc::make_mut(&mut g.host_unit_facts)
        .get_mut(&ids[2])
        .unwrap()
        .maintenance = Some(0.0);
    assert_eq!(
        ai.religion_upkeep_card(&g, 0),
        None,
        "below the two-Gold minimum"
    );
}

#[test]
fn religion_upkeep_waits_until_the_five_turn_runway_is_short() {
    let (mut g, ai) = board();
    for (gold, income, wanted) in [
        (40.0, -8.0, false),
        (39.0, -8.0, true),
        (0.0, 0.0, false),
        (1.0, 8.0, false),
    ] {
        g.players[0].gold = gold;
        g.players[0].gold_per_turn = income;
        assert_eq!(ai.religion_upkeep_card(&g, 0).is_some(), wanted);
    }
}

#[test]
fn religion_upkeep_keeps_the_discount_while_removing_it_restores_a_deficit() {
    let (mut g, mut ai) = board();
    g.players[0].policies.remove(&crate::name!("ilkum"));
    g.players[0].policies.insert(crate::name!("conscription"));
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 6.0;
    assert_eq!(ai.religion_upkeep_card(&g, 0), Some(("conscription", 14.0)));
    ai.strategic_policies(&mut g, 0, GrandStrategy::Religion);
    assert!(has(&g, "conscription"));
    // The base planner may already have removed it during this turn.
    ai.turn_start_policies.insert(crate::name!("conscription"));
    g.players[0].policies.remove(&crate::name!("conscription"));
    assert_eq!(ai.religion_upkeep_card(&g, 0), Some(("conscription", 14.0)));
    g.players[0].gold_per_turn = 14.0;
    assert_eq!(ai.religion_upkeep_card(&g, 0), None);
}

#[test]
fn religion_upkeep_prefers_the_available_successor_and_does_not_stack_discounts() {
    let (mut g, ai) = board();
    g.players[0].civics.insert(crate::name!("mobilization"));
    assert_eq!(
        ai.religion_upkeep_card(&g, 0),
        Some(("levee_en_masse", 16.0))
    );
    g.players[0].policies.insert(crate::name!("levee_en_masse"));
    assert_eq!(
        ai.religion_upkeep_card(&g, 0),
        Some(("levee_en_masse", 16.0))
    );
}

#[test]
fn religion_upkeep_requires_the_founder_target_and_an_offered_card() {
    for case in [
        "off",
        "no religion",
        "other target",
        "disabled victory",
        "no civic",
    ] {
        let (mut g, mut ai) = board();
        match case {
            "off" => ai.disable_religion_seat_cuts_the_upkeep(),
            "no religion" => g.players[0].religion = None,
            "other target" => ai.victory_target = Some(VictoryTarget::Science),
            "disabled victory" => g.victory_conditions.religious = false,
            "no civic" => {
                g.players[0].civics.remove(&crate::name!("state_workforce"));
            }
            _ => unreachable!(),
        }
        assert_eq!(ai.religion_upkeep_card(&g, 0), None, "{case}");
    }
}

#[test]
fn religion_upkeep_never_opens_a_paid_host_window() {
    let (mut g, ai) = board();
    g.host_policy_windows.get_mut(&0).unwrap().open = false;
    let before = g.players[0].policies.clone();
    ai.strategic_policies(&mut g, 0, GrandStrategy::Religion);
    assert_eq!(g.players[0].policies, before);
    assert_eq!(g.players[0].gold, 1.0);
    assert_eq!(ai.drain_card_turn.get(), None);
}

#[test]
fn religion_upkeep_protects_theological_strength_work_ethic_and_paying_gold() {
    let (g, _) = board();
    for card in [
        "scripture",
        "religious_orders",
        "revelation",
        "limitanei",
        "praetorium",
    ] {
        assert!(
            !AdvancedAi::religion_upkeep_may_displace(&g, 0, card, 14.0),
            "{card}"
        );
    }
    assert!(AdvancedAi::religion_upkeep_may_displace(
        &g,
        0,
        "urban_planning",
        14.0
    ));
    assert!(
        !AdvancedAi::religion_upkeep_may_displace(&g, 0, "god_king", 2.0),
        "one Gold gained after giving up capital Gold is below the floor"
    );
    let mut ordinary = g;
    ordinary.players[0].religion_beliefs.clear();
    assert!(AdvancedAi::religion_upkeep_may_displace(
        &ordinary,
        0,
        "scripture",
        14.0
    ));
}
