use super::*;
use crate::game::Game;

/// No shipped step is cheaper than the route ladder's cheapest rung.
///
/// ⚠ THIS NO LONGER GUARDS THE ENVELOPE CACHE, AND USED TO SAY IT DID.
/// Until #2163 an envelope was keyed on a disk whose radius divided a
/// unit's movement by 0.25, so a sub-0.25 step would have let an enemy
/// reach past its own key and be served a stale envelope. The cache keys
/// on `envelope_sensitive_tiles` now — the unit's own flood, no radius
/// anywhere — so nothing is served stale if this drifts.
///
/// It is kept because the floor it pins is still a fact about the shipped
/// data that movement code elsewhere assumes: terrain defaults to 1 MP,
/// every feature that names a cost adds to it, and the only discounts are
/// the route ladder's, whose cheapest rung is a Railroad at 0.25. A test
/// that has outlived its original argument earns its place on a new one or
/// goes; this one has a new one, stated here rather than implied.
#[test]
fn no_shipped_step_costs_less_than_the_route_ladder_allows() {
    let rules = crate::rules::Rules::shipped();
    for (name, terrain) in rules.terrains.iter() {
        assert!(
            terrain.move_cost >= 1.0,
            "terrain {name} costs {} -- below the 1 MP an off-route step \
                 is assumed to cost",
            terrain.move_cost
        );
    }
    for (name, feature) in rules.features.iter() {
        assert!(
            feature.move_cost >= 0.0,
            "feature {name} refunds {} movement, which no movement rule \
                 in this engine is written to expect",
            feature.move_cost
        );
    }
}

