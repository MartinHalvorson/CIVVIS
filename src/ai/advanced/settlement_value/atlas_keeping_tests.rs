//! The settlement atlas keeps a site's static value across a restart only
//! when nothing it reads has moved, so a game plays exactly as it did when
//! every restart started empty.
use super::super::*;
use super::{SettlementAtlasTest, SETTLEMENT_ATLAS_TEST};
use crate::ai::run_game;
use crate::game::GameOptions;

/// Long enough for every seat to settle its first few cities, which is when
/// the atlas is asked the most.
const TURNS: u32 = 70;

/// One native game of four production players and three city-states, the
/// atlas set up as `test` says. Returns the final board and how many kept
/// values the audit checked.
fn played(seed: u64, test: SettlementAtlasTest) -> (Game, u64) {
    SETTLEMENT_ATLAS_TEST.with(|cell| cell.set(test));
    let mut game = Game::new_with(GameOptions {
        speed: "online".to_string(),
        ..GameOptions::new(4, 44, 28, seed, TURNS, 3)
    });
    let mut ais: Vec<AdvancedAi> = (0..game.players.len())
        .map(|_| {
            let mut ai = AdvancedAi::new();
            ai.enable_engine_repairs();
            ai
        })
        .collect();
    run_game(&mut game, &mut ais);
    let audited = SETTLEMENT_ATLAS_TEST.with(|cell| cell.get().audited);
    SETTLEMENT_ATLAS_TEST.with(|cell| cell.set(SettlementAtlasTest::default()));
    (game, audited)
}

fn saved(game: &Game) -> String {
    serde_json::to_string(game).expect("a game serializes")
}

/// Keeping is an optimization: the board at the end must be the board an
/// atlas that forgot everything at each restart reaches, field for field.
#[test]
fn an_atlas_that_keeps_sites_plays_the_game_it_played_without() {
    let (forgot, _) = played(
        3_770_001,
        SettlementAtlasTest {
            forget: true,
            ..SettlementAtlasTest::default()
        },
    );
    let (kept, _) = played(3_770_001, SettlementAtlasTest::default());
    assert_eq!(kept.turn, forgot.turn);
    assert!(
        saved(&kept) == saved(&forgot),
        "keeping settlement sites changed the game"
    );
}

/// Every value a restart keeps is the value the site derives on the board it
/// is asked about, and the run really kept some.
#[test]
fn every_kept_site_derives_what_it_kept() {
    let (_, audited) = played(
        3_770_002,
        SettlementAtlasTest {
            audit: true,
            ..SettlementAtlasTest::default()
        },
    );
    assert!(audited > 0, "the audit must check kept values, or this proves nothing");
}
