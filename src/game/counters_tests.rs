use super::*;

fn mixed() -> Counters {
    let mut counters = Counters::default();
    *counters.entry("goody_huts_claimed".into()).or_insert(0) += 2;
    counters.insert("great_work:writing".into(), 1);
    *counters.entry("commit:made".into()).or_insert(0) += 3;
    counters.insert("raid_wars".into(), 1);
    counters
}

#[test]
fn the_rules_tallies_and_the_agents_notes_live_apart() {
    let counters = mixed();
    assert_eq!(
        counters.rules().keys().collect::<Vec<_>>(),
        ["goody_huts_claimed", "great_work:writing"]
    );
    assert_eq!(
        counters.notes().keys().collect::<Vec<_>>(),
        ["commit:made", "raid_wars"]
    );
    // One map to every caller.
    assert_eq!(counters["goody_huts_claimed"], 2);
    assert_eq!(counters.get("commit:made"), Some(&3));
    assert!(counters.contains_key("raid_wars"));
    assert_eq!(counters.len(), 4);
    let keys: Vec<&String> = counters.iter().map(|(key, _)| key).collect();
    assert_eq!(
        keys,
        [
            "commit:made",
            "goody_huts_claimed",
            "great_work:writing",
            "raid_wars"
        ]
    );
}

#[test]
fn a_save_writes_one_map_and_reads_it_back_apart() {
    let counters = mixed();
    let text = serde_json::to_string(&counters).unwrap();
    assert_eq!(
        text,
        r#"{"commit:made":3,"goody_huts_claimed":2,"great_work:writing":1,"raid_wars":1}"#
    );
    let back: Counters = serde_json::from_str(&text).unwrap();
    assert_eq!(back, counters);
    assert_eq!(back.notes().len(), 2);
}

#[test]
fn a_fork_shares_the_agents_notes_until_it_writes_one() {
    let counters = mixed();
    let mut fork = counters.clone();
    assert!(Arc::ptr_eq(&counters.notes, &fork.notes));
    *fork.entry("goody_huts_claimed".into()).or_insert(0) += 1;
    assert!(Arc::ptr_eq(&counters.notes, &fork.notes));
    *fork.entry("commit:made".into()).or_insert(0) += 1;
    assert!(!Arc::ptr_eq(&counters.notes, &fork.notes));
    assert_eq!(counters["commit:made"], 3);
    assert_eq!(fork["commit:made"], 4);
}

#[test]
fn rule_tallies_are_not_mistaken_for_notes() {
    for key in [
        "kills",
        "inquisition",
        "warrior",
        "diplomatic_favor",
        "great_work:relic",
        "historic_moment_awards:first_unique_unit",
        "project_effect:nuclear_devices",
        "purchased:rock_band",
        "strongest_unit_built",
    ] {
        assert!(!is_agent_note(key), "{key} is a rules tally");
    }
}
