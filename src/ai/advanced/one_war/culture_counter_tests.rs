use super::*;
use crate::ai::advanced::StrategicPlan;
use std::sync::Arc;

/// A Domination seat beside a walled culture rival at match point, with no
/// aircraft and no army staged on its city.
fn fixture(armies: usize) -> (Game, AdvancedAi, StrategicPlan) {
    let mut g = Game::new_full(2, 32, 20, 370_001, 400, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    g.found_city_for(0, (2, 8), None);
    g.found_city_for(0, (2, 14), None);
    let city = g.found_city_for(1, (9, 8), None);
    g.cities.get_mut(&city).unwrap().wall_hp = 400;
    g.record_contact(0, 1);
    g.at_war.clear();
    g.current = 0;
    g.turn = 170;
    g.players[0].gold = 10000.0;
    for y in 0..armies {
        g.spawn_test_unit("musketman", 0, (1, 2 + y as i32));
    }
    g.spawn_test_unit("musketman", 1, (12, 8));
    let observed = Arc::make_mut(&mut g.observed_public_empire_stats);
    observed.entry(0).or_default().domestic_tourists = Some(100);
    observed.entry(1).or_default().foreign_tourists = Some(85);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.coalition_before_war = false;
    ai.coalition_before_war_2 = false;
    ai.coalition_before_war_3 = false;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(city),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    };
    assert!(ai.urgent_victory_threat(&g, 1));
    assert_eq!(
        ai.rival_victory_pressure(&g, 1).strategy,
        GrandStrategy::Culture
    );
    assert!(!ai.campaign_staged_for_war(&g, 0, 1, g.cities[&city].pos, true));
    (g, ai, plan)
}

/// See `culture_counter_due`: with the gene, an urgent culture rival is
/// declared on without a staged siege once we hold the ratio over it.
#[test]
fn an_urgent_culture_rival_is_declared_on_without_a_staged_siege() {
    for gene in [false, true] {
        let (mut g, mut ai, plan) = fixture(4);
        assert!(g.military_power(0) >= CULTURE_COUNTER_RATIO * g.military_power(1));
        if gene {
            ai.enable_culture_counter_declares();
        }
        ai.advanced_diplomacy(&mut g, 0, &plan);
        assert_eq!(g.is_at_war(0, 1), gene, "gene {gene}");
    }
}

/// Under the ratio the declaration still waits for the army.
#[test]
fn the_culture_counter_needs_the_ratio() {
    let (mut g, mut ai, plan) = fixture(1);
    assert!(g.military_power(0) < CULTURE_COUNTER_RATIO * g.military_power(1));
    ai.enable_culture_counter_declares();
    assert!(!ai.culture_counter_due(&g, 0, 1));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1));
}
