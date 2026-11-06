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
    camp_row_version(u8::from(gene), short, countdown_end)
}

/// `camp_row` under version 0 (off), 1, 2 or 3 of the family.
fn camp_row_version(version: u8, short: i64, countdown_end: Option<u32>) -> Objective {
    camp_row_at(version, short, countdown_end, at(15, 8))
}

/// The same board with the camp at `camp`.
fn camp_row_at(version: u8, short: i64, countdown_end: Option<u32>, camp: Pos) -> Objective {
    let mut g = flat_board(17, &[at(6, 8), at(30, 8)], true);
    g.barb_camps.insert(camp, g.turn);
    assert!(g.wdist(camp, at(6, 8)) <= CAMP_RADIUS);
    g.spawn_test_unit("warrior", 0, at(7, 8));
    g.spawn_test_unit("warrior", 0, at(7, 9));
    g.players[0].normal_age_threshold = 11;
    g.players[0].era_score = 11 - short;
    g.world_era_countdown_end = countdown_end;
    let mut ai = on();
    match version {
        1 => ai.enable_camps_close_the_age(),
        2 => ai.enable_camps_close_the_age_2(),
        3 => ai.enable_camps_close_the_age_3(),
        _ => {}
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

#[test]
fn version_two_ships_off_and_turns_version_one_off() {
    let ai = AdvancedAi::new();
    assert!(!ai.camps_close_the_age_2, "an opt-in ships off");
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "camps_close_the_age_2"));
    super::super::test_support::opt_in_off_in_both_controllers("camps-close-the-age-2", |ai| {
        ai.camps_close_the_age_2
    });
    let mut on = AdvancedAi::new();
    on.enable_camps_close_the_age();
    on.enable_camps_close_the_age_2();
    assert_eq!(
        (on.camps_close_the_age, on.camps_close_the_age_2),
        (false, true),
        "one version of the family plays"
    );
}

/// Version two keeps version one's premium, bodies and deadline, asks a
/// shooter, and widens the strength margin.
#[test]
fn version_two_arms_the_age_closing_party() {
    let v1 = camp_row_version(1, 2, Some(40));
    let v2 = camp_row_version(2, 2, Some(40));
    assert!((v2.value - v1.value).abs() < 1e-9, "the same premium");
    assert_eq!(v2.requirement.bodies, v1.requirement.bodies);
    assert_eq!(v2.deadline, v1.deadline);
    assert_eq!(v1.requirement.ranged, 0);
    assert_eq!(
        v2.requirement.ranged,
        super::super::camps_close_the_age::CAMP_AGE_SHOOTERS
    );
    assert!(
        (v2.requirement.strength
            - v1.requirement.strength * super::super::camps_close_the_age::CAMP_AGE_MARGIN)
            .abs()
            < 1e-9,
        "{} vs {}",
        v2.requirement.strength,
        v1.requirement.strength
    );
    // Not short: version two leaves the plain row too.
    let plain = camp_row_version(0, 0, Some(40));
    let idle = camp_row_version(2, 0, Some(40));
    assert_eq!(idle.requirement.ranged, plain.requirement.ranged);
    assert!((idle.requirement.strength - plain.requirement.strength).abs() < 1e-9);
}

#[test]
fn version_three_ships_off_and_turns_the_others_off() {
    let ai = AdvancedAi::new();
    assert!(!ai.camps_close_the_age_3, "an opt-in ships off");
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "camps_close_the_age_3"));
    super::super::test_support::opt_in_off_in_both_controllers("camps-close-the-age-3", |ai| {
        ai.camps_close_the_age_3
    });
    let mut on = AdvancedAi::new();
    on.enable_camps_close_the_age();
    on.enable_camps_close_the_age_2();
    on.enable_camps_close_the_age_3();
    assert_eq!(
        (
            on.camps_close_the_age,
            on.camps_close_the_age_2,
            on.camps_close_the_age_3
        ),
        (false, false, true),
        "one version of the family plays"
    );
}

/// At or over the Age bar, a near camp is still raised and armed under
/// version three, and only under version three; a far camp is not.
#[test]
fn version_three_clears_a_near_camp_in_any_era() {
    let near = at(12, 8);
    {
        let g = flat_board(17, &[at(6, 8), at(30, 8)], true);
        assert!(g.wdist(near, at(6, 8)) <= CAMP_NEAR_RADIUS, "a near camp");
    }
    let v2 = camp_row_at(2, 0, Some(40), near);
    let v3 = camp_row_at(3, 0, Some(40), near);
    assert!(
        (v3.value - v2.value - super::super::camps_close_the_age::CAMP_AGE_PREMIUM).abs() < 1e-9,
        "{} vs {}",
        v3.value,
        v2.value
    );
    assert_eq!(v2.requirement.ranged, 0, "version two waits for an Age gap");
    assert_eq!(
        v3.requirement.ranged,
        super::super::camps_close_the_age::CAMP_AGE_SHOOTERS
    );
    assert!(
        (v3.requirement.strength
            - v2.requirement.strength * super::super::camps_close_the_age::CAMP_AGE_MARGIN)
            .abs()
            < 1e-9
    );
    assert!(v3.deadline.is_none(), "no Age to date it to");
    let far_v2 = camp_row_version(2, 0, Some(40));
    let far_v3 = camp_row_version(3, 0, Some(40));
    assert!(
        (far_v3.value - far_v2.value).abs() < 1e-9,
        "a far camp keeps the plain row"
    );
    assert_eq!(far_v3.requirement.ranged, far_v2.requirement.ranged);
}
