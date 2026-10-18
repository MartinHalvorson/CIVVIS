use super::super::{AdvancedAi, GrandStrategy, VictoryTarget};
use super::FOOTHOLD_VALUE;
use crate::game::Game;

/// Our capital far west; the rival's capital far east, with one town 9
/// tiles from it and another, nearer us, 13 tiles from it.
fn board() -> (Game, u32, u32) {
    let mut g = Game::new_full(2, 40, 24, 373_207, 300, 0, false);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
    }
    g.found_city_for(0, (6, 10), None);
    let capital = g.found_city_for(1, (32, 10), None);
    assert!(g.cities[&capital].is_capital);
    let foothold = g.found_city_for(1, (23, 10), None);
    let nearer = g.found_city_for(1, (19, 10), None);
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    (g, foothold, nearer)
}

#[test]
fn the_town_in_bomber_reach_of_the_capital_leads_the_campaign() {
    let (g, foothold, nearer) = board();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let campaign = |ai: &AdvancedAi, city| {
        ai.campaign_city_value(&g, 0, &g.cities[&city], GrandStrategy::Conquest)
    };
    assert_eq!(
        ai.foothold_value(&g, 0, &g.cities[&foothold], GrandStrategy::Conquest),
        0.0,
        "off by default"
    );
    assert!(
        campaign(&ai, nearer) < campaign(&ai, foothold),
        "without the gene the town nearer home leads"
    );

    ai.enable_foothold_faces_the_capital();
    let value = |city| ai.foothold_value(&g, 0, &g.cities[&city], GrandStrategy::Conquest);
    assert_eq!(value(foothold), FOOTHOLD_VALUE);
    assert_eq!(value(nearer), 0.0, "13 tiles out bases no strike");
    assert!(
        campaign(&ai, foothold) < campaign(&ai, nearer),
        "the foothold now leads"
    );
    assert_eq!(
        ai.foothold_value(&g, 0, &g.cities[&foothold], GrandStrategy::Expansion),
        0.0,
        "only a conquest prices it"
    );
}

#[test]
fn no_credit_once_we_hold_a_base_or_outside_domination() {
    let (mut g, foothold, _) = board();
    let mut ai = AdvancedAi::new();
    ai.enable_foothold_faces_the_capital();
    assert_eq!(
        ai.foothold_value(&g, 0, &g.cities[&foothold], GrandStrategy::Conquest),
        0.0,
        "no Domination target, no credit"
    );

    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_foothold_faces_the_capital();
    g.found_city_for(0, (30, 6), None);
    assert_eq!(
        ai.foothold_value(&g, 0, &g.cities[&foothold], GrandStrategy::Conquest),
        0.0,
        "a city of ours already reaches the capital"
    );
}
