use super::{choose_joint_axes, Doctrine, StrategicAi, FIRST_REVIEW_TURN};
use crate::ai::{run_game, Ai, BasicAi, VictoryTarget, Weights};
use crate::game::{Action, Game};
use crate::valuenet::ValueNet;

fn found_capitals(game: &mut Game, players: usize) {
    for pid in 0..players {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.current = pid;
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    game.current = 0;
}

#[test]
fn terminal_tempo_is_lexicographic_and_default_play_is_unchanged() {
    let mut game = Game::new_full(3, 20, 14, 27, 100, 0, false);
    game.turn = 25;

    let stock = StrategicAi::new();
    game.winner = Some(0);
    assert_eq!(stock.branch_value(&game, 0), 1.0);
    game.winner = Some(1);
    assert_eq!(stock.branch_value(&game, 0), 0.0);

    let mut tempo = StrategicAi::new();
    tempo.terminal_tempo = true;
    game.winner = Some(0);
    let win = tempo.branch_value(&game, 0);
    assert!((win - 2.75).abs() < 1e-12);
    game.winner = None;
    let unresolved = tempo.branch_value(&game, 0);
    game.winner = Some(1);
    let loss = tempo.branch_value(&game, 0);
    assert!((loss - 0.25).abs() < 1e-12);
    assert!(win > unresolved && unresolved > loss);

    game.turn = 60;
    game.winner = Some(0);
    assert!(win > tempo.branch_value(&game, 0), "faster wins rank first");
    game.winner = Some(1);
    assert!(
        tempo.branch_value(&game, 0) > loss,
        "later losses rank first"
    );
}

#[test]
fn religious_finisher_applies_only_an_exact_conversion_gain() {
    let mut game = Game::new_full(3, 20, 14, 28, 120, 0, false);
    found_capitals(&mut game, 3);
    game.players[0].religion = Some("Home Faith".to_string());
    let target = game.player_city_ids(1)[0];
    let target_city = game.cities.get_mut(&target).unwrap();
    target_city.atheist_pressure = 0.0;
    target_city.pressure.insert("Rival Faith".to_string(), 50.0);
    let missionary = game.spawn_test_unit("missionary", 0, game.cities[&target].pos);
    game.units.get_mut(&missionary).unwrap().religion = Some("Home Faith".to_string());
    game.current = 0;

    let stock = StrategicAi::new();
    assert!(!stock.religious_finish_search);
    assert!(!stock.religious_checkmate_search);
    let before = StrategicAi::conversion_value(&game, 0).unwrap();
    let plan = stock.religious_action_plan(&game, 0, false);
    assert!(
        matches!(
            plan.as_slice(),
            [Action::Move { unit, .. }, Action::Spread { unit: spread }]
                if *unit == missionary && *spread == missionary
        ),
        "the exact search should find the stronger move-then-spread sequence: {plan:?}"
    );
    assert!(
        game.cities
            .values()
            .all(|city| city.pressure.get("Home Faith").copied().unwrap_or(0.0) == 0.0),
        "planning must not mutate the live game"
    );

    assert!(
        stock.religious_action_plan(&game, 0, true).is_empty(),
        "an exact local gain must not masquerade as checkmate"
    );
    let converted = game.player_city_ids(2)[0];
    game.cities
        .get_mut(&converted)
        .unwrap()
        .pressure
        .insert("Home Faith".to_string(), 10_000.0);
    let checkmate = stock.religious_action_plan(&game, 0, true);
    assert!(
        !checkmate.is_empty(),
        "the exact winning sequence must be found"
    );
    assert_eq!(game.winner, None, "search must not mutate the live game");
    let mut checkmate_game = game.clone();
    assert!(stock.apply_religious_action_plan(&mut checkmate_game, 0, true));
    assert_eq!(checkmate_game.winner, Some(0));

    assert!(stock.apply_religious_conversion_plan(&mut game, 0));
    assert_eq!(game.winner, Some(0));
    let after = StrategicAi::conversion_value(&game, 0).unwrap();
    assert!(after.better_than(&before));
    assert!(game.cities.values().any(|city| {
        city.owner != 0 && city.pressure.get("Home Faith").copied().unwrap_or(0.0) > 0.0
    }));
    assert!(
        stock.religious_action_plan(&game, 2, false).is_empty(),
        "a civilization without a founded religion has no conversion plan"
    );
    game.players[0].team = Some(0);
    assert!(
        StrategicAi::conversion_value(&game, 0).is_none(),
        "single-founder geometry must not stand in for team victory"
    );
}

#[test]
fn default_macro_search_looks_forty_rounds_ahead() {
    assert_eq!(StrategicAi::new().horizon, 40);
}

#[test]
fn rival_lane_model_is_off_by_default() {
    assert!(!StrategicAi::new().model_rival_lanes);
}

#[test]
fn rival_lane_inference_abstains_before_public_progress_separates() {
    let mut game = Game::new(4, 20, 14, 200, 12, 0);
    game.turn = FIRST_REVIEW_TURN;
    assert_eq!(StrategicAi::new().inferred_rival_target(&game, 1), None);
}

#[test]
fn rival_lane_inference_reads_public_science_religion_and_diplomacy_state() {
    let strategic = StrategicAi::new();

    let mut science = Game::new(4, 20, 14, 201, 12, 0);
    science.turn = FIRST_REVIEW_TURN;
    let needed = science.rules.techs.len().div_ceil(6);
    let techs: Vec<crate::name::Name> = science.rules.techs.keys().take(needed).cloned().collect();
    science.players[1].techs.extend(techs);
    assert_eq!(
        strategic.inferred_rival_target(&science, 1),
        Some(VictoryTarget::Science)
    );

    let mut religion = Game::new(4, 20, 14, 202, 12, 0);
    religion.turn = FIRST_REVIEW_TURN;
    religion.players[1].religion = Some("Visible Faith".to_string());
    assert_eq!(
        strategic.inferred_rival_target(&religion, 1),
        Some(VictoryTarget::Religion)
    );

    let mut diplomacy = Game::new(4, 20, 14, 203, 12, 0);
    diplomacy.turn = FIRST_REVIEW_TURN;
    diplomacy.players[1].dvp = 10;
    assert_eq!(
        strategic.inferred_rival_target(&diplomacy, 1),
        Some(VictoryTarget::Diplomacy)
    );
}

#[test]
fn score_only_control_refuses_a_runtime_value_model() {
    assert!(StrategicAi::score_only_with_weights(Default::default())
        .net
        .is_none());
}

#[test]
fn counterfactual_samples_cover_the_evaluated_endpoint_distribution() {
    // Three players avoid the duel-religion prior, which intentionally
    // bypasses value rollouts. A one-round horizon keeps the seven exact
    // branch continuations cheap while still exercising terminal labels.
    let game = Game::new(3, 20, 14, 200, 12, 0);
    let mut strategic = StrategicAi::score_only_with_weights(Default::default());
    strategic.horizon = 1;

    let samples = strategic.counterfactual_value_samples_with_observation(&game, 0, true);
    assert_eq!(samples.len(), 1 + VictoryTarget::ALL.len());
    assert_eq!(
        samples,
        strategic.counterfactual_value_samples_with_observation(&game, 0, true),
        "branch endpoints and their terminal labels must be deterministic"
    );
    assert!(samples.iter().all(|sample| {
        sample.planes.len() == 25 * 20 * 14
            && sample.globals.len() == sample.global_names.len()
            && !sample.global_names.is_empty()
    }));
    assert_eq!(samples[0].target, None);
    assert_eq!(
        samples
            .iter()
            .skip(1)
            .filter_map(|sample| sample.target)
            .collect::<Vec<_>>(),
        VictoryTarget::ALL
    );
    assert!(samples.iter().all(|sample| {
        sample.features.len() == 25
            && sample.features.iter().all(|value| value.is_finite())
            && sample.turn_fraction > 0.0
    }));
}

#[test]
fn learned_values_are_regularized_toward_score_share() {
    let game = Game::new(4, 24, 16, 20, 180, 0);
    let total: i64 = game
        .players
        .iter()
        .filter(|player| !player.is_minor && !player.is_barbarian)
        .map(|player| game.score(player.id).max(0))
        .sum();
    let score_share = if total > 0 {
        game.score(0).max(0) as f64 / total as f64
    } else {
        0.5
    };
    let mut strategic = StrategicAi::new();
    strategic.net = None;
    assert!((strategic.position_value(&game, 0) - score_share).abs() < 1e-12);
    strategic.net = Some(ValueNet {
        sizes: vec![25, 64, 32, 1],
        weights: vec![
            vec![vec![0.0; 64]; 25],
            vec![vec![0.0; 32]; 64],
            vec![vec![0.0; 1]; 32],
        ],
        biases: vec![vec![0.0; 64], vec![0.0; 32], vec![0.0; 1]],
    });

    let expected = score_share + 0.25 * (0.5 - score_share);
    assert!((strategic.position_value(&game, 0) - expected).abs() < 1e-12);
}

/// Rotation must still examine every lane, just on a cycle. A subset
/// that permanently excluded a lane would not be a cheaper search, it
/// would be a smaller game.
#[test]
fn rotation_reaches_every_enabled_lane_on_a_cycle() {
    let g = Game::new(4, 28, 18, 41000, 200, 2);
    let enabled: Vec<VictoryTarget> = VictoryTarget::ALL
        .into_iter()
        .filter(|target| {
            let mut probe = StrategicAi::new();
            probe.rotate_lanes = false;
            probe.lane_candidates(&g).contains(target)
        })
        .collect();
    assert!(enabled.len() > 2, "need a real lane set to rotate over");
    let mut seen: Vec<VictoryTarget> = Vec::new();
    let mut rotating = StrategicAi::new();
    rotating.rotate_lanes = true;
    for _ in 0..enabled.len() * 2 {
        let candidates = rotating.lane_candidates(&g);
        assert!(
            candidates.len() <= 2,
            "a rotating review projected {} lanes",
            candidates.len()
        );
        for target in candidates {
            if !seen.contains(&target) {
                seen.push(target);
            }
        }
        rotating.advance_rotation_for_test();
    }
    for target in &enabled {
        assert!(
            seen.contains(target),
            "{target:?} never came up in two full cycles"
        );
    }
}

/// Without rotation the candidate set must be exactly what it always
/// was, so the default agent is untouched.
#[test]
fn rotation_is_off_by_default_and_projects_every_lane() {
    let g = Game::new(4, 28, 18, 41000, 200, 2);
    let plain = StrategicAi::new();
    assert!(!plain.rotate_lanes);
    let all: Vec<VictoryTarget> = VictoryTarget::ALL
        .into_iter()
        .filter(|target| plain.lane_candidates(&g).contains(target))
        .collect();
    assert_eq!(plain.lane_candidates(&g), all);
    assert!(all.len() > 2);
}

/// Halving the review period must actually buy reviews. This is the
/// mechanism behind the cadence result: if the counter never moved, the
/// dose would be nominal and the measured win difference would have to
/// come from somewhere else.
#[test]
fn a_shorter_review_period_reaches_the_search_more_often() {
    let census = |review_every: u32| {
        let mut g = Game::new(4, 28, 18, 41001, 160, 2);
        let mut strategic = StrategicAi::new();
        strategic.review_every = review_every;
        strategic.horizon = 4;
        let mut others = BasicAi::fleet(&g);
        while g.winner.is_none() && g.turn <= g.max_turns {
            let pid = g.current;
            if pid == 0 {
                strategic.take_turn(&mut g, pid);
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &Action::EndTurn);
            }
        }
        strategic.review_census()
    };
    let sparse = census(40);
    let dense = census(10);
    assert!(
        dense.rollouts > sparse.rollouts,
        "cadence 10 reached the rollouts {} times against 40's {}",
        dense.rollouts,
        sparse.rollouts
    );
}

