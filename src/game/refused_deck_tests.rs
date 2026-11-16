//! Kept city yields across a policy card the rules refuse, and one they
//! accept. See the refused-card note in `Game::apply`.

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

#[test]
fn a_refused_card_keeps_every_kept_yield_and_a_slotted_one_drops_them() {
    let (mut game, ours, theirs) = two_capitals();
    let before = (game.city_yields(ours), game.city_yields(theirs));
    assert_eq!(kept(&game), (vec![theirs], vec![ours]));

    // God King needs Code of Laws, which no seat has on turn 2.
    let card = Action::SlotPolicy {
        policy: crate::name!("god_king"),
    };
    assert!(game.apply(0, &card).is_err());
    assert!(game
        .apply(
            0,
            &Action::UnslotPolicy {
                policy: crate::name!("god_king"),
            },
        )
        .is_err());
    assert_eq!(kept(&game), (vec![theirs], vec![ours]));
    let (reused, derived) = game.rival_yields_reused();
    assert_eq!((game.city_yields(ours), game.city_yields(theirs)), before);
    assert_eq!(game.rival_yields_reused(), (reused + 2, derived));

    // Seated, the card drops everything, and the capital reads it.
    game.players[0].civics.insert(crate::name!("code_of_laws"));
    game.board_written();
    game.city_yields(ours);
    game.city_yields(theirs);
    game.apply(0, &card)
        .expect("Code of Laws opens a slot for it");
    assert_eq!(kept(&game), (vec![], vec![]));
    let seated = game.city_yields(ours);
    assert!(seated.gold > before.0.gold && seated.faith > before.0.faith);
    assert_eq!(game.city_yields(theirs), before.1);

    // And unslotting it drops them again.
    game.apply(
        0,
        &Action::UnslotPolicy {
            policy: crate::name!("god_king"),
        },
    )
    .expect("the card is seated");
    assert_eq!(kept(&game), (vec![], vec![]));
    assert_eq!(game.city_yields(ours), before.0);
}
