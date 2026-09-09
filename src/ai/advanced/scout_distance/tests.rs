use super::*;

fn board() -> (Game, AdvancedAi, u32, Pos) {
    let mut g = Game::new_full(2, 28, 18, 717009, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.improvement = None;
        tile.resource = None;
        tile.owner_city = None;
        tile.district = None;
        tile.river_edges = [false; 6];
    }
    g.current = 0;
    let home = crate::hex::offset_to_axial(17, 9);
    g.found_city_for(0, home, None);
    let scout = g.spawn_test_unit("scout", 0, g.nbrs(home)[0]);
    g.players[0].explored = g.wdisk(home, 4).into_iter().collect();
    (g, AdvancedAi::new(), scout, home)
}

fn plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 0,
        rush: false,
    }
}

#[test]
fn free_scout_leaves_home_and_continues_revealing_distant_ground() {
    let (mut g, mut ai, scout, home) = board();
    ai.chokepoint_garrison = true;
    let known = g.players[0].explored.len();
    for _ in 0..8 {
        g.turn += 1;
        let moves = g.unit_max_moves(scout);
        let unit = g.units.get_mut(&scout).unwrap();
        unit.moves_left = moves;
        unit.moved = false;
        unit.acted = false;
        unit.attacks_left = 1;
        assert!(ai.advanced_military_step_with_decline(&mut g, 0, scout, &plan(), true));
    }
    assert!(g.wdist(home, g.units[&scout].pos) >= 7);
    assert!(g.players[0].explored.len() > known + 15);
}

#[test]
fn routine_gate_goes_to_a_combat_unit_instead_of_the_scout() {
    let (mut g, mut ai, scout, _) = board();
    for row in 0..18 {
        if row != 9 {
            let at = crate::hex::offset_to_axial(14, row);
            g.map.tiles.get_mut(&at).unwrap().terrain = crate::name!("mountain");
        }
    }
    let gap = crate::hex::offset_to_axial(14, 9);
    g.units.get_mut(&scout).unwrap().pos = gap;
    let warrior = g.spawn_test_unit("warrior", 0, crate::hex::offset_to_axial(16, 9));
    g.found_city_for(1, crate::hex::offset_to_axial(8, 9), None);
    g.players[0].explored = g.map.tiles.keys().copied().collect();
    ai.chokepoint_garrison = true;
    ai.chokepoint_gate_plan(&g, 0);
    assert_eq!(ai.chokepoint_gates.post(scout), None);
    assert!(ai.chokepoint_gates.post(warrior).is_some());
}

#[test]
fn bound_guard_and_frozen_controller_keep_their_existing_roles() {
    let (mut g, mut ai, scout, home) = board();
    let settler = g.spawn_test_unit("settler", 0, home);
    ai.settler_guards.insert(settler, scout);
    assert_eq!(ai.distance_scout_step(&mut g, 0, scout), None);
    ai.settler_guards.clear();
    assert!(ai.distance_scout_available(&g, 0, scout));
    ai.base.explore_commit = false;
    assert_eq!(ai.distance_scout_step(&mut g, 0, scout), None);
}

#[test]
fn fully_charted_map_does_not_issue_exploration_orders() {
    let (mut g, mut ai, scout, _) = board();
    g.players[0].explored = g.map.tiles.keys().copied().collect();
    assert_eq!(ai.distance_scout_step(&mut g, 0, scout), None);
}

#[test]
fn a_wounded_scout_recovers_before_returning_to_exploration() {
    let (mut g, mut ai, scout, home) = board();
    g.units.get_mut(&scout).unwrap().pos = home;
    g.units.get_mut(&scout).unwrap().hp = 20;
    let known = g.players[0].explored.len();
    ai.advanced_military_step_with_decline(&mut g, 0, scout, &plan(), true);
    assert_eq!(g.units[&scout].pos, home);
    assert_eq!(g.players[0].explored.len(), known);
}
