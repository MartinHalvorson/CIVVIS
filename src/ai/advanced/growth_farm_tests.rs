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
    assert!(
        (gene.growth_farm_premium(&game, 0, work, "farm") - expected).abs() < 1e-9,
        "premium = Housing × {} (+ Food × {} when stalled)",
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
