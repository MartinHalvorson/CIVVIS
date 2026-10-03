use super::*;
use crate::mirror::{LiveMirror, Snapshot, StateSnapshot, TilesChunk};
use crate::setup::GameSpeed;
use serde_json::{json, Value};

fn board(missing: &[&str], quotes: Value, current: Option<&str>, progress: f64) -> Game {
    let reference = Game::new_full(2, 14, 14, 10_033_878, 500, 0, false);
    let path = &reference.rules.tech_ancestors[*missing.last().unwrap()];
    let techs: Vec<_> = path
        .iter()
        .filter(|name| !missing.contains(&name.as_str()))
        .map(|name| format!("TECH_{}", name.as_str().to_uppercase()))
        .collect();
    let plots = (0..14)
        .flat_map(|x| {
            (0..14).map(move |y| {
                serde_json::from_value(json!({"x":x,"y":y,"t":"TERRAIN_GRASS","o":-1})).unwrap()
            })
        })
        .collect();
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 180,
        width: 14,
        height: 14,
        chunk: 1,
        plots,
    }]);
    // JSON exercises the native schema, including backward-compatible unknown
    // fields on the fail-first implementation, rather than hand-seeding Game.
    let state: StateSnapshot = serde_json::from_value(json!({
        "turn":180,
        "techs":techs,
        "research_quotes":quotes,
        "research":current.map(|name| format!("TECH_{}", name.to_uppercase())),
        "research_progress":progress,
        "cities":[{"id":1,"name":"Bogota","x":5,"y":5,"pop":8,"capital":true,
                   "yields":{"science":10.0,"production":80.0}}]
    }))
    .unwrap();
    LiveMirror::new(&snapshot, &state, 4, 10_033_878, 500, 0).game
}

#[test]
fn native_research_eta_uses_adjusted_cost_and_saved_non_current_progress() {
    let g = board(
        &["radio"],
        json!([{"t":"TECH_RADIO","c":100.0,"p":75.0}]),
        None,
        0.0,
    );
    assert_eq!(
        AdvancedAi::war_remaining_research_cost(&g, 0, crate::name!("radio")),
        25.0
    );
}

#[test]
fn native_research_eta_does_not_subtract_current_progress_twice() {
    let g = board(
        &["radio"],
        json!([{"t":"TECH_RADIO","c":100.0,"p":75.0}]),
        Some("radio"),
        75.0,
    );
    assert_eq!(
        AdvancedAi::war_remaining_research_cost(&g, 0, crate::name!("radio")),
        25.0
    );
}

#[test]
fn native_research_eta_quotes_each_missing_node_on_the_path() {
    let g = board(
        &["flight", "radio", "advanced_flight"],
        json!([
            {"t":"TECH_FLIGHT","c":100.0,"p":60.0},
            {"t":"TECH_RADIO","c":90.0,"p":60.0},
            {"t":"TECH_ADVANCED_FLIGHT","c":80.0,"p":60.0},
            {"t":"TECH_MINING","c":1000.0,"p":0.0}
        ]),
        Some("flight"),
        60.0,
    );
    assert_eq!(
        AdvancedAi::war_remaining_research_cost(&g, 0, crate::name!("advanced_flight")),
        90.0,
        "sum only missing path nodes; a completed-node quote cannot add cost"
    );
}

#[test]
fn native_research_eta_is_not_rescaled_by_game_speed() {
    let mut g = board(
        &["radio"],
        json!([{"t":"TECH_RADIO","c":100.0,"p":75.0}]),
        None,
        0.0,
    );
    for speed in [GameSpeed::Online, GameSpeed::Standard, GameSpeed::Marathon] {
        g.game_speed = speed;
        assert_eq!(
            AdvancedAi::war_remaining_research_cost(&g, 0, crate::name!("radio")),
            25.0
        );
    }
}

#[test]
fn native_research_eta_missing_or_invalid_quotes_keep_the_original_fallback() {
    for quotes in [
        json!([]),
        json!([{"t":"TECH_RADIO","c":-1.0,"p":0.0}]),
        json!([{"t":"TECH_RADIO","c":100.0}]),
        json!([{"t":"TECH_RADIO","c":100.0,"p":-1.0}]),
    ] {
        let g = board(&["radio"], quotes, Some("radio"), 7.0);
        assert_eq!(
            AdvancedAi::war_remaining_research_cost(&g, 0, crate::name!("radio")),
            (g.tech_cost("radio") - 7.0).max(0.0)
        );
    }
}

#[test]
fn native_research_eta_mixes_quotes_and_unquoted_nodes_without_losing_progress() {
    let g = board(
        &["flight", "radio"],
        json!([{"t":"TECH_RADIO","c":100.0,"p":75.0}]),
        Some("flight"),
        7.0,
    );
    assert_eq!(
        AdvancedAi::war_remaining_research_cost(&g, 0, crate::name!("radio")),
        g.tech_cost("flight") - 7.0 + 25.0
    );
}

#[test]
fn native_research_eta_rejects_an_air_package_that_the_model_price_says_fits() {
    let mut g = board(
        &["advanced_flight"],
        json!([{"t":"TECH_ADVANCED_FLIGHT","c":5000.0,"p":0.0}]),
        None,
        0.0,
    );
    let mut ai = AdvancedAi::new();
    ai.enable_air_surge_2();
    let legacy_research =
        (g.tech_cost("advanced_flight") / AdvancedAi::war_science_per_turn(&g, 0)).ceil() as u32;
    let (_, production) = ai.air_surge_launch_estimate(&g, 0);
    g.max_turns = g.turn
        + legacy_research
        + production
        + g.standard_duration(air_surge::AIR_SURGE_ENDGAME_RESERVE)
        + 1;
    assert!(g.tech_cost("advanced_flight") < 5000.0);
    assert!(!ai.air_surge_affordable(&g, 0));
}

#[test]
fn native_research_eta_admits_an_air_package_with_already_earned_progress() {
    let mut g = board(
        &["advanced_flight"],
        json!([{"t":"TECH_ADVANCED_FLIGHT","c":100.0,"p":99.0}]),
        None,
        0.0,
    );
    let mut ai = AdvancedAi::new();
    ai.enable_air_surge_2();
    let (_, production) = ai.air_surge_launch_estimate(&g, 0);
    g.max_turns =
        g.turn + 1 + production + g.standard_duration(air_surge::AIR_SURGE_ENDGAME_RESERVE) + 1;
    assert!(g.tech_cost("advanced_flight") > AdvancedAi::war_science_per_turn(&g, 0));
    assert!(ai.air_surge_affordable(&g, 0));
}
