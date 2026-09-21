use super::*;

#[test]
fn air_pillage_crosses_as_the_native_air_attack_operation() {
    let (snapshot, mut state) = tests::local_barbarian_defense_board();
    state.units.iter_mut().find(|u| u.id == 100).unwrap().kind = "UNIT_BOMBER".into();
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let unit = *mirror
        .civ6_of
        .iter()
        .find(|(_, native)| **native == 100)
        .unwrap()
        .0;
    // Native t154 repeatedly planned this mission without exporting an order.
    let target = civvis::hex::offset_to_axial(18, 20);
    let order = translate(&Action::AirPillage { unit, target }, &mirror, &state)
        .expect("strategic bombing must reach the native operation");
    assert_eq!(order.kind, "unit");
    assert_eq!(order.subject, Some(100));
    assert_eq!(order.verb.as_deref(), Some("AIR_ATTACK"));
    assert_eq!(order.pos, Some((18, 20)));
}

#[test]
fn air_pillage_without_a_native_aircraft_id_is_not_exported() {
    let (snapshot, state) = tests::local_barbarian_defense_board();
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    assert!(translate(
        &Action::AirPillage {
            unit: u32::MAX,
            target: (8, 20)
        },
        &mirror,
        &state,
    )
    .is_none());
}
