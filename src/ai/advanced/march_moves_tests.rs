use super::*;

/// A flat field at war: our city at (4, 12), the rival's at (20, 12), and one
/// of our `kind` standing `distance` tiles west of the rival's city with a
/// full turn of movement.
fn fixture(kind: &str, distance: i32) -> (Game, AdvancedAi, StrategicPlan, u32, ForceGroup) {
    let mut g = Game::new_full(2, 52, 30, 374_400, 250, 0, false);
    g.clear_mirror_cities();
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    g.found_city_for(0, (4, 12), None);
    let target = g.found_city_for(1, (20, 12), None);
    g.at_war.insert((0, 1));
    g.current = 0;
    let uid = g.spawn_test_unit(kind, 0, (20 - distance, 12));
    let full = g.unit_max_moves(uid);
    g.units.get_mut(&uid).unwrap().moves_left = full;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_war_reinforcement();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    };
    let group = ForceGroup {
        id: 1,
        domain: ForceDomain::Land,
        units: vec![uid],
        anchor: (4, 12),
        objective: (20, 12),
        focus_target: None,
        posture: ForcePosture::Muster,
        readiness: 0.0,
        local_strength_ratio: 0.5,
    };
    (g, ai, plan, uid, group)
}

/// Off, one decision takes the router's single step and leaves the rest of
/// the unit's movement for a later decision; on, the one decision walks as
/// far toward the ring as the movement reaches.
#[test]
fn a_reinforcement_walks_as_far_as_its_movement_reaches_under_the_gene() {
    let walk = |on: bool| {
        let (mut g, mut ai, plan, uid, group) = fixture("horseman", 14);
        if on {
            ai.enable_march_uses_its_moves();
        }
        let full = g.units[&uid].moves_left;
        assert!(
            full >= 3.0,
            "the fixture's horseman moves at least three tiles"
        );
        let before = g.wdist(g.units[&uid].pos, group.objective);
        assert_eq!(
            ai.wartime_reinforcement_step(&mut g, 0, uid, &plan, Some(&group), &[1]),
            Some(true)
        );
        let after = g.wdist(g.units[&uid].pos, group.objective);
        (before - after, g.units[&uid].moves_left, full)
    };
    let (off_gain, off_left, full) = walk(false);
    assert_eq!(off_gain, 1, "off, the router's single step");
    assert!(off_left > 0.0, "off, movement is left for another decision");
    let (on_gain, on_left, _) = walk(true);
    assert_eq!(
        on_gain as f64, full,
        "on, a tile for every movement point on open ground"
    );
    assert!(on_left <= 0.0, "on, the walk spends the turn's movement");
}

/// The walk stops at the ring: a unit whose movement would carry it past
/// the staging ring ends on it, never inside it.
#[test]
fn the_walk_ends_on_the_staging_ring_not_inside_it() {
    let (mut g, mut ai, plan, uid, group) = fixture("horseman", 7);
    ai.enable_march_uses_its_moves();
    assert_eq!(
        ai.wartime_reinforcement_step(&mut g, 0, uid, &plan, Some(&group), &[1]),
        Some(true)
    );
    let after = g.wdist(g.units[&uid].pos, group.objective);
    assert_eq!(after, 5, "the ring's outer edge, the nearest goal tile");
    assert!(
        g.units[&uid].moves_left > 0.0,
        "the most movement left breaks the tie"
    );
}

/// The reach list reads the router's own field: nearest the goals first,
/// then the most movement left.
#[test]
fn the_reach_list_is_nearest_first_then_most_movement_left() {
    let (g, _, _, uid, _) = fixture("warrior", 10);
    let goals: HashSet<Pos> = g
        .wdisk((20, 12), 5)
        .into_iter()
        .filter(|pos| (3..=5).contains(&g.wdist(*pos, (20, 12))))
        .collect();
    let (start, reach) = g
        .march_reach_toward_any(uid, &goals)
        .expect("a route to the ring");
    assert_eq!(start, 5);
    let full = g.units[&uid].moves_left;
    let (pos, steps, left) = reach[0];
    assert_eq!(steps, start - full as i32);
    assert!(left <= 0.0);
    assert_eq!(g.wdist(pos, g.units[&uid].pos), full as i32);
    assert!(reach
        .windows(2)
        .all(|pair| pair[0].1 < pair[1].1 || (pair[0].1 == pair[1].1 && pair[0].2 >= pair[1].2)));
}
