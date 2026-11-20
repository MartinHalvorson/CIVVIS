use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

#[test]
fn tribute_and_friendship_land_is_opt_in() {
    opt_in_off_in_both_controllers("tribute-and-friendship-land", |ai| {
        ai.tribute_and_friendship_land
    });
}

#[test]
fn the_tribute_is_priced_by_the_world_era_only_with_the_gene() {
    let mut ai = AdvancedAi::new();
    assert!(!ai.tribute_and_friendship_land_enabled());
    assert_eq!(
        ai.tribute_gold_per_point(Some(0)),
        None,
        "off: the bridge keeps ten"
    );
    ai.enable_tribute_and_friendship_land();
    assert!(ai.tribute_and_friendship_land_enabled());
    assert_eq!(ai.tribute_gold_per_point(Some(0)), Some(17.0));
    assert_eq!(ai.tribute_gold_per_point(Some(1)), Some(15.0));
    assert_eq!(ai.tribute_gold_per_point(Some(2)), Some(13.0));
    assert_eq!(ai.tribute_gold_per_point(Some(3)), Some(11.0));
    assert_eq!(
        ai.tribute_gold_per_point(Some(8)),
        Some(11.0),
        "every later era"
    );
    assert_eq!(ai.tribute_gold_per_point(Some(-1)), Some(17.0));
    assert_eq!(
        ai.tribute_gold_per_point(None),
        None,
        "no era read keeps ten"
    );
    ai.disable_tribute_and_friendship_land();
    assert_eq!(ai.tribute_gold_per_point(Some(3)), None);
}
