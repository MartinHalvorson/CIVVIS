use crate::mirror::{snapshot_from_events_at, LiveMirror};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

// Native Cumaná: the settler lens warned -20 before founding at10,18 on
// turn61. The initial native city rate was -20; a governor only raised it
// to-12, and the colony revolted at69. The mirror cannot see all pressure.
fn fixture(warning: Option<f64>) -> LiveMirror {
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
