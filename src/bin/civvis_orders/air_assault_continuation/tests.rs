use super::*;

fn state(turn: u32, frame: u32, owner: usize, at_war: bool) -> StateSnapshot {
    serde_json::from_value(serde_json::json!({
        "turn":turn, "frame":frame, "rivals":[{
            "player":owner, "at_war":at_war, "cities":[{
                "id":42, "name":"Sheffield", "x":26, "y":23, "pop":6,
                "damage":140, "defense":67, "wall_damage":400
            }]
        }]
    }))
    .unwrap()
}

fn pending() -> Continuation {
    Continuation {
        pending: Some(Pending {
            turn: 182,
            frame: 0,
            target: (26, 23),
            owner: 1,
        }),
    }
}

#[test]
fn fresh_spotting_frame_keeps_native_identity_not_a_model_city_id() {
    let mut memory = pending();
    assert_eq!(memory.take_target(&state(182, 1, 1, true)), Some((15, 23)));
    assert!(
        memory.pending.is_none(),
        "each barrier owns just its next observation"
    );
}

#[test]
fn another_turn_ownership_change_or_peace_releases_custody() {
    for observed in [
        state(183, 0, 1, true),
        state(182, 1, 2, true),
        state(182, 1, 1, false),
    ] {
        let mut memory = pending();
        assert_eq!(memory.take_target(&observed), None);
        assert!(memory.pending.is_none());
    }
}

#[test]
fn same_or_older_frame_does_not_count_as_the_required_observation() {
    let mut memory = pending();
    assert_eq!(memory.take_target(&state(182, 0, 1, true)), None);
    assert!(memory.pending.is_some());
    assert_eq!(memory.take_target(&state(182, 1, 1, true)), Some((15, 23)));
}

#[test]
fn a_native_capture_or_missing_city_releases_custody() {
    let mut memory = pending();
    let mut observed = state(182, 1, 1, true);
    observed.rivals[0].cities.clear();
    assert_eq!(memory.take_target(&observed), None);
    assert!(memory.pending.is_none());
}

#[test]
fn observed_own_city_wins_over_a_stale_rival_city_record() {
    let mut memory = pending();
    let mut observed = state(182, 1, 1, true);
    observed.cities.push(observed.rivals[0].cities[0].clone());
    assert_eq!(memory.take_target(&observed), None);
    assert!(memory.pending.is_none());
}

#[test]
fn absence_of_an_issued_barrier_cannot_create_a_continuation() {
    let mut memory = pending();
    memory.record(&state(182, 0, 1, true), None, &[]);
    assert!(memory.pending.is_none());
}

#[test]
fn only_an_issued_observation_records_the_observed_native_owner() {
    let report = AirCityAssault {
        target: (15, 23),
        cavalry: 7,
        spot: (13, 23),
        moved_to_spot: true,
        aircraft: vec![20],
    };
    let observation = Order {
        kind: "observe",
        subject: None,
        verb: Some("AIR_ASSAULT".into()),
        pos: None,
    };
    let mut memory = Continuation::default();
    memory.record(&state(182, 0, 1, true), Some(&report), &[observation]);
    assert_eq!(memory.take_target(&state(182, 1, 1, true)), Some((15, 23)));
    assert!(memory.pending.is_none());
    memory.record(&state(182, 1, 1, true), Some(&report), &[]);
    assert!(memory.pending.is_none());
}
