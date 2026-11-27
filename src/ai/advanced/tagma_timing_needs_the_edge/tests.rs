use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use std::sync::Arc;

#[test]
fn tagma_timing_needs_the_edge_is_opt_in() {
    opt_in_off_in_both_controllers("tagma-timing-needs-the-edge", |ai| {
        ai.tagma_timing_needs_the_edge
    });
}

/// The elective war's own bar: over 1.8 times the weakest rival plus 20, and
/// never against nobody.
#[test]
fn the_edge_is_the_elective_war_bar() {
    assert!(!tagma_has_the_edge(66.0, 120.0));
    assert!(!tagma_has_the_edge(236.0, 120.0));
    assert!(tagma_has_the_edge(237.0, 120.0));
    assert!(!tagma_has_the_edge(10_000.0, f64::INFINITY));
}

/// A founded Byzantine Religion seat at the reviewed games' power (66 against
/// 377, 101919Z t31). Shipped, "Tagma timing is live" takes the plan to
/// Conquest; with the gene it stays off Conquest until the army clears the
/// bar, and then the timing fires as before.
#[test]
fn a_weak_tagma_army_leaves_the_religion_lane_its_plan() {
    let base = || {
        let mut game = Game::new_full(2, 24, 16, 111, 200, 0, false);
        game.players[0].civ = "Byzantium".to_string();
        game.players[0].religion = Some("Eastern Orthodoxy".to_string());
        game.record_contact(0, 1);
        let powers = Arc::make_mut(&mut game.observed_military_power);
        powers.insert(0, 66.0);
        powers.insert(1, 377.0);
        game
    };
    let game = base();
    assert!(game.has_ability(0, "taxis"));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
    assert_eq!(
        ai.assess(&game, 0).strategy,
        GrandStrategy::Conquest,
        "off, the Tagma timing takes the plan whatever the army"
    );
    ai.enable_tagma_timing_needs_the_edge();
    assert_ne!(
        ai.assess(&game, 0).strategy,
        GrandStrategy::Conquest,
        "on, 66 against 377 is no timing"
    );
    let mut strong = base();
    Arc::make_mut(&mut strong.observed_military_power).insert(0, 2_000.0);
    assert_eq!(
        ai.assess(&strong, 0).strategy,
        GrandStrategy::Conquest,
        "on, an army over the bar keeps the timing"
    );
}
