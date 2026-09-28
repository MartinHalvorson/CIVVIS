use super::*;
use crate::{game::Action, name};

fn at(col: i32, row: i32) -> Pos {
    crate::hex::offset_to_axial(col, row)
}

fn board() -> (Game, u32, u32) {
    let mut g = Game::new_full(3, 40, 24, 109_106_000, 300, 0, false);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
    }
    let home = g.found_city_for(0, at(4, 8), None);
    let target = g.found_city_for(1, at(24, 8), None);
    let third = g.found_city_for(2, at(4, 18), None);
    g.cities.get_mut(&third).unwrap().owner = 0;
    let pressure = g.found_city_for(1, at(28, 8), None);
    g.cities.get_mut(&pressure).unwrap().pop = 40;
    g.cities.get_mut(&target).unwrap().pop = 1;
    g.cities.get_mut(&target).unwrap().hp = 0;
    g.cities.get_mut(&target).unwrap().wall_hp = 0;
    g.current = 0;
    g.turn = 90;
    g.at_war.insert((0, 1));
    g.record_contact(0, 1);
    g.record_contact(0, 2);
    assert!(AdvancedAi::population_loyalty_delta_with_capture(&g, 0, target, true) <= -12.5);
    (g, home, target)
}

/// Exercise the actual capture/keep path, so a mirrored AI predicate cannot
/// claim a win merely because its own assertions agree with one another.
fn take(mut g: Game, target: u32) -> Game {
    let pos = g.cities[&target].pos;
    let unit = g.spawn_test_unit("giant_death_robot", 0, at(23, 8));
    g.apply(0, &Action::Attack { unit, target: pos }).unwrap();
    assert_eq!(g.cities[&target].owner, 0);
    g.apply(0, &Action::KeepCity { city: target }).unwrap();
    g
}

#[test]
fn a_lost_home_capital_preserves_occupation_safety_on_the_last_foreign_capital() {
    let (mut g, home, target) = board();
    assert!(AdvancedAi::capture_completes_domination(&g, 0, target));
    assert!(!AdvancedAi::should_defer_city_capture(&g, 0, target));
    assert_eq!(take(g.clone(), target).winner, Some(0));

    g.cities.get_mut(&home).unwrap().owner = 2;
    assert!(!AdvancedAi::capture_completes_domination(&g, 0, target));
    assert!(AdvancedAi::should_defer_city_capture(&g, 0, target));
    assert_eq!(take(g, target).winner, None);
}

#[test]
fn disabled_domination_cannot_waive_occupation_safety() {
    let (mut g, _, target) = board();
    g.victory_conditions.domination = false;
    assert!(!AdvancedAi::capture_completes_domination(&g, 0, target));
    assert!(AdvancedAi::should_defer_city_capture(&g, 0, target));
    assert_eq!(take(g, target).winner, None);
}

#[test]
fn a_domination_milestone_only_waives_safety_when_it_completes_require_n() {
    let (mut g, _, target) = board();
    g.required_victory_types = 2;
    assert!(!AdvancedAi::capture_completes_domination(&g, 0, target));
    assert!(AdvancedAi::should_defer_city_capture(&g, 0, target));
    let banked = take(g.clone(), target);
    assert_eq!(banked.winner, None);
    assert!(banked.victories_won[&0].contains("domination"));

    g.victories_won
        .entry(0)
        .or_default()
        .insert("science".into());
    assert!(AdvancedAi::capture_completes_domination(&g, 0, target));
    assert!(!AdvancedAi::should_defer_city_capture(&g, 0, target));
    assert_eq!(take(g, target).winner, Some(0));
}

#[test]
fn an_unfounded_living_major_is_still_a_remaining_domination_opponent() {
    let (mut g, _, target) = board();
    let third = g
        .cities
        .values()
        .find(|city| city.original_owner == 2)
        .unwrap()
        .id;
    g.cities.remove(&third);
    g.players[2].alive = true;
    assert!(!AdvancedAi::capture_completes_domination(&g, 0, target));
    assert!(AdvancedAi::should_defer_city_capture(&g, 0, target));
    g.players[2].alive = false;
    assert!(AdvancedAi::capture_completes_domination(&g, 0, target));
}

