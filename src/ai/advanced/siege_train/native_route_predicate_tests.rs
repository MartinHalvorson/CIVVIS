use super::tests::walled_city;
use super::*;

fn open_game() -> (Game, u32, Pos) {
    let (mut g, cid) = walled_city();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.owner_city = None;
    }
    let city = g.cities[&cid].pos;
    let start = g
        .map
        .tiles
        .iter()
        .map(|(pos, _)| *pos)
        .filter(|pos| g.wdist(*pos, city) > 3)
        .min()
        .unwrap();
    let uid = g.spawn_unit("warrior", 0, start);
    (g, uid, city)
}

fn goals(g: &Game) -> Vec<Pos> {
    let positions: Vec<Pos> = g.map.tiles.iter().map(|(pos, _)| *pos).collect();
    positions
        .iter()
        .step_by((positions.len() / 16).max(1))
        .copied()
        .collect()
}

fn compare_siege(g: &Game, uid: u32, city: Pos) -> usize {
    let mut reachable = 0;
    for goal in goals(g) {
        let expected = old_siege_route_step(g, 0, uid, goal, city);
        assert_eq!(
            siege_route_step(g, 0, uid, goal, city),
            expected,
            "goal {goal:?}"
        );
        reachable += usize::from(expected.is_some());
    }
    reachable
}

#[test]
fn predicate_routes_match_old_set_bfs_with_terrain_and_blocked_tiles() {
    let mut reachable = 0;
    for pattern in 0..4 {
        let (mut g, uid, city) = open_game();
        let start = g.units[&uid].pos;
        let positions = goals(&g);
        for pos in positions
            .iter()
            .copied()
            .filter(|pos| *pos != start && *pos != city)
        {
            if (pos.0 + pos.1 * 2 + pattern) % 7 == 0 {
                g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("coast");
            }
        }
        g.players[0].techs.insert(crate::name!("shipbuilding"));
        for blocked_pattern in 0..4 {
            let blocked: BTreeSet<Pos> = g
                .map
                .tiles
                .iter()
                .map(|(pos, _)| *pos)
                .filter(|pos| blocked_pattern != 0 && (pos.0 + 3 * pos.1) % 5 == blocked_pattern)
                .collect();
            for target in positions.iter().copied() {
                let expected = g.route_step_avoiding_tiles(uid, target, &blocked);
                let actual = g.route_step_avoiding_when(uid, target, |pos| blocked.contains(&pos));
                assert_eq!(
                    actual, expected,
                    "terrain {pattern}, blocked {blocked_pattern}, target {target:?}"
                );
                reachable += usize::from(expected.is_some());
            }
        }
    }
    assert!(
        reachable > 20,
        "the oracle comparisons must exercise real routes"
    );
}

#[test]
fn land_siege_routes_match_the_frozen_water_set_caller() {
    let mut reachable = 0;
    for kind in ["warrior", "catapult", "archer"] {
        for pattern in 0..3 {
            let (mut g, original, city) = open_game();
            let start = g.units[&original].pos;
            g.remove_unit(original);
            let uid = g.spawn_unit(kind, 0, start);
            let positions: Vec<Pos> = g.map.tiles.iter().map(|(pos, _)| *pos).collect();
            for pos in positions {
                if pos != start && pos != city && (pos.0 + pos.1 + pattern) % 5 == 0 {
                    g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("coast");
                }
            }
            g.players[0].techs.insert(crate::name!("shipbuilding"));
            reachable += compare_siege(&g, uid, city);
        }
    }
    assert!(
        reachable > 10,
        "land-route comparisons include reachable goals"
    );
}

#[test]
fn siege_water_goal_exception_and_non_land_routes_are_preserved() {
    let (mut g, uid, city) = open_game();
    let start = g.units[&uid].pos;
    let goal = g.nbrs(start).into_iter().find(|pos| *pos != city).unwrap();
    g.players[0].techs.insert(crate::name!("shipbuilding"));
    g.players[0].techs.insert(crate::name!("cartography"));
    g.map.tiles.get_mut(&goal).unwrap().terrain = crate::name!("coast");
    assert!(keeps_to_land(&g, uid));
    assert_eq!(old_siege_route_step(&g, 0, uid, goal, city), Some(goal));
    assert_eq!(siege_route_step(&g, 0, uid, goal, city), Some(goal));

    for kind in ["warrior", "galley"] {
        let (mut g, original, city) = open_game();
        let start = g.units[&original].pos;
        g.remove_unit(original);
        for tile in g.map.tiles.values_mut() {
            if tile.pos != city {
                tile.terrain = crate::name!("coast");
            }
        }
        g.players[0].techs.insert(crate::name!("shipbuilding"));
        g.players[0].techs.insert(crate::name!("cartography"));
        let uid = g.spawn_unit(kind, 0, start);
        assert!(!keeps_to_land(&g, uid));
        assert!(compare_siege(&g, uid, city) > 0);
    }
}

