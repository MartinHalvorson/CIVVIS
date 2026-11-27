use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

#[test]
fn religion_lane_outlasts_the_denial_is_opt_in() {
    opt_in_off_in_both_controllers("religion-lane-outlasts-the-denial", |ai| {
        ai.religion_lane_outlasts_the_denial
    });
}

/// Only a founded Religion lane asked for a Science counter keeps its plan:
/// a Conquest counter, another lane, or a lane still racing for its religion
/// reads as before.
#[test]
fn only_a_founded_religion_lane_outlasts_a_science_counter() {
    let mut game = Game::new_full(2, 40, 28, 7_931, 250, 0, false);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
    let religion = Some(VictoryTarget::Religion);
    game.players[0].religion = Some("Home Faith".to_string());
    assert!(
        !ai.religion_lane_outlasts_the_denial_for(&game, 0, religion, GrandStrategy::Science),
        "off, nothing moves"
    );
    ai.enable_religion_lane_outlasts_the_denial();
    assert!(ai.religion_lane_outlasts_the_denial_for(&game, 0, religion, GrandStrategy::Science));
    assert!(!ai.religion_lane_outlasts_the_denial_for(&game, 0, religion, GrandStrategy::Conquest));
    assert!(!ai.religion_lane_outlasts_the_denial_for(
        &game,
        0,
        Some(VictoryTarget::Domination),
        GrandStrategy::Science
    ));
    game.players[0].religion = None;
    assert!(!ai.religion_lane_outlasts_the_denial_for(&game, 0, religion, GrandStrategy::Science));
}
