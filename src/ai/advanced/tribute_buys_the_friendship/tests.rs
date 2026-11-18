use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

#[test]
fn tribute_buys_the_friendship_is_opt_in() {
    opt_in_off_in_both_controllers("tribute-buys-the-friendship", |ai| {
        ai.tribute_buys_the_friendship
    });
}

#[test]
fn the_bridge_reads_the_flag() {
    let mut ai = AdvancedAi::new();
    assert!(!ai.tribute_buys_the_friendship_enabled());
    ai.enable_tribute_buys_the_friendship();
    assert!(ai.tribute_buys_the_friendship_enabled());
    ai.disable_tribute_buys_the_friendship();
    assert!(!ai.tribute_buys_the_friendship_enabled());
}

#[test]
fn tribute_buys_the_friendship_2_is_opt_in() {
    opt_in_off_in_both_controllers("tribute-buys-the-friendship-2", |ai| {
        ai.tribute_buys_the_friendship_2
    });
}

/// Version two carries version one's bridge and aims past the line; version
/// one alone does not.
#[test]
fn version_two_runs_the_bridge_and_aims_past_the_line() {
    let mut ai = AdvancedAi::new();
    ai.enable_tribute_buys_the_friendship();
    assert!(!ai.tribute_aims_past_the_line());
    ai.disable_tribute_buys_the_friendship();
    ai.enable_tribute_buys_the_friendship_2();
    assert!(ai.tribute_buys_the_friendship_enabled());
    assert!(ai.tribute_aims_past_the_line());
    ai.disable_tribute_buys_the_friendship_2();
    assert!(!ai.tribute_buys_the_friendship_enabled());
    assert!(!ai.tribute_aims_past_the_line());
}
