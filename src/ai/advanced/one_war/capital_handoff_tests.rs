use super::*;
use crate::ai::advanced::{GrandStrategy, StrategicPlan};
use crate::ai::VictoryTarget;
use crate::game::Action;
use std::sync::Arc;

fn captured_front() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut g = Game::new_full(4, 36, 22, 91_120, 500, 0, false);
    let ids: Vec<_> = g.units.keys().copied().collect();
    for id in ids {
        g.remove_unit(id);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    g.found_city_for(0, (0, 8), None);
    let capital = g.found_city_for(0, (5, 8), None);
    {
        let c = g.cities.get_mut(&capital).unwrap();
        c.is_capital = true;
        c.original_owner = 1;
        c.loyalty = 100.0;
    }
    let town = g.found_city_for(1, (8, 8), None);
    g.cities.get_mut(&town).unwrap().is_capital = false;
    g.found_city_for(2, (18, 8), None);
    let pressure = g.found_city_for(2, (21, 8), None);
    g.cities.get_mut(&pressure).unwrap().pop = 30;
    g.found_city_for(3, (25, 16), None);
    g.record_contact(0, 3);
    g.players[0].friends_until.insert(3, 1000);
    g.players[3].friends_until.insert(0, 1000);
    Arc::make_mut(&mut g.observed_city_loyalty_per_turn).insert(capital, 10.0);
    for other in [1, 2] {
        g.record_contact(0, other);
    }
    for _ in 0..4 {
        g.spawn_test_unit("modern_armor", 0, (5, 8));
    }
    for _ in 0..2 {
        g.spawn_test_unit("modern_armor", 2, (18, 8));
    }
    g.at_war.insert((0, 1));
    g.turn = 250;
    g.current = 0;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(town),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    ai.retarget(VictoryTarget::Domination);
    ai.enable_one_war_at_a_time();
    ai.one_war_observe(&g, 0);
    (g, ai, plan, capital)
}

#[test]
fn a_stable_captured_capital_closes_the_front_through_a_peace_offer() {
    let (mut g, mut ai, plan, _) = captured_front();
    assert!(
        ai.one_war_peace(&g, 0, 1).is_some(),
        "the front has delivered its stable original capital"
    );
    ai.advanced_diplomacy(&mut g, 0, &plan);
    let deal = g
        .pending_deals
        .iter()
        .find(|d| d.from == 0 && d.to == 1 && d.peace)
        .expect("offer white peace")
        .id;
    assert!(
        g.is_at_war(0, 1),
        "an offer does not unilaterally end the war"
    );
    assert!(
        ai.one_war_holds_declaration(&g, 0, 2),
        "wait for peace before opening another front"
    );
    g.current = 1;
    g.apply(1, &Action::AcceptDeal { deal }).unwrap();
    assert!(!g.is_at_war(0, 1));
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.assess(&g, 0).target_player, Some(2));
}

#[test]
fn the_next_capital_owner_precedes_mopping_up_the_finished_rival() {
    let (mut g, mut ai, _, _) = captured_front();
    g.apply(0, &Action::MakePeace { player: 1 }).unwrap();
    ai.one_war_observe(&g, 0);
    let next = g
        .cities
        .values()
        .find(|c| c.owner == 2 && c.is_capital)
        .unwrap()
        .id;
    assert!(
        AdvancedAi::should_defer_city_capture(&g, 0, next),
        "the next campaign may need frontier cities before the capital"
    );
    assert_eq!(
        ai.assess(&g, 0).target_player,
        Some(2),
        "prepare against an owner of a needed capital, not the completed rival's last town"
    );
}

#[test]
fn an_unstable_capture_or_an_explicit_order_keeps_the_front() {
    let (g, ai, _, capital) = captured_front();
    let mut unstable = g.clone();
    unstable.cities.get_mut(&capital).unwrap().loyalty = 40.0;
    assert!(ai.one_war_peace(&unstable, 0, 1).is_none());
    let mut bleeding = g.clone();
    Arc::make_mut(&mut bleeding.observed_city_loyalty_per_turn).insert(capital, -1.0);
    assert!(ai.one_war_peace(&bleeding, 0, 1).is_none());
    let mut forced = ai.clone();
    forced.forced_target_player = Some(1);
    assert!(forced.one_war_peace(&g, 0, 1).is_none());
    let mut adaptive = AdvancedAi::new();
    adaptive.enable_one_war_at_a_time();
    adaptive.one_war_observe(&g, 0);
    assert!(adaptive.one_war_peace(&g, 0, 1).is_none());
}

#[test]
fn a_front_holding_another_required_capital_is_not_finished() {
    let (mut g, ai, _, _) = captured_front();
    let original = g
        .cities
        .values()
        .find(|c| c.is_capital && c.original_owner == 3)
        .unwrap()
        .id;
    g.cities.get_mut(&original).unwrap().is_capital = false;
    let held = g.found_city_for(1, (11, 8), None);
    g.cities.get_mut(&held).unwrap().is_capital = true;
    g.cities.get_mut(&held).unwrap().original_owner = 3;
    assert!(
        ai.one_war_peace(&g, 0, 1).is_none(),
        "this front still holds another capital we need"
    );
}

#[test]
fn an_urgent_victory_threat_is_not_left_after_losing_its_capital() {
    let (mut g, ai, _, _) = captured_front();
    g.players[1].dvp = 19;
    assert!(ai.urgent_victory_threat(&g, 1));
    assert!(ai.one_war_peace(&g, 0, 1).is_none());
}
