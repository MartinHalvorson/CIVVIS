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
        "exercise the lower urgent row's steal path: {defenses:?}"
    );
    let force = board
        .forces
        .iter()
        .find(|force| force.units.contains(&guard))
        .unwrap();
    assert_eq!(force.objective_key, ObjectiveKey::Defend(first));
}

/// `capital-defense-holds`, the board of live King civvis-20261005T013110Z
/// (game 91) at turn 40: Bogota at 20 of 200, a German unit beside it, and
/// our defenders standing close enough that the pressure ratio reads under
/// [`BASTION_PRESSURE`]. Off, the Defend row lapses; on, it stays and is
/// urgent, so the capital's guard is not handed to a Siege.
mod capital_defense_holds {
    use super::super::tests::{at, conquest, flat_board, on, war};
    use super::*;

    /// Our capital at (6, 8) at 20 hp with a hostile warrior beside it and
    /// four of our warriors around it; the rival's capital at (30, 8) is the
    /// plan's target, so a Siege row stands. Returns the game, our capital
    /// and the rival's.
    fn bogota_at_turn_40() -> (Game, u32, u32) {
        let mut g = flat_board(374501, &[at(6, 8), at(30, 8)], false);
        let capital = g.city_at(at(6, 8)).unwrap();
        let munich = g.city_at(at(30, 8)).unwrap();
        g.cities.get_mut(&capital).unwrap().pop = 6;
        war(&mut g, 0, 1);
        g.spawn_test_unit("warrior", 1, at(7, 8));
        for pos in [at(5, 8), at(6, 7), at(5, 9), at(6, 9)] {
            g.spawn_test_unit("warrior", 0, pos);
        }
        g.cities.get_mut(&capital).unwrap().hp = 20;
        (g, capital, munich)
    }

    fn board(g: &Game, ai: &mut AdvancedAi, target: Option<u32>) -> Vec<Objective> {
        ai.rebuild_force_groups(g, 0, &conquest(g, target));
        ai.objective_board().rows.clone()
    }

    #[test]
    fn the_setup_reads_under_the_pressure_gate() {
        let (g, capital, _) = bogota_at_turn_40();
        let pressure = AdvancedAi::city_pressure(&g, 0, capital);
        assert!(
            pressure < BASTION_PRESSURE,
            "our defenders hold the ratio under the gate: {pressure}"
        );
    }

    #[test]
    fn off_the_defend_row_of_a_falling_capital_lapses() {
        let (g, capital, munich) = bogota_at_turn_40();
        let mut ai = on();
        let rows = board(&g, &mut ai, Some(munich));
        assert!(
            !rows.iter().any(|row| row.key == ObjectiveKey::Defend(capital)),
            "the shipped gate drops the row: {rows:?}"
        );
    }

    #[test]
    fn on_the_capital_keeps_an_urgent_defend_ahead_of_the_siege() {
        let (g, capital, munich) = bogota_at_turn_40();
        let mut ai = on();
        ai.enable_capital_defense_holds();
        let rows = board(&g, &mut ai, Some(munich));
        let defend = rows
            .iter()
            .position(|row| row.key == ObjectiveKey::Defend(capital))
            .expect("the capital keeps its Defend row");
        assert!(rows[defend].urgent, "a capital under attack is urgent");
        let siege = rows
            .iter()
            .position(|row| row.key == ObjectiveKey::Siege(munich))
            .expect("the target's Siege row stands");
        assert!(defend < siege, "the capital outranks the siege: {rows:?}");
    }

    #[test]
    fn on_a_damaged_town_keeps_its_row_but_not_the_capitals_urgency() {
        let (mut g, _, _) = bogota_at_turn_40();
        let town = g.found_city_for(0, at(6, 16), None);
        g.spawn_test_unit("warrior", 1, at(7, 16));
        for pos in [at(5, 16), at(6, 15), at(5, 17), at(6, 17)] {
            g.spawn_test_unit("warrior", 0, pos);
        }
        g.cities.get_mut(&town).unwrap().hp = 60;
        assert!(!g.cities[&town].is_capital);
        assert!(AdvancedAi::city_pressure(&g, 0, town) < BASTION_PRESSURE);
        let mut ai = on();
        ai.enable_capital_defense_holds();
        let rows = board(&g, &mut ai, None);
        let row = rows
            .iter()
            .find(|row| row.key == ObjectiveKey::Defend(town))
            .expect("a damaged town with a hostile beside it keeps its row");
        assert!(!row.urgent, "only a capital is marked urgent: {row:?}");
    }

    /// The Defend need of our capital at (6, 8) with three hostile warriors
    /// beside it and no defender of ours, at `hp`, with the gene on or off,
    /// and the city's own strength as `Game::city_strength` reads it (which
    /// already loses up to nine points to damage).
    fn capital_need(hp: i32, gene: bool) -> (f64, f64) {
        let mut g = flat_board(374502, &[at(6, 8), at(30, 8)], false);
        let capital = g.city_at(at(6, 8)).unwrap();
        g.cities.get_mut(&capital).unwrap().pop = 6;
        war(&mut g, 0, 1);
        for pos in [at(7, 8), at(7, 7), at(7, 9)] {
            g.spawn_test_unit("warrior", 1, pos);
        }
        g.cities.get_mut(&capital).unwrap().hp = hp;
        let mut ai = on();
        if gene {
            ai.enable_capital_defense_holds();
        }
        let rows = board(&g, &mut ai, None);
        let need = rows
            .iter()
            .find(|row| row.key == ObjectiveKey::Defend(capital))
            .expect("three warriors at the gate raise a Defend row")
            .requirement
            .strength;
        (need, g.city_strength(capital))
    }

