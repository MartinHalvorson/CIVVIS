use super::*;
use civvis::mirror::{StateCity, StateSnapshot};
use std::collections::BTreeMap;

fn state(frame: u32, producing: Option<&str>) -> StateSnapshot {
    StateSnapshot {
        turn: 54,
        frame,
        cities: vec![StateCity {
            id: 327684,
            name: "Jinju".to_string(),
            producing: producing.map(str::to_string),
            ..StateCity::default()
        }],
        ..StateSnapshot::default()
    }
}

fn allows_handoff(state: &StateSnapshot, city: i64, first: &str, next: &str) -> bool {
    super::same_turn_wall_handoff(
        state,
        city,
        &HostProductionLease {
            verb: first.to_string(),
            frame: 0,
        },
        next,
    )
}

fn produce(verb: &str) -> Order {
    Order {
        kind: "produce",
        subject: Some(327684),
        verb: Some(verb.to_string()),
        pos: None,
    }
}

#[test]
fn same_turn_wall_handoff_keeps_the_fresh_defense_and_its_ownership() {
    let mut refusals = HostMoveRefusals::default();
    let mut ours = BTreeMap::new();
    let hub = "DISTRICT_COMMERCIAL_HUB";
    let wall = "BUILDING_WALLS";
    let mut orders = vec![produce(hub)];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut orders, &state(0, None), &mut ours),
        0
    );

    // Native Jinju t54 f1: the Hub is now observed, and the planner has
    // requested Walls. The old lease dropped that handoff until t56.
    let mut orders = vec![produce(wall)];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut orders, &state(1, Some(hub)), &mut ours),
        0
    );
    assert_eq!(orders.len(), 1);
    assert_eq!(ours.get(&327684).map(String::as_str), Some(wall));

    // The new lease holds even while the host is still applying the Walls.
    for verb in [wall, "UNIT_GALLEY", hub] {
        let mut replay = vec![produce(verb)];
        assert_eq!(
            refusals.suppress_same_turn_replays(&mut replay, &state(2, Some(hub)), &mut ours),
            1
        );
        assert!(replay.is_empty());
        assert_eq!(ours.get(&327684).map(String::as_str), Some(wall));
    }
}

#[test]
fn same_turn_wall_handoff_requires_a_fresh_matching_host_readback() {
    let hub = "DISTRICT_COMMERCIAL_HUB";
    for observed in [
        state(0, Some(hub)),
        state(1, None),
        state(1, Some("UNIT_BUILDER")),
    ] {
        let mut refusals = HostMoveRefusals::default();
        let mut ours = BTreeMap::new();
        refusals.suppress_same_turn_replays(&mut vec![produce(hub)], &state(0, None), &mut ours);
        let mut walls = vec![produce("BUILDING_WALLS")];
        assert_eq!(
            refusals.suppress_same_turn_replays(&mut walls, &observed, &mut ours),
            1
        );
        assert!(walls.is_empty());
        assert_eq!(ours.get(&327684).map(String::as_str), Some(hub));
    }
    assert!(!allows_handoff(
        &state(1, Some(hub)),
        7,
        hub,
        "BUILDING_WALLS"
    ));
}

#[test]
fn same_turn_wall_handoff_leaves_routine_changes_and_wall_swaps_suppressed() {
    for (first, next) in [
        ("UNIT_BUILDER", "UNIT_ARCHER"),
        ("DISTRICT_COMMERCIAL_HUB", "UNIT_GALLEY"),
        ("BUILDING_WALLS", "BUILDING_CASTLE"),
        ("UNIT_BUILDER", "BUILDING_UNKNOWN_WALL"),
    ] {
        assert!(!allows_handoff(&state(1, Some(first)), 327684, first, next));
    }
}

#[test]
fn same_turn_wall_handoff_discovers_fortifications_and_their_host_names() {
    let rules = civvis::rules::Rules::shared();
    let mut checked = 0;
    for (name, spec) in &rules.buildings {
        if spec.outer_defense <= 0 {
            continue;
        }
        let host = civ6_building_type(name);
        assert!(
            allows_handoff(
                &state(1, Some("UNIT_BUILDER")),
                327684,
                "UNIT_BUILDER",
                &host
            ),
            "{name}: {host}"
        );
        checked += 1;
    }
    assert!(checked > 0, "the ruleset must expose its fortifications");
}

#[test]
fn same_turn_wall_handoff_replays_jinjus_actual_native_request_and_readback() {
    let evidence: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/eval/2026-11-05-same-turn-wall-handoff.native.json"
    )))
    .unwrap();
    let frame = |number| {
        evidence["observations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["turn"] == 54 && row["frame"] == number)
            .unwrap()
    };
    let first = frame(0);
    let next = frame(1);
    let state0: StateSnapshot = serde_json::from_value(first["state"].clone()).unwrap();
    let state1: StateSnapshot = serde_json::from_value(next["state"].clone()).unwrap();
    let native = first["original_reply"]["orders"]
        .as_array()
        .unwrap()
        .iter()
        .find(|order| order["kind"] == "produce" && order["subject"] == 327684)
        .unwrap();
    let first_verb = native["verb"].as_str().unwrap();
    assert_eq!(first_verb, "DISTRICT_COMMERCIAL_HUB");
    assert!(next["original_reply"]["orders"]
        .as_array()
        .unwrap()
        .iter()
        .all(|order| order["kind"] != "produce" || order["subject"] != 327684));
    let action = next["original_reply"]["decision"]["native_actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["type"] == "produce" && action["city"] == 54)
        .unwrap();
    let item: civvis::game::Item = serde_json::from_value(action["item"].clone()).unwrap();
    let requested = civ6_build_name(&item).unwrap();
    assert_eq!(requested, "BUILDING_WALLS");
    let mut refusals = HostMoveRefusals::default();
    let mut ours = BTreeMap::new();
    let mut first_orders = vec![produce(first_verb)];
    refusals.suppress_same_turn_replays(&mut first_orders, &state0, &mut ours);
    let mut handoff = vec![produce(&requested)];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut handoff, &state1, &mut ours),
        0
    );
    assert_eq!(handoff.len(), 1, "the recorded request must reach the host");
    assert_eq!(
        ours.get(&327684).map(String::as_str),
        Some("BUILDING_WALLS")
    );
}

#[test]
fn same_turn_wall_handoff_requires_a_frame_after_the_sent_request() {
    let mut refusals = HostMoveRefusals::default();
    let mut ours = BTreeMap::new();
    let hub = "DISTRICT_COMMERCIAL_HUB";
    let observed = state(2, Some(hub));
    refusals.suppress_same_turn_replays(&mut vec![produce(hub)], &observed, &mut ours);
    let mut stale = vec![produce("BUILDING_WALLS")];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut stale, &observed, &mut ours),
        1
    );
    assert!(stale.is_empty(), "a repeated positive frame is still stale");
    let mut fresh = vec![produce("BUILDING_WALLS")];
    assert_eq!(
        refusals.suppress_same_turn_replays(&mut fresh, &state(3, Some(hub)), &mut ours),
        0
    );
    assert_eq!(fresh.len(), 1);
}
