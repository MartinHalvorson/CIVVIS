use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::GrandStrategy;

/// Two majors at peace: our capital, a second city of ours, and a rival
/// city, with the host quoting a 5-Gold foreign route and a 1-Food,
/// 3-Production domestic one (Surplus Logistics at the destination).
fn routes() -> (Game, AdvancedAi, u32, u32, u32) {
    let mut g = Game::new_full(2, 40, 24, 936_511, 650, 0, false);
    let origin = g.found_city_for(0, (10, 12), None);
    let home = g.found_city_for(0, (16, 12), None);
    let foreign = g.found_city_for(1, (30, 12), None);
    g.record_contact(0, 1);
    g.current = 0;
    g.turn = 30;
    let options = std::sync::Arc::make_mut(&mut g.observed_route_options);
    options.insert(
        (origin, foreign),
        Yields {
            gold: 5.0,
            ..Default::default()
        },
    );
    options.insert(
        (origin, home),
        Yields {
            food: 1.0,
            production: 3.0,
            ..Default::default()
        },
    );
    let mut ai = AdvancedAi::new();
    ai.enable_commercial_hub_and_traders();
    (g, ai, origin, home, foreign)
}

fn value(ai: &AdvancedAi, g: &Game, origin: u32, city: u32) -> f64 {
    ai.trade_route_destination_value_from(
        g,
        0,
        Some(origin),
        &g.cities[&city],
        GrandStrategy::Expansion,
    )
}

#[test]
fn early_route_feeds_production_is_opt_in() {
    opt_in_off_in_both_controllers("early-route-feeds-production", |ai| {
        ai.early_route_feeds_production
    });
}

/// Off, the foreign Gold route outbids the domestic Production one; under
/// the gene, inside the window, the domestic route wins; past it, the
/// pricing is as before.
#[test]
fn the_first_route_goes_home_for_production() {
    let (mut g, mut ai, origin, home, foreign) = routes();
    assert!(
        value(&ai, &g, origin, foreign) > value(&ai, &g, origin, home),
        "off: Gold wins"
    );
    ai.enable_early_route_feeds_production();
    assert!(
        value(&ai, &g, origin, home) > value(&ai, &g, origin, foreign),
        "on: the domestic route's Production wins"
    );
    g.turn = g.standard_duration(EARLY_ROUTE_STANDARD_TURNS) + 1;
    let mut off = ai.clone();
    off.disable_early_route_feeds_production();
    assert_eq!(
        value(&ai, &g, origin, foreign),
        value(&off, &g, origin, foreign),
        "past the window: priced as before"
    );
}