    #[test]
    fn on_a_damaged_capital_asks_for_more_than_a_healthy_one() {
        let (healthy, _) = capital_need(200, true);
        let (falling, _) = capital_need(20, true);
        assert!(
            falling > healthy,
            "20 of 200 credits a tenth of the city's strength: {falling} against {healthy}"
        );
        // At full health the gene credits the whole city, as off does.
        assert_eq!(healthy, capital_need(200, false).0);
        // And it asks for more than the shipped damage penalty alone adds.
        let (off_falling, _) = capital_need(20, false);
        assert!(falling > off_falling, "{falling} against off's {off_falling}");
    }

    #[test]
    fn off_the_need_moves_only_by_the_shipped_damage_penalty() {
        let (falling, weak) = capital_need(20, false);
        let (healthy, strong) = capital_need(200, false);
        assert!(
            (falling - healthy - (strong - weak)).abs() < 1e-9,
            "off subtracts the city's whole strength: need {falling} vs {healthy}, strength {weak} vs {strong}"
        );
    }

    #[test]
    fn on_a_walled_capital_keeps_its_row_through_the_walls_phase() {
        // Whole hit points, Ancient Walls at 40 of 100, a hostile beside it.
        let (mut g, capital, _) = bogota_at_turn_40();
        {
            let city = g.cities.get_mut(&capital).unwrap();
            city.hp = 200;
            city.buildings.push(crate::name!("walls"));
            city.wall_hp = 40;
        }
        assert_eq!(g.city_max_wall_hp(&g.cities[&capital]), 100);
        assert!(AdvancedAi::city_pressure(&g, 0, capital) < BASTION_PRESSURE);
        let mut off = on();
        let rows = board(&g, &mut off, None);
        assert!(!rows.iter().any(|row| row.key == ObjectiveKey::Defend(capital)));
        let mut ai = on();
        ai.enable_capital_defense_holds();
        let rows = board(&g, &mut ai, None);
        let row = rows
            .iter()
            .find(|row| row.key == ObjectiveKey::Defend(capital))
            .expect("battered walls count as damage");
        assert!(row.urgent, "a capital under bombardment is urgent: {row:?}");
    }

    #[test]
    fn on_a_capital_with_no_hostile_beside_it_is_not_made_urgent() {
        // Half health and pressed from five tiles out (seen by our unit seven
        // out), so the shipped gate raises the row; that unit is inside the
        // deadline, so the shipped relief rule leaves the row calm.
        let mut g = flat_board(374503, &[at(6, 8), at(30, 8)], false);
        let capital = g.city_at(at(6, 8)).unwrap();
        g.cities.get_mut(&capital).unwrap().pop = 6;
        war(&mut g, 0, 1);
        for pos in [at(11, 8), at(11, 7), at(11, 9)] {
            g.spawn_test_unit("warrior", 1, pos);
        }
        g.spawn_test_unit("warrior", 0, at(13, 8));
        g.cities.get_mut(&capital).unwrap().hp = 100;
        assert!(AdvancedAi::city_pressure(&g, 0, capital) >= BASTION_PRESSURE);
        let urgent = |gene: bool| {
            let mut ai = on();
            if gene {
                ai.enable_capital_defense_holds();
            }
            let rows = board(&g, &mut ai, None);
            rows.iter()
                .find(|row| row.key == ObjectiveKey::Defend(capital))
                .expect("the shipped gate raises the row")
                .urgent
        };
        assert!(!urgent(false), "the setup is calm off");
        assert!(!urgent(true), "no hostile within two: the gene adds no urgency");
    }

    #[test]
    fn on_a_healthy_capital_or_a_distant_hostile_reads_as_off() {
        // Full health, hostile beside it: no row either way.
        let (mut g, capital, _) = bogota_at_turn_40();
        g.cities.get_mut(&capital).unwrap().hp = 200;
        let mut ai = on();
        ai.enable_capital_defense_holds();
        let rows = board(&g, &mut ai, None);
        assert!(!rows.iter().any(|row| row.key == ObjectiveKey::Defend(capital)));
        // Damaged, but the hostile three tiles out: no contact, no row.
        let (mut g, capital, _) = bogota_at_turn_40();
        let near = g
            .units
            .values()
            .find(|unit| unit.owner == 1)
            .map(|unit| unit.id)
            .unwrap();
        g.units.get_mut(&near).unwrap().pos = at(9, 8);
        assert!(AdvancedAi::city_pressure(&g, 0, capital) < BASTION_PRESSURE);
        let mut ai = on();
        ai.enable_capital_defense_holds();
        let rows = board(&g, &mut ai, None);
        assert!(!rows.iter().any(|row| row.key == ObjectiveKey::Defend(capital)));
    }
}
