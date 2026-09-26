use super::tests::{at, conquest, flat_board, on, war};
use super::*;

#[test]
fn a_small_hit_does_not_postpone_relief_from_an_adjacent_minor_army() {
    let mut g = flat_board(379100, &[at(6, 8), at(30, 8)], false);
    war(&mut g, 0, 1);
    g.players[1].is_minor = true;
    let home = g.city_at(at(6, 8)).unwrap();
    for pos in [at(8, 8), at(7, 9), at(7, 7)] {
        g.spawn_test_unit("man_at_arms", 1, pos);
    }
    let mut ai = on();
    ai.rebuild_force_groups(&g, 0, &conquest(&g, None));
    let initial = ai
        .objective_board()
        .rows
        .iter()
        .find(|row| row.key == ObjectiveKey::Defend(home))
        .unwrap()
        .deadline;
    g.turn += 1;
    let city = g.cities.get_mut(&home).unwrap();
    city.hp = 192;
    city.last_attacked = g.turn;
    ai.rebuild_force_groups(&g, 0, &conquest(&g, None));
    let damaged = ai
        .objective_board()
        .rows
        .iter()
        .find(|row| row.key == ObjectiveKey::Defend(home))
        .unwrap();
    assert_eq!(initial, Some(DEFEND_DEADLINE_FLOOR));
    assert_eq!(
        damaged.deadline, initial,
        "eight damage must not replace an imminent attack with a 24-turn deadline"
    );
}

#[test]
fn lightly_damaged_city_recalls_relief_from_a_valuable_siege() {
    let mut g = flat_board(379101, &[at(6, 8), at(30, 8)], false);
    war(&mut g, 0, 1);
    let home = g.city_at(at(6, 8)).unwrap();
    let target = g.city_at(at(30, 8)).unwrap();
    g.cities.get_mut(&target).unwrap().pop = 20;
    for pos in [at(8, 8), at(7, 9), at(7, 7)] {
        g.spawn_test_unit("warrior", 1, pos);
    }
    for pos in [at(5, 8), at(5, 9)] {
        g.spawn_test_unit("warrior", 0, pos);
    }
    let relief = g.spawn_test_unit("warrior", 0, at(18, 8));
    let mut ai = on();
    ai.rebuild_force_groups(&g, 0, &conquest(&g, Some(target)));
    g.turn += 1;
    let city = g.cities.get_mut(&home).unwrap();
    city.hp = 183;
    city.last_attacked = g.turn;
    ai.objective_board_state.city_health.insert(home, (g.turn - 1, 192));
    ai.rebuild_force_groups(&g, 0, &conquest(&g, Some(target)));
    let row = ai
        .objective_board()
        .rows
        .iter()
        .find(|row| row.key == ObjectiveKey::Defend(home))
        .unwrap();
    assert!(row.urgent, "nearby attackers still need timely relief: {row:?}");
    assert!(ai.objective_board().forces.iter().any(|force| {
        force.objective_key == ObjectiveKey::Defend(home) && force.units.contains(&relief)
    }));
}
