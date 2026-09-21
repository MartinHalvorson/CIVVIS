use super::*;
use std::sync::Arc;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, Pos) {
    let mut g = Game::new_full(2, 32, 20, 370000, 400, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    let base = (2, 8);
    g.found_city_for(0, base, None);
    g.found_city_for(0, (2, 14), None);
    let city = g.found_city_for(1, (9, 8), None);
    g.cities.get_mut(&city).unwrap().wall_hp = 400;
    Arc::make_mut(&mut g.observed_city_strength).insert(city, 100.0);
    Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(city, 400);
    let theater = (8, 8);
    g.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), theater);
    let tile = g.map.tiles.get_mut(&theater).unwrap();
    tile.owner_city = Some(city);
    tile.district = Some(crate::name!("theater_square"));
    tile.pillaged = false;
    g.record_contact(0, 1);
    g.at_war.clear();
    g.current = 0;
    g.turn = 170;
    g.players[0].gold = 10000.0;
    g.players[0]
        .strategic_resources
        .insert(crate::name!("aluminum"), 100.0);
    let bomber = g.spawn_test_unit("bomber", 0, base);
    // Native evidence includes a currently visible Theater Square. One
    // spotter supplies that visibility without a staged capture formation.
    g.spawn_test_unit("scout", 0, (7, 8));
    let observed = Arc::make_mut(&mut g.observed_public_empire_stats);
    observed.entry(0).or_default().domestic_tourists = Some(100);
    observed.entry(1).or_default().foreign_tourists = Some(85);
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
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
    (g, ai, plan, bomber, theater)
}

#[test]
fn urgent_culture_air_sortie_opens_without_a_ground_capture_force() {
    let (mut g, mut ai, plan, bomber, _) = fixture();
    assert!(!ai.campaign_staged_for_war(&g, 0, 1, g.cities[&plan.target_city.unwrap()].pos, true));
    let opening = ai.preferred_war_opening(&g, 0, 1).expect("legal opening");
    let mut forecast = g.speculative_clone();
    forecast.apply(0, &opening).unwrap();
    assert!(forecast.is_at_war(0, 1), "{opening:?}");
    assert!(
        matches!(
            ai.advanced_air_action(&forecast, 0, bomber, &plan),
            Some(Action::AirPillage { .. })
        ),
        "{:?}",
        ai.advanced_air_action(&forecast, 0, bomber, &plan)
    );
    assert!(ai.urgent_culture_air_opening_ready(&g, 0, 1, &plan));
    assert!(
        !g.is_at_war(0, 1),
        "the forecast must leave the real board unchanged"
    );
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g.is_at_war(0, 1));
}

#[test]
fn air_denial_keeps_home_threat_lane_and_urgency_guards() {
    let (g, ai, mut plan, _, _) = fixture();
    plan.threatened_city = g.player_city_ids(0).first().copied();
    assert!(!ai.urgent_culture_air_opening_ready(&g, 0, 1, &plan));
    plan.threatened_city = None;
    let other = AdvancedAi::targeting(VictoryTarget::Science);
    assert!(!other.urgent_culture_air_opening_ready(&g, 0, 1, &plan));
    let mut low = g.speculative_clone();
    Arc::make_mut(&mut low.observed_public_empire_stats)
        .entry(1)
        .or_default()
        .foreign_tourists = Some(20);
    assert!(!ai.urgent_culture_air_opening_ready(&low, 0, 1, &plan));
}

#[test]
fn air_denial_requires_a_ready_bomber_and_a_useful_theater_mission() {
    let (g, ai, plan, bomber, theater) = fixture();
    let mut spent = g.speculative_clone();
    spent.units.get_mut(&bomber).unwrap().attacks_left = 0;
    assert!(!ai.urgent_culture_air_opening_ready(&spent, 0, 1, &plan));
    let mut hurt = g.speculative_clone();
    hurt.units.get_mut(&bomber).unwrap().hp = 79;
    assert!(!ai.urgent_culture_air_opening_ready(&hurt, 0, 1, &plan));
    let mut empty = g.speculative_clone();
    empty.map.tiles.get_mut(&theater).unwrap().pillaged = true;
    assert!(!ai.urgent_culture_air_opening_ready(&empty, 0, 1, &plan));
}
