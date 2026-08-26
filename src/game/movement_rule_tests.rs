//! The stacking half of Civilization VI's movement rules, one test per rule.
//!
//! ★★★★★ THESE TESTS ARE THE RULE. Everything here was, until 2026-08-26,
//! decided by a single boolean named `can_enter` that answered two different
//! questions at once — may this unit step onto that hex, and may it be left
//! standing there — and answered both with "not while one of ours is on it".
//! The shipped game asks the second question only at the END of a move. The
//! whole engine consequently read a friendly unit as a wall: no column filed
//! through a defile, no front line rotated, and every route planned around its
//! own army. It was found by a person playing the Tactics arena.
//!
//! Nothing in the repository would have caught it. `tools/civ6_fidelity.py`
//! compares *data* against the shipped database and cannot see a behavioural
//! rule; `tools/live_divergence.py` is projection-only and replays no orders;
//! and every gene screen, ladder and arena bench plays both arms under the
//! same rules, so a rule error cancels out of all of them. A rule is pinned
//! here, against its published source, or it is not pinned at all.
//!
//! Reference basis: the in-game Civilopedia "Movement" entry,
//! <https://www.civilopedia.net/en-US/standard-rules/concepts/movement_3/>,
//! the shipped end-turn blocker `ENDTURN_BLOCKING_STACKED_UNITS` that this
//! repository's own Civilization VI bridge already handles (see
//! `docs/CIV6_COMPUTER_CONTROL.md`) and which exists precisely because units
//! may be stacked in the middle of a turn, and `docs/MOVEMENT.md`.

use super::*;

/// Flat plains, no rivers, no borders, no roads, nothing owned, two seats at
/// war. Every answer in this file is decided by the units placed into it.
fn plain_board(seed: u64) -> (Game, Pos, Vec<Pos>) {
    let mut g = Game::new_full(2, 20, 14, seed, 40, 0, false);
    let ids: Vec<u32> = g.units.keys().copied().collect();
    for id in ids {
        g.remove_unit(id);
    }
    for player in g.players.iter_mut() {
        player.civ = "Rome".to_string();
        player.unit_lifetimes.clear();
        player.government = None;
        player.policies.clear();
        player.techs.clear();
        player.civics.clear();
    }
    g.map.clear_rivers();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.owner_city = None;
        tile.hills = false;
        tile.road = 0;
    }
    let center = *g
        .map
        .tiles
        .keys()
        .find(|p| g.wdisk(**p, 2).len() == 19)
        .expect("the controlled map has an interior tile");
    let ring = g.nbrs(center).to_vec();
    assert_eq!(ring.len(), 6);
    g.current = 0;
    (g, center, ring)
}

/// A straight line of three tiles: where the mover stands, the tile in front
/// of it, and the tile beyond that. Derived from the board rather than
/// assumed, so a change of hex geometry fails loudly here instead of quietly
/// asserting nothing.
fn straight_line(g: &Game, start: Pos) -> (Pos, Pos) {
    for middle in g.nbrs(start) {
        for beyond in g.nbrs(middle) {
            if beyond != start && g.wdist(start, beyond) == 2 {
                return (middle, beyond);
            }
        }
    }
    panic!("the controlled map has a two-step line from the interior tile");
}

// ---------------------------------------------------------------- T1, T2, T3

