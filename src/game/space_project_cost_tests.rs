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
