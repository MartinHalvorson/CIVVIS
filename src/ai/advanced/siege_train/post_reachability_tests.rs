use super::tests::walled_city;
use super::*;

fn separated_firing_tiles(kind: &str) -> (Game, u32, u32, Pos, Pos) {
    let (mut g, cid) = walled_city();
    let city = g.cities[&cid].pos;
    let at = |x, y| (city.0 + x, city.1 + y);
    let start = at(-4, 0);
    let pocket = at(-2, 0);
    let reachable = at(-2, 2);
    let ground = [
        city,
        start,
        pocket,
        at(-1, 0), // Sight into the pocket crosses the reserved inner ring.
        at(-1, 1), // Sight from the reachable firing tile.
        at(-4, 1),
        at(-4, 2),
        at(-3, 2),
        reachable,
    ];
    for tile in g.map.tiles.values_mut() {
        tile.terrain = if ground.contains(&tile.pos) {
            crate::name!("grassland")
        } else {
            crate::name!("mountain")
        };
        tile.feature = None;
        tile.hills = false;
    }
    let uid = g.spawn_unit(kind, 0, start);
    assert_eq!(g.unit_attack_range(uid), 2);
    assert!(g.wdist(start, pocket) < g.wdist(start, reachable));
    assert!(g.unit_has_line_of_sight_from(uid, pocket, city));
    assert!(g.unit_has_line_of_sight_from(uid, reachable, city));
    assert_eq!(siege_route_step(&g, 0, uid, pocket, city), None);
    assert!(siege_route_step(&g, 0, uid, reachable, city).is_some());
    (g, cid, uid, pocket, reachable)
}

fn reaches_and_fires(kind: &str) {
    let (mut g, cid, uid, _, reachable) = separated_firing_tiles(kind);
    let city = CityView::of(&g, cid).unwrap();
    let post = siege_posts(&g, 0, &city, &[uid], None)[&uid];
    assert_eq!(
        post, reachable,
        "a clear but sealed firing pocket must not displace a reachable post"
    );
    let mut ai = AdvancedAi::new();
    ai.enable_recorded_tactical_step();
    for _ in 0..4 {
        ai.approach(&mut g, 0, uid, post, city.pos);
        g.apply(0, &Action::EndTurn).unwrap();
        g.apply(1, &Action::EndTurn).unwrap();
        if g.units[&uid].pos == post {
            break;
        }
    }
    assert_eq!(
        g.units[&uid].pos, post,
        "the assigned post is physically reached"
    );
    let walls = g.cities[&cid].wall_hp;
    g.apply(
        0,
        &Action::Ranged {
            unit: uid,
            target: city.pos,
        },
    )
    .unwrap();
    assert!(
        g.cities[&cid].wall_hp < walls,
        "the arrived unit fires at the wall"
    );
}

#[test]
fn a_siege_gun_reaches_and_fires_from_the_available_post() {
    reaches_and_fires("catapult");
}

#[test]
fn a_ranged_shooter_reaches_and_fires_from_the_available_post() {
    reaches_and_fires("archer");
}

#[test]
fn a_gun_without_any_reachable_firing_tile_gets_no_post() {
    let (mut g, cid, uid, _, _) = separated_firing_tiles("catapult");
    let city = CityView::of(&g, cid).unwrap();
    let start = g.units[&uid].pos;
    for pos in g.nbrs(start) {
        g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("mountain");
    }
    assert_eq!(
        siege_route_step(&g, 0, uid, (city.pos.0 - 2, city.pos.1 + 2), city.pos),
        None
    );
    assert!(
        !siege_posts(&g, 0, &city, &[uid], None).contains_key(&uid),
        "an unreachable clear shot must not be assigned as a firing post"
    );
    assert_eq!(g.units[&uid].pos, start);
}
