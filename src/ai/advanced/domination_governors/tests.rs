use super::*;
use crate::ai::advanced::{GrandStrategy, StrategicPlan};
use crate::game::Action;

fn board() -> (Game, AdvancedAi, StrategicPlan) {
    let mut g = Game::new_full(2, 32, 20, 366900, 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    let capital = g.found_city_for(0, (6, 8), None);
    let other = g.found_city_for(0, (12, 8), None);
    g.found_city_for(0, (18, 8), None);
    g.players[0]
        .counters
        .insert("district_governor_titles".into(), 7);
    for (governor, city, promotions) in [
        ("magnus", capital, vec!["provision", "surplus_logistics"]),
        (
            "moksha",
            other,
            vec!["grand_inquisitor", "laying_on_of_hands", "citadel_of_god"],
        ),
    ] {
        g.apply(
            0,
            &Action::AppointGovernor {
                governor: governor.into(),
                city,
            },
        )
        .unwrap();
        for promotion in promotions {
            g.apply(
                0,
                &Action::PromoteGovernor {
                    governor: governor.into(),
                    promotion: promotion.into(),
                },
            )
            .unwrap();
        }
    }
    g.players[1].religion = Some("faith_theirs".into());
    let city = g.cities.get_mut(&capital).unwrap();
    city.pressure.insert("faith_theirs".into(), 400.0);
    city.atheist_pressure = 0.0;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_moksha_defends_the_faithless();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    assert!(ai.home_conversion_threat(&g, 0).is_some());
    (g, ai, plan)
}

#[test]
fn secured_defense_funds_pingala_and_both_yields_before_advanced_moksha() {
    let (mut g, ai, plan) = board();
    for titles in 8..=10 {
        g.players[0]
            .counters
            .insert("district_governor_titles".into(), titles);
        ai.strategic_governors(&mut g, 0, &plan);
        assert_eq!(g.governor_titles_available(0), 0);
        assert_eq!(g.players[0].governor_roster["moksha"].promotions.len(), 3);
    }
    let pingala = &g.players[0].governor_roster["pingala"];
    assert!(pingala.city.is_some());
    assert!(pingala.promotions.contains("researcher"));
    assert!(pingala.promotions.contains("connoisseur"));
}

#[test]
fn multiple_titles_stop_the_economic_detour_at_the_foundation() {
    let (mut g, ai, plan) = board();
    g.players[0]
        .counters
        .insert("district_governor_titles".into(), 11);
    ai.strategic_governors(&mut g, 0, &plan);
    assert_eq!(g.players[0].governor_roster["pingala"].promotions.len(), 2);
    assert!(g.players[0].governor_roster["moksha"]
        .promotions
        .contains("patron_saint"));
}

#[test]
fn unfinished_religious_defense_keeps_its_next_title() {
    let (mut g, ai, plan) = board();
    g.players[0]
        .governor_roster
        .get_mut("moksha")
        .unwrap()
        .promotions
        .remove("citadel_of_god");
    g.players[0].governor_titles_spent -= 1;
    ai.strategic_governors(&mut g, 0, &plan);
    assert!(g.players[0].governor_roster["moksha"]
        .promotions
        .contains("citadel_of_god"));
    assert!(!g.players[0].governor_roster.contains_key("pingala"));
}

#[test]
fn other_lanes_founders_and_disabled_defense_keep_their_priority() {
    let (mut g, mut ai, plan) = board();
    for target in [
        VictoryTarget::Science,
        VictoryTarget::Culture,
        VictoryTarget::Religion,
    ] {
        ai.victory_target = Some(target);
        assert_eq!(ai.governor_priority_for(&g, 0, plan.strategy)[0], "moksha");
    }
    ai.victory_target = Some(VictoryTarget::Domination);
    g.players[0].religion = Some("ours".into());
    assert_eq!(ai.governor_priority_for(&g, 0, plan.strategy)[0], "magnus");
    g.players[0].religion = None;
    ai.disable_moksha_defends_the_faithless();
    assert_eq!(ai.governor_priority_for(&g, 0, plan.strategy)[0], "magnus");
}

#[test]
fn a_temple_keeps_titles_available_for_apostle_promotions() {
    let (mut g, ai, plan) = board();
    let city = g.players[0].governor_roster["moksha"].city.unwrap();
    g.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("temple"));
    g.players[0]
        .counters
        .insert("district_governor_titles".into(), 8);
    ai.strategic_governors(&mut g, 0, &plan);
    assert!(!g.players[0].governor_roster.contains_key("pingala"));
    assert!(g.players[0].governor_roster["moksha"]
        .promotions
        .contains("patron_saint"));
}
