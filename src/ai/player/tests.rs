use super::*;
use crate::ai::{run_game, Ai};

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
        let a = begin_player_turn(&mut native, &mut first, 0, &mapped);
        let b = begin_player_turn(&mut external, &mut second, 0, &mapped);
        native.plan_observed_turn(&mut first, 0);
        external.plan_observed_turn(&mut second, 0);
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
