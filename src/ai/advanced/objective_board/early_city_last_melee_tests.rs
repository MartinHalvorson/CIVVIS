use super::tests::{at, conquest, flat_board, on};
use super::*;

fn fixture() -> (Game, AdvancedAi, u32, u32, u32, TaskForce) {
    let mut g = flat_board(917800, &[at(6, 8), at(30, 8)], false);
    let city = g.city_at(at(6, 8)).unwrap();
    let guard = g.spawn_test_unit("warrior", 0, at(6, 8));
    let archer = g.spawn_test_unit("archer", 0, at(7, 8));
    let force = TaskForce {
        id: 1,
        objective_key: ObjectiveKey::Defend(city),
        domain: ForceDomain::Land,
        units: vec![guard, archer],
        rally: at(6, 8),
        doctrine_state: ForcePosture::Hold,
        aimed_at: at(6, 8),
        formed: g.turn - 1,
    };
    let mut ai = on();
    ai.enable_capital_keeps_its_defender_3();
    ai.battlefront_observation = false;
    (g, ai, city, guard, archer, force)
}

fn facts(g: &Game) -> BTreeMap<u32, UnitFacts> {
    g.units
        .values()
        .map(|u| (u.id, UnitFacts::of(g, u.id)))
        .collect()
}

#[test]
fn early_city_last_melee_is_retained_only_under_version_three() {
    let (g, mut ai, _, guard, archer, force) = fixture();
    let facts = facts(&g);
    assert!(ai.early_city_keeps_last_melee(&g, 0, guard, &force, &facts));
    assert!(!ai.early_city_keeps_last_melee(&g, 0, archer, &force, &facts));
    ai.capital_keeps_its_defender_3 = false;
    assert!(!ai.early_city_keeps_last_melee(&g, 0, guard, &force, &facts));
    ai.enable_capital_keeps_its_defender_2();
    assert!(!ai.early_city_keeps_last_melee(&g, 0, guard, &force, &facts));
}

#[test]
fn early_city_last_melee_releases_surplus_but_retains_the_remaining_guard() {
    let (mut g, ai, _, guard, _, mut force) = fixture();
    let backup = g.spawn_test_unit("warrior", 0, at(6, 9));
    force.units.push(backup);
    let facts = facts(&g);
    assert!(!ai.early_city_keeps_last_melee(&g, 0, guard, &force, &facts));
    force.units.retain(|uid| *uid != guard);
    assert!(ai.early_city_keeps_last_melee(&g, 0, backup, &force, &facts));
}

#[test]
fn early_city_last_melee_needs_a_local_non_recon_backup_in_the_same_force() {
    let (mut g, ai, _, guard, _, mut force) = fixture();
    let distant = g.spawn_test_unit("warrior", 0, at(11, 8));
    let scout = g.spawn_test_unit("scout", 0, at(6, 9));
    let unassigned = g.spawn_test_unit("warrior", 0, at(5, 8));
    force.units.extend([distant, scout]);
    let facts = facts(&g);
    assert!(ai.early_city_keeps_last_melee(&g, 0, guard, &force, &facts));
    assert!(!ai.early_city_keeps_last_melee(&g, 0, distant, &force, &facts));
    assert!(!ai.early_city_keeps_last_melee(&g, 0, scout, &force, &facts));
    force.units.push(unassigned);
    assert!(!ai.early_city_keeps_last_melee(&g, 0, guard, &force, &facts));
}

#[test]
fn early_city_last_melee_does_not_reserve_for_a_later_city_or_non_defense() {
    let (mut g, ai, _, guard, _, mut force) = fixture();
    g.found_city_for(0, at(6, 2), None);
    g.found_city_for(0, at(18, 8), None);
    let fourth = g.found_city_for(0, at(24, 2), None);
    assert!(!early_cities(&g, 0).contains(&fourth));
    g.units.get_mut(&guard).unwrap().pos = at(24, 2);
    force.objective_key = ObjectiveKey::Defend(fourth);
    assert!(!ai.early_city_keeps_last_melee(&g, 0, guard, &force, &facts(&g)));
    force.objective_key = ObjectiveKey::Reserve;
    assert!(!ai.early_city_keeps_last_melee(&g, 0, guard, &force, &facts(&g)));
}

#[test]
fn early_city_last_melee_does_not_reserve_for_a_lost_city_or_sea_force() {
    let (mut g, ai, city, guard, _, mut force) = fixture();
    force.domain = ForceDomain::Sea;
    assert!(!ai.early_city_keeps_last_melee(&g, 0, guard, &force, &facts(&g)));
    force.domain = ForceDomain::Land;
    g.cities.get_mut(&city).unwrap().owner = 1;
    assert!(!ai.early_city_keeps_last_melee(&g, 0, guard, &force, &facts(&g)));
}

#[test]
fn early_city_last_melee_survives_a_higher_ranked_urgent_defense_allocation() {
    let (mut g, mut ai, capital, guard, archer, capital_force) = fixture();
    let second_pos = at(11, 8);
    let second = g.found_city_for(0, second_pos, None);
    g.cities.get_mut(&second).unwrap().hp = 10;
    let second_guard = g.spawn_test_unit("warrior", 0, second_pos);
    g.spawn_test_unit("swordsman", 1, at(12, 8));
    g.spawn_test_unit("swordsman", 1, at(11, 9));
    g.at_war.insert((0, 1));
    ai.objective_board_state.forces = vec![
        capital_force,
        TaskForce {
            id: 2,
            objective_key: ObjectiveKey::Defend(second),
            domain: ForceDomain::Land,
            units: vec![second_guard],
            rally: second_pos,
            doctrine_state: ForcePosture::Hold,
            aimed_at: second_pos,
            formed: g.turn - 1,
        },
    ];
    ai.objective_board_state.next_force_id = 3;
    ai.assess_board(&g, 0, &conquest(&g, None));
    let board = ai.objective_board();
    assert_eq!(board.rows[0].key, ObjectiveKey::Defend(second));
    assert!(board.rows[0].urgent);
    let capital_force = board
        .forces
        .iter()
        .find(|f| f.objective_key == ObjectiveKey::Defend(capital))
        .expect("the lower-ranked capital still has a force");
    assert!(capital_force.units.contains(&guard));
    assert!(
        !capital_force.units.contains(&archer),
        "surplus still reinforces the urgent city"
    );
    let second_force = board
        .forces
        .iter()
        .find(|f| f.objective_key == ObjectiveKey::Defend(second))
        .unwrap();
    assert!(second_force.units.contains(&archer));
    assert!(second_force.units.contains(&second_guard));
}
