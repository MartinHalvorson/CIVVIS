//! Exact rival-yield reuse across moves and the writes that must reject it.
use super::*;

fn fixture() -> (Game, u32, u32, u32, Pos) {
    let mut game = Game::new_full(2, 24, 16, 91_171, 100, 0, false);
    for pid in 0..2 {
        while game.current != pid {
            let current = game.current;
            game.apply(current, &Action::EndTurn).unwrap();
        }
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|id| game.units[id].kind == "settler")
            .unwrap();
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    while game.current != 0 {
        let current = game.current;
        game.apply(current, &Action::EndTurn).unwrap();
    }
    let ground: BTreeSet<Pos> = game.map.tiles.keys().copied().collect();
    let seats = game.players.len();
    for pid in 0..seats {
        game.players[pid].explored = ground.clone();
        game.players[pid].met = (0..seats).filter(|other| *other != pid).collect();
    }
    let ours = game.player_city_ids(0)[0];
    let theirs = game.player_city_ids(1)[0];
    let (unit, to) = game
        .player_unit_ids(0)
        .into_iter()
        .filter(|id| game.rules.units[game.units[id].kind].class == "military")
        .find_map(|unit| {
            game.nbrs(game.units[&unit].pos).into_iter().find_map(|to| {
                (game.unit_ids_at(to).is_empty()
                    && game.city_at(to).is_none()
                    && game.map.tiles[&to].district.is_none()
                    && game.can_move(unit, to))
                .then_some((unit, to))
            })
        })
        .expect("opening military unit has an uncontested adjacent move");
    game.keep_rival_yields(true);
    game.keep_own_yields(true);
    game.audit_rival_yields(true);
    (game, ours, theirs, unit, to)
}

fn bits(y: Yields) -> [u64; 6] {
    [y.food, y.production, y.gold, y.science, y.culture, y.faith].map(f64::to_bits)
}

#[test]
fn quiet_step_reuses_only_rivals_and_preserves_the_complete_state() {
    let (mut game, ours, theirs, unit, to) = fixture();
    let mut fresh = game.clone();
    fresh.keep_rival_yields(false);
    game.with_deferred_visibility(|game| {
        fresh.with_deferred_visibility(|fresh| {
            game.city_yields(ours);
            game.city_yields(theirs);
            let reused = game.rival_yields_reused().0;
            let action = Action::Move { unit, to };
            assert!(game.quiet_move_yield_candidate(0, &action).is_some());
            game.apply(0, &action).unwrap();
            fresh.apply(0, &action).unwrap();
            assert!(game.query_memo.own_yields.borrow().is_empty());
            assert!(game.query_memo.rival_yields.borrow().contains_key(&theirs));
            assert_eq!(
                bits(game.city_yields(theirs)),
                bits(fresh.city_yields(theirs))
            );
            assert_eq!(game.rival_yields_reused().0, reused + 1);
            assert_eq!(bits(game.city_yields(ours)), bits(fresh.city_yields(ours)));
            assert_eq!(
                serde_json::to_value(&*game).unwrap(),
                serde_json::to_value(&*fresh).unwrap()
            );
        })
    });
    assert!(game.query_memo.rival_yields.borrow().is_empty());
    assert!(game.query_memo.own_yields.borrow().is_empty());
    assert_eq!(game.query_memo.acting.get(), 0);
    assert_eq!(
        serde_json::to_value(&game).unwrap(),
        serde_json::to_value(&fresh).unwrap()
    );
}

#[test]
fn a_refused_move_drops_both_sides() {
    let (mut game, ours, theirs, unit, to) = fixture();
    game.units.get_mut(&unit).unwrap().moves_left = 0.0;
    game.with_deferred_visibility(|game| {
        game.city_yields(ours);
        game.city_yields(theirs);
        assert!(game.apply(0, &Action::Move { unit, to }).is_err());
        assert!(game.query_memo.own_yields.borrow().is_empty());
        assert!(game.query_memo.rival_yields.borrow().is_empty());
        assert_eq!(game.query_memo.acting.get(), 0);
    });
}

