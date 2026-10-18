//! `air-wing-grows-with-the-aluminum`.

use super::*;
use crate::name;

/// Two cities of ours with Advanced Flight, `mines` Aluminum mines spread
/// over their land, and `bank` Aluminum in the stockpile.
fn aluminum_empire(mines: usize, bank: f64) -> Game {
    let mut g = Game::new_full(2, 40, 24, 936_001, 1000, 0, false);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    let first = g.found_city_for(0, (6, 12), None);
    let second = g.found_city_for(0, (14, 12), None);
    for tech in g.rules.tech_ancestors[AIR_SURGE_GOAL_TECH].clone() {
        g.players[0].techs.insert(Name::new(&tech));
    }
    g.players[0].techs.insert(Name::new(AIR_SURGE_GOAL_TECH));
    g.players[0]
        .strategic_resources
        .insert(name!("aluminum"), bank);
    let mut sites: Vec<Pos> = [first, second]
        .iter()
        .flat_map(|cid| {
            let centre = g.cities[cid].pos;
            g.cities[cid]
                .owned_tiles
                .iter()
                .copied()
                .filter(move |pos| *pos != centre)
                .collect::<Vec<_>>()
        })
        .collect();
    sites.sort_unstable();
    for pos in sites.into_iter().take(mines) {
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.resource = Some(name!("aluminum"));
        tile.improvement = Some(name!("mine"));
    }
    g.turn = 160;
    g.current = 0;
    g
}

#[test]
fn a_rich_aluminum_income_grows_the_wing_past_four() {
    let g = aluminum_empire(10, 25.0);
    let stock = AdvancedAi::new();
    assert_eq!(
        stock.air_wing_bomber_goal(&g, 0),
        AIR_SURGE_BOMBERS,
        "off: the shipped ceiling"
    );
    let mut ai = AdvancedAi::new();
    ai.enable_air_wing_grows_with_the_aluminum();
    let grown = ai.air_wing_bomber_goal(&g, 0);
    assert!(
        grown > AIR_SURGE_BOMBERS && grown <= AIR_WING_GROWN_BOMBERS,
        "on: the metal sustains more than four ({grown})"
    );
}

#[test]
fn a_thin_income_keeps_the_shipped_goal() {
    let g = aluminum_empire(1, 0.0);
    let stock = AdvancedAi::new();
    let mut ai = AdvancedAi::new();
    ai.enable_air_wing_grows_with_the_aluminum();
    assert_eq!(
        ai.air_wing_bomber_goal(&g, 0),
        stock.air_wing_bomber_goal(&g, 0)
    );
    assert!(ai.air_wing_bomber_goal(&g, 0) <= AIR_SURGE_BOMBERS);
}
