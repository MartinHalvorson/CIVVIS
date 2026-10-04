use super::*;

fn at(col: i32, row: i32) -> Pos {
    crate::hex::offset_to_axial(col, row)
}

fn field() -> Game {
    let mut g = crate::doctrine::build(
        crate::doctrine::position("the_reserve").expect("fixture"),
        3,
    )
    .expect("buildable");
    let units: Vec<_> = g.units.keys().copied().collect();
    for uid in units {
        g.remove_unit(uid);
    }
    for player in g.players.iter_mut() {
        player.civ = "Rome".to_string();
        player.government = None;
        player.policies.clear();
        player.techs.clear();
        player.civics.clear();
    }
    g.map.clear_rivers();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.road = 0;
        tile.owner_city = None;
    }
    g
}

fn closing_enemy(g: &mut Game) -> (u32, u32) {
    let ours = g.spawn_unit("warrior", 0, at(10, 6));
    let enemy = g.spawn_unit("warrior", 1, at(12, 6));
    assert_eq!(g.unit_max_moves(enemy), 2.0);
    assert_eq!(g.wdist(g.units[&enemy].pos, g.units[&ours].pos), 2);
    (ours, enemy)
}

fn walked(g: &Game, enemy: u32) -> Game {
    let mut after = g.speculative_clone();
    after.current = 1;
    after
        .apply(
            1,
            &Action::MoveTo {
                unit: enemy,
                to: at(11, 6),
            },
        )
        .expect("the closing stand is legal");
    assert_eq!(after.units[&enemy].pos, at(11, 6));
    assert_eq!(after.units[&enemy].moves_left, 1.0);
    after
}

#[test]
fn rough_tile_does_not_receive_a_melee_blow_the_executor_cannot_pay() {
    let mut g = field();
    let (ours, enemy) = closing_enemy(&mut g);
    g.map.tiles.get_mut(&at(10, 6)).unwrap().hills = true;
    let mut after = walked(&g, enemy);
    assert_eq!(after.step_cost_for(enemy, at(11, 6), at(10, 6)), 2.0);
    assert!(!after.can_pay_melee_entry(enemy, at(10, 6)));
    assert_eq!(
        after.apply(
            1,
            &Action::Attack {
                unit: enemy,
                target: at(10, 6)
            }
        ),
        Err("not enough movement to attack".to_string())
    );
    assert!(!strike_reach_of(&mut g, 0, enemy).contains(&at(10, 6)));
    assert_eq!(strike_danger(&g, 0, at(10, 6), ours), 0.0);
}

#[test]
fn river_entry_must_fit_after_the_approach() {
    let mut g = field();
    let (ours, enemy) = closing_enemy(&mut g);
    assert!(g.map.set_river_edge(at(11, 6), at(10, 6), true));
    let mut after = walked(&g, enemy);
    assert!(after.step_cost_for(enemy, at(11, 6), at(10, 6)) > 1.0);
    assert!(!after.can_pay_melee_entry(enemy, at(10, 6)));
    assert!(after
        .apply(
            1,
            &Action::Attack {
                unit: enemy,
                target: at(10, 6)
            }
        )
        .is_err());
    assert!(!strike_reach_of(&mut g, 0, enemy).contains(&at(10, 6)));
    assert_eq!(strike_danger(&g, 0, at(10, 6), ours), 0.0);
}

#[test]
fn a_flat_closing_blow_remains_a_real_threat() {
    let mut g = field();
    let (ours, enemy) = closing_enemy(&mut g);
    let mut after = walked(&g, enemy);
    assert!(after.can_pay_melee_entry(enemy, at(10, 6)));
    after
        .apply(
            1,
            &Action::Attack {
                unit: enemy,
                target: at(10, 6),
            },
        )
        .expect("the flat closing blow lands");
    assert!(strike_reach_of(&mut g, 0, enemy).contains(&at(10, 6)));
    assert!(strike_danger(&g, 0, at(10, 6), ours) > 0.0);
}

#[test]
fn a_full_movement_adjacent_blow_keeps_the_expensive_entry_exception() {
    let mut g = field();
    let ours = g.spawn_unit("warrior", 0, at(10, 6));
    let enemy = g.spawn_unit("warrior", 1, at(11, 6));
    let tile = g.map.tiles.get_mut(&at(10, 6)).unwrap();
    tile.hills = true;
    tile.feature = Some(crate::name!("forest"));
    assert!(g.step_cost_for(enemy, at(11, 6), at(10, 6)) > g.unit_max_moves(enemy));
    assert!(g.can_pay_melee_entry(enemy, at(10, 6)));
    assert!(strike_reach_of(&mut g, 0, enemy).contains(&at(10, 6)));
    assert!(strike_danger(&g, 0, at(10, 6), ours) > 0.0);
}

#[test]
fn ranged_reach_does_not_pay_melee_terrain_entry() {
    let mut g = field();
    let ours = g.spawn_unit("warrior", 0, at(10, 6));
    let enemy = g.spawn_unit("archer", 1, at(12, 6));
    g.map.tiles.get_mut(&at(10, 6)).unwrap().hills = true;
    assert!(strike_reach_of(&mut g, 0, enemy).contains(&at(10, 6)));
    assert!(strike_danger(&g, 0, at(10, 6), ours) > 0.0);
}

#[test]
fn a_reach_query_restores_unit_state_spatial_indexes_and_action_log() {
    let mut g = field();
    let (_, enemy) = closing_enemy(&mut g);
    let unit = g.units.get_mut(&enemy).unwrap();
    unit.moves_left = 0.5;
    unit.moved = true;
    unit.acted = true;
    unit.zoc_stopped = true;
    unit.started_turn_in_zoc = true;
    let before = serde_json::to_value(g.units.values().collect::<Vec<_>>()).unwrap();
    let positions: Vec<_> = g.map.tiles.keys().copied().collect();
    let index: Vec<_> = positions
        .iter()
        .map(|pos| g.unit_ids_at(*pos).to_vec())
        .collect();
    let log = g.log.len();
    let _ = strike_reach_of(&mut g, 0, enemy);
    assert_eq!(
        serde_json::to_value(g.units.values().collect::<Vec<_>>()).unwrap(),
        before
    );
    assert_eq!(g.log.len(), log);
    for (pos, ids) in positions.iter().zip(index) {
        assert_eq!(g.unit_ids_at(*pos), ids);
    }
}

#[test]
#[ignore = "read-only native fixture supplied by CIVVIS_BREACH_PREFIX"]
fn inspect_native_breach_approach_cost() {
    let path = std::env::var("CIVVIS_BREACH_PREFIX").expect("frozen native prefix");
    let path = std::path::Path::new(&path);
    let snapshot = crate::mirror::snapshot_from_events(path).unwrap();
    let state = crate::mirror::state_from_events(path, Some(190)).unwrap();
    assert_eq!(state.frame, 2);
    let live = crate::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 6);
    let mut g = live.game;
    let ours = live.uid_of[&9568265];
    let enemy = live.foreign_uid_of[&14483458];
    let target = at(30, 17);
    g.apply(
        0,
        &Action::MoveTo {
            unit: ours,
            to: target,
        },
    )
    .unwrap();
    let flood = g.attack_reach(enemy).contains(&target);
    let strike = strike_reach_of(&mut g, 0, enemy).contains(&target);
    let read = strike_danger(&g, 0, target, ours);
    eprintln!("NATIVE_BREACH_APPROACH flood={flood} strike={strike} danger={read}");
}