#[test]
fn an_unbatched_move_keeps_the_original_invalidation() {
    let (mut game, ours, theirs, unit, to) = fixture();
    game.city_yields(ours);
    game.city_yields(theirs);
    assert!(game
        .quiet_move_yield_candidate(0, &Action::Move { unit, to })
        .is_none());
    game.apply(0, &Action::Move { unit, to }).unwrap();
    assert!(game.query_memo.own_yields.borrow().is_empty());
    assert!(game.query_memo.rival_yields.borrow().is_empty());
}

#[test]
fn a_village_effect_changes_the_witness_and_drops_rivals() {
    let (mut game, _, theirs, unit, to) = fixture();
    game.map.tiles.get_mut(&to).unwrap().improvement = Some(crate::name!("goody_hut"));
    let mut fresh = game.clone();
    fresh.keep_rival_yields(false);
    game.with_deferred_visibility(|game| {
        fresh.with_deferred_visibility(|fresh| {
            game.city_yields(theirs);
            let action = Action::Move { unit, to };
            let before = game.quiet_move_yield_candidate(0, &action).unwrap();
            game.apply(0, &action).unwrap();
            fresh.apply(0, &action).unwrap();
            assert_ne!(before, game.quiet_move_yield_stamp());
            assert!(game.query_memo.rival_yields.borrow().is_empty());
            assert_eq!(
                bits(game.city_yields(theirs)),
                bits(fresh.city_yields(theirs))
            );
        })
    });
    assert_eq!(
        serde_json::to_value(&game).unwrap(),
        serde_json::to_value(&fresh).unwrap()
    );
}

#[test]
fn in_place_player_city_and_tile_writes_each_change_the_witness() {
    let (mut game, _, theirs, _, to) = fixture();
    let before = game.quiet_move_yield_stamp();
    game.players[1].envoys.push((0, 2));
    assert_ne!(before, game.quiet_move_yield_stamp());
    let before = game.quiet_move_yield_stamp();
    game.players[1].envoys[0].1 += 1;
    assert_ne!(before, game.quiet_move_yield_stamp());
    let before = game.quiet_move_yield_stamp();
    game.cities.get_mut(&theirs).unwrap().food += 1.0;
    assert_ne!(before, game.quiet_move_yield_stamp());
    let before = game.quiet_move_yield_stamp();
    game.map.tiles.get_mut(&to).unwrap().hills = true;
    assert_ne!(before, game.quiet_move_yield_stamp());
}

#[test]
fn occupied_linked_and_non_move_orders_do_not_enter_the_fast_path() {
    let (mut game, _, theirs, unit, to) = fixture();
    game.with_deferred_visibility(|game| {
        game.city_yields(theirs);
        assert!(game
            .quiet_move_yield_candidate(0, &Action::Fortify { unit })
            .is_none());
        let other = game.spawn_unit("builder", 1, to);
        assert!(game
            .quiet_move_yield_candidate(0, &Action::Move { unit, to })
            .is_none());
        game.remove_unit(other);
        game.units.get_mut(&unit).unwrap().linked_to = Some(other);
        assert!(game
            .quiet_move_yield_candidate(0, &Action::Move { unit, to })
            .is_none());
    });
}

#[test]
fn only_the_outer_deferred_visibility_boundary_clears_retained_answers() {
    let (mut game, _, theirs, unit, to) = fixture();
    game.with_deferred_visibility(|game| {
        game.city_yields(theirs);
        game.with_deferred_visibility(|game| {
            game.apply(0, &Action::Move { unit, to }).unwrap();
            assert!(game.query_memo.rival_yields.borrow().contains_key(&theirs));
        });
        assert!(game.query_memo.rival_yields.borrow().contains_key(&theirs));
    });
    assert!(game.query_memo.rival_yields.borrow().is_empty());
    assert_eq!(game.query_memo.acting.get(), 0);
}

#[test]
fn civilian_religious_and_support_steps_reuse_the_same_exact_path() {
    for kind in ["builder", "apostle", "battering_ram"] {
        let (mut game, _, theirs, military, to) = fixture();
        let unit = game.spawn_unit(kind, 0, game.units[&military].pos);
        let mut fresh = game.clone();
        fresh.keep_rival_yields(false);
        game.with_deferred_visibility(|game| {
            fresh.with_deferred_visibility(|fresh| {
                game.city_yields(theirs);
                let action = Action::Move { unit, to };
                assert!(game.quiet_move_yield_candidate(0, &action).is_some());
                game.apply(0, &action).unwrap();
                fresh.apply(0, &action).unwrap();
                assert!(
                    game.query_memo.rival_yields.borrow().contains_key(&theirs),
                    "{kind}"
                );
                assert_eq!(
                    bits(game.city_yields(theirs)),
                    bits(fresh.city_yields(theirs))
                );
            })
        });
        assert_eq!(
            serde_json::to_value(&game).unwrap(),
            serde_json::to_value(&fresh).unwrap(),
            "{kind}"
        );
    }
}

