use super::super::*;
use super::{
    HARVEST_CITY_STATE_FROM_TURN, HARVEST_DECLARE_PATIENCE, HARVEST_FROM_TURN,
    HARVEST_MUSTER_BODIES,
};

/// Seat 0 with two cities and a Warrior; major 1 far away; city-state 2 with
/// one population-6 city four tiles from our capital, met and explored.
fn fixture() -> (Game, u32) {
    let mut g = Game::new_full(3, 40, 20, 4_354_001, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (4, 8), None);
    g.found_city_for(0, (4, 14), None);
    g.found_city_for(1, (34, 8), None);
    let city = g.found_city_for(2, (8, 8), None);
    g.players[2].is_minor = true;
    g.cities.get_mut(&city).unwrap().pop = 6;
    for pid in 0..3 {
        for other in pid + 1..3 {
            g.record_contact(pid, other);
        }
    }
    g.players[0].explored.insert((8, 8));
    for _ in 0..4 {
        g.spawn_test_unit("warrior", 0, (4, 8));
    }
    g.current = 0;
    g.turn = HARVEST_CITY_STATE_FROM_TURN + 5;
    (g, city)
}

fn harvest_ai() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_city_state_harvest();
    ai
}

#[test]
fn off_names_nothing() {
    let (mut g, _) = fixture();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.maintain_city_state_harvest(&mut g, 0);
    assert_eq!(ai.city_state_harvest_target, None);
    assert!(!g.players[0]
        .counters
        .contains_key("city_state_harvest:named"));
}

#[test]
fn a_soft_city_state_city_beside_us_is_named() {
    let (mut g, city) = fixture();
    let mut ai = harvest_ai();
    assert_eq!(ai.city_state_harvest_candidate(&g, 0), Some((2, city)));
    ai.maintain_city_state_harvest(&mut g, 0);
    let target = ai.city_state_harvest_target.clone().expect("named");
    assert_eq!((target.player, target.city), (2, city));
    assert_eq!(
        g.players[0].counters.get("city_state_harvest:named"),
        Some(&1)
    );
}

#[test]
fn not_before_the_turn_floor_or_when_unexplored() {
    let (mut g, _) = fixture();
    let ai = harvest_ai();
    g.players[0].explored.remove(&(8, 8));
    assert_eq!(ai.city_state_harvest_candidate(&g, 0), None, "fog-honest");
    let (mut g, _) = fixture();
    let mut ai = harvest_ai();
    g.turn = HARVEST_FROM_TURN - 1;
    ai.maintain_city_state_harvest(&mut g, 0);
    assert_eq!(ai.city_state_harvest_target, None);
}

#[test]
fn an_invested_or_foreign_suzerainty_is_kept() {
    let (mut g, _) = fixture();
    let ai = harvest_ai();
    g.players[1].envoys = vec![(2, 3)];
    assert_eq!(g.suzerain_of(2), Some(1));
    assert_eq!(
        ai.city_state_harvest_candidate(&g, 0),
        None,
        "a major's city-state"
    );
    let (mut g, _) = fixture();
    g.players[0].envoys = vec![(2, 2)];
    assert_eq!(
        ai.city_state_harvest_candidate(&g, 0),
        None,
        "our own investment"
    );
}

#[test]
fn a_far_or_small_or_strong_city_state_is_skipped() {
    let ai = harvest_ai();
    let (mut g, city) = fixture();
    g.cities.get_mut(&city).unwrap().pop = 3;
    assert_eq!(ai.city_state_harvest_candidate(&g, 0), None, "small");
    let (mut g, _) = fixture();
    for _ in 0..8 {
        g.spawn_test_unit("warrior", 2, (9, 8));
    }
    assert_eq!(ai.city_state_harvest_candidate(&g, 0), None, "strong");
}

#[test]
fn the_overlay_points_the_war_at_the_city_under_the_lane() {
    let (mut g, city) = fixture();
    let mut ai = harvest_ai();
    ai.maintain_city_state_harvest(&mut g, 0);
    let science = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    let mut before = science.clone();
    ai.apply_city_state_harvest_to_strategy(&mut before);
    assert_eq!(
        before, science,
        "named, not yet at war: the plan is untouched"
    );
    ai.maintain_city_state_harvest(&mut g, 0);
    assert!(g.is_at_war(0, 2), "the muster declared");
    let mut plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    ai.apply_city_state_harvest_to_strategy(&mut plan);
    assert_eq!(
        plan.strategy,
        GrandStrategy::Science,
        "the lane keeps its strategy"
    );
    assert_eq!(plan.target_player, Some(2));
    assert_eq!(plan.target_city, Some(city));

    let mut recovery = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        ..plan.clone()
    };
    recovery.target_player = None;
    ai.apply_city_state_harvest_to_strategy(&mut recovery);
    assert_eq!(recovery.strategy, GrandStrategy::Recovery, "home first");
}

