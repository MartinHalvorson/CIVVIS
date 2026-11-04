use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{GrandStrategy, VictoryTarget};

/// Two majors, our seat founded, no spreader afield and no Faith banked: the
/// shape a Religion-target founder has while its plan reads Expansion.
fn founded(target: VictoryTarget) -> (Game, AdvancedAi) {
    let mut g = Game::new_full(2, 40, 24, 936_311, 650, 0, false);
    g.players[0].religion = Some("buddhism".to_string());
    g.players[0].faith = 0.0;
    let ai = AdvancedAi::targeting(target);
    (g, ai)
}

#[test]
fn religion_lane_spreads_from_founding_is_opt_in() {
    opt_in_off_in_both_controllers("religion-lane-spreads-from-founding", |ai| {
        ai.religion_lane_spreads_from_founding
    });
}

/// After founding the plan reads Expansion; off, the posture is defensive
/// with no spreader afield and no bank, and under the gene a Religion-target
/// seat stays offensive.
#[test]
fn a_religion_target_founder_keeps_spreading_in_an_expansion_plan() {
    let (g, mut ai) = founded(VictoryTarget::Religion);
    assert!(
        g.victory_conditions.religious,
        "fixture: religious victory on"
    );
    assert!(
        !ai.religious_offensive_posture(&g, 0, GrandStrategy::Expansion),
        "off: the Expansion plan idles the corps"
    );
    ai.enable_religion_lane_spreads_from_founding();
    assert!(ai.religion_lane_spreads(&g, 0));
    assert!(ai.religious_offensive_posture(&g, 0, GrandStrategy::Expansion));
}

/// Before founding there is no religion to spread, and on any other target
/// the gene reads nothing.
#[test]
fn only_a_founded_religion_target_reads_the_gene() {
    let (mut g, mut ai) = founded(VictoryTarget::Religion);
    ai.enable_religion_lane_spreads_from_founding();
    g.players[0].religion = None;
    assert!(!ai.religion_lane_spreads(&g, 0), "not yet founded");

    let (g, mut ai) = founded(VictoryTarget::Culture);
    ai.enable_religion_lane_spreads_from_founding();
    assert!(!ai.religion_lane_spreads(&g, 0), "a Culture seat");
    assert!(!ai.religious_offensive_posture(&g, 0, GrandStrategy::Expansion));
}
