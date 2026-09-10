use super::*;
use crate::ai::{run_game, Ai};

#[test]
fn allocation_does_not_execute_the_stale_ordinary_tail() {
    let mut game = Game::new_full(2, 24, 16, 41, 20, 0, false);
    let founding = game
        .legal_actions(0)
        .into_iter()
        .find(|a| matches!(a, Action::FoundCity { .. }))
        .unwrap();
    let research = game
        .legal_actions(0)
        .into_iter()
        .find(|a| matches!(a, Action::Research { .. }))
        .unwrap();
    let prior_research = game.players[0].research.clone();
    assert!(execute_frame(
        &mut game,
        0,
        &Default::default(),
        [(0, founding), (0, research)].iter()
    ));
    assert_eq!(game.player_city_ids(0).len(), 1);
    assert_eq!(game.players[0].research, prior_research);
}

#[test]
fn finishing_refusal_stops_other_lines_and_ordinary_orders() {
    let mut game = Game::new_full(2, 24, 16, 41, 20, 0, false);
    let target = game.player_unit_ids(1)[0];
    let research = game
        .legal_actions(0)
        .into_iter()
        .find(|a| matches!(a, Action::Research { .. }))
        .unwrap();
    let prior_research = game.players[0].research.clone();
    let finishing = super::super::finishing::WarFinishingVolley {
        execution: vec![
            (target, vec![Action::FoundCity { unit: u32::MAX }]),
            (target, vec![research.clone()]),
        ],
        ..Default::default()
    };
    assert!(execute_frame(
        &mut game,
        0,
        &finishing,
        [(0, research)].iter()
    ));
    assert_eq!(game.players[0].research, prior_research);
    assert_eq!(game.players[0].counters["player:refused"], 1);
}

#[test]
fn refused_action_invalidates_batch_without_mutating_authoritative_assets() {
    let mut game = Game::new_full(2, 24, 16, 41, 20, 0, false);
    let units = game.units.clone();
    let gold = game.players[0].gold;
    assert!(execute_observed_action(
        &mut game,
        0,
        &Action::FoundCity { unit: u32::MAX }
    ));
    assert_eq!(game.players[0].counters["player:refused"], 1);
    assert_eq!(game.players[0].gold, gold);
    assert_eq!(
        serde_json::to_value(game.units.values().collect::<Vec<_>>()).unwrap(),
        serde_json::to_value(units.values().collect::<Vec<_>>()).unwrap()
    );
}

#[test]
fn successful_allocation_requires_new_observation_but_research_does_not() {
    let mut game = Game::new_full(2, 24, 16, 41, 20, 0, false);
    let research = game
        .legal_actions(0)
        .into_iter()
        .find(|a| matches!(a, Action::Research { .. }))
        .unwrap();
    assert!(!execute_observed_action(&mut game, 0, &research));
    let founding = game
        .legal_actions(0)
        .into_iter()
        .find(|a| matches!(a, Action::FoundCity { .. }))
        .unwrap();
    assert!(execute_observed_action(&mut game, 0, &founding));
    assert_eq!(game.player_city_ids(0).len(), 1);
}

#[test]
fn an_executor_never_applies_another_seats_plan_after_turn_changes() {
    let mut game = Game::new_full(2, 24, 16, 41, 20, 0, false);
    game.current = 1;
    let before = serde_json::to_value(&game).unwrap();
    assert!(execute_observed_action(&mut game, 0, &Action::EndTurn));
    assert_eq!(serde_json::to_value(&game).unwrap(), before);
}

#[test]
fn targets_are_seeded_per_seat_without_consuming_genome_randomness() {
    let targets = parse_targets(TRAINING_TARGETS).unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for seed in 0..100 {
        for seat in 0..6 {
            let target = target_for(seed, seat, &targets);
            assert_eq!(target, target_for(seed, seat, &targets));
            seen.insert(target.map_or("civvis", |t| t.as_str()));
        }
    }
    assert_eq!(seen.len(), targets.len());
    assert!(parse_targets("").is_err());
    assert!(parse_targets("science,typo").is_err());
}

#[test]
fn production_adapters_plan_the_same_actions_from_the_same_observation() {
    for seed in [41, 73, 191] {
        let game = Game::new_full(2, 24, 16, seed, 20, 0, false);
        let mut native = AdvancedAi::new();
        native.enable_engine_repairs();
        let mut external = AdvancedAi::new();
        external.enable_live_bridge();
        assert!(native.uses_player_observation());
        assert!(external.uses_player_observation());
        let mut first = game.player_decision_view(0);
        let mut second = first.clone();
        let mapped = first.units.keys().map(|id| (*id, i64::from(*id))).collect();
        let (a, a_begin) = plan_frame(&mut native, &mut first, 0, &mapped);
        let (b, b_begin) = plan_frame(&mut external, &mut second, 0, &mapped);
        assert_eq!(a_begin, b_begin);
        assert_eq!(
            serde_json::to_value(a.actions).unwrap(),
            serde_json::to_value(b.actions).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&first).unwrap(),
            serde_json::to_value(&second).unwrap()
        );
    }
}

#[test]
fn all_six_observed_players_finish_a_native_game() {
    let mut game = Game::new_full(6, 40, 28, 392, 12, 0, false);
    let mut ais: Vec<AdvancedAi> = (0..game.players.len())
        .map(|_| {
            let mut ai = AdvancedAi::new();
            ai.enable_engine_repairs();
            ai
        })
        .collect();
    run_game(&mut game, &mut ais);
    assert!(game.winner.is_some());
    for pid in 0..6 {
        assert!(!game.players[pid].remembered_tiles.is_empty());
        assert!(
            !game.player_city_ids(pid).is_empty(),
            "seat {pid} never founded"
        );
    }
}