/// The delta notices every unit field the board key hashes.
///
/// ★★★★★ THE TWO ARE ONE INVARIANT AND THEY DRIFTED. The board key decides
/// whether *any* envelope may be reused; the delta decides which
/// *individual* ones may be. The delta shipped tracking three of the five
/// unit fields the key hashes, so a unit forming a corps or being stopped
/// by zone of control without moving produced an empty delta and every
/// envelope was reused across it.
///
/// ⚠ This compares behaviour, not source: it mutates one field at a time
/// and asks whether each notices. A field added to the key and forgotten
/// here fails, which is the drift that happened.
#[test]
fn the_delta_tracks_every_field_the_board_key_hashes() {
    let base = Game::new_full(2, 24, 16, 7_007, 300, 0, false);
    let uid = base.player_unit_ids(0).into_iter().next().unwrap();
    type Edit = (&'static str, fn(&mut crate::game::Unit));
    let edits: Vec<Edit> = vec![
        ("formation", |u| u.formation = u.formation.wrapping_add(1)),
        ("zoc_stopped", |u| u.zoc_stopped = !u.zoc_stopped),
        ("owner", |u| u.owner = 1),
        ("kind", |u| u.kind = crate::name!("swordsman")),
        ("air_patrol_pos", |u| {
            u.air_patrol = true;
            u.air_patrol_pos = Some((0, 0));
        }),
    ];
    for (field, edit) in edits {
        let ai = BasicAi::new();
        let mut game = base.clone();
        assert!(
            ai.envelope_board_delta(&game).is_none(),
            "{field}: first ask"
        );
        assert_eq!(
            ai.envelope_board_delta(&game),
            Some(Vec::new()),
            "{field}: an unchanged board"
        );
        let key_before = BasicAi::attack_envelope_fingerprint(&game, None);
        edit(game.units.get_mut(&uid).unwrap());
        let key_after = BasicAi::attack_envelope_fingerprint(&game, None);
        let delta = ai.envelope_board_delta(&game);
        let noticed = delta.as_ref().is_none_or(|touched| !touched.is_empty());
        if key_before != key_after {
            assert!(
                noticed,
                "the board key hashes {field} and the delta reported nothing, \
                     so every envelope would be reused across a change the key \
                     itself calls significant"
            );
        }
    }
}

/// A unit that cannot move is still sensitive to its own ground.
///
/// ★★★★★ THIS IS THE DEFECT THAT REFUSED THE WHOLE OPTIMISATION TWICE.
/// `flow_past` returns nothing at all when
/// `formation_movement_locked_by_zoc` holds, so a sensitivity set built
/// only from the flood was **empty** — and an empty set is touched by no
/// board change ever, so the envelope froze at empty for the rest of the
/// game, long after the lock lifted. `tools/speed_ab.py` refused the change
/// on the same two seeds every time and two rounds of inspection missed it;
/// a reuse audit found it in one run, every report showing tiles *gained*
/// and none lost around a unit whose cached envelope was empty.
///
/// The lock is read off the unit's own ground, so that ground is always
/// sensitive, flood or no flood.
#[test]
fn a_unit_that_cannot_move_is_still_sensitive_to_its_own_ground() {
    let mut game = Game::new_full(2, 24, 16, 9_090, 300, 0, false);
    let home = game.units[&game.player_unit_ids(0).into_iter().next().unwrap()].pos;
    let uid = game.spawn_test_unit("warrior", 1, home);
    let unit = game.units[&uid].clone();

    let locked = BasicAi::envelope_sensitive_tiles(&game, &unit, &[]);
    assert!(
        !locked.is_empty(),
        "an empty flood must not give an empty sensitivity: nothing would \
             ever invalidate the envelope again"
    );
    assert!(
        locked.contains(&home),
        "its own tile decides whether it is locked"
    );
    for neighbour in game.nbrs(home) {
        assert!(
            locked.contains(&neighbour),
            "{neighbour:?} is where the zone of control that locks it comes from"
        );
    }
}

/// The board delta names every tile a change touched, and gives up
/// entirely on the changes it does not track.
///
/// ★★★★★ TESTED HERE BECAUSE IT CANNOT BE TESTED ABOVE. The reuse gate is
/// only sound if the delta is complete, and two of its halves are
/// unreachable from a whole-board fixture: a unit stepping *out* of an
/// enemy's radius (its new tile is outside, so only the tile it left says
/// anything happened), and a map, war or city change. Planted defects in
/// both survived `a_warm_envelope_cache_answers_what_a_cold_one_computes`,
/// because with shipped units an own unit's move never changes an enemy's
/// envelope at all — see that test's own note. So the delta is pinned
/// directly.
#[test]
fn the_board_delta_reports_every_tile_a_change_touched() {
    let mut game = Game::new_full(2, 24, 16, 5_150, 300, 0, false);
    let ai = BasicAi::new();
    assert!(
        ai.envelope_board_delta(&game).is_none(),
        "a first ask has no previous board and must assume everything changed"
    );
    assert_eq!(
        ai.envelope_board_delta(&game),
        Some(Vec::new()),
        "an unchanged board touches no tile"
    );

    let uid = game.player_unit_ids(0).into_iter().next().unwrap();
    let from = game.units[&uid].pos;
    let to = game.nbrs(from).into_iter().next().unwrap();
    game.units.get_mut(&uid).unwrap().pos = to;
    let moved = ai
        .envelope_board_delta(&game)
        .expect("a move is not wholesale");
    assert!(
        moved.contains(&from),
        "the tile a unit LEFT must be reported: its zone of control was there \
             and is not any more, and the tile it arrived on may be outside the \
             radius that cared"
    );
    assert!(
        moved.contains(&to),
        "the tile a unit arrived on must be reported"
    );

    let spawned = game.spawn_test_unit("warrior", 0, from);
    let born = ai
        .envelope_board_delta(&game)
        .expect("a spawn is not wholesale");
    assert!(born.contains(&from), "a new unit's tile must be reported");

    game.units.remove(&spawned);
    let died = ai
        .envelope_board_delta(&game)
        .expect("a death is not wholesale");
    assert!(
        died.contains(&from),
        "a removed unit's last tile must be reported"
    );

    // Acting where it stands costs a unit its entire flood, via
    // `formation_movement_locked_by_zoc`, and moves none of the fields the
    // board key hashes — so only this notices.
    {
        let stander = game.units.get_mut(&uid).unwrap();
        stander.started_turn_in_zoc = true;
        stander.acted = true;
        stander.moved = false;
    }
    let acted = ai
        .envelope_board_delta(&game)
        .expect("acting in place is not wholesale");
    assert!(
        acted.contains(&game.units[&uid].pos),
        "a unit that acted where it stands loses its whole flood; the tile it \
             stands on must be reported or its envelope is never recomputed"
    );

    // A patrol is the one thing an envelope notices that the *board key*
    // does not hash, so `the_delta_tracks_every_field_the_board_key_hashes`
    // cannot reach it. `can_enter_past` refuses a step onto a tile a
    // hostile fighter is patrolling and finds that fighter by scanning
    // every unit in the world, so starting one changes envelopes without
    // the unit moving at all.
    let patrolled = game.nbrs(from).into_iter().next().unwrap();
    {
        let flier = game.units.get_mut(&uid).unwrap();
        flier.air_patrol = true;
        flier.air_patrol_pos = Some(patrolled);
    }
    let flying = ai
        .envelope_board_delta(&game)
        .expect("starting a patrol is not wholesale");
    assert!(
        flying.contains(&patrolled),
        "the tile a fighter began patrolling must be reported: a step onto it \
             is now refused, and the fighter itself may be nowhere near it"
    );

    // The global changes the delta does not track individually. (The war
    // ledger is the third and takes the same path -- `stamp` carries
    // `wars.len()` -- but a fixture cannot open one without the engine's
    // private declaration path, so the two that can be reached stand for
    // it.)
    let city = game.found_city_for(0, from, None);
    assert!(
        ai.envelope_board_delta(&game).is_none(),
        "a new city must fall back to recomputing everything"
    );
    game.cities.get_mut(&city).unwrap().owner = 1;
    assert!(
        ai.envelope_board_delta(&game).is_none(),
        "a city changing hands must fall back to recomputing everything"
    );
    game.map.tiles.get_mut(&from).unwrap().road = 5;
    assert!(
        ai.envelope_board_delta(&game).is_none(),
        "a map edit must fall back to recomputing everything"
    );
}

/// The merge diff reports exactly what a whole-board diff reports.
///
/// ★★★★ THE DIFF IS THE ONE THING THAT MAY NOT GET CHEAPER BY GETTING
/// WEAKER. `envelope_board_change` walks the two rosters side by side
/// because both are ordered by id, instead of building a `HashMap` of
/// every unit in the game per ask and looking each id up in the last one.
/// That is only a saving if it still names every tile the table it
/// replaced named, so the table it replaced is written out here and the
/// two are compared as sets — with all four kinds of change in flight at
/// once, which is where a merge with a mis-stepped cursor goes wrong and a
/// one-change-at-a-time test does not look.
#[test]
fn the_merge_diff_reports_what_a_whole_board_diff_would() {
    let whole_board_diff = |before: &HashMap<u32, EnvelopeUnit>, g: &Game| -> Vec<Pos> {
        let mut current: HashMap<u32, EnvelopeUnit> = HashMap::new();
        for unit in g.units.values() {
            current.insert(unit.id, EnvelopeUnit::of(unit));
        }
        let mut touched = Vec::new();
        for (id, now) in &current {
            match before.get(id) {
                Some(was) if was == now => {}
                Some(was) => {
                    touched.push(was.pos);
                    touched.push(now.pos);
                    touched.extend(was.patrol);
                    touched.extend(now.patrol);
                }
                None => {
                    touched.push(now.pos);
                    touched.extend(now.patrol);
                }
            }
        }
        for (id, was) in before {
            if !current.contains_key(id) {
                touched.push(was.pos);
                touched.extend(was.patrol);
            }
        }
        touched.sort_unstable();
        touched.dedup();
        touched
    };

    let mut game = Game::new_full(2, 24, 16, 4_242, 300, 0, false);
    let ai = BasicAi::new();
    let mine = game.player_unit_ids(0).into_iter().next().unwrap();
    let home = game.units[&mine].pos;
    let doomed = game.spawn_test_unit("warrior", 0, home);
    let flier = game.spawn_test_unit("warrior", 1, home);
    assert!(ai.envelope_board_delta(&game).is_none(), "the first ask");
    assert_eq!(
        ai.envelope_board_delta(&game),
        Some(Vec::new()),
        "an unchanged board"
    );

    // The board the delta will be taken against, kept the old way.
    let mut before: HashMap<u32, EnvelopeUnit> = HashMap::new();
    for unit in game.units.values() {
        before.insert(unit.id, EnvelopeUnit::of(unit));
    }

    // Every kind of change the diff distinguishes, at once: a unit that
    // moved, one that was born, one that died, and one that changed only
    // the tile it is patrolling. The born and the dead sit on ids either
    // side of the moved one so the merge cursor has to step over both.
    let step = game.nbrs(home).into_iter().next().unwrap();
    let patrolled = game.nbrs(home).into_iter().next_back().unwrap();
    game.units.get_mut(&mine).unwrap().pos = step;
    game.units.remove(&doomed);
    {
        let flier = game.units.get_mut(&flier).unwrap();
        flier.air_patrol = true;
        flier.air_patrol_pos = Some(patrolled);
    }
    let born = game.spawn_test_unit("warrior", 0, home);
    assert!(born > flier, "the new id must sort after the changed ones");

    let mut merged = ai
        .envelope_board_delta(&game)
        .expect("four unit changes are not wholesale");
    merged.sort_unstable();
    merged.dedup();
    assert_eq!(
        merged,
        whole_board_diff(&before, &game),
        "the merge reported a different set of tiles than a whole-board diff"
    );
    assert!(
        merged.contains(&home) && merged.contains(&step) && merged.contains(&patrolled),
        "non-vacuity: the tile left, the tile arrived on and the patrolled \
             tile must all be in there, or the comparison above is between two \
             empty answers"
    );
}

/// A unit that leaves the board still clears its entry from the per-enemy
/// store, now that the sweep runs only when something left.
///
/// ★★★ THE SWEEP IS NOT MERELY HYGIENE. Two branches of one game draw
/// their new unit ids from the same counter, so a speculative board and
/// the real one can mint the same id for different units — and an entry
/// left behind by a board that no longer exists would then be read as
/// belonging to a unit it never described. `compute_enemy_attack_envelopes`
/// skips the sweep only when the delta says nothing left the board; this
/// holds it to that.
#[test]
fn a_removal_still_clears_the_per_enemy_store() {
    let mut game = Game::new_full(2, 32, 22, 8_181, 300, 0, false);
    game.at_war.insert((0, 1));
    game.at_war.insert((1, 0));
    let mine = game.player_unit_ids(0).into_iter().next().unwrap();
    let home = game.units[&mine].pos;
    let beside = game
        .nbrs(home)
        .into_iter()
        .find(|pos| {
            game.map.get(*pos).is_some_and(|t| !game.rules.is_water(t))
                && game.unit_ids_at(*pos).is_empty()
        })
        .expect("the fixture offers dry ground beside the capital");
    let enemy = game.spawn_test_unit("warrior", 1, beside);

    let ai = BasicAi::new();
    let _ = ai.enemy_attack_envelopes(&game, 0);
    assert!(
        ai.enemy_envelope_cache.lock().unwrap().contains_key(&enemy),
        "the fixture must put the enemy in the store, or the removal below \
             is asserted against an empty cache"
    );

    game.units.remove(&enemy);
    let _ = ai.enemy_attack_envelopes(&game, 0);
    assert!(
        !ai.enemy_envelope_cache.lock().unwrap().contains_key(&enemy),
        "a unit left the board and its envelope stayed in the store"
    );
}

/// A warm per-enemy cache answers exactly what a cold one computes.
#[test]
fn a_warm_envelope_cache_answers_what_a_cold_one_computes() {
    let mut game = Game::new_full(2, 32, 22, 8_181, 300, 0, false);
    game.at_war.insert((0, 1));
    game.at_war.insert((1, 0));
    let mine = game.player_unit_ids(0).into_iter().next().unwrap();
    let home = game.units[&mine].pos;
    let dry = |g: &Game, pos: Pos| {
        g.map.get(pos).is_some_and(|t| !g.rules.is_water(t)) && g.unit_ids_at(pos).is_empty()
    };
    let far = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| game.wdist(*pos, home) > 12 && dry(&game, *pos))
        .min_by_key(|pos| (game.wdist(*pos, home), *pos))
        .expect("the fixture offers distant open land");
    let enemy = game.spawn_test_unit("warrior", 1, far);
    let scout = game.spawn_test_unit("warrior", 0, home);
    let cold = |g: &Game| BasicAi::new().enemy_attack_envelopes(g, 0).to_vec();
    let baseline = cold(&game);
    assert!(
        !baseline.is_empty(),
        "the fixture must offer at least one visible enemy envelope, or every \
             comparison below is vacuously equal"
    );

    let warm = BasicAi::new();
    let _ = warm.enemy_attack_envelopes(&game, 0);
    type Step = Box<dyn Fn(&mut Game)>;
    let steps: Vec<Step> = vec![
        Box::new(move |g: &mut Game| {
            let to = g.nbrs(g.units[&scout].pos).into_iter().next().unwrap();
            g.units.get_mut(&scout).unwrap().pos = to;
        }),
        Box::new(move |g: &mut Game| {
            let beside = g.nbrs(g.units[&enemy].pos).into_iter().next().unwrap();
            g.units.get_mut(&scout).unwrap().pos = beside;
        }),
        // The enemy itself moving is what must always invalidate.
        Box::new(move |g: &mut Game| {
            let to = g.nbrs(g.units[&enemy].pos).into_iter().next_back().unwrap();
            g.units.get_mut(&enemy).unwrap().pos = to;
        }),
        Box::new(move |g: &mut Game| {
            g.units.get_mut(&enemy).unwrap().kind = crate::name!("horseman");
        }),
    ];
    let mut seen = vec![baseline];
    for (index, step) in steps.iter().enumerate() {
        step(&mut game);
        let expected = cold(&game);
        let actual = warm.enemy_attack_envelopes(&game, 0).to_vec();
        assert_eq!(actual, expected, "step {index}: the warm cache went stale");
        seen.push(expected);
    }
    // Non-vacuity, and pointed rather than general: the enemy's own move is
    // step 2, and it *must* move the answer. A key that leaves the enemy's
    // own place out would serve a stale envelope exactly here, and a
    // blanket "some step differed" assertion does not notice.
    assert_ne!(
        seen[2], seen[3],
        "the enemy moved and its envelope did not, so the comparison above \
             cannot detect a stale one"
    );
}

