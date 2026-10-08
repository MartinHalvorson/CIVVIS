use super::*;
use std::sync::Arc;

/// A faith founded by seat 1 holds seats 1, 2 and 3; our majority is still
/// out. Its Missionary stands beside our garrison in our borders, a war on
/// seat 2 is running, and the faith out-guns us: ours 200, the faith 250,
/// seat 2 100.
fn last_holdout() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut g = Game::new_full(4, 40, 24, 372100, 250, 0, false);
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
    g.found_city_for(2, (18, 8), None);
    g.found_city_for(3, (28, 8), None);
    g.players[1].religion = Some("Orthodoxy".into());
    for pid in [1, 2, 3] {
        Arc::make_mut(&mut g.observed_majority_religion).insert(pid, "Orthodoxy".into());
    }
    for rival in 1..4 {
        g.record_contact(0, rival);
    }
    g.at_war.clear();
    g.current = 0;
    g.turn = 93;
    g.players[0].gold = 10000.0;
    g.spawn_test_unit("spearman", 0, (4, 8));
    let missionary = g.spawn_test_unit("missionary", 1, (4, 8));
    g.units.get_mut(&missionary).unwrap().religion = Some("Orthodoxy".into());
    g.apply(0, &Action::DeclareWar { player: 2 }).unwrap();
    Arc::make_mut(&mut g.observed_military_power).extend([(0, 200.0), (1, 250.0), (2, 100.0)]);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_religious_match_point_defence();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    assert!(AdvancedAi::we_are_the_last_holdout(&g, 0, 1));
    assert!(g.is_at_war(0, 2) && !g.is_at_war(0, 1));
    (g, ai, plan, missionary)
}

/// G425's gate: 200 against 250 is short of the shipped floor and of the
/// 1.2 edge beside a running war, so the shipped seat holds. As the faith's
/// last holdout, 0.7 times its steady power opens the war and condemns the
/// spreader.
#[test]
fn the_last_holdout_intercepts_below_the_army_gates() {
    let (mut g, mut ai, plan, missionary) = last_holdout();
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "off: the power gates hold");
    assert!(g.units.contains_key(&missionary));

    let (mut g, mut ai, plan, missionary) = last_holdout();
    ai.enable_match_point_interception_ignores_power();
    assert!(ai.last_holdout_power_suffices(&g, 0, 1));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g.is_at_war(0, 1), "the last holdout opens the interception");
    assert!(
        !g.units.contains_key(&missionary),
        "the spreader is condemned"
    );
    assert!(g.is_at_war(0, 2), "the running war is untouched");
}

/// Below 0.7 times the faith's steady power, with our majority already on
/// the faith (so we are not its holdout), or with a city of ours under
/// threat, nothing opens.
#[test]
fn the_floor_the_holdout_and_the_city_guard_still_hold() {
    let (mut g, mut ai, plan, missionary) = last_holdout();
    ai.enable_match_point_interception_ignores_power();
    Arc::make_mut(&mut g.observed_military_power).insert(0, 170.0);
    assert!(!ai.last_holdout_power_suffices(&g, 0, 1));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "170 is short of 0.7 times 250");
    assert!(g.units.contains_key(&missionary));

    let (mut g, mut ai, plan, missionary) = last_holdout();
    ai.enable_match_point_interception_ignores_power();
    Arc::make_mut(&mut g.observed_majority_religion).insert(0, "Orthodoxy".into());
    assert!(!AdvancedAi::we_are_the_last_holdout(&g, 0, 1));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "converted, we are not the holdout");
    assert!(g.units.contains_key(&missionary));

    let (mut g, mut ai, plan, missionary) = last_holdout();
    ai.enable_match_point_interception_ignores_power();
    for _ in 0..3 {
        g.spawn_test_unit("swordsman", 2, (3, 9));
    }
    assert!(ai.threatened_city(&g, 0).is_some());
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "a city under threat comes first");
    assert!(g.units.contains_key(&missionary));
}
