use super::super::objective_board::TaskForce;
use super::super::{ForceGroup, ForcePosture};
use super::*;

fn board() -> (Game, AdvancedAi, u32, u32, Pos, u32) {
    let mut g = Game::new_full(2, 32, 24, 91_989, 250, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    let cid = g.found_city_for(0, g.units[&settler].pos, None);
    let center = g.cities[&cid].pos;
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    g.turn = 20;
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    g.cities.get_mut(&cid).unwrap().hp = 170;
    let far = g
        .wring(center, 5)
        .into_iter()
        .find(|pos| g.map.get(*pos).is_some())
        .unwrap();
    let stand = g
        .wring(center, 2)
        .into_iter()
        .filter(|p| g.map.get(*p).is_some())
        .min_by_key(|p| g.wdist(*p, far))
        .unwrap();
    let observer = g
        .wring(center, 4)
        .into_iter()
        .filter(|p| g.map.get(*p).is_some())
        .min_by_key(|p| g.wdist(*p, far))
        .unwrap();
    let uid = g.spawn_unit("archer", 0, stand);
    g.spawn_unit("builder", 0, observer);
    let near = g
        .nbrs(center)
        .into_iter()
        .max_by_key(|p| g.wdist(*p, far))
        .unwrap();
    let besieger = g.spawn_unit("warrior", 1, near);
    let scout = g.spawn_unit("scout", 1, far);
    g.units.get_mut(&scout).unwrap().hp = 1;
    let mut ai = AdvancedAi::new();
    ai.enable_battle_planner_3();
    ai.objective_board = true;
    ai.objective_board_state.forces.push(TaskForce {
        id: 7,
        objective_key: ObjectiveKey::Defend(cid),
        domain: ForceDomain::Land,
        units: vec![uid],
        rally: center,
        doctrine_state: ForcePosture::Engage,
        aimed_at: center,
        formed: g.turn,
    });
    ai.force_groups.push(ForceGroup {
        id: 7,
        domain: ForceDomain::Land,
        units: vec![uid],
        anchor: center,
        objective: near,
        focus_target: Some(far),
        posture: ForcePosture::Engage,
        readiness: 1.0,
        local_strength_ratio: 0.65,
    });
    (g, ai, uid, cid, far, besieger)
}

#[test]
fn the_joint_combat_plan_does_not_walk_a_defender_outward_for_a_distant_kill() {
    let (g, mut ai, uid, cid, far, _) = board();
    let center = g.cities[&cid].pos;
    let ordinary = ai.kill_sequence(&g, 0);
    assert!(
        ordinary
            .iter()
            .any(|b| b.unit == uid && b.ranged && b.target == far && g.wdist(b.from, center) > 2),
        "control must open a distant firing stand"
    );
    ai.enable_defenders_keep_the_city_in_reach();
    let guarded = ai.kill_sequence(&g, 0);
    assert!(guarded
        .iter()
        .filter(|b| b.unit == uid && b.ranged)
        .all(|b| g.wdist(b.from, center) <= 2));
    assert!(!guarded.iter().any(|b| b.unit == uid && b.target == far));
}

#[test]
fn an_immediate_shot_at_the_besieger_remains_available() {
    let (mut g, mut ai, uid, cid, far, besieger) = board();
    let scout = g
        .units
        .values()
        .find(|u| u.pos == far && u.owner == 1)
        .unwrap()
        .id;
    g.units.get_mut(&scout).unwrap().hp = 100;
    g.units.get_mut(&besieger).unwrap().hp = 1;
    ai.enable_defenders_keep_the_city_in_reach();
    let blows = ai.kill_sequence(&g, 0);
    assert!(blows
        .iter()
        .any(|b| b.unit == uid && b.ranged && b.defender == besieger));
    assert!(blows
        .iter()
        .filter(|b| b.unit == uid && b.ranged)
        .all(|b| g.wdist(b.from, g.cities[&cid].pos) <= 2));
}

#[test]
fn an_arriving_defender_retains_its_current_firing_radius() {
    let (mut g, mut ai, uid, cid, far, _) = board();
    let center = g.cities[&cid].pos;
    let stand = g
        .wring(center, 3)
        .into_iter()
        .filter(|p| g.map.get(*p).is_some())
        .min_by_key(|p| g.wdist(*p, far))
        .unwrap();
    g.units.get_mut(&uid).unwrap().pos = stand;
    ai.enable_defenders_keep_the_city_in_reach();
    assert_eq!(ai.defensive_firing_radius(&g, 0, uid), Some((center, 3)));
    let blows = ai.kill_sequence(&g, 0);
    assert!(blows
        .iter()
        .any(|b| b.unit == uid && b.ranged && b.target == far));
    assert!(blows
        .iter()
        .filter(|b| b.unit == uid && b.ranged)
        .all(|b| g.wdist(b.from, center) <= 3));
}

#[test]
fn relief_offense_wounded_unassigned_and_unthreatened_units_have_no_new_bound() {
    let (mut g, mut ai, uid, cid, _, _) = board();
    ai.enable_defenders_keep_the_city_in_reach();
    for key in [
        ObjectiveKey::Relieve(cid),
        ObjectiveKey::Siege(cid),
        ObjectiveKey::Reserve,
    ] {
        ai.objective_board_state.forces[0].objective_key = key;
        assert_eq!(ai.defensive_firing_radius(&g, 0, uid), None);
    }
    ai.objective_board_state.forces[0].objective_key = ObjectiveKey::Defend(cid);
    g.units.get_mut(&uid).unwrap().hp = 30;
    assert_eq!(ai.defensive_firing_radius(&g, 0, uid), None);
    g.units.get_mut(&uid).unwrap().hp = 100;
    ai.force_groups[0].id = 8;
    assert_eq!(ai.defensive_firing_radius(&g, 0, uid), None);
    ai.force_groups[0].id = 7;
    g.cities.get_mut(&cid).unwrap().hp = 200;
    g.at_war.clear();
    assert_eq!(ai.defensive_firing_radius(&g, 0, uid), None);
    g.cities.get_mut(&cid).unwrap().owner = 1;
    g.cities.get_mut(&cid).unwrap().hp = 170;
    assert_eq!(ai.defensive_firing_radius(&g, 0, uid), None);
}

#[test]
fn the_independent_gene_is_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "defenders-keep-the-city-in-reach",
        |ai| ai.defenders_keep_the_city_in_reach,
    );
}