#[test]
fn a_major_war_or_spent_patience_releases_the_name() {
    let (mut g, _) = fixture();
    let mut ai = harvest_ai();
    ai.maintain_city_state_harvest(&mut g, 0);
    assert!(ai.city_state_harvest_target.is_some());
    g.at_war.insert((0, 1));
    ai.maintain_city_state_harvest(&mut g, 0);
    assert_eq!(ai.city_state_harvest_target, None, "a major war");
    assert_eq!(
        g.players[0].counters.get("city_state_harvest:released"),
        Some(&1)
    );

    let (mut g, _) = fixture();
    let mut ai = harvest_ai();
    ai.maintain_city_state_harvest(&mut g, 0);
    for uid in g.player_unit_ids(0) {
        g.remove_unit(uid);
    }
    ai.maintain_city_state_harvest(&mut g, 0);
    assert!(!g.is_at_war(0, 2), "no muster, no declaration");
    g.turn += HARVEST_DECLARE_PATIENCE;
    ai.maintain_city_state_harvest(&mut g, 0);
    assert_eq!(ai.city_state_harvest_target, None, "no declaration in time");
    ai.maintain_city_state_harvest(&mut g, 0);
    assert_eq!(ai.city_state_harvest_target, None, "cooldown");
}

#[test]
fn the_capture_is_counted() {
    let (mut g, city) = fixture();
    let mut ai = harvest_ai();
    ai.maintain_city_state_harvest(&mut g, 0);
    g.at_war.insert((0, 2));
    ai.maintain_city_state_harvest(&mut g, 0);
    assert_eq!(
        g.players[0].counters.get("city_state_harvest:declared"),
        Some(&1)
    );
    g.cities.get_mut(&city).unwrap().owner = 0;
    ai.maintain_city_state_harvest(&mut g, 0);
    assert_eq!(ai.city_state_harvest_target, None);
    assert_eq!(
        g.players[0].counters.get("city_state_harvest:captured"),
        Some(&1)
    );
}

#[test]
fn a_rebel_free_city_beside_us_is_named() {
    let (mut g, city) = fixture();
    g.players[2].is_barbarian = true;
    g.players[2].is_free_city = true;
    let mut ai = harvest_ai();
    assert_eq!(ai.city_state_harvest_candidate(&g, 0), Some((2, city)));
    ai.maintain_city_state_harvest(&mut g, 0);
    let target = ai.city_state_harvest_target.clone().expect("named");
    assert_eq!(
        target.declared_turn,
        Some(g.turn),
        "a Free City is always at war"
    );
}

#[test]
fn a_muster_beside_the_city_declares_and_a_thin_one_does_not() {
    let (mut g, _) = fixture();
    let mut ai = harvest_ai();
    ai.maintain_city_state_harvest(&mut g, 0);
    ai.maintain_city_state_harvest(&mut g, 0);
    assert!(g.is_at_war(0, 2));
    assert_eq!(
        g.players[0].counters.get("city_state_harvest:declared"),
        Some(&1)
    );

    let (mut g, _) = fixture();
    let mut ai = harvest_ai();
    ai.maintain_city_state_harvest(&mut g, 0);
    let ours = g.player_unit_ids(0);
    for uid in ours.iter().skip(HARVEST_MUSTER_BODIES - 1) {
        g.remove_unit(*uid);
    }
    ai.maintain_city_state_harvest(&mut g, 0);
    assert!(!g.is_at_war(0, 2), "two bodies are not a muster");
}

#[test]
fn a_city_state_waits_for_the_expansion_window_and_skips_a_science_seat() {
    let (mut g, city) = fixture();
    let ai = harvest_ai();
    g.turn = HARVEST_CITY_STATE_FROM_TURN - 1;
    assert_eq!(
        ai.city_state_harvest_candidate(&g, 0),
        None,
        "inside the window"
    );
    g.turn = HARVEST_CITY_STATE_FROM_TURN;
    assert_eq!(ai.city_state_harvest_candidate(&g, 0), Some((2, city)));

    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_city_state_harvest();
    assert_eq!(
        science.city_state_harvest_candidate(&g, 0),
        None,
        "a Science seat"
    );

    g.players[2].is_barbarian = true;
    g.players[2].is_free_city = true;
    g.turn = HARVEST_FROM_TURN + 1;
    assert_eq!(
        science.city_state_harvest_candidate(&g, 0),
        Some((2, city)),
        "our rebel Free City is retaken early and on any lane"
    );
}
