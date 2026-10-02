use super::tests::{at_distance, walled_city};
use super::*;

#[test]
fn a_firing_post_must_allow_an_actual_shot_past_the_terrain() {
    let (mut g, cid) = walled_city();
    let target = g.cities[&cid].pos;
    for pos in g.wdisk(target, 3) {
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    let gun = g.spawn_unit("catapult", 0, at_distance(&g, cid, 3)[0]);
    let city = CityView::of(&g, cid).unwrap();
    let first = siege_posts(&g, 0, &city, &[gun], None)[&gun];
    for pos in g
        .nbrs(first)
        .into_iter()
        .filter(|p| g.wdist(*p, target) == 1)
        .collect::<Vec<_>>()
    {
        g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("mountain");
    }
    assert!(!g.unit_has_line_of_sight_from(gun, first, target));
    let chosen = siege_posts(&g, 0, &city, &[gun], None)[&gun];
    assert!(
        g.unit_has_line_of_sight_from(gun, chosen, target),
        "the nearest ring tile is blocked; select a position with a shot instead"
    );
    g.remove_unit(gun);
    let ready_gun = g.spawn_unit("catapult", 0, chosen);
    let walls = g.cities[&cid].wall_hp;
    g.apply(
        0,
        &Action::Ranged {
            unit: ready_gun,
            target,
        },
    )
    .unwrap();
    assert!(
        g.cities[&cid].wall_hp < walls,
        "a ready gun at the assigned tile damages the wall"
    );
}

#[test]
fn a_gun_with_a_clear_shot_keeps_its_firing_position() {
    let (mut g, cid) = walled_city();
    let here = at_distance(&g, cid, 2)
        .into_iter()
        .find(|pos| g.line_of_sight_from(*pos, g.cities[&cid].pos))
        .expect("a clear firing tile");
    let gun = g.spawn_unit("catapult", 0, here);
    let city = CityView::of(&g, cid).unwrap();
    assert!(g.unit_has_line_of_sight_from(gun, here, city.pos));
    assert_eq!(siege_posts(&g, 0, &city, &[gun], None)[&gun], here);
}

/// A city whose every range-2 line is blocked still gets its guns: an
/// adjacent tile always has the shot.
#[test]
fn a_gun_with_no_range_two_line_takes_an_adjacent_post() {
    let (mut g, cid) = walled_city();
    let target = g.cities[&cid].pos;
    for pos in g.wdisk(target, 3) {
        if pos == target {
            continue;
        }
        let adjacent = g.wdist(pos, target) == 1;
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.hills = false;
        tile.feature = adjacent.then(|| crate::name!("forest"));
    }
    let gun = g.spawn_unit("catapult", 0, at_distance(&g, cid, 3)[0]);
    assert!(
        at_distance(&g, cid, 2)
            .into_iter()
            .all(|pos| !g.unit_has_line_of_sight_from(gun, pos, target)),
        "fixture: the forest ring blocks every range-2 line"
    );
    let city = CityView::of(&g, cid).unwrap();
    let post = siege_posts(&g, 0, &city, &[gun], None)
        .get(&gun)
        .copied()
        .expect("the gun gets a firing post");
    assert_eq!(g.wdist(post, target), 1);
    assert!(g.unit_has_line_of_sight_from(gun, post, target));
}

/// See `taker_corridor`: guns on every land tile two out from a breached
/// coastal city seal its taker's approach; one of those tiles is kept clear.
#[test]
fn a_breached_city_keeps_a_corridor_clear_for_its_taker() {
    let (mut g, cid) = walled_city();
    let target = g.cities[&cid].pos;
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    let ring = g
        .nbrs(target)
        .into_iter()
        .find(|pos| g.map.get(*pos).is_some())
        .unwrap();
    let approach: Vec<Pos> = g
        .nbrs(ring)
        .into_iter()
        .filter(|pos| g.wdist(*pos, target) == 2)
        .take(2)
        .collect();
    assert_eq!(approach.len(), 2, "fixture: two approach tiles");
    let start = g
        .nbrs(approach[0])
        .into_iter()
        .find(|pos| g.wdist(*pos, target) == 3 && g.nbrs(approach[1]).contains(pos))
        .or_else(|| {
            g.nbrs(approach[0])
                .into_iter()
                .find(|pos| g.wdist(*pos, target) == 3)
        })
        .expect("fixture: a start three out");
    let land: Vec<Pos> = [target, ring, start]
        .into_iter()
        .chain(approach.iter().copied())
        .collect();
    for pos in g.wdisk(target, 4) {
        let outer = g.wdist(pos, target) == 4;
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.feature = None;
        tile.hills = false;
        tile.terrain = if land.contains(&pos) || outer {
            crate::name!("grassland")
        } else {
            crate::name!("coast")
        };
    }
    let guns: Vec<u32> = approach
        .iter()
        .map(|pos| g.spawn_unit("catapult", 0, *pos))
        .collect();
    let taker = g.spawn_unit("swordsman", 0, start);
    let city = CityView::of(&g, cid).unwrap();
    let mut force = guns.clone();
    force.push(taker);
    let open_land = |pos: Pos| {
        g.map
            .get(pos)
            .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
    };
    let corridor = taker_corridor(&g, &city, Some(taker), &BTreeSet::new(), &open_land)
        .expect("a corridor two out");
    assert!(approach.contains(&corridor));
    let posts = siege_posts(&g, 0, &city, &force, Some(taker));
    assert!(
        guns.iter().all(|gun| posts.get(gun) != Some(&corridor)),
        "no gun keeps the corridor: {posts:?}"
    );

    // With the walls standing, the guns keep their firing tiles.
    g.cities.get_mut(&cid).unwrap().wall_hp = 100;
    let walled = CityView::of(&g, cid).unwrap();
    assert_eq!(
        taker_corridor(&g, &walled, Some(taker), &BTreeSet::new(), &open_land),
        None
    );
}
