use super::*;
use crate::ai::advanced::{GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::Game;

fn captured_frontier() -> (Game, StrategicPlan, u32) {
    let mut g = Game::new(2, 32, 20, 3800, 250, 0);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
    }
    g.players[0].civ = "Gran Colombia".to_string();
    g.found_city_for(0, (5, 8), None);
    g.found_city_for(0, (13, 8), None);
    g.found_city_for(1, (24, 8), None);
    let captured = *g
        .player_city_ids(0)
        .iter()
        .find(|cid| g.cities[cid].pos == (13, 8))
        .unwrap();
    g.cities.get_mut(&captured).unwrap().original_owner = 1;
    g.at_war.insert((0, 1));
    g.players[0].techs.extend([
        crate::name!("mining"),
        crate::name!("bronze_working"),
        crate::name!("archery"),
    ]);
    g.players[0].boosted_techs.insert(crate::name!("masonry"));
    for _ in 0..3 {
        g.spawn_test_unit("archer", 0, (5, 8));
    }
    g.players[0].research = None;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, plan, captured)
}

#[test]
fn captured_frontier_unlocks_walls_before_waiting_for_a_visible_attacker() {
    let (mut g, plan, cid) = captured_frontier();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_live_bridge_universe();
    let forced = include_str!("../../../../deploy/live-force-on.txt")
        .trim()
        .split(',')
        .collect::<Vec<_>>();
    ai.apply_gene_ledger_with_forced_live(&forced);
    ai.enable_captured_city_wall_research();
    assert_eq!(g.city_max_wall_hp(&g.cities[&cid]), 0);
    assert!(ai.defensive_walls_research_goal(&g, 0, &plan).is_none());
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("masonry"));
}
