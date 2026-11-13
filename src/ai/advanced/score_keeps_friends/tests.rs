use super::*;

#[test]
fn score_seat_keeps_friends_is_opt_in() {
    super::super::test_support::opt_in_off_in_both_controllers("score-seat-keeps-friends", |ai| {
        ai.score_seat_keeps_friends
    });
}

/// Three majors in contact, each with a capital, major 2 the strongest, on
/// turn 31 (not seat 0's `befriend-the-strongest` cadence turn).
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
    let capital = game.cities[&game.player_city_ids(2)[0]].pos;
    for _ in 0..4 {
        game.spawn_test_unit("warrior", 2, capital);
    }
    game.turn = 31;
    game
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    ai.disable_befriend_the_strongest();
    if gene {
        ai.enable_score_seat_keeps_friends();
    }
    ai
}

fn asked(game: &Game) -> Vec<usize> {
    game.pending_deals
        .iter()
        .filter(|deal| deal.from == 0 && deal.friendship)
        .map(|deal| deal.to)
        .collect()
}

/// The lease reaches the live closer only for a Score seat with the gene.
#[test]
fn the_lease_is_a_score_seats_with_the_gene() {
    let game = three_majors();
    assert_eq!(
        seat(VictoryTarget::Score, true).score_seat_keeps_friends_lease(&game),
        Some(None)
    );
    for ai in [
        seat(VictoryTarget::Score, false),
        seat(VictoryTarget::Domination, true),
        seat(VictoryTarget::Culture, true),
    ] {
        assert_eq!(ai.score_seat_keeps_friends_lease(&game), None);
    }
}

/// Off the cadence turn, a Score seat with the gene asks the nearest major
/// and, the next turn, the other; off, or for another target, nobody is
/// asked.
#[test]
fn a_score_seat_asks_every_turn_nearest_first() {
    for ai in [
        seat(VictoryTarget::Score, false),
        seat(VictoryTarget::Science, true),
    ] {
        let mut game = three_majors();
        ai.propose_protective_friendship(&mut game, 0);
        assert!(asked(&game).is_empty());
    }
    let mut game = three_majors();
    let near = |game: &Game, other| AdvancedAi::score_friend_distance(game, 0, other);
    let nearest = if near(&game, 1) <= near(&game, 2) {
        1
    } else {
        2
    };
    let ai = seat(VictoryTarget::Score, true);
    ai.propose_protective_friendship(&mut game, 0);
    assert_eq!(asked(&game), [nearest], "the nearest major first");
    game.turn += 1;
    ai.propose_protective_friendship(&mut game, 0);
    let mut both = asked(&game);
    both.sort_unstable();
    assert_eq!(both, [1, 2], "the next turn asks the other");
}

/// The rival whose city the Score campaign names is never asked, and a
/// rival at war with us is never asked.
#[test]
fn the_campaigns_target_and_a_war_are_never_asked() {
    let mut game = three_majors();
    let mut ai = seat(VictoryTarget::Score, true);
    ai.enable_score_seat_takes_what_pays();
    let target_city = game.player_city_ids(1)[0];
    ai.score_take_state.named = Some((
        game.cities[&target_city].pos,
        game.players[1].civ.clone(),
        10.0,
    ));
    assert_eq!(ai.score_keeps_friends_spares(&game), Some(1));
    assert_eq!(ai.score_seat_keeps_friends_lease(&game), Some(Some(1)));
    game.at_war.insert((0, 2));
    for _ in 0..3 {
        ai.propose_protective_friendship(&mut game, 0);
        game.turn += 1;
    }
    assert!(asked(&game).is_empty(), "{:?}", asked(&game));
}
