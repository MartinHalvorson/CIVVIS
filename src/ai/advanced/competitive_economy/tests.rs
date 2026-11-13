use super::*;
use crate::ai::advanced::{genes, GrandStrategy};
use std::sync::Arc;

fn board() -> (Game, u32, u32) {
    let mut g = Game::new(2, 24, 16, 910_3447, 250, 0);
    for pid in 0..2 {
        let pos = g.units[&g.player_unit_ids(pid)[0]].pos;
        g.found_city_for(pid, pos, None);
    }
    g.turn = 60;
    g.current = 0;
    g.record_contact(0, 1);
    let origin = g.player_city_ids(0)[0];
    let dest = g.player_city_ids(1)[0];
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 5.0;
    let city = g.cities.get_mut(&origin).unwrap();
    city.pop = 3;
    city.loyalty = 100.0;
    city.districts.insert(crate::name!("campus"), city.pos);
    let housing = g.city_housing(&g.cities[&origin]);
    Arc::make_mut(&mut g.observed_city_housing_adjustments).insert(origin, 8.0 - housing);
    let food = g.city_yields(origin).food;
    Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        origin,
        Yields {
            food: 7.0 - food,
            ..Yields::default()
        },
    );
    g.cities.get_mut(&origin).unwrap().food = g.growth_cost(3) - 18.0;
    assert!(g.city_amenity_surplus(&g.cities[&origin]) >= 0);
    (g, origin, dest)
}

fn growth_ai() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_trade_growth_to_district();
    ai
}

#[test]
fn the_gene_is_an_opt_in_with_a_reversible_toggle() {
    let mut ai = AdvancedAi::new();
    let gene = genes::gene("trade-growth-to-district").unwrap();
    assert!(matches!(gene.kind, genes::Kind::OptIn));
    assert!(!ai.trade_growth_to_district);
    (gene.enable)(&mut ai);
    assert!(ai.trade_growth_to_district);
    (gene.disable)(&mut ai);
    assert!(!ai.trade_growth_to_district);
}

#[test]
fn route_food_can_beat_gold_when_it_releases_a_district_slot() {
    let (mut g, origin, dest) = board();
    let stock = AdvancedAi::new();
    let ai = growth_ai();
    let score = |g: &Game, ai: &AdvancedAi| {
        ai.trade_route_destination_value_from(
            g,
            0,
            Some(origin),
            &g.cities[&dest],
            GrandStrategy::Science,
        )
    };
    // Observed options are the host authority; even a foreign route can feed
    // this origin. This exercises the real valuation entry point.
    Arc::make_mut(&mut g.observed_route_options).insert(
        (origin, dest),
        Yields {
            gold: 12.0,
            ..Yields::default()
        },
    );
    let gold_score = score(&g, &stock);
    assert_eq!(score(&g, &ai), gold_score);
    Arc::make_mut(&mut g.observed_route_options).insert(
        (origin, dest),
        Yields {
            food: 3.0,
            production: 1.0,
            ..Yields::default()
        },
    );
    assert!(score(&g, &stock) < gold_score);
    assert!(score(&g, &ai) > gold_score);
    // Once the citizen arrives the same route has only its ordinary yields.
    g.cities.get_mut(&origin).unwrap().pop = 4;
    assert_eq!(score(&g, &ai), score(&g, &stock));
}

#[test]
fn growth_premium_requires_a_reachable_useful_slot_and_solvent_origin() {
    let (g, origin, _) = board();
    let ai = growth_ai();
    let food = Yields {
        food: 3.0,
        ..Yields::default()
    };
    let premium = |g: &Game| ai.trade_growth_to_district_premium(g, 0, Some(origin), food);
    assert!(premium(&g) > 0.0);
    assert_eq!(ai.trade_growth_to_district_premium(&g, 0, None, food), 0.0);
    let mut spare = g.clone();
    spare.cities.get_mut(&origin).unwrap().districts.clear();
    assert_eq!(premium(&spare), 0.0);
    let mut crowded = g.clone();
    Arc::make_mut(&mut crowded.observed_city_housing_adjustments).insert(origin, -10.0);
    assert_eq!(premium(&crowded), 0.0);
    let mut insolvent = g.clone();
    insolvent.players[0].gold = 0.0;
    insolvent.players[0].gold_per_turn = -5.0;
    assert_eq!(premium(&insolvent), 0.0);
    let mut late = g.clone();
    late.turn = late.max_turns - 1;
    assert_eq!(premium(&late), 0.0);
    let mut distant = g.clone();
    distant.cities.get_mut(&origin).unwrap().food = -1000.0;
    assert_eq!(premium(&distant), 0.0);
}
