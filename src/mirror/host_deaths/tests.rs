use super::*;
use serde_json::json;

fn combat(turn: u32) -> serde_json::Value {
    json!({"kind":"combat", "turn":turn, "defender_killed":true,
        "defender":{"type":"unit", "player":63, "id":851980}})
}

#[test]
fn only_confirmed_unit_deaths_are_recorded() {
    let mut deaths = HostDeaths::default();
    let mut event = combat(9);
    event["defender_killed"] = json!(false);
    deaths.observe(&event.to_string());
    event["defender_killed"] = json!(true);
    event["defender"]["type"] = json!("city");
    deaths.observe(&event.to_string());
    event["defender"]["type"] = json!("district");
    deaths.observe(&event.to_string());
    deaths.observe("not JSON");
    assert!(deaths.through(12).is_empty());
    deaths.observe(&combat(9).to_string());
    assert_eq!(
        deaths.through(12),
        vec![HostUnitDeath {
            player: 63,
            unit: 851980,
            turn: 9
        }]
    );
    assert!(deaths.through(8).is_empty());
}

#[test]
fn attacker_and_defender_ids_are_scoped_to_their_owners() {
    let mut deaths = HostDeaths::default();
    let mut event = combat(9);
    event["attacker_killed"] = json!(true);
    event["attacker"] = json!({"type":"unit", "player":0, "id":851980});
    deaths.observe(&event.to_string());
    assert_eq!(deaths.through(9).len(), 2);
}

#[test]
fn death_evidence_stops_at_the_selected_state_frame() {
    let path =
        std::env::temp_dir().join(format!("civvis-host-deaths-{}.jsonl", std::process::id()));
    let rows = [
        json!({"kind":"state", "turn":8, "frame":0}),
        combat(9),
        json!({"kind":"state", "turn":12, "frame":0}),
        combat(12),
    ];
    let text = rows
        .iter()
        .map(|row| format!("{row}\n"))
        .collect::<String>();
    std::fs::write(&path, &text).unwrap();
    let selected = crate::mirror::state_from_events(&path, Some(12)).unwrap();
    assert_eq!(
        selected.confirmed_unit_deaths[0].turn, 9,
        "a later combat in this turn is not known to frame zero"
    );
    assert!(crate::mirror::state_from_events(&path, Some(8))
        .unwrap()
        .confirmed_unit_deaths
        .is_empty());
    std::fs::write(
        &path,
        format!("{text}{}\n", json!({"kind":"state", "turn":12, "frame":1})),
    )
    .unwrap();
    let selected = crate::mirror::state_from_events(&path, Some(12)).unwrap();
    assert_eq!(selected.frame, 1);
    assert_eq!(selected.confirmed_unit_deaths[0].turn, 12);
    std::fs::remove_file(path).unwrap();
}
