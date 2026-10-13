use super::tests::{at, conquest, flat_board};
use super::*;
use crate::ai::VictoryTarget;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, Pos) {
    let objective = at(24, 10);
    let approach = at(12, 10);
    let mut game = flat_board(
        20_261_013_070,
        &[at(6, 10), objective, at(6, 18), at(24, 18)],
        false,
    );
    let target = game.city_at(objective).unwrap();
    for col in 10..14 {
        for row in 9..12 {
            game.spawn_test_unit("warrior", 0, at(col, row));
        }
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_objective_board();
    ai.enable_war_policy_via_board();
    // The unchanged registry has no preparation capability. Still execute
    // the fixture and assert the missing behavior, rather than skipping it.
    if let Some(policy) = crate::ai::gene("prewar-siege-board") {
        (policy.enable)(&mut ai);
    }
    let plan = conquest(&game, Some(target));
    (game, ai, plan, target, approach)
}

#[test]
fn a_named_peace_target_gets_a_staging_row_before_declaration() {
    let (game, mut ai, plan, target, _) = fixture();
    ai.rebuild_force_groups(&game, 0, &plan);
    let row = ai
        .objective_board()
        .rows
        .iter()
        .find(|row| row.key == ObjectiveKey::Siege(target))
        .expect("a declaration that asks for staging needs a preparation row first");
    assert_eq!(row.state, RowState::Staging);
    assert!(!game.is_at_war(0, 1), "allocation cannot declare a war");
}

#[test]
fn the_preparation_force_musters_on_a_legal_near_side_ring() {
    let (game, mut ai, plan, target, approach) = fixture();
    ai.rebuild_force_groups(&game, 0, &plan);
    let force = ai
        .objective_board()
        .forces
        .iter()
        .find(|force| force.objective_key == ObjectiveKey::Siege(target))
        .expect("available field soldiers must prepare instead of remaining reserve");
    let objective = game.cities[&target].pos;
    assert!(!force.units.is_empty());
    assert!((2..=3).contains(&game.wdist(force.rally, objective)));
    assert!(game.wdist(force.rally, approach) < game.wdist(objective, approach));
    assert!(game
        .map
        .get(force.rally)
        .unwrap()
        .owner_city
        .and_then(|city| game.cities.get(&city))
        .is_none_or(|city| city.owner == 0));
    let group = ai.force_groups.iter().find(|group| group.id == force.id).unwrap();
    assert_eq!(group.posture, ForcePosture::Muster);
    assert!(!game.is_at_war(0, 1));
}
