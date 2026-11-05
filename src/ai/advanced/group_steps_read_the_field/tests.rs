use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::doctrine::{build, position};
use crate::hex;

fn open_field() -> Game {
    let mut g = build(position("the_reserve").expect("known"), 3).expect("buildable");
    let seeded: Vec<u32> = (0..2).flat_map(|pid| g.player_unit_ids(pid)).collect();
    for uid in seeded {
        g.remove_unit(uid);
    }
    g
}

fn at(col: i32, row: i32) -> Pos {
    hex::offset_to_axial(col, row)
}

#[test]
fn group_steps_read_the_field_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(!ai.group_steps_read_the_field, "an opt-in ships off");
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "group_steps_read_the_field"));
    opt_in_off_in_both_controllers("group-steps-read-the-field", |ai| {
        ai.group_steps_read_the_field
    });
}

/// A Horseman three tiles from the step is out of the one-step forecast's
/// reach (its range plus one approach step) and inside its own movement:
/// the field prices its blow, the forecast alone does not.
#[test]
fn a_fast_hostile_the_one_step_forecast_misses_is_read_by_the_field() {
    let mut g = open_field();
    let ours = g.spawn_unit("warrior", 0, at(10, 4));
    let step = at(10, 5);
    let rider = g.spawn_unit("horseman", 1, at(10, 8));
    let distance = g.wdist(step, g.units[&rider].pos);
    assert!(
        distance >= 3,
        "beyond range plus one approach step: {distance}"
    );
    let mut ai = AdvancedAi::new();
    let forecast = ai.base.projected_counter_damage(&g, ours, step, &[rider]);
    assert_eq!(forecast, 0.0, "the one-step forecast does not reach");
    assert_eq!(ai.group_step_danger(&g, 0, ours, step, forecast), forecast);
    ai.enable_group_steps_read_the_field();
    let read = ai.group_step_danger(&g, 0, ours, step, forecast);
    assert!(read > 0.0, "the field reads the rider's blow: {read}");
    // The frame's field is reused while the hostiles stand still ...
    assert_eq!(ai.group_step_danger(&g, 0, ours, step, forecast), read);
    // ... and rebuilt when one moves out of reach.
    g.remove_unit(rider);
    assert_eq!(ai.group_step_danger(&g, 0, ours, step, forecast), 0.0);
}
