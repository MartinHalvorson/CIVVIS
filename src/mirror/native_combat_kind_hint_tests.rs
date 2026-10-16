use super::*;

type Selected = (u32, u32, Vec<host_deaths::HostUnitDeath>);

fn path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "civvis-combat-hints-{tag}-{}.jsonl",
        std::process::id()
    ))
}

fn check(raw: &str) {
    for range in line_ranges_containing(raw, "") {
        assert_eq!(
            combat_line_may_match_in_log(raw, range),
            json_may_have_value(&raw[range.0..range.1], "\"kind\"", "\"combat\""),
            "line {:?}",
            &raw[range.0..range.1]
        );
    }
}

#[test]
fn combat_hints_preserve_the_exact_lexical_predicate() {
    let file = path("lexical");
    let lines = [
        r#"{"kind":"combat","turn":7}"#,
        "{\"kind\" \t:\r \"combat\",\"label\":\"α\"}",
        r#"{"kind":"state","units":[{"combat":20}]}"#,
        r#"{"kind":"state","nested":{"kind":"combat"}}"#,
        r#"{"kind":"combat","kind":"state"}"#,
        r#"{"kind":"state","kind":"combat"}"#,
        r#"{"ki\u006ed":"combat"}"#,
        r#"{"kind":"com\u0062at"}"#,
        r#"{"kind":"combatant"}"#,
        r#"{"kind":null,"combat":20}"#,
        r#"{"kind":["combat"]}"#,
        r#"{"label":"\"kind\":\"combat\""}"#,
        "malformed α \"kind\":\"combat\"",
        "\"combat\" without a kind",
        "",
    ];
    std::fs::write(&file, lines.join("\r\n")).unwrap();
    let raw = read_events(&file).unwrap();
    check(&raw);
    check(&raw);
    check("{\"kind\":\"combat\"}\n{\"kind\":\"state\"}");
    std::fs::remove_file(file).unwrap();
}

#[test]
fn combat_hints_follow_append_path_switch_and_old_raw_buffers() {
    let a = path("append-a");
    let b = path("append-b");
    let first = "{\"kind\":\"state\",\"combat\":20}\n";
    std::fs::write(&a, first).unwrap();
    let old = read_events(&a).unwrap();
    check(&old);
    std::fs::write(&a, format!("{first}{{\"kind\":\"combat\"}}\n")).unwrap();
    let grown = read_events(&a).unwrap();
    check(&grown);
    check(&old);
    std::fs::write(&b, "{\"kind\":\"combat\"}\n").unwrap();
    check(&read_events(&b).unwrap());
    check(&grown);
    check(&read_events(&a).unwrap());
    std::fs::remove_file(a).unwrap();
    std::fs::remove_file(b).unwrap();
}

#[test]
fn combat_hints_follow_replacement_shrink_and_partial_append() {
    let file = path("rewrite");
    let first = "{\"kind\":\"combat\"}\n";
    std::fs::write(&file, first).unwrap();
    let old = read_events(&file).unwrap();
    check(&old);
    let replacement = "{\"kind\":\"state \"}\n";
    assert_eq!(first.len(), replacement.len());
    std::fs::write(&file, replacement).unwrap();
    let replaced = read_events(&file).unwrap();
    check(&replaced);
    check(&old);
    std::fs::write(&file, "{}\n").unwrap();
    check(&read_events(&file).unwrap());
    std::fs::write(&file, "{}\n{\"kind\":\"comb").unwrap();
    let partial = read_events(&file).unwrap();
    assert_eq!(partial.as_str(), "{}\n");
    check(&partial);
    std::fs::write(&file, "{}\n{\"kind\":\"combat\"}\n").unwrap();
    check(&read_events(&file).unwrap());
    check(&replaced);
    std::fs::remove_file(file).unwrap();
}

fn old_selected(raw: &str, turn: Option<u32>) -> Option<Selected> {
    let mut deaths = host_deaths::HostDeaths::default();
    let mut best: Option<Selected> = None;
    for line in raw.lines() {
        deaths.observe(line);
        if !line.contains("\"state\"") || !state_line_can_match_turn(line, turn) {
            continue;
        }
        let Ok(state) = state_from_json(line) else {
            continue;
        };
        if turn.is_some_and(|wanted| state.turn != wanted) {
            continue;
        }
        if best.as_ref().is_none_or(|(old, _, _)| state.turn >= *old) {
            best = Some((state.turn, state.frame, deaths.through(state.turn)));
        }
    }
    best
}

#[test]
fn combat_hints_keep_deaths_at_the_selected_frame_and_turn() {
    let file = path("selection");
    let text = concat!(
        "{\"kind\":\"state\",\"turn\":3,\"frame\":0,\"units\":[]}\n",
        "{\"kind\":\"combat\",\"turn\":4,\"attacker_killed\":true,\"attacker\":{\"type\":\"unit\",\"player\":0,\"id\":17},\"defender\":{\"player\":1}}\n",
        "{\"kind\":\"state\",\"turn\":4,\"frame\":2}\n",
        "{\"kind\":\"combat\",\"turn\":4,\"defender_killed\":true,\"defender\":{\"type\":\"unit\",\"player\":1,\"id\":17},\"attacker\":{\"player\":0}}\n",
        "{\"kind\":\"state\",\"turn\":4,\"frame\":1}\n"
        , "{\"kind\":\"combat\",\"turn\":4,\"attacker_killed\":true,\"attacker\":{\"type\":\"unit\",\"player\":0,\"id\":33}}\n"
    );
    for input in [
        text.to_owned(),
        format!("{text}{{\"kind\":\"state\",\"turn\":5,\"frame\":0}}\n"),
        text.to_owned(),
    ] {
        std::fs::write(&file, &input).unwrap();
        for turn in [Some(3), Some(4), Some(5), None] {
            for _ in 0..2 {
                let observed = state_from_events(&file, turn)
                    .map(|s| (s.turn, s.frame, s.confirmed_unit_deaths));
                assert_eq!(observed, old_selected(&input, turn));
            }
        }
    }
    std::fs::remove_file(file).unwrap();
}
