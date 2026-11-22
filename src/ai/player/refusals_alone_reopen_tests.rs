//! `refusals-alone-reopen-the-turn`: with the gene on, a seat's turn opens a
//! second frame only after the board refused one of its orders.
use super::*;
use crate::ai::{run_game, Ai};
use crate::game::GameOptions;

const TURNS: u32 = 40;

/// One native game of four production players and three city-states, every
/// major with the gene as `on` says. Returns each major's replans and
/// refusals.
fn played(seed: u64, on: bool) -> Vec<(i64, i64)> {
    let mut game = Game::new_with(GameOptions {
        speed: "online".to_string(),
        ..GameOptions::new(4, 44, 28, seed, TURNS, 3)
    });
    let mut ais: Vec<AdvancedAi> = (0..game.players.len())
        .map(|_| {
            let mut ai = AdvancedAi::new();
            ai.enable_engine_repairs();
            if on {
                ai.enable_refusals_alone_reopen_the_turn();
            }
            ai
        })
        .collect();
    assert!(ais[0].uses_player_observation());
    run_game(&mut game, &mut ais);
    game.players
        .iter()
        .filter(|player| !player.is_minor && !player.is_barbarian)
        .map(|player| {
            let read = |key: &str| player.counters.get(key).copied().unwrap_or(0);
            (read("player:replans"), read("player:refused"))
        })
        .collect()
}

/// Every replan the gene allows is paid for by at least one refusal in the
/// frame before it, so no seat can replan more often than it was refused;
/// without the gene the same seats replan far more often than that.
#[test]
fn only_a_refusal_reopens_the_turn() {
    let off = played(3_760_001, false);
    let on = played(3_760_001, true);
    for (seat, (replans, refused)) in on.iter().enumerate() {
        assert!(
            replans <= refused,
            "seat {seat} replanned {replans} times on {refused} refusals"
        );
    }
    let (off_replans, off_refused) = off.iter().fold((0, 0), |(a, b), (replans, refused)| {
        (a + replans, b + refused)
    });
    assert!(
        off_replans > off_refused,
        "the control must replan for something besides refusals \
         ({off_replans} replans on {off_refused} refusals), or this test proves nothing"
    );
}
