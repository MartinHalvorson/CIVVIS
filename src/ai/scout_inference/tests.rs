use super::*;

fn board() -> (Game, u32, Pos) {
    let mut g = Game::new_full(2, 24, 16, 91780, 250, 0, false);
    let center = *g.map.tiles.keys().find(|p| p.1 == 8).unwrap();
    let scout = g.spawn_test_unit("scout", 0, center);
    g.players[0].explored.clear();
    (g, scout, center)
}

#[test]
fn charted_habitable_land_beats_ocean_and_polar_terrain() {
    let (mut g, scout, target) = board();
    let known = g.nbrs(target)[0];
    g.players[0].explored.insert(known);
    let mut scores = Vec::new();
    for terrain in ["grassland", "coast", "ocean", "tundra", "snow"] {
        g.map.tiles.get_mut(&known).unwrap().terrain = terrain.into();
        scores.push(BasicAi::rival_frontier_prior(&g, 0, scout, target, None));
    }
    assert!(scores[0] > scores[1] && scores[1] > scores[2]);
    assert!(scores[0] > scores[3] && scores[3] > scores[4]);
}

#[test]
fn hidden_terrain_and_cities_do_not_change_the_prior() {
    let (mut g, scout, target) = board();
    g.found_city_for(1, target, None);
    assert!(!g.cities.is_empty());
    let before = BasicAi::rival_frontier_prior(&g, 0, scout, target, None);
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("ocean");
    }
    for city in g.cities.values_mut() {
        city.pos = target;
    }
    assert_eq!(
        before,
        BasicAi::rival_frontier_prior(&g, 0, scout, target, None)
    );
}

#[test]
fn spacing_encourages_search_beyond_home_without_claiming_certainty() {
    let (g, scout, target) = board();
    let far = g.wring(target, 9)[0];
    assert!(
        BasicAi::rival_frontier_prior(&g, 0, scout, target, Some(far))
            > BasicAi::rival_frontier_prior(&g, 0, scout, target, Some(target))
    );
}

#[test]
fn charted_rival_reduces_search_interest_and_observed_edge_discourages_pole() {
    let (mut g, scout, target) = board();
    g.found_city_for(1, target, None);
    let unknown = BasicAi::rival_frontier_prior(&g, 0, scout, target, None);
    g.players[0].explored.insert(target);
    g.map.tiles.get_mut(&target).unwrap().terrain = crate::name!("unknown");
    assert!(BasicAi::rival_frontier_prior(&g, 0, scout, target, None) < unknown);
    g.players[0].explored.clear();
    let edge = *g.map.tiles.keys().find(|p| p.1 == 0).unwrap();
    g.map.tiles.get_mut(&edge).unwrap().terrain = crate::name!("unknown");
    let before = BasicAi::rival_frontier_prior(&g, 0, scout, edge, None);
    g.players[0].explored.insert(edge);
    assert!(BasicAi::rival_frontier_prior(&g, 0, scout, edge, None) < before);
}

#[test]
fn explorer_chooses_habitable_frontier_when_reveal_and_distance_tie() {
    let (mut g, scout, origin) = board();
    let ring = g.wring(origin, 4);
    let a = ring[0];
    let b = *ring.iter().find(|p| g.wdist(a, **p) >= 7).unwrap();
    let (cold, warm) = if a < b { (a, b) } else { (b, a) };
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
    }
    for pos in g.wdisk(cold, 3) {
        g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("snow");
    }
    g.players[0].explored = g.map.tiles.keys().copied().collect();
    g.players[0].explored.remove(&cold);
    g.players[0].explored.remove(&warm);
    let mut ai = BasicAi::new();
    ai.explore_commit = true;
    assert_eq!(
        BasicAi::frontier_reveal_value(&g, 0, scout, cold),
        BasicAi::frontier_reveal_value(&g, 0, scout, warm)
    );
    assert_eq!(ai.exploration_goal(&g, 0, scout, true), Some(warm));
}
