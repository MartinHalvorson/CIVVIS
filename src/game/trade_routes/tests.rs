use super::*;

fn distant_route() -> (Game, u32, u32) {
    let mut game = Game::new_full(2, 64, 16, 3, 250, 0, false);
    let sites = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| !game.rules.is_water(tile) && game.rules.is_passable(tile))
        .map(|(pos, _)| *pos)
        .collect::<Vec<_>>();
    let origin = sites[0];
    let destination = sites
        .into_iter()
        .find(|pos| game.wdist(origin, *pos) > 15)
        .unwrap();
    let origin = game.place_city(0, origin, None);
    let destination = game.place_city(1, destination, None);
    game.record_contact(0, 1);
    game.players[0].civics.insert(crate::name!("foreign_trade"));
    (game, origin, destination)
}

#[test]
fn a_current_native_offer_allows_a_distant_trader_to_start() {
    let (mut game, origin, destination) = distant_route();
    let trader = game.spawn_unit("trader", 0, game.cities[&origin].pos);
    assert!(game.do_trade_route(0, trader, destination).is_err());
    Arc::make_mut(&mut game.observed_route_options)
        .insert((origin, destination), crate::rules::Yields::default());
    game.do_trade_route(0, trader, destination).unwrap();
    assert_eq!(game.routes.len(), 1);
    assert_eq!(game.routes[0].dest, destination);
    assert!(!game.units.contains_key(&trader));
    assert!(!game.can_establish_trade_route(0, origin, destination));
}

#[test]
fn native_range_evidence_is_pair_specific_and_expires() {
    let (mut game, origin, destination) = distant_route();
    Arc::make_mut(&mut game.observed_route_options)
        .insert((destination, origin), crate::rules::Yields::default());
    assert!(!game.can_establish_trade_route(0, origin, destination));
    Arc::make_mut(&mut game.observed_route_options)
        .insert((origin, destination), crate::rules::Yields::default());
    assert!(game.can_establish_trade_route(0, origin, destination));
    Arc::make_mut(&mut game.observed_route_options).clear();
    assert!(!game.can_establish_trade_route(0, origin, destination));
}

#[test]
fn native_range_evidence_preserves_refusal_war_and_capacity_checks() {
    let (mut game, origin, destination) = distant_route();
    Arc::make_mut(&mut game.observed_route_options)
        .insert((origin, destination), crate::rules::Yields::default());
    let pair = (game.cities[&origin].pos, game.cities[&destination].pos);
    Arc::make_mut(&mut game.blocked_trade_routes).insert(pair);
    assert!(!game.can_establish_trade_route(0, origin, destination));
    Arc::make_mut(&mut game.blocked_trade_routes).clear();
    let trader = game.spawn_unit("trader", 0, pair.0);
    Arc::make_mut(&mut game.observed_trade_capacity).insert(0, 0);
    assert!(game.do_trade_route(0, trader, destination).is_err());
    Arc::make_mut(&mut game.observed_trade_capacity).insert(0, 1);
    assert!(game.can_establish_trade_route(0, origin, destination));
    game.do_declare_war(0, 1).unwrap();
    assert!(!game.can_establish_trade_route(0, origin, destination));
}
