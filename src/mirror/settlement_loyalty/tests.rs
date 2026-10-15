use crate::mirror::{snapshot_from_events_at, LiveMirror, Snapshot, StateSnapshot};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

// Native Cumaná: the settler lens warned -20 before founding at10,18 on
// turn61. The initial native city rate was -20; a governor only raised it
// to-12, and the colony revolted at69. The mirror cannot see all pressure.
fn boards(warning: Option<f64>) -> (Snapshot, StateSnapshot) {
    let state = json!({
        "kind":"state","turn":61,"frame":0,"gold":100,
        "cities":[{"id":1,"name":"Home","x":4,"y":18,"pop":6,"capital":true}],
        "units":[{"id":1441793,"kind":"UNIT_SETTLER","x":10,"y":18,
                  "hp":100,"moves":2,"max_moves":2}]
    });
    let plots: Vec<_> = (0..22)
        .flat_map(|y| {
            (0..16).map(move |x| {
                json!({
                    "x":x,"y":y,"t":"TERRAIN_GRASS","o":-1,"fw":true,"vis":true
                })
            })
        })
        .collect();
    let tiles = json!({"kind":"tiles","turn":61,"frame":0,
                       "width":16,"height":22,"plots":plots});
    let mut lines = Vec::new();
    if let Some(loyalty) = warning {
        lines.push(
            json!({"kind":"settlement_loyalty","turn":61,"frame":0,
                          "available":true,"plots":[{"x":10,"y":18,"loyalty":loyalty}]})
            .to_string(),
        );
    }
    lines.push(state.to_string());
    lines.push(tiles.to_string());
    let path = std::env::temp_dir().join(format!(
        "civvis-settlement-pressure-{}-{}.jsonl",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
    let snapshot = snapshot_from_events_at(&path, Some(61)).unwrap();
    std::fs::remove_file(&path).unwrap();
    let state = crate::mirror::state_from_json(&state.to_string()).unwrap();
    (snapshot, state)
}

fn fixture(warning: Option<f64>) -> LiveMirror {
    let (snapshot, state) = boards(warning);
    LiveMirror::new(&snapshot, &state, 4, 1, 650, 0)
}

#[test]
fn native_warning_prevents_founding_a_colony_the_known_city_forecast_misses() {
    let live = fixture(Some(-20.0));
    let settler = live.uid_of[&1441793];
    assert!(
        !live.game.can_found_city(settler),
        "the native warning must reach the founding guard"
    );
}

#[test]
fn legacy_snapshot_without_native_warning_keeps_its_forecast() {
    let live = fixture(None);
    assert!(live.game.can_found_city(live.uid_of[&1441793]));
}

#[test]
fn a_native_refresh_without_a_warning_reopens_the_site() {
    let mut live = fixture(Some(-20.0));
    let (snapshot, mut state) = boards(None);
    state.turn += 1;
    live.sync(&snapshot, &state, 0);
    assert!(live.game.observed_settlement_loyalty.is_none());
    assert!(live.game.can_found_city(live.uid_of[&1441793]));
}

#[test]
fn invalid_or_unrevealed_pressure_is_not_imported() {
    let invalid = fixture(Some(-21.0));
    assert!(invalid.game.can_found_city(invalid.uid_of[&1441793]));
    let (mut snapshot, state) = boards(Some(-20.0));
    snapshot.revealed.remove(&(10, 18));
    let live = LiveMirror::new(&snapshot, &state, 4, 1, 650, 0);
    assert!(live
        .game
        .observed_settlement_loyalty
        .as_ref()
        .unwrap()
        .plots
        .is_empty());
}

#[test]
fn explicit_host_refusals_survive_a_safe_native_reading() {
    let (snapshot, mut state) = boards(Some(10.0));
    let site = crate::hex::offset_to_axial(10, 18);
    state.refused_sites.insert(site);
    let live = LiveMirror::new(&snapshot, &state, 4, 1, 650, 0);
    assert!(!live.game.can_found_city(live.uid_of[&1441793]));
}

fn event(turn: u32, frame: u32, available: bool, loyalty: f64) -> serde_json::Value {
    json!({"kind":"settlement_loyalty","turn":turn,"frame":frame,
           "available":available,"plots":[{"x":10,"y":18,"loyalty":loyalty}]})
}

fn read(before: &[serde_json::Value], after: &[serde_json::Value]) -> Option<super::Observation> {
    let mut raw: String = before
        .iter()
        .map(|value| value.to_string() + "\n")
        .collect();
    let boundary = raw.len();
    raw.push_str("{\"kind\":\"state\",\"turn\":61}\n");
    for value in after {
        raw.push_str(&(value.to_string() + "\n"));
    }
    super::read(&raw, Some(boundary), Some((61, 0)))
}

#[test]
fn a_later_frame_or_turn_never_leaks_into_the_selected_board() {
    let selected = read(&[event(61, 0, true, -4.0)], &[event(61, 1, true, -20.0)]).unwrap();
    assert_eq!(selected.plots[0].loyalty, -4.0);
    for (turn, frame) in [(60, 0), (62, 0), (61, 1)] {
        assert!(read(&[event(turn, frame, true, -20.0)], &[]).is_none());
    }
}

#[test]
fn unavailable_empty_and_malformed_latest_events_do_not_revive_an_old_table() {
    let old = event(61, 0, true, -20.0);
    assert!(read(&[old.clone(), event(61, 0, false, -20.0)], &[]).is_none());
    let empty = json!({"kind":"settlement_loyalty","turn":61,"available":true,"plots":[]});
    assert!(read(&[old.clone(), empty], &[]).unwrap().plots.is_empty());
    let malformed = json!({"kind":"settlement_loyalty","turn":61,"available":true,"plots":"bad"});
    assert!(read(&[old, malformed], &[]).is_none());
}
