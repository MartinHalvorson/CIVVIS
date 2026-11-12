//! Focused tests for `score-seat-takes-what-pays`
//! (`advanced/score_takes_what_pays.rs`).
//!
//! The board is the live shape: our capital in the west, a neighbour whose
//! capital stands far away and an open city of ten citizens six tiles from
//! ours, read at a centre of 25, with three of our Swordsmen in the field
//! between them. Nobody is at war; turn 60 of 170.

use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::Game;
use crate::name;
use crate::Pos;
use std::sync::Arc;

const FORWARD: Pos = (12, 12);

/// Our capital at (6, 12); seat 1 holds (30, 12) and the open city at
/// [`FORWARD`] (ten citizens); seat 2, met and far away, holds (54, 4).
/// Returns the board and the forward city.
fn board() -> (Game, u32) {
    let mut game = Game::new_full(3, 60, 30, 90_909, 170, 0, false);
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
    game.found_city_for(2, (54, 4), None);
    game.cities.get_mut(&forward).unwrap().pop = 10;
    game.at_war.clear();
    game.game_speed = crate::setup::GameSpeed::Online;
    game.turn = 60;
    game.max_turns = 170;
    game.current = 0;
    for (a, b) in [(0, 1), (0, 2), (1, 2)] {
        game.players[a].met.insert(b);
        game.players[b].met.insert(a);
    }
    Arc::make_mut(&mut game.observed_city_strength).insert(forward, 25.0);
    (game, forward)
}

/// Three Swordsmen of ours two to four tiles west of the forward city.
fn swordsmen(game: &mut Game) -> Vec<u32> {
    [(10, 12), (9, 12), (10, 11)]
        .into_iter()
        .map(|pos| game.spawn_test_unit("swordsman", 0, pos))
        .collect()
}

/// Every rival at a tenth of our power.
fn weak_rivals(game: &mut Game) {
    let ours = game.military_power(0).max(10.0);
    let power = Arc::make_mut(&mut game.observed_military_power);
    power.insert(1, 0.1 * ours);
    power.insert(2, 0.1 * ours);
}

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Score);
    if gene {
        ai.enable_score_seat_takes_what_pays();
    }
    ai
}

fn conquest_plan(city: u32) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(city),
        threatened_city: None,
        desired_cities: 10,
        assessed_turn: 60,
        rush: false,
    }
}

#[test]
fn ships_off() {
    opt_in_off_in_both_controllers("score-seat-takes-what-pays", |ai| {
        ai.score_seat_takes_what_pays
    });
}

#[test]
fn a_city_is_worth_its_score_parts() {
    let (mut game, forward) = board();
    assert_eq!(
        AdvancedAi::score_city_points(&game, forward),
        15.0,
        "5 + 10 citizens"
    );
    let city = game.cities.get_mut(&forward).unwrap();
    city.buildings.push(name!("monument"));
    city.wonders.insert(name!("pyramids"), FORWARD);
    assert_eq!(
        AdvancedAi::score_city_points(&game, forward),
        31.0,
        "+1 building, +15 wonder"
    );
}

#[test]
fn the_open_city_pays_and_is_picked_only_with_the_gene_on_a_score_seat() {
    let (mut game, forward) = board();
    swordsmen(&mut game);
    weak_rivals(&mut game);
    assert_eq!(
        seat(false).score_take_pick(&game, 0, false),
        None,
        "off: nothing"
    );
    let mut domination = AdvancedAi::targeting(VictoryTarget::Domination);
    domination.enable_score_seat_takes_what_pays();
    assert_eq!(
        domination.score_take_pick(&game, 0, false),
        None,
        "a Domination seat: nothing"
    );
    let take = seat(true)
        .score_take_pick(&game, 0, false)
        .expect("the open city pays");
    assert_eq!((take.rival, take.city), (1, forward));
    assert_eq!(take.points, 15.0);
    assert!(take.turns <= SCORE_TAKE_TURNS, "{take:?}");
    assert!(
        take.standing >= 1 && take.bodies <= SCORE_TAKE_MAX_BODIES,
        "{take:?}"
    );
    assert!(AdvancedAi::score_take_pays(&take));
}

#[test]
fn no_paying_city_changes_nothing() {
    // Five points of city: under the bar.
    let (mut game, forward) = board();
    swordsmen(&mut game);
    weak_rivals(&mut game);
    game.cities.get_mut(&forward).unwrap().pop = 3;
    let ai = seat(true);
    assert_eq!(ai.score_take_pick(&game, 0, false), None);
    let off = seat(false).assess(&game, 0);
    let on = ai.assess(&game, 0);
    assert_eq!(on.strategy, off.strategy);
    assert_eq!(on.target_city, off.target_city);
    assert_eq!(on.target_player, off.target_player);
}

