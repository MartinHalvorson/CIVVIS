use super::*;
use crate::name;

fn board(cities: &[(usize, Pos)]) -> Game {
    let mut game = Game::new_full(3, 36, 22, 91777, 1000, 0, false);
    for id in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(id);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    for (owner, pos) in cities {
        game.found_city_for(*owner, *pos, None);
    }
    game.turn = 54;
    game
}

fn memory(taker: u32, shooter: u32) -> Siege {
    Siege {
        stage: SiegeStage::Invest,
        taker: Some(taker),
        entered: 49,
        assessed: 53,
        posts: [(taker, (13, 12)), (shooter, (12, 12))].into(),
    }
}

#[test]
fn siege_rebuild_keeps_patience_posts_and_taker_with_their_host_identities() {
    let mut previous = board(&[(0, (6, 12)), (1, (14, 12))]);
    let mut next = board(&[(1, (14, 12)), (0, (6, 12))]);
    let old_taker = previous.spawn_unit("warrior", 0, (13, 12));
    let old_shooter = previous.spawn_unit("archer", 0, (12, 12));
    let shooter = next.spawn_unit("archer", 0, (12, 12));
    let taker = next.spawn_unit("warrior", 0, (13, 12));
    let old_city = previous.city_at((14, 12)).unwrap();
    let city = next.city_at((14, 12)).unwrap();
    assert_ne!(old_city, city);
    assert_ne!(old_taker, taker);
    let mut ai = AdvancedAi::new();
    ai.sieges.insert(old_city, memory(old_taker, old_shooter));
    ai.reserved_units.insert(old_taker);
    ai.remap_siege_memory(
        &previous,
        &next,
        &[(old_taker, taker), (old_shooter, shooter)].into(),
    );
    assert_eq!(ai.sieges.len(), 1);
    assert_eq!(ai.sieges.get(&city), Some(&memory(taker, shooter)));
    assert_eq!(ai.reserved_units, [taker].into());
    // Repeated same-board remaps must not restart Invest's patience clock.
    ai.remap_siege_memory(&next, &next, &[(taker, taker), (shooter, shooter)].into());
    assert_eq!(ai.sieges.get(&city), Some(&memory(taker, shooter)));
}

#[test]
fn siege_rebuild_drops_missing_cities_and_units_without_reserving_reused_ids() {
    let mut previous = board(&[(0, (6, 12)), (1, (14, 12)), (1, (18, 12))]);
    let mut next = board(&[(0, (6, 12)), (2, (22, 12)), (1, (14, 12))]);
    let lost = previous.spawn_unit("warrior", 0, (13, 12));
    let old_shooter = previous.spawn_unit("archer", 0, (12, 12));
    next.spawn_unit("warrior", 2, (21, 12));
    let shooter = next.spawn_unit("archer", 0, (12, 12));
    let mut ai = AdvancedAi::new();
    for pos in [(14, 12), (18, 12)] {
        ai.sieges
            .insert(previous.city_at(pos).unwrap(), memory(lost, old_shooter));
    }
    ai.reserved_units.insert(lost);
    ai.remap_siege_memory(&previous, &next, &[(old_shooter, shooter)].into());
    assert_eq!(ai.sieges.len(), 1);
    let siege = &ai.sieges[&next.city_at((14, 12)).unwrap()];
    assert_eq!(siege.taker, None);
    assert_eq!(siege.posts, [(shooter, (12, 12))].into());
    assert!(ai.reserved_units.is_empty());
    assert_eq!(siege.entered, 49);
}

#[test]
fn siege_rebuild_preserves_real_city_capture_for_normal_maintenance() {
    let previous = board(&[(0, (6, 12)), (1, (14, 12))]);
    let next = board(&[(0, (14, 12)), (0, (6, 12))]);
    let mut ai = AdvancedAi::new();
    let mut siege = memory(999, 998);
    siege.stage = SiegeStage::Take;
    ai.sieges.insert(previous.city_at((14, 12)).unwrap(), siege);
    ai.remap_siege_memory(&previous, &next, &BTreeMap::new());
    let city = next.city_at((14, 12)).unwrap();
    assert_eq!(next.cities[&city].owner, 0);
    assert_eq!(ai.sieges[&city].stage, SiegeStage::Take);
    assert_eq!(ai.sieges[&city].taker, None);
    assert!(ai.sieges[&city].posts.is_empty());
}
