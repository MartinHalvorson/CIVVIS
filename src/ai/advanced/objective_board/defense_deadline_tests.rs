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
    ai.objective_board_state
        .city_health
        .insert(home, (g.turn - 1, 192));
    ai.rebuild_force_groups(&g, 0, &conquest(&g, Some(target)));
    let row = ai
        .objective_board()
        .rows
        .iter()
        .find(|row| row.key == ObjectiveKey::Defend(home))
        .unwrap();
    assert!(
        row.urgent,
        "nearby attackers still need timely relief: {row:?}"
    );
    assert!(ai.objective_board().forces.iter().any(|force| {
        force.objective_key == ObjectiveKey::Defend(home) && force.units.contains(&relief)
    }));
}

fn assigned_relief() -> (Game, AdvancedAi, u32, u32, u32) {
    let mut g = flat_board(379110, &[at(19, 8), at(30, 15)], false);
    war(&mut g, 0, 1);
    let home = g.city_at(at(19, 8)).unwrap();
    let archer = g.spawn_test_unit("archer", 0, at(16, 12));
    let victim = g.spawn_test_unit("archer", 1, at(17, 13));
    g.units.get_mut(&victim).unwrap().hp = 1;
    let mut ai = on();
    ai.enable_battle_planner_2();
    ai.objective_board_state.rows = vec![Objective {
        kind: ObjectiveKind::Defend,
        key: ObjectiveKey::Defend(home),
        at: g.cities[&home].pos,
        value: 140.0,
        requirement: ForceNeed::default(),
        deadline: Some(2),
        state: RowState::Open,
        depends_on: None,
        land: true,
        sea: false,
        label: g.cities[&home].name.clone(),
        urgent: true,
    }];
    ai.objective_board_state.forces = vec![TaskForce {
        id: 1,
        objective_key: ObjectiveKey::Defend(home),
        domain: ForceDomain::Land,
        units: vec![archer],
        rally: g.cities[&home].pos,
        doctrine_state: ForcePosture::Hold,
        aimed_at: g.cities[&home].pos,
        formed: g.turn - 2,
    }];
    (g, ai, home, archer, victim)
}

#[test]
fn overdue_city_relief_does_not_spend_its_turn_on_a_remote_kill() {
    let (g, ai, _, archer, victim) = assigned_relief();
    let blows = ai.kill_sequence(&g, 0);
    assert!(
        blows
            .iter()
            .all(|blow| blow.unit != archer || blow.target != g.units[&victim].pos),
        "the relief mission is due; this remote kill must not consume its next move: {blows:?}"
    );
}

#[test]
fn disabling_the_board_does_not_leave_a_stale_relief_attack_veto() {
    let (g, mut ai, _, archer, victim) = assigned_relief();
    ai.objective_board = false;
    assert!(ai
        .kill_sequence(&g, 0)
        .iter()
        .any(|blow| blow.unit == archer && blow.target == g.units[&victim].pos));
}

#[test]
fn relief_can_finish_a_remote_kill_when_it_retains_arrival_time() {
    let (mut g, mut ai, _, archer, victim) = assigned_relief();
    ai.objective_board_state.forces[0].formed = g.turn;
    // Match the recorded Gran Colombia allowance independently of unit type.
    std::sync::Arc::make_mut(&mut g.host_unit_facts).insert(
        archer,
        crate::game::HostUnitFacts {
            max_moves: Some(3.0),
            ..Default::default()
        },
    );
    assert!(ai
        .kill_sequence(&g, 0)
        .iter()
        .any(|blow| { blow.unit == archer && blow.target == g.units[&victim].pos }));
    g.turn += 1;
    assert!(
        ai.kill_sequence(&g, 0)
            .iter()
            .all(|blow| { blow.unit != archer || blow.target != g.units[&victim].pos }),
        "the same remote kill must not regain its travel slack on the next turn"
    );
}

