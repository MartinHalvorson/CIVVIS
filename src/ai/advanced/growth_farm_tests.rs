//! `growth-prices-the-farm`: a small city that is Housing-bound or not
//! growing prices the Farm's Housing and Food; a city with room keeps its
//! Mine.

use super::*;

/// A founded capital with Mining and Irrigation, one owned grassland hill a
/// Mine can work and one owned flat grassland a Farm can.
fn capital_with_a_hill_and_a_flat() -> (Game, u32, Pos, Pos) {
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
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = hills;
        tile.resource = None;
        tile.improvement = None;
        tile.pillaged = false;
    }
    assert!(
        game.valid_improvements(0, hill)
            .contains(&crate::name!("mine")),
        "fixture: a Mine is legal on the hill"
    );
    assert!(
        game.valid_improvements(0, flat)
            .contains(&crate::name!("farm")),
        "fixture: a Farm is legal on the flat"
    );
    (game, capital, hill, flat)
}

/// Population `headroom` citizens below the capital's Housing.
fn set_headroom(game: &mut Game, capital: u32, headroom: f64) {
    let housing = game.city_housing(&game.cities[&capital]);
    let pop = ((housing - headroom).floor() as i32).max(1);
    game.cities.get_mut(&capital).unwrap().pop = pop;
}

/// The Builder's value of a Farm on the flat less a Mine on the hill.
fn farm_over_mine(game: &Game, ai: &AdvancedAi, hill: Pos, flat: Pos) -> f64 {
    ai.improvement_value_for(game, 0, flat, "farm", GrandStrategy::Conquest)
        - ai.improvement_value_for(game, 0, hill, "mine", GrandStrategy::Conquest)
}

#[test]
fn a_housing_bound_city_farms_the_flat_before_mining_the_hill() {
    let (mut game, capital, hill, flat) = capital_with_a_hill_and_a_flat();
    set_headroom(&mut game, capital, 0.0);
    assert!(
        game.city_housing_headroom(&game.cities[&capital]) <= 1.0,
        "fixture: the city is Housing-bound"
    );
    let stock = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(
        farm_over_mine(&game, &stock, hill, flat) < 0.0,
        "stock: the Mine outbids the Farm under Conquest"
    );
    let mut gene = AdvancedAi::targeting(VictoryTarget::Domination);
    gene.enable_growth_prices_the_farm();
    assert!(
        farm_over_mine(&game, &gene, hill, flat) > 0.0,
        "gene: the Farm's Housing wins the Housing-bound city"
    );
    assert_eq!(
        gene.growth_farm_premium(&game, 0, hill, "mine"),
        0.0,
        "a Mine has no Housing or Food to price"
    );
}

#[test]
fn a_city_with_room_and_food_keeps_its_mine() {
    let (mut game, capital, hill, flat) = capital_with_a_hill_and_a_flat();
    game.cities.get_mut(&capital).unwrap().pop = 1;
    let headroom = game.city_housing_headroom(&game.cities[&capital]);
    let surplus = game.city_yields(capital).food - 2.0;
    assert!(
        headroom > 1.0 && surplus > 1.0,
        "fixture: room ({headroom}) and Food ({surplus})"
    );
    let mut gene = AdvancedAi::targeting(VictoryTarget::Domination);
    gene.enable_growth_prices_the_farm();
    assert_eq!(
        gene.growth_farm_premium(&game, 0, flat, "farm"),
        0.0,
        "a city with room and Food pays no premium"
    );
    let stock = AdvancedAi::targeting(VictoryTarget::Domination);
    assert_eq!(
        farm_over_mine(&game, &gene, hill, flat),
        farm_over_mine(&game, &stock, hill, flat),
        "the gene leaves a growing city's values alone"
    );
}

#[test]
fn the_premium_prices_housing_and_food_by_the_constants() {
    let (mut game, capital, _hill, work) = capital_with_a_hill_and_a_flat();
    set_headroom(&mut game, capital, 0.0);
    let mut gene = AdvancedAi::targeting(VictoryTarget::Domination);
    gene.enable_growth_prices_the_farm();
    let farm = &game.rules.improvements["farm"];
    let surplus = game.city_yields(capital).food - 2.0 * game.cities[&capital].pop as f64;
    let mut expected = farm.housing * growth_farm::GROWTH_HOUSING_VALUE;
    if surplus <= 1.0 {
        expected += farm.yields.food * growth_farm::GROWTH_FOOD_VALUE;
    }
    let shortfall = gene.city_production_foundation_shortfall(&game, 0, capital);
    expected *= 1.0 + shortfall * PRODUCTION_FOUNDATION_IMPROVEMENT_MULTIPLIER;
    assert!(
        (gene.growth_farm_premium(&game, 0, work, "farm") - expected).abs() < 1e-9,
        "premium = (Housing × {} + Food × {} when stalled) × the foundation multiplier",
        growth_farm::GROWTH_HOUSING_VALUE,
        growth_farm::GROWTH_FOOD_VALUE
    );
    assert_eq!(
        gene.growth_farm_premium(&game, 1, work, "farm"),
        0.0,
        "no premium on a tile another player's city owns"
    );
    game.cities.get_mut(&capital).unwrap().pop = growth_farm::GROWTH_CITY_POP_MAX;
    assert_eq!(
        gene.growth_farm_premium(&game, 0, work, "farm"),
        0.0,
        "a large city pays no premium"
    );
}