/// A prior-driven interrupt must not postpone the periodic search when
/// the agent has opted out of deferring. The shipped default still
/// defers, so this is a behaviour switch and not a silent change.
#[test]
fn an_interrupt_need_not_postpone_the_periodic_search() {
    assert!(
        StrategicAi::new().defer_periodic_on_interrupt,
        "the shipped default must be unchanged"
    );
    let census = |defer: bool| {
        let mut g = Game::new(4, 28, 18, 41003, 170, 2);
        let mut strategic = StrategicAi::new();
        strategic.defer_periodic_on_interrupt = defer;
        strategic.horizon = 4;
        let mut others = BasicAi::fleet(&g);
        while g.winner.is_none() && g.turn <= g.max_turns {
            let pid = g.current;
            if pid == 0 {
                strategic.take_turn(&mut g, pid);
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &Action::EndTurn);
            }
        }
        strategic.review_census()
    };
    let deferring = census(true);
    let holding = census(false);
    assert!(
        holding.total() >= deferring.total(),
        "holding the schedule reviewed {} times against {}",
        holding.total(),
        deferring.total()
    );
}

/// Every doctrine must stay inside the bounds evolution respects, so a
/// multiple of an already-extreme evolved gene cannot produce a genome
/// that is not a play style at all.
#[test]
fn doctrines_stay_inside_the_evolution_bounds() {
    let extreme = Weights::from_vec(
        &Weights::bounds()
            .iter()
            .map(|(_, high)| *high)
            .collect::<Vec<f64>>(),
    );
    for base in [Weights::default(), extreme] {
        for doctrine in Doctrine::ALL {
            let genes = doctrine.apply(&base).to_vec();
            for (index, (gene, (low, high))) in genes.iter().zip(Weights::bounds()).enumerate() {
                assert!(
                    *gene >= low && *gene <= high,
                    "{} gene {index} = {gene} escaped [{low}, {high}]",
                    doctrine.name()
                );
            }
        }
    }
}

