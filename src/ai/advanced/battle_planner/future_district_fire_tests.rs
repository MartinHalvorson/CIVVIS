use super::*;
use crate::game::defending_district_tests::fixture;

fn position(oppidum: bool) -> (Game, u32, Pos, Pos, u32) {
    let (mut g, city, enc, opp) = fixture();
    let source = if oppidum { opp } else { enc };
    let target = (source.0 + if oppidum { 1 } else { -1 }, source.1);
    let victim = g.spawn_test_unit("warrior", 1, target);
    assert!(g.wdist(g.cities[&city].pos, target) > 2);
    assert!(g.wdist(if oppidum { enc } else { opp }, target) > 2);
    g.current = 1;
    (g, city, source, target, victim)
}

fn forecasts(g: &Game, target: Pos, victim: u32) -> [(usize, f64); 3] {
    [
        DangerField::new(g, 1),
        DangerField::with_reach(g, 1, true),
        DangerField::second_turn(g, 1),
    ]
    .map(|mut field| {
        let blows = field.contributions(target, victim);
        assert!(blows.iter().all(|(source, _)| source.is_none()));
        (blows.len(), blows.iter().map(|(_, damage)| damage).sum())
    })
}

fn spend_shot(g: &mut Game, city: u32, source: Pos) {
    let mut state = g.defending_district_state(city, source).unwrap();
    state.struck = true;
    state.extra_strikes_used = 10;
    g.set_defending_district_state(city, state);
}

fn shot(g: &Game, city: u32, source: Pos, target: Pos) -> Action {
    let state = g.defending_district_state(city, source).unwrap();
    g.defending_district_strike_action(city, &state, target)
}

fn assert_actual_next_turn_shot(g: &Game, city: u32, source: Pos, target: Pos, victim: u32) {
    let action = shot(g, city, source, target);
    let mut same_turn = g.clone();
    same_turn.current = 0;
    assert!(!same_turn.legal_actions(0).contains(&action));
    assert!(same_turn.apply(0, &action).is_err());

    let mut reply = g.clone();
    for _ in 0..reply.players.len() {
        if reply.current == 0 {
            break;
        }
        reply.apply(reply.current, &Action::EndTurn).unwrap();
    }
    assert_eq!(
        reply.current, 0,
        "the enemy's next turn must actually begin"
    );
    let reset = reply.defending_district_state(city, source).unwrap();
    assert!(!reset.struck);
    assert_eq!(reset.extra_strikes_used, 0);
    assert!(reply.legal_actions(0).contains(&action));
    let before = reply.units[&victim].hp;
    reply.apply(0, &action).unwrap();
    assert!(reply.units.get(&victim).is_none_or(|unit| unit.hp < before));
}

fn assert_spent_fort_is_future_danger(oppidum: bool) {
    let (mut g, city, source, target, victim) = position(oppidum);
    let ready = forecasts(&g, target, victim);
    assert!(ready
        .iter()
        .all(|(count, damage)| *count == 1 && *damage > 0.0));
    spend_shot(&mut g, city, source);
    let spent = g.defending_district_state(city, source).unwrap();
    assert_actual_next_turn_shot(&g, city, source, target, victim);
    let future = forecasts(&g, target, victim);
    assert_eq!(g.current, 1);
    assert_eq!(g.units[&victim].hp, 100);
    assert_eq!(g.defending_district_state(city, source), Some(spent));
    assert_eq!(
        future, ready,
        "previous-turn fire must not erase future fire"
    );
}

#[test]
fn spent_encampment_still_threatens_the_next_turn() {
    assert_spent_fort_is_future_danger(false);
}

#[test]
fn spent_oppidum_still_threatens_the_next_turn() {
    assert_spent_fort_is_future_danger(true);
}

#[test]
fn encampment_budget_blocks_an_immediate_shot_and_resets_at_its_turn() {
    let (mut g, city, source, target, victim) = position(false);
    spend_shot(&mut g, city, source);
    assert_actual_next_turn_shot(&g, city, source, target, victim);
}

