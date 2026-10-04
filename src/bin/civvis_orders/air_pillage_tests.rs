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

#[test]
fn a_terrain_bombing_cooldown_reaches_the_planning_board_and_expires() {
    let (snapshot, mut state) = tests::local_barbarian_defense_board();
    state
        .units
        .iter_mut()
        .find(|unit| unit.id == 100)
        .unwrap()
        .kind = "UNIT_BOMBER".into();
    let mut mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let uid = *mirror
        .civ6_of
        .iter()
        .find(|(_, host)| **host == 100)
        .unwrap()
        .0;
    let target = (8, 8);
    let mut refusals = HostOrderRefusals::default();
    refusals.seen.insert(
        (
            "unit".into(),
            Some("AIR_ATTACK".into()),
            Some(100),
            Some(target),
        ),
        RefusalRecord {
            strikes: 3,
            reason: "host_refused_strike".into(),
            until: Some(210),
        },
    );
    let axial = civvis::hex::offset_to_axial(target.0, target.1);
    assert!(
        mirror.game.map.get(axial).is_some(),
        "the planning board knows this plot"
    );
    assert!(
        mirror.game.city_at(axial).is_none(),
        "this is not a city bombing target"
    );
    air_assault::apply_cooldowns(&mut mirror.game, &mirror.civ6_of, 209, &refusals);
    assert!(mirror.game.strike_blocked(uid, axial));
    mirror.game.blocked_strikes = Default::default();
    air_assault::apply_cooldowns(&mut mirror.game, &mirror.civ6_of, 210, &refusals);
    assert!(!mirror.game.strike_blocked(uid, axial));
}

#[test]
fn bombing_cooldowns_keep_exact_unit_verb_target_and_observation_semantics() {
    let (snapshot, mut state) = tests::local_barbarian_defense_board();
    state
        .units
        .iter_mut()
        .find(|unit| unit.id == 100)
        .unwrap()
        .kind = "UNIT_BOMBER".into();
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let uid = *mirror
        .civ6_of
        .iter()
        .find(|(_, host)| **host == 100)
        .unwrap()
        .0;
    let target = (8, 8);
    let axial = civvis::hex::offset_to_axial(target.0, target.1);
    for (kind, verb, subject, pos, until) in [
        ("unit", "RANGE_ATTACK", Some(100), Some(target), Some(210)),
        ("unit", "AIR_ATTACK", Some(101), Some(target), Some(210)),
        ("unit", "AIR_ATTACK", Some(100), None, Some(210)),
        ("unit", "AIR_ATTACK", Some(100), Some(target), None),
        ("unit", "AIR_ATTACK", Some(100), Some(target), Some(209)),
        (
            "city_strike",
            "AIR_ATTACK",
            Some(100),
            Some(target),
            Some(210),
        ),
    ] {
        let mut game = mirror.game.clone();
        let mut refusals = HostOrderRefusals::default();
        refusals.seen.insert(
            (kind.into(), Some(verb.into()), subject, pos),
            RefusalRecord {
                strikes: 3,
                reason: "host_refused_strike".into(),
                until,
            },
        );
        air_assault::apply_cooldowns(&mut game, &mirror.civ6_of, 209, &refusals);
        assert!(
            !game.strike_blocked(uid, axial),
            "{kind} {verb} {subject:?} {pos:?} {until:?}"
        );
    }
}

#[test]
fn bombing_cooldowns_follow_matching_failures_verification_and_expiry() {
    let (snapshot, mut state) = tests::local_barbarian_defense_board();
    state
        .units
        .iter_mut()
        .find(|unit| unit.id == 100)
        .unwrap()
        .kind = "UNIT_BOMBER".into();
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let uid = *mirror
        .civ6_of
        .iter()
        .find(|(_, host)| **host == 100)
        .unwrap()
        .0;
    let target = civvis::hex::offset_to_axial(8, 8);
    assert!(mirror.game.map.get(target).is_some());
    let check = |verdict| OrderCheck {
        order: IssuedOrder {
            kind: "unit".into(),
            subject: Some(100),
            verb: Some("AIR_ATTACK".into()),
            pos: Some((8, 8)),
        },
        verdict,
    };
    let mut refusals = HostOrderRefusals::default();
    let blocked = |refusals: &HostOrderRefusals, turn| {
        let mut game = mirror.game.clone();
        air_assault::apply_cooldowns(&mut game, &mirror.civ6_of, turn, refusals);
        game.strike_blocked(uid, target)
    };
    for _ in 0..2 {
        refusals.observe(&[check(Verdict::Failed("host_refused_strike".into()))], 200);
        assert!(
            !blocked(&refusals, 200),
            "one or two failures must allow a retry"
        );
    }
    refusals.observe(&[check(Verdict::Unverifiable)], 200);
    assert!(
        !blocked(&refusals, 200),
        "ambiguous observations must not count"
    );
    refusals.observe(&[check(Verdict::Failed("host_refused_strike".into()))], 200);
    assert!(blocked(&refusals, 200));
    assert!(blocked(&refusals, 209));
    refusals.observe(&[check(Verdict::Unverifiable)], 205);
    assert!(blocked(&refusals, 209));
    assert!(
        !blocked(&refusals, 210),
        "expiry is a host turn, not a replan frame"
    );
    refusals.observe(&[check(Verdict::Verified)], 205);
    assert!(
        !blocked(&refusals, 205),
        "observed success clears the exact cooldown"
    );
    for _ in 0..2 {
        refusals.observe(&[check(Verdict::Failed("host_refused_strike".into()))], 211);
    }
    refusals.observe(&[check(Verdict::Failed("target_unharmed".into()))], 211);
    assert!(
        !blocked(&refusals, 211),
        "different failure causes do not accumulate"
    );
}