/// The incumbent must be the evolved genome untouched, so a tie leaves
/// the tuned agent exactly as it was.
#[test]
fn the_incumbent_doctrine_changes_nothing() {
    let base = Weights::default();
    assert_eq!(Doctrine::Incumbent.apply(&base).to_vec(), base.to_vec());
    for doctrine in [
        Doctrine::Expand,
        Doctrine::Consolidate,
        Doctrine::Militarize,
    ] {
        assert_ne!(
            doctrine.apply(&base).to_vec(),
            base.to_vec(),
            "{} is indistinguishable from the incumbent",
            doctrine.name()
        );
    }
}

/// The default agent must be untouched by this feature: `strategic` is
/// a published entrant and the A/B is only meaningful if the control
/// did not move.
#[test]
fn doctrine_search_is_off_by_default() {
    let plain = StrategicAi::new();
    assert!(!plain.doctrine_search);
    assert!(!plain.joint_axis_search);
    assert_eq!(plain.doctrine(), Doctrine::Incumbent);
    assert_eq!(plain.doctrine_switches(), 0);
}

#[test]
fn joint_search_recovers_an_interacting_lane_and_doctrine() {
    let lanes = vec![
        None,
        Some(VictoryTarget::Science),
        Some(VictoryTarget::Domination),
    ];
    let values = vec![
        vec![0.50, 0.50, 0.50, 0.50],
        vec![0.52, 0.53, 0.51, 0.50],
        vec![0.49, 0.50, 0.50, 0.56],
    ];

    let (choice, sequential) = choose_joint_axes(&lanes, &values, Doctrine::Incumbent);
    assert_eq!(choice.target, Some(VictoryTarget::Domination));
    assert_eq!(choice.doctrine, Doctrine::Militarize);
    assert_eq!(choice.value, 0.56);
    assert_eq!(sequential.target, Some(VictoryTarget::Science));
    assert_eq!(sequential.doctrine, Doctrine::Expand);
}

