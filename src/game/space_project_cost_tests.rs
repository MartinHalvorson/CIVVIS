//! `GameOptions::ai_space_project_cost_pct`: the four space projects priced
//! for a handicapped AI major only. See `gene_screen --rival-victory-pace`.
use super::{Game, Item};

fn paced() -> Game {
    let mut g = Game::new_full(2, 30, 18, 4_291, 300, 0, false);
    g.handicap_exempt.insert(0);
    g.ai_space_project_cost_pct = Some(150);
    g
}

#[test]
fn only_the_rivals_space_projects_are_priced() {
    let g = paced();
    assert_eq!(g.space_project_cost_scale(1, "launch_moon_landing"), 1.5);
    assert_eq!(g.space_project_cost_scale(1, "exoplanet_expedition"), 1.5);
    assert_eq!(
        g.space_project_cost_scale(0, "launch_moon_landing"),
        1.0,
        "an exempt (measured) seat pays the ordinary cost"
    );
    assert_eq!(g.space_project_cost_scale(1, "manhattan_project"), 1.0);
    let moon = Item::Project {
        project: crate::name!("launch_moon_landing"),
    };
    let mut unpaced = g.clone();
    unpaced.ai_space_project_cost_pct = None;
    assert!(
        (g.item_cost_for(1, &moon) - 1.5 * unpaced.item_cost_for(1, &moon)).abs() <= 2.0,
        "the rival pays half again"
    );
    assert_eq!(g.item_cost_for(0, &moon), unpaced.item_cost_for(0, &moon));
}

#[test]
fn the_setting_survives_a_save_and_is_absent_unset() {
    let g = paced();
    let restored: Game = serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
    assert_eq!(restored.ai_space_project_cost_pct, Some(150));
    let plain = Game::new_full(2, 30, 18, 4_291, 300, 0, false);
    assert_eq!(
        plain.space_project_cost_scale(1, "launch_moon_landing"),
        1.0
    );
    assert!(!serde_json::to_string(&plain)
        .unwrap()
        .contains("ai_space_project_cost_pct"));
}

/// `ai_late_science_pct`: from the late mark the rival's Science bonus is
/// rescaled so its Science is that share; before it, and for an exempt seat,
/// nothing changes.
#[test]
fn late_science_scales_the_rivals_science_from_the_mark() {
    let mut g = Game::new_full(2, 30, 18, 4_292, 300, 0, false);
    g.difficulty = "immortal".to_string();
    g.handicap_exempt.insert(0);
    let before = g.handicap_yield_pct(1).science;
    g.ai_late_science_pct = Some(85);
    g.turn = 1;
    assert_eq!(g.handicap_yield_pct(1).science, before, "before the mark");
    g.turn = g.standard_duration(super::AI_LATE_SCIENCE_FROM_STANDARD);
    let after = g.handicap_yield_pct(1).science;
    assert!(
        ((100.0 + after) - 0.85 * (100.0 + before)).abs() < 1e-9,
        "{before} -> {after}"
    );
    assert_eq!(g.handicap_yield_pct(0).science, 0.0, "exempt");
}

/// `ai_space_tech_floor`: a handicapped AI major may not start a space
/// project before it knows that many techs; an exempt seat and any other
/// project are never held.
#[test]
fn the_space_floor_holds_only_the_rivals_space_projects() {
    let mut g = Game::new_full(2, 30, 18, 4_293, 300, 0, false);
    g.handicap_exempt.insert(0);
    assert!(
        g.space_project_tech_floor_met(1, "launch_earth_satellite"),
        "unset"
    );
    g.ai_space_tech_floor = Some(58);
    assert!(!g.space_project_tech_floor_met(1, "launch_earth_satellite"));
    assert!(g.space_project_tech_floor_met(1, "manhattan_project"));
    assert!(
        g.space_project_tech_floor_met(0, "launch_earth_satellite"),
        "exempt"
    );
    let techs: Vec<_> = g.rules.techs.keys().take(58).copied().collect();
    g.players[1].techs.extend(techs);
    assert!(g.space_project_tech_floor_met(1, "launch_earth_satellite"));
}