/// **The rule.** A unit walks through a tile held by its own unit of the same
/// stacking layer and finishes beyond it; it may not finish on it.
#[test]
fn a_unit_passes_through_its_own_military_with_movement_to_spare() {
    let (mut g, start, _) = plain_board(6101);
    let (middle, beyond) = straight_line(&g, start);
    let mover = g.spawn_unit("warrior", 0, start);
    let friend = g.spawn_unit("warrior", 0, middle);
    g.begin_turn(0);
    assert_eq!(
        g.units[&mover].moves_left, 2.0,
        "a warrior on flat plains has the two movement points this rule needs"
    );

    let reach = g.reachable(mover);
    assert!(
        reach.contains(&beyond),
        "the tile past our own column is reachable: one plains step to cross, one to arrive"
    );
    assert!(
        !reach.contains(&middle),
        "and the tile our own unit stands on is not somewhere the mover may be left"
    );
    assert_eq!(
        g.path_to(mover, beyond),
        Some(vec![middle, beyond]),
        "the path crosses the friendly tile rather than detouring around it"
    );

    g.apply(
        0,
        &Action::MoveTo {
            unit: mover,
            to: beyond,
        },
    )
    .expect("a walk across our own column is a legal order");
    assert_eq!(g.units[&mover].pos, beyond);
    assert_eq!(
        g.units[&friend].pos, middle,
        "the unit crossed stays exactly where it stood"
    );
}

/// The half of the rule that has always been right, and must stay right.
#[test]
fn a_unit_cannot_end_on_its_own_unit() {
    let (mut g, start, _) = plain_board(6102);
    let (middle, _) = straight_line(&g, start);
    let mover = g.spawn_unit("warrior", 0, start);
    g.spawn_unit("warrior", 0, middle);
    g.begin_turn(0);

    assert!(!g.can_move(mover, middle));
    assert_eq!(
        g.apply(
            0,
            &Action::Move {
                unit: mover,
                to: middle
            }
        ),
        Err("invalid move".to_string())
    );
    assert_eq!(
        g.apply(
            0,
            &Action::MoveTo {
                unit: mover,
                to: middle
            }
        ),
        Err("unreachable".to_string())
    );
    assert_eq!(
        g.units[&mover].pos, start,
        "a refused order moves nothing at all"
    );
}

/// Passing through is not free: the unit needs the Movement to leave again,
/// and one movement point buys the crossing and nothing else.
#[test]
fn passing_through_needs_the_movement_to_get_out() {
    let (mut g, start, _) = plain_board(6103);
    let (middle, beyond) = straight_line(&g, start);
    let mover = g.spawn_unit("warrior", 0, start);
    g.spawn_unit("warrior", 0, middle);
    g.begin_turn(0);
    g.units.get_mut(&mover).expect("the mover").moves_left = 1.0;

    assert!(
        !g.reachable(mover).contains(&beyond),
        "one movement point pays for the crossing and leaves nothing to arrive with"
    );
    assert_eq!(g.path_to(mover, beyond), None);
    assert!(g
        .apply(
            0,
            &Action::MoveTo {
                unit: mover,
                to: beyond
            }
        )
        .is_err());
    assert_eq!(
        g.units[&mover].pos, start,
        "and the unit is left where it was, never halfway on top of its own"
    );
}

/// Civilization VI lets one untouched unit take a step it cannot afford. That
/// allowance is for arriving somewhere, and it never lands a unit on its own.
#[test]
fn the_first_free_step_never_lands_on_a_friend() {
    let (mut g, start, _) = plain_board(6104);
    let (middle, beyond) = straight_line(&g, start);
    let mover = g.spawn_unit("settler", 0, start);
    g.spawn_unit("settler", 0, middle);
    g.begin_turn(0);
    g.units.get_mut(&mover).expect("the mover").moves_left = g.unit_max_moves(mover).min(1.0);

    assert!(
        !g.reachable(mover).contains(&middle),
        "the free step is not a licence to stand on our own settler"
    );
    assert!(
        !g.reachable(mover).contains(&beyond),
        "nor to cross it with nothing left to arrive on"
    );
}

// -------------------------------------------------------------------- T5, T6

