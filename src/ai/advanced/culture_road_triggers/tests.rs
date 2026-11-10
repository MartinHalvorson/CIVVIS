use super::super::test_support::opt_in_off_in_both_controllers;
use super::*;
use crate::name;

#[test]
fn culture_road_builds_its_triggers_is_a_native_opt_in_off_in_both_controllers() {
    opt_in_off_in_both_controllers("culture-road-builds-its-triggers", |ai| {
        ai.culture_road_builds_its_triggers
    });
}

fn culture_ai(enabled: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    if enabled {
        ai.enable_culture_road_builds_its_triggers();
    }
    ai
}

/// Every prerequisite of `civic` known (not `civic` itself), so it is open.
fn open_civic(g: &mut Game, civic: &str) {
    let mut stack: Vec<Name> = g.rules.civics[civic].requires.clone();
    while let Some(known) = stack.pop() {
        if g.players[0].civics.insert(known) {
            stack.extend(g.rules.civics[known.as_str()].requires.iter().copied());
        }
    }
}

/// The emperor test empire with Feudalism open and two Builders' charges.
fn farm_board(charges: i32) -> (Game, Pos) {
    let (mut g, cities) =
        crate::ai::commercial_hub::tests::emperor_empire("culture-road-farms", 4811);
    open_civic(&mut g, "feudalism");
    for tile in g.map.tiles.values_mut() {
        if tile.improvement.as_deref() == Some("farm") {
            tile.improvement = None;
        }
    }
    let pos = g.cities[&cities[0]].pos;
    let builder = g.spawn_unit("builder", 0, pos);
    g.units.get_mut(&builder).unwrap().charges = charges;
    let tile = g.cities[&cities[0]]
        .owned_tiles
        .iter()
        .copied()
        .find(|tile| *tile != pos)
        .expect("an owned tile");
    (g, tile)
}

#[test]
fn a_farm_short_of_feudalism_earns_the_premium() {
    let (g, tile) = farm_board(6);
    let ai = culture_ai(true);
    assert!(ai.culture_road_trigger_civic_near(&g, 0, "feudalism"));
    assert_eq!(
        ai.culture_road_builder_premium(&g, tile, "farm"),
        ROAD_IMPROVEMENT_PREMIUM
    );
    assert_eq!(ai.culture_road_builder_premium(&g, tile, "mine"), 0.0);
    assert_eq!(
        culture_ai(false).culture_road_builder_premium(&g, tile, "farm"),
        0.0,
        "off"
    );
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_culture_road_builds_its_triggers();
    assert_eq!(science.culture_road_builder_premium(&g, tile, "farm"), 0.0);
}

#[test]
fn too_few_charges_or_feudalism_inspired_pays_nothing() {
    let (g, tile) = farm_board(1);
    assert_eq!(
        culture_ai(true).culture_road_builder_premium(&g, tile, "farm"),
        0.0
    );
    let (mut g, tile) = farm_board(6);
    g.players[0].boosted_civics.insert(name!("feudalism"));
    assert_eq!(
        culture_ai(true).culture_road_builder_premium(&g, tile, "farm"),
        0.0
    );
}

/// Guilds open; one city with a Market, another with a Commercial Hub.
fn market_board() -> (Game, AdvancedAi, Vec<u32>) {
    let (mut g, cities) =
        crate::ai::commercial_hub::tests::emperor_empire("culture-road-market", 4812);
    open_civic(&mut g, "guilds");
    g.cities
        .get_mut(&cities[1])
        .unwrap()
        .buildings
        .push(name!("market"));
    crate::game::install_test_district(&mut g, cities[0], "commercial_hub");
    g.cities.get_mut(&cities[0]).unwrap().queue.clear();
    std::sync::Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        cities[0],
        crate::rules::Yields {
            production: 20.0,
            ..Default::default()
        },
    );
    (g, culture_ai(true), cities)
}

#[test]
fn guilds_one_market_short_claims_the_market() {
    let (mut g, ai, cities) = market_board();
    let market = Item::Building {
        building: name!("market"),
    };
    assert!(g.can_produce(0, cities[0], &market));
    let plan = ai.assess(&g, 0);
    ai.claim_culture_road_triggers(&mut g, 0, &plan);
    assert_eq!(g.cities[&cities[0]].queue.first(), Some(&market));
    // A second pass queues nothing more: the Market is already coming.
    let queued = |g: &Game| {
        g.player_city_ids(0)
            .into_iter()
            .filter(|cid| g.cities[cid].queue.contains(&market))
            .count()
    };
    assert_eq!(queued(&g), 1);
    ai.claim_culture_road_triggers(&mut g, 0, &plan);
    assert_eq!(queued(&g), 1);
}

#[test]
fn two_markets_short_or_gene_off_claims_nothing() {
    let (mut g, ai, cities) = market_board();
    g.cities
        .get_mut(&cities[1])
        .unwrap()
        .buildings
        .retain(|building| *building != "market");
    let plan = ai.assess(&g, 0);
    let before = g.cities[&cities[0]].queue.clone();
    ai.claim_culture_road_triggers(&mut g, 0, &plan);
    assert_eq!(
        g.cities[&cities[0]].queue, before,
        "two short costs more than it saves"
    );
    let (mut g, _, cities) = market_board();
    let off = culture_ai(false);
    let plan = off.assess(&g, 0);
    let before = g.cities[&cities[0]].queue.clone();
    off.claim_culture_road_triggers(&mut g, 0, &plan);
    assert_eq!(g.cities[&cities[0]].queue, before, "off");
}
