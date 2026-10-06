use super::*;
use crate::ai::commercial_hub::tests::emperor_empire;
use std::sync::Arc;

#[test]
fn commercial_hub_and_traders_is_a_native_opt_in_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "commercial-hub-and-traders",
        |ai| {
            assert_eq!(
                ai.commercial_hub_and_traders, ai.base.commercial_hub_and_traders,
                "the controller and its delegated governor agree"
            );
            ai.commercial_hub_and_traders
        },
    );
}

/// See `BasicAi::commercial_hub_and_traders`: through the delegated governor
/// the Emperor-shaped empire's most productive city starts a Commercial Hub
/// under the gene and the army without it, and the plan's threatened city
/// keeps the stock queue. The plan's threatened city is lent for the call
/// only.
#[test]
fn the_delegated_governor_opens_the_hub_but_not_in_the_threatened_city() {
    let (mut game, ranked) = emperor_empire("COMMERCIALDELEGATED", 92_021);
    // No route slot: the city's first economy build is the hub itself.
    Arc::make_mut(&mut game.observed_trade_capacity).insert(0, 0);
    let top = ranked[0];
    let run = |gene: bool, threatened: Option<u32>| {
        let mut game = game.clone();
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.base.book_pos = 4;
        ai.base.w.city_target = 6.0;
        if gene {
            ai.enable_commercial_hub_and_traders();
        }
        let plan = StrategicPlan {
            strategy: GrandStrategy::Expansion,
            target_player: None,
            target_city: None,
            threatened_city: threatened,
            desired_cities: 6,
            assessed_turn: game.turn,
            rush: false,
        };
        ai.delegated_cities(&mut game, 0, &plan);
        assert_eq!(
            ai.base.plan_threatened_city, None,
            "the lend ends with the call"
        );
        game.cities[&top].queue.first().cloned()
    };
    let is_hub = |item: &Option<Item>| {
        matches!(item, Some(Item::District { district, .. })
            if game.district_family(*district) == "commercial_hub")
    };
    let stock = run(false, None);
    assert!(stock.is_some(), "the stock governor builds something");
    assert!(
        !is_hub(&stock),
        "the stock governor builds no hub: {stock:?}"
    );
    let gene = run(true, None);
    assert!(is_hub(&gene), "the gene opens the hub: {gene:?}");
    assert_eq!(
        run(true, Some(top)),
        run(false, Some(top)),
        "threatened: the stock queue"
    );
}

/// Our capital (the route origin) and a second city of ours, a rival's city
/// and a third major's city, all within route range; two route slots; the
/// host prices the domestic route at 2 Food and 3 Production, the rival's
/// at 6 Gold and the third major's at 1 Gold.
fn route_board(seed: u64) -> (Game, u32, u32, u32, u32) {
    let mut game = Game::new_full(3, 30, 18, seed, 250, 0, false);
    game.current = 0;
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let origin = game.player_city_ids(0)[0];
    let home = game.cities[&origin].pos;
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    let site = |game: &Game| {
        game.map
            .tiles
            .values()
            .filter(|tile| {
                game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && tile.owner_city.is_none()
                    && (4..=8).contains(&game.wdist(home, tile.pos))
                    && game
                        .cities
                        .values()
                        .all(|city| game.wdist(city.pos, tile.pos) >= 4)
            })
            .map(|tile| tile.pos)
            .min()
            .expect("a city site within route range")
    };
    let domestic = game.found_city_for(0, site(&game), None);
    let rival = game.found_city_for(1, site(&game), None);
    let third = game.found_city_for(2, site(&game), None);
    Arc::make_mut(&mut game.observed_trade_capacity).insert(0, 2);
    let options = Arc::make_mut(&mut game.observed_route_options);
    options.insert(
        (origin, domestic),
        Yields {
            food: 2.0,
            production: 3.0,
            ..Default::default()
        },
    );
    options.insert(
        (origin, rival),
        Yields {
            gold: 6.0,
            ..Default::default()
        },
    );
    options.insert(
        (origin, third),
        Yields {
            gold: 1.0,
            ..Default::default()
        },
    );
    (game, origin, domestic, rival, third)
}

/// See `international_gold_route_premium`: at peace the stock chooser takes
/// the domestic Food and Production route and the gene the rival's Gold
/// route, and a Trader in the origin runs it. At war with any major, or
/// toward the plan's target, the stock pricing stands.
#[test]
fn at_peace_the_trader_runs_the_international_gold_route() {
    let (game, origin, domestic, rival, third) = route_board(92_031);
    for city in [domestic, rival, third] {
        assert!(
            game.can_establish_trade_route(0, origin, city),
            "fixture: a legal route to {city}"
        );
    }
    let strategy = GrandStrategy::Conquest;
    let best = |ai: &AdvancedAi, game: &Game| {
        ai.best_trade_route_destination(game, 0, origin, strategy)
            .map(|(_, city)| city)
    };
    let gene = || {
        let mut ai = AdvancedAi::new();
        ai.enable_commercial_hub_and_traders();
        ai
    };
    assert_eq!(
        best(&AdvancedAi::new(), &game),
        Some(domestic),
        "stock: the domestic Production route"
    );
    assert_eq!(best(&gene(), &game), Some(rival), "gene: the rival's Gold");

    let six_gold = Yields {
        gold: 6.0,
        ..Default::default()
    };
    assert_eq!(
        gene().international_gold_route_premium(&game, 0, 1, six_gold),
        INTERNATIONAL_ROUTE_GOLD_PREMIUM * 6.0
    );
    assert_eq!(
        gene().international_gold_route_premium(&game, 0, 0, six_gold),
        0.0,
        "our own city"
    );
    assert_eq!(
        AdvancedAi::new().international_gold_route_premium(&game, 0, 1, six_gold),
        0.0,
        "off"
    );

    let mut war = game.clone();
    war.at_war.insert((0, 2));
    assert_eq!(
        best(&gene(), &war),
        Some(domestic),
        "at war with a major: the stock pricing"
    );

    let mut targeting = gene();
    targeting.plan = Some(StrategicPlan {
        strategy,
        target_player: Some(1),
        target_city: Some(rival),
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: game.turn,
        rush: false,
    });
    assert_eq!(
        best(&targeting, &game),
        Some(domestic),
        "the plan's target: the stock pricing"
    );

    let run = |ai: &AdvancedAi| {
        let mut game = game.clone();
        let pos = game.cities[&origin].pos;
        let trader = game.spawn_test_unit("trader", 0, pos);
        assert!(ai.advanced_trader_step(&mut game, 0, trader, strategy));
        game.routes
            .iter()
            .find(|route| route.owner == 0 && route.origin == origin)
            .map(|route| route.dest)
    };
    assert_eq!(run(&AdvancedAi::new()), Some(domestic));
    assert_eq!(run(&gene()), Some(rival), "the Trader runs the Gold route");
}