#[test]
fn growth_prices_the_farm_is_off_by_default_and_reversible() {
    let (mut game, capital, _hill, work) = capital_with_a_hill_and_a_flat();
    set_headroom(&mut game, capital, 0.0);
    assert!(!AdvancedAi::new().growth_prices_the_farm);
    assert!(!AdvancedAi::legacy().growth_prices_the_farm);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert_eq!(ai.growth_farm_premium(&game, 0, work, "farm"), 0.0);
    ai.enable_growth_prices_the_farm();
    assert!(ai.growth_farm_premium(&game, 0, work, "farm") > 0.0);
    ai.disable_growth_prices_the_farm();
    assert_eq!(ai.growth_farm_premium(&game, 0, work, "farm"), 0.0);
}

#[test]
fn a_weak_housing_bound_city_farms_even_against_the_foundation_bonus() {
    let (mut game, capital, hill, flat) = capital_with_a_hill_and_a_flat();
    set_headroom(&mut game, capital, 0.0);
    let builder_value = |ai: &AdvancedAi, pos: Pos, improvement: &str| {
        let shortfall = ai.city_production_foundation_shortfall(&game, 0, capital);
        ai.production_foundation_improvement_value(
            &game,
            0,
            pos,
            improvement,
            GrandStrategy::Conquest,
            shortfall,
        )
    };
    let stock = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(
        stock.city_production_foundation_shortfall(&game, 0, capital) > 0.0,
        "fixture: the young capital is below its production foundation"
    );
    assert!(
        builder_value(&stock, flat, "farm") < builder_value(&stock, hill, "mine"),
        "stock: the foundation bonus sends the Builder to the Mine"
    );
    let mut gene = AdvancedAi::targeting(VictoryTarget::Domination);
    gene.enable_growth_prices_the_farm();
    assert!(
        builder_value(&gene, flat, "farm") > builder_value(&gene, hill, "mine"),
        "gene: the Housing-bound city's Farm outbids the Mine's foundation bonus"
    );
}

/// A Domination seat with version 2 on, as the live bundle seats it beside
/// `builders-improve-the-worked-for-production`.
fn version_two() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_builders_improve_the_worked_for_production();
    ai.enable_growth_prices_the_farm_2();
    ai
}

/// The same seat with version 1 instead, the bundle before this gene.
fn version_one() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_builders_improve_the_worked_for_production();
    ai.enable_growth_prices_the_farm();
    ai
}

#[test]
fn version_two_prices_a_growing_citys_food_where_version_one_pays_nothing() {
    let (mut game, capital, hill, flat) = capital_with_a_hill_and_a_flat();
    set_headroom(&mut game, capital, 3.0);
    let headroom = game.city_housing_headroom(&game.cities[&capital]);
    assert!(headroom >= 2.0, "fixture: the city has room ({headroom})");
    let v1 = version_one();
    let v2 = version_two();
    assert_eq!(
        v1.growing_city_food_premium(&game, 0, flat, "farm", GrandStrategy::Conquest),
        0.0,
        "version 1 alone prices nothing in a city with room"
    );
    assert!(
        v2.growing_city_food_premium(&game, 0, flat, "farm", GrandStrategy::Conquest) > 0.0,
        "version 2 prices the Farm's Food in a growing city"
    );
    assert!(
        farm_over_mine(&game, &v2, hill, flat) > farm_over_mine(&game, &v1, hill, flat),
        "the Farm closes on the Mine"
    );
    assert_eq!(
        v2.growing_city_food_premium(&game, 0, hill, "mine", GrandStrategy::Conquest),
        0.0,
        "a Mine has no Food to price"
    );
}