/// The expensive raw flood belongs to the board, not to one controller.
///
/// A fresh `BasicAi` must reuse it, and a speculative `Game` clone must
/// reuse it too. The clone check is important: tactical search makes many
/// clones, which was the source of the repeated flood work this cache is
/// meant to remove.
#[test]
fn raw_enemy_reach_is_shared_by_controllers_and_game_clones() {
    let mut game = Game::new_full(2, 32, 22, 8_181, 300, 0, false);
    game.at_war.insert((0, 1));
    game.at_war.insert((1, 0));
    let mine = game.player_unit_ids(0).into_iter().next().unwrap();
    let home = game.units[&mine].pos;
    let dry = |g: &Game, pos: Pos| {
        g.map.get(pos).is_some_and(|tile| !g.rules.is_water(tile)) && g.units_at(pos).is_empty()
    };
    let far = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| game.wdist(*pos, home) > 12 && dry(&game, *pos))
        .min_by_key(|pos| (game.wdist(*pos, home), *pos))
        .expect("the fixture offers distant open land");
    let enemy = game.spawn_test_unit("warrior", 1, far);

    let expected = BasicAi::new().enemy_attack_envelopes(&game, 0).to_vec();
    assert!(
        !expected.is_empty(),
        "the fixture must cause at least one raw enemy reach to be cached"
    );
    let envelope = expected
        .iter()
        .find(|(uid, _)| *uid == enemy)
        .map(|(_, reach)| reach)
        .expect("the visible spawned enemy must have an envelope");
    let raw = game.cached_attack_reach_from_flood(
        (game.turn, BasicAi::attack_envelope_fingerprint(&game, None)),
        enemy,
    );
    assert_eq!(
        envelope.as_slice().as_ptr(),
        raw.shared_targets().as_ptr(),
        "an envelope must retain the raw cache's target allocation instead of copying it"
    );
    let after_first_controller = game.attack_reach_cache_computations();
    assert!(after_first_controller > 0);

    let from_second_controller = BasicAi::new().enemy_attack_envelopes(&game, 0).to_vec();
    assert_eq!(from_second_controller, expected);
    assert_eq!(
        game.attack_reach_cache_computations(),
        after_first_controller,
        "a new controller on the same board must reuse the raw flood"
    );

    let clone = game.clone();
    let from_clone = BasicAi::new().enemy_attack_envelopes(&clone, 0).to_vec();
    assert_eq!(from_clone, expected);
    assert_eq!(
        clone.attack_reach_cache_computations(),
        after_first_controller,
        "a speculative clone must share the parent's raw flood"
    );

    let mut changed_clone = clone;
    let to = changed_clone
        .nbrs(changed_clone.units[&enemy].pos)
        .into_iter()
        .next_back()
        .unwrap();
    changed_clone.units.get_mut(&enemy).unwrap().pos = to;
    let _ = BasicAi::new().enemy_attack_envelopes(&changed_clone, 0);
    assert!(
        changed_clone.attack_reach_cache_computations() > after_first_controller,
        "a changed speculative board must discard the parent snapshot"
    );
}

