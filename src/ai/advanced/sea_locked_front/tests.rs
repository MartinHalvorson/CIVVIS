use super::super::siege_road::ROAD_HOLD_TURNS;
use super::super::siege_train::StageMarch;
use super::super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
use super::SEA_LOCKED_SWAP_RATIO;
use crate::game::Game;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Two islands: us at the west end of the west island with player 3 at its
/// east end, both at war with us; player 2 alone on the east island, also at
/// war with us. No land road crosses the strait.
fn islands() -> (Game, u32, u32) {
    let mut g = Game::new_full(4, 40, 12, 931_037, 300, 0, false);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    for tile in g.map.tiles.values_mut() {
        let (x, y) = tile.pos;
        let land = (4..=8).contains(&y) && ((2..=16).contains(&x) || (24..=36).contains(&x));
        tile.terrain = if land {
            crate::name!("grassland")
        } else {
            crate::name!("ocean")
        };
        tile.feature = None;
        tile.hills = false;
    }
    g.found_city_for(0, (4, 6), None);
    g.found_city_for(3, (14, 6), None);
    let target = g.found_city_for(2, (32, 6), None);
    for rival in [2, 3] {
        g.at_war.insert((0, rival));
        g.at_war.insert((rival, 0));
    }
    let soldier = g.spawn_test_unit("warrior", 0, (8, 6));
    let powers: BTreeMap<usize, f64> = [(0, 900.0), (2, 300.0), (3, 300.0)].into();
    g.observed_military_power = Arc::new(powers);
    (g, target, soldier)
}

fn ai(gene: bool, g: &Game, target: u32) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_capture_go_or_stand_down_2();
    ai.enable_siege_target_needs_a_road();
    ai.enable_one_war_at_a_time();
    if gene {
        ai.enable_sea_locked_front_moves_on();
    }
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(2),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    });
    ai
}

/// The front reads player 2 (the plan's target), then `ROAD_HOLD_TURNS`
/// turns of the train held short of the water stand its siege down.
fn stand_down(ai: &mut AdvancedAi, g: &mut Game, target: u32, soldier: u32) {
    ai.one_war_observe(g, 0);
    assert_eq!(ai.one_war_front(), Some(2), "fixture: the plan's target is the front");
    let hold = StageMarch::Hold { wet: 18 };
    for _ in 0..ROAD_HOLD_TURNS {
        let _ = ai.note_stage_march(g, soldier, target, &hold);
        ai.reconcile_commitments(g, 0);
        g.turn += 1;
    }
    assert!(ai.capture_stood_down_holds(g, target), "fixture: stood down");
    assert!(ai.siege_road_cuts_off(g, 0, 2), "fixture: player 2 is cut off");
    assert!(ai.rival_reachable_by_land(g, 0, 3), "fixture: player 3 is a walk away");
}

/// Under the gene the cut-off front moves to the enemy on our island; with
/// the gene off it sticks.
#[test]
fn a_cut_off_front_moves_to_the_enemy_the_army_can_walk_to() {
    let (game, target, soldier) = islands();

    let mut on = ai(true, &game, target);
    let mut g = game.clone();
    stand_down(&mut on, &mut g, target, soldier);
    on.one_war_observe(&g, 0);
    assert_eq!(on.one_war_front(), Some(3), "gene on: the front moves");
    // Once moved, the front sticks while its war lasts.
    g.turn += 1;
    on.one_war_observe(&g, 0);
    assert_eq!(on.one_war_front(), Some(3));

    let mut off = ai(false, &game, target);
    let mut g = game.clone();
    stand_down(&mut off, &mut g, target, soldier);
    off.one_war_observe(&g, 0);
    assert_eq!(off.one_war_front(), Some(2), "gene off: the front sticks");
}

/// No stand-down, no move: the front is not cut off until its siege was
/// stood down for want of a road.
#[test]
fn a_front_whose_siege_was_never_stood_down_stays() {
    let (game, target, _) = islands();
    let mut on = ai(true, &game, target);
    let g = game.clone();
    on.one_war_observe(&g, 0);
    assert_eq!(on.one_war_front(), Some(2));
    assert_eq!(on.sea_locked_front_swap(&g, 0, 2, &[2, 3]), None);
}

