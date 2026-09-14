use super::tests::{at, flat_board, on};
use crate::ai::{GrandStrategy, VictoryTarget};

#[test]
fn domination_can_prepare_an_understrength_siege_without_declaring_it() {
    let mut g = flat_board(3413, &[at(6, 8), at(20, 8)]);
    g.found_city_for(0, at(6, 15), None);
    g.turn = 300;
    let city = g.city_at(at(20, 8)).unwrap();
    g.cities.get_mut(&city).unwrap().wall_hp = 200;
    let soldier = g.spawn_test_unit("warrior", 0, at(7, 8));
    g.spawn_test_unit("scout", 0, at(17, 8));
    let mut ai = on();
    assert!(!ai.war_policy_siege_feasible(&g, 0, city));
    assert!(!ai.war_policy_target_feasible(&g, 0, 1));
    ai.retarget(VictoryTarget::Domination);
    let plan = ai.assess(&g, 0);
    assert_eq!(plan.strategy, GrandStrategy::Conquest);
    assert_eq!(
        plan.target_player,
        Some(1),
        "preparation needs a target before the army exists"
    );
    assert_eq!(plan.target_city, Some(city));
    let home = g.city_at(at(6, 8)).unwrap();
    let warrior = crate::game::Item::Unit {
        unit: crate::name!("warrior"),
    };
    let counts = ai.counts(&g, 0);
    let mut untargeted = plan.clone();
    untargeted.target_city = None;
    untargeted.target_player = None;
    assert_eq!(
        ai.production_value(&g, 0, home, &warrior, &untargeted, &counts),
        -2_000.0
    );
    assert!(
        ai.production_value(&g, 0, home, &warrior, &plan, &counts) > 0.0,
        "naming the objective releases the conquest army's production budget"
    );
    let before = g.units[&soldier].pos;
    assert_eq!(
        ai.campaign_staging_step(&mut g, 0, soldier, &plan),
        Some(true)
    );
    assert_ne!(g.units[&soldier].pos, before);
    assert!(
        matches!(ai.war_policy_declaration(&g, 0, 1, &plan), Some(Err(_))),
        "a preparation objective must not authorize an unready war"
    );
    assert!(!g.is_at_war(0, 1));
    g.victory_conditions.domination = false;
    assert!(
        !ai.war_policy_target_feasible(&g, 0, 1),
        "a disabled domination lane does not request its preparation exception"
    );
}