#[test]
fn joint_search_keeps_the_sequential_choice_below_the_lane_margin() {
    let lanes = vec![
        None,
        Some(VictoryTarget::Science),
        Some(VictoryTarget::Domination),
    ];
    let values = vec![
        vec![0.50, 0.50, 0.50, 0.50],
        vec![0.52, 0.53, 0.51, 0.50],
        vec![0.49, 0.50, 0.50, 0.539],
    ];

    let (choice, sequential) = choose_joint_axes(&lanes, &values, Doctrine::Incumbent);
    assert_eq!(choice.target, Some(VictoryTarget::Science));
    assert_eq!(choice.doctrine, Doctrine::Expand);
    assert_eq!(choice.value, 0.53);
    assert_eq!(choice, sequential);
}

#[test]
fn joint_comparator_reproduces_the_base_genome_lane_pass_after_a_doctrine_switch() {
    let lanes = vec![None, Some(VictoryTarget::Domination)];
    // Under the evolved base genome (column 0), Domination loses to the
    // adaptive lane. Under the currently active Expand doctrine (column
    // 1) it wins. The shipped coordinate descent uses column 0 for its
    // lane pass, then compares doctrines on that selected lane.
    let values = vec![vec![0.50, 0.10, 0.09, 0.08], vec![0.49, 0.30, 0.20, 0.10]];

    let (choice, sequential) = choose_joint_axes(&lanes, &values, Doctrine::Expand);
    assert_eq!(sequential.target, None);
    assert_eq!(sequential.doctrine, Doctrine::Incumbent);
    assert_eq!(choice, sequential);
}

