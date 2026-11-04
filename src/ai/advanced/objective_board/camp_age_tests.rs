//! `camps-close-the-age`: ClearCamp rows close a first-era Age gap.

use super::tests::{at, conquest, flat_board, on};
use super::*;

fn row(ai: &AdvancedAi, key: ObjectiveKey) -> Option<&Objective> {
    ai.objective_board().rows.iter().find(|row| row.key == key)
}

/// A far camp (no near-camp body floor) on a board whose seat stands
/// `short` Era Score under its Normal threshold, read with and without the
/// gene. The world-era countdown ends at `countdown_end` when given.
fn camp_row(gene: bool, short: i64, countdown_end: Option<u32>) -> Objective {
    let mut g = flat_board(17, &[at(6, 8), at(30, 8)], true);
    let camp = at(15, 8);
    g.barb_camps.insert(camp, g.turn);
    assert!(g.wdist(camp, at(6, 8)) <= CAMP_RADIUS);
    assert!(g.wdist(camp, at(6, 8)) > CAMP_NEAR_RADIUS, "a far camp");
    g.spawn_test_unit("warrior", 0, at(7, 8));
    g.spawn_test_unit("warrior", 0, at(7, 9));
    g.players[0].normal_age_threshold = 11;
    g.players[0].era_score = 11 - short;
    g.world_era_countdown_end = countdown_end;
    let mut ai = on();
    if gene {
        ai.enable_camps_close_the_age();
    }
    let plan = conquest(&g, None);
    ai.rebuild_force_groups(&g, 0, &plan);
    row(&ai, ObjectiveKey::Camp(camp))
        .expect("a ClearCamp row")
        .clone()
}

#[test]
fn camps_close_the_age_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(!ai.camps_close_the_age, "an opt-in ships off");
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "camps_close_the_age"));
    super::super::test_support::opt_in_off_in_both_controllers("camps-close-the-age", |ai| {
        ai.camps_close_the_age
    });
}

/// Two short of a Normal Age: the row gains an Age's worth, asks two bodies
/// and takes the era's end as its deadline. Without the gene it is the
/// plain far-camp row.
#[test]
fn a_seat_two_short_raises_the_camp_and_dates_it_to_the_era() {
    let plain = camp_row(false, 2, Some(40));
    let raised = camp_row(true, 2, Some(40));
    assert_eq!(plain.requirement.bodies, 0, "a far camp asks no bodies");
    assert!(plain.deadline.is_none());
    assert!(
        (raised.value - plain.value - super::super::camps_close_the_age::CAMP_AGE_PREMIUM).abs()
            < 1e-9,
        "an Age's worth: {} vs {}",
        raised.value,
        plain.value
    );
    assert_eq!(
        raised.requirement.bodies,
        super::super::camps_close_the_age::CAMP_AGE_BODIES
    );
    let g = flat_board(17, &[at(6, 8), at(30, 8)], true);
    assert_eq!(raised.deadline, Some(40u32.saturating_sub(g.turn).max(1)));
    // With no countdown yet the row keeps no deadline but is still raised.
    let early = camp_row(true, 2, None);
    assert!(early.deadline.is_none());
    assert!((early.value - raised.value).abs() < 1e-9);
}

/// At or over the bar, or further from it than two camps reach, nothing
/// changes.
#[test]
fn a_seat_at_the_bar_or_far_from_it_keeps_the_plain_row() {
    for short in [0, -3, super::super::camps_close_the_age::CAMP_AGE_SHORT + 1] {
        let plain = camp_row(false, short, Some(40));
        let gene = camp_row(true, short, Some(40));
        assert!(
            (gene.value - plain.value).abs() < 1e-9,
            "short {short}: value unchanged"
        );
        assert_eq!(gene.requirement.bodies, plain.requirement.bodies);
        assert_eq!(gene.deadline, plain.deadline);
    }
}

/// Past the Medieval era a destroyed camp scores no Moment, so the gene
/// stands down even two short.
#[test]
fn past_the_medieval_era_a_camp_closes_no_age() {
    let mut g = flat_board(17, &[at(6, 8), at(30, 8)], true);
    g.players[0].normal_age_threshold = 11;
    g.players[0].era_score = 9;
    let mut ai = AdvancedAi::new();
    ai.enable_camps_close_the_age();
    assert_eq!(ai.camp_age_short(&g, 0), Some(2));
    g.world_era = 4;
    assert_eq!(
        ai.camp_age_short(&g, 0),
        None,
        "the Moment does not score in the Renaissance"
    );
}
