use super::*;
use crate::game::ObservedPublicEmpireStats;

fn board() -> Game {
    let mut game = Game::new_full(2, 24, 16, 38_720, 650, 0, false);
    super::tests::found_capitals(&mut game);
    game.turn = 205;
    for pid in 0..2 {
        for project in [
            "launch_earth_satellite",
            "launch_moon_landing",
            "launch_mars_colony",
            "exoplanet_expedition",
        ] {
            game.players[pid].science_projects.insert(project.into());
        }
    }
    game
}

fn observe(game: &mut Game, pid: usize, points: f64, target: f64) {
    std::sync::Arc::make_mut(&mut game.observed_public_empire_stats).insert(
        pid,
        ObservedPublicEmpireStats {
            science_victory_points: Some(points),
            science_victory_points_needed: Some(target),
            ..ObservedPublicEmpireStats::default()
        },
    );
}

#[test]
fn native_rival_expedition_distance_and_speed_target_reach_ai_pressure() {
    let mut game = board();
    observe(&mut game, 1, 23.0, 25.0);
    game.players[1].dvp = 18;
    assert_eq!(game.players[1].exoplanet_distance, 0.0);
    assert_eq!(
        AdvancedAi::new().rival_pressure(&game, 1),
        (GrandStrategy::Science, 98),
        "the observed expedition must outrank the rival's 90% diplomatic clock"
    );
}

#[test]
fn native_own_expedition_distance_and_speed_target_reach_lane_progress() {
    let mut game = board();
    observe(&mut game, 0, 23.0, 25.0);
    assert_eq!(game.players[0].exoplanet_distance, 0.0);
    assert_eq!(
        AdvancedAi::new().lane_progress_table_uncached(&game, 0)[0],
        92
    );
}

#[test]
fn native_rebuild_and_sync_update_the_ai_clock_without_advancing_simulator_distance() {
    use crate::mirror::{state_from_json, LiveMirror, Snapshot, TilesChunk};
    let mut state = state_from_json(
        r#"{"turn":205,"science_projects":["PROJECT_LAUNCH_EXOPLANET_EXPEDITION"],
        "science_victory_points":23,"science_victory_points_needed":25,
        "rivals":[{"player":1,"science_projects":["PROJECT_LAUNCH_EXOPLANET_EXPEDITION"],
        "science_victory_points":7,"science_victory_points_needed":25}]}"#,
    )
    .unwrap();
    let snapshot = Snapshot::from_chunks(&[TilesChunk {
        turn: 205,
        width: 8,
        height: 8,
        chunk: 1,
        plots: vec![],
    }]);
    let mut mirror = LiveMirror::new(&snapshot, &state, 2, 1, 650, 0);
    let ai = AdvancedAi::new();
    assert_eq!(mirror.game.players[1].exoplanet_distance, 0.0);
    assert_eq!(
        ai.rival_pressure(&mirror.game, 1),
        (GrandStrategy::Science, 84)
    );
    assert_eq!(ai.lane_progress_table_uncached(&mirror.game, 0)[0], 92);

    state.turn += 1;
    state.rivals[0].science_victory_points = 23.0;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(mirror.game.players[1].exoplanet_distance, 0.0);
    assert_eq!(
        ai.rival_pressure(&mirror.game, 1),
        (GrandStrategy::Science, 98)
    );

    state.turn += 1;
    state.rivals[0].science_victory_points = f64::NAN;
    state.rivals[0].science_victory_points_needed = 0.0;
    mirror.sync(&snapshot, &state, 0);
    assert_eq!(
        ai.rival_pressure(&mirror.game, 1),
        (GrandStrategy::Science, 78)
    );
}

#[test]
fn simulator_and_invalid_observation_fallbacks_keep_the_existing_clock() {
    let mut game = board();
    let ai = AdvancedAi::new();
    for pid in 0..2 {
        game.players[pid].exoplanet_distance = 49.0;
    }
    assert_eq!(ai.rival_pressure(&game, 1), (GrandStrategy::Science, 99));
    assert_eq!(ai.lane_progress_table_uncached(&game, 0)[0], 98);
    for pid in 0..2 {
        observe(&mut game, pid, -1.0, f64::NAN);
    }
    assert_eq!(ai.rival_pressure(&game, 1), (GrandStrategy::Science, 99));
    assert_eq!(ai.lane_progress_table_uncached(&game, 0)[0], 98);
}

#[test]
fn observations_do_not_create_an_unlaunched_race_or_exceed_completion() {
    let mut game = board();
    let ai = AdvancedAi::new();
    for pid in 0..2 {
        observe(&mut game, pid, 30.0, 25.0);
    }
    assert_eq!(ai.rival_pressure(&game, 1), (GrandStrategy::Science, 100));
    assert_eq!(ai.lane_progress_table_uncached(&game, 0)[0], 100);
    for pid in 0..2 {
        game.players[pid].science_projects.clear();
    }
    assert_eq!(ai.rival_pressure(&game, 1), (GrandStrategy::Expansion, 0));
    assert_eq!(ai.lane_progress_table_uncached(&game, 0)[0], 25);
}
