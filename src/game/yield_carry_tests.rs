//! `Game::keep_rival_yields`: what is kept between a seat's passes, and every way it is dropped.

use super::*;

/// Two capitals, with seat 0 to act.
fn two_capitals() -> (Game, u32, u32) {
    let mut game = Game::new_full(2, 24, 16, 91_171, 100, 0, false);
    for pid in 0..2 {
        while game.current != pid {
            let current = game.current;
            game.apply(current, &Action::EndTurn)
                .expect("a seat with nothing to decide ends its turn");
        }
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("the opening roster includes a settler");
        game.apply(pid, &Action::FoundCity { unit: settler })
            .expect("found the opening city");
    }
    while game.current != 0 {
        let current = game.current;
        game.apply(current, &Action::EndTurn)
            .expect("the round comes back to seat 0");
    }
    let ours = game.player_city_ids(0)[0];
    let theirs = game.player_city_ids(1)[0];
    (game, ours, theirs)
}

fn kept_own(game: &Game) -> Vec<u32> {
    game.query_memo
        .own_yields
        .borrow()
        .yields
        .keys()
        .copied()
        .collect()
}

fn kept(game: &Game) -> Vec<u32> {
    game.query_memo
        .rival_yields
        .borrow()
        .yields
        .keys()
        .copied()
        .collect()
}

#[test]
fn a_rival_citys_yields_are_kept_from_one_pass_to_the_next() {
    let (mut game, _, theirs) = two_capitals();
    game.keep_rival_yields(true);

    let first = game.city_yields(theirs);
    assert_eq!(kept(&game), vec![theirs]);
    assert_eq!(game.rival_yields_reused(), (0, 1));

    // No memo is open here: before this, a second pass derived it again.
    assert_eq!(game.city_yields(theirs), first);
    assert_eq!(game.rival_yields_reused(), (1, 1));
}

#[test]
fn the_acting_seats_own_cities_are_not_kept_unless_asked() {
    let (mut game, ours, _) = two_capitals();
    game.keep_rival_yields(true);

    let before = game.city_yields(ours);
    assert!(kept(&game).is_empty());

    // The way an agent weighs its own empire: write it in place, read it,
    // and no action says so. Its own cities must read the board as it is.
    game.cities.get_mut(&ours).expect("our capital").pop += 3;
    assert_ne!(
        game.city_yields(ours),
        before,
        "three more citizens work three more tiles"
    );
    assert_eq!(game.rival_yields_reused(), (0, 0));
}

#[test]
fn every_action_drops_what_was_kept() {
    let (mut game, _, theirs) = two_capitals();
    game.keep_rival_yields(true);
    game.city_yields(theirs);
    assert_eq!(kept(&game), vec![theirs]);

    assert!(game
        .apply(1, &Action::EndTurn)
        .is_err_and(|refusal| refusal == "not your turn"));
    assert!(
        kept(&game).is_empty(),
        "a refused action drops it too: nothing here reasons about which refusals are clean"
    );

    game.city_yields(theirs);
    assert_eq!(kept(&game), vec![theirs]);
    game.apply(0, &Action::EndTurn)
        .expect("seat 0 may end its turn");
    assert!(
        kept(&game).is_empty(),
        "a successful action changes the board"
    );
}

#[test]
fn naming_what_a_city_builds_keeps_the_rivals_and_drops_our_own() {
    let (mut game, ours, theirs) = two_capitals();
    game.keep_rival_yields(true);
    game.keep_own_yields(true);
    let rival_before = game.city_yields(theirs);
    game.city_yields(ours);
    assert_eq!(kept(&game), vec![theirs]);
    assert_eq!(kept_own(&game), vec![ours]);

    // The one action that reaches nothing of another seat's: it writes this
    // city's queue, and a city's own yields do read its queue.
    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    game.apply(
        0,
        &Action::Produce {
            city: ours,
            item: warrior,
        },
    )
    .expect("an opening city can train a warrior");
    assert_eq!(kept(&game), vec![theirs], "their capital is as it was");
    assert!(kept_own(&game).is_empty(), "ours has a new queue");
    assert_eq!(game.city_yields(theirs), rival_before);
}

