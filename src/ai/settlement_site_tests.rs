use super::*;

#[test]
fn settlement_requires_known_friendly_ownership_or_unclaimed_land() {
    let mut game = Game::new_full(2, 24, 18, 91_773, 120, 0, false);
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.owner_city = None;
    }
    let own = game.found_city_for(0, (3, 3), None);
    let enemy = game.found_city_for(1, (3, 12), None);
    let site = (12, 8);
    let ai = BasicAi::new();
    game.map.tiles.get_mut(&site).unwrap().owner_city = None;
    assert!(ai.valid_settle_site(&game, 0, site));
    game.map.tiles.get_mut(&site).unwrap().owner_city = Some(own);
    assert!(ai.valid_settle_site(&game, 0, site));
    game.map.tiles.get_mut(&site).unwrap().owner_city = Some(enemy);
    assert!(!ai.valid_settle_site(&game, 0, site));
    game.cities.remove(&enemy);
    assert!(game.map.tiles[&site].owner_city.is_some());
    assert!(!ai.valid_settle_site(&game, 0, site));
}