#[test]
fn oppidum_budget_blocks_an_immediate_shot_and_resets_at_its_turn() {
    let (mut g, city, source, target, victim) = position(true);
    spend_shot(&mut g, city, source);
    assert_actual_next_turn_shot(&g, city, source, target, victim);
}

#[test]
fn ready_encampment_is_one_future_fire_source() {
    let (g, _, _, target, victim) = position(false);
    assert!(forecasts(&g, target, victim)
        .iter()
        .all(|(count, damage)| *count == 1 && *damage > 0.0));
}

#[test]
fn ready_oppidum_is_one_future_fire_source() {
    let (g, _, _, target, victim) = position(true);
    assert!(forecasts(&g, target, victim)
        .iter()
        .all(|(count, damage)| *count == 1 && *damage > 0.0));
}

fn assert_inactive_fort_has_no_future_fire(oppidum: bool) {
    let (g, city, source, target, victim) = position(oppidum);
    for condition in 0..4 {
        let mut inactive = g.clone();
        let mut state = inactive.defending_district_state(city, source).unwrap();
        match condition {
            0 => state.hp = 0,
            1 => state.wall_hp = 0,
            2 => state.pillaged = true,
            _ => inactive.map.tiles.get_mut(&source).unwrap().pillaged = true,
        }
        inactive.set_defending_district_state(city, state);
        assert_eq!(forecasts(&inactive, target, victim), [(0, 0.0); 3]);
    }
}

#[test]
fn depleted_or_pillaged_encampment_has_no_future_fire() {
    assert_inactive_fort_has_no_future_fire(false);
}

#[test]
fn depleted_or_pillaged_oppidum_has_no_future_fire() {
    assert_inactive_fort_has_no_future_fire(true);
}

#[test]
fn fort_outside_two_tiles_has_no_future_fire() {
    for oppidum in [false, true] {
        let (mut g, _, source, _, victim) = position(oppidum);
        let target = (source.0 + if oppidum { 3 } else { -3 }, source.1);
        g.relocate(victim, target);
        assert!(g.wdist(source, target) > 2);
        assert_eq!(forecasts(&g, target, victim), [(0, 0.0); 3]);
    }
}

#[test]
fn peaceful_forts_have_no_future_fire() {
    for oppidum in [false, true] {
        let (mut g, _, _, target, victim) = position(oppidum);
        g.at_war.clear();
        assert_eq!(forecasts(&g, target, victim), [(0, 0.0); 3]);
    }
}

fn assert_actual_previous_shot_is_future_danger(oppidum: bool) {
    let (mut g, city, source, target, victim) = position(oppidum);
    assert_eq!(g.governor_effect(0, city, "city_extra_strike"), 0.0);
    g.current = 0;
    let action = shot(&g, city, source, target);
    g.apply(0, &action).unwrap();
    let health = g.units[&victim].hp;
    assert!(health > 0 && health < 100, "the preceding shot must land");
    let spent = g.defending_district_state(city, source).unwrap();
    assert!(spent.struck);
    assert_eq!(spent.extra_strikes_used, 0);
    g.current = 1;

    let mut ready = g.clone();
    let mut reset = spent;
    reset.struck = false;
    ready.set_defending_district_state(city, reset);
    let expected = forecasts(&ready, target, victim);
    assert!(expected
        .iter()
        .all(|(count, damage)| *count == 1 && *damage > 0.0));
    assert_actual_next_turn_shot(&g, city, source, target, victim);
    let actual = forecasts(&g, target, victim);
    assert_eq!(g.units[&victim].hp, health);
    assert_eq!(g.defending_district_state(city, source), Some(spent));
    assert_eq!(
        actual, expected,
        "an actual previous shot cannot erase future fire"
    );
}

#[test]
fn an_actual_encampment_shot_does_not_erase_next_turn_danger() {
    assert_actual_previous_shot_is_future_danger(false);
}

#[test]
fn an_actual_oppidum_shot_does_not_erase_next_turn_danger() {
    assert_actual_previous_shot_is_future_danger(true);
}
