use super::*;

fn path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "civvis-turn-hints-{tag}-{}.jsonl",
        std::process::id()
    ))
}

fn check(raw: &str) {
    for range in line_ranges_containing(raw, "") {
        for turn in [
            None,
            Some(0),
            Some(3),
            Some(4),
            Some(5),
            Some(7),
            Some(8),
            Some(9),
            Some(70),
            Some(u32::MAX),
        ] {
            assert_eq!(
                state_line_can_match_turn_in_log(raw, range, turn),
                state_line_can_match_turn(&raw[range.0..range.1], turn),
                "range {range:?}, turn {turn:?}, line {:?}",
                &raw[range.0..range.1]
            );
        }
    }
}

#[test]
fn turn_hints_preserve_ambiguous_nested_duplicate_and_numeric_headers() {
    let file = path("headers");
    let lines = [
        r#"{"kind":"state","turn":7}"#,
        "{\"kind\":\"state\",\"turn\" \t:\r 7,\"label\":\"α\"}",
        r#"{"kind":"state","turn":3,"nested":{"turn":7}}"#,
        r#"{"kind":"state","turn":3,"turn":7}"#,
        r#"{"kind":"state","tu\u0072n":7}"#,
        r#"{"kind":"state","tu\u0072n":8,"nested":{"turn":7}}"#,
        r#"{"kind":"state","turn":null}"#,
        r#"{"kind":"state","turn":"7"}"#,
        r#"{"kind":"state","turn":-7}"#,
        r#"{"kind":"state","turn":7.0}"#,
        r#"{"kind":"state","turn":7e0}"#,
        r#"{"kind":"state","turn":007}"#,
        r#"{"kind":"state","turn":18446744073709551616}"#,
        r#"{"kind":"state","turn":4294967295}"#,
        r#"{"kind":"state"}"#,
        "malformed α \"turn\":7",
    ];
    std::fs::write(&file, lines.join("\r\n")).unwrap();
    let raw = read_events(&file).unwrap();
    check(&raw);
    check(&raw);
    std::fs::remove_file(file).unwrap();
}

#[test]
fn turn_hints_follow_growth_rewrites_path_switches_and_old_buffers() {
    let a = path("growth-a");
    let b = path("growth-b");
    let first = "{\"kind\":\"state\",\"turn\":3}\n";
    std::fs::write(&a, first).unwrap();
    let old = read_events(&a).unwrap();
    check(&old);
    let grown = format!("{first}{{\"kind\":\"state\",\"turn\":4}}\n");
    std::fs::write(&a, &grown).unwrap();
    let appended = read_events(&a).unwrap();
    check(&appended);
    check(&old);
    let rewritten = grown.replace("\"turn\":4", "\"turn\":5");
    assert_eq!(rewritten.len(), grown.len());
    std::fs::write(&a, rewritten).unwrap();
    let replaced = read_events(&a).unwrap();
    check(&replaced);
    check(&appended);
    std::fs::write(&b, "{\"kind\":\"state\",\"turn\":9}\n").unwrap();
    let other = read_events(&b).unwrap();
    check(&other);
    check(&replaced);
    std::fs::write(&a, "{\"kind\":\"state\",\"turn\":7}\n").unwrap();
    check(&read_events(&a).unwrap());
    std::fs::remove_file(a).unwrap();
    std::fs::remove_file(b).unwrap();
}

#[test]
fn turn_hints_keep_confirmed_deaths_at_the_selected_state() {
    let file = path("deaths");
    let text = concat!(
        "{\"kind\":\"state\",\"turn\":3,\"frame\":0,\"gold\":5}\n",
        "{\"kind\":\"combat\",\"turn\":4,\"attacker_killed\":true,",
        "\"attacker\":{\"type\":\"unit\",\"player\":0,\"id\":17},",
        "\"defender\":{\"player\":1}}\n",
        "{\"kind\":\"state\",\"turn\":4,\"frame\":2,\"gold\":9}\n",
        "{\"kind\":\"state\",\"turn\":4,\"frame\":1,\"gold\":10}\n"
    );
    std::fs::write(&file, text).unwrap();
    let later = state_from_events(&file, Some(4)).unwrap();
    assert_eq!((later.turn, later.frame, later.gold), (4, 1, 10));
    assert_eq!(later.confirmed_unit_deaths.len(), 1);
    assert_eq!(later.confirmed_unit_deaths[0].unit, 17);
    assert_eq!(later.confirmed_unit_deaths[0].opponent, Some(1));
    let earlier = state_from_events(&file, Some(3)).unwrap();
    assert!(earlier.confirmed_unit_deaths.is_empty());
    let raw = read_events(&file).unwrap();
    assert_eq!(latest_state_range(&raw, Some(4)).unwrap().1, (4, 1));
    assert_eq!(state_from_events(&file, None).unwrap().frame, 1);
    std::fs::remove_file(file).unwrap();
}

#[test]
fn state_and_snapshot_selection_keep_duplicate_key_and_lookback_rules() {
    let file = path("selection");
    let text = concat!(
        "{\"kind\":\"state\",\"turn\":2,\"frame\":0}\n",
        "{\"kind\":\"state\",\"turn\":3,\"frame\":0,\"nested\":{\"turn\":7}}\n",
        "{\"kind\":\"state\",\"turn\":4,\"turn\":5,\"frame\":2}\n",
        "{\"kind\":\"state\",\"turn\":null,\"frame\":7}\n"
    );
    std::fs::write(&file, text).unwrap();
    for (turn, expected) in [
        (None, Some((5, 2))),
        (Some(2), Some((2, 0))),
        (Some(3), Some((3, 0))),
        (Some(4), None),
        (Some(5), Some((5, 2))),
        (Some(7), None),
    ] {
        let state = state_from_events(&file, turn).map(|s| (s.turn, s.frame));
        let raw = read_events(&file).unwrap();
        assert_eq!(state, expected);
        assert_eq!(
            latest_state_range(&raw, turn).map(|(_, stamp)| stamp),
            expected
        );
        check(&raw);
    }
    // An arbitrary raw string must not borrow hints from the current file.
    check("{\"kind\":\"state\",\"turn\":8}\r\n{\"turn\":7}");
    std::fs::remove_file(file).unwrap();
}
