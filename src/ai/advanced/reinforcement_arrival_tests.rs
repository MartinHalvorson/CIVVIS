use super::*;

fn fixture(distance: i32) -> (Game, AdvancedAi, StrategicPlan, u32, ForceGroup) {
    let mut g = Game::new_full(2, 52, 30, 374_400, 250, 0, false);
    g.clear_mirror_cities();
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    g.found_city_for(0, (4, 12), None);
    let target = g.found_city_for(1, (20, 12), None);
    g.at_war.insert((0, 1));
    g.current = 0;
    let uid = g.spawn_test_unit("bombard", 0, (20 - distance, 12));
    g.units.get_mut(&uid).unwrap().moves_left = 8.0;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_war_reinforcement();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    };
    let group = ForceGroup {
        id: 1,
        domain: ForceDomain::Land,
        units: vec![uid],
        anchor: (4, 12),
        objective: (20, 12),
        focus_target: None,
        posture: ForcePosture::Muster,
        readiness: 0.0,
        local_strength_ratio: 0.5,
    };
    (g, ai, plan, uid, group)
}

#[test]
fn an_arrived_bombard_keeps_its_moves_despite_a_rearward_group_anchor() {
    for distance in 2..=5 {
        let (mut g, mut ai, plan, uid, group) = fixture(distance);
        let before = (g.units[&uid].pos, g.units[&uid].moves_left);
        assert_eq!(
            ai.wartime_reinforcement_step(&mut g, 0, uid, &plan, Some(&group), &[1]),
            None
        );
        assert_eq!((g.units[&uid].pos, g.units[&uid].moves_left), before);
    }
}

#[test]
fn reinforcement_marches_to_the_ring_then_stops_without_reforming_its_group() {
    let (mut g, mut ai, plan, uid, group) = fixture(6);
    assert_eq!(
        ai.wartime_reinforcement_step(&mut g, 0, uid, &plan, Some(&group), &[1]),
        Some(true)
    );
    assert_eq!(g.wdist(g.units[&uid].pos, group.objective), 5);
    let arrived = (g.units[&uid].pos, g.units[&uid].moves_left);
    assert_eq!(
        ai.wartime_reinforcement_step(&mut g, 0, uid, &plan, Some(&group), &[1]),
        None
    );
    assert_eq!((g.units[&uid].pos, g.units[&uid].moves_left), arrived);
}

#[test]
fn an_ungrouped_arrival_also_passes_to_the_tactical_controller() {
    let (mut g, mut ai, plan, uid, _) = fixture(4);
    assert_eq!(
        ai.wartime_reinforcement_step(&mut g, 0, uid, &plan, None, &[1]),
        None
    );
}
