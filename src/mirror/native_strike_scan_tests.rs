use super::*;

#[test]
fn overlapping_refusal_words_keep_exact_kind_turn_and_air_exemption() {
    let path = std::env::temp_dir().join(format!(
        "civvis-native-strike-words-{}.jsonl",
        std::process::id()
    ));
    let rows = [
        r#"{"kind":"range_attack_refused","turn":40,"unit":7,"x":5,"y":6,"why":"war_refused"}"#,
        r#"{"kind":"war_refused","turn":40,"unit":8,"x":4,"y":3,"why":"range_attack_refused"}"#,
        r#"{"kind":"combat","turn":40,"unit":9,"x":2,"y":1,"why":"range_attack_refused war_refused"}"#,
        "malformed range_attack_refused war_refused",
        r#"{"kind":"war_refused","turn":40,"target":3}"#,
        r#"{"kind":"range_attack_refused","turn":40,"unit":10,"x":2,"y":1,"verb":"AIR_ATTACK"}"#,
        r#"{"kind":"range_attack_refused","turn":39,"unit":11,"x":2,"y":1}"#,
        r#"{"kind":"war_refused","turn":40,"unit":"12","x":2,"y":1}"#,
        r#"{"kind":"war_refused","kind":"range_attack_refused","turn":40,"unit":13,"x":2,"y":1}"#,
    ];
    std::fs::write(&path, rows.join("\r\n")).unwrap();
    assert_eq!(
        refused_strikes_on(&path, 40),
        [(7, 5, 6), (8, 4, 3), (13, 2, 1)].into()
    );
    assert_eq!(refused_strikes_on(&path, 39), [(11, 2, 1)].into());
    assert!(refused_strikes_on(&path, 41).is_empty());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn refusal_reads_follow_growth_rewrites_and_turn_lookback() {
    let path = std::env::temp_dir().join(format!(
        "civvis-native-strike-growth-{}.jsonl",
        std::process::id()
    ));
    let mut text = r#"{"kind":"range_attack_refused","turn":70,"unit":1,"x":2,"y":3}"#.to_string();
    std::fs::write(&path, &text).unwrap();
    assert_eq!(refused_strikes_on(&path, 70), [(1, 2, 3)].into());
    text.push_str(concat!(
        "\r\n",
        r#"{"kind":"war_refused","turn":71,"unit":2,"x":3,"y":4}"#,
        "\r\n"
    ));
    std::fs::write(&path, &text).unwrap();
    assert_eq!(refused_strikes_on(&path, 71), [(2, 3, 4)].into());
    text.push_str(concat!(
        r#"{"kind":"range_attack_refused","turn":71,"unit":3,"x":4,"y":5}"#,
        "\n"
    ));
    std::fs::write(&path, &text).unwrap();
    assert_eq!(refused_strikes_on(&path, 71), [(2, 3, 4), (3, 4, 5)].into());
    assert_eq!(refused_strikes_on(&path, 70), [(1, 2, 3)].into());
    let rewritten = text.replace("\"unit\":3", "\"unit\":4");
    assert_eq!(rewritten.len(), text.len());
    std::fs::write(&path, rewritten).unwrap();
    assert_eq!(refused_strikes_on(&path, 71), [(2, 3, 4), (4, 4, 5)].into());
    std::fs::remove_file(path).unwrap();
}
