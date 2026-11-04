use super::*;
use crate::setup::GameSpeed;

fn at(col: i32, row: i32) -> Pos {
    crate::hex::offset_to_axial(col, row)
}

fn opening() -> (Game, u32, u32, StrategicPlan) {
    let mut g = Game::new_full(2, 36, 22, 440500, 1_000, 0, true);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    let capital = g.found_city_for(0, at(14, 8), None);
    g.found_city_for(1, at(30, 8), None);
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    g.at_war.clear();
    g.at_war.insert((0, 1));
    g.game_speed = GameSpeed::Standard;
    g.turn = 12;
    g.current = 0;
    g.spawn_test_unit("swordsman", 1, at(11, 8));
    let soldier = g.spawn_test_unit("warrior", 0, at(7, 8));
    g.barb_camps.insert(at(6, 10), g.turn);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: Some(capital),
        desired_cities: 5,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, capital, soldier, plan)
}

fn ai(enabled: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_objective_board();
    ai.battlefront_observation = false;
    if enabled {
        ai.enable_first_capital_before_the_camp();
    }
    ai
}

#[test]
fn first_capital_defense_default_off_and_withhold_restore_priority() {
    let (g, capital, _, plan) = opening();
    let mut ai = ai(false);
    assert!(!ai.first_capital_defense_is_urgent(&g, 0, capital, &plan));
    ai.enable_first_capital_before_the_camp();
    assert!(ai.first_capital_defense_is_urgent(&g, 0, capital, &plan));
    ai.disable_first_capital_before_the_camp();
    assert!(!ai.first_capital_defense_is_urgent(&g, 0, capital, &plan));
}

#[test]
fn first_capital_defense_releases_after_expansion_or_warning_clear() {
    let (mut g, capital, _, mut plan) = opening();
    let ai = ai(true);
    plan.threatened_city = None;
    assert!(!ai.first_capital_defense_is_urgent(&g, 0, capital, &plan));
    plan.threatened_city = Some(capital);
    g.found_city_for(0, at(14, 16), None);
    assert!(!ai.first_capital_defense_is_urgent(&g, 0, capital, &plan));
}

#[test]
fn first_capital_defense_ignores_noncapital_and_other_owners() {
    let (mut g, capital, _, plan) = opening();
    let ai = ai(true);
    assert!(!ai.first_capital_defense_is_urgent(&g, 1, capital, &plan));
    g.cities.get_mut(&capital).unwrap().is_capital = false;
    assert!(!ai.first_capital_defense_is_urgent(&g, 0, capital, &plan));
}

#[test]
fn first_capital_defense_window_scales_with_the_economic_clock() {
    let (mut g, capital, _, plan) = opening();
    let ai = ai(true);
    for speed in [
        GameSpeed::Online,
        GameSpeed::Quick,
        GameSpeed::Standard,
        GameSpeed::Epic,
    ] {
        g.game_speed = speed;
        g.turn = g.standard_duration(OPENING_DEFENSE_STANDARD_TURNS);
        assert!(ai.first_capital_defense_is_urgent(&g, 0, capital, &plan));
        g.turn += 1;
        assert!(!ai.first_capital_defense_is_urgent(&g, 0, capital, &plan));
    }
}

#[test]
fn first_capital_defense_recalls_the_camp_party_before_the_capital_is_hit() {
    use super::super::objective_board::{ObjectiveKey, ObjectiveKind};
    let (g, capital, soldier, plan) = opening();
    let mut off = ai(false);
    off.rebuild_force_groups(&g, 0, &plan);
    let board = off.objective_board();
    let defend = board
        .rows
        .iter()
        .find(|r| r.key == ObjectiveKey::Defend(capital))
        .unwrap();
    assert!(!defend.urgent, "exercise the nonurgent opening gap");
    assert_eq!(board.rows[0].kind, ObjectiveKind::ClearCamp);
    assert!(board
        .forces
        .iter()
        .any(|f| f.objective_key == ObjectiveKey::Camp(at(6, 10)) && f.units.contains(&soldier)));

    let mut on = ai(true);
    on.rebuild_force_groups(&g, 0, &plan);
    let board = on.objective_board();
    assert_eq!(board.rows[0].key, ObjectiveKey::Defend(capital));
    assert!(board.rows[0].urgent);
    assert!(board
        .forces
        .iter()
        .any(|f| f.objective_key == ObjectiveKey::Defend(capital) && f.units.contains(&soldier)));
    assert_eq!(
        g.cities[&capital].hp, 200,
        "defence need not wait for damage"
    );
}
