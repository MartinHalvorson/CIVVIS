//! Source-level guardrails for the fixed Strategic-map tile slots.
//!
//! The browser paints the same helpers for flat and planet maps.  Keep the
//! woodland anchor independent from hill presence, the hill on its lower edge,
//! and improvement calls at the unshifted tile origin.

const INDEX: &str = include_str!("../web/index.html");

fn function_source(name: &str) -> &str {
    let start = INDEX.find(name).unwrap_or_else(|| panic!("missing {name}"));
    let after = &INDEX[start + name.len()..];
    let end = after
        .find("\nfunction ")
        .map(|offset| start + name.len() + offset)
        .unwrap_or(INDEX.len());
    &INDEX[start..end]
}

#[test]
fn strategic_tile_layers_keep_fixed_slots() {
    assert!(INDEX.contains("const STRATEGIC_WOODLAND_UPPER_HALF_LIFT = -7;"));

    let hills = function_source("function drawStrategicHillIcon");
    assert!(hills.contains("const hillBaseY = y + S * YS / 2;"));

    let woodland = function_source("function drawStrategicWoodlandIcon");
    assert!(woodland.contains("const lift = STRATEGIC_WOODLAND_UPPER_HALF_LIFT;"));
    assert!(
        !woodland.contains("t.hills"),
        "woods and rainforest must not change position when a tile has hills"
    );
}

#[test]
fn strategic_improvements_use_the_tile_centre_in_both_renderers() {
    let improvement = function_source("function drawImprovement");
    assert!(improvement.starts_with("function drawImprovement(t, x, y)"));
    assert!(INDEX.contains("drawImprovement(t, 0, 0);"));
    assert!(INDEX.contains("drawImprovement(t, x, y);"));
}
