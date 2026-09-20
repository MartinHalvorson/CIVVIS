use super::*;
use crate::ai::advanced::{ForceDomain, GrandStrategy};
use crate::name;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut g = Game::new_full(2, 40, 24, 936008, 1000, 0, false);
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
    g.map.clear_rivers();
    g.found_city_for(0, (26, 12), None);
    let target = g.found_city_for(1, (4, 12), None);
    let uid = g.spawn_test_unit("horseman", 0, (10, 12));
    g.spawn_test_unit("infantry", 1, (13, 12));
    g.spawn_test_unit("infantry", 1, (13, 13));
    g.spawn_test_unit("builder", 0, (14, 11));
    g.at_war.clear();
    g.at_war.insert((0, 1));
    for pid in 0..2 {
        g.players[pid].met.insert(1 - pid);
        g.players[pid].explored.extend(g.map.tiles.keys().copied());
    }
    g.turn = 100;
    g.current = 0;
    let mut ai = AdvancedAi::new();
    ai.enable_battle_planner_2();
    ai.enable_doomed_blow_veto_2();
    ai.enable_siege_train();
    ai.force_groups.push(ForceGroup {
        id: 1,
        domain: ForceDomain::Land,
        units: vec![uid],
        anchor: (10, 12),
        objective: (4, 12),
        focus_target: None,
        posture: ForcePosture::Muster,
        readiness: 0.0,
        local_strength_ratio: 1.0,
    });
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan, uid)
}

#[test]
fn a_vetoed_attack_does_not_prevent_a_safe_siege_approach() {
    let (mut g, mut ai, plan, uid) = fixture();
    let before = g.units[&uid].pos;
    let target = g.cities[&plan.target_city.unwrap()].pos;
    let enemy_hp: Vec<_> = g
        .units
        .values()
        .filter(|u| u.owner == 1)
        .map(|u| (u.id, u.hp))
        .collect();
    let mut field = DangerField::with_reach(&g, 0, ai.strike_reach);
    let (_, _, _, doomed) = ai.kill_sequence_in(&g, 0, &mut field);
    assert!(
        doomed.contains(&uid),
        "fixture must offer only suicidal attacks; moves={} reach={:?}",
        g.units[&uid].moves_left,
        g.reachable(uid)
    );
    assert!(
        field.danger((9, 12), uid) <= NO_DANGER,
        "the approach step is safe"
    );
    ai.plan_battle(&mut g, 0, &plan);
    assert!(
        g.wdist(g.units[&uid].pos, target) < g.wdist(before, target),
        "the attack veto must leave safe siege progress available"
    );
    assert!(ai.battle_planner_ordered.contains(&uid));
    assert!(!ai.battle_planner_recovering.contains(&uid));
    assert_eq!(g.units[&uid].attacks_left, 1);
    for (id, hp) in enemy_hp {
        assert_eq!(g.units[&id].hp, hp, "no vetoed attack may execute");
    }
}

#[test]
fn a_vetoed_unit_outside_a_siege_still_holds() {
    let (mut g, mut ai, plan, uid) = fixture();
    ai.disable_siege_train();
    let before = g.units[&uid].pos;
    ai.plan_battle(&mut g, 0, &plan);
    assert_eq!(g.units[&uid].pos, before);
    assert!(ai.battle_planner_ordered.contains(&uid));
}

#[test]
fn a_vetoed_siege_unit_with_no_safe_closing_step_still_holds() {
    let (mut g, mut ai, plan, uid) = fixture();
    let before = g.units[&uid].pos;
    let target = g.cities[&plan.target_city.unwrap()].pos;
    for pos in g.nbrs(before) {
        if g.wdist(pos, target) < g.wdist(before, target) {
            g.map.tiles.get_mut(&pos).unwrap().terrain = name!("mountain");
        }
    }
    ai.plan_battle(&mut g, 0, &plan);
    assert_eq!(g.units[&uid].pos, before);
    assert!(ai.battle_planner_ordered.contains(&uid));
}

#[test]
fn wounded_siege_units_remain_in_recovery() {
    let (mut g, mut ai, plan, uid) = fixture();
    g.units.get_mut(&uid).unwrap().hp = 40;
    ai.plan_battle(&mut g, 0, &plan);
    assert!(ai.battle_planner_recovering.contains(&uid));
    assert!(ai.battle_planner_ordered.contains(&uid));
}

#[test]
fn a_partially_wounded_vetoed_unit_holds_to_heal() {
    let (mut g, mut ai, plan, uid) = fixture();
    g.units.get_mut(&uid).unwrap().hp = RETURN_HP - 1;
    let before = g.units[&uid].pos;
    ai.plan_battle(&mut g, 0, &plan);
    assert_eq!(g.units[&uid].pos, before);
    assert!(ai.battle_planner_ordered.contains(&uid));
}

#[test]
fn a_safe_approach_does_not_capture_an_enemy_civilian() {
    let (mut g, mut ai, plan, uid) = fixture();
    let before = g.units[&uid].pos;
    let target = g.cities[&plan.target_city.unwrap()].pos;
    let only_step = (9, 12);
    for pos in g.nbrs(before) {
        if pos != only_step && g.wdist(pos, target) < g.wdist(before, target) {
            g.map.tiles.get_mut(&pos).unwrap().terrain = name!("mountain");
        }
    }
    let civilian = g.spawn_test_unit("builder", 1, only_step);
    let mut field = DangerField::with_reach(&g, 0, ai.strike_reach);
    assert!(g.can_move(uid, only_step));
    assert!(field.danger(only_step, uid) <= NO_DANGER);
    assert!(!ai.advance_vetoed_siege_unit(&mut g, 0, uid, &mut field));
    assert_eq!(g.units[&uid].pos, before);
    assert_eq!(g.units[&civilian].owner, 1);
}
