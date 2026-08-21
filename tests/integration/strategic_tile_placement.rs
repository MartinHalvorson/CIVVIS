//! Source-level guardrails for the fixed Strategic-map tile slots.
//!
//! The browser paints the same helpers for flat and planet maps.  Keep the
//! woodland anchor independent from hill presence, the hill on its lower edge,
//! and improvement calls at the unshifted tile origin.

const INDEX: &str = include_str!("../../web/assets/app.js");

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

#[test]
fn coastal_cliffs_are_unmistakable_escarpments_in_both_strategic_views() {
    let cliffs = function_source("function drawCliffEscarpments");

    // A thin shoreline ribbon is not enough to communicate that this edge is
    // impassable. The mark needs a substantial face, an outlined pale crest,
    // and fractures that make its vertical drop legible at map zoom.
    assert!(cliffs.contains("const depth = 14.5 * scale;"));
    assert!(cliffs.contains("const foot = depth * 1.18;"));
    assert!(cliffs.contains("cx.strokeStyle = \"#211914\";"));
    assert!(cliffs.contains("cx.strokeStyle = \"#f2d79c\";"));
    assert!(cliffs.contains("[[.19, -.34], [.50, .24], [.81, -.27]]"));

    let planet = function_source("function drawPlanetStrategicCliffs");
    assert!(planet.contains("drawCliffEscarpments(groups.byWeight.values());"));
    assert!(
        INDEX.contains("drawCliffEscarpments([{cliffs: cliffEscarpments, weight:1, alpha:1}]);"),
        "the flat strategic renderer must use the same clear cliff vocabulary"
    );
}

#[test]
fn feitoria_has_a_distinct_coastal_trading_post_marker() {
    let painter = function_source("function paintImprovementMarker");
    let feitoria = painter
        .split("case \"feitoria\":")
        .nth(1)
        .and_then(|tail| tail.split("case \"arena\":").next())
        .expect("Feitoria's dedicated marker case");

    assert!(feitoria.contains("Portugal's fortified overseas trade post"));
    assert!(feitoria.contains("cx.ellipse(x, y + 6.5, 15, 4.7"));
    assert!(feitoria.contains("const quay = new Path2D()"));
    assert!(feitoria.contains("const tower = new Path2D()"));
    assert!(feitoria.contains("cx.fillStyle = \"#ba4039\""));
    assert!(feitoria.contains("break;"));
}

#[test]
fn every_improvement_marker_has_a_specific_renderer_path() {
    let (_, registry_tail) = INDEX
        .split_once("const IMPROVEMENT_MARKERS = Object.freeze({")
        .expect("improvement marker registry");
    let (registry, _) = registry_tail
        .split_once("\n});")
        .expect("end of improvement marker registry");
    let painter = function_source("function paintImprovementMarker");
    let direct = function_source("function drawImprovement");
    const DIRECT: &[&str] = &[
        "farm",
        "pasture",
        "plantation",
        "camp",
        "quarry",
        "mine",
        "lumber_mill",
        "fishing_boats",
        "fishery",
        "seastead",
        "oil_well",
        "offshore_oil_rig",
        "wind_farm",
        "offshore_wind_farm",
        "solar_farm",
        "fort",
        "missile_silo",
        "great_wall",
        "sphinx",
        "goody_hut",
    ];

    // Camps intentionally bypass drawImprovement so their larger palisade and
    // fire can render in both map modes before ordinary improvement art.
    assert!(INDEX.contains("if (t.improvement === \"barbarian_camp\" || campSet.has(tileKey))"));
    assert!(INDEX.contains("if (t.improvement === \"barbarian_camp\" || campSet.has(k))"));

    let mut checked = 0;
    for line in registry.lines() {
        let line = line.trim();
        let Some((improvement, marker)) = line.split_once(": \"") else {
            continue;
        };
        let marker = marker.trim_end_matches(',').trim_end_matches('"');
        if improvement == "barbarian_camp" {
            continue;
        }

        if DIRECT.contains(&improvement) {
            assert!(
                direct.contains(&format!("imp === \"{improvement}\"")),
                "{improvement} must retain its dedicated direct renderer"
            );
        } else {
            assert!(
                painter.contains(&format!("case \"{marker}\"")),
                "{improvement} must resolve to the dedicated {marker} painter case"
            );
        }
        checked += 1;
    }

    assert_eq!(
        checked,
        registry.matches(": \"").count() - 1,
        "every registry entry except the separately-painted barbarian camp must be checked"
    );
}
