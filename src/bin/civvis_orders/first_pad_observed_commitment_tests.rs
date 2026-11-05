use super::*;
use std::collections::{BTreeMap, BTreeSet};

fn produce(city: i64, verb: &str, pos: Option<(i32, i32)>) -> Order {
    Order {
        kind: "produce",
        subject: Some(city),
        verb: Some(verb.to_string()),
        pos,
    }
}

#[test]
fn discarded_forge_pad_does_not_release_a_slow_first_native_pad() {
    // Native 151/0: Daegu can place the pad in 22 turns; Seoul needs 56.
    // Daegu's final repair must stay, without treating its discarded pad as
    // an executed placement that releases Seoul from the producer floor.
    let mut orders = vec![
        produce(196610, "DISTRICT_SPACEPORT", Some((54, 31))),
        produce(196610, "DISTRICT_AQUEDUCT", None),
        produce(458758, "DISTRICT_SPACEPORT", Some((49, 35))),
    ];
    let allowed = BTreeSet::from([65536, 196610]);
    let previous = BTreeMap::new();
    let mut ours = BTreeMap::from([
        (196610, "DISTRICT_AQUEDUCT".into()),
        (458758, "DISTRICT_SPACEPORT".into()),
    ]);
    assert_eq!(
        keep_observed_first_pad_floor(&mut orders, &allowed, &previous, &mut ours),
        1
    );
    let state = civvis::mirror::StateSnapshot::default();
    let mut memory = HostMoveRefusals::default();
    memory.suppress_same_turn_replays(&mut orders, &state, &mut ours);
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].subject, Some(196610));
    assert_eq!(orders[0].verb.as_deref(), Some("DISTRICT_AQUEDUCT"));
    assert!(!ours.contains_key(&458758));
    assert!(!memory.same_turn_production_orders.contains_key(&458758));
}

#[test]
fn a_held_pad_restores_previous_ownership_without_creating_a_host_lease() {
    let previous = BTreeMap::from([(2, "UNIT_BUILDER".into())]);
    let mut ours = BTreeMap::from([(2, "DISTRICT_SPACEPORT".into())]);
    let mut orders = vec![produce(2, "DISTRICT_SPACEPORT", Some((4, 6)))];
    assert_eq!(
        keep_observed_first_pad_floor(&mut orders, &BTreeSet::from([1]), &previous, &mut ours),
        1
    );
    assert!(orders.is_empty());
    assert_eq!(ours, previous);
}

#[test]
fn defense_repair_and_a_real_producer_keep_their_orders() {
    let mut orders = vec![
        produce(1, "DISTRICT_SPACEPORT", Some((4, 6))),
        produce(2, "BUILDING_WALLS", None),
        produce(3, "DISTRICT_SPACEPORT", None), // Existing repair/resume request.
        produce(4, "UNIT_ARCHER", None),
    ];
    assert_eq!(
        keep_observed_first_pad_floor(
            &mut orders,
            &BTreeSet::from([1]),
            &BTreeMap::new(),
            &mut BTreeMap::new()
        ),
        0
    );
    assert_eq!(orders.len(), 4);
}

#[test]
fn a_remaining_routine_head_keeps_the_current_choice_for_the_lease_filter() {
    let mut orders = vec![
        produce(2, "DISTRICT_SPACEPORT", Some((4, 6))),
        produce(2, "UNIT_ARCHER", None),
    ];
    let mut ours = BTreeMap::from([(2, "UNIT_ARCHER".into())]);
    keep_observed_first_pad_floor(
        &mut orders,
        &BTreeSet::from([1]),
        &BTreeMap::new(),
        &mut ours,
    );
    assert_eq!(ours.get(&2).map(String::as_str), Some("UNIT_ARCHER"));
    assert_eq!(orders.len(), 1);
}
