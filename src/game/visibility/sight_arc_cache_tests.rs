use super::*;

fn globe_game() -> Game {
    let mut game = Game::new_full(2, 20, 14, 91_611, 40, 0, false);
    for id in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(id);
    }
    game.map = crate::world::WorldMap::globe(2);
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
    }
    game
}

#[test]
fn short_sight_arc_matches_scalar_for_every_small_globe_pair_and_live_heights() {
    let mut game = globe_game();
    let positions: Vec<Pos> = game.map.tiles.keys().copied().collect();
    let mut clear = 0;
    let mut blocked = 0;
    for mixed in [false, true] {
        if mixed {
            for (index, tile) in game.map.tiles.values_mut().enumerate() {
                tile.hills = index % 3 == 0;
                tile.feature = (index % 4 == 0).then_some(crate::name!("forest"));
                if index % 7 == 0 {
                    tile.terrain = crate::name!("mountain");
                }
            }
        }
        for &from in &positions {
            for &to in &positions {
                let distance = game.wdist(from, to);
                if !(2..=3).contains(&distance) {
                    continue;
                }
                for (viewer, target) in [(-1, 0), (0, 0), (0, 1), (1, 1), (1, 2), (2, 3), (3, 2)] {
                    for woods in [false, true] {
                        let original = game.arc_is_clear_uncached(
                            &mut HeightField::none(),
                            from,
                            to,
                            distance,
                            viewer,
                            target,
                            woods,
                        );
                        let cached = game.arc_is_clear(
                            &mut HeightField::none(),
                            from,
                            to,
                            distance,
                            viewer,
                            target,
                            woods,
                        );
                        assert_eq!(
                            cached, original,
                            "{from:?}->{to:?}; {viewer}/{target}/{woods}"
                        );
                        clear += usize::from(original);
                        blocked += usize::from(!original);
                    }
                }
            }
        }
    }
    assert!(clear > 0 && blocked > 0);
}

#[test]
fn short_sight_arc_keeps_geometry_across_clones_but_reads_changed_cover() {
    let game = globe_game();
    let sphere = game.map.sphere().unwrap();
    let from = sphere.positions().next().unwrap();
    let to = sphere.ring(from, 2)[0];
    let ray = sphere
        .short_sight_arc(from, to, 2, || {
            game.draw_short_sight_arc(sphere, from, to, 2)
        })
        .unwrap();
    let blocker = ray.interior()[0];
    assert!(game.arc_is_clear(&mut HeightField::none(), from, to, 2, 0, 0, false));
    let mut changed = game.clone();
    changed.map.tiles.get_mut(&blocker).unwrap().feature = Some(crate::name!("forest"));
    assert!(std::ptr::eq(
        ray,
        changed
            .map
            .sphere()
            .unwrap()
            .short_sight_arc(from, to, 2, || panic!("clone should reuse warm geometry"))
            .unwrap()
    ));
    assert!(!changed.arc_is_clear(&mut HeightField::none(), from, to, 2, 0, 0, false));
    assert!(changed.arc_is_clear(&mut HeightField::none(), from, to, 2, 0, 0, true));
    assert!(game.arc_is_clear(&mut HeightField::none(), from, to, 2, 0, 0, false));
    changed.map.tiles.get_mut(&blocker).unwrap().terrain = crate::name!("mountain");
    assert!(!changed.arc_is_clear(&mut HeightField::none(), from, to, 2, 0, 0, true));
}

#[test]
fn long_sight_arcs_retain_original_scalar_path() {
    let mut game = globe_game();
    for (index, tile) in game.map.tiles.values_mut().enumerate() {
        tile.hills = index % 2 == 0;
    }
    let sphere = game.map.sphere().unwrap();
    for from in sphere.positions() {
        for to in sphere.positions() {
            let steps = game.wdist(from, to);
            if steps <= 3 {
                continue;
            }
            assert!(sphere
                .short_sight_arc(from, to, steps, || panic!("long ray stays scalar"))
                .is_none());
            assert_eq!(
                game.arc_is_clear(&mut HeightField::none(), from, to, steps, 0, 0, false),
                game.arc_is_clear_uncached(&mut HeightField::none(), from, to, steps, 0, 0, false),
            );
        }
    }
}
