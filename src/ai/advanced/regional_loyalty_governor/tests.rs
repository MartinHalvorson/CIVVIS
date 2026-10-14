use super::*;
use crate::ai::advanced::{GrandStrategy, StrategicPlan};
use std::sync::Arc;

fn board() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new_full(2, 36, 24, 384500, 500, 0, false);
    let target = g.found_city_for(0, (6, 8), None);
    let capital = g.found_city_for(0, (12, 8), None);
    let anchor = g.found_city_for(0, (10, 12), None);
    g.players[0]
        .counters
        .insert("district_governor_titles".into(), 4);
    for (governor, city) in [("magnus", target), ("pingala", capital)] {
        g.apply(
            0,
            &Action::AppointGovernor {
                governor: governor.into(),
                city,
            },
        )
        .unwrap();
    }
    for promotion in ["provision", "surplus_logistics"] {
        g.apply(
            0,
            &Action::PromoteGovernor {
                governor: "magnus".into(),
                promotion: promotion.into(),
            },
        )
        .unwrap();
    }
    g.turn = 102;
    g.cities.get_mut(&target).unwrap().loyalty = 49.8906;
    Arc::make_mut(&mut g.observed_city_loyalty_per_turn).extend([
        (target, -1.34375),
        (capital, 10.0),
        (anchor, 10.0),
    ]);
    let mut ai = AdvancedAi::new();
    ai.enable_regional_loyalty_governor();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan, target, anchor)
}

#[test]
fn separately_earned_titles_complete_the_regional_rescue() {
    let (mut g, ai, plan, target, anchor) = board();
    g.players[0]
        .counters
        .insert("district_governor_titles".into(), 5);
    ai.strategic_governors(&mut g, 0, &plan);
    assert_eq!(
        g.governor_titles_available(0),
        1,
        "bank the first half instead of promoting Pingala"
    );
    assert!(g.players[0].governor_roster["pingala"]
        .promotions
        .is_empty());
    g.turn = 108;
    g.players[0]
        .counters
        .insert("district_governor_titles".into(), 6);
    g.cities.get_mut(&target).unwrap().loyalty = 37.9531;
    Arc::make_mut(&mut g.observed_city_loyalty_per_turn).insert(target, -2.28125);
    ai.strategic_governors(&mut g, 0, &plan);
    let victor = &g.players[0].governor_roster["victor"];
    assert_eq!(victor.city, Some(anchor));
    assert!(victor.promotions.contains("garrison_commander"));
    assert_eq!(g.governor_titles_available(0), 0);
    assert_eq!(g.players[0].governor_roster["magnus"].city, Some(target));
    assert_eq!(g.city_loyalty_per_turn(&g.cities[&target]), -2.28125,
               "native observations remain snapshots; the planner must not depend on speculative recomputation");
}

#[test]
fn ordinary_governor_spending_continues_without_the_regional_gene() {
    let (mut g, mut ai, plan, _, _) = board();
    ai.disable_regional_loyalty_governor();
    ai.base.loyalty_rate_alarm = true; // Published alarm alone must retain its behavior.
    g.players[0]
        .counters
        .insert("district_governor_titles".into(), 5);
    ai.strategic_governors(&mut g, 0, &plan);
    assert_eq!(g.governor_titles_available(0), 0);
    assert!(!g.players[0].governor_roster.contains_key("victor"));
}

#[test]
fn a_resolved_or_unrescuable_emergency_does_not_hold_titles() {
    for (loyalty, rate) in [(49.0, 1.0), (80.0, -1.0), (40.0, -6.0), (3.0, -2.0)] {
        let (mut g, ai, plan, target, _) = board();
        g.cities.get_mut(&target).unwrap().loyalty = loyalty;
        Arc::make_mut(&mut g.observed_city_loyalty_per_turn).insert(target, rate);
        g.players[0]
            .counters
            .insert("district_governor_titles".into(), 5);
        ai.strategic_governors(&mut g, 0, &plan);
        assert_eq!(
            g.governor_titles_available(0),
            0,
            "loyalty={loyalty}, rate={rate}"
        );
        assert!(!g.players[0].governor_roster.contains_key("victor"));
    }
}

#[test]
fn existing_nearby_victor_needs_only_the_promotion_title() {
    let (mut g, ai, plan, _, anchor) = board();
    g.players[0]
        .counters
        .insert("district_governor_titles".into(), 6);
    g.apply(
        0,
        &Action::AppointGovernor {
            governor: "victor".into(),
            city: anchor,
        },
    )
    .unwrap();
    ai.strategic_governors(&mut g, 0, &plan);
    assert!(g.players[0].governor_roster["victor"]
        .promotions
        .contains("garrison_commander"));
    assert!(
        ai.regional_loyalty_governor_action(&g, 0).is_none(),
        "do not double count the pending aura"
    );
}

#[test]
fn no_stable_unoccupied_regional_post_means_no_reserved_title() {
    let (mut g, ai, _, _, anchor) = board();
    Arc::make_mut(&mut g.observed_city_loyalty_per_turn).insert(anchor, -1.0);
    assert!(ai.regional_loyalty_governor_action(&g, 0).is_none());
}

#[test]
fn registry_toggle_is_independent_and_default_off() {
    let (g, _, _, _, _) = board();
    let gene = super::super::genes::GENES
        .iter()
        .find(|gene| gene.tag == "regional-loyalty-governor")
        .unwrap();
    assert!(gene.opt_in());
    let mut ai = AdvancedAi::new();
    ai.base.loyalty_rate_alarm = true;
    assert!(ai.regional_loyalty_governor_action(&g, 0).is_none());
    (gene.enable)(&mut ai);
    ai.base.loyalty_rate_alarm = false;
    assert!(ai.regional_loyalty_governor_action(&g, 0).is_some());
    (gene.disable)(&mut ai);
    assert!(ai.regional_loyalty_governor_action(&g, 0).is_none());
}

#[test]
fn a_saved_title_is_released_when_pressure_recovers() {
    let (mut g, ai, plan, target, _) = board();
    g.players[0]
        .counters
        .insert("district_governor_titles".into(), 5);
    ai.strategic_governors(&mut g, 0, &plan);
    assert_eq!(g.governor_titles_available(0), 1);
    g.turn += 1;
    Arc::make_mut(&mut g.observed_city_loyalty_per_turn).insert(target, 1.0);
    ai.strategic_governors(&mut g, 0, &plan);
    assert_eq!(g.governor_titles_available(0), 0);
    assert!(!g.players[0].governor_roster.contains_key("victor"));
}
