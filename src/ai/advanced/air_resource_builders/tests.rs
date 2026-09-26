use super::super::{AdvancedAi, GrandStrategy, VictoryTarget};
use crate::game::Game;
use crate::name::Name;
use crate::Pos;

fn fixture() -> (Game, AdvancedAi, u32, usize, Pos) {
    let mut g = Game::new_full(2, 40, 24, 377900, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let home = g.found_city_for(0, (8, 10), None);
    g.found_city_for(0, (4, 10), None);
    let minor = 1;
    g.players[minor].is_minor = true;
    g.players[minor].civ = "Taruga".to_string();
    g.found_city_for(minor, (15, 10), None);
    g.players[0].envoys.push((minor, 3));
    g.players[0].techs.extend(
        ["mining", "industrialization", "flight", "radio"]
            .into_iter()
            .map(Name::new),
    );
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    crate::game::install_test_district(&mut g, home, "aerodrome");
    let deposit = (14, 10);
    g.map.tiles.get_mut(&deposit).unwrap().resource = Some(crate::name!("aluminum"));
    let builder = g.spawn_test_unit("builder", 0, (13, 10));
    g.current = 0;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    (g, ai, builder, minor, deposit)
}

#[test]
fn nearby_suzerain_aluminum_is_connected_during_the_beeline() {
    let (mut g, mut ai, builder, _, deposit) = fixture();
    assert_eq!(g.strategic_resource_rate(0, "aluminum"), 0.0);
    assert!(!g.players[0].techs.contains(&crate::name!("advanced_flight")));
    assert!(g.valid_improvements(0, deposit).contains(&crate::name!("mine")));
    for _ in 0..2 {
        assert!(ai.advanced_builder_step(&mut g, 0, builder, GrandStrategy::Conquest));
    }
    assert_eq!(g.map.tiles[&deposit].improvement, Some(crate::name!("mine")));
    assert_eq!(g.strategic_resource_rate(0, "aluminum"), 2.0);
}
