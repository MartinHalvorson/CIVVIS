use super::*;

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
