//! Focused tests for the soft city in reach
//! (`first-capital-strikes-the-open-window`, `advanced/soft_city_window.rs`).
//!
//! The board is the live shape: our capital, a rival whose capital stands
//! far away, and a city it founded six tiles from ours with no walls.

use super::super::{GrandStrategy, StrategicPlan, VictoryTarget};
use super::*;
use crate::game::Game;
use crate::name;
use crate::Pos;
use std::sync::Arc;

const FORWARD: Pos = (12, 12);

/// A flat 60x30 board: we hold (6, 12), seat 1 holds (30, 12) and the
/// forward city at [`FORWARD`]; every starting unit cleared, nobody at war,
/// Online speed, turn 60. Returns the board and the forward city.
fn board() -> (Game, u32) {
    let mut game = Game::new_full(2, 60, 30, 90_909, 1_000, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    game.found_city_for(0, (6, 12), None);
    game.found_city_for(1, (30, 12), None);
    let forward = game.found_city_for(1, FORWARD, None);
    game.at_war.clear();
    game.game_speed = crate::setup::GameSpeed::Online;
    game.turn = 60;
    game.current = 0;
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    Arc::make_mut(&mut game.observed_city_strength).insert(forward, 13.0);
    (game, forward)
}

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_early_conquest_opening();
    ai.enable_first_capital_before_the_walls();
    if gene {
        ai.enable_first_capital_strikes_the_open_window();
    }
    ai
}

fn plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 10,
        assessed_turn: 60,
        rush: false,
    }
}

/// `count` Warriors of ours two and three tiles west of the forward city.
fn warriors(game: &mut Game, count: usize) {
    for pos in [(10, 12), (10, 11), (10, 13), (9, 12)]
        .into_iter()
        .take(count)
    {
        game.spawn_test_unit("warrior", 0, pos);
    }
}

#[test]
fn three_warriors_beside_an_unwalled_city_open_the_war() {
    let (mut game, forward) = board();
    warriors(&mut game, 3);
    assert_eq!(
        seat(false).soft_city_in_reach(&game, 0, &plan()),
        None,
        "off: nothing is read"
    );
    let mut ai = seat(true);
    let soft = ai
        .soft_city_in_reach(&game, 0, &plan())
        .expect("the forward city is in reach");
    assert_eq!((soft.rival, soft.city, soft.bodies), (1, forward, 3));
    assert!(soft.blows > soft.needed, "{soft:?}");
    assert!(ai.soft_city_window_opening(&mut game, 0, &plan()));
    assert!(game.is_at_war(0, 1));
    // At war nothing more is declared.
    assert_eq!(ai.soft_city_in_reach(&game, 0, &plan()), None);
}

#[test]
fn each_condition_holds_the_war() {
    // Two soldiers are too few.
    let (mut game, _) = board();
    warriors(&mut game, 2);
    let ai = seat(true);
    assert_eq!(ai.soft_city_in_reach(&game, 0, &plan()), None);

    // Walls.
    let (mut game, forward) = board();
    warriors(&mut game, 3);
    Arc::make_mut(&mut game.observed_city_max_wall_hp).insert(forward, 100);
    assert_eq!(ai.soft_city_in_reach(&game, 0, &plan()), None);

    // A centre more than ten above a Warrior.
    let (mut game, forward) = board();
    warriors(&mut game, 3);
    Arc::make_mut(&mut game.observed_city_strength).insert(forward, 31.0);
    assert_eq!(ai.soft_city_in_reach(&game, 0, &plan()), None);

    // A centre three Warriors do not take in two turns.
    let (mut game, forward) = board();
    warriors(&mut game, 3);
    Arc::make_mut(&mut game.observed_city_strength).insert(forward, 28.0);
    assert_eq!(ai.soft_city_in_reach(&game, 0, &plan()), None);

    // An owner at more than 1.3 times our power.
    let (mut game, _) = board();
    warriors(&mut game, 3);
    let ours = game.military_power(0);
    Arc::make_mut(&mut game.observed_military_power).insert(1, 1.5 * ours);
    assert_eq!(ai.soft_city_in_reach(&game, 0, &plan()), None);

    // A threatened home city, and a Recovery plan.
    let (mut game, _) = board();
    warriors(&mut game, 3);
    let home = game.city_at((6, 12));
    let threatened = StrategicPlan {
        threatened_city: home,
        ..plan()
    };
    assert_eq!(ai.soft_city_in_reach(&game, 0, &threatened), None);
    let recovery = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        ..plan()
    };
    assert_eq!(ai.soft_city_in_reach(&game, 0, &recovery), None);
    assert!(ai.soft_city_in_reach(&game, 0, &plan()).is_some());
}
