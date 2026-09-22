use super::*;
use serde_json::{json, Value};

fn state(turn: u32, era: i64) -> Value {
    json!({"kind":"state", "run":"test", "turn":turn, "world_era":era,
        "boosted_techs":[], "boosted_civics":[], "cities":[{"id":1,"x":3,"y":3,"pop":4,"buildings":[],"districts":[]}],
        "dark_age":false,"golden_age":false,"heroic_golden_age":false,
        "dedication_choices":0})
}
fn log(rows: &[Value]) -> String {
    rows.iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}
fn history() -> Vec<Value> {
    let first = state(1, 0);
    let mut middle = state(20, 0);
    middle["boosted_techs"] = json!(["TECH_ASTROLOGY", "TECH_WRITING", "TECH_BRONZE_WORKING"]);
    middle["boosted_civics"] = json!(["CIVIC_EARLY_EMPIRE", "CIVIC_MYSTICISM"]);
    middle["cities"][0]["buildings"] = json!(["BUILDING_MONUMENT", "BUILDING_LIBRARY"]);
    middle["cities"][0]["districts"] = json!([
        {"type":"DISTRICT_CITY_CENTER","x":3,"y":3,"complete":true},
        {"type":"DISTRICT_CAMPUS","x":3,"y":4,"complete":true},
        {"type":"DISTRICT_HOLY_SITE","x":4,"y":4,"complete":false}
    ]);
    let mut boundary = middle.clone();
    boundary["turn"] = json!(31);
    boundary["world_era"] = json!(1);
    boundary["dedication_choices"] = json!(1);
    boundary["boosted_techs"]
        .as_array_mut()
        .unwrap()
        .push(json!("TECH_MACHINERY"));
    vec![first, middle.clone(), middle, boundary]
}

#[test]
fn native_dedication_history_counts_observed_completions_once_before_boundary() {
    let counts = through(&log(&history()), 31).unwrap();
    assert_eq!(counts.get("eureka"), Some(&3));
    assert_eq!(counts.get("inspiration"), Some(&2));
    assert_eq!(counts.get("culture_building"), Some(&1));
    assert_eq!(counts.get("science_building"), Some(&1));
    assert_eq!(counts.get("district"), Some(&1));
    assert!(!counts.contains_key("city_converted"));
}

#[test]
fn native_dedication_history_changes_the_real_choice_after_event_import() {
    let path = std::env::temp_dir().join(format!(
        "civvis-dedication-{}-{}.jsonl",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, log(&history())).unwrap();
    let observed = super::super::state_from_events(&path, Some(31)).unwrap();
    std::fs::remove_file(path).unwrap();
    let mut game = crate::game::Game::new_full(2, 12, 10, 3740, 150, 0, false);
    game.world_era = 1;
    let mut legacy = observed.clone();
    legacy.observed_dedication_activity = None;
    super::super::apply_player_ages(&mut game, &legacy);
    let mut control = game.clone();
    crate::ai::choose_dedications(&mut control, 0, crate::ai::DedicationChoice::Banking);
    assert!(control.players[0]
        .dedications
        .contains("exodus_of_the_evangelists"));
    super::super::apply_player_ages(&mut game, &observed);
    crate::ai::choose_dedications(&mut game, 0, crate::ai::DedicationChoice::Banking);
    assert!(game.players[0].dedications.contains("free_inquiry"));
}

#[test]
fn native_dedication_history_does_not_read_future_eras_or_invent_reload_activity() {
    let rows = history();
    let expected = through(&log(&rows), 31);
    let mut future = rows.last().unwrap().clone();
    future["turn"] = json!(61);
    future["world_era"] = json!(2);
    let mut extended = rows;
    extended.push(future.clone());
    assert_eq!(through(&log(&extended), 31), expected);
    assert_eq!(through(&log(&[future.clone()]), 61), None);
    let mut next = future.clone();
    next["turn"] = json!(91);
    next["world_era"] = json!(3);
    assert_eq!(through(&log(&[future, next]), 91), Some(Counts::new()));
}

#[test]
fn native_dedication_history_ignores_unknown_fields_and_first_seen_city_inventory() {
    let mut first = state(1, 0);
    first.as_object_mut().unwrap().remove("boosted_techs");
    first["cities"][0]
        .as_object_mut()
        .unwrap()
        .remove("buildings");
    first["cities"][0]["districts"] = json!([{"type":"DISTRICT_CAMPUS","x":3,"y":4}]);
    let mut later = state(2, 0);
    later["boosted_techs"] = json!(["TECH_WRITING"]);
    later["cities"] = json!([
        {"id":1,"buildings":["BUILDING_LIBRARY"],"districts":[{"type":"DISTRICT_CAMPUS","x":3,"y":4,"complete":true}]},
        {"id":2,"buildings":["BUILDING_LIBRARY"],"districts":[{"type":"DISTRICT_CAMPUS","x":7,"y":4,"complete":true}]}
    ]);
    let mut boundary = later.clone();
    boundary["world_era"] = json!(1);
    boundary["turn"] = json!(31);
    assert_eq!(
        through(&log(&[first, later, boundary]), 31),
        Some(Counts::new())
    );
}

#[test]
fn native_dedication_history_resets_at_new_runs_and_skipped_eras() {
    let mut rows = history();
    let mut other = state(1, 0);
    other["run"] = json!("other");
    rows.push(other);
    assert_eq!(through(&log(&rows), 31), None);
    let mut rows = history();
    rows.last_mut().unwrap()["world_era"] = json!(3);
    assert_eq!(through(&log(&rows), 31), None);
}

#[test]
fn native_dedication_history_uses_only_the_immediately_preceding_era() {
    let mut rows = history();
    let mut second = rows.last().unwrap().clone();
    second["turn"] = json!(40);
    second["dedication_choices"] = json!(0);
    second["boosted_techs"]
        .as_array_mut()
        .unwrap()
        .push(json!("TECH_ENGINEERING"));
    second["cities"][0]["buildings"]
        .as_array_mut()
        .unwrap()
        .push(json!("BUILDING_FACTORY"));
    rows.push(second.clone());
    second["turn"] = json!(61);
    second["world_era"] = json!(2);
    rows.push(second);
    let counts = through(&log(&rows), 61).unwrap();
    assert_eq!(counts.get("eureka"), Some(&1));
    assert_eq!(counts.get("industrial_building"), Some(&1));
    assert!(!counts.contains_key("inspiration"));
    assert!(!counts.contains_key("science_building"));
}

#[test]
fn native_dedication_history_does_not_credit_a_recaptured_citys_new_buildings() {
    let mut first = state(1, 0);
    first["cities"][0]["buildings"] = json!(["BUILDING_MONUMENT"]);
    let mut lost = state(2, 0);
    lost["cities"] = json!([]);
    let mut recaptured = first.clone();
    recaptured["turn"] = json!(3);
    recaptured["cities"][0]["buildings"] = json!(["BUILDING_MONUMENT", "BUILDING_LIBRARY"]);
    let mut next = recaptured.clone();
    next["turn"] = json!(4);
    let mut boundary = next.clone();
    boundary["turn"] = json!(31);
    boundary["world_era"] = json!(1);
    assert_eq!(
        through(&log(&[first, lost, recaptured, next, boundary]), 31),
        Some(Counts::new())
    );
}
