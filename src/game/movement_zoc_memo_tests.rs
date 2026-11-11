//! Movement-flood ZOC reuse expires between formations and board changes.

use super::*;

fn board(planet: bool) -> (Game, Pos, Pos) {
    let mut game = if planet {
        let size = crate::setup::MapSize::for_players(2);
        let (width, height) = size.dimensions(crate::setup::MapTopology::Planet);
        Game::new_with(GameOptions {
            map_topology: crate::setup::MapTopology::Planet,
            ..GameOptions::new(2, width, height, 6117, 40, 0)
        })
    } else {
        Game::new_full(2, 20, 14, 6117, 40, 0, false)
    };
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    game.map.clear_rivers();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.owner_city = None;
        tile.road = 0;
    }
    let start = *game
        .map
        .tiles
        .keys()
        .find(|pos| game.wdisk(**pos, 3).len() >= 37)
        .unwrap();
    let watcher = game
        .wdisk(start, 2)
        .into_iter()
        .find(|pos| game.wdist(start, *pos) == 2)
        .unwrap();
    (game, start, watcher)
}

// Compare both score representations AND every parent chosen by strict
// improvement/LIFO traversal. A fresh clone has no scratch from earlier asks.
fn check_fresh(game: &Game, uid: u32) -> Vec<(Pos, u64)> {
    fn snapshot<V: FloodArrival>(game: &Game, uid: u32) -> (BTreeMap<Pos, V>, BTreeMap<Pos, Pos>) {
        let _memo = game.query_memo();
        let mut parents = BTreeMap::new();
        let result = game.relax_movement(
            uid,
            game.units[&uid].pos,
            5.0,
            |from, to| game.can_pass_neighbor(uid, from, to),
            |to, from| {
                parents.insert(to, from);
            },
        );
        (result, parents)
    }
    let fresh = game.clone();
    let (actual, parents) = snapshot::<f64>(game, uid);
    let (expected, expected_parents) = snapshot::<f64>(&fresh, uid);
    let bits = |map: BTreeMap<Pos, f64>| {
        map.into_iter()
            .map(|(pos, value)| (pos, value.to_bits()))
            .collect::<Vec<_>>()
    };
    let actual = bits(actual);
    assert_eq!(actual, bits(expected));
    assert_eq!(parents, expected_parents);
    let (actual_approach, parents) = snapshot::<(f64, bool)>(game, uid);
    let (expected_approach, expected_parents) = snapshot::<(f64, bool)>(&fresh, uid);
    let approach_bits = |map: BTreeMap<Pos, (f64, bool)>| {
        map.into_iter()
            .map(|(pos, (value, stopped))| (pos, value.to_bits(), stopped))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        approach_bits(actual_approach),
        approach_bits(expected_approach)
    );
    assert_eq!(parents, expected_parents);
    actual
}

#[test]
fn changing_war_and_removing_a_projector_expire_both_flood_modes() {
    for planet in [false, true] {
        let (mut game, start, watcher) = board(planet);
        let mover = game.spawn_unit("warrior", 0, start);
        let enemy = game.spawn_unit("warrior", 1, watcher);
        let peace = check_fresh(&game, mover);
        game.at_war.insert(pair(0, 1));
        let war = check_fresh(&game, mover);
        assert_ne!(peace, war, "the projector must change the flood");
        let scratch = game.route_scratch.borrow();
        assert!(scratch.movement_zoc.contains(&Some(true)));
        assert!(scratch.movement_zoc.contains(&Some(false)));
        drop(scratch);
        game.remove_unit(enemy);
        let removed = check_fresh(&game, mover);
        assert_ne!(war, removed);
    }
}

#[test]
fn another_formation_does_not_inherit_cavalry_immunity() {
    let (mut game, start, watcher) = board(false);
    let mover = game.spawn_unit("warrior", 0, start);
    game.spawn_unit("warrior", 1, watcher);
    game.at_war.insert(pair(0, 1));
    let infantry = check_fresh(&game, mover);
    game.units.get_mut(&mover).unwrap().kind = crate::name!("horseman");
    let cavalry = check_fresh(&game, mover);
    assert_ne!(infantry, cavalry);
    game.units.get_mut(&mover).unwrap().kind = crate::name!("warrior");
    assert_eq!(infantry, check_fresh(&game, mover));
}

#[test]
fn religious_zoc_changes_without_a_war_or_tile_mutation() {
    let (mut game, start, watcher) = board(false);
    let mover = game.spawn_unit("apostle", 0, start);
    let other = game.spawn_unit("apostle", 1, watcher);
    game.units.get_mut(&mover).unwrap().religion = Some("Buddhism".into());
    game.units.get_mut(&other).unwrap().religion = Some("Catholicism".into());
    let rival_faith = check_fresh(&game, mover);
    game.units.get_mut(&other).unwrap().religion = Some("Buddhism".into());
    assert_ne!(rival_faith, check_fresh(&game, mover));
}
