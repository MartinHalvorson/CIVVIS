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
            assert!(!ai.culture_lane_forecast_reaches(&game, 0, 60));
            assert_eq!(
                game.rival_yields_reused(),
                (0, 0),
                "the nontraced zero numerator needs no rival city yields"
            );
        }
    }
}

#[test]
fn full_score_callers_retain_the_original_complete_rival_sweep() {
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
            assert_eq!(game.rival_yields_reused().1, 3);
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
            .insert(1, per_visitor * 100.0);
        assert!(game.foreign_tourists(0) > 0);
        let ai = forecaster(revised);
        let expected = full_score(&ai, &game);
        assert!(expected >= 60);
        assert!(ai.culture_lane_forecast_reaches(&game, 0, 60));
        assert_eq!(game.rival_yields_reused().1, 3);
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
        assert!(!forecaster(revised).culture_lane_forecast_reaches(&game, 0, 60));
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
        assert!(!ai.culture_lane_forecast_reaches(&game, 0, 60));
        assert_eq!(game.rival_yields_reused(), (0, 0));
        std::sync::Arc::make_mut(&mut game.observed_tourism_per_turn).insert(0, 10_000.0);
        game.board_written();
        let expected = full_score(&ai, &game);
        assert!(expected >= 60);
        assert!(ai.culture_lane_forecast_reaches(&game, 0, 60));
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
    assert!(!ai.culture_lane_forecast_reaches(&game, 0, 60));
    game.max_turns = 250;
    game.victory_conditions.culture = false;
    assert_eq!(ai.culture_lane_forecast_score_with(&game, 0, true), 0);
    assert!(!ai.culture_lane_forecast_reaches(&game, 0, 60));
    assert_eq!(game.rival_yields_reused(), (0, 0));
}

#[test]
fn threshold_decisions_match_full_scores_for_rates_and_banked_visitors() {
    for revised in [false, true] {
        for tourism in [0.0, -0.0, 10.0, 500.0, -4.0, f64::INFINITY, f64::NAN] {
            for visitors in [0.0, 50.0] {
                let mut game = world(tourism);
                game.players[0]
                    .tourism_pressure
                    .insert(1, 4.0 * crate::game::TOURISM_PER_VISITOR * visitors);
                let ai = forecaster(revised);
                let expected = full_score(&ai, &game);
                for minimum in [-1, 0, 1, 40, 60, 100, 101] {
                    assert_eq!(
                        ai.culture_lane_forecast_reaches(&game, 0, minimum),
                        expected >= minimum,
                        "version {revised}, rate {tourism}, visitors {visitors}, threshold {minimum}"
                    );
                }
            }
        }
    }
}

#[test]
fn one_actual_rival_can_rule_out_positive_tourism_without_pricing_the_rest() {
    for revised in [false, true] {
        let game = world(10.0);
        let ai = forecaster(revised);
        assert!(full_score(&ai, &game) < 60);
        assert!(!ai.culture_lane_forecast_reaches(&game, 0, 60));
        assert_eq!(
            game.rival_yields_reused(),
            (0, 1),
            "derive the first rival's actual culture, skip the other two"
        );
    }
}

#[test]
fn reaching_the_threshold_requires_the_complete_rival_bar() {
    for revised in [false, true] {
        let game = world(10_000.0);
        let ai = forecaster(revised);
        assert!(full_score(&ai, &game) >= 60);
        assert!(ai.culture_lane_forecast_reaches(&game, 0, 60));
        assert_eq!(game.rival_yields_reused(), (0, 3));
    }
}

fn correct_city_culture(game: &mut Game, rival: usize, adjustment: f64) {
    let city = game.player_city_ids(rival)[0];
    std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        city,
        Yields {
            culture: adjustment,
            ..Yields::default()
        },
    );
}

#[test]
fn negative_first_bars_do_not_hide_a_later_decisive_rival() {
    for revised in [false, true] {
        let mut game = world(10.0);
        correct_city_culture(&mut game, 1, -100_000.0);
        correct_city_culture(&mut game, 2, -100_000.0);
        correct_city_culture(&mut game, 3, 10_000.0);
        let ai = forecaster(revised);
        assert!(full_score(&ai, &game) < 60);
        assert!(!ai.culture_lane_forecast_reaches(&game, 0, 60));
        assert_eq!(game.rival_yields_reused(), (0, 3));
    }
}

