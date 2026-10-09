use super::*;
use crate::name::Name;

/// A flat grassland map with a Settler on an interior tile, a site beside it
/// and a site four tiles out.
fn board() -> (Game, u32, Pos, Pos) {
    let mut game = Game::new_full(1, 24, 16, 52_311, 120, 0, false);
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = Name::new("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.owner_city = None;
        tile.river_edges = [false; 6];
        tile.cliff_edges = [false; 6];
    }
    let start = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.wdisk(*position, 5).len() == 91)
        .expect("the test map has a deep interior tile");
    let near = game
        .nbrs(start)
        .into_iter()
        .find(|position| game.map.get(*position).is_some())
        .expect("the start has a neighbour");
    let far = game
        .wring(start, 4)
        .into_iter()
        .find(|position| game.map.get(*position).is_some())
        .expect("the start has a ring four tiles out");
    let settler = game.spawn_test_unit("settler", 0, start);
    (game, settler, near, far)
}

#[test]
fn the_far_site_wins_only_by_the_margin_and_only_with_the_gene() {
    let (game, settler, near, far) = board();
    let paths = game.paths_to(settler);
    assert!(
        paths.contains_key(&near),
        "the neighbour is in this turn's reach"
    );
    assert!(
        !paths.contains_key(&far),
        "four tiles out is beyond this turn's reach"
    );

    let stock = AdvancedAi::new();
    let mut gene = AdvancedAi::new();
    gene.enable_settler_walks_to_the_better_site();

    let much_better = [(far, 80.0), (near, 50.0)];
    assert_eq!(
        stock.better_site_beyond_the_turn(&game, settler, &much_better, &paths, (near, 50.0)),
        None
    );
    assert_eq!(
        gene.better_site_beyond_the_turn(&game, settler, &much_better, &paths, (near, 50.0)),
        Some((far, 80.0))
    );

    let near_tie = [(far, 50.0 + FAR_SITE_MARGIN), (near, 50.0)];
    assert_eq!(
        gene.better_site_beyond_the_turn(&game, settler, &near_tie, &paths, (near, 50.0)),
        None,
        "a site worth no more than the margin above the near one is founded now"
    );
}

#[test]
fn a_far_site_without_a_route_does_not_replace_the_near_one() {
    let (mut game, settler, near, far) = board();
    for position in game.wring(far, 1) {
        if let Some(tile) = game.map.tiles.get_mut(&position) {
            tile.terrain = Name::new("ocean");
        }
    }
    let paths = game.paths_to(settler);
    let mut gene = AdvancedAi::new();
    gene.enable_settler_walks_to_the_better_site();
    let candidates = [(far, 90.0), (near, 50.0)];
    assert_eq!(
        gene.better_site_beyond_the_turn(&game, settler, &candidates, &paths, (near, 50.0)),
        None
    );
}