/// ★★★★ THE ORDER IS THE PART THAT COULD CHANGE A GAME. An envelope is
/// unioned into the whole-map safety scan and `evacuation_tile_cmp` breaks
/// its last tie on `Pos`, so a sorted `Vec` replacing a `BTreeSet` is only
/// safe while it iterates in the same sequence — including when the tiles
/// arrive unordered and with duplicates, which is exactly what an air
/// unit's disk hands over.
#[test]
fn an_envelope_iterates_in_the_order_the_tree_did() {
    let messy = vec![(3, -1), (0, 0), (3, -1), (-2, 7), (0, 0), (1, 1)];
    let tree: BTreeSet<Pos> = messy.iter().copied().collect();
    let reach = EnvelopeReach::from_tiles(messy.clone());
    assert_eq!(
        reach.iter().copied().collect::<Vec<Pos>>(),
        tree.iter().copied().collect::<Vec<Pos>>(),
        "the slice must hand back the tree's ascending, distinct sequence"
    );
    for position in &messy {
        assert!(reach.contains(position), "{position:?} is in the envelope");
    }
    assert!(!reach.contains(&(9, 9)), "and nothing else is");
    // Already ascending and distinct: the common path, adopted untouched.
    let sorted: Vec<Pos> = tree.iter().copied().collect();
    assert_eq!(
        EnvelopeReach::from_tiles(sorted.clone())
            .iter()
            .copied()
            .collect::<Vec<Pos>>(),
        sorted
    );
}

