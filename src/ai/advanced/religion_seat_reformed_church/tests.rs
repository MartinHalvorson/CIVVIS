use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

fn founder(target: VictoryTarget) -> (Game, AdvancedAi) {
    let mut g = Game::new_full(2, 40, 24, 936_314, 250, 0, false);
    g.players[0].religion = Some("Buddhism".into());
    g.players[0].civics.insert(crate::name!("theology"));
    (g, AdvancedAi::targeting(target))
}

#[test]
fn the_gene_is_opt_in() {
    opt_in_off_in_both_controllers("religion-seat-reaches-reformed-church", |ai| {
        ai.religion_seat_reaches_reformed_church
    });
}

/// A founder with Theology reaches for Reformed Church, and stops once it has it.
#[test]
fn a_founder_reaches_for_reformed_church() {
    let (mut g, mut ai) = founder(VictoryTarget::Religion);
    assert_eq!(ai.religion_seat_civic_goal(&g, 0), None, "gene off");
    ai.enable_religion_seat_reaches_reformed_church();
    assert_eq!(ai.religion_seat_civic_goal(&g, 0), Some("reformed_church"));
    g.players[0].civics.insert(crate::name!("reformed_church"));
    assert_eq!(ai.religion_seat_civic_goal(&g, 0), None, "already adopted");
}

/// Before Theology or a founding, and on another lane, there is no such goal.
#[test]
fn the_goal_waits_for_the_founding_and_the_lane() {
    let (mut g, mut ai) = founder(VictoryTarget::Religion);
    ai.enable_religion_seat_reaches_reformed_church();
    g.players[0].civics.remove(&crate::name!("theology"));
    assert_eq!(ai.religion_seat_civic_goal(&g, 0), None, "no Theology");

    let (mut g, mut ai) = founder(VictoryTarget::Religion);
    ai.enable_religion_seat_reaches_reformed_church();
    g.players[0].religion = None;
    assert_eq!(ai.religion_seat_civic_goal(&g, 0), None, "not founded");

    let (g, mut ai) = founder(VictoryTarget::Culture);
    ai.enable_religion_seat_reaches_reformed_church();
    assert_eq!(ai.religion_seat_civic_goal(&g, 0), None, "a Culture seat");
}
