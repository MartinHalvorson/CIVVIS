use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

#[test]
fn science_specializes_earlier_is_opt_in() {
    opt_in_off_in_both_controllers("science-specializes-earlier", |ai| {
        ai.science_specializes_earlier
    });
}

/// Turn 90 of 250 is past a third and before half: the Science lane
/// specializes only with the gene on, and turn 80 is still the development
/// period either way.
#[test]
fn science_specializes_at_a_third_of_the_clock() {
    let mut game = Game::new_full(2, 24, 16, 91_013, 250, 0, false);
    game.turn = 90;
    let stock = AdvancedAi::targeting(VictoryTarget::Science);
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_science_specializes_earlier();
    assert!(
        !stock.phase_specialization_active(&game),
        "off: turn 90 is the development half"
    );
    assert!(
        science.phase_specialization_active(&game),
        "on: turn 90 of 250 is past a third"
    );
    game.turn = 80;
    assert!(
        !science.phase_specialization_active(&game),
        "turn 80 is before a third"
    );
    game.turn = 130;
    assert!(
        stock.phase_specialization_active(&game),
        "the halfway clock still stands"
    );
}

/// Only the Science lane's clock moves; a Religion seat handed to Science
/// moves with it.
#[test]
fn only_the_science_lane_moves() {
    let mut game = Game::new_full(2, 24, 16, 91_013, 250, 0, false);
    game.turn = 90;
    for target in [VictoryTarget::Domination, VictoryTarget::Religion] {
        let mut ai = AdvancedAi::targeting(target);
        let off = ai.phase_specialization_active(&game);
        ai.enable_science_specializes_earlier();
        assert_eq!(
            ai.phase_specialization_active(&game),
            off,
            "{target:?} unchanged"
        );
    }
    let mut handed = AdvancedAi::targeting(VictoryTarget::Religion);
    handed.enable_science_specializes_earlier();
    handed.retarget(VictoryTarget::Science);
    assert!(
        handed.phase_specialization_active(&game),
        "a handed-over seat follows Science"
    );
}