/// With the axis on, a real game must reach the doctrine search and the
/// agent must still finish a legal game. Repeating the same seed must
/// give the same doctrine: the rollouts are seed-free clones, so the
/// second axis has to be as deterministic as the first.
#[test]
fn doctrine_search_runs_and_is_deterministic() {
    let play = || {
        let mut g = Game::new(4, 28, 18, 31, 130, 2);
        let mut strategic = StrategicAi::new();
        strategic.doctrine_search = true;
        strategic.horizon = 4;
        let mut others = BasicAi::fleet(&g);
        while g.winner.is_none() && g.turn <= g.max_turns {
            let pid = g.current;
            if pid == 0 {
                strategic.take_turn(&mut g, pid);
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &Action::EndTurn);
            }
        }
        (
            strategic.review_census(),
            strategic.doctrine(),
            strategic.doctrine_switches(),
        )
    };
    let first = play();
    assert!(
        first.0.rollouts > 0,
        "the lane search never ran, so the doctrine axis was never reached: {:?}",
        first.0
    );
    assert_eq!(first, play(), "the doctrine axis must be deterministic");
}

/// A two-player game with religion enabled is answered by the duel
/// prior, so the economic rollouts — and the value evaluator with them —
/// never run. Every published duel number for this agent measures a
/// forced religious lane rather than macro search, which is exactly what
/// the census exists to say out loud.
#[test]
fn a_duel_never_reaches_the_rollouts() {
    let mut g = Game::new(2, 20, 14, 21, 200, 0);
    assert!(g.victory_conditions.religious);
    let mut strategic = StrategicAi::new();
    strategic.horizon = 4;
    let mut ais = BasicAi::fleet(&g);
    for _ in 0..70 {
        if g.winner.is_some() {
            break;
        }
        for (pid, ai) in ais.iter_mut().enumerate() {
            if g.winner.is_some() {
                break;
            }
            if pid == 0 {
                strategic.take_turn(&mut g, pid);
            } else {
                ai.take_turn(&mut g, pid);
            }
        }
    }
    let census = strategic.review_census();
    assert!(census.total() > 0, "the agent must have reviewed at all");
    assert_eq!(
        census.rollouts, 0,
        "a duel reached the rollouts {} times: {census:?}",
        census.rollouts
    );
    assert_eq!(census.search_exposure(), Some(0.0));
    assert_eq!(strategic.current_target(), Some(VictoryTarget::Religion));
}

/// With religion switched off the duel prior cannot fire, so the same
/// two-player position does reach the search. This is the control that
/// keeps the test above a statement about the prior rather than about
/// two-player games.
#[test]
fn a_duel_without_a_religious_race_does_reach_the_rollouts() {
    let mut g = Game::new(2, 20, 14, 21, 200, 0);
    g.victory_conditions.religious = false;
    let mut strategic = StrategicAi::new();
    strategic.horizon = 3;
    let mut ais = BasicAi::fleet(&g);
    for _ in 0..40 {
        if g.winner.is_some() {
            break;
        }
        for (pid, ai) in ais.iter_mut().enumerate() {
            if g.winner.is_some() {
                break;
            }
            if pid == 0 {
                strategic.take_turn(&mut g, pid);
            } else {
                ai.take_turn(&mut g, pid);
            }
        }
    }
    let census = strategic.review_census();
    assert!(
        census.rollouts > 0,
        "the search never ran without a religious race: {census:?}"
    );
    assert_eq!(census.duel_religion, 0);
}

/// An agent with no search reports none, so an evaluator can tell
/// "searched zero times" from "has nothing to search".
#[test]
fn a_scripted_agent_reports_no_census() {
    assert_eq!(BasicAi::new().review_census(), None);
    assert_eq!(
        crate::ai::AdvancedAi::new().review_census(),
        None,
        "the scripted parent has no macro search of its own"
    );
    // Reached through the trait, where an evaluator sees it: the
    // inherent method returns the census itself.
    assert!(Ai::review_census(&StrategicAi::new()).is_some());
}

/// The review must pick a lane deterministically and commit it to the
/// wrapped agent; a tiny horizon keeps the six rollouts fast.
#[test]
fn review_selects_and_commits_a_victory_lane() {
    let mut g = Game::new(2, 20, 14, 21, 120, 0);
    let mut ais = BasicAi::fleet(&g);
    for _ in 0..12 {
        for (pid, ai) in ais.iter_mut().enumerate() {
            if g.winner.is_some() {
                break;
            }
            ai.take_turn(&mut g, pid);
        }
    }
    let mut strategic = StrategicAi::new();
    strategic.horizon = 4;
    strategic.next_review = 0;
    let first = strategic.review(&g, 0);
    assert_eq!(
        first,
        strategic.review(&g, 0),
        "review must be deterministic"
    );
    assert_eq!(strategic.current_target(), None);
    strategic.take_turn(&mut g, 0);
    assert_eq!(strategic.current_target(), first);
}

