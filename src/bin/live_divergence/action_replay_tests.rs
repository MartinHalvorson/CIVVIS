use super::*;

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
    let begin = json!({"sequence": 1, "turn": 1,
        "order": {"kind": "research", "verb": "TECH_POTTERY"}});
    let end = json!({"sequence": 1, "turn": 1, "accepted": true});
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
    let begin = json!({"sequence": 1, "turn": 1, "order": {"kind": "preview", "verb": "ATTACK"}});
    let end = json!({"turn": 1, "accepted": true});
    let mut snapshot = Snapshot::default();
    let no_map = compare(&begin, &state, &state, &end, &snapshot);
    assert_eq!(no_map["predictions"], json!({}));
    snapshot.width = 16;
    snapshot.height = 12;
    let unsupported = compare(&begin, &state, &state, &end, &snapshot);
    assert_eq!(unsupported["predictions"], json!({}));
    assert!(unsupported["coverage_gap"].is_string());
}
