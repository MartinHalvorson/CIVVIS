use super::*;
use crate::ai::advanced::{GrandStrategy, StrategicPlan};
use crate::ai::VictoryTarget;

fn blind_war() -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(3, 48, 28, 73_110, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    g.found_city_for(0, (7, 8), None);
    let alternative = g.found_city_for(2, (14, 8), None);
    g.record_contact(0, 1);
    g.record_contact(0, 2);
    g.at_war.insert((0, 1));
    g.players[1].alive = true;
    g.turn = 80;
    g.current = 0;

    let mut ai = AdvancedAi::new();
    ai.retarget(VictoryTarget::Domination);
    ai.enable_one_war_at_a_time();
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(1));
    (g, ai, alternative)
}

#[test]
fn a_long_blind_war_yields_to_a_known_reachable_domination_city() {
    let (mut g, mut ai, alternative) = blind_war();
    assert_eq!(ai.one_war_peace(&g, 0, 1), None);
    g.turn += g.standard_duration(ONE_WAR_BLIND_FRONT_PATIENCE).max(1);
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_peace(&g, 0, 1), Some(OneWarPeace::BlindFront));
    assert!(ai.one_war_holds_declaration(&g, 0, 2));

    let mut offer_board = g.clone();
    let mut offer_ai = ai.clone();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    offer_ai.advanced_diplomacy(&mut offer_board, 0, &plan);
    assert!(offer_ai.peace_offers.contains(&1));
    assert_eq!(
        offer_board.players[0]
            .counters
            .get("one_war:peace:blind_front"),
        Some(&1)
    );

    // This is a white-peace reason, and only an actual end to the old war
    // unlocks a declaration on the next frontier.
    let mut forced = ai.clone();
    forced.forced_target_player = Some(1);
    assert_eq!(forced.one_war_peace(&g, 0, 1), None);
    g.cities.remove(&alternative);
    assert_eq!(ai.one_war_peace(&g, 0, 1), None);
}

#[test]
fn a_blind_front_is_kept_when_the_other_city_is_out_of_declaration_range() {
    let (mut g, mut ai, alternative) = blind_war();
    let far = (35, 24);
    assert!(g.wdist((7, 8), far) > 18);
    g.cities.get_mut(&alternative).unwrap().pos = far;
    g.turn += g.standard_duration(ONE_WAR_BLIND_FRONT_PATIENCE).max(1);
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_peace(&g, 0, 1), None);
}
