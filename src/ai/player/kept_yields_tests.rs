//! A production player's planning view keeps city yields between its passes, and the game it plays is the game it played when every yield was derived afresh.
use super::*;
use crate::ai::{run_game, Ai};
use crate::game::GameOptions;

/// Long enough for second and third cities, a policy deck or two and the
/// first wars, which is where a plan's passes start to sweep the same cities.
const TURNS: u32 = 60;

/// One native game of four production players and three city-states, every
/// major's views answering for yields as `mode` says.
fn played(seed: u64, mode: ViewYields) -> (Game, Vec<AdvancedAi>) {
    let mut game = Game::new_with(GameOptions {
        speed: "online".to_string(),
        ..GameOptions::new(4, 44, 28, seed, TURNS, 3)
    });
    let mut ais: Vec<AdvancedAi> = (0..game.players.len())
        .map(|_| {
            let mut ai = AdvancedAi::new();
            ai.enable_engine_repairs();
            ai.set_view_yields(mode);
            ai
        })
        .collect();
    assert!(ais[0].uses_player_observation());
    run_game(&mut game, &mut ais);
    (game, ais)
}

fn saved(game: &Game) -> String {
    serde_json::to_string(game).expect("a game serializes")
}

fn reads(ais: &[AdvancedAi]) -> (u64, u64) {
    ais.iter().fold((0, 0), |(reused, derived), ai| {
        let seat = ai.view_yields();
        (reused + seat.reused, derived + seat.derived)
    })
}

/// Keeping is an optimization, so the board at the end must be the board the
/// slow way reaches, field for field. This is the comparison that sees what
/// an audit of values cannot: a caller that depends on a derivation having
/// *run* (the policy scorer's read trace did, on the site, and the first
/// build that kept a seat's own cities played a different game from turn 26
/// with every kept value correct).
#[test]
fn a_view_that_keeps_yields_plays_the_game_it_played_without() {
    let (derived, derived_seats) = played(3_750_002, ViewYields::Derived);
    assert_eq!(
        reads(&derived_seats),
        (0, 0),
        "`Derived` must keep nothing, or this compares a thing with itself"
    );
    let (kept, kept_seats) = played(3_750_002, ViewYields::Kept);
    let (reused, _) = reads(&kept_seats);
    assert!(reused > 0, "the run this compares must actually reuse");
    assert_eq!(kept.turn, derived.turn);
    assert!(
        saved(&kept) == saved(&derived),
        "a planning view that keeps city yields played a different game"
    );
}

/// The stronger form for values: every reused yield, citizen plan and luxury
/// split is derived again on the spot and compared. A write to a view that
/// no action, tile write or `Game::board_written` reported panics here with
/// the city and the turn.
#[test]
fn every_yield_a_view_keeps_is_what_the_view_gives() {
    let (audited, seats) = played(3_750_003, ViewYields::Audited);
    let (reused, derived) = reads(&seats);
    assert!(reused > 0, "an audit of nothing proves nothing");
    assert!(derived > 0);
    assert!(audited.turn > TURNS || audited.winner.is_some());
}

/// Keeping is the default, it is per seat, and it is the native executor's:
/// the board the game is played on keeps nothing because of it.
#[test]
fn keeping_is_the_default_and_stays_on_the_view() {
    assert_eq!(AdvancedAi::new().view_yields().mode, ViewYields::Kept);
    let mut game = Game::new_full(2, 30, 20, 918, 6, 0, false);
    let mut ais: Vec<AdvancedAi> = (0..game.players.len())
        .map(|_| {
            let mut ai = AdvancedAi::new();
            ai.enable_engine_repairs();
            ai
        })
        .collect();
    ais[1].set_view_yields(ViewYields::Derived);
    run_game(&mut game, &mut ais);
    assert!(ais[0].view_yields().derived > 0, "seat 0's views kept");
    assert_eq!(
        (ais[1].view_yields().reused, ais[1].view_yields().derived),
        (0, 0),
        "seat 1 was told to derive"
    );
    assert_eq!(
        game.rival_yields_reused(),
        (0, 0),
        "the authoritative board was never asked to keep"
    );
}
