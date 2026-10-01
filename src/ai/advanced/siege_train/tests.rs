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
fn wounded_knight_waits_out_a_lethal_city_volley_before_taking_its_post() {
    let (mut g, cid) = walled_city();
    let city = g.cities[&cid].pos;
    let start = (city.0 - 3, city.1);
    let transit = (city.0 - 2, city.1);
    let post = (city.0 - 1, city.1);
    for pos in [start, transit, post] {
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    let uid = g.spawn_unit("knight", 0, start);
    g.units.get_mut(&uid).unwrap().hp = 70;
    std::sync::Arc::make_mut(&mut g.observed_city_ranged_strength).insert(cid, 95.0);
    assert!(
        super::super::battle_planner::strike_danger(&g, 0, transit, uid) + 20.0 >= 70.0,
        "the first exposed tile would leave no reserve for another volley"
    );
    let mut ai = AdvancedAi::new();
    assert_eq!(ai.approach(&mut g, 0, uid, post, city), None);
    assert_eq!(g.units[&uid].pos, start);

    std::sync::Arc::make_mut(&mut g.observed_city_ranged_strength).insert(cid, 3.0);
    assert_eq!(ai.approach(&mut g, 0, uid, post, city), Some(true));
    assert_eq!(g.units[&uid].pos, post);
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

/// `shared-danger`: a gun approaching its firing post reads each defender's
/// blow split among the train in its reach, so a crowd of defenders no longer
/// charges every blow to every gun at once.
#[test]
fn a_shared_reading_lets_a_gun_into_the_firing_ring() {
    let (mut g, cid) = walled_city();
    let city = g.cities[&cid].pos;
    for pos in g.wdisk(city, 5) {
        if pos == city {
            continue;
        }
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    let start = (city.0 - 3, city.1);
    let transit = (city.0 - 2, city.1);
    for pos in [
        (city.0 + 1, city.1 - 1),
        (city.0 + 1, city.1),
        (city.0, city.1 + 1),
    ] {
        g.spawn_unit("crossbowman", 1, pos);
    }
    for pos in [
        (city.0 - 2, city.1 + 1),
        (city.0 - 1, city.1 + 2),
        (city.0 + 2, city.1 - 2),
    ] {
        g.spawn_unit("pikeman", 0, pos);
    }
    let gun = g.spawn_unit("catapult", 0, start);
    let mut ai = AdvancedAi::new();
    ai.enable_shared_danger();
    // A hit-point level the full reading stops and the shared one admits.
    let hp = (30..=100)
        .rev()
        .find(|hp| {
            g.units.get_mut(&gun).unwrap().hp = *hp;
            let full = super::super::battle_planner::strike_danger(&g, 0, transit, gun);
            let shared = ai.approach_danger(&g, 0, transit, gun);
            f64::from(*hp) <= full + 20.0 && f64::from(*hp) > shared + 20.0
        })
        .expect("fixture: sharing admits a gun the full reading stops");
    g.units.get_mut(&gun).unwrap().hp = hp;

    let mut probe = g.speculative_clone();
    let mut plain = AdvancedAi::new();
    assert_eq!(plain.approach(&mut probe, 0, gun, transit, city), None);
    assert_eq!(ai.approach(&mut g, 0, gun, transit, city), Some(true));
    assert_eq!(g.units[&gun].pos, transit);
}
