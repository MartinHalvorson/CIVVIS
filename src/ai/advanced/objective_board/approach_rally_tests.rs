use super::*;
use crate::ai::advanced::siege_train::Siege;
use crate::name;

fn fixture(center: Pos, hostile: bool) -> (Game, AdvancedAi, StrategicPlan, Pos) {
    let mut g = Game::new_full(2, 40, 24, 936007, 1000, 0, false);
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
    }
    g.found_city_for(0, (26, 12), None);
    let objective = (10, 12);
    let target = g.found_city_for(1, objective, None);
    let units = [center, (center.0, center.1 + 1), (center.0 + 1, center.1)]
        .into_iter()
        .map(|p| g.spawn_test_unit("swordsman", 0, p))
        .collect();
    if hostile {
        g.spawn_test_unit("warrior", 1, (14, 12));
        g.spawn_test_unit("scout", 0, (15, 12));
    }
    g.at_war.clear();
    g.at_war.insert((0, 1));
    for pid in 0..2 {
        g.players[pid].met.insert(1 - pid);
        g.players[pid].explored.extend(g.map.tiles.keys().copied());
    }
    g.turn = 100;
    g.current = 0;
    let mut ai = AdvancedAi::new();
    ai.enable_objective_board();
    ai.enable_siege_train();
    ai.objective_board_state.rows.push(Objective {
        kind: ObjectiveKind::Siege,
        key: ObjectiveKey::Siege(target),
        at: objective,
        value: 200.0,
        requirement: ForceNeed::default(),
        deadline: None,
        state: RowState::Staging,
        depends_on: None,
        land: true,
        sea: false,
        label: "test siege".into(),
        urgent: false,
    });
    ai.objective_board_state.forces.push(TaskForce {
        id: 1,
        objective_key: ObjectiveKey::Siege(target),
        domain: ForceDomain::Land,
        units,
        rally: center,
        doctrine_state: ForcePosture::Muster,
        aimed_at: objective,
        formed: g.turn,
    });
    ai.sieges.insert(
        target,
        Siege {
            stage: SiegeStage::Stage,
            taker: None,
            entered: g.turn,
            assessed: g.turn,
            posts: BTreeMap::new(),
        },
    );
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan, objective)
}

#[test]
fn a_land_siege_musters_on_its_approach_side_of_the_city() {
    let center = (20, 12);
    let (g, mut ai, plan, objective) = fixture(center, true);
    let visible = ai.battlefront_visibility(&g, 0);
    let old_rally = ai.far_side(&g, 0, objective, center, &visible);
    assert!(
        g.wdist(old_rally, center) > g.wdist(objective, center),
        "the old rule must demonstrate the far-side detour: {old_rally:?}"
    );
    ai.project_forces(&g, 0, &plan);
    let group = &ai.force_groups[0];
    assert_eq!(group.posture, ForcePosture::Muster);
    assert!(
        g.wdist(group.anchor, center) < g.wdist(objective, center),
        "rally {:?} sends the army beyond city {:?}",
        group.anchor,
        objective
    );
    assert!((2..=3).contains(&g.wdist(group.anchor, objective)));
}

#[test]
fn a_land_siege_approaches_even_without_a_visible_field_army() {
    let center = (20, 12);
    let (g, mut ai, plan, objective) = fixture(center, false);
    ai.project_forces(&g, 0, &plan);
    assert!(g.wdist(ai.force_groups[0].anchor, objective) < g.wdist(center, objective));
}

#[test]
fn a_siege_already_in_staging_range_keeps_its_assembly_point() {
    let center = (12, 12);
    let (g, mut ai, plan, _) = fixture(center, true);
    ai.project_forces(&g, 0, &plan);
    assert_eq!(ai.force_groups[0].anchor, center);
}

#[test]
fn no_passable_approach_keeps_the_force_together() {
    let center = (20, 12);
    let (mut g, mut ai, plan, objective) = fixture(center, true);
    for pos in g.wdisk(objective, 3) {
        if g.wdist(pos, objective) >= 2 {
            g.map.tiles.get_mut(&pos).unwrap().terrain = name!("ocean");
        }
    }
    ai.project_forces(&g, 0, &plan);
    assert_eq!(ai.force_groups[0].anchor, center);
}

#[test]
fn naval_sieges_keep_the_existing_rally_policy() {
    let center = (20, 12);
    let (g, mut ai, plan, objective) = fixture(center, true);
    ai.objective_board_state.forces[0].domain = ForceDomain::Sea;
    let visible = ai.battlefront_visibility(&g, 0);
    let old_rally = ai.far_side(&g, 0, objective, center, &visible);
    ai.project_forces(&g, 0, &plan);
    assert_eq!(ai.force_groups[0].anchor, old_rally);
}
