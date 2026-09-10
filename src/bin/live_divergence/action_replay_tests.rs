use super::*;

fn fixture_events() -> Vec<Value> {
    include_str!("../../../data/fixtures/player_transfer/isolated_movement_20260910.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn replay_events(events: &[Value]) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut cases = Vec::new();
    let text = events
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    replay(std::io::Cursor::new(text), |case| cases.push(case))?;
    Ok(cases)
}

#[test]
fn the_streaming_cli_parser_replays_all_ten_real_probes() {
    let cases = replay_events(&fixture_events()).unwrap();
    assert_eq!(cases.len(), 10);
    for case in cases {
        assert_eq!(case["same_turn"], true);
        assert_eq!(case["phase"], "settled");
        assert!(case.get("coverage_gap").is_none());
        assert_eq!(case["predictions"], case["observed"]);
    }
}

#[test]
fn a_missing_seat_cannot_silently_use_default_rules_for_a_settled_probe() {
    let mut events = fixture_events();
    events.retain(|event| event["kind"] != "seat");
    for case in replay_events(&events).unwrap() {
        assert!(case["coverage_gap"].is_string());
        assert!(case.get("predictions").is_none());
    }
}

#[test]
fn unsupported_rules_or_missing_identity_cannot_default_to_a_passing_model() {
    for (field, value) in [
        ("ruleset", json!("RULESET_STANDARD")),
        ("modes", json!(["heroes"])),
        ("speed", json!("GAMESPEED_UNKNOWN")),
        ("difficulty", json!("DIFFICULTY_UNKNOWN")),
        ("civ", json!("CIVILIZATION_UNKNOWN")),
        ("players", json!(0)),
    ] {
        let mut events = fixture_events();
        events
            .iter_mut()
            .find(|event| event["kind"] == "seat")
            .unwrap()[field] = value;
        assert!(replay_events(&events).is_err(), "{field}");
    }
}

#[test]
fn future_map_information_cannot_be_used_as_a_pre_action_snapshot() {
    let mut events = fixture_events();
    events
        .iter_mut()
        .find(|event| event["kind"] == "tiles")
        .unwrap()["turn"] = json!(200);
    assert!(replay_events(&events)
        .unwrap_err()
        .to_string()
        .contains("after the requested action"));
}

#[test]
fn distinct_but_reversed_transition_sequences_are_rejected() {
    let mut events = fixture_events();
    for event in &mut events {
        if event["sequence"] == 1 {
            event["sequence"] = json!(99);
        }
    }
    assert!(replay_events(&events)
        .unwrap_err()
        .to_string()
        .contains("out-of-order"));
}

#[test]
fn unchanged_location_and_moves_do_not_prove_fortification() {
    let mut events = fixture_events();
    let begin = events
        .iter()
        .position(|e| e["kind"] == "action_transition_begin")
        .unwrap();
    let subject = events[begin]["order"]["subject"].clone();
    events[begin]["order"]["verb"] = json!("FORTIFY");
    events[begin + 2] = events[begin + 1].clone();
    events[begin + 2]["kind"] = json!("action_transition_after");
    let unit = events[begin + 2]["units"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|u| u["id"] == subject)
        .unwrap();
    unit["moves"] = json!(0);
    unit["fortified"] = json!(false);
    let cases = replay_events(&events).unwrap();
    assert_eq!(cases[0]["predictions"]["fortified"], true);
    assert_eq!(cases[0]["observed"]["fortified"], false);
}

#[test]
fn city_count_alone_does_not_prove_that_the_requested_settler_founded() {
    let mut events = fixture_events();
    let begin = events
        .iter()
        .position(|e| e["kind"] == "action_transition_begin")
        .unwrap();
    let settler = events[begin + 1]["units"]
        .as_array()
        .unwrap()
        .iter()
        .find(|unit| unit["kind"] == "UNIT_SETTLER")
        .unwrap()
        .clone();
    let mut city = events
        .iter()
        .filter_map(|e| e["cities"].as_array())
        .find_map(|cities| cities.first())
        .unwrap()
        .clone();
    city["x"] = json!(settler["x"].as_i64().unwrap() + 5);
    city["y"] = settler["y"].clone();
    events[begin]["order"]["verb"] = json!("FOUND_CITY");
    events[begin]["order"]["subject"] = settler["id"].clone();
    events[begin + 2] = events[begin + 1].clone();
    events[begin + 2]["kind"] = json!("action_transition_after");
    events[begin + 2]["cities"] = json!([city]);
    let cases = replay_events(&events).unwrap();
    assert_eq!(
        cases[0]["predictions"]["cities"],
        cases[0]["observed"]["cities"]
    );
    assert_eq!(cases[0]["predictions"]["settler_present"], false);
    assert_eq!(cases[0]["observed"]["settler_present"], true);
    assert_eq!(cases[0]["predictions"]["city_at_settler"], true);
    assert_eq!(cases[0]["observed"]["city_at_settler"], false);
}

#[test]
fn out_of_order_duplicate_or_interleaved_evidence_fails_closed() {
    let original = fixture_events();
    let before = original
        .iter()
        .position(|e| e["kind"] == "action_transition_before")
        .unwrap();
    let after = before + 1;
    let begin = before - 1;
    for mutation in 0..7 {
        let mut events = original.clone();
        match mutation {
            0 => events.swap(before, after),
            1 => events.insert(before, events[before].clone()),
            2 => events.insert(after, json!({"kind": "state"})),
            3 => events.insert(after, json!({"kind": "orders"})),
            4 => {
                events[begin].as_object_mut().unwrap().remove("sequence");
            }
            5 => {
                events[begin].as_object_mut().unwrap().remove("frame");
            }
            6 => events.insert(after, json!({"kind": "seat"})),
            _ => unreachable!(),
        }
        assert!(
            replay_events(&events).is_err(),
            "mutation {mutation} was accepted"
        );
    }
    let mut duplicated = original.clone();
    duplicated.extend(original);
    assert!(replay_events(&duplicated).is_err());
}

#[test]
fn each_observation_must_belong_to_the_same_turn_and_frame() {
    for kind in [
        "action_transition_begin",
        "action_transition_before",
        "action_transition_after",
        "action_transition_end",
    ] {
        for field in ["turn", "frame"] {
            let mut events = fixture_events();
            let event = events.iter_mut().find(|e| e["kind"] == kind).unwrap();
            event[field] = json!(event[field].as_u64().unwrap() + 1);
            let cases = replay_events(&events).unwrap();
            assert_eq!(cases[0]["same_turn"], false);
            assert!(cases[0]["coverage_gap"].is_string());
            assert_eq!(cases[0]["predictions"], json!({}));
        }
    }
}

#[test]
fn ten_real_isolated_movements_match_destination_and_remaining_movement() {
    let fixture =
        include_str!("../../../data/fixtures/player_transfer/isolated_movement_20260910.jsonl");
    let mut snapshot = Snapshot::default();
    let mut seat = Seat::default();
    let mut begin = Value::Null;
    let mut before = StateSnapshot::default();
    let mut after = StateSnapshot::default();
    let mut preceding_map = Snapshot::default();
    let mut compared = 0;
    for line in fixture.lines() {
        let event: Value = serde_json::from_str(line).unwrap();
        match event["kind"].as_str().unwrap() {
            "seat" => seat = serde_json::from_value(event).unwrap(),
            "tiles" => {
                let chunk = serde_json::from_value(event.clone()).unwrap();
                if event["delta"] == true {
                    snapshot.merge_delta(&chunk);
                } else {
                    snapshot.merge_sweep(&chunk);
                }
            }
            "action_transition_begin" => {
                begin = event;
                preceding_map = snapshot.clone();
            }
            "action_transition_before" => {
                before = mirror::state_from_json(line).unwrap();
                before.seat = seat.clone();
            }
            "action_transition_after" => {
                after = mirror::state_from_json(line).unwrap();
                after.seat = seat.clone();
            }
            "action_transition_end" => {
                let case = compare(&begin, &before, &after, &event, &preceding_map);
                assert_eq!(case["phase"], "settled");
                assert_eq!(case["same_turn"], true);
                assert!(case.get("coverage_gap").is_none(), "{case}");
                assert_eq!(case["predictions"], case["observed"], "{case}");
                assert_eq!(case["predictions"].as_object().unwrap().len(), 2);
                compared += 1;
            }
            _ => (),
        }
    }
    assert_eq!(compared, 10);
}

#[test]
fn incomplete_probes_cannot_claim_settled_equivalence() {
    let mut snapshot = Snapshot::default();
    snapshot.width = 16;
    snapshot.height = 12;
    let state = StateSnapshot {
        turn: 1,
        ..Default::default()
    };
    let begin = json!({"sequence": 1, "turn": 1, "isolated": true,
        "order": {"kind": "research", "verb": "TECH_POTTERY"}});
    for end in [
        json!({"turn": 1, "phase": "settled", "isolated": true, "settled": false}),
        json!({"turn": 1, "phase": "settled", "settled": true}),
    ] {
        let case = compare(&begin, &state, &state, &end, &snapshot);
        assert_eq!(case["predictions"], json!({}));
        assert!(case["coverage_gap"].is_string());
    }
}

#[test]
fn research_is_replayed_by_the_engine_not_copied_from_the_observation() {
    let mut snapshot = Snapshot::default();
    snapshot.width = 16;
    snapshot.height = 12;
    let before = StateSnapshot {
        turn: 1,
        ..Default::default()
    };
    let mut after = before.clone();
    after.research = Some("TECH_MINING".into());
    let begin = json!({"sequence": 1, "turn": 1, "frame": 0,
        "order": {"kind": "research", "verb": "TECH_POTTERY"}});
    let end = json!({"sequence": 1, "turn": 1, "frame": 0, "accepted": true});
    let case = compare(&begin, &before, &after, &end, &snapshot);
    assert_eq!(case["predictions"]["research"], "pottery");
    assert_eq!(case["observed"]["research"], "mining");
    assert_eq!(case["phase"], "request_boundary");
}

#[test]
fn missing_map_and_unsupported_requests_never_create_passing_comparisons() {
    let state = StateSnapshot {
        turn: 1,
        ..Default::default()
    };
    let begin = json!({"sequence": 1, "turn": 1, "frame": 0, "order": {"kind": "preview", "verb": "ATTACK"}});
    let end = json!({"turn": 1, "frame": 0, "accepted": true});
    let mut snapshot = Snapshot::default();
    let no_map = compare(&begin, &state, &state, &end, &snapshot);
    assert_eq!(no_map["predictions"], json!({}));
    snapshot.width = 16;
    snapshot.height = 12;
    let unsupported = compare(&begin, &state, &state, &end, &snapshot);
    assert_eq!(unsupported["predictions"], json!({}));
    assert!(unsupported["coverage_gap"].is_string());
}