#[test]
fn overdue_relief_keeps_a_direct_city_defense_shot() {
    let (mut g, ai, home, archer, victim) = assigned_relief();
    let city = g.cities[&home].pos;
    g.relocate(archer, at(18, 9));
    g.relocate(victim, at(20, 9));
    assert!(g.wdist(g.units[&archer].pos, city) <= 2);
    assert!(ai
        .kill_sequence(&g, 0)
        .iter()
        .any(|blow| { blow.unit == archer && blow.target == g.units[&victim].pos }));
}

#[test]
fn overdue_relief_can_stop_an_attacker_that_reaches_the_city_from_beyond_two_tiles() {
    let (mut g, ai, home, archer, victim) = assigned_relief();
    let city = g.cities[&home].pos;
    g.relocate(archer, at(14, 8));
    g.relocate(victim, at(16, 8));
    assert!(g.wdist(g.units[&victim].pos, city) > 2);
    let mut probe = g.speculative_clone();
    assert!(
        super::super::battle_planner::strike_reach_of(&mut probe, 0, victim)
            .binary_search(&city)
            .is_ok()
    );
    assert!(ai
        .kill_sequence(&g, 0)
        .iter()
        .any(|blow| { blow.unit == archer && blow.target == g.units[&victim].pos }));
}

#[test]
fn ordinary_military_scan_does_not_reopen_the_overdue_remote_kill() {
    let (mut g, mut ai, home, archer, victim) = assigned_relief();
    let city = g.cities[&home].pos;
    ai.victory_planning = true;
    ai.force_groups_dirty = false;
    ai.force_groups = vec![ForceGroup {
        id: 1,
        domain: ForceDomain::Land,
        units: vec![archer],
        anchor: city,
        objective: city,
        focus_target: None,
        posture: ForcePosture::Hold,
        readiness: 0.0,
        local_strength_ratio: 1.0,
    }];
    let mut plan = conquest(&g, None);
    plan.threatened_city = Some(home);
    assert!(ai.advanced_military_step(&mut g, 0, archer, &plan));
    assert_eq!(
        g.units[&victim].hp, 1,
        "the ordinary scan must preserve the veto"
    );

    // The wounded Archer still threatens the first approach tiles. Once
    // another body clears that front, the same defender must really march
    // on its next turn, rather than merely acquire a new force label. Keep
    // threat pricing in charge while the corridor is dangerous.
    g.remove_unit(victim);
    g.turn += 1;
    let moves = g.unit_max_moves(archer);
    let unit = g.units.get_mut(&archer).unwrap();
    unit.moves_left = moves;
    unit.attacks_left = 1;
    unit.moved = false;
    let before = g.wdist(g.units[&archer].pos, city);
    let hostiles = Vec::new();
    let next = g.route_step(archer, city, 2).unwrap();
    assert_eq!(
        ai.base
            .projected_counter_damage(&g, archer, next, &hostiles),
        0.0
    );
    assert!(
        ai.advanced_military_step(&mut g, 0, archer, &plan),
        "fresh relief dispatch should act with {moves} moves"
    );
    assert!(
        g.wdist(g.units[&archer].pos, city) < before,
        "the available relief body should actually close through the safe corridor"
    );
}

#[test]
fn frozen_legacy_control_preserves_its_historical_approach_score() {
    let (mut g, _, home, archer, victim) = assigned_relief();
    g.remove_unit(victim);
    g.relocate(archer, (9, 12));
    let city = g.cities[&home].pos;
    let before = g.wdist(g.units[&archer].pos, city);
    assert_eq!(before, 6);
    let group = ForceGroup {
        id: 1,
        domain: ForceDomain::Land,
        units: vec![archer],
        anchor: city,
        objective: city,
        focus_target: None,
        posture: ForcePosture::Hold,
        readiness: 0.0,
        local_strength_ratio: 1.0,
    };
    let ai = AdvancedAi::legacy();
    assert!(ai.coordinated_tactical_step(&mut g, 0, archer, &group, &[1], false));
    assert!(g.wdist(g.units[&archer].pos, city) >= before);
}
