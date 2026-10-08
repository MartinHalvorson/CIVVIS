//! `builders-improve-the-worked-for-production`: a Mine under a citizen
//! outbids a Farm on an idle flat; a starving city still farms; off, every
//! value is the stock one.

use super::*;
use std::sync::Arc;

/// A founded capital with Mining and Irrigation, one owned hill a Mine can
/// work and one owned flat a Farm can, both of `terrain`.
fn capital_with_a_hill_and_a_flat(terrain: &str) -> (Game, u32, Pos, Pos) {
    let mut game = Game::new(2, 24, 16, 936015, 200, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|id| game.units[id].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    game.players[0]
        .techs
        .extend([crate::name!("mining"), crate::name!("irrigation")]);
    let mut owned = game.cities[&capital]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != home);
    let hill = owned.next().expect("the capital owns a workable plot");
    let flat = owned.next().expect("the capital owns a second plot");
    for (pos, hills) in [(hill, true), (flat, false)] {
        let tile = game.map.tiles.get_mut(&pos).expect("work plot");
        tile.terrain = Name::new(terrain);
        tile.feature = None;
        tile.hills = hills;
        tile.resource = None;
        tile.improvement = None;
        tile.pillaged = false;
    }
    assert!(
        game.valid_improvements(0, hill).contains(&crate::name!("mine")),
        "fixture: a Mine is legal on the hill"
    );
    assert!(
        game.valid_improvements(0, flat).contains(&crate::name!("farm")),
        "fixture: a Farm is legal on the flat"
    );
    (game, capital, hill, flat)
}

/// The capital works exactly `tiles`, as a live mirror reports it.
fn works(game: &mut Game, capital: u32, tiles: &[Pos]) {
    Arc::make_mut(&mut game.observed_city_worked_tiles).insert(capital, tiles.to_vec());
}

/// The Builder's score for `improvement` on `pos`: the value every routing
/// site maximises, foundation bonus included.
fn builder_value(
    game: &Game,
    ai: &AdvancedAi,
    capital: u32,
    pos: Pos,
    improvement: &str,
    strategy: GrandStrategy,
) -> f64 {
    let shortfall = ai.city_production_foundation_shortfall(game, 0, capital);
    ai.production_foundation_improvement_value(game, 0, pos, improvement, strategy, shortfall)
}

/// The live seat's armed lineage: `growth-prices-the-farm` on.
fn governor(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_growth_prices_the_farm();
    if gene {
        ai.enable_builders_improve_the_worked_for_production();
    }
    ai
}

#[test]
fn builders_improve_the_worked_for_production_is_off_by_default_and_reversible() {
    let (mut game, capital, hill, flat) = capital_with_a_hill_and_a_flat("grassland");
    works(&mut game, capital, &[hill]);
    assert!(!AdvancedAi::new().builders_improve_the_worked_for_production);
    assert!(!AdvancedAi::legacy().builders_improve_the_worked_for_production);
    let stock = governor(false);
    let strategy = GrandStrategy::Conquest;
    assert_eq!(stock.worked_production_premium(&game, 0, hill, "mine", strategy), 0.0);
    let mut ai = governor(false);
    ai.enable_builders_improve_the_worked_for_production();
    assert!(ai.worked_production_premium(&game, 0, hill, "mine", strategy) > 0.0);
    ai.disable_builders_improve_the_worked_for_production();
    for (pos, improvement) in [(hill, "mine"), (flat, "farm")] {
        assert_eq!(
            builder_value(&game, &ai, capital, pos, improvement, strategy),
            builder_value(&game, &stock, capital, pos, improvement, strategy),
            "off: the stock value of a {improvement}"
        );
        assert_eq!(
            ai.worthwhile_improvements(&game, 0, pos, strategy),
            stock.worthwhile_improvements(&game, 0, pos, strategy),
            "off: the stock ranking on the {improvement} tile"
        );
    }
}

/// The capital at its Housing works the hill and not the flat. Under the
/// armed `growth-prices-the-farm` the stock Builder farms the idle flat; under
/// the gene the Mine under the citizen comes first.
#[test]
fn a_worked_hill_beats_an_unworked_farm() {
    let (mut game, capital, hill, flat) = capital_with_a_hill_and_a_flat("grassland");
    let housing = game.city_housing(&game.cities[&capital]);
    game.cities.get_mut(&capital).unwrap().pop = (housing.floor() as i32).max(1);
    works(&mut game, capital, &[hill]);
    assert!(
        game.city_housing_headroom(&game.cities[&capital]) <= 1.0,
        "fixture: the capital is at its Housing"
    );
    let strategy = GrandStrategy::Conquest;
    let stock = governor(false);
    assert!(
        builder_value(&game, &stock, capital, flat, "farm", strategy)
            > builder_value(&game, &stock, capital, hill, "mine", strategy),
        "stock: the idle flat's Farm outbids the worked hill's Mine"
    );
    let gene = governor(true);
    assert!(
        builder_value(&game, &gene, capital, hill, "mine", strategy)
            > builder_value(&game, &gene, capital, flat, "farm", strategy),
        "gene: the Mine under the citizen comes first"
    );
    assert_eq!(
        gene.worked_production_premium(&game, 0, flat, "farm", strategy),
        0.0,
        "a Farm adds no Production"
    );
    // The same hill with no citizen on it earns nothing.
    works(&mut game, capital, &[flat]);
    assert_eq!(
        gene.worked_production_premium(&game, 0, hill, "mine", strategy),
        0.0,
        "an idle hill pays no premium"
    );
}