#[test]
fn religious_match_point_interrupts_the_economic_rollouts() {
    let mut game = Game::new_full(2, 24, 16, 22, 180, 0, false);
    found_capitals(&mut game, 2);
    game.players[0].religion = Some("Home Faith".to_string());
    game.players[1].religion = Some("Rival Faith".to_string());
    for (owner, religion) in [(0, "Home Faith"), (1, "Rival Faith")] {
        let capital = game.player_city_ids(owner)[0];
        game.cities
            .get_mut(&capital)
            .unwrap()
            .pressure
            .insert(religion.to_string(), 1_000.0);
    }

    let strategic = StrategicAi::new();
    assert_eq!(
        strategic.urgent_counter_target(&game, 0),
        Some(VictoryTarget::Religion)
    );
    assert_eq!(strategic.review(&game, 0), Some(VictoryTarget::Religion));

    game.players[0].religion = None;
    assert_eq!(
        strategic.urgent_counter_target(&game, 0),
        Some(VictoryTarget::Domination)
    );
}

#[test]
fn strategic_review_preserves_a_viable_prophet_investment() {
    let mut game = Game::new_full(3, 24, 16, 24, 180, 0, false);
    found_capitals(&mut game, 3);
    game.players[0].research = Some("astrology".to_string());

    assert!(StrategicAi::viable_religious_commitment(&game, 0));
    assert_eq!(
        StrategicAi::new().review(&game, 0),
        Some(VictoryTarget::Religion)
    );

    game.victory_conditions.religious = false;
    assert!(!StrategicAi::viable_religious_commitment(&game, 0));
}

#[test]
fn duel_treats_the_prophet_race_as_a_mandatory_objective() {
    let mut game = Game::new_full(2, 24, 16, 26, 180, 0, false);
    found_capitals(&mut game, 2);
    let strategic = StrategicAi::new();

    assert!(StrategicAi::duel_religious_race(&game, 0));
    assert_eq!(strategic.review(&game, 0), Some(VictoryTarget::Religion));

    game.victory_conditions.religious = false;
    assert!(!StrategicAi::duel_religious_race(&game, 0));
}

/// A treatment has to be shown to fire before it is worth evaluating.
/// #380 cost a 240-game run to discover its treatment was inert with
/// every diagnostic identical to the digit, so this asserts directly
/// that projecting from the plan in force changes the numbers the
/// search reads — on the same agent, in the same position, with only
/// the flag moved.
#[test]
fn projecting_from_the_plan_in_force_changes_what_the_search_sees() {
    let mut changed = 0;
    let mut checked = 0;
    for seed in [31u64, 57, 83] {
        let mut g = Game::new(3, 20, 14, seed, 200, 0);
        let mut strategic = StrategicAi::with_weights(Default::default());
        let mut rivals = BasicAi::fleet(&g);
        // Let the searching seat accumulate the plan, campaign and
        // force-group state a cold branch throws away.
        for _ in 0..45 {
            if g.winner.is_some() {
                break;
            }
            for (pid, rival) in rivals.iter_mut().enumerate() {
                if g.winner.is_some() {
                    break;
                }
                if pid == 0 {
                    strategic.take_turn(&mut g, pid);
                } else {
                    rival.take_turn(&mut g, pid);
                }
            }
        }
        if g.winner.is_some() {
            continue;
        }
        checked += 1;
        strategic.continue_from_plan = false;
        let cold = strategic.lane_values(&g, 0);
        strategic.continue_from_plan = true;
        let warm = strategic.lane_values(&g, 0);
        assert_eq!(cold.len(), warm.len());
        assert!(warm.iter().all(|(value, _)| value.is_finite()));
        // Determinism: the same flag must give the same numbers twice.
        assert_eq!(warm, strategic.lane_values(&g, 0));
        if cold
            .iter()
            .zip(&warm)
            .any(|((left, _), (right, _))| left != right)
        {
            changed += 1;
        }
    }
    assert!(checked > 0, "no ordinary mid-game position was reached");
    assert!(
        changed > 0,
        "the warm and cold projections agreed to the digit in all {checked} positions, \
             so the plan in force carries nothing the rollout cannot re-derive and an \
             evaluation would measure nothing"
    );
}