/// Zone of control ends movement on entry, so a friendly tile inside one is a
/// dead end and not a crossing: the unit arrives with nothing left to leave
/// with, on a tile it may not be left on.
#[test]
fn enemy_zone_of_control_on_a_friends_tile_is_a_dead_end() {
    let (mut g, start, _) = plain_board(6105);
    let (middle, beyond) = straight_line(&g, start);
    let mover = g.spawn_unit("warrior", 0, start);
    g.spawn_unit("warrior", 0, middle);
    // A hostile unit adjacent to the crossing, and not adjacent to the mover,
    // so the only zone of control in play is the one over `middle`.
    let watcher = g
        .nbrs(middle)
        .into_iter()
        .find(|pos| *pos != start && *pos != beyond && g.wdist(*pos, start) > 1)
        .expect("a tile adjacent to the crossing and not to the mover");
    g.at_war.insert(pair(0, 1));
    g.spawn_unit("warrior", 1, watcher);
    g.begin_turn(0);

    assert!(
        g.formation_enters_enemy_zoc(mover, middle),
        "the fixture puts the crossing inside a hostile zone of control"
    );
    assert!(
        !g.reachable(mover).contains(&beyond),
        "movement ends on entering the zone, so there is nothing left to leave our own tile with"
    );

    // The control, so this is a measurement and not an empty fixture: take the
    // hostile unit away and the identical crossing works.
    let hostile = g.unit_ids_at(watcher).to_vec();
    for id in hostile {
        g.remove_unit(id);
    }
    assert!(
        g.reachable(mover).contains(&beyond),
        "with nothing projecting a zone of control the same walk crosses our own unit"
    );
}

/// Only our own units may be crossed. Everybody else's block the step itself,
/// at peace and at war alike.
#[test]
fn foreign_units_still_block_the_step_itself() {
    let (mut g, start, _) = plain_board(6106);
    let (middle, beyond) = straight_line(&g, start);
    let mover = g.spawn_unit("warrior", 0, start);
    let foreign = g.spawn_unit("warrior", 1, middle);
    g.begin_turn(0);

    assert!(
        !g.can_pass(mover, start, middle),
        "a rival's unit is not somewhere we may walk through at peace"
    );
    assert!(!g.reachable(mover).contains(&beyond));

    g.at_war.insert(pair(0, 1));
    assert!(
        !g.can_pass(mover, start, middle),
        "nor at war — that tile is an attack, not a crossing"
    );
    assert!(!g.reachable(mover).contains(&beyond));

    // The capture case is untouched: a lone enemy civilian is taken by
    // entering its tile, which is an arrival and not a crossing.
    g.remove_unit(foreign);
    let civilian = g.spawn_unit("builder", 1, middle);
    assert!(g.can_move(mover, middle));
    g.apply(
        0,
        &Action::Move {
            unit: mover,
            to: middle,
        },
    )
    .expect("a military unit at war captures a lone civilian by entering");
    assert_eq!(g.units[&civilian].owner, 0);
}

// ------------------------------------------------------------------------ T7

/// The layers are the units' own. A civilian crosses a civilian and stops
/// beside it; a civilian and a military unit never contend at all.
#[test]
fn the_stacking_layers_decide_who_crosses_whom() {
    let (mut g, start, _) = plain_board(6107);
    let (middle, beyond) = straight_line(&g, start);
    let builder = g.spawn_unit("builder", 0, start);
    let other_builder = g.spawn_unit("builder", 0, middle);
    g.begin_turn(0);

    assert!(
        !g.can_stop(builder, middle),
        "two Builders contend for the civilian layer"
    );
    assert!(g.can_pass(builder, start, middle));

    // Replace the civilian in the way with a military unit: different layer,
    // so the tile is simply open.
    g.remove_unit(other_builder);
    g.spawn_unit("warrior", 0, middle);
    assert!(
        g.can_stop(builder, middle),
        "a Builder and a Warrior share a tile in the shipped game and here"
    );
    assert!(g.reachable(builder).contains(&middle));
    let _ = beyond;
}

// ------------------------------------------------------------------------ T9

