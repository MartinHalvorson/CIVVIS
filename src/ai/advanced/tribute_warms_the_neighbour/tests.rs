use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

#[test]
fn tribute_warms_the_neighbour_is_opt_in() {
    opt_in_off_in_both_controllers("tribute-warms-the-neighbour", |ai| {
        ai.tribute_warms_the_neighbour
    });
}

#[test]
fn the_bridge_reads_the_neighbour_flag() {
    let mut ai = AdvancedAi::new();
    assert!(!ai.tribute_warms_the_neighbour_enabled());
    ai.enable_tribute_warms_the_neighbour();
    assert!(ai.tribute_warms_the_neighbour_enabled());
    ai.disable_tribute_warms_the_neighbour();
    assert!(!ai.tribute_warms_the_neighbour_enabled());
}