/// An enemy we do not outgun `SEA_LOCKED_SWAP_RATIO` times is no place to
/// move the front to.
#[test]
fn a_stronger_land_enemy_does_not_take_the_front() {
    let (mut game, target, soldier) = islands();
    let strong = 900.0 / SEA_LOCKED_SWAP_RATIO + 1.0;
    let powers: BTreeMap<usize, f64> = [(0, 900.0), (2, 300.0), (3, strong)].into();
    game.observed_military_power = Arc::new(powers);
    let mut on = ai(true, &game, target);
    let mut g = game.clone();
    stand_down(&mut on, &mut g, target, soldier);
    assert_eq!(on.sea_locked_front_swap(&g, 0, 2, &[2, 3]), None);
    on.one_war_observe(&g, 0);
    assert_eq!(on.one_war_front(), Some(2));
}

/// An enemy across the water as well is no better front, and a front that a
/// war on a third party has opened a land road to is no longer cut off.
#[test]
fn only_a_land_reachable_enemy_takes_the_front() {
    let (game, target, soldier) = islands();
    let mut on = ai(true, &game, target);
    let mut g = game.clone();
    stand_down(&mut on, &mut g, target, soldier);
    // Player 3 moved to the east island beside player 2: no enemy is a walk away.
    let mut far = g.clone();
    let own = far.player_city_ids(3);
    for cid in own {
        far.cities.get_mut(&cid).unwrap().pos = (28, 6);
    }
    assert!(!on.rival_reachable_by_land(&far, 0, 3), "fixture: 3 is across the water");
    assert_eq!(on.sea_locked_front_swap(&far, 0, 2, &[2, 3]), None);
    // A land bridge to player 2: the front is no longer cut off.
    let mut bridged = g.clone();
    for tile in bridged.map.tiles.values_mut() {
        if tile.pos.1 == 6 && (17..=23).contains(&tile.pos.0) {
            tile.terrain = crate::name!("grassland");
        }
    }
    assert!(!on.siege_road_cuts_off(&bridged, 0, 2));
    assert_eq!(on.sea_locked_front_swap(&bridged, 0, 2, &[2, 3]), None);
}

/// Outside the Domination target the gene does nothing.
#[test]
fn another_victory_target_keeps_the_front() {
    let (game, target, soldier) = islands();
    let mut on = ai(true, &game, target);
    let mut g = game.clone();
    stand_down(&mut on, &mut g, target, soldier);
    on.victory_target = Some(VictoryTarget::Science);
    assert_eq!(on.sea_locked_front_swap(&g, 0, 2, &[2, 3]), None);
}

/// `islands` with a foothold of ours on the east island and a second city of
/// player 2 there: a land road from some city of ours reaches player 2, so
/// `siege_road_cuts_off` never reads it cut off, while the army stands on
/// the west island.
fn foothold() -> (Game, u32, u32, u32) {
    let (mut g, target, soldier) = islands();
    g.found_city_for(0, (25, 6), None);
    let second = g.found_city_for(2, (29, 6), None);
    (g, target, second, soldier)
}

fn hold_turns(ai: &mut AdvancedAi, g: &mut Game, city: u32, soldier: u32) {
    if let Some(plan) = ai.plan.as_mut() {
        plan.target_city = Some(city);
    }
    let hold = StageMarch::Hold { wet: 18 };
    for _ in 0..ROAD_HOLD_TURNS {
        let _ = ai.note_stage_march(g, soldier, city, &hold);
        ai.reconcile_commitments(g, 0);
        g.turn += 1;
    }
    assert!(ai.capture_stood_down_holds(g, city), "fixture: {city} stood down");
}

/// With a foothold keeping the rival "reachable", one water stand-down is not
/// enough (an island city of a mainland rival), two are.
#[test]
fn two_water_stand_downs_read_the_front_sea_locked_despite_a_foothold() {
    let (game, target, second, soldier) = foothold();
    let mut on = ai(true, &game, target);
    let mut g = game.clone();
    on.one_war_observe(&g, 0);
    assert_eq!(on.one_war_front(), Some(2));
    hold_turns(&mut on, &mut g, target, soldier);
    assert!(on.rival_reachable_by_land(&g, 0, 2), "fixture: the foothold reaches player 2");
    assert!(!on.siege_road_cuts_off(&g, 0, 2));
    assert_eq!(on.water_road_stand_downs(&g, 0, 2), 1);
    on.one_war_observe(&g, 0);
    assert_eq!(on.one_war_front(), Some(2), "one stand-down: the front stays");
    hold_turns(&mut on, &mut g, second, soldier);
    assert_eq!(on.water_road_stand_downs(&g, 0, 2), 2);
    on.one_war_observe(&g, 0);
    assert_eq!(on.one_war_front(), Some(3), "two stand-downs: the front moves");
}