/// ★★★★ THE SKIP IS ONLY LEGAL BECAUSE THE ANSWER WAS ALREADY ZERO.
/// `retreat_step` now returns without pricing its own tile, and
/// `incoming_damage` returns without pricing the defender, whenever
/// `anything_can_reach` rejects the tile. Both rest on one implication,
/// and an implication argued in a comment is a guess with good posture: if
/// nothing can reach the tile, the incoming damage there is exactly `0.0`,
/// so the decision the caller skipped could only have been the one it
/// takes.
#[test]
fn a_tile_nothing_can_reach_prices_at_exactly_zero() {
    let mut game = Game::new_full(2, 32, 22, 8_181, 300, 0, false);
    game.at_war.insert((0, 1));
    game.at_war.insert((1, 0));
    let mine = game.player_unit_ids(0).into_iter().next().unwrap();
    let home = game.units[&mine].pos;
    let beside = game
        .nbrs(home)
        .into_iter()
        .find(|pos| {
            game.map
                .get(*pos)
                .is_some_and(|tile| !game.rules.is_water(tile))
                && game.unit_ids_at(*pos).is_empty()
        })
        .expect("the fixture offers dry ground beside the starting unit");
    game.spawn_test_unit("warrior", 1, beside);

    let ai = BasicAi::new();
    let envelopes = ai.enemy_attack_envelopes(&game, 0);
    assert!(
        !envelopes.is_empty(),
        "without a visible hostile envelope every tile is trivially quiet"
    );
    let (mut quiet, mut loud) = (0usize, 0usize);
    for position in game.map.tiles.keys().copied() {
        if !BasicAi::anything_can_reach(&game, 0, position, &envelopes) {
            quiet += 1;
            assert_eq!(
                BasicAi::evacuation_incoming_damage(&game, 0, mine, position, &envelopes),
                0.0,
                "{position:?} was accepted as out of reach and is not"
            );
        } else {
            loud += 1;
        }
    }
    assert!(
        quiet > 0,
        "the skip never fires, so it is measuring nothing"
    );
    assert!(
        loud > 0,
        "every tile is quiet, so the assertion above is vacuous"
    );
}

