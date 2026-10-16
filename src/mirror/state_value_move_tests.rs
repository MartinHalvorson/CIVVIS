use super::*;

fn clone_pipeline(line: &str) -> serde_json::Result<StateSnapshot> {
    let value: serde_json::Value = serde_json::from_str(line)?;
    let mut state: StateSnapshot = serde_json::from_value(value.clone())?;
    state.schema_gaps = state_schema_gaps(&value);
    Ok(state)
}

#[test]
fn parsed_state_and_nested_schema_warnings_match_the_clone_pipeline() {
    let line = r#"{
        "turn":178,"frame":2,"science":123.5,"gold":201,
        "techs":["TECH_STEEL"],"future_top":{"nested":[1,2,3]},
        "cities":[{"id":7,"x":3,"y":4,"future_city":true}],
        "units":[{"id":9,"kind":"UNIT_ARTILLERY","x":3,"y":5,
                  "future_unit":[1,2]}],
        "rivals":[{"player":1,"future_rival":"unknown"}],
        "minors":[{"player":4,"future_minor":null}],
        "religions":[{"type":"RELIGION_CATHOLICISM","future_religion":1}]
    }"#;
    let expected = clone_pipeline(line).unwrap();
    let actual = state_from_json(line).unwrap();
    // Debug includes every typed field, including the NaN defaults that cannot
    // be compared with ordinary float equality. These structs use ordered maps.
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    assert_eq!(
        actual.schema_gaps,
        [
            "schema:city.future_city",
            "schema:minor.future_minor",
            "schema:religion.future_religion",
            "schema:rival.future_rival",
            "schema:state.future_top",
            "schema:unit.future_unit",
        ]
    );
    assert_eq!(actual.units[0].kind, "UNIT_ARTILLERY");
    assert_eq!(actual.turn, 178);
    assert_eq!(actual.frame, 2);
}

#[test]
fn sparse_and_duplicate_key_states_match_the_clone_pipeline() {
    for line in [
        r#"{"turn":1}"#,
        r#"{"turn":1,"turn":2,"future":1,"future":2}"#,
        r#"{"turn":3,"cities":[],"units":[],"rivals":[],"minors":[]}"#,
    ] {
        assert_eq!(
            format!("{:?}", state_from_json(line).unwrap()),
            format!("{:?}", clone_pipeline(line).unwrap())
        );
    }
}

#[test]
fn invalid_json_and_typed_state_errors_match_the_clone_pipeline() {
    for line in [
        "{",
        "null",
        "[]",
        "{}",
        r#"{"turn":"bad","future":true}"#,
        r#"{"turn":-1}"#,
        r#"{"turn":1,"cities":false}"#,
        r#"{"turn":1,"units":[{"id":"bad"}]}"#,
    ] {
        let expected = clone_pipeline(line).unwrap_err();
        let actual = state_from_json(line).unwrap_err();
        assert_eq!(actual.to_string(), expected.to_string());
        assert_eq!(actual.classify(), expected.classify());
        assert_eq!(
            (actual.line(), actual.column()),
            (expected.line(), expected.column())
        );
    }
}