/// The same lineage without a named victory, so no production-foundation
/// bonus is in play and the stock pricing is the printed one.
fn untargeted_governor(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_growth_prices_the_farm();
    if gene {
        ai.enable_builders_improve_the_worked_for_production();
    }
    ai
}

/// A starving capital with Housing to grow into keeps the stock pricing, and
/// the stock pricing farms; a fed capital working the same kind of hill takes
/// the Mine.
#[test]
fn a_starving_city_still_farms() {
    let strategy = GrandStrategy::Expansion;
    let (mut game, capital, hill, flat) = capital_with_a_hill_and_a_flat("plains");
    works(&mut game, capital, &[hill, flat]);
    // Two citizens on two one-Food plains, with a Granary's Housing so the
    // city is short of Food, not of Housing.
    let city = game.cities.get_mut(&capital).unwrap();
    city.pop = 2;
    city.buildings.push(crate::name!("granary"));
    let surplus = game.city_yields(capital).food - 4.0;
    let headroom = game.city_housing_headroom(&game.cities[&capital]);
    assert!(
        surplus <= worked_production::WORKED_FOOD_SURPLUS_FLOOR && headroom > 1.0,
        "fixture: starving ({surplus}) with Housing to grow into ({headroom})"
    );
    let value = |game: &Game, ai: &AdvancedAi, pos: Pos, improvement: &str| {
        ai.production_foundation_improvement_value(game, 0, pos, improvement, strategy, 0.0)
    };
    let stock = untargeted_governor(false);
    let gene = untargeted_governor(true);
    assert_eq!(
        gene.worked_production_premium(&game, 0, hill, "mine", strategy),
        0.0,
        "a starving city pays no premium"
    );
    for (pos, improvement) in [(hill, "mine"), (flat, "farm")] {
        assert_eq!(
            value(&game, &gene, pos, improvement),
            value(&game, &stock, pos, improvement),
            "starving: the stock value of a {improvement}"
        );
    }
    assert!(
        value(&game, &gene, flat, "farm") > value(&game, &gene, hill, "mine"),
        "starving: the Farm still comes first"
    );
    // Fed: grassland, one citizen on the hill.
    let (mut fed, capital, hill, flat) = capital_with_a_hill_and_a_flat("grassland");
    works(&mut fed, capital, &[hill]);
    fed.cities.get_mut(&capital).unwrap().pop = 1;
    let surplus = fed.city_yields(capital).food - 2.0;
    assert!(
        surplus > worked_production::WORKED_FOOD_SURPLUS_FLOOR,
        "fixture: fed ({surplus})"
    );
    assert!(
        value(&fed, &gene, hill, "mine") > value(&fed, &gene, flat, "farm"),
        "fed: the worked hill's Mine comes first"
    );
}

/// See `WORKED_PRODUCTION_PREMIUM`: the lane's weight on the Mine's
/// Production (Apprenticeship's +1 included) times the census ratio, scaled
/// by the foundation shortfall.
#[test]
fn the_premium_prices_production_by_the_census_ratio() {
    let (mut game, capital, hill, _flat) = capital_with_a_hill_and_a_flat("grassland");
    game.cities.get_mut(&capital).unwrap().pop = 1;
    works(&mut game, capital, &[hill]);
    let strategy = GrandStrategy::Conquest;
    let gene = governor(true);
    let expected = |game: &Game| {
        let production = game.rules.improvements["mine"].yields.production
            + game.tree_effect(0, "mine_production");
        gene.yield_value(
            Yields {
                production,
                ..Yields::default()
            },
            strategy,
        ) * worked_production::WORKED_PRODUCTION_PREMIUM
            * (1.0
                + gene.city_production_foundation_shortfall(game, 0, capital)
                    * PRODUCTION_FOUNDATION_IMPROVEMENT_MULTIPLIER)
    };
    let premium = gene.worked_production_premium(&game, 0, hill, "mine", strategy);
    assert!(premium > 0.0, "fixture: the fed capital pays the premium");
    assert!((premium - expected(&game)).abs() < 1e-9);
    game.players[0].techs.insert(crate::name!("apprenticeship"));
    let upgraded = gene.worked_production_premium(&game, 0, hill, "mine", strategy);
    assert!(upgraded > premium, "Apprenticeship's +1 is priced");
    assert!((upgraded - expected(&game)).abs() < 1e-9);
    assert_eq!(
        gene.worked_production_premium(&game, 1, hill, "mine", strategy),
        0.0,
        "no premium on a tile another player's city owns"
    );
}
