//! Differential fixtures for carrying one read-only Culture forecast.

use super::*;
use crate::ai::advanced::BattlefrontFrame;
use crate::game::GameOptions;
use std::sync::Arc;

fn fixture() -> Game {
    let mut game = Game::new_with(GameOptions {
        barbarians: false,
        speed: "online".to_string(),
        ..GameOptions::new(4, 40, 26, 77_000, 250, 4)
    });
    game.turn = 200;
    for rival in 1..4 {
        game.players[rival].culture_lifetime = 10_000.0;
        game.players[0]
            .tourism_pressure
            .insert(rival, 10.0 * 4.0 * crate::game::TOURISM_PER_VISITOR);
    }
    Arc::make_mut(&mut game.observed_tourism_per_turn).insert(0, 200.0);
    game.players[0].science_projects.extend([
        "launch_earth_satellite".to_string(),
        "launch_moon_landing".to_string(),
        "launch_mars_colony".to_string(),
        "exoplanet_expedition".to_string(),
    ]);
    game
}

fn plan(strategy: GrandStrategy) -> StrategicPlan {
    StrategicPlan {
        strategy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: 0,
        rush: false,
    }
}

fn controller(version: usize) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.culture_lane_forecast = version == 1;
    ai.culture_lane_forecast_2 = version == 2;
    ai
}

// The original spending decision, with None explicitly selecting the
// original complete adaptive forecast. No carried score enters this oracle.
fn original_spends(ai: &AdvancedAi, g: &Game, pid: usize, plan: &StrategicPlan) -> bool {
    if ai.victory_portfolio {
        return plan.strategy != GrandStrategy::Recovery
            && ai.portfolio_supports(VictoryTarget::Culture);
    }
    let _memo = g.query_memo();
    let culture_focus = ai.victory_focus(g, pid).strategy == GrandStrategy::Culture;
    let named_culture = ai.active_victory_target(g) == Some(VictoryTarget::Culture);
    (plan.strategy != GrandStrategy::Recovery
        || named_culture
            && (AdvancedAi::recovery_faith_has_only_culture_units(g, pid)
                || ai.culture_recovery_bank_overflows(g, pid)))
        && (named_culture
            || culture_focus && ai.lane_culture_spending
            || ai.adaptive_culture_lane_spends(g, pid, plan, culture_focus, None))
}

#[test]
fn enabled_uncached_versions_carry_the_same_score_and_focus() {
    let game = fixture();
    let before = serde_json::to_vec(&game).unwrap();
    for version in [1, 2] {
        let ai = controller(version);
        let expected = ai.culture_lane_forecast_score_with(&game, 0, true);
        let original = ai.victory_focus(&game, 0);
        let mut forecast = None;
        let focus = ai.victory_focus_recording_culture_forecast(&game, 0, Some(&mut forecast));
        assert_eq!(forecast, Some(expected));
        assert_eq!(focus.strategy, original.strategy);
        assert_eq!(focus.progress, original.progress);
    }
    assert_eq!(serde_json::to_vec(&game).unwrap(), before);
}

#[test]
fn disabled_forecast_genes_leave_the_original_adaptive_fallback() {
    let game = fixture();
    let ai = controller(0);
    let mut forecast = None;
    ai.victory_focus_recording_culture_forecast(&game, 0, Some(&mut forecast));
    assert_eq!(forecast, None);
    assert!(ai.adaptive_culture_spending_signal(&game, 0, forecast));
    let plan = plan(GrandStrategy::Science);
    assert_eq!(
        ai.culture_lane_spends(&game, 0, &plan),
        original_spends(&ai, &game, 0, &plan)
    );
}

#[test]
fn target_and_disabled_planning_shortcuts_do_not_invent_a_forecast() {
    let game = fixture();
    for target in [VictoryTarget::Culture, VictoryTarget::Science] {
        let ai = AdvancedAi::targeting(target);
        let mut forecast = None;
        let focus = ai.victory_focus_recording_culture_forecast(&game, 0, Some(&mut forecast));
        assert_eq!(forecast, None);
        assert_eq!(focus.strategy, ai.victory_focus(&game, 0).strategy);
    }
    let mut ai = controller(1);
    ai.victory_planning = false;
    let mut forecast = None;
    ai.victory_focus_recording_culture_forecast(&game, 0, Some(&mut forecast));
    assert_eq!(forecast, None);
}