#[test]
fn threshold_decisions_preserve_nonfinite_and_negative_culture_bars() {
    for revised in [false, true] {
        for adjustment in [-100_000.0, f64::NEG_INFINITY, f64::INFINITY, f64::NAN] {
            for tourism in [10.0, f64::INFINITY, f64::NAN] {
                let mut game = world(tourism);
                correct_city_culture(&mut game, 1, adjustment);
                let ai = forecaster(revised);
                let expected = full_score(&ai, &game);
                for minimum in [0, 1, 60, 100, 101] {
                    assert_eq!(
                        ai.culture_lane_forecast_reaches(&game, 0, minimum),
                        expected >= minimum,
                        "version {revised}, correction {adjustment}, rate {tourism}, threshold {minimum}"
                    );
                }
            }
        }
    }
}

#[test]
fn thresholds_use_the_original_rounding_at_the_score_boundary() {
    for revised in [false, true] {
        for minimum in [1, 60, 100] {
            for offset in [-1e-10, 0.0, 1e-10] {
                let mut game = world(1.0);
                let bar = (1..4)
                    .map(|rival| {
                        game.domestic_tourists(rival) as f64
                            + game
                                .player_city_ids(rival)
                                .into_iter()
                                .map(|city| game.city_yields(city).culture)
                                .sum::<f64>()
                                * 190.0
                                / 100.0
                    })
                    .fold(0.0_f64, f64::max);
                let tourism = (f64::from(minimum) - 0.5 + offset) * bar / 100.0
                    * (4.0 * crate::game::TOURISM_PER_VISITOR)
                    / (3.0 * 190.0);
                std::sync::Arc::make_mut(&mut game.observed_tourism_per_turn).insert(0, tourism);
                game.board_written();
                let ai = forecaster(revised);
                assert_eq!(
                    ai.culture_lane_forecast_reaches(&game, 0, minimum),
                    full_score(&ai, &game) >= minimum,
                    "version {revised}, threshold {minimum}, boundary offset {offset}"
                );
            }
        }
    }
}

#[test]
fn threshold_traces_ask_every_question_of_the_full_score() {
    for revised in [false, true] {
        for tourism in [0.0, 10.0] {
            let game = world(tourism);
            let ai = forecaster(revised);
            let reference = game.trace_policy_reads(0);
            let expected_score = ai.culture_lane_forecast_score_with(&game, 0, true);
            let expected = reference.finish();
            drop(reference);

            let trace = game.trace_policy_reads(0);
            assert_eq!(
                ai.culture_lane_forecast_reaches(&game, 0, 60),
                expected_score >= 60
            );
            let actual = trace.finish();
            drop(trace);
            assert_eq!(actual.is_opaque(), expected.is_opaque());
            for card in game.rules.policies.keys() {
                assert_eq!(
                    actual.card_is_inert(&game.rules, *card),
                    expected.card_is_inert(&game.rules, *card),
                    "same policy dependencies for {card} at Tourism {tourism}"
                );
            }
        }
    }
}

#[test]
fn first_rival_bound_preserves_religion_borders_and_team_rules() {
    for kind in ["relic", "writing"] {
        let mut game = Game::new(2, 24, 16, 9_136_200, 250, 0);
        game.turn = 150;
        for pid in 0..2 {
            game.players[pid].civ = "Rome".to_string();
            game.players[pid].religion = Some("Home Faith".to_string());
            game.players[pid].culture_lifetime = 1_000.0;
            let position = game
                .units
                .values()
                .find(|unit| unit.owner == pid && unit.kind == "settler")
                .unwrap()
                .pos;
            game.found_city_for(pid, position, None);
        }
        game.grant_great_work(0, kind, 0, "first rival fixture");
        assert_eq!(game.religious_tourism_per_turn(0) > 0.0, kind == "relic");
        for phase in 0..6 {
            match phase {
                1 => game.players[1].religion = Some("Other Faith".to_string()),
                2 => {
                    game.players[1]
                        .civics
                        .insert(crate::name!("the_enlightenment"));
                }
                3 => {
                    game.players[1]
                        .open_borders_until
                        .insert(0, game.turn + 100);
                }
                4 => {
                    game.players[1].open_borders_until.insert(0, game.turn);
                }
                5 => {
                    game.players[0].team = Some(7);
                    game.players[1].team = Some(7);
                }
                _ => {}
            }
            game.board_written();
            for revised in [false, true] {
                let ai = forecaster(revised);
                let expected = full_score(&ai, &game);
                for minimum in [1, expected - 1, expected, expected + 1, 60, 100] {
                    assert_eq!(
                        ai.culture_lane_forecast_reaches(&game, 0, minimum),
                        expected >= minimum,
                        "work {kind}, market phase {phase}, curve {revised}, threshold {minimum}"
                    );
                }
            }
        }
    }
}