/// The envelope key moves for exactly the board changes `attack_reach`
/// can see, and for no others.
///
/// ★★★★★ THIS IS THE WHOLE PERFORMANCE CLAIM, AS A TEST. `attack_reach`
/// is a *next-turn* envelope: it flows from `unit_max_moves`, never
/// `moves_left`, and `flow_past` relaxes the stacking layer so other units
/// reach it only through `in_enemy_zoc_for` — owner, kind, religion and
/// promotions, never health, movement or attacks. Hashing health, movement
/// and attacks anyway made every attack, every fortify and every spent
/// movement point a cache miss that recomputed one flow field per visible
/// enemy, and a `sample` of head put `enemy_attack_envelopes` at 74.9% of
/// the main thread.
///
/// ⚠ The negative half is the one that decays. If a future reach rule
/// starts reading a unit's health or remaining movement, the key must gain
/// it back and this test is where that argument has to be had.
#[test]
fn the_envelope_key_ignores_state_no_reach_rule_reads() {
    let mut game = Game::new_full(2, 24, 16, 4_242, 300, 0, false);
    let position = game.player_unit_ids(0).into_iter().next().unwrap();
    let position = game.units[&position].pos;
    let uid = game.spawn_test_unit("warrior", 0, position);
    let key = |g: &Game| BasicAi::attack_envelope_fingerprint(g, None);
    let before = key(&game);

    for (field, apply) in [
        (
            "hp",
            (|u: &mut crate::game::Unit| u.hp -= 7) as fn(&mut crate::game::Unit),
        ),
        ("moves_left", |u: &mut crate::game::Unit| u.moves_left = 0.0),
        ("attacks_left", |u: &mut crate::game::Unit| {
            u.attacks_left = 0
        }),
        ("fortified", |u: &mut crate::game::Unit| {
            u.fortified = !u.fortified
        }),
    ] {
        let mut board = game.clone();
        apply(board.units.get_mut(&uid).unwrap());
        assert_eq!(
            key(&board),
            before,
            "{field} does not change any enemy's next-turn reach, so it must \
                 not invalidate the envelope cache"
        );
    }

    // And the positive half: what reach does read still moves the key.
    let mut moved = game.clone();
    let step = moved.nbrs(position).into_iter().next().unwrap();
    moved.units.get_mut(&uid).unwrap().pos = step;
    assert_ne!(key(&moved), before, "a unit's tile is its zone of control");

    let mut upgraded = game.clone();
    upgraded.units.get_mut(&uid).unwrap().kind = crate::name!("swordsman");
    assert_ne!(
        key(&upgraded),
        before,
        "the spec decides whether a unit exerts zone of control at all"
    );
}
