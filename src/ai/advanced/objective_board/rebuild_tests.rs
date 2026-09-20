use super::*;
use crate::name;

fn board(rebuilt: bool) -> Game {
    let mut g = Game::new_full(2, 40, 24, 936002, 1000, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    let mut cities = vec![(0, (6, 12)), (1, (18, 12))];
    if rebuilt {
        cities.reverse();
    }
    for (pid, pos) in cities {
        g.found_city_for(pid, pos, None);
    }
    if rebuilt {
        g.spawn_test_unit("builder", 0, (6, 12));
    }
    for pos in [(12, 12), (13, 12), (12, 13)] {
        g.spawn_test_unit("swordsman", 0, pos);
    }
    for pid in 0..2 {
        g.players[pid].met.insert(1 - pid);
        g.players[pid].explored.extend(g.map.tiles.keys().copied());
    }
    g.at_war.clear();
    g.at_war.insert((0, 1));
    g.turn = 60;
    g.current = 0;
    g
}

fn correspondence(old: &Game, next: &Game) -> BTreeMap<u32, u32> {
    old.units
        .values()
        .filter_map(|u| {
            next.units
                .values()
                .find(|v| v.owner == u.owner && v.kind == u.kind && v.pos == u.pos)
                .map(|v| (u.id, v.id))
        })
        .collect()
}

fn plan(g: &Game) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: g.city_at((18, 12)),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    }
}

#[test]
fn rebuilt_board_keeps_the_same_siege_force_and_its_actual_soldiers() {
    let old = board(false);
    let mut next = board(true);
    let map = correspondence(&old, &next);
    assert!(
        map.iter().any(|(a, b)| a != b),
        "fixture must renumber units"
    );
    let old_city = old.city_at((18, 12)).unwrap();
    let new_city = next.city_at((18, 12)).unwrap();
    assert_ne!(old_city, new_city);
    let mut ai = AdvancedAi::new();
    ai.enable_objective_board();
    ai.board_rebuild_force_groups(&old, 0, &plan(&old));
    let force = ai
        .objective_board_state
        .forces
        .iter()
        .find(|f| f.objective_key == ObjectiveKey::Siege(old_city))
        .expect("fixture siege force")
        .clone();
    let expected: Vec<_> = force.units.iter().map(|u| map[u]).collect();
    ai.remap_objective_board_memory(&old, &next, &map);
    let carried = ai
        .objective_board_state
        .forces
        .iter()
        .find(|f| f.id == force.id)
        .unwrap();
    assert_eq!(carried.objective_key, ObjectiveKey::Siege(new_city));
    assert_eq!(carried.units, expected);
    assert_eq!(carried.formed, force.formed);
    assert_eq!(
        ai.objective_board_state.assessed,
        Some((60, 0)),
        "same-turn frames preserve the assessment cadence"
    );
    next.turn += 1;
    ai.board_rebuild_force_groups(&next, 0, &plan(&next));
    let reviewed = ai
        .objective_board_state
        .forces
        .iter()
        .find(|f| f.objective_key == ObjectiveKey::Siege(new_city))
        .unwrap();
    assert_eq!(
        reviewed.id, force.id,
        "reassessment must retain the force identity"
    );
    assert_eq!(
        reviewed.units.iter().copied().collect::<BTreeSet<_>>(),
        expected.into_iter().collect()
    );
}

#[test]
fn pressure_history_and_requisition_follow_the_city_instead_of_its_old_id() {
    let old = board(false);
    let next = board(true);
    let home = old.city_at((6, 12)).unwrap();
    let new_home = next.city_at((6, 12)).unwrap();
    let mut ai = AdvancedAi::new();
    ai.objective_board_state.city_health.insert(home, (59, 155));
    ai.objective_board_state.damage_rate.insert(home, 45.0);
    ai.objective_board_state.requisitions.push(Requisition {
        kind: ObjectiveKind::Defend,
        count: 1,
        by_turn: Some(62),
        city: Some(home),
        unmet: ForceNeed::default(),
        have: ForceNeed::default(),
        sea_only: false,
        label: "home defense".into(),
    });
    ai.remap_objective_board_memory(&old, &next, &correspondence(&old, &next));
    assert_eq!(
        ai.objective_board_state.city_health.get(&new_home),
        Some(&(59, 155))
    );
    assert_eq!(
        ai.objective_board_state.damage_rate.get(&new_home),
        Some(&45.0)
    );
    assert!(!ai.objective_board_state.city_health.contains_key(&home));
    assert_eq!(ai.requisitions()[0].city, Some(new_home));
}

fn row(key: ObjectiveKey, dependency: Option<ObjectiveKey>) -> Objective {
    Objective {
        kind: ObjectiveKind::Siege,
        key,
        at: (18, 12),
        value: 100.0,
        requirement: ForceNeed::default(),
        deadline: None,
        state: RowState::Open,
        depends_on: dependency,
        land: true,
        sea: false,
        label: "fixture".into(),
        urgent: false,
    }
}

