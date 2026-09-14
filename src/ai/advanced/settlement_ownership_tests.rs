use super::*;

#[test]
fn settlement_scans_reject_claims_without_a_known_friendly_city() {
    let mut game = Game::new_full(2, 24, 18, 9143270, 120, 0, false);
    game.clear_mirror_cities();
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.owner_city = None;
    }
    let mut cities = Vec::new();
    for (owner, pos) in [(0, (3, 6)), (1, (15, 6))] {
        game.current = owner;
        let unit = game.spawn_test_unit("settler", owner, pos);
        game.apply(owner, &Action::FoundCity { unit }).unwrap();
        cities.push(game.city_at(pos).unwrap());
    }
    game.current = 0;
    let site = (8, 10);
    assert!(game
        .cities
        .values()
        .all(|city| game.wdist(city.pos, site) > 3));
    let missing_city = u32::MAX;
    assert!(!game.cities.contains_key(&missing_city));
    let ai = AdvancedAi::legacy();
    for (claim, accepted) in [
        (None, true),
        (Some(cities[0]), true),
        (Some(cities[1]), false),
        (Some(missing_city), false),
    ] {
        game.map.tiles.get_mut(&site).unwrap().owner_city = claim;
        for stop_at_first in [false, true] {
            // Hold scoring above the acceptance floor so this exercises the
            // real candidate filter in both ranking and existence searches.
            let mut scores = BTreeMap::from([(site, 100.0)]);
            let sites = ai.settle_sites_scanning(
                &game,
                0,
                site,
                0,
                None,
                Some(&mut scores),
                stop_at_first,
                false,
            );
            assert_eq!(
                sites.iter().any(|(pos, _)| *pos == site),
                accepted,
                "claim={claim:?}, stop_at_first={stop_at_first}"
            );
        }
    }
}
