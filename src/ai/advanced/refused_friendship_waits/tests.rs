use super::*;

#[test]
fn refused_friendship_waits_is_opt_in() {
    super::super::test_support::opt_in_off_in_both_controllers("refused-friendship-waits", |ai| {
        ai.refused_friendship_waits
    });
}

/// Three majors in contact, each with a capital, on turn 54: the board of
/// `score_keeps_friends`'s tests, three turns after a refusal on turn 51.
fn three_majors() -> Game {
    let mut game = Game::new_full(3, 24, 16, 7_923, 300, 0, false);
    for pid in 0..3 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("every major starts with a settler");
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    for pid in 0..3 {
        for other in pid + 1..3 {
            game.record_contact(pid, other);
        }
    }
    game.turn = 54;
    game
}

/// A Score seat that asks every turn (`score-seat-keeps-friends`), as seat
/// p1 of `selfplay-duel-20261127T0654Z` did.
fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Score);
    ai.disable_befriend_the_strongest();
    ai.enable_score_seat_keeps_friends();
    if gene {
        ai.enable_refused_friendship_waits();
    }
    ai
}

fn asked(game: &Game) -> Vec<usize> {
    let mut to: Vec<usize> = game
        .pending_deals
        .iter()
        .filter(|deal| deal.from == 0 && deal.friendship)
        .map(|deal| deal.to)
        .collect();
    to.sort_unstable();
    to
}

/// The major `score-seat-keeps-friends` asks first on this board.
fn first_choice() -> usize {
    let mut game = three_majors();
    seat(false).propose_protective_friendship(&mut game, 0);
    let to = asked(&game);
    assert_eq!(to.len(), 1, "the seat asks one major a turn: {to:?}");
    to[0]
}

#[test]
fn off_the_refused_major_is_asked_again() {
    let refuser = first_choice();
    let mut game = three_majors();
    let mut ai = seat(false);
    ai.set_refused_friendships(BTreeMap::from([(refuser, 51)]));
    ai.propose_protective_friendship(&mut game, 0);
    assert_eq!(
        asked(&game),
        [refuser],
        "the shipped choice ignores the refusal"
    );
}

#[test]
fn on_the_refused_major_waits_and_the_other_is_asked() {
    let refuser = first_choice();
    let other = 3 - refuser;
    let mut game = three_majors();
    let mut ai = seat(true);
    ai.set_refused_friendships(BTreeMap::from([(refuser, 51)]));
    ai.propose_protective_friendship(&mut game, 0);
    assert_eq!(asked(&game), [other], "the other major is asked instead");
}

#[test]
fn on_with_every_major_refused_nobody_is_asked_until_the_wait_ends() {
    let mut game = three_majors();
    let mut ai = seat(true);
    ai.set_refused_friendships(BTreeMap::from([(1, 51), (2, 52)]));
    ai.propose_protective_friendship(&mut game, 0);
    assert!(asked(&game).is_empty(), "{:?}", asked(&game));
    // Turn 61: ten turns after major 1's refusal, two short of major 2's.
    game.turn = 51 + REFUSED_FRIENDSHIP_WAIT_TURNS;
    ai.propose_protective_friendship(&mut game, 0);
    assert_eq!(
        asked(&game),
        [1],
        "major 1's wait is over, major 2's is not"
    );
}

#[test]
fn a_refusal_from_a_later_turn_than_the_board_is_not_read() {
    // A replay reading a log past its own turn must answer as the game did.
    let refuser = first_choice();
    let mut game = three_majors();
    let mut ai = seat(true);
    ai.set_refused_friendships(BTreeMap::from([(refuser, 60)]));
    ai.propose_protective_friendship(&mut game, 0);
    assert_eq!(asked(&game), [refuser]);
}
