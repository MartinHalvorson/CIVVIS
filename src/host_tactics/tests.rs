//! The switch refuses only what it holds, the census reads a turn as the
//! fit reads the host's, and an unset seat is the bare AI.

use super::*;
use crate::hex;

fn at(col: i32, row: i32) -> Pos {
    hex::offset_to_axial(col, row)
}

/// An arena (permanently at war, no economy) with both sides' units removed.
fn field() -> Game {
    let mut g = crate::doctrine::build(
        crate::doctrine::position("the_reserve").expect("fixture"),
        3,
    )
    .expect("buildable");
    for pid in 0..2 {
        for uid in g.player_unit_ids(pid) {
            g.remove_unit(uid);
        }
    }
    assert!(g.is_at_war(0, 1));
    g.current = 1;
    g
}

fn gate(player: usize, victims: Vec<usize>) -> Gate {
    Gate {
        player,
        victims,
        ..Gate::default()
    }
}

/// The free neighbour of `from` nearest to, or farthest from, `of`.
fn step_tile(g: &Game, from: Pos, of: Pos, closer: bool) -> Pos {
    let mut tiles: Vec<Pos> = g
        .nbrs(from)
        .into_iter()
        .filter(|p| g.unit_ids_at(*p).is_empty() && g.map.get(*p).is_some())
        .collect();
    tiles.sort_by_key(|p| (g.wdist(*p, of), *p));
    if closer {
        tiles[0]
    } else {
        *tiles.last().unwrap()
    }
}

#[test]
fn the_gate_refuses_a_held_blow_and_nothing_else() {
    let mut g = field();
    let ours = at(9, 6);
    g.spawn_unit("warrior", 0, ours);
    let theirs = g.spawn_unit("warrior", 1, at(10, 6));
    let mut held = gate(1, vec![0]);
    held.no_unit_strike.insert(theirs);
    g.host_gate = Some(Box::new(held));
    let blow = Action::Attack {
        unit: theirs,
        target: ours,
    };
    let refused = g.apply(1, &blow).unwrap_err();
    assert!(refused.contains("holds its blow"), "{refused}");
    // The same order from a seat the gate does not govern is not its business.
    assert!(gate_refuses(&mut g, 0, &blow).is_none());
    // With the gate gone the blow is the engine's to judge, and it lands.
    g.host_gate = None;
    assert!(g.apply(1, &blow).is_ok());
}

#[test]
fn a_governed_unit_is_moved_by_the_harness_not_its_ai() {
    let mut g = field();
    let ours = at(5, 6);
    g.spawn_unit("warrior", 0, ours);
    let held = g.spawn_unit("warrior", 1, at(9, 6));
    let free = g.spawn_unit("warrior", 1, at(9, 9));
    let mut rules = gate(1, vec![0]);
    rules.hold.insert(held);
    g.host_gate = Some(Box::new(rules));
    let walk = Action::Move {
        unit: held,
        to: step_tile(&g, at(9, 6), ours, true),
    };
    let refused = g.apply(1, &walk).unwrap_err();
    assert!(refused.contains("the harness's"), "{refused}");
    // A unit the gate does not hold walks as its AI says.
    let other = Action::Move {
        unit: free,
        to: step_tile(&g, at(9, 9), ours, false),
    };
    assert!(g.apply(1, &other).is_ok());
}

#[test]
fn the_harness_step_closes_and_backs_off() {
    let mut g = field();
    let ours = at(5, 6);
    g.spawn_unit("warrior", 0, ours);
    let walker = g.spawn_unit("warrior", 1, at(9, 6));
    let before = g.wdist(at(9, 6), ours);
    assert!(step(&mut g, 1, walker, &[ours], true));
    let after = g.wdist(g.units[&walker].pos, ours);
    assert!(after < before, "{before} -> {after}");
    let mut g2 = field();
    g2.spawn_unit("warrior", 0, ours);
    let backer = g2.spawn_unit("warrior", 1, at(9, 6));
    assert!(step(&mut g2, 1, backer, &[ours], false));
    assert!(g2.wdist(g2.units[&backer].pos, ours) > before);
}

#[test]
fn a_unit_refused_again_and_again_is_spent_so_a_retry_loop_ends() {
    let mut g = field();
    let ours = at(5, 6);
    g.spawn_unit("warrior", 0, ours);
    let held = g.spawn_unit("warrior", 1, at(9, 6));
    let mut rules = gate(1, vec![0]);
    rules.hold.insert(held);
    g.host_gate = Some(Box::new(rules));
    let walk = Action::Move {
        unit: held,
        to: step_tile(&g, at(9, 6), ours, true),
    };
    for _ in 0..VALVE {
        assert!(g.apply(1, &walk).is_err());
    }
    assert_eq!(g.units[&held].moves_left, 0.0);
    assert_eq!(g.units[&held].attacks_left, 0);
    let spent = g.host_gate.as_deref().unwrap();
    assert_eq!((spent.refusals, spent.valves), (VALVE, 1));
}

