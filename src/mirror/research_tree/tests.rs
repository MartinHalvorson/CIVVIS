use super::*;
use crate::mirror::{LiveMirror, Snapshot, TilesChunk};
use std::sync::Arc;

fn game() -> Game {
    Game::new_full(2, 8, 8, 1, 250, 0, false)
}

fn state(rows: serde_json::Value) -> StateSnapshot {
    serde_json::from_value(serde_json::json!({
        "turn":163, "techs":["TECH_NANOTECHNOLOGY"], "research_tree":rows
    }))
    .unwrap()
}

fn observed() -> StateSnapshot {
    // Native002152 at163/0: Smart Materials is revealed, but Fusion is missing.
    state(serde_json::json!([{
        "t":"TECH_SMART_MATERIALS",
        "requires":["TECH_NANOTECHNOLOGY","TECH_NUCLEAR_FUSION"],
        "can_research":false
    }]))
}

#[test]
fn revealed_native_requirements_replace_the_generated_route_and_follow_new_knowledge() {
    let mut g = game();
    let baseline = g.rules.clone();
    g.players[0].techs.insert(crate::name!("nanotechnology"));
    let mut gaps = Vec::new();
    apply(&mut g, &observed(), &mut gaps);
    assert!(gaps.is_empty());
    assert_eq!(
        g.rules.techs["smart_materials"].requires,
        vec![
            crate::name!("nanotechnology"),
            crate::name!("nuclear_fusion")
        ]
    );
    assert!(!g
        .available_techs(0)
        .contains(&crate::name!("smart_materials")));
    g.players[0].techs.insert(crate::name!("nuclear_fusion"));
    assert!(g
        .available_techs(0)
        .contains(&crate::name!("smart_materials")));
    assert_eq!(
        baseline.techs["smart_materials"].requires,
        vec![crate::name!("advanced_ai")]
    );
    assert_eq!(
        g.rules.techs["offworld_mission"].requires, baseline.techs["offworld_mission"].requires,
        "hidden facts were not inferred"
    );
    assert_eq!(
        g.rules.techs["smart_materials"].cost,
        baseline.techs["smart_materials"].cost
    );
}

#[test]
fn absent_empty_and_semantically_unchanged_observations_keep_the_original_rules() {
    let mut g = game();
    let baseline = g.rules.clone();
    let mut gaps = Vec::new();
    for s in [
        StateSnapshot::default(),
        state(serde_json::json!([])),
        state(serde_json::json!([
            {"t":"TECH_THE_WHEEL","requires":["TECH_MINING"]},
            {"t":"TECH_POTTERY","requires":[]}
        ])),
    ] {
        apply(&mut g, &s, &mut gaps);
        assert!(Arc::ptr_eq(&baseline, &g.rules));
        assert!(g.host_research_base_rules.is_none());
    }
    assert!(gaps.is_empty());
}

#[test]
fn persistent_and_fresh_mirrors_refresh_and_rewind_without_poisoning_the_template() {
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 163,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![],
    }]);
    let mut s = observed();
    let old = LiveMirror::new(
        &snapshot,
        &StateSnapshot {
            turn: 163,
            ..Default::default()
        },
        2,
        1,
        250,
        0,
    );
    let generated = old.game.rules.techs["smart_materials"].requires.clone();
    let mut live = LiveMirror::new(&snapshot, &s, 2, 1, 250, 0);
    assert_ne!(live.game.rules.techs["smart_materials"].requires, generated);
    s.frame = 1;
    s.research_tree = None;
    live.sync(&snapshot, &s, 0);
    assert_eq!(live.game.rules.techs["smart_materials"].requires, generated);
    s.turn += 1;
    s.research_tree = observed().research_tree;
    live.sync(&snapshot, &s, 0);
    assert_eq!(
        live.game.rules.techs["smart_materials"].requires,
        vec![
            crate::name!("nanotechnology"),
            crate::name!("nuclear_fusion")
        ]
    );
    let fresh = LiveMirror::new(
        &snapshot,
        &StateSnapshot {
            turn: 163,
            ..Default::default()
        },
        2,
        1,
        250,
        0,
    );
    assert_eq!(
        fresh.game.rules.techs["smart_materials"].requires,
        generated
    );
    assert_eq!(old.game.rules.techs["smart_materials"].requires, generated);
}

