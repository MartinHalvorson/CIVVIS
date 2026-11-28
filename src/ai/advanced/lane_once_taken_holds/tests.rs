use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

#[test]
fn lane_once_taken_holds_is_opt_in() {
    opt_in_off_in_both_controllers("lane-once-taken-holds", |ai| ai.lane_once_taken_holds);
}

/// A Science seat with one city, short of its city target, on open land.
/// Once it has followed its lane, the Expansion branch's conditions holding
/// again (041325Z: science at t49, expansion at t54) send the stock seat back
/// to Expansion, and the gene keeps the lane it took. The city target the
/// Settler pipeline reads does not move.
#[test]
fn a_taken_lane_does_not_return_to_expansion() {
    let mut game = Game::new_full(2, 40, 28, 7_931, 250, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    let all: Vec<_> = game.map.tiles.keys().copied().collect();
    game.players[0].explored.extend(all);
    game.game_speed = crate::setup::GameSpeed::Online;
    game.current = 0;
    // Past the stock specialization clock (halfway, t125): every seat takes
    // its lane whatever its city count.
    game.turn = 130;

    let mut stock = AdvancedAi::targeting(VictoryTarget::Science);
    stock.city_target_floor = 8;
    let mut held = AdvancedAi::targeting(VictoryTarget::Science);
    held.city_target_floor = 8;
    held.enable_lane_once_taken_holds();
    // A gene seat that never saw its lane: the latch is not a lane switch.
    let mut fresh = AdvancedAi::targeting(VictoryTarget::Science);
    fresh.city_target_floor = 8;
    fresh.enable_lane_once_taken_holds();
    assert!(stock.phase_specialization_active(&game));

    // The lane is taken: both follow it.
    assert_eq!(stock.assess(&game, 0).strategy, GrandStrategy::Science);
    assert_eq!(held.assess(&game, 0).strategy, GrandStrategy::Science);
    assert_eq!(held.lane_taken.get(), Some(VictoryTarget::Science));
    assert_eq!(stock.lane_taken.get(), None, "off: no state is kept");

    // Back in the Expansion half with sites in reach and the target short:
    // the same pair of conditions the reviewed relapses met again after the
    // lane was taken (a risen target, a site back in reach).
    game.turn = 40;
    assert!(!stock.phase_specialization_active(&game));
    let stock_plan = stock.assess(&game, 0);
    assert_eq!(
        stock_plan.strategy,
        GrandStrategy::Expansion,
        "the reviewed relapse: the stock seat goes back to Expansion"
    );
    assert!(game.player_city_ids(0).len() < stock_plan.desired_cities);
    assert_eq!(
        fresh.assess(&game, 0).strategy,
        GrandStrategy::Expansion,
        "a lane never taken still expands first"
    );
    let journal = crate::reasoning::Journal::recording();
    held.attach_journal(journal.handle());
    let held_plan = held.assess(&game, 0);
    assert_eq!(
        held_plan.strategy,
        GrandStrategy::Science,
        "on: the taken lane holds"
    );
    assert_eq!(
        held_plan.desired_cities, stock_plan.desired_cities,
        "the city target the Settlers read is unchanged"
    );
    assert!(journal.since(0).thoughts.iter().any(|thought| thought
        .detail
        .contains("following the assigned victory lane")));
}

/// The latch names one lane: Domination keeps its own hand-over check, and a
/// different assigned lane starts over.
#[test]
fn the_latch_is_per_lane_and_skips_domination() {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_lane_once_taken_holds();
    assert!(!ai.lane_once_taken_holds_for(VictoryTarget::Science));
    ai.note_lane_taken(VictoryTarget::Science);
    assert!(ai.lane_once_taken_holds_for(VictoryTarget::Science));
    assert!(!ai.lane_once_taken_holds_for(VictoryTarget::Culture));
    ai.note_lane_taken(VictoryTarget::Domination);
    assert!(!ai.lane_once_taken_holds_for(VictoryTarget::Domination));
    assert!(!ai.lane_once_taken_holds_for(VictoryTarget::Science));

    let off = AdvancedAi::targeting(VictoryTarget::Science);
    off.note_lane_taken(VictoryTarget::Science);
    assert!(!off.lane_once_taken_holds_for(VictoryTarget::Science));
}
