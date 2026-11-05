use super::*;
use crate::setup::GameSpeed;

fn game(turn: u32) -> Game {
    let mut g = Game::new(3, 32, 22, 71, 250, 0);
    g.game_speed = GameSpeed::Online;
    g.turn = turn;
    g
}

/// Three losses inside the window pause training; with the gene off, or once
/// one ages out, nothing pauses. At Online the 30 standard turns are 20
/// (`GameSpeed::scale_turns`).
#[test]
fn three_recent_losses_pause_spy_training() {
    let g = game(140);
    let window = g.standard_duration(SPY_LOSS_WINDOW_STANDARD);
    assert_eq!(window, 20);

    let mut off = AdvancedAi::new();
    off.observe_spy_losses(vec![129, 131, 139]);
    assert_eq!(off.recent_spy_losses(&g), 3);
    assert!(!off.spy_losses_pause_training(&g));

    let mut on = AdvancedAi::new();
    on.enable_denial_spies_count_their_losses();
    on.observe_spy_losses(vec![119, 129, 131, 139]);
    assert_eq!(on.recent_spy_losses(&g), 3);
    assert!(on.spy_losses_pause_training(&g));

    // t115 and t119 are 20 or more turns back; only two remain.
    on.observe_spy_losses(vec![115, 119, 131, 139]);
    assert!(!on.spy_losses_pause_training(&g));

    // A loss logged after this turn (a replay reads a log that runs on) does
    // not count.
    on.observe_spy_losses(vec![131, 139, 150]);
    assert!(!on.spy_losses_pause_training(&g));
}

/// Version two pauses on the losses the bridge passes it exactly as version
/// one does (the bridge passes only Disrupt losses under version two).
#[test]
fn version_two_pauses_like_version_one() {
    let g = game(140);
    let mut v2 = AdvancedAi::new();
    v2.observe_spy_losses(vec![129, 131, 139]);
    assert!(!v2.spy_losses_pause_training(&g));
    v2.enable_denial_spies_count_their_losses_2();
    assert!(v2.denial_spies_count_their_losses_2_enabled());
    assert!(v2.spy_losses_pause_training(&g));
    v2.observe_spy_losses(vec![131, 139]);
    assert!(!v2.spy_losses_pause_training(&g));
}