#[test]
fn a_what_if_against_the_deck_is_noticed_without_being_reported() {
    let (mut game, ours, _) = two_capitals();
    game.keep_rival_yields(true);
    game.keep_own_yields(true);
    let before = game.city_yields(ours);
    assert_eq!(game.city_yields(ours), before);
    assert_eq!(game.rival_yields_reused(), (1, 1));

    // The policy scorer's move, with nobody calling `board_written`: the
    // deck is read back on every lookup.
    let card = crate::name!("urban_planning");
    game.players[0].policies.insert(card);
    assert_ne!(
        game.city_yields(ours),
        before,
        "Urban Planning is a point of Production in every city"
    );
    game.players[0].policies.remove(&card);
    assert_eq!(game.city_yields(ours), before);
    assert_eq!(
        game.rival_yields_reused(),
        (1, 3),
        "both what-if reads were derived"
    );
}

#[test]
fn nothing_is_reused_while_a_policy_trace_records() {
    let (mut game, _, theirs) = two_capitals();
    game.keep_rival_yields(true);
    game.city_yields(theirs);
    assert_eq!(game.rival_yields_reused(), (0, 1));

    // The trace's caller reads which questions a sweep asked. A kept answer
    // asks none, so a traced sweep is always derived.
    let trace = game.trace_policy_reads(0);
    game.city_yields(theirs);
    assert_eq!(
        game.rival_yields_reused(),
        (0, 1),
        "neither reused nor filled"
    );
    drop(trace);

    game.city_yields(theirs);
    assert_eq!(game.rival_yields_reused(), (1, 1));
}

#[test]
fn a_tile_write_drops_what_was_kept() {
    let (mut game, _, theirs) = two_capitals();
    game.keep_rival_yields(true);
    let before = game.city_yields(theirs);

    // Whoever writes a tile moves the grid's epoch; nobody has to report it.
    let centre = game.cities[&theirs].pos;
    game.map
        .tiles
        .get_mut(&centre)
        .expect("the capital stands on a tile")
        .pillaged = true;
    game.city_yields(theirs);
    assert_eq!(
        game.rival_yields_reused(),
        (0, 2),
        "the second read was derived, not reused"
    );

    game.map
        .tiles
        .get_mut(&centre)
        .expect("the capital stands on a tile")
        .pillaged = false;
    assert_eq!(game.city_yields(theirs), before);
}

#[test]
fn a_reported_write_is_read_and_an_unreported_one_is_what_the_audit_is_for() {
    let (mut game, _, theirs) = two_capitals();
    game.keep_rival_yields(true);
    let before = game.city_yields(theirs);

    game.cities.get_mut(&theirs).expect("their capital").pop += 3;
    assert_eq!(
        game.city_yields(theirs),
        before,
        "an unreported write to another seat is exactly the staleness the contract names"
    );

    game.board_written();
    assert_ne!(game.city_yields(theirs), before);
}

#[test]
#[should_panic(expected = "a kept city yield went stale")]
fn the_audit_panics_on_a_write_nobody_reported() {
    let (mut game, _, theirs) = two_capitals();
    game.keep_rival_yields(true);
    game.audit_rival_yields(true);
    game.city_yields(theirs);
    game.cities.get_mut(&theirs).expect("their capital").pop += 3;
    game.city_yields(theirs);
}

#[test]
fn nothing_is_kept_unless_a_driver_asks_and_a_clone_never_keeps() {
    let (mut game, _, theirs) = two_capitals();
    game.city_yields(theirs);
    assert!(kept(&game).is_empty(), "off by default");

    game.keep_rival_yields(true);
    game.city_yields(theirs);
    let copy = game.clone();
    assert!(kept(&copy).is_empty());
    copy.city_yields(theirs);
    assert!(
        kept(&copy).is_empty(),
        "a search branch is about to be played forward"
    );

    game.keep_rival_yields(false);
    assert!(kept(&game).is_empty(), "turning it off drops what it held");
}

