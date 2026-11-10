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
    assert_eq!(
        ai.victory_target,
        Some(VictoryTarget::Religion),
        "the first turn's reading only starts the wait"
    );
    g.turn += 1;
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

#[test]
fn native_exhaustion_hands_over_before_the_last_rival_founds() {
    let (mut g, mut ai) = open_race();
    g.players[0].live_great_person_exhausted = Some(["prophet".to_string()].into_iter().collect());
    assert!(g.religions_founded() < g.max_religions());
    assert!(AdvancedAi::religion_race_lost(&g, 0));
    ai.hand_over_a_lost_religion_lane(&g, 0);
    assert_eq!(ai.victory_target, Some(VictoryTarget::Religion), "flag off");
    ai.enable_religion_lane_hands_over();
    ai.hand_over_a_lost_religion_lane(&g, 0);
    assert_eq!(
        ai.victory_target,
        Some(VictoryTarget::Religion),
        "the first turn's reading only starts the wait"
    );
    g.turn += 1;
    ai.hand_over_a_lost_religion_lane(&g, 0);
    assert_eq!(ai.victory_target, Some(VictoryTarget::Science));
}

#[test]
fn missing_or_other_exhaustion_does_not_close_the_prophet_race() {
    for exhausted in [
        None,
        Some(BTreeSet::new()),
        Some(["scientist".to_string()].into_iter().collect()),
    ] {
        let (mut g, mut ai) = open_race();
        g.players[0].live_great_person_exhausted = exhausted;
        ai.enable_religion_lane_hands_over();
        assert!(!AdvancedAi::religion_race_lost(&g, 0));
        ai.hand_over_a_lost_religion_lane(&g, 0);
        assert_eq!(ai.victory_target, Some(VictoryTarget::Religion));
    }
}

#[test]
fn native_exhaustion_keeps_our_pending_prophet_or_founded_religion() {
    for founded in [false, true] {
        let (mut g, mut ai) = open_race();
        g.players[0].live_great_person_exhausted =
            Some(["prophet".to_string()].into_iter().collect());
        if founded {
            g.players[0].religion = Some("our_faith".to_string());
        } else {
            g.players[0].prophet_pending = true;
        }
        ai.enable_religion_lane_hands_over();
        assert!(!AdvancedAi::religion_race_lost(&g, 0));
        ai.hand_over_a_lost_religion_lane(&g, 0);
        assert_eq!(ai.victory_target, Some(VictoryTarget::Religion));
    }
}

/// Live t19: the race read lost for one frame as our own Prophet arrived, and
/// the Prophet was pending the next turn.
#[test]
fn a_reading_gone_by_the_next_turn_keeps_the_lane() {
    let (mut g, mut ai) = open_race();
    ai.enable_religion_lane_hands_over();
    close_the_race(&mut g);
    ai.hand_over_a_lost_religion_lane(&g, 0);
    ai.hand_over_a_lost_religion_lane(&g, 0);
    assert_eq!(
        ai.victory_target,
        Some(VictoryTarget::Religion),
        "a second frame of the same turn is not a later turn"
    );
    g.turn += 1;
    g.players[0].prophet_pending = true;
    ai.hand_over_a_lost_religion_lane(&g, 0);
    assert_eq!(ai.victory_target, Some(VictoryTarget::Religion));
    assert_eq!(ai.religion_race_lost_since, None, "the wait starts over");

    // The next lost reading must wait a turn of its own.
    g.turn += 1;
    g.players[0].prophet_pending = false;
    ai.hand_over_a_lost_religion_lane(&g, 0);
    assert_eq!(ai.victory_target, Some(VictoryTarget::Religion));
    g.turn += 1;
    ai.hand_over_a_lost_religion_lane(&g, 0);
    assert_eq!(ai.victory_target, Some(VictoryTarget::Science));
}
