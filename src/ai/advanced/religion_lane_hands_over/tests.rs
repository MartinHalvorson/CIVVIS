use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::VictoryTarget;

/// Six majors, none founded, our seat on the Religion target.
fn open_race() -> (Game, AdvancedAi) {
    let g = Game::new_full(6, 60, 40, 936_411, 650, 0, false);
    let ai = AdvancedAi::targeting(VictoryTarget::Religion);
    (g, ai)
}

/// Rivals found until every slot is claimed.
fn close_the_race(g: &mut Game) {
    let slots = g.max_religions();
    for (index, rival) in (1..g.players.len()).take(slots).enumerate() {
        g.players[rival].religion = Some(format!("faith_{index}"));
    }
    assert!(
        g.religions_founded() >= slots,
        "fixture: the race is closed"
    );
}

#[test]
fn religion_lane_hands_over_is_opt_in() {
    opt_in_off_in_both_controllers("religion-lane-hands-over", |ai| ai.religion_lane_hands_over);
}

/// Every slot claimed by rivals: under the gene the seat retargets to
/// Science; off, it keeps the Religion target.
#[test]
fn a_religion_seat_that_lost_the_race_plays_science() {
    let (mut g, mut ai) = open_race();
    close_the_race(&mut g);
    assert!(AdvancedAi::religion_race_lost(&g, 0));
    ai.hand_over_a_lost_religion_lane(&g, 0);
    assert_eq!(ai.victory_target, Some(VictoryTarget::Religion), "off");
    ai.enable_religion_lane_hands_over();
    ai.hand_over_a_lost_religion_lane(&g, 0);
    assert_eq!(ai.victory_target, Some(VictoryTarget::Science));
}

/// An open slot, our own Prophet pending, or our own religion keeps the lane;
/// so does any other target.
#[test]
fn an_open_race_keeps_the_religion_lane() {
    let (g, mut ai) = open_race();
    ai.enable_religion_lane_hands_over();
    assert!(!AdvancedAi::religion_race_lost(&g, 0), "slots open");
    ai.hand_over_a_lost_religion_lane(&g, 0);
    assert_eq!(ai.victory_target, Some(VictoryTarget::Religion));

    let (mut g, mut ai) = open_race();
    ai.enable_religion_lane_hands_over();
    close_the_race(&mut g);
    g.players[0].prophet_pending = true;
    assert!(
        !AdvancedAi::religion_race_lost(&g, 0),
        "our Prophet pending"
    );

    let (mut g, _) = open_race();
    close_the_race(&mut g);
    let mut culture = AdvancedAi::targeting(VictoryTarget::Culture);
    culture.enable_religion_lane_hands_over();
    culture.hand_over_a_lost_religion_lane(&g, 0);
    assert_eq!(culture.victory_target, Some(VictoryTarget::Culture));
}