/// ★★★ THE INVARIANT. However a walk ends — out of Movement, refused by a
/// linked escort, stopped by a zone of control — it never leaves two of one
/// player's units contending for one tile.
#[test]
fn a_walk_never_leaves_two_units_stacked() {
    for seed in [6201, 6202, 6203, 6204] {
        let (mut g, start, ring) = plain_board(seed);
        let mover = g.spawn_unit("warrior", 0, start);
        for (index, pos) in ring.iter().enumerate() {
            // A ring of our own units around the mover, so every direction it
            // could take is a crossing.
            if index % 2 == 0 {
                g.spawn_unit("warrior", 0, *pos);
            }
        }
        g.begin_turn(0);
        let targets: Vec<Pos> = g.wdisk(start, 3).into_iter().collect();
        for to in targets {
            let _ = g.apply(0, &Action::MoveTo { unit: mover, to });
            let at = g.units[&mover].pos;
            let same_layer = g
                .unit_ids_at(at)
                .iter()
                .filter(|id| **id != mover && g.units[id].owner == 0)
                .filter(|id| {
                    g.rules.units[g.units[id].kind].class
                        == g.rules.units[g.units[&mover].kind].class
                })
                .count();
            assert_eq!(
                same_layer, 0,
                "seed {seed}: a walk to {to:?} left the mover stacked at {at:?}"
            );
        }
    }
}

// ----------------------------------------------------------------------- T12

/// Two adjacent units of one player exchange tiles, and both pay for it.
#[test]
fn adjacent_friendly_units_swap_and_both_pay_the_step() {
    let (mut g, start, _) = plain_board(6301);
    let (middle, _) = straight_line(&g, start);
    let front = g.spawn_unit("warrior", 0, middle);
    let back = g.spawn_unit("warrior", 0, start);
    g.begin_turn(0);

    g.apply(
        0,
        &Action::Swap {
            unit: back,
            other: front,
        },
    )
    .expect("two adjacent warriors may exchange tiles");
    assert_eq!(g.units[&back].pos, middle);
    assert_eq!(g.units[&front].pos, start);
    assert_eq!(
        g.units[&back].moves_left, 1.0,
        "each half of a swap is a step and is charged like one"
    );
    assert_eq!(g.units[&front].moves_left, 1.0);
    assert!(!g.units[&front].fortified && !g.units[&back].fortified);
}

/// A swap is two moves, so a unit with nothing left cannot make one.
#[test]
fn a_swap_needs_movement_on_both_sides() {
    let (mut g, start, _) = plain_board(6302);
    let (middle, _) = straight_line(&g, start);
    let front = g.spawn_unit("warrior", 0, middle);
    let back = g.spawn_unit("warrior", 0, start);
    g.begin_turn(0);
    g.units.get_mut(&front).expect("the front unit").moves_left = 0.0;

    assert!(g
        .apply(
            0,
            &Action::Swap {
                unit: back,
                other: front
            }
        )
        .is_err());
    assert_eq!(g.units[&back].pos, start, "a refused swap moves nothing");
    assert_eq!(g.units[&front].pos, middle);
}

/// Units on different layers already share a tile, so there is nothing to
/// exchange and the swap is refused rather than silently doing a move.
#[test]
fn a_swap_across_stacking_layers_is_refused() {
    let (mut g, start, _) = plain_board(6303);
    let (middle, _) = straight_line(&g, start);
    let builder = g.spawn_unit("builder", 0, middle);
    let warrior = g.spawn_unit("warrior", 0, start);
    g.begin_turn(0);

    assert!(g
        .apply(
            0,
            &Action::Swap {
                unit: warrior,
                other: builder
            }
        )
        .is_err());
    assert_eq!(g.units[&warrior].pos, start);
    assert!(
        g.can_move(warrior, middle),
        "it did not need a swap: that tile was open to it all along"
    );
}

/// A swap belongs to one player. It is not a way to displace somebody else's
/// unit, at peace or at war.
#[test]
fn a_swap_never_touches_a_foreign_unit() {
    let (mut g, start, _) = plain_board(6304);
    let (middle, _) = straight_line(&g, start);
    let foreign = g.spawn_unit("warrior", 1, middle);
    let mine = g.spawn_unit("warrior", 0, start);
    g.at_war.insert(pair(0, 1));
    g.begin_turn(0);

    assert!(g
        .apply(
            0,
            &Action::Swap {
                unit: mine,
                other: foreign
            }
        )
        .is_err());
    assert_eq!(g.units[&mine].pos, start);
    assert_eq!(g.units[&foreign].pos, middle);
}

