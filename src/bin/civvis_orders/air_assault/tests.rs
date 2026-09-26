use super::*;
use civvis::mirror::TilesChunk;

fn order(unit: i64, verb: &str, pos: (i32, i32)) -> Order {
    Order {
        kind: "unit",
        subject: Some(unit),
        verb: Some(verb.into()),
        pos: Some(pos),
    }
}

fn fixture(visible: bool) -> (Vec<Order>, AirCityAssault, Snapshot, BTreeMap<u32, i64>) {
    let target = (11, 8);
    let plan = AirCityAssault {
        target: civvis::hex::offset_to_axial(target.0, target.1),
        cavalry: 7,
        spot: civvis::hex::offset_to_axial(9, 8),
        moved_to_spot: true,
        aircraft: vec![20, 21],
    };
    let chunk: TilesChunk = serde_json::from_value(serde_json::json!({
        "turn": 150, "width": 40, "height": 24,
        "plots": [{"x":11,"y":8,"vis":visible}]
    }))
    .unwrap();
    let orders = vec![
        order(7, "MOVE_TO", (9, 8)),
        order(20, "AIR_ATTACK", target),
        order(21, "AIR_ATTACK", target),
        order(7, "MOVE_TO", (10, 8)),
        order(7, "ATTACK", target),
        order(50, "MOVE_TO", (12, 8)),
    ];
    (
        orders,
        plan,
        Snapshot::from_chunks(&[chunk]),
        BTreeMap::from([(7, 7), (20, 20), (21, 21)]),
    )
}

#[test]
fn a_remembered_city_waits_for_the_observed_spot() {
    let (mut orders, plan, snapshot, mapped) = fixture(false);
    assert_eq!(
        defer_followups(&mut orders, Some(&plan), &snapshot, &mapped),
        5
    );
    assert_eq!(orders.len(), 2);
    assert_eq!(orders[0].pos, Some((9, 8)));
    assert_eq!(orders[1].kind, "observe");
}

#[test]
fn visible_city_receives_bombs_before_any_speculative_follow_up() {
    let (mut orders, plan, snapshot, mapped) = fixture(true);
    assert_eq!(
        defer_followups(&mut orders, Some(&plan), &snapshot, &mapped),
        3
    );
    assert_eq!(orders.len(), 4);
    assert!(orders[1..3]
        .iter()
        .all(|order| order.verb.as_deref() == Some("AIR_ATTACK")));
    assert_eq!(orders[3].kind, "observe");
}

#[test]
fn the_post_volley_frame_releases_capture_or_retreat() {
    let (mut orders, mut plan, snapshot, mapped) = fixture(true);
    plan.aircraft.clear();
    assert_eq!(
        defer_followups(&mut orders, Some(&plan), &snapshot, &mapped),
        0
    );
    assert_eq!(orders.len(), 6);
}

#[test]
fn legacy_seats_do_not_invent_a_numeric_replan_budget() {
    let old: civvis::mirror::Seat =
        serde_json::from_value(serde_json::json!({"replan_frames":true})).unwrap();
    let new: civvis::mirror::Seat =
        serde_json::from_value(serde_json::json!({"replan_frames":true,"replan_frame_limit":2}))
            .unwrap();
    assert_eq!(old.replan_frame_limit, None);
    assert_eq!(new.replan_frame_limit, Some(2));
    let mut replay = old;
    super::super::assume_seat_capabilities(&mut replay, &["replan_frame_limit=2".into()]).unwrap();
    assert_eq!(replay.replan_frame_limit, Some(2));
    assert!(super::super::assume_seat_capabilities(
        &mut replay,
        &["replan_frame_limit=unknown".into()]
    )
    .is_err());
}

#[test]
fn a_native_cooldown_is_visible_before_the_assault_is_planned() {
    let mut g = civvis::game::Game::new_full(2, 40, 24, 377301, 300, 0, false);
    let settler = g
        .units
        .values()
        .find(|unit| unit.owner == 1 && unit.kind == "settler")
        .unwrap()
        .id;
    g.current = 1;
    g.apply(1, &civvis::game::Action::FoundCity { unit: settler })
        .unwrap();
    let target = g.cities.values().find(|city| city.owner == 1).unwrap().pos;
    let uid = g
        .units
        .values()
        .find(|unit| unit.owner == 0 && unit.kind == "warrior")
        .unwrap()
        .id;
    g.units.get_mut(&uid).unwrap().kind = civvis::name!("bomber");
    g.at_war.insert((0, 1));
    let mut refusals = super::super::HostOrderRefusals::default();
    refusals.seen.insert(
        (
            "unit".into(),
            Some("AIR_ATTACK".into()),
            Some(20),
            Some(civvis::hex::axial_to_offset(target.0, target.1)),
        ),
        super::super::RefusalRecord {
            strikes: 3,
            reason: "cannot_air_attack".into(),
            until: Some(151),
        },
    );
    let mapped = BTreeMap::from([(uid, 20)]);
    apply_cooldowns(&mut g, &mapped, 150, &refusals);
    assert!(g.strike_blocked(uid, target));
    g.blocked_strikes = Default::default();
    apply_cooldowns(&mut g, &mapped, 151, &refusals);
    assert!(!g.strike_blocked(uid, target));
}