#[test]
fn each_circumstance_holds_the_campaign() {
    let ai = seat(true);
    // The owner stronger than us.
    let (mut game, _) = board();
    swordsmen(&mut game);
    let ours = game.military_power(0);
    Arc::make_mut(&mut game.observed_military_power).insert(1, 2.0 * ours.max(10.0));
    Arc::make_mut(&mut game.observed_military_power).insert(2, 0.0);
    assert_eq!(ai.score_take_pick(&game, 0, false), None, "power");
    // A threatened city of ours.
    let (mut game, _) = board();
    swordsmen(&mut game);
    weak_rivals(&mut game);
    assert_eq!(ai.score_take_pick(&game, 0, true), None, "threatened");
    // A war on another major.
    let (mut game, _) = board();
    swordsmen(&mut game);
    weak_rivals(&mut game);
    game.at_war.insert((0, 2));
    assert_eq!(ai.score_take_pick(&game, 0, false), None, "another war");
    // Walls standing.
    let (mut game, forward) = board();
    swordsmen(&mut game);
    weak_rivals(&mut game);
    game.cities.get_mut(&forward).unwrap().wall_hp = 100;
    assert_eq!(ai.score_take_pick(&game, 0, false), None, "walls");
    // A centre nothing we field takes in time.
    let (mut game, forward) = board();
    swordsmen(&mut game);
    weak_rivals(&mut game);
    Arc::make_mut(&mut game.observed_city_strength).insert(forward, 80.0);
    assert_eq!(ai.score_take_pick(&game, 0, false), None, "too strong");
    // The tally comes first.
    let (mut game, _) = board();
    swordsmen(&mut game);
    weak_rivals(&mut game);
    game.max_turns = game.turn + 3;
    assert_eq!(ai.score_take_pick(&game, 0, false), None, "the tally");
}

#[test]
fn the_plan_turns_to_conquest_on_the_paying_city() {
    let (mut game, forward) = board();
    swordsmen(&mut game);
    weak_rivals(&mut game);
    let off = seat(false).assess(&game, 0);
    assert_ne!(
        off.strategy,
        GrandStrategy::Conquest,
        "off: the Score lane's plan, which never declares"
    );
    let on = seat(true).assess(&game, 0);
    assert_eq!(on.strategy, GrandStrategy::Conquest);
    assert_eq!(on.target_player, Some(1));
    assert_eq!(on.target_city, Some(forward));
}

#[test]
fn the_named_city_opens_the_domination_gates() {
    let (mut game, forward) = board();
    swordsmen(&mut game);
    weak_rivals(&mut game);
    let mut ai = seat(true);
    assert!(!ai.conquest_gates_open(&game), "nothing named yet");
    ai.plan = Some(conquest_plan(forward));
    ai.score_take_observe(&game, 0);
    assert_eq!(
        ai.score_take_state.named.map(|(r, c, _)| (r, c)),
        Some((1, forward))
    );
    assert!(ai.conquest_gates_open(&game));
    assert!(ai.score_take_lifts_the_cap());
    // Off, the gate is the Domination test alone.
    let mut off = seat(false);
    off.plan = Some(conquest_plan(forward));
    off.score_take_observe(&game, 0);
    assert!(!off.conquest_gates_open(&game));
    assert!(!off.score_take_lifts_the_cap());
}

#[test]
fn the_takers_at_the_city_open_the_war_and_every_hold_keeps_it() {
    let (mut game, forward) = board();
    swordsmen(&mut game);
    weak_rivals(&mut game);
    let plan = conquest_plan(forward);
    let mut off = seat(false);
    off.plan = Some(plan.clone());
    off.score_take_observe(&game, 0);
    assert!(!off.score_take_opening(&mut game, 0, &plan), "off: nothing");

    // A peace treaty's term holds it.
    let mut ai = seat(true);
    ai.plan = Some(plan.clone());
    ai.score_take_observe(&game, 0);
    ai.peace_until = game.turn + 5;
    assert!(!ai.score_take_opening(&mut game, 0, &plan), "peace term");
    ai.peace_until = 0;
    // No takers within the stage reach.
    let (mut far, forward) = board();
    for pos in [(4, 12), (4, 13), (5, 11)] {
        far.spawn_test_unit("swordsman", 0, pos);
    }
    weak_rivals(&mut far);
    let mut held = seat(true);
    held.plan = Some(conquest_plan(forward));
    held.score_take_observe(&far, 0);
    assert!(
        !held.score_take_opening(&mut far, 0, &conquest_plan(forward)),
        "not staged"
    );

    // Staged: the war opens (a denouncement first).
    let opened = ai.score_take_opening(&mut game, 0, &plan);
    assert!(opened, "the takers stand at the open city");
    let denounced = game.players[0]
        .denounced_until
        .get(&1)
        .is_some_and(|until| *until > game.turn);
    assert!(denounced || game.is_at_war(0, 1));
}

#[test]
fn a_named_city_lifts_the_peacetime_army_hold_while_the_corps_musters() {
    let (mut game, forward) = board();
    swordsmen(&mut game);
    weak_rivals(&mut game);
    let mut ai = seat(true);
    ai.enable_score_seat_plays_for_points();
    assert!(
        ai.score_seat_holds_the_army(false),
        "the hold stands at peace"
    );
    // `score-seat-trims-the-idle-army` (#5219): the second argument is now
    // "the trim acts", not "a major is at war with us".
    assert_eq!(ai.score_seat_army_target(4, true, 8), 4);
    ai.plan = Some(conquest_plan(forward));
    ai.score_take_observe(&game, 0);
    assert!(!game.is_at_war(0, 1), "still at peace: the muster stage");
    assert!(
        !ai.score_seat_holds_the_army(false),
        "named: lifted at peace"
    );
    assert_eq!(ai.score_seat_army_target(4, false, 8), 8);
    // Off, the hold is the shipped one.
    let mut off = seat(false);
    off.enable_score_seat_plays_for_points();
    off.plan = Some(conquest_plan(forward));
    off.score_take_observe(&game, 0);
    assert!(off.score_seat_holds_the_army(false));
}