// ----------------------------------------------------------------------- T13

/// The reading a watcher is shown for somebody else's unit is unchanged by any
/// of this: it already passed units, and it still stops at zone of control.
#[test]
fn the_threat_reading_still_passes_everything() {
    let (mut g, start, _) = plain_board(6401);
    let (middle, beyond) = straight_line(&g, start);
    let mover = g.spawn_unit("warrior", 0, start);
    g.spawn_unit("warrior", 1, middle);
    g.begin_turn(0);

    assert!(
        g.threat_reach(mover).contains(&middle) && g.threat_reach(mover).contains(&beyond),
        "a threat envelope reads through whatever is parked in the way, ours or theirs"
    );
    assert!(
        !g.reachable(mover).contains(&beyond),
        "and it is a reading, never a permission: a rival's unit still blocks the march"
    );
}

// ----------------------------------------------------------------------- T11

/// ★★★★ THE REASON THIS MATTERS AT THE BOARD. A column in a one-tile defile
/// has no legal step at all under the old rule: each unit's only way forward
/// holds the unit in front of it, so the whole column stands still for the
/// rest of the game. Here the controller walks the rear unit through its own
/// column and out the far end, which is one order (`MoveTo`) and never one
/// step.
#[test]
fn the_controller_files_a_column_through_a_defile() {
    let (mut g, start, _) = plain_board(6501);
    // A one-tile-wide corridor: everything that is not the line is mountain.
    let mut line = vec![start];
    let mut cursor = start;
    let mut previous: Option<Pos> = None;
    for _ in 0..5 {
        let step = g
            .nbrs(cursor)
            .into_iter()
            .find(|pos| {
                Some(*pos) != previous
                    && !line.contains(pos)
                    && line.iter().all(|held| g.wdist(*held, *pos) >= 1)
                    && g.wdist(start, *pos) == line.len() as i32
            })
            .expect("the controlled map continues the corridor");
        previous = Some(cursor);
        cursor = step;
        line.push(step);
    }
    let corridor: std::collections::BTreeSet<Pos> = line.iter().copied().collect();
    for pos in g.wdisk(start, 6) {
        if !corridor.contains(&pos) {
            g.map.tiles.get_mut(&pos).expect("a tile").terrain = crate::name!("mountain");
        }
    }
    let target = *line.last().expect("the corridor has an end");

    // Three horsemen nose to tail, the rear one ordered forward. Four
    // movement points: enough to cross both and arrive, and not enough to
    // make the arrival free.
    let front = g.spawn_unit("horseman", 0, line[2]);
    let middle_unit = g.spawn_unit("horseman", 0, line[1]);
    let rear = g.spawn_unit("horseman", 0, line[0]);
    g.begin_turn(0);

    assert!(
        g.nbrs(line[0])
            .into_iter()
            .filter(|pos| corridor.contains(pos))
            .all(|pos| !g.can_move(rear, pos)),
        "the fixture is a real defile: the rear unit has no single legal step forward"
    );

    let ai = crate::ai::BasicAi::new();
    let moved = ai.step_toward_range(&mut g, 0, rear, target, 0);

    assert!(
        moved,
        "the rear unit advances through its own column instead of standing still"
    );
    assert_eq!(
        g.units[&rear].pos, line[4],
        "it crosses both units in front and spends the rest of its movement \
         advancing, exactly as a walk ordered to a distant tile does"
    );
    assert_eq!(
        g.units[&rear].moves_left, 0.0,
        "and it paid a movement point for every tile, crossings included"
    );
    assert_eq!(
        g.units[&middle_unit].pos, line[1],
        "the column does not shuffle"
    );
    assert_eq!(g.units[&front].pos, line[2]);
}
