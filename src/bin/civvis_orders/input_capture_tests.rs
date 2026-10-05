#[test]
fn default_reply_is_byte_identical_without_capture() {
    let reply = "{ \"turn\": 1, \"orders\": [] }\n".to_string();
    assert_eq!(super::attach_input_capture(reply.clone(), None), reply);
}

#[test]
fn optional_capture_does_not_change_planned_orders_or_decision() {
    let directory =
        std::env::temp_dir().join(format!("civvis-reply-capture-{}", std::process::id()));
    let capture = civvis::mirror::input_capture::InputCapture::create(
        &directory,
        &directory.join("absent.jsonl"),
    )
    .unwrap();
    let request = capture.begin().unwrap();
    assert!(civvis::mirror::read_events(&directory.join("absent.jsonl")).is_err());
    let expected = serde_json::json!({"turn": 12, "orders": [{"verb": "MOVE_TO"}],
        "decision": {"schema": 1, "turn": 12, "frame": 0, "native_actions": []}});
    let mut observed: serde_json::Value = serde_json::from_str(&super::attach_input_capture(
        expected.to_string(),
        Some(request),
    ))
    .unwrap();
    let descriptor = observed
        .as_object_mut()
        .unwrap()
        .remove("input_capture")
        .unwrap();
    assert_eq!(expected, observed);
    assert_eq!(descriptor["complete"], true);
    assert!(descriptor["reads"][0]["error"].is_string());
    // finish detached the request before the next one.
    assert!(capture.begin().is_ok());
    drop(capture);
    std::fs::remove_dir_all(directory).unwrap();
}