/// #380 spent a 240-game run to discover its treatment was inert, with
/// every diagnostic identical to the digit. This asserts the adaptive
/// horizon actually declines rounds in ordinary positions, so that a
/// pre-registered evaluation is measuring something that happens.
#[test]
fn the_adaptive_horizon_stops_early_in_ordinary_games() {
    let mut checked = 0;
    let mut fired = 0;
    for seed in [31u64, 57, 83] {
        // Three players: a duel is answered by the religion prior and
        // never reaches the rollouts this measures.
        let mut g = Game::new(3, 20, 14, seed, 200, 0);
        let mut strategic = StrategicAi::with_weights(Default::default());
        strategic.adaptive_horizon = true;
        // The promoted deep budget, which is the one the saturation
        // measurement was taken at and the only one this can help: at
        // horizon 40 the median spread between branches is 0.0045
        // *over the undecided subset*, under the 0.01 commitment
        // margin, so those branches never separate and the search runs
        // its full count regardless. (Over all reviews the median is
        // 0.031-0.085 and the margin is cleared 61-94% of the time; the
        // two figures describe different populations. See
        // docs/SUPERHUMAN.md.)
        strategic.horizon = 80;
        let mut ais = BasicAi::fleet(&g);
        for _ in 0..40 {
            if g.winner.is_some() {
                break;
            }
            for (pid, ai) in ais.iter_mut().enumerate() {
                if g.winner.is_some() {
                    break;
                }
                ai.take_turn(&mut g, pid);
            }
        }
        if g.winner.is_some() {
            continue;
        }
        let mut targets: Vec<Option<VictoryTarget>> = vec![None];
        targets.extend(strategic.lane_candidates(&g).into_iter().map(Some));
        let (values, projected) = strategic.lockstep_values_projected(&g, 0, &targets);
        assert_eq!(values.len(), targets.len());
        assert!(values.iter().all(|(value, _)| value.is_finite()));
        checked += 1;
        if projected < strategic.horizon {
            fired += 1;
        }
    }
    assert!(checked > 0, "no ordinary mid-game position was reached");
    assert!(
        fired > 0,
        "the adaptive horizon projected every round in all {checked} positions, \
             so it is inert and an evaluation would measure nothing"
    );
}

#[test]
fn imminent_victory_interrupts_before_the_periodic_review() {
    let mut game = Game::new_full(2, 24, 16, 25, 180, 0, false);
    found_capitals(&mut game, 2);
    game.players[0].religion = Some("Home Faith".to_string());
    game.players[1].religion = Some("Rival Faith".to_string());
    for (owner, religion) in [(0, "Home Faith"), (1, "Rival Faith")] {
        let capital = game.player_city_ids(owner)[0];
        game.cities
            .get_mut(&capital)
            .unwrap()
            .pressure
            .insert(religion.to_string(), 1_000.0);
    }

    let mut strategic = StrategicAi::new();
    strategic.inner.retarget(VictoryTarget::Science);
    strategic.next_review = game.turn + 100;
    strategic.take_turn(&mut game, 0);

    assert_eq!(strategic.current_target(), Some(VictoryTarget::Religion));
    assert!(strategic.next_review < game.turn + 100);
}

#[test]
fn imminent_space_race_routes_to_denial() {
    let mut game = Game::new_full(3, 24, 16, 23, 300, 0, false);
    found_capitals(&mut game, 3);
    for rival in 1..game.players.len() {
        game.record_contact(0, rival);
    }
    game.players[2].science_projects.extend([
        "launch_earth_satellite".to_string(),
        "launch_moon_landing".to_string(),
        "launch_mars_colony".to_string(),
        "exoplanet_expedition".to_string(),
    ]);
    game.players[2].exoplanet_distance = 42.0;

    let mut strategic = StrategicAi::new();
    assert_eq!(
        strategic.urgent_counter(&game, 0),
        Some((2, VictoryTarget::Domination))
    );
    strategic.next_review = game.turn + 100;
    strategic.take_turn(&mut game, 0);
    assert_eq!(strategic.current_target(), Some(VictoryTarget::Domination));
    assert_eq!(
        strategic
            .inner
            .current_plan()
            .and_then(|plan| plan.target_player),
        Some(2),
        "an urgent counter-campaign must pursue the civilization about to win"
    );
}

