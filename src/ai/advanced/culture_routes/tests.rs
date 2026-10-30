use super::*;
use crate::ai::advanced::VictoryTarget;
use crate::game::Action;
use std::sync::Arc;

/// Three majors with a city each; seat 0 already runs a route to rival 1.
fn three_cities(tourism: f64) -> (Game, u32, u32, u32) {
    let mut game = Game::new_full(3, 18, 10, 79_001, 200, 0, false);
    for pid in 0..3 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.current = pid;
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    game.current = 0;
    game.record_contact(0, 1);
    game.record_contact(0, 2);
    let origin = game.player_city_ids(0)[0];
    let connected = game.player_city_ids(1)[0];
    let unconnected = game.player_city_ids(2)[0];
    game.routes.push(crate::game::TradeRoute {
        origin,
        dest: connected,
        owner: 0,
        ends: 30,
    });
    Arc::make_mut(&mut game.observed_tourism_per_turn).insert(0, tourism);
    (game, origin, connected, unconnected)
}

fn culture_seat(enabled: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    if enabled {
        ai.enable_culture_routes_reach_every_rival();
    }
    ai
}

#[test]
fn a_trader_goes_to_the_rival_without_our_route() {
    let (game, origin, _, unconnected) = three_cities(60.0);
    assert!(game.can_establish_trade_route(0, origin, unconnected));

    let claim = culture_seat(true).culture_route_claim(&game, 0, origin, GrandStrategy::Expansion);

    assert_eq!(claim.map(|(_, city)| city), Some(unconnected));
}

#[test]
fn the_rival_holding_more_domestic_tourists_comes_first() {
    let (mut game, origin, connected, unconnected) = three_cities(60.0);
    game.routes.clear();
    game.players[1].culture_lifetime = 9_000.0;
    game.players[2].culture_lifetime = 1_000.0;
    assert!(game.can_establish_trade_route(0, origin, connected));
    assert!(game.can_establish_trade_route(0, origin, unconnected));

    let claim = culture_seat(true).culture_route_claim(&game, 0, origin, GrandStrategy::Expansion);

    assert_eq!(claim.map(|(_, city)| city), Some(connected));
}

#[test]
fn off_or_below_the_floor_or_off_the_culture_lane_nothing_is_claimed() {
    let (game, origin, _, _) = three_cities(60.0);
    assert_eq!(
        culture_seat(false).culture_route_claim(&game, 0, origin, GrandStrategy::Expansion),
        None
    );
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_culture_routes_reach_every_rival();
    assert_eq!(
        science.culture_route_claim(&game, 0, origin, GrandStrategy::Expansion),
        None
    );

    let (low, origin, _, _) = three_cities(CULTURE_ROUTE_TOURISM_FLOOR - 1.0);
    assert_eq!(
        culture_seat(true).culture_route_claim(&low, 0, origin, GrandStrategy::Expansion),
        None
    );
}

#[test]
fn every_rival_covered_leaves_the_trader_to_its_usual_route() {
    let (mut game, origin, _, unconnected) = three_cities(60.0);
    game.routes.push(crate::game::TradeRoute {
        origin,
        dest: unconnected,
        owner: 0,
        ends: 30,
    });

    assert_eq!(
        culture_seat(true).culture_route_claim(&game, 0, origin, GrandStrategy::Expansion),
        None
    );
}