#[test]
fn team_completion_keeps_its_own_capitals_and_only_needs_opponents_to_lose_theirs() {
    let (mut g, _, target) = board();
    g.players[0].team = Some(1);
    g.players[2].team = Some(1);
    let third = g
        .cities
        .values()
        .find(|city| city.original_owner == 2)
        .unwrap()
        .id;
    g.cities.get_mut(&third).unwrap().owner = 2;
    assert!(AdvancedAi::capture_completes_domination(&g, 0, target));
    assert_eq!(take(g.clone(), target).winner, Some(0));
    g.cities.get_mut(&third).unwrap().owner = 1;
    assert!(!AdvancedAi::capture_completes_domination(&g, 0, target));
    assert!(AdvancedAi::should_defer_city_capture(&g, 0, target));
    assert_eq!(take(g, target).winner, None);
}

#[test]
fn team_require_n_uses_the_engines_first_qualifying_member() {
    let (mut g, _, target) = board();
    g.players[0].team = Some(1);
    g.players[2].team = Some(1);
    let third = g
        .cities
        .values()
        .find(|city| city.original_owner == 2)
        .unwrap()
        .id;
    g.cities.get_mut(&third).unwrap().owner = 2;
    g.required_victory_types = 2;
    g.victories_won
        .entry(2)
        .or_default()
        .insert("science".into());
    assert!(!AdvancedAi::capture_completes_domination(&g, 0, target));
    assert_eq!(take(g.clone(), target).winner, None);
    g.victories_won
        .entry(0)
        .or_default()
        .insert("science".into());
    assert!(AdvancedAi::capture_completes_domination(&g, 0, target));
    assert!(AdvancedAi::capture_completes_domination(&g, 2, target));
    let result = take(g, target);
    assert_eq!(result.winner, Some(0));
    assert!(result.winning_players().contains(&2));
}

#[test]
fn a_diplomatic_match_clock_routes_the_army_to_its_last_required_capital() {
    let (mut g, home, capital) = board();
    // The old front has already yielded its original capital, but still has
    // a town and an active war. This is the live failure shape: the ordinary
    // denial response switches to Diplomacy while the army mops up that town.
    let old_front_town = g.found_city_for(2, at(14, 18), None);
    g.cities.get_mut(&old_front_town).unwrap().is_capital = false;
    g.at_war.remove(&(0, 1));
    g.at_war.insert((0, 2));
    g.players[1].dvp = 16;

    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_deny_while_targeted();
    ai.enable_stock_denial_lead_time();
    ai.enable_denial_outranks_expansion();
    assert_eq!(
        ai.actionable_victory_denial(&g, 0),
        Some((1, GrandStrategy::Diplomacy)),
        "the public 16-point clock must be the actual competing plan"
    );
    assert_eq!(ai.domination_finishing_capital_for(&g, 0, 1), Some(capital));
    let plan = ai.assess(&g, 0);
    assert_eq!(plan.strategy, GrandStrategy::Conquest);
    assert_eq!(plan.target_player, Some(1));
    assert_eq!(g.cities[&plan.target_city.unwrap()].owner, 1);

    let mut forced = ai.clone();
    forced.forced_target_player = Some(2);
    assert_eq!(forced.assess(&g, 0).target_player, Some(2));

    let mut one_war = ai.clone();
    one_war.enable_one_war_at_a_time();
    one_war.one_war_observe(&g, 0);
    assert_eq!(one_war.assess(&g, 0).target_player, Some(2));

    let mut threatened = g.clone();
    threatened.spawn_test_unit("giant_death_robot", 2, at(5, 8));
    let defensive = ai.assess(&threatened, 0);
    assert_eq!(defensive.strategy, GrandStrategy::Recovery);
    assert_eq!(defensive.target_player, Some(2));

    g.cities.get_mut(&home).unwrap().owner = 2;
    assert_eq!(ai.domination_finishing_capital_for(&g, 0, 1), None);
    assert_eq!(ai.assess(&g, 0).strategy, GrandStrategy::Diplomacy);
}