#[test]
fn the_food_premium_is_the_grown_citizens_priced_as_production() {
    let (mut game, capital, _hill, flat) = capital_with_a_hill_and_a_flat();
    set_headroom(&mut game, capital, 3.0);
    let ai = version_two();
    let city = &game.cities[&capital];
    let farm = &game.rules.improvements["farm"];
    let food = farm.yields.food + AdvancedAi::improvement_tree_yields(&game, 0, "farm").food;
    let citizens = AdvancedAi::growth_citizens(&game, city.pop, game.city_housing_headroom(city));
    let horizon = game.standard_duration(growth_farm::GROWTH_HORIZON_STANDARD_TURNS) as f64;
    assert!(
        (citizens
            - (horizon / game.growth_cost(city.pop)).min(game.city_housing_headroom(city) - 1.0))
        .abs()
            < 1e-9,
        "citizens = horizon / growth cost, capped one below the Housing"
    );
    let strategy = GrandStrategy::Conquest;
    let production = ai.yield_value(
        Yields {
            production: food * growth_farm::CITIZEN_PRODUCTION_POINTS * citizens / 2.0,
            ..Yields::default()
        },
        strategy,
    );
    let foundation = 1.0
        + ai.city_production_foundation_shortfall(&game, 0, capital)
            * PRODUCTION_FOUNDATION_IMPROVEMENT_MULTIPLIER;
    let worked = if ai.city_works_the_tile(&game, 0, capital, flat) {
        worked_production::WORKED_PRODUCTION_PREMIUM
    } else {
        0.0
    };
    let paid = ai.yield_value(
        Yields {
            food: farm.yields.food,
            ..Yields::default()
        },
        strategy,
    );
    let expected = (production * foundation * (1.0 + worked) - paid).max(0.0);
    assert!(
        (ai.growing_city_food_premium(&game, 0, flat, "farm", strategy) - expected).abs() < 1e-9,
        "premium = Production of the grown citizens × foundation × worked − the Food already paid"
    );
}

#[test]
fn the_food_premium_stops_without_room_in_a_large_city_and_on_foreign_land() {
    let (mut game, capital, _hill, flat) = capital_with_a_hill_and_a_flat();
    let ai = version_two();
    set_headroom(&mut game, capital, 1.0);
    assert_eq!(
        ai.growing_city_food_premium(&game, 0, flat, "farm", GrandStrategy::Conquest),
        0.0,
        "within one of the cap the Food is mostly wasted; version 1's Housing price answers"
    );
    set_headroom(&mut game, capital, 3.0);
    assert_eq!(
        ai.growing_city_food_premium(&game, 1, flat, "farm", GrandStrategy::Conquest),
        0.0,
        "no premium on a tile another player's city owns"
    );
    game.cities.get_mut(&capital).unwrap().pop = growth_farm::GROWTH_CITY_POP_MAX;
    assert_eq!(
        ai.growing_city_food_premium(&game, 0, flat, "farm", GrandStrategy::Conquest),
        0.0,
        "a large city pays no premium"
    );
}

#[test]
fn version_two_prices_housing_before_the_cap() {
    let (mut game, capital, _hill, flat) = capital_with_a_hill_and_a_flat();
    let farm_housing = game.rules.improvements["farm"].housing;
    assert!(farm_housing > 0.0, "fixture: a Farm gives Housing");
    set_headroom(&mut game, capital, 2.0);
    let headroom = game.city_housing_headroom(&game.cities[&capital]);
    assert!(
        headroom > 1.0 && headroom < 4.0,
        "fixture: room 2-3 ({headroom})"
    );
    assert_eq!(
        version_one().growth_farm_premium(&game, 0, flat, "farm"),
        0.0,
        "version 1 waits for the cap"
    );
    assert!(
        version_two().growth_farm_premium(&game, 0, flat, "farm") > 0.0,
        "version 2 prices the Farm's Housing as the city approaches the cap"
    );
    set_headroom(&mut game, capital, 0.0);
    assert_eq!(
        version_one().growth_farm_premium(&game, 0, flat, "farm"),
        version_two().growth_farm_premium(&game, 0, flat, "farm"),
        "at the cap both versions price Housing alike"
    );
}

#[test]
fn growth_prices_the_farm_2_is_off_by_default_and_reversible() {
    let (mut game, capital, _hill, flat) = capital_with_a_hill_and_a_flat();
    set_headroom(&mut game, capital, 3.0);
    assert!(!AdvancedAi::new().growth_prices_the_farm_2);
    assert!(!AdvancedAi::legacy().growth_prices_the_farm_2);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let strategy = GrandStrategy::Conquest;
    assert_eq!(
        ai.growing_city_food_premium(&game, 0, flat, "farm", strategy),
        0.0
    );
    ai.enable_growth_prices_the_farm_2();
    assert!(ai.growing_city_food_premium(&game, 0, flat, "farm", strategy) > 0.0);
    ai.disable_growth_prices_the_farm_2();
    assert_eq!(
        ai.growing_city_food_premium(&game, 0, flat, "farm", strategy),
        0.0
    );
    assert_eq!(ai.growth_farm_premium(&game, 0, flat, "farm"), 0.0);
}
