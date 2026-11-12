//! Quiet fortify retention, real post-action changes, and the visibility boundary.

use super::*;

fn fixture() -> (Game, u32, u32, u32) {
    let mut game = Game::new_full(2, 24, 16, 521_515, 250, 0, false);
    let homes: Vec<_> = (0..2)
        .map(|pid| {
            game.player_unit_ids(pid)
                .into_iter()
                .find(|id| game.units[id].kind == "settler")
                .map(|id| game.units[&id].pos)
                .unwrap()
        })
        .collect();
    for id in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(id);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
    }
    let ours = game.found_city_for(0, homes[0], None);
    let theirs = game.found_city_for(1, homes[1], None);
    let unit = game.spawn_test_unit("warrior", 0, homes[0]);
    game.spawn_test_unit("warrior", 1, homes[1]);
    game.players[0].policies.insert(crate::name!("retainers"));
    game.keep_rival_yields(true);
    game.keep_own_yields(true);
    game.audit_rival_yields(true);
    (game, ours, theirs, unit)
}

fn warm(game: &Game, ours: u32, theirs: u32) -> (Yields, Yields) {
    let result = (game.city_yields(ours), game.city_yields(theirs));
    assert!(game.query_memo.own_yields.borrow().contains_key(&ours));
    assert!(game.query_memo.rival_yields.borrow().contains_key(&theirs));
    result
}

fn empty(game: &Game) {
    assert!(game.query_memo.own_yields.borrow().is_empty());
    assert!(game.query_memo.rival_yields.borrow().is_empty());
}

#[test]
fn quiet_fortify_reuses_both_city_yields_and_flush_discards_them() {
    let (mut game, ours, theirs, unit) = fixture();
    let mut control = game.clone();
    game.with_deferred_visibility(|game| {
        let before = warm(game, ours, theirs);
        let reads = game.rival_yields_reused().0;
        game.apply(0, &Action::Fortify { unit }).unwrap();
        assert!(game.units[&unit].fortified);
        assert_eq!(game.units[&unit].moves_left, 0.0);
        assert_eq!(warm(game, ours, theirs), before);
        assert_eq!(game.rival_yields_reused().0, reads + 2);
        assert_eq!(game.query_memo.acting.get(), 0);
    });
    // Postponed discovery/contact publication is a separate write boundary.
    empty(&game);
    assert_eq!(game.query_memo.acting.get(), 0);
    control.with_deferred_visibility(|control| {
        control.apply(0, &Action::Fortify { unit }).unwrap();
    });
    assert_eq!(
        serde_json::to_value(&game).unwrap(),
        serde_json::to_value(&control).unwrap(),
        "the original uncached action and every deferred post-effect stay identical"
    );
}

#[test]
fn quiet_fortify_refusal_clears_even_an_unchanged_board() {
    let (mut game, ours, theirs, _) = fixture();
    game.with_deferred_visibility(|game| {
        warm(game, ours, theirs);
        assert!(game.apply(0, &Action::Fortify { unit: u32::MAX }).is_err());
        empty(game);
        assert_eq!(game.query_memo.acting.get(), 0);
    });
}

#[test]
fn quiet_fortify_without_visibility_batch_keeps_original_invalidation() {
    let (mut game, ours, theirs, unit) = fixture();
    warm(&game, ours, theirs);
    game.apply(0, &Action::Fortify { unit }).unwrap();
    empty(&game);
}

#[test]
fn quiet_fortify_post_action_monopoly_award_invalidates_both_sides() {
    let (mut game, ours, theirs, unit) = fixture();
    // A city-centre luxury is connected immediately. The successful action
    // still has to award its first-monopoly moment after fortifying.
    let home = game.cities[&ours].pos;
    game.map.tiles.get_mut(&home).unwrap().resource = Some(crate::name!("diamonds"));
    game.query_memo.monopoly_context.borrow_mut().take();
    let key = "historic_moment:first_luxury_monopoly";
    game.players[0].counters.remove(key);
    game.with_deferred_visibility(|game| {
        warm(game, ours, theirs);
        let stamp = game.fortify_yield_stamp();
        game.apply(0, &Action::Fortify { unit }).unwrap();
        assert!(game.players[0].counters.contains_key(key));
        assert!(stamp != game.fortify_yield_stamp());
        empty(game);
    });
}

#[test]
fn quiet_fortify_does_not_retain_for_other_orders() {
    let (mut game, ours, theirs, unit) = fixture();
    game.with_deferred_visibility(|game| {
        warm(game, ours, theirs);
        assert!(game
            .apply(
                0,
                &Action::Move {
                    unit,
                    to: (0, -500)
                }
            )
            .is_err());
        empty(game);
    });
}

#[test]
fn quiet_fortify_does_not_suppress_policy_read_tracing() {
    let (mut game, ours, theirs, unit) = fixture();
    game.with_deferred_visibility(|game| {
        warm(game, ours, theirs);
        game.apply(0, &Action::Fortify { unit }).unwrap();
        let reference = game.clone();
        let trace = game.trace_policy_reads(0);
        let actual = game.city_yields(ours);
        let actual_reads = trace.finish();
        let reference_trace = reference.trace_policy_reads(0);
        let expected = reference.city_yields(ours);
        let expected_reads = reference_trace.finish();
        assert_eq!(actual, expected);
        assert_eq!(actual_reads.effects, expected_reads.effects);
        assert_eq!(actual_reads.cards, expected_reads.cards);
        // A policy trace records the derivation rather than taking a kept answer.
        assert!(!actual_reads.effects.is_empty());
    });
}
