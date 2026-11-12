//! Exact zero forecasts skip work; policy tracing retains every question.

use super::*;

fn world(tourism: f64) -> Game {
    let mut game = Game::new(4, 40, 26, 8_602, 250, 0);
    game.turn = 60;
    game.found_city_for(0, (12, 10), None);
    for rival in 1..4 {
        game.found_city_for(rival, (12 + 4 * rival as i32, 16), None);
        game.players[rival].culture_lifetime = 4_000.0 * rival as f64;
    }
    std::sync::Arc::make_mut(&mut game.observed_tourism_per_turn).insert(0, tourism);
    game.keep_rival_yields(true);
    game.keep_own_yields(false);
    assert_eq!(game.foreign_tourists(0), 0);
    game
}

fn forecaster(revised: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    if revised {
        ai.enable_culture_lane_forecast_2();
    } else {
        ai.enable_culture_lane_forecast();
    }
    ai
}

fn full_score(ai: &AdvancedAi, game: &Game) -> i32 {
    let trace = game.trace_policy_reads(0);
    assert!(game.is_tracing_policy_reads());
    let score = ai.culture_lane_forecast_score_with(game, 0, true);
    drop(trace);
    assert!(!game.is_tracing_policy_reads());
    score
}

#[test]
fn zero_visitors_and_zero_rate_avoid_rival_yield_derivations() {
    for revised in [false, true] {
        for zero in [0.0, -0.0] {
            let game = world(zero);
            let ai = forecaster(revised);
            assert_eq!(full_score(&ai, &game), 0);
            assert_eq!(game.rival_yields_reused(), (0, 0));
            assert_eq!(ai.culture_lane_forecast_score_with(&game, 0, true), 0);
            assert_eq!(
                game.rival_yields_reused(),
                (0, 0),
                "the nontraced zero numerator needs no rival city yields"
            );
        }
    }
}

#[test]
fn finite_and_nonfinite_rates_match_the_full_traced_calculation() {
    for revised in [false, true] {
        for tourism in [0.0, -0.0, 3.0, -4.0, f64::INFINITY, f64::NAN] {
            let game = world(tourism);
            let ai = forecaster(revised);
            let expected = full_score(&ai, &game);
            assert_eq!(
                ai.culture_lane_forecast_score_with(&game, 0, true),
                expected,
                "forecast version {revised}, tourism {tourism}"
            );
            if tourism != 0.0 {
                assert!(
                    game.rival_yields_reused().1 > 0,
                    "nonzero and NaN rates retain the complete rival calculation"
                );
            }
        }
    }
}

#[test]
fn existing_visitors_are_not_discarded_when_the_current_rate_is_zero() {
    for revised in [false, true] {
        let mut game = world(0.0);
        let per_visitor = 4.0 * crate::game::TOURISM_PER_VISITOR;
        game.players[0]
            .tourism_pressure
            .insert(1, per_visitor * 5.0);
        assert!(game.foreign_tourists(0) > 0);
        let ai = forecaster(revised);
        let expected = full_score(&ai, &game);
        assert!(expected > 0);
        assert_eq!(
            ai.culture_lane_forecast_score_with(&game, 0, true),
            expected
        );
        assert!(game.rival_yields_reused().1 > 0);
    }
}

#[test]
fn policy_traces_preserve_all_original_forecast_card_classifications() {
    for revised in [false, true] {
        let game = world(0.0);
        let reference = game.trace_policy_reads(0);
        {
            let _memo = game.query_memo();
            for rival in 1..4 {
                game.domestic_tourists(rival);
                for city in game.player_city_ids(rival) {
                    game.city_yields(city);
                }
            }
            game.tourism_per_turn(0);
            game.foreign_tourists(0);
        }
        let expected = reference.finish();
        drop(reference);

        let trace = game.trace_policy_reads(0);
        assert_eq!(
            forecaster(revised).culture_lane_forecast_score_with(&game, 0, true),
            0
        );
        let actual = trace.finish();
        drop(trace);
        assert_eq!(actual.is_opaque(), expected.is_opaque());
        let mut relevant = 0;
        for card in game.rules.policies.keys() {
            let is_inert = actual.card_is_inert(&game.rules, *card);
            assert_eq!(
                is_inert,
                expected.card_is_inert(&game.rules, *card),
                "the same dependency answer for {card}"
            );
            relevant += usize::from(!is_inert);
        }
        assert!(
            relevant > 0,
            "the fixture actually asks about policy effects"
        );
    }
}

#[test]
fn a_new_rate_is_recomputed_after_a_zero_forecast() {
    for revised in [false, true] {
        let mut game = world(0.0);
        let ai = forecaster(revised);
        assert_eq!(ai.culture_lane_forecast_score_with(&game, 0, true), 0);
        assert_eq!(game.rival_yields_reused(), (0, 0));
        std::sync::Arc::make_mut(&mut game.observed_tourism_per_turn).insert(0, 10.0);
        game.board_written();
        let expected = full_score(&ai, &game);
        assert_eq!(
            ai.culture_lane_forecast_score_with(&game, 0, true),
            expected
        );
        assert!(game.rival_yields_reused().1 > 0);
    }
}

#[test]
fn existing_disabled_and_unbounded_clock_guards_still_skip_the_sweep() {
    let mut game = world(10.0);
    let ai = forecaster(false);
    assert_eq!(ai.culture_lane_forecast_score_with(&game, 0, false), 0);
    game.max_turns = 0;
    assert_eq!(ai.culture_lane_forecast_score_with(&game, 0, true), 0);
    game.max_turns = 250;
    game.victory_conditions.culture = false;
    assert_eq!(ai.culture_lane_forecast_score_with(&game, 0, true), 0);
    assert_eq!(game.rival_yields_reused(), (0, 0));
}
