//! `one-replan-per-turn`: with the gene on, a seat's turn re-opens for a
//! fresh view and another plan at most once.
use super::*;
use crate::ai::{run_game, Ai};
use crate::game::GameOptions;

/// The gene is the frame bound and nothing else: one re-plan with it,
/// `REPLAN_FRAMES` without.
#[test]
fn the_gene_allows_one_re_plan() {
    let mut ai = AdvancedAi::new();
    assert_eq!(replan_frames(&ai), REPLAN_FRAMES);
    ai.enable_one_replan_per_turn();
    assert_eq!(replan_frames(&ai), 1);
    ai.disable_one_replan_per_turn();
    assert_eq!(replan_frames(&ai), REPLAN_FRAMES);
}

/// A native game of four production players and three city-states plays to
/// its limit with every major on the gene, and the seats still re-plan: the
/// first re-plan is kept.
#[test]
fn seats_on_the_gene_still_re_plan() {
    let mut game = Game::new_with(GameOptions {
        speed: "online".to_string(),
        ..GameOptions::new(4, 44, 28, 3_760_001, 40, 3)
    });
    let mut ais: Vec<AdvancedAi> = (0..game.players.len())
        .map(|_| {
            let mut ai = AdvancedAi::new();
            ai.enable_engine_repairs();
            ai.enable_one_replan_per_turn();
            ai
        })
        .collect();
    assert!(ais[0].uses_player_observation());
    run_game(&mut game, &mut ais);
    let replans: i64 = game
        .players
        .iter()
        .filter(|player| !player.is_minor && !player.is_barbarian)
        .map(|player| player.counters.get("player:replans").copied().unwrap_or(0))
        .sum();
    assert!(replans > 0, "no seat re-planned at all");
}