#[test]
fn urgent_denial_retargets_when_a_different_rival_takes_the_lead() {
    let mut game = Game::new_full(3, 24, 16, 24, 300, 0, false);
    found_capitals(&mut game, 3);
    for rival in 1..game.players.len() {
        game.record_contact(0, rival);
    }
    for rival in [1, 2] {
        game.players[rival]
            .science_projects
            .insert("exoplanet_expedition".to_string());
    }
    game.players[1].exoplanet_distance = 40.0;
    game.players[2].exoplanet_distance = 45.0;

    let mut strategic = StrategicAi::new();
    strategic.next_review = game.turn + 100;
    strategic.take_turn(&mut game, 0);
    assert_eq!(strategic.inner.forced_target_player(), Some(2));

    // The counter-lane remains Domination, but player 1 is now closer to
    // the science victory. Refreshing only on lane changes would leave
    // the campaign pinned to player 2.
    game.players[1].exoplanet_distance = 50.0;
    assert_eq!(
        strategic.urgent_counter(&game, 0),
        Some((1, VictoryTarget::Domination))
    );
    game.current = 0;
    strategic.take_turn(&mut game, 0);

    assert_eq!(strategic.current_target(), Some(VictoryTarget::Domination));
    assert_eq!(strategic.inner.forced_target_player(), Some(1));
    assert_eq!(
        strategic
            .inner
            .current_plan()
            .and_then(|plan| plan.target_player),
        Some(1)
    );
}

#[test]
fn final_space_launch_interrupts_before_the_first_light_year() {
    let mut game = Game::new_full(3, 24, 16, 23_001, 300, 0, false);
    found_capitals(&mut game, 3);
    game.players[2]
        .science_projects
        .insert("exoplanet_expedition".to_string());

    assert_eq!(
        StrategicAi::victory_progress(&game, 2, VictoryTarget::Science),
        78
    );
    assert_eq!(
        StrategicAi::new().urgent_counter_target(&game, 0),
        Some(VictoryTarget::Domination),
        "the macro planner must interrupt before its next expensive review"
    );
}

#[test]
fn explicit_lane_must_clear_the_adaptive_rollout() {
    let strategic = StrategicAi::new();
    assert_eq!(
        strategic.choose_rollout_target(&[(0.30, None), (0.305, Some(VictoryTarget::Domination)),]),
        None
    );
    assert_eq!(
        strategic.choose_rollout_target(&[(0.30, None), (0.32, Some(VictoryTarget::Science)),]),
        Some(VictoryTarget::Science)
    );
}

#[test]
fn periodic_review_can_return_a_targeted_agent_to_adaptive_planning() {
    let mut game = Game::new_full(3, 24, 16, 31, 300, 0, false);
    found_capitals(&mut game, 3);
    game.victory_conditions.culture = false;
    game.victory_conditions.religious = false;
    game.victory_conditions.diplomatic = false;
    game.victory_conditions.domination = false;
    game.victory_conditions.score = false;
    let mut strategic = StrategicAi::new();
    strategic.inner.retarget(VictoryTarget::Science);
    strategic.horizon = 0;
    strategic.next_review = 0;
    assert_eq!(strategic.current_target(), Some(VictoryTarget::Science));
    strategic.take_turn(&mut game, 0);
    assert_eq!(strategic.current_target(), None);
}

#[test]
fn review_never_selects_a_disabled_lane() {
    let mut game = Game::new_full(3, 24, 16, 30, 300, 0, false);
    found_capitals(&mut game, 3);
    game.victory_conditions.culture = false;
    game.victory_conditions.religious = false;
    game.victory_conditions.diplomatic = false;
    game.victory_conditions.domination = false;
    game.victory_conditions.score = false;
    let mut strategic = StrategicAi::new();
    strategic.horizon = 0;

    assert_eq!(strategic.review(&game, 0), None);
}

/// Full smoke game: a strategic seat finishes a real game without
/// upsetting the loop.
#[test]
fn strategic_seat_completes_a_game() {
    let mut g = Game::new(2, 20, 14, 3, 80, 0);
    let mut strategic = StrategicAi::new();
    strategic.horizon = 6;
    strategic.review_every = 25;
    // One AI per player seat, barbarians included.
    let mut ais: Vec<Box<dyn Ai>> = g
        .players
        .iter()
        .map(|p| {
            if p.id == 0 {
                Box::new(std::mem::take(&mut strategic)) as Box<dyn Ai>
            } else {
                Box::new(BasicAi::new()) as Box<dyn Ai>
            }
        })
        .collect();
    run_game(&mut g, &mut ais);
    assert!(g.winner.is_some());
}
