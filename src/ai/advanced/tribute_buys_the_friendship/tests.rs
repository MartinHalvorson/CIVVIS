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
