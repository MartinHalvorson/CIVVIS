use super::super::{AdvancedAi, VictoryTarget};
use crate::game::Game;
use crate::name::Name;
use crate::Pos;

fn fixture() -> (Game, AdvancedAi, Pos, u32) {
    let mut g = Game::new_full(3, 40, 24, 378700, 300, 0, false);
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
    g.found_city_for(1, (30, 10), None);
    g.players[2].is_minor = true;
    g.found_city_for(2, (18, 10), None);
    g.players[0].envoys.push((2, 6));
    g.players[0].techs.extend(
        ["mining", "industrialization", "flight", "radio"]
            .into_iter()
            .map(Name::new),
    );
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    crate::game::install_test_district(&mut g, home, "aerodrome");
    let deposit = (12, 10);
    g.map.tiles.get_mut(&deposit).unwrap().resource = Some(crate::name!("aluminum"));
    let guard = g.spawn_test_unit("musketman", 0, (12, 11));
    g.current = 0;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    (g, ai, deposit, guard)
}

#[test]
fn needed_defended_aluminum_colony_enters_the_actual_site_scan() {
    let (g, ai, deposit, _) = fixture();
    assert_eq!(g.suzerain_of(2), Some(0));
    assert_eq!(g.strategic_resource_rate(0, "aluminum"), 0.0);
    assert!(!ai.city_state_settlement_exclusion(&g, 0).contains(&deposit),
        "a needed, defended aluminum center beside our city-state remains a candidate");
    assert!(ai.settle_ranking(&g, 0, (8, 10), 8).iter().any(|(pos, _)| *pos == deposit));
}
