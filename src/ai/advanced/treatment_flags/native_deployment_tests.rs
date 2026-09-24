use crate::ai::{run_game, AdvancedAi, Ai};
use crate::game::{Game, GameOptions};

fn played(options: &GameOptions, seat: impl Fn() -> AdvancedAi) -> String {
    let mut game = Game::new_with(options.clone());
    let mut ais: Vec<AdvancedAi> = game.players.iter().map(|_| seat()).collect();
    run_game(&mut game, &mut ais);
    serde_json::to_string(&game.log).expect("the action log serializes")
}

/// `enable_native_deployment` is the ledger's deployment genome — exactly
/// `enable_engine_repairs` — with only the planning board changed: it plans
/// on the authoritative board, as the site's agents always did, instead of a
/// fog-redacted view it would replay. Pinned by the whole decision stream,
/// not by a list of flags that could drift away from the ledger.
#[test]
fn native_deployment_is_the_ledger_genome_on_the_authoritative_board() {
    let options = GameOptions::new(3, 32, 20, 90_911, 30, 1);
    let native = || {
        let mut ai = AdvancedAi::new();
        ai.enable_native_deployment();
        ai
    };
    assert!(!native().uses_player_observation());
    let ledger_on_the_board = || {
        let mut ai = AdvancedAi::new();
        ai.enable_engine_repairs();
        assert!(ai.uses_player_observation());
        ai.observed_player = false;
        ai
    };
    assert_eq!(
        played(&options, native),
        played(&options, ledger_on_the_board),
        "the native deployment plays exactly the ledger genome"
    );
    assert_ne!(
        played(&options, native),
        played(&options, AdvancedAi::new),
        "the ledger changes decisions the stock agent makes: this is not the stock agent"
    );
}
