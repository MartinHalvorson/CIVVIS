use super::super::battle_planner::{DangerField, PositionPlan};
use super::super::objective_board::TaskForce;
use super::super::ForcePosture;
use super::*;
use std::collections::BTreeSet;

fn board() -> (Game, AdvancedAi, ForceGroup, u32, Pos) {
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
    let (g, mut ai, group, cid, far) = board();
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
    let (mut g, mut ai, mut group, cid, _) = board();
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
    let (mut g, mut ai, mut group, cid, far) = board();
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

#[test]
fn the_independent_gene_is_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "defenders-keep-the-city-in-reach",
        |ai| ai.defenders_keep_the_city_in_reach,
    );
}

#[test]
fn the_later_strike_opening_cannot_send_a_healthy_defender_farther_away() {
    let (mut g, mut ai, group, cid, far) = board();
    let center = g.cities[&cid].pos;
    let uid = group.units[0];
    let outward = g
        .wring(center, 4)
        .into_iter()
        .filter(|pos| g.map.get(*pos).is_some())
        .min_by_key(|pos| g.wdist(*pos, far))
        .unwrap();
    assert_eq!(g.wdist(g.units[&uid].pos, center), 3);
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
    assert_eq!(
        score(&g, &ai, outward),
        f64::NEG_INFINITY,
        "the movement gate applies even when the independent strike-opening premium is off"
    );
    assert_eq!(
        score(&g, &ai, g.units[&uid].pos),
        0.0,
        "a shot from the current stand remains available"
    );
    g.units.get_mut(&uid).unwrap().hp = 30;
    assert_eq!(
        score(&g, &ai, outward),
        0.0,
        "wounded recovery keeps its range"
    );
    ai.objective_board_state.forces[0].objective_key = ObjectiveKey::Relieve(cid);
    g.units.get_mut(&uid).unwrap().hp = 100;
    assert_eq!(score(&g, &ai, outward), 0.0, "Relieve still marches");
}
