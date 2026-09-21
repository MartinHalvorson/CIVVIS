use super::*;

fn fixture() -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new_full(2, 28, 18, 369_000, 250, 0, false);
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
    g.at_war.insert((0, 1));
    g.current = 0;
    let scout = g.spawn_test_unit("scout", 0, (8, 8));
    let enemy = g.spawn_test_unit("spearman", 1, (10, 8));
    let unit = g.units.get_mut(&enemy).unwrap();
    unit.fortified = true;
    unit.fortify_turns = 2;
    g.players[0].explored = g.map.tiles.keys().copied().collect();
    let mut ai = AdvancedAi::new();
    ai.enable_battle_planner_2();
    ai.enable_doomed_blow_veto_2();
    ai.enable_strike_reach();
    (g, ai, scout, enemy)
}

#[test]
fn healthy_vetoed_scout_leaves_contact_without_attacking() {
    let (mut g, mut ai, scout, enemy) = fixture();
    let before = g.units[&scout].pos;
    let mut field = DangerField::with_reach(&g, 0, true);
    let here = field.danger(before, scout);
    assert!(
        here > NO_DANGER && here < 80.0,
        "healthy but near contact: {here}"
    );
    assert!(g
        .reachable(scout)
        .iter()
        .any(|pos| field.danger(*pos, scout) <= NO_DANGER));
    ai.rotate_wounded(
        &mut g,
        0,
        &mut field,
        &BTreeSet::new(),
        &BTreeSet::from([scout]),
    );
    assert_ne!(
        g.units[&scout].pos, before,
        "a vetoed attack must not pin a free scout"
    );
    assert!(field.danger(g.units[&scout].pos, scout) <= NO_DANGER);
    assert_eq!(g.units[&enemy].hp, 100);
    assert_eq!(g.units[&scout].hp, 100);
    assert!(
        ai.battle_planner_claims(scout),
        "the attack stays vetoed for this turn"
    );
    assert!(!ai.battle_planner_recovering.contains(&scout));
}

#[test]
fn reserved_or_frozen_recon_does_not_take_an_escape_assignment() {
    for reserved in [true, false] {
        let (mut g, mut ai, scout, _) = fixture();
        let before = g.units[&scout].pos;
        if reserved {
            let settler = g.spawn_test_unit("settler", 0, before);
            ai.settler_guards.insert(settler, scout);
        } else {
            ai.base.explore_commit = false;
        }
        let mut field = DangerField::with_reach(&g, 0, true);
        assert!(!ai.escape_vetoed_recon(&mut g, 0, scout, &mut field));
        assert_eq!(g.units[&scout].pos, before);
    }
}

#[test]
fn no_safe_escape_keeps_the_attack_veto() {
    let (mut g, mut ai, scout, enemy) = fixture();
    let before = g.units[&scout].pos;
    let mut field = DangerField::with_reach(&g, 0, true);
    let safe = g
        .reachable(scout)
        .into_iter()
        .filter(|pos| field.danger(*pos, scout) <= NO_DANGER)
        .collect::<Vec<_>>();
    for pos in safe {
        g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("mountain");
    }
    let mut field = DangerField::with_reach(&g, 0, true);
    ai.rotate_wounded(
        &mut g,
        0,
        &mut field,
        &BTreeSet::new(),
        &BTreeSet::from([scout]),
    );
    assert_eq!(g.units[&scout].pos, before);
    assert_eq!(g.units[&enemy].hp, 100);
    assert!(ai.battle_planner_claims(scout));
}

#[test]
fn a_combat_unit_does_not_take_the_recon_escape() {
    let (mut g, mut ai, scout, _) = fixture();
    let before = g.units[&scout].pos;
    g.remove_unit(scout);
    let warrior = g.spawn_test_unit("warrior", 0, before);
    let mut field = DangerField::with_reach(&g, 0, true);
    assert!(!ai.escape_vetoed_recon(&mut g, 0, warrior, &mut field));
    assert_eq!(g.units[&warrior].pos, before);
}

#[test]
fn escaped_scout_resumes_exploration_on_later_turns() {
    let (mut g, mut ai, scout, enemy) = fixture();
    let origin = g.units[&scout].pos;
    g.players[0].explored = g.wdisk(origin, 4).into_iter().collect();
    let known = g.players[0].explored.len();
    let plan = StrategicPlan {
        strategy: super::super::GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 0,
        rush: false,
    };
    let mut positions = Vec::new();
    for _ in 0..8 {
        g.turn += 1;
        let moves = g.unit_max_moves(scout);
        let unit = g.units.get_mut(&scout).unwrap();
        unit.moves_left = moves;
        unit.attacks_left = 1;
        unit.moved = false;
        unit.acted = false;
        ai.plan_battle(&mut g, 0, &plan);
        for _ in 0..8 {
            if ai.battle_planner_claims(scout) || g.units[&scout].moves_left <= 0.0 {
                break;
            }
            if ai.distance_scout_step(&mut g, 0, scout) != Some(true) {
                break;
            }
        }
        positions.push(g.units[&scout].pos);
    }
    assert!(
        g.players[0].explored.len() > known + 10,
        "exploration did not resume: {positions:?}"
    );
    assert!(
        g.wdist(origin, g.units[&scout].pos) >= 5,
        "scout remains near the contact: {positions:?}"
    );
    assert_eq!(g.units[&enemy].hp, 100);
}
