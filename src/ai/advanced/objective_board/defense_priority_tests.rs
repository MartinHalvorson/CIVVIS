use super::*;

fn defense(id: u32, value: f64, deadline: u32, urgent: bool) -> Objective {
    Objective {
        kind: ObjectiveKind::Defend,
        key: ObjectiveKey::Defend(id),
        at: (0, 0),
        value,
        requirement: ForceNeed::default(),
        deadline: Some(deadline),
        state: RowState::Open,
        depends_on: None,
        land: true,
        sea: false,
        label: id.to_string(),
        urgent,
    }
}

#[test]
fn endangered_city_precedes_lower_scoring_urgent_defense_and_offense() {
    let bogota = defense(1, 120.0, 3, true);
    let panama = defense(2, 160.0, 2, false);
    let mut siege = defense(3, 1000.0, 1, false);
    siege.kind = ObjectiveKind::Siege;
    siege.key = ObjectiveKey::Siege(3);
    let mut rows = vec![siege, bogota, panama];
    AdvancedAi::order_board_rows(&mut rows);
    assert_eq!(
        rows.iter().map(|row| row.key).collect::<Vec<_>>(),
        vec![
            ObjectiveKey::Defend(2),
            ObjectiveKey::Defend(1),
            ObjectiveKey::Siege(3)
        ]
    );
}

#[test]
fn offense_keeps_score_priority_when_no_defense_is_urgent() {
    let mut siege = defense(3, 1000.0, 1, false);
    siege.kind = ObjectiveKind::Siege;
    siege.key = ObjectiveKey::Siege(3);
    let mut rows = vec![defense(1, 120.0, 3, false), siege];
    AdvancedAi::order_board_rows(&mut rows);
    assert_eq!(rows[0].key, ObjectiveKey::Siege(3));
}

#[test]
fn relief_cannot_precede_its_city_even_with_a_higher_score() {
    let mut relief = defense(2, 1000.0, 1, false);
    relief.kind = ObjectiveKind::Relieve;
    relief.key = ObjectiveKey::Relieve(1);
    relief.depends_on = Some(ObjectiveKey::Defend(1));
    let mut rows = vec![relief, defense(1, 120.0, 3, true)];
    AdvancedAi::order_board_rows(&mut rows);
    assert_eq!(rows[0].key, ObjectiveKey::Defend(1));
    assert_eq!(rows[1].key, ObjectiveKey::Relieve(1));
}

#[test]
fn lower_urgent_defense_does_not_take_the_higher_citys_only_guard() {
    use super::tests::{at, conquest, flat_board, on, war};
    let mut g = flat_board(374500, &[at(6, 8), at(30, 8)], false);
    let first = g.city_at(at(6, 8)).unwrap();
    let second = g.found_city_for(0, at(6, 20), None);
    g.cities.get_mut(&first).unwrap().pop = 10;
    g.cities.get_mut(&second).unwrap().pop = 1;
    war(&mut g, 0, 1);
    for pos in [at(8, 8), at(7, 9), at(8, 20), at(7, 21)] {
        g.spawn_test_unit("warrior", 1, pos);
    }
    let guard = g.spawn_test_unit("warrior", 0, at(5, 8));
    let mut ai = on();
    ai.rebuild_force_groups(&g, 0, &conquest(&g, None));
    let board = ai.objective_board();
    let defenses: Vec<_> = board
        .rows
        .iter()
        .filter(|row| row.kind == ObjectiveKind::Defend)
        .collect();
    assert_eq!(defenses.len(), 2);
    assert_eq!(defenses[0].key, ObjectiveKey::Defend(first));
    assert!(
        defenses[1].urgent,
        "exercise the lower urgent row's steal path"
    );
    let force = board
        .forces
        .iter()
        .find(|force| force.units.contains(&guard))
        .unwrap();
    assert_eq!(force.objective_key, ObjectiveKey::Defend(first));
}