#[test]
fn policy_read_traces_keep_the_fresh_derivations_questions() {
    let (mut game, _, theirs, unit, to) = fixture();
    game.with_deferred_visibility(|game| {
        game.city_yields(theirs);
        game.apply(0, &Action::Move { unit, to }).unwrap();
        let fresh = game.clone();
        let trace = game.trace_policy_reads(1);
        let held = game.city_yields(theirs);
        let held_reads = trace.finish();
        drop(trace);
        let trace = fresh.trace_policy_reads(1);
        let expected = fresh.city_yields(theirs);
        let expected_reads = trace.finish();
        assert_eq!(bits(held), bits(expected));
        assert_eq!(held_reads.effects, expected_reads.effects);
        assert_eq!(held_reads.cards, expected_reads.cards);
        assert_eq!(held_reads.opaque, expected_reads.opaque);
    });
}

#[test]
fn a_quiet_moves_player_write_changes_only_fog_memory() {
    for track_memory in [false, true] {
        let (mut game, _, theirs, unit, to) = fixture();
        game.track_fog_memory = track_memory;
        let before_player = serde_json::to_value(&game.players[0]).unwrap();
        game.with_deferred_visibility(|game| {
            game.city_yields(theirs);
            let before = game.quiet_move_yield_stamp();
            let all_writes = game.players.epoch();
            let fog_writes = game.query_memo.fog_only_player_writes.get();
            game.apply(0, &Action::Move { unit, to }).unwrap();
            assert!(game.players.epoch() > all_writes);
            assert!(game.query_memo.fog_only_player_writes.get() > fog_writes);
            assert_eq!(
                before,
                game.quiet_move_yield_stamp(),
                "track_memory={track_memory}"
            );
            let mut before = before_player.clone();
            let mut after = serde_json::to_value(&game.players[0]).unwrap();
            for value in [&mut before, &mut after] {
                let fields = value.as_object_mut().unwrap();
                fields.remove("remembered_tiles");
                fields.remove("remembered_cities");
                fields.remove("turn_units");
                fields.remove("turn_visible");
            }
            assert_eq!(before, after, "every other player field is identical");
        });
    }
}

#[test]
fn newly_explored_ground_is_never_counted_as_a_memory_only_write() {
    let (mut game, _, theirs, unit, to) = fixture();
    game.players[0].explored.clear();
    game.with_deferred_visibility(|game| {
        game.city_yields(theirs);
        let before = game.quiet_move_yield_stamp();
        game.apply(0, &Action::Move { unit, to }).unwrap();
        assert!(!game.players[0].explored.is_empty());
        assert_ne!(before, game.quiet_move_yield_stamp());
        assert!(game.query_memo.rival_yields.borrow().is_empty());
    });
}

#[test]
fn a_contact_made_while_walking_still_invalidates_rivals() {
    let (mut game, _, theirs, unit, to) = fixture();
    let near =
        game.nbrs(to)
            .into_iter()
            .find(|pos| {
                *pos != to
                    && *pos != game.units[&unit].pos
                    && game.unit_ids_at(*pos).is_empty()
                    && game.map.get(*pos).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .unwrap();
    game.spawn_unit("builder", 1, near);
    game.players[0].met.remove(&1);
    game.players[1].met.remove(&0);
    game.with_deferred_visibility(|game| {
        game.city_yields(theirs);
        let before = game.quiet_move_yield_stamp();
        game.apply(0, &Action::Move { unit, to }).unwrap();
        assert!(game.has_met(0, 1));
        assert!(game.has_met(1, 0));
        assert_ne!(before, game.quiet_move_yield_stamp());
        assert!(game.query_memo.rival_yields.borrow().is_empty());
    });
}