#[test]
fn malformed_duplicate_unknown_and_cyclic_snapshots_restore_the_baseline() {
    for rows in [
        serde_json::json!([{"t":"TECH_WRITING"}]),
        serde_json::json!([{"t":"TECH_MISSING","requires":[]}]),
        serde_json::json!([{"t":"TECH_WRITING","requires":["TECH_MISSING"]}]),
        serde_json::json!([
            {"t":"TECH_WRITING","requires":[]},
            {"t":"TECH_WRITING","requires":["TECH_POTTERY"]}
        ]),
        serde_json::json!([{"t":"TECH_WRITING","requires":["TECH_WRITING"]}]),
        serde_json::json!([{"t":"TECH_POTTERY","requires":["TECH_WRITING"]}]),
    ] {
        let mut g = game();
        let baseline = g.rules.clone();
        apply(&mut g, &observed(), &mut Vec::new());
        let mut gaps = Vec::new();
        apply(&mut g, &state(rows), &mut gaps);
        assert!(Arc::ptr_eq(&baseline, &g.rules));
        assert_eq!(
            g.rules.techs["writing"].requires,
            vec![crate::name!("pottery")]
        );
        assert!(!gaps.is_empty());
    }
}

#[test]
fn native_graph_round_trip_keeps_a_valid_generated_layout_and_observed_differences() {
    let mut g = game();
    let generated = g.rules.future_tree_layout();
    let ordinary = serde_json::to_value(&g).unwrap();
    assert!(ordinary.get("host_research_prerequisites").is_none());
    apply(&mut g, &observed(), &mut Vec::new());
    let saved = serde_json::to_value(&g).unwrap();
    assert_eq!(
        saved["future_tree_layout"],
        serde_json::to_value(&generated).unwrap()
    );
    assert_eq!(
        saved["host_research_prerequisites"]["smart_materials"],
        serde_json::json!(["nanotechnology", "nuclear_fusion"])
    );
    let mut loaded: Game = serde_json::from_value(saved).unwrap();
    assert_eq!(
        loaded.rules.techs["smart_materials"].requires,
        g.rules.techs["smart_materials"].requires
    );
    loaded
        .set_host_research_prerequisites(BTreeMap::new())
        .unwrap();
    assert_eq!(loaded.rules.future_tree_layout(), generated);
    assert_ne!(
        loaded.rules.techs["smart_materials"].requires,
        g.rules.techs["smart_materials"].requires
    );
}

#[test]
fn speculative_clone_can_clear_observations_without_changing_the_observed_board() {
    let mut g = game();
    apply(&mut g, &observed(), &mut Vec::new());
    let observed_rules = g.rules.clone();
    let mut branch = g.clone();
    branch
        .set_host_research_prerequisites(BTreeMap::new())
        .unwrap();
    assert!(Arc::ptr_eq(&g.rules, &observed_rules));
    assert_ne!(
        branch.rules.techs["smart_materials"].requires,
        g.rules.techs["smart_materials"].requires
    );
}

#[test]
fn actual_planner_uses_the_observed_ancestor_instead_of_the_generated_branch() {
    let mut legacy = game();
    for tile in legacy.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    legacy.found_city_for(0, (2, 2), None);
    legacy.turn = 163;
    legacy.world_era = 7;
    legacy.players[0].techs = legacy
        .rules
        .techs
        .iter()
        .filter(|(tech, spec)| spec.era < 8 && **tech != "nuclear_fusion")
        .map(|(tech, _)| *tech)
        .collect();
    legacy.players[0].science_projects.extend([
        "launch_earth_satellite".to_string(),
        "launch_moon_landing".to_string(),
    ]);
    legacy.players[0].research = None;
    let mut observed_board = legacy.clone();
    apply(&mut observed_board, &observed(), &mut Vec::new());
    assert!(observed_board.rules.tech_ancestors["smart_materials"].contains("nuclear_fusion"));
    assert!(!observed_board.rules.tech_ancestors["smart_materials"].contains("advanced_ai"));
    assert!(!legacy.rules.tech_ancestors["smart_materials"].contains("nuclear_fusion"));
    let mut old_ai = crate::ai::AdvancedAi::targeting(crate::ai::VictoryTarget::Science);
    let mut observed_ai = crate::ai::AdvancedAi::targeting(crate::ai::VictoryTarget::Science);
    old_ai.plan_observed_turn(&mut legacy, 0);
    observed_ai.plan_observed_turn(&mut observed_board, 0);
    assert_eq!(legacy.players[0].research.as_deref(), Some("advanced_ai"));
    assert_eq!(
        observed_board.players[0].research.as_deref(),
        Some("nuclear_fusion")
    );
}
