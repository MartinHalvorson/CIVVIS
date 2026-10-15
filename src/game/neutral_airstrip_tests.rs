use super::*;

#[test]
fn an_engineer_can_build_an_airstrip_on_unowned_flat_ground() {
    let mut game = Game::new_full(2, 24, 16, 4064, 500, 0, false);
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    let target = (8, 6);
    let tile = game.map.tiles.get_mut(&target).unwrap();
    tile.terrain = crate::name!("grassland");
    tile.feature = None;
    tile.hills = false;
    tile.resource = None;
    tile.improvement = None;
    tile.owner_city = None;
    tile.district = None;
    tile.district_foundation = None;
    tile.wonder = None;
    tile.flooded = false;
    game.players[0].techs.insert(crate::name!("flight"));
    let engineer = game.spawn_test_unit("military_engineer", 0, target);
    assert!(game.units[&engineer].charges > 0);
    assert!(game.units[&engineer].moves_left > 0.0);
    assert!(game.rules.units["military_engineer"]
        .builds
        .contains(&crate::name!("airstrip")));
    assert!(game.map.tiles[&target].owner_city.is_none());
    assert!(
        game.valid_improvements(0, target)
            .contains(&crate::name!("airstrip")),
        "Firaxis Improvements.xml permits Airstrips outside territory"
    );
    let action = Action::Improve {
        unit: engineer,
        improvement: crate::name!("airstrip"),
    };
    assert!(game.legal_actions(0).contains(&action));
    game.apply(0, &action).unwrap();
    assert_eq!(
        game.map.tiles[&target].improvement.as_deref(),
        Some("airstrip")
    );
    assert!(game.map.tiles[&target].owner_city.is_none());
    assert_eq!(game.air_capacity_at(0, target), 3);
}