/// What a yield is made of is kept beside it: the citizen plan, which the
/// Builder census and the state document ask for by itself, and the split of
/// the empire's luxuries, which every city's amenities ask for.
#[test]
fn a_citys_plan_and_its_empires_luxury_split_are_kept_beside_its_yields() {
    let (mut game, _, theirs) = two_capitals();
    let reference = game.clone();
    game.keep_rival_yields(true);

    // Deriving the yield works the plan out and splits the luxuries once.
    game.city_yields(theirs);
    assert_eq!(game.kept_plans_reused(), (0, 0));
    {
        let held = game.query_memo.rival_yields.borrow();
        assert!(held.plans.contains_key(&theirs));
        assert!(held.luxuries.contains_key(&1));
    }

    // Asked for by themselves, with no memo open, they answer from there —
    // and are what a board that keeps nothing works out.
    assert_eq!(
        game.city_citizen_plan(theirs),
        reference.city_citizen_plan(theirs)
    );
    assert_eq!(game.kept_plans_reused().0, 1);
    assert_eq!(
        game.city_amenities(&game.cities[&theirs]),
        reference.city_amenities(&reference.cities[&theirs])
    );
    assert_eq!(game.kept_plans_reused().1, 1);

    // A what-if under other weights is neither kept nor answered from this.
    let weighted = game.city_citizen_plan_weighted(theirs, Some(Yields::default()));
    assert_eq!(
        weighted,
        reference.city_citizen_plan_weighted(theirs, Some(Yields::default()))
    );
    assert_eq!(game.kept_plans_reused().0, 1);

    // The plan asked for first is kept for the yield that follows it.
    game.board_written();
    game.city_citizen_plan(theirs);
    assert!(kept(&game).is_empty(), "no yield was derived");
    assert_eq!(game.city_yields(theirs), reference.city_yields(theirs));
    assert_eq!(
        game.kept_plans_reused().0,
        2,
        "the yield summed the kept plan"
    );
}

/// Whatever drops a kept yield drops the plan and the split kept with it.
#[test]
fn the_kept_plan_and_split_go_wherever_the_kept_yield_goes() {
    let (mut game, _, theirs) = two_capitals();
    game.keep_rival_yields(true);
    let nothing_kept = |game: &Game| {
        let held = game.query_memo.rival_yields.borrow();
        held.yields.is_empty() && held.plans.is_empty() && held.luxuries.is_empty()
    };

    game.city_yields(theirs);
    assert!(!nothing_kept(&game));
    assert!(game.apply(1, &Action::EndTurn).is_err());
    assert!(nothing_kept(&game), "an action, even a refused one");

    game.city_yields(theirs);
    game.board_written();
    assert!(nothing_kept(&game), "a reported write");

    game.city_yields(theirs);
    let tile = game.cities[&theirs].pos;
    game.map
        .tiles
        .get_mut(&tile)
        .expect("their centre")
        .pillaged = false;
    game.city_citizen_plan(theirs);
    assert_eq!(
        game.kept_plans_reused().0,
        0,
        "a tile write moves the grid's epoch, and the plan is worked out again"
    );

    game.city_yields(theirs);
    game.keep_rival_yields(false);
    assert!(nothing_kept(&game), "turning it off");
}

/// The audit checks a reused plan the way it checks a reused yield.
#[test]
#[should_panic(expected = "a kept citizen plan went stale")]
fn the_audit_panics_on_a_stale_plan() {
    let (mut game, _, theirs) = two_capitals();
    game.keep_rival_yields(true);
    game.audit_rival_yields(true);
    game.city_citizen_plan(theirs);
    game.cities.get_mut(&theirs).expect("their capital").pop += 3;
    game.city_citizen_plan(theirs);
}
