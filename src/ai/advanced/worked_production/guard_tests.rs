//! Housing can settle the Builder guard without deriving food; prices stay exact.

use super::*;
use std::sync::Arc;

fn world(surplus: f64, headroom: f64) -> (Game, u32, Pos) {
    let mut game = Game::new(2, 24, 16, 936_015, 200, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|id| game.units[id].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let home = game.cities[&city].pos;
    let hill = *game.cities[&city]
        .owned_tiles
        .iter()
        .find(|pos| **pos != home)
        .unwrap();
    let tile = game.map.tiles.get_mut(&hill).unwrap();
    tile.terrain = crate::name!("grassland");
    tile.hills = true;
    tile.feature = None;
    tile.resource = None;
    tile.improvement = None;
    tile.pillaged = false;
    game.players[0].techs.insert(crate::name!("mining"));
    Arc::make_mut(&mut game.observed_city_worked_tiles).insert(city, vec![hill]);
    let pop = game.cities[&city].pop as f64;
    let food = game.city_yields(city).food;
    let housing = game.city_housing(&game.cities[&city]);
    Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        city,
        Yields {
            food: 2.0 * pop + surplus - food,
            ..Yields::default()
        },
    );
    Arc::make_mut(&mut game.observed_city_housing_adjustments)
        .insert(city, pop + headroom - housing);
    game.keep_rival_yields(true);
    game.keep_own_yields(true);
    assert_eq!(game.current, 0);
    assert_eq!(game.rival_yields_reused(), (0, 0));
    (game, city, hill)
}

fn site_player() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_site_deployment();
    assert!(ai.builders_improve_the_worked_for_production);
    assert!(ai.victory_target.is_none());
    ai
}

// The original calculation for this fixture's worked Mine, including the
// unconditional food derivation and unchanged final floating-point order.
fn previous_premium(ai: &AdvancedAi, game: &Game, city: u32, strategy: GrandStrategy) -> f64 {
    let production = game.rules.improvements["mine"].yields.production
        + AdvancedAi::improvement_tree_yields(game, 0, "mine").production;
    let surplus = game.city_yields(city).food - 2.0 * game.cities[&city].pop as f64;
    if surplus <= WORKED_FOOD_SURPLUS_FLOOR && game.city_housing_headroom(&game.cities[&city]) > 1.0
    {
        return 0.0;
    }
    ai.yield_value(
        Yields {
            production,
            ..Yields::default()
        },
        strategy,
    ) * WORKED_PRODUCTION_PREMIUM
        * (1.0
            + ai.city_production_foundation_shortfall(game, 0, city)
                * PRODUCTION_FOUNDATION_IMPROVEMENT_MULTIPLIER)
}

#[test]
fn housing_bound_site_players_skip_the_food_derivation() {
    for headroom in [-10.0, 0.0, 1.0, f64::NEG_INFINITY, f64::NAN] {
        let (game, city, hill) = world(0.0, headroom);
        let (reference, reference_city, _) = world(0.0, headroom);
        let ai = site_player();
        let expected = previous_premium(&ai, &reference, reference_city, GrandStrategy::Conquest);
        let actual = ai.worked_production_premium(&game, 0, hill, "mine", GrandStrategy::Conquest);
        assert!(actual > 0.0);
        assert_eq!(actual.to_bits(), expected.to_bits());
        assert_eq!(reference.rival_yields_reused().1, 1);
        assert_eq!(game.rival_yields_reused(), (0, 0));
        assert_eq!(game.cities[&city].owner, 0);
    }
}

#[test]
fn room_to_grow_still_derives_food_and_refuses_a_starving_premium() {
    for surplus in [-2.0, 0.0, 1.0, 1.000_001, 10.0, f64::INFINITY, f64::NAN] {
        let (game, city, hill) = world(surplus, 3.0);
        let ai = site_player();
        let expected = previous_premium(&ai, &game.clone(), city, GrandStrategy::Expansion);
        let actual = ai.worked_production_premium(&game, 0, hill, "mine", GrandStrategy::Expansion);
        assert_eq!(actual.to_bits(), expected.to_bits(), "surplus {surplus}");
        assert_eq!(game.rival_yields_reused().1, 1);
        if surplus <= WORKED_FOOD_SURPLUS_FLOOR {
            assert_eq!(actual.to_bits(), 0.0_f64.to_bits());
        } else {
            assert!(actual > 0.0);
        }
    }
}

#[test]
fn targeted_foundation_pricing_keeps_its_own_yield_derivation() {
    let (game, city, hill) = world(0.0, 1.0);
    let mut ai = site_player();
    ai.retarget(VictoryTarget::Domination);
    let expected = previous_premium(&ai, &game.clone(), city, GrandStrategy::Conquest);
    let actual = ai.worked_production_premium(&game, 0, hill, "mine", GrandStrategy::Conquest);
    assert_eq!(actual.to_bits(), expected.to_bits());
    assert_eq!(game.rival_yields_reused().1, 1);
}

#[test]
fn boundary_and_nonfinite_inputs_keep_every_original_price_bit() {
    for headroom in [0.999_999, 1.0, 1.000_001, f64::INFINITY, f64::NAN] {
        for surplus in [0.999_999, 1.0, 1.000_001, f64::NEG_INFINITY, f64::NAN] {
            for strategy in [
                GrandStrategy::Expansion,
                GrandStrategy::Conquest,
                GrandStrategy::Science,
            ] {
                let (game, city, hill) = world(surplus, headroom);
                let ai = site_player();
                let expected = previous_premium(&ai, &game.clone(), city, strategy);
                let actual = ai.worked_production_premium(&game, 0, hill, "mine", strategy);
                assert_eq!(
                    actual.to_bits(),
                    expected.to_bits(),
                    "headroom {headroom}, food surplus {surplus}, strategy {strategy:?}"
                );
            }
        }
    }
}

#[test]
fn cards_classified_inert_by_the_new_guard_cannot_change_its_price() {
    for (surplus, headroom) in [(0.0, 1.0), (0.0, 3.0), (3.0, 3.0)] {
        let (game, city, hill) = world(surplus, headroom);
        let ai = site_player();
        let trace = game.trace_policy_reads(0);
        let baseline =
            ai.worked_production_premium(&game, 0, hill, "mine", GrandStrategy::Conquest);
        let reads = trace.finish();
        drop(trace);
        assert!(!reads.is_opaque());
        let cards: Vec<_> = game.rules.policies.keys().copied().collect();
        let mut inert = 0;
        for card in cards {
            let mut changed = game.clone();
            changed.players[0].policies.insert(card);
            changed.board_written();
            let actual =
                ai.worked_production_premium(&changed, 0, hill, "mine", GrandStrategy::Conquest);
            let expected = previous_premium(&ai, &changed, city, GrandStrategy::Conquest);
            assert_eq!(actual.to_bits(), expected.to_bits(), "card {card}");
            if reads.card_is_inert(&game.rules, card) {
                inert += 1;
                assert_eq!(actual.to_bits(), baseline.to_bits(), "inert card {card}");
            }
        }
        assert!(
            inert > 0,
            "the test must exercise actual inert-card decisions"
        );
    }
}
