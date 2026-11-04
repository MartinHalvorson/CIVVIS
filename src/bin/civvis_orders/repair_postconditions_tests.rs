use super::*;
use civvis::mirror::{StateCity, StateDistrict};

fn check(city: StateCity, before: StateCity, kind: &str, verb: &str) -> Verdict {
    let (tiles, mut initial) = tests::local_barbarian_defense_board();
    initial.turn = 160;
    initial.units.clear();
    initial.cities = vec![before];
    let mut after = initial.clone();
    after.turn += 1;
    let subject = city.id;
    after.cities = vec![city];
    verify_order(
        &IssuedOrder {
            kind: kind.into(),
            subject: Some(subject),
            verb: Some(verb.into()),
            pos: None,
        },
        initial.turn,
        &initial,
        &after,
        &tiles,
        &[],
    )
}

fn pillaged_city() -> StateCity {
    StateCity {
        id: 7,
        name: "Repair fixture".into(),
        x: 4,
        y: 4,
        producing: Some("UNIT_WARRIOR".into()),
        buildings: vec!["BUILDING_WORKSHOP".into()],
        pillaged_buildings: Some(vec!["BUILDING_WORKSHOP".into()]),
        districts: vec![serde_json::from_value::<StateDistrict>(serde_json::json!({
            "type": "DISTRICT_INDUSTRIAL_ZONE",
            "x": 4, "y": 5, "complete": true, "pillaged": true
        }))
        .unwrap()],
        ..StateCity::default()
    }
}

#[test]
fn native_pillaged_predecessors_do_not_verify_a_repair() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/fixtures/native_repair_postconditions.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let before: StateCity = serde_json::from_value(case["before"].clone()).unwrap();
        let after: StateCity = serde_json::from_value(case["after"].clone()).unwrap();
        let result = check(after, before, "produce", case["verb"].as_str().unwrap());
        if case["expected_postcondition"] == "verified" {
            assert_eq!(result, Verdict::Verified, "native repaired control: {case}");
        } else {
            assert!(
                matches!(result, Verdict::Failed(_)),
                "still pillaged with another item queued: {} t{} {}",
                case["run"],
                case["turn"],
                case["verb"]
            );
        }
    }
}

#[test]
fn a_still_pillaged_building_with_another_queue_is_not_verified() {
    let city = pillaged_city();
    assert!(matches!(
        check(city.clone(), city, "produce", "BUILDING_WORKSHOP"),
        Verdict::Failed(_)
    ));
}

#[test]
fn a_queued_district_repair_is_verified_before_it_finishes() {
    let before = pillaged_city();
    let mut after = before.clone();
    after.producing = Some("DISTRICT_INDUSTRIAL_ZONE".into());
    assert_eq!(
        check(after, before, "produce", "DISTRICT_INDUSTRIAL_ZONE"),
        Verdict::Verified
    );
}

#[test]
fn a_queued_building_repair_is_verified_before_it_finishes() {
    let before = pillaged_city();
    let mut after = before.clone();
    after.producing = Some("BUILDING_WORKSHOP".into());
    assert_eq!(
        check(after, before, "produce", "BUILDING_WORKSHOP"),
        Verdict::Verified
    );
}

#[test]
fn an_observed_repaired_building_is_verified_after_its_queue_clears() {
    let before = pillaged_city();
    let mut after = before.clone();
    after.pillaged_buildings = Some(vec![]);
    assert_eq!(
        check(after, before, "produce", "BUILDING_WORKSHOP"),
        Verdict::Verified
    );
}

#[test]
fn an_intact_new_district_foundation_still_proves_placement() {
    let mut before = pillaged_city();
    before.districts.clear();
    let mut after = before.clone();
    after.districts.push(
        serde_json::from_value::<StateDistrict>(serde_json::json!({
            "type": "DISTRICT_CAMPUS", "x": 4, "y": 5,
            "complete": false, "pillaged": false
        }))
        .unwrap(),
    );
    assert_eq!(
        check(after, before, "produce", "DISTRICT_CAMPUS"),
        Verdict::Verified
    );
}

#[test]
fn a_pillaged_existing_building_does_not_prove_a_purchase() {
    let city = pillaged_city();
    assert!(matches!(
        check(city.clone(), city, "purchase", "BUILDING_WORKSHOP"),
        Verdict::Failed(_)
    ));
}

#[test]
fn missing_status_cannot_prove_a_known_building_repair_completed() {
    let before = pillaged_city();
    let mut after = before.clone();
    after.pillaged_buildings = None;
    assert_eq!(
        check(after, before, "produce", "BUILDING_WORKSHOP"),
        Verdict::Unverifiable
    );
}

#[test]
fn missing_status_cannot_prove_a_known_pillaged_building_was_purchased() {
    let before = pillaged_city();
    let mut after = before.clone();
    after.pillaged_buildings = None;
    assert_eq!(
        check(after, before, "purchase", "BUILDING_WORKSHOP"),
        Verdict::Unverifiable
    );
}

#[test]
fn ordinary_new_buildings_keep_legacy_presence_evidence_without_pillage_status() {
    let mut before = pillaged_city();
    before.buildings.clear();
    before.pillaged_buildings = None;
    let mut after = before.clone();
    after.buildings.push("BUILDING_WORKSHOP".into());
    assert_eq!(
        check(after, before, "produce", "BUILDING_WORKSHOP"),
        Verdict::Verified
    );
}
