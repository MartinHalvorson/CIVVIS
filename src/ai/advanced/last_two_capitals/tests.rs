use super::super::*;
use crate::name;

fn at(col: i32, row: i32) -> Pos {
    crate::hex::offset_to_axial(col, row)
}

/// Four majors on open grassland. We hold seat 3's capital; seat 1 (at war)
/// holds its capital and a nearer town; seat 2's capital stands far off.
/// Domination needs seat 1's and seat 2's capitals.
fn two_left() -> (Game, u32, u32, u32) {
    let mut g = Game::new_full(4, 48, 28, 109_106_154, 300, 0, false);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
    }
    g.found_city_for(0, at(4, 8), None);
    let third = g.found_city_for(3, at(4, 18), None);
    g.cities.get_mut(&third).unwrap().owner = 0;
    let capital = g.found_city_for(1, at(17, 8), None);
    let town = g.found_city_for(1, at(10, 8), None);
    let far = g.found_city_for(2, at(44, 24), None);
    for cid in [capital, town, far] {
        g.cities.get_mut(&cid).unwrap().wall_hp = 100;
    }
    for seat in 1..4 {
        g.record_contact(0, seat);
    }
    g.at_war.insert((0, 1));
    g.current = 0;
    g.turn = 200;
    for y in 0..8 {
        g.spawn_test_unit("modern_armor", 0, at(5, 6 + y % 6));
    }
    g.spawn_test_unit("warrior", 1, at(17, 9));
    (g, capital, town, third)
}

fn committed(ai: &mut AdvancedAi, g: &Game, town: u32) {
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(town),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    });
}

#[test]
fn with_two_capitals_left_the_targets_capital_outranks_a_committed_town() {
    let (g, capital, town, _) = two_left();
    assert_eq!(
        AdvancedAi::domination_capitals_needed(&g, 0).map(|needed| needed.len()),
        Some(2)
    );
    assert!(!AdvancedAi::capture_completes_domination(&g, 0, capital));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_siege_commitment();
    committed(&mut ai, &g, town);
    assert_eq!(ai.assess(&g, 0).target_city, Some(town), "off");
    ai.enable_the_last_two_capitals_come_first();
    assert_eq!(
        ai.last_two_capitals_objective(&g, 0, Some(1)),
        Some(capital)
    );
    committed(&mut ai, &g, town);
    assert_eq!(ai.assess(&g, 0).target_city, Some(capital));
}

#[test]
fn three_left_peace_or_a_far_capital_name_nothing() {
    let (mut g, capital, _, third) = two_left();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_the_last_two_capitals_come_first();
    assert_eq!(
        ai.last_two_capitals_objective(&g, 0, Some(2)),
        None,
        "too far"
    );
    g.at_war.remove(&(0, 1));
    assert_eq!(
        ai.last_two_capitals_objective(&g, 0, Some(1)),
        None,
        "peace"
    );
    g.at_war.insert((0, 1));
    g.cities.get_mut(&third).unwrap().owner = 3;
    assert_eq!(
        AdvancedAi::domination_capitals_needed(&g, 0).map(|needed| needed.len()),
        Some(3)
    );
    assert_eq!(
        ai.last_two_capitals_objective(&g, 0, Some(1)),
        None,
        "three left"
    );
    g.cities.get_mut(&third).unwrap().owner = 0;
    assert_eq!(
        ai.last_two_capitals_objective(&g, 0, Some(1)),
        Some(capital)
    );
}

/// A living major whose capital we have never seen keeps the count unknown:
/// a four-major game holding one foreign capital is not "two left" because the
/// mirror lacks the third.
#[test]
fn an_unseen_capital_keeps_the_count_unknown() {
    let (mut g, capital, _, _) = two_left();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_the_last_two_capitals_come_first();
    assert_eq!(
        ai.last_two_capitals_objective(&g, 0, Some(1)),
        Some(capital)
    );
    let far = g
        .cities
        .values()
        .find(|city| city.is_capital && city.original_owner == 2)
        .map(|city| city.id)
        .expect("seat 2's capital");
    g.cities.remove(&far);
    assert!(g.players[2].alive);
    assert_eq!(AdvancedAi::domination_capitals_needed(&g, 0), None);
    assert_eq!(ai.last_two_capitals_objective(&g, 0, Some(1)), None);
}
