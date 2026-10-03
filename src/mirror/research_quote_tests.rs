use super::*;
use crate::game::Game;
use serde_json::json;

fn state(quotes: serde_json::Value) -> StateSnapshot {
    serde_json::from_value(json!({"turn":120,"research_quotes":quotes})).unwrap()
}

#[test]
fn native_research_quote_import_preserves_values_and_seat_identity() {
    let mut g = Game::new_full(2, 14, 14, 10_033_878, 500, 0, false);
    let observed = state(json!([{"t":"TECH_RADIO","c":100.0,"p":75.0}]));
    let mut unmapped = Vec::new();
    apply_research_quotes(&mut g, &observed, &mut unmapped);
    assert_eq!(
        g.host_remaining_research_cost(0, crate::name!("radio")),
        Some(25.0)
    );
    assert_eq!(
        g.host_remaining_research_cost(1, crate::name!("radio")),
        None
    );
    assert!(unmapped.is_empty());
}

#[test]
fn refreshed_and_legacy_exports_clear_stale_research_quotes() {
    let mut g = Game::new_full(2, 14, 14, 10_033_878, 500, 0, false);
    let mut unmapped = Vec::new();
    apply_research_quotes(
        &mut g,
        &state(json!([{"t":"TECH_RADIO","c":100.0,"p":75.0}])),
        &mut unmapped,
    );
    apply_research_quotes(
        &mut g,
        &state(json!([{"t":"TECH_RADIO","c":100.0,"p":90.0}])),
        &mut unmapped,
    );
    assert_eq!(
        g.host_remaining_research_cost(0, crate::name!("radio")),
        Some(10.0)
    );
    let legacy: StateSnapshot = serde_json::from_str(r#"{"turn":121}"#).unwrap();
    apply_research_quotes(&mut g, &legacy, &mut unmapped);
    assert_eq!(
        g.host_remaining_research_cost(0, crate::name!("radio")),
        None
    );
}

#[test]
fn invalid_unknown_and_partial_rows_report_once_and_keep_fallback() {
    let mut g = Game::new_full(2, 14, 14, 10_033_878, 500, 0, false);
    let mut observed = state(json!([
        {"t":"TECH_RADIO","c":-1.0,"p":0.0},
        {"t":"TECH_FLIGHT","c":100.0},
        {"t":"TECH_UNKNOWN","c":100.0,"p":0.0}
    ]));
    observed.research_quotes.push(StateResearchQuote {
        t: "TECH_ADVANCED_FLIGHT".into(),
        c: Some(100.0),
        p: Some(f64::NAN),
    });
    let mut unmapped = Vec::new();
    for _ in 0..2 {
        apply_research_quotes(&mut g, &observed, &mut unmapped);
    }
    assert_eq!(unmapped.len(), 4);
    for tech in [
        crate::name!("radio"),
        crate::name!("flight"),
        crate::name!("advanced_flight"),
    ] {
        assert_eq!(g.host_remaining_research_cost(0, tech), None);
    }
}

#[test]
fn empty_and_zero_quote_exports_are_distinct() {
    let mut g = Game::new_full(2, 14, 14, 10_033_878, 500, 0, false);
    apply_research_quotes(&mut g, &state(json!([])), &mut Vec::new());
    assert_eq!(
        g.host_remaining_research_cost(0, crate::name!("radio")),
        None
    );
    apply_research_quotes(
        &mut g,
        &state(json!([{"t":"TECH_RADIO","c":0.0,"p":0.0}])),
        &mut Vec::new(),
    );
    assert_eq!(
        g.host_remaining_research_cost(0, crate::name!("radio")),
        Some(0.0)
    );
}

#[test]
fn full_mirror_and_persistent_sync_both_refresh_native_research_quotes() {
    let plots = (0..14)
        .flat_map(|x| {
            (0..14).map(move |y| {
                serde_json::from_value(json!({"x":x,"y":y,"t":"TERRAIN_GRASS","o":-1})).unwrap()
            })
        })
        .collect();
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 120,
        width: 14,
        height: 14,
        chunk: 1,
        plots,
    }]);
    let mut observed = state(json!([{"t":"TECH_RADIO","c":100.0,"p":75.0}]));
    observed.cities.push(StateCity {
        id: 1,
        name: "Bogota".into(),
        x: 5,
        y: 5,
        pop: 8,
        capital: true,
        ..Default::default()
    });
    let mut mirror = LiveMirror::new(&snapshot, &observed, 4, 10_033_878, 500, 0);
    assert_eq!(
        mirror
            .game
            .host_remaining_research_cost(0, crate::name!("radio")),
        Some(25.0)
    );
    observed.research_quotes =
        state(json!([{"t":"TECH_RADIO","c":100.0,"p":90.0}])).research_quotes;
    mirror.sync(&snapshot, &observed, 4);
    assert_eq!(
        mirror
            .game
            .host_remaining_research_cost(0, crate::name!("radio")),
        Some(10.0)
    );
    observed.research_quotes.clear();
    mirror.sync(&snapshot, &observed, 4);
    assert_eq!(
        mirror
            .game
            .host_remaining_research_cost(0, crate::name!("radio")),
        None
    );
}