#[test]
fn the_later_movement_score_cannot_restore_a_denied_outward_shot() {
    let (mut g, mut ai, uid, cid, far, _) = board();
    let center = g.cities[&cid].pos;
    let group = ai.force_groups[0].clone();
    let outward = g
        .wring(center, 3)
        .into_iter()
        .filter(|p| g.map.get(*p).is_some())
        .min_by_key(|p| g.wdist(*p, far))
        .unwrap();
    ai.strike_opening = false;
    let score = |g: &Game, ai: &AdvancedAi, tile| {
        ai.strike_opening_value(super::super::StrikeOpening {
            g,
            pid: 0,
            uid,
            tile,
            group: &group,
            enemies: &[1],
            visible: None,
        })
    };
    assert_eq!(score(&g, &ai, outward), 0.0);
    ai.enable_defenders_keep_the_city_in_reach();
    assert_eq!(score(&g, &ai, outward), f64::NEG_INFINITY);
    assert_eq!(score(&g, &ai, g.units[&uid].pos), 0.0);
    g.units.get_mut(&uid).unwrap().hp = 30;
    assert_eq!(score(&g, &ai, outward), 0.0);
}

use super::super::battle_planner::{DangerField, PositionPlan};
use std::collections::BTreeSet;

fn formation_board() -> (Game, AdvancedAi, ForceGroup, u32, Pos) {
    let mut g = Game::new_full(2, 32, 24, 91_989, 250, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    let cid = g.found_city_for(0, g.units[&settler].pos, None);
    let center = g.cities[&cid].pos;
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    g.turn = 20;
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    g.cities.get_mut(&cid).unwrap().hp = 170;
    let far = g
        .wring(center, 5)
        .into_iter()
        .find(|pos| g.map.get(*pos).is_some())
        .unwrap();
    let mut ring = g.wring(center, 3);
    ring.retain(|pos| g.map.get(*pos).is_some());
    ring.sort_by_key(|pos| (g.wdist(*pos, far), *pos));
    let mut units: Vec<_> = ring
        .into_iter()
        .take(2)
        .map(|pos| g.spawn_unit("archer", 0, pos))
        .collect();
    let near = g.nbrs(center);
    let enemy = *near.iter().min_by_key(|pos| g.wdist(**pos, far)).unwrap();
    let own = *near.iter().max_by_key(|pos| g.wdist(**pos, far)).unwrap();
    units.push(g.spawn_unit("warrior", 0, own));
    g.spawn_unit("warrior", 1, enemy);
    let scout = g.spawn_unit("scout", 1, far);
    g.units.get_mut(&scout).unwrap().hp = 30;
    let mut ai = AdvancedAi::new();
    ai.enable_battle_planner_3();
    ai.objective_board = true;
    ai.objective_board_state.forces.push(TaskForce {
        id: 7,
        objective_key: ObjectiveKey::Defend(cid),
        domain: ForceDomain::Land,
        units: units.clone(),
        rally: center,
        doctrine_state: ForcePosture::Engage,
        aimed_at: center,
        formed: g.turn,
    });
    let group = ForceGroup {
        id: 7,
        domain: ForceDomain::Land,
        units,
        anchor: center,
        objective: enemy,
        focus_target: Some(far),
        posture: ForcePosture::Engage,
        readiness: 1.0,
        local_strength_ratio: 0.65,
    };
    (g, ai, group, cid, far)
}

fn positions(g: &Game, ai: &AdvancedAi, group: &ForceGroup) -> PositionPlan {
    ai.position_plan(g, 0, group, &mut DangerField::new(g, 0), &BTreeSet::new())
        .unwrap()
}

#[test]
fn defensive_combat_slots_stay_in_reach_of_the_assigned_city() {
    let (g, mut ai, group, cid, far) = formation_board();
    let ordinary = positions(&g, &ai, &group);
    assert_eq!(
        ordinary.target, far,
        "the ordinary formation follows its distant focus"
    );
    ai.enable_defenders_keep_the_city_in_reach();
    let guarded = positions(&g, &ai, &group);
    let center = g.cities[&cid].pos;
    assert_eq!(guarded.target, center);
    assert!(!guarded.slots.is_empty());
    assert!(guarded
        .slots
        .iter()
        .all(|slot| g.wdist(slot.tile, center) <= 2));
    assert!(guarded.placements.iter().any(|p| p.role.is_some()));
    assert!(guarded
        .placements
        .iter()
        .filter(|p| p.role.is_some())
        .all(|p| g.wdist(p.tile, center) <= 2));
    ai.disable_defenders_keep_the_city_in_reach();
    assert_eq!(positions(&g, &ai, &group), ordinary);
}

#[test]
fn relief_offense_unassigned_and_unthreatened_formations_keep_their_plan() {
    let (mut g, mut ai, mut group, cid, _) = formation_board();
    ai.enable_defenders_keep_the_city_in_reach();
    for key in [
        ObjectiveKey::Relieve(cid),
        ObjectiveKey::Siege(cid),
        ObjectiveKey::Reserve,
    ] {
        ai.objective_board_state.forces[0].objective_key = key;
        let enabled = positions(&g, &ai, &group);
        ai.disable_defenders_keep_the_city_in_reach();
        assert_eq!(positions(&g, &ai, &group), enabled);
        ai.enable_defenders_keep_the_city_in_reach();
    }
    ai.objective_board_state.forces[0].objective_key = ObjectiveKey::Defend(cid);
    group.id = 8;
    assert_eq!(ai.defensive_formation_anchor(&g, 0, &group), None);
    group.id = 7;
    g.cities.get_mut(&cid).unwrap().hp = 200;
    g.at_war.clear();
    assert_eq!(ai.defensive_formation_anchor(&g, 0, &group), None);
    g.cities.get_mut(&cid).unwrap().owner = 1;
    g.cities.get_mut(&cid).unwrap().hp = 170;
    assert_eq!(ai.defensive_formation_anchor(&g, 0, &group), None);
}

#[test]
fn a_wounded_member_can_still_heal_outside_the_defensive_ring() {
    let (mut g, mut ai, mut group, cid, far) = formation_board();
    let center = g.cities[&cid].pos;
    let tile = g
        .wring(center, 5)
        .into_iter()
        .filter(|p| g.map.get(*p).is_some())
        .max_by_key(|p| g.wdist(*p, far))
        .unwrap();
    let wounded = g.spawn_unit("archer", 0, tile);
    g.units.get_mut(&wounded).unwrap().hp = 30;
    group.units.push(wounded);
    ai.enable_defenders_keep_the_city_in_reach();
    let plan = positions(&g, &ai, &group);
    let recovery = plan
        .placements
        .iter()
        .find(|p| p.unit == wounded)
        .expect("safe recovery placement");
    assert_eq!(recovery.role, None);
    assert!(g.wdist(recovery.tile, center) > 2);
}
