//! Kept city yields across an ordinary unit move: what survives and what the
//! move's watch must notice. See the ordinary-move note in `Game::apply`.

use super::*;

/// Two capitals, with seat 0 to act, keeping both seats' yields.
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
    game.keep_rival_yields(true);
    game.keep_own_yields(true);
    let ours = game.player_city_ids(0)[0];
    let theirs = game.player_city_ids(1)[0];
    (game, ours, theirs)
}

fn kept(game: &Game) -> (Vec<u32>, Vec<u32>) {
    let keys = |held: &std::cell::RefCell<KeptDerivations>| {
        held.borrow().yields.keys().copied().collect::<Vec<_>>()
    };
    (
        keys(&game.query_memo.rival_yields),
        keys(&game.query_memo.own_yields),
    )
}

/// A seat-0 warrior on open land two steps from our capital, and a free
/// neighbouring tile that is not a city centre.
fn walker(game: &mut Game, ours: u32) -> (u32, Pos) {
    let centre = game.cities[&ours].pos;
    let open = |game: &Game, pos: Pos| {
        game.city_at(pos).is_none()
            && game.unit_ids_at(pos).is_empty()
            && game
                .map
                .get(pos)
                .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
    };
    let start = game
        .wring(centre, 2)
        .into_iter()
        .find(|pos| {
            open(game, *pos)
                && game
                    .nbrs(*pos)
                    .into_iter()
                    .any(|next| open(game, next) && game.wdist(next, centre) >= 2)
        })
        .expect("open land two steps out");
    let uid = game.spawn_test_unit("warrior", 0, start);
    let step = game
        .nbrs(start)
        .into_iter()
        .find(|next| open(game, *next) && game.wdist(*next, centre) >= 2)
        .expect("an open step that keeps clear of the centre");
    (uid, step)
}

#[test]
fn an_ordinary_move_and_a_fortify_keep_every_kept_yield() {
    let (mut game, ours, theirs) = two_capitals();
    let (uid, step) = walker(&mut game, ours);
    let (rival, own) = (game.city_yields(theirs), game.city_yields(ours));
    assert_eq!(kept(&game), (vec![theirs], vec![ours]));

    game.apply(
        0,
        &Action::Move {
            unit: uid,
            to: step,
        },
    )
    .expect("the warrior steps");
    assert_eq!(
        kept(&game),
        (vec![theirs], vec![ours]),
        "a quiet step keeps both"
    );
    assert_eq!(game.city_yields(theirs), rival);
    assert_eq!(game.city_yields(ours), own);

    let _ = game.apply(0, &Action::Fortify { unit: uid });
    assert_eq!(
        kept(&game),
        (vec![theirs], vec![ours]),
        "fortifying writes only the unit"
    );
}

#[test]
fn a_move_onto_or_off_a_city_centre_drops_them() {
    let (mut game, ours, theirs) = two_capitals();
    let centre = game.cities[&ours].pos;
    let beside =
        game.nbrs(centre)
            .into_iter()
            .find(|pos| {
                game.unit_ids_at(*pos).is_empty()
                    && game.map.get(*pos).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .expect("open land beside the capital");
    for uid in game.unit_ids_at(centre).to_vec() {
        game.remove_unit(uid);
    }
    let uid = game.spawn_test_unit("warrior", 0, beside);
    game.city_yields(theirs);
    game.city_yields(ours);
    game.apply(
        0,
        &Action::Move {
            unit: uid,
            to: centre,
        },
    )
    .expect("the warrior garrisons the capital");
    assert_eq!(kept(&game), (vec![], vec![]), "a garrison arrived");

    game.city_yields(theirs);
    game.city_yields(ours);
    game.units.get_mut(&uid).unwrap().moves_left = game.unit_max_moves(uid);
    game.apply(
        0,
        &Action::Move {
            unit: uid,
            to: beside,
        },
    )
    .expect("the warrior steps out again");
    assert_eq!(kept(&game), (vec![], vec![]), "a garrison left");
}

#[test]
fn the_watch_notices_a_meeting_a_capture_and_a_unit_leaving() {
    let (mut game, ours, _) = two_capitals();
    let (uid, _) = walker(&mut game, ours);
    let before = game.ordinary_move_watch(uid);
    assert!(before == game.ordinary_move_watch(uid));

    let mut met = game.clone();
    if met.players[0].met.contains(&1) {
        met.players[0].met.remove(&1);
    } else {
        met.record_contact(0, 1);
    }
    assert!(met.ordinary_move_watch(uid) != before, "first contact");

    let mut taken = game.clone();
    taken.units.get_mut(&uid).unwrap().owner = 1;
    assert!(taken.ordinary_move_watch(uid) != before, "a capture");

    let mut gone = game.clone();
    let other = gone
        .player_unit_ids(1)
        .into_iter()
        .next()
        .expect("seat 1 has a unit");
    gone.remove_unit(other);
    assert!(
        gone.ordinary_move_watch(uid) != before,
        "a unit left the board"
    );

    let mut lost = game.clone();
    lost.cities.get_mut(&ours).unwrap().owner = 1;
    assert!(
        lost.ordinary_move_watch(uid) != before,
        "a city changed hands"
    );
}
