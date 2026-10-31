//! `deck-prices-growth`: the policy counterfactual reads each city's growth
//! surplus only when `Weights::pol_growth` is set.
use super::*;
use crate::rules::Yields;
use std::sync::Arc;

/// One founded city of four citizens with plenty of Food, its housing held at
/// `housing`.
fn city_with_housing(housing: f64) -> (Game, u32) {
    let mut g = Game::new_full(2, 24, 16, 91_781, 250, 0, false);
    g.current = 0;
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = g.player_city_ids(0)[0];
    g.cities.get_mut(&cid).unwrap().pop = 4;
    Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        cid,
        Yields {
            food: 12.0,
            ..Default::default()
        },
    );
    let base = g.city_housing(&g.cities[&cid]);
    Arc::make_mut(&mut g.observed_city_housing_adjustments).insert(cid, housing - base);
    (g, cid)
}

#[test]
fn growth_is_read_only_when_priced() {
    // Housing one above population grows at half; three above, in full.
    let (capped, cid) = city_with_housing(5.0);
    let (roomy, _) = city_with_housing(7.0);
    assert_eq!(capped.city_housing(&capped.cities[&cid]), 5.0);
    assert!(
        capped.city_growth_surplus(
            0,
            cid,
            capped.city_yields(cid).food,
            5.0,
            capped.city_amenity_surplus(&capped.cities[&cid]),
        ) > 0.0,
        "fixture: the city grows"
    );

    let stock = Weights::default();
    assert_eq!(stock.pol_growth, 0.0);
    assert_eq!(
        empire_reading(&capped, 0, &stock, false, false),
        empire_reading(&roomy, 0, &stock, false, false),
        "stock: housing moves nothing the deck reads"
    );

    let priced = Weights {
        pol_growth: stock.pol_food,
        ..Weights::default()
    };
    assert!(
        empire_reading(&roomy, 0, &priced, false, false)
            > empire_reading(&capped, 0, &priced, false, false),
        "priced: the housing that lifts the throttle reads as growth"
    );
}
