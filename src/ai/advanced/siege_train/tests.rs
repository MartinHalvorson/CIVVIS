use super::tests::walled_city;
use super::*;

#[test]
fn infantry_takes_a_detour_away_from_its_post_around_a_blocked_pass() {
    let (mut g, cid) = walled_city();
    let city = g.cities[&cid].pos;
    let at = |x, y| (city.0 + x, city.1 + y);
    let start = at(-2, 0);
    let goal = at(-1, 1);
    let corridor = [
        start,
        at(-3, 0),
        at(-4, 1),
        at(-4, 2),
        at(-3, 2),
        at(-2, 2),
        goal,
    ];
    for tile in g.map.tiles.values_mut() {
        tile.terrain = if corridor.contains(&tile.pos) || tile.pos == city {
            crate::name!("grassland")
        } else {
            crate::name!("mountain")
        };
        tile.feature = None;
        tile.hills = false;
    }
    let uid = g.spawn_unit("warrior", 0, start);
    assert!(g.can_move(uid, at(-3, 0)));
    assert!(g.wdist(at(-3, 0), goal) > g.wdist(start, goal));
    let mut ai = AdvancedAi::new();
    for _ in 0..5 {
        ai.approach(&mut g, 0, uid, goal, city);
        if g.units[&uid].pos == goal {
            break;
        }
        g.apply(0, &Action::EndTurn).unwrap();
        g.apply(1, &Action::EndTurn).unwrap();
    }
    assert_eq!(
        g.units[&uid].pos, goal,
        "the reachable siege post must not be abandoned at a local distance minimum"
    );
}

#[test]
fn approach_does_not_cross_another_reserved_ring_tile() {
    let (mut g, cid) = walled_city();
    let city = g.cities[&cid].pos;
    let start = (city.0, city.1 - 2);
    let wrong_ring = (city.0, city.1 - 1);
    let goal = (city.0 - 1, city.1);
    for tile in g.map.tiles.values_mut() {
        tile.terrain = if [city, start, wrong_ring, goal].contains(&tile.pos) {
            crate::name!("grassland")
        } else {
            crate::name!("mountain")
        };
        tile.feature = None;
        tile.hills = false;
    }
    let uid = g.spawn_unit("warrior", 0, start);
    assert!(g.can_move(uid, wrong_ring));
    let mut ai = AdvancedAi::new();
    assert_eq!(ai.approach(&mut g, 0, uid, goal, city), None);
    assert_eq!(g.units[&uid].pos, start);
}

#[test]
fn open_approach_reaches_the_post_without_displacing_a_friendly_builder() {
    let (mut g, cid) = walled_city();
    let city = g.cities[&cid].pos;
    let start = (city.0 - 3, city.1);
    let transit = (city.0 - 2, city.1);
    let goal = (city.0 - 1, city.1);
    for pos in [start, transit, goal] {
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    let uid = g.spawn_unit("warrior", 0, start);
    let builder = g.spawn_unit("builder", 0, transit);
    let mut ai = AdvancedAi::new();
    assert_eq!(ai.approach(&mut g, 0, uid, goal, city), Some(true));
    assert_eq!(g.units[&uid].pos, goal);
    assert_eq!(g.units[&builder].pos, transit);
}

#[test]
fn siege_assigns_a_reachable_post_instead_of_a_closer_sealed_pocket() {
    let (mut g, cid) = walled_city();
    let city = g.cities[&cid].pos;
    let at = |x, y| (city.0 + x, city.1 + y);
    let start = at(-3, 0);
    let pocket = at(-1, 0);
    let reachable = at(-1, 1);
    let corridor = [
        city,
        start,
        pocket,
        at(-3, 1),
        at(-3, 2),
        at(-2, 2),
        reachable,
    ];
    for tile in g.map.tiles.values_mut() {
        tile.terrain = if corridor.contains(&tile.pos) {
            crate::name!("grassland")
        } else {
            crate::name!("mountain")
        };
        tile.feature = None;
        tile.hills = false;
    }
    let uid = g.spawn_unit("warrior", 0, start);
    assert!(g.wdist(start, pocket) < g.wdist(start, reachable));
    let view = CityView::of(&g, cid).unwrap();
    let posts = siege_posts(&g, 0, &view, &[uid], None);
    assert_eq!(posts.get(&uid), Some(&reachable));
    let mut ai = AdvancedAi::new();
    for _ in 0..4 {
        ai.approach(&mut g, 0, uid, reachable, city);
        if g.units[&uid].pos == reachable {
            break;
        }
        g.apply(0, &Action::EndTurn).unwrap();
        g.apply(1, &Action::EndTurn).unwrap();
    }
    assert_eq!(g.units[&uid].pos, reachable);
}