#[test]
fn siege_occupancy_city_ring_and_foreign_access_are_preserved() {
    let mut reachable = 0;
    for (kind, owner) in [
        ("builder", 0),
        ("warrior", 0),
        ("builder", 1),
        ("archer", 1),
        ("fighter", 1),
    ] {
        let (mut g, uid, city) = open_game();
        let start = g.units[&uid].pos;
        let occupied = g.nbrs(start).into_iter().find(|pos| *pos != city).unwrap();
        g.spawn_unit(kind, owner, occupied);
        reachable += compare_siege(&g, uid, city);
    }
    assert!(reachable > 0);
    for granted in [false, true] {
        let (mut g, uid, city) = open_game();
        g.map_script = crate::setup::MapScript::Continents;
        g.at_war.clear();
        g.players[1].civics.insert(crate::name!("early_empire"));
        g.players[1].open_borders_until.clear();
        if granted {
            g.players[1].open_borders_until.insert(0, u32::MAX);
        }
        assert!(!g.is_at_war(0, 1));
        assert_eq!(g.has_open_borders(0, 1), granted);
        let goal = g
            .nbrs(g.units[&uid].pos)
            .into_iter()
            .find(|pos| *pos != city)
            .unwrap();
        let cid = *g.cities.keys().next().unwrap();
        g.map.tiles.get_mut(&goal).unwrap().owner_city = Some(cid);
        let expected = old_siege_route_step(&g, 0, uid, goal, city);
        assert_eq!(expected.is_some(), granted);
        assert_eq!(siege_route_step(&g, 0, uid, goal, city), expected);
    }
}

#[test]
fn predicate_routes_keep_boundary_zoc_target_and_cache_rules() {
    let (mut g, uid, city) = open_game();
    g.map.topology = crate::world::Topology::Cylinder;
    let row = (1..g.map.height - 1)
        .find(|row| city != (0, *row) && city != (g.map.width - 1, *row))
        .unwrap();
    let start = (g.map.width - 1, row);
    let target = (0, row);
    g.units.get_mut(&uid).unwrap().pos = start;
    let empty = BTreeSet::new();
    assert_eq!(
        g.route_step_avoiding_tiles(uid, target, &empty),
        Some(target)
    );
    assert_eq!(
        g.route_step_avoiding_when(uid, target, |_| false),
        Some(target)
    );
    assert_eq!(g.route_step_avoiding_when(uid, start, |_| false), None);
    assert_eq!(
        g.route_step_avoiding_when(u32::MAX, target, |_| false),
        None
    );
    assert_eq!(
        g.route_step_avoiding_when(uid, target, |pos| pos == target),
        None
    );
    assert_eq!(
        g.route_step_avoiding_when(uid, (-100, -100), |_| false),
        None
    );

    let ordinary = g.route_step(uid, target, 0);
    assert!(ordinary.is_some());
    g.route_step_avoiding_when(uid, target, |pos| pos == target);
    assert_eq!(
        g.route_step(uid, target, 0),
        ordinary,
        "temporary exclusions must not populate the ordinary route cache"
    );
    g.units.get_mut(&uid).unwrap().zoc_stopped = true;
    assert_eq!(g.route_step_avoiding_tiles(uid, target, &empty), None);
    assert_eq!(g.route_step_avoiding_when(uid, target, |_| false), None);
}

fn old_siege_route_step(g: &Game, pid: usize, uid: u32, goal: Pos, city: Pos) -> Option<Pos> {
    let unit = g.units.get(&uid)?;
    let mut avoid: BTreeSet<Pos> = g
        .wdisk(city, 1)
        .into_iter()
        .filter(|pos| *pos != goal)
        .collect();
    avoid.extend(
        g.units
            .values()
            .filter(|other| {
                other.id != uid
                    && g.rules.units[other.kind].domain.as_deref() != Some("air")
                    && (other.owner != pid || g.rules.units[other.kind].class == "military")
            })
            .map(|other| other.pos),
    );
    avoid.remove(&unit.pos);
    // A land unit on land keeps to land on its way to a post. Embarking
    // spends its moves, and the next turn `disembark_step` lands it again
    // before the train moves it, so a wet first step is a turn lost and the
    // next one too. Live King civvis-20261004T100903Z (game 52): with our
    // own soldiers walling the land approach, four Trebuchets before Ray
    // shuttled between (31,27) and the water at (32,27), five and four tiles
    // out, every other turn from turn 109 to 145, and none fired; the siege
    // ran seventy turns and Ray never fell.
    if keeps_to_land(g, uid) {
        avoid.extend(
            g.map
                .tiles
                .iter()
                .filter(|(pos, tile)| **pos != goal && g.rules.is_water(tile))
                .map(|(pos, _)| *pos),
        );
    }
    g.route_step_avoiding_tiles(uid, goal, &avoid)
}