#[test]
fn the_census_reads_a_blow_from_the_action_log() {
    let mut g = field();
    let ours = at(9, 6);
    let victim = g.spawn_unit("warrior", 0, ours);
    let theirs = g.spawn_unit("warrior", 1, at(10, 6));
    let host = HostTactics {
        seed: 7,
        victims: vec![0],
        gate: false,
    };
    let watch = Watch::begin(&g, 1, &host);
    assert_eq!(watch.rows.len(), 1);
    assert_eq!(watch.rows[0].targets.len(), 1);
    assert!(
        watch.rows[0].targets[0].4,
        "adjacent: in reach without moving"
    );
    g.apply(
        1,
        &Action::Attack {
            unit: theirs,
            target: ours,
        },
    )
    .expect("the blow lands");
    let mut census = TacticsCensus::default();
    watch.finish(&g, 1, &mut census);
    let rate = |key: &str| census.rows.get(key).copied().unwrap_or_default();
    assert_eq!(rate("strike | in reach"), Tally { hits: 1.0, rows: 1 });
    assert_eq!(rate("strike | in reach without moving").rows, 1);
    assert_eq!(rate("strike | must move first").rows, 0);
    assert_eq!(
        rate("closes | struck nothing").rows,
        0,
        "a unit that struck is not a closing row"
    );
    let _ = victim;
}

#[test]
fn the_census_reads_a_unit_that_held_and_one_that_closed() {
    let mut g = field();
    let ours = at(5, 6);
    g.spawn_unit("warrior", 0, ours);
    // Both two tiles off, in the warrior's sight.
    let still = g.spawn_unit("warrior", 1, at(7, 6));
    let walker = g.spawn_unit("warrior", 1, at(5, 8));
    assert_eq!(g.wdist(at(7, 6), ours), 2);
    assert_eq!(g.wdist(at(5, 8), ours), 2);
    let host = HostTactics {
        seed: 7,
        victims: vec![0],
        gate: false,
    };
    let watch = Watch::begin(&g, 1, &host);
    assert_eq!(watch.rows.len(), 2, "both in sight");
    let to = step_tile(&g, at(5, 8), ours, true);
    g.apply(1, &Action::Move { unit: walker, to })
        .expect("a step");
    let mut census = TacticsCensus::default();
    watch.finish(&g, 1, &mut census);
    let closes = census.rows["closes | struck nothing"];
    assert_eq!(closes.rows, 2);
    assert_eq!(closes.hits, 1.0);
    let _ = still;
}

#[test]
fn a_governed_turn_strikes_or_holds_by_the_draw_and_sets_the_gate() {
    let mut g = field();
    let ours = at(9, 6);
    g.spawn_unit("warrior", 0, ours);
    let theirs = g.spawn_unit("warrior", 1, at(10, 6));
    let mut struck = 0;
    for seed in 0..40u64 {
        let mut world = g.clone();
        let host = HostTactics {
            seed,
            victims: vec![0],
            gate: true,
        };
        let mut counts = GateCounts::default();
        let mut share = ReachShare::default();
        govern(&mut world, 1, &host, &mut counts, &mut share);
        assert_eq!(counts.units, 1);
        let set = world.host_gate.as_deref().expect("a gate for the turn");
        assert!(set.no_unit_strike.contains(&theirs) && set.hold.contains(&theirs));
        if counts.strikes_drawn == 1 {
            assert_eq!(counts.strikes_landed + counts.strikes_failed, 1);
            struck += counts.strikes_landed;
        } else {
            assert!(
                world.units[&theirs].attacks_left > 0,
                "no blow drawn, none played"
            );
        }
    }
    // Adjacent, healthy, even: the fitted chance is near two in five.
    assert!((3..=30).contains(&struck), "{struck} of 40");
}

#[test]
fn draws_are_fixed_by_seed_turn_unit_and_purpose() {
    let a = draw(1, 2, 3, 4);
    assert_eq!(a, draw(1, 2, 3, 4));
    assert_ne!(a, draw(1, 2, 3, 5));
    assert_ne!(a, draw(2, 2, 3, 4));
    let mean = (0..2000).map(|k| draw(9, 1, k, 1)).sum::<f64>() / 2000.0;
    assert!((0.45..0.55).contains(&mean), "{mean}");
    assert!((0..2000).all(|k| (0.0..1.0).contains(&draw(9, 1, k, 1))));
}

#[test]
fn the_live_table_is_the_fit_tools_rates() {
    // `tools/fit_host_enemy.py rates`: a quarter of in-reach turns struck,
    // 43% without moving, 11% when it had to move first.
    let rate = |key: &str| {
        let (_, hits, rows, _, _) = LIVE.iter().find(|row| row.0 == key).unwrap();
        hits / *rows as f64
    };
    assert!((rate("strike | in reach") - 0.250).abs() < 0.001);
    assert!((rate("strike | in reach without moving") - 0.430).abs() < 0.001);
    assert!((rate("strike | must move first") - 0.110).abs() < 0.001);
    assert!((rate("target: nearest") - 0.879).abs() < 0.001);
}