#[test]
fn every_identity_key_and_dependency_is_remapped_without_changing_spatial_keys() {
    let old = board(false);
    let next = board(true);
    let map = correspondence(&old, &next);
    let home = old.city_at((6, 12)).unwrap();
    let new_home = next.city_at((6, 12)).unwrap();
    let uid = *map.keys().next().unwrap();
    let new_uid = map[&uid];
    let pairs = [
        (ObjectiveKey::Defend(home), ObjectiveKey::Defend(new_home)),
        (ObjectiveKey::Relieve(home), ObjectiveKey::Relieve(new_home)),
        (ObjectiveKey::Siege(home), ObjectiveKey::Siege(new_home)),
        (ObjectiveKey::Deter(home), ObjectiveKey::Deter(new_home)),
        (ObjectiveKey::Destroy(uid), ObjectiveKey::Destroy(new_uid)),
        (ObjectiveKey::Escort(uid), ObjectiveKey::Escort(new_uid)),
        (ObjectiveKey::Flag((10, 12)), ObjectiveKey::Flag((10, 12))),
        (ObjectiveKey::Camp((11, 12)), ObjectiveKey::Camp((11, 12))),
        (ObjectiveKey::Recon(7), ObjectiveKey::Recon(7)),
        (ObjectiveKey::Reserve, ObjectiveKey::Reserve),
    ];
    let mut ai = AdvancedAi::new();
    ai.objective_board_state.rows = pairs
        .iter()
        .map(|(key, _)| row(*key, Some(ObjectiveKey::Defend(home))))
        .collect();
    ai.remap_objective_board_memory(&old, &next, &map);
    for (row, (_, expected)) in ai.objective_board_state.rows.iter().zip(pairs) {
        assert_eq!(row.key, expected);
        assert_eq!(row.depends_on, Some(ObjectiveKey::Defend(new_home)));
    }
}

#[test]
fn missing_objectives_and_soldiers_do_not_attach_to_reused_ids() {
    let old = board(false);
    let mut next = board(true);
    let old_city = old.city_at((18, 12)).unwrap();
    let new_city = next.city_at((18, 12)).unwrap();
    next.mirror_remove_city(new_city);
    let uid = old.player_unit_ids(0)[0];
    let mut ai = AdvancedAi::new();
    ai.objective_board_state.rows = vec![
        row(ObjectiveKey::Siege(old_city), None),
        row(ObjectiveKey::Destroy(uid), None),
    ];
    ai.objective_board_state.forces = vec![TaskForce {
        id: 42,
        objective_key: ObjectiveKey::Siege(old_city),
        domain: ForceDomain::Land,
        units: vec![uid],
        rally: (15, 12),
        doctrine_state: ForcePosture::Muster,
        aimed_at: (18, 12),
        formed: 55,
    }];
    ai.objective_board_state
        .city_health
        .insert(old_city, (59, 100));
    ai.remap_objective_board_memory(&old, &next, &BTreeMap::new());
    assert!(ai.objective_board_state.rows.is_empty());
    assert!(ai.objective_board_state.forces.is_empty());
    assert!(ai.objective_board_state.city_health.is_empty());
}

#[test]
fn enemy_objectives_use_host_identity_and_owner_when_only_own_units_are_carried() {
    let mut old = board(false);
    let mut next = board(true);
    let own = correspondence(&old, &next);
    let enemy = old.spawn_test_unit("archer", 1, (18, 13));
    let new_enemy = next.spawn_test_unit("archer", 1, (19, 13));
    let ours = *own.keys().next().unwrap();
    let new_ours = own[&ours];
    for (g, uid) in [(&mut old, enemy), (&mut next, new_enemy)] {
        std::sync::Arc::make_mut(&mut g.host_unit_facts).insert(
            uid,
            crate::game::HostUnitFacts {
                civ6_id: Some(123),
                ..Default::default()
            },
        );
    }
    std::sync::Arc::make_mut(&mut next.host_unit_facts).insert(
        new_ours,
        crate::game::HostUnitFacts {
            civ6_id: Some(123),
            ..Default::default()
        },
    );
    let mut ai = AdvancedAi::new();
    ai.objective_board_state.rows = vec![row(ObjectiveKey::Destroy(enemy), None)];
    ai.objective_board_state.forces = vec![TaskForce {
        id: 42,
        objective_key: ObjectiveKey::Destroy(enemy),
        domain: ForceDomain::Land,
        units: vec![ours],
        rally: (15, 12),
        doctrine_state: ForcePosture::Engage,
        aimed_at: (18, 13),
        formed: 55,
    }];
    let mut without_foreign_identity = old.clone();
    std::sync::Arc::make_mut(&mut without_foreign_identity.host_unit_facts).clear();
    let mut control = ai.clone();
    control.remap_objective_board_memory(&without_foreign_identity, &next, &own);
    assert!(
        control.objective_board_state.forces.is_empty(),
        "own-unit mapping alone cannot carry a foreign objective"
    );
    ai.remap_objective_board_memory(&old, &next, &own);
    let force = &ai.objective_board_state.forces[0];
    assert_eq!(force.id, 42);
    assert_eq!(force.objective_key, ObjectiveKey::Destroy(new_enemy));
    assert_eq!(force.units, vec![new_ours]);
    assert_eq!(ai.objective_board_state.rows[0].key, force.objective_key);
}

#[test]
fn a_missing_soldier_is_dropped_without_replacing_it_with_the_new_holder_of_its_id() {
    let old = board(false);
    let mut next = board(true);
    let mut map = correspondence(&old, &next);
    let ids: Vec<_> = map.keys().copied().collect();
    let lost = ids[0];
    next.remove_unit(map.remove(&lost).unwrap());
    let survivor = ids[1];
    let mut ai = AdvancedAi::new();
    ai.objective_board_state.forces.push(TaskForce {
        id: 42,
        objective_key: ObjectiveKey::Siege(old.city_at((18, 12)).unwrap()),
        domain: ForceDomain::Land,
        units: vec![lost, survivor],
        rally: (15, 12),
        doctrine_state: ForcePosture::Muster,
        aimed_at: (18, 12),
        formed: 55,
    });
    ai.remap_objective_board_memory(&old, &next, &map);
    assert_eq!(ai.objective_board_state.forces.len(), 1);
    assert_eq!(ai.objective_board_state.forces[0].id, 42);
    assert_eq!(
        ai.objective_board_state.forces[0].units,
        vec![map[&survivor]]
    );
}