#[test]
fn battlefront_lane_cache_never_supplies_a_fresh_forecast() {
    let game = fixture();
    let mut ai = controller(1);
    ai.battlefront_frame = Some(BattlefrontFrame {
        visible: game.player_vision_frame(0),
        units: Default::default(),
    });
    ai.lane_progress_table(&game, 0);
    assert_eq!(ai.lane_progress_cache.0.borrow().len(), 1);
    let mut forecast = None;
    ai.victory_focus_recording_culture_forecast(&game, 0, Some(&mut forecast));
    assert_eq!(forecast, None);
    let plan = plan(GrandStrategy::Science);
    assert_eq!(
        ai.culture_lane_spends(&game, 0, &plan),
        original_spends(&ai, &game, 0, &plan)
    );
}

#[test]
fn spending_matches_the_original_across_forecasts_postures_and_targets() {
    let game = fixture();
    for version in [0, 1, 2] {
        for target in [
            None,
            Some(VictoryTarget::Culture),
            Some(VictoryTarget::Science),
        ] {
            let mut ai = target.map_or_else(|| controller(version), AdvancedAi::targeting);
            ai.culture_lane_forecast = version == 1;
            ai.culture_lane_forecast_2 = version == 2;
            for turn in [0, 49, 50, 200, 250] {
                for tourism in [0.0, 20.0, 200.0] {
                    let mut board = game.clone();
                    board.turn = turn;
                    Arc::make_mut(&mut board.observed_tourism_per_turn).insert(0, tourism);
                    for strategy in [
                        GrandStrategy::Expansion,
                        GrandStrategy::Science,
                        GrandStrategy::Culture,
                        GrandStrategy::Conquest,
                        GrandStrategy::Recovery,
                    ] {
                        let plan = plan(strategy);
                        assert_eq!(ai.culture_lane_spends(&board, 0, &plan), original_spends(&ai, &board, 0, &plan), "version={version}, target={target:?}, turn={turn}, tourism={tourism}, strategy={strategy:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn reuse_preserves_nested_policy_read_sets_and_serialized_boards() {
    for version in [1, 2] {
        let fast = fixture();
        let original = fast.clone();
        let before = serde_json::to_vec(&fast).unwrap();
        let ai = controller(version);
        let other_ai = ai.clone();
        let plan = plan(GrandStrategy::Science);
        let _fast_memo = fast.query_memo();
        let _original_memo = original.query_memo();
        let fast_trace = fast.trace_policy_reads(0);
        let original_trace = original.trace_policy_reads(0);
        assert_eq!(
            ai.culture_lane_spends(&fast, 0, &plan),
            original_spends(&other_ai, &original, 0, &plan)
        );
        let fast_reads = fast_trace.finish();
        let original_reads = original_trace.finish();
        assert_eq!(format!("{fast_reads:?}"), format!("{original_reads:?}"));
        assert_eq!(fast_reads.is_opaque(), original_reads.is_opaque());
        for &card in fast.rules.policies.keys() {
            assert_eq!(
                fast_reads.card_is_inert(&fast.rules, card),
                original_reads.card_is_inert(&original.rules, card)
            );
        }
        assert_eq!(serde_json::to_vec(&fast).unwrap(), before);
        assert_eq!(serde_json::to_vec(&original).unwrap(), before);
    }
}

#[test]
fn later_decisions_read_changed_tourism_instead_of_retaining_a_score() {
    let mut game = fixture();
    let ai = controller(1);
    let mut strong = None;
    ai.victory_focus_recording_culture_forecast(&game, 0, Some(&mut strong));
    Arc::make_mut(&mut game.observed_tourism_per_turn).insert(0, 0.0);
    let mut weak = None;
    ai.victory_focus_recording_culture_forecast(&game, 0, Some(&mut weak));
    assert!(strong.unwrap() > weak.unwrap());
    assert_eq!(
        weak,
        Some(ai.culture_lane_forecast_score_with(&game, 0, true))
    );
    assert_eq!(
        ai.culture_lane_spends(&game, 0, &plan(GrandStrategy::Science)),
        original_spends(&ai, &game, 0, &plan(GrandStrategy::Science))
    );
}

#[test]
fn missing_clock_disabled_culture_and_eliminations_keep_the_original_answer() {
    let game = fixture();
    for version in [1, 2] {
        for variant in 0..3 {
            let mut board = game.clone();
            match variant {
                0 => board.max_turns = 0,
                1 => board.victory_conditions.culture = false,
                _ => {
                    for rival in 1..4 {
                        board.players[rival].alive = false;
                    }
                }
            }
            let ai = controller(version);
            let plan = plan(GrandStrategy::Conquest);
            assert_eq!(
                ai.culture_lane_spends(&board, 0, &plan),
                original_spends(&ai, &board, 0, &plan)
            );
        }
    }
}
