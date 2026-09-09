use super::*;
use crate::game::Action;

fn corridor() -> (Game, AdvancedAi, u32, Pos, Pos) {
    let mut g = Game::new_full(2, 24, 16, 7925, 250, 1, false);
    g.current = 0;
    g.clear_mirror_cities();
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("mountain");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.cliff_edges = [false; 6];
        tile.river_edges = [false; 6];
    }
    let here = (6, 6);
    let blocked = (7, 6);
    let target = (8, 6);
    for pos in [
        here,
        blocked,
        target,
        (5, 7),
        (5, 8),
        (6, 8),
        (7, 8),
        (8, 7),
    ] {
        g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("grassland");
    }
    let uid = g.spawn_test_unit("settler", 0, here);
    let mut ai = AdvancedAi::new();
    ai.enable_live_move_refusal_break();
    ai.settlement_safety = true;
    ai.settler_targets.insert(uid, target);
    ai.base
        .move_refusal_blocks
        .insert(uid, (blocked, g.turn + 20));
    (g, ai, uid, blocked, target)
}

#[test]
fn refused_approach_keeps_the_site_and_founds_after_a_backward_detour() {
    let (mut g, mut ai, uid, blocked, target) = corridor();
    let initial = g.wdist(g.units[&uid].pos, target);
    assert_eq!(g.route_step(uid, target, 0), Some(blocked));
    for _ in 0..3 {
        ai.retire_frozen_settler_targets(&g);
        assert_eq!(ai.settler_targets.get(&uid), Some(&target));
    }
    assert!(ai.settler_step_toward_safe(&mut g, 0, uid, target));
    assert!(g.wdist(g.units[&uid].pos, target) > initial);
    for _ in 0..10 {
        if g.units[&uid].pos == target {
            break;
        }
        g.turn += 1;
        g.units.get_mut(&uid).unwrap().moves_left = 2.0;
        ai.retire_frozen_settler_targets(&g);
        assert_eq!(ai.settler_targets.get(&uid), Some(&target));
        assert!(ai.settler_step_toward_safe(&mut g, 0, uid, target));
        assert_ne!(g.units[&uid].pos, blocked);
    }
    assert_eq!(g.units[&uid].pos, target);
    g.units.get_mut(&uid).unwrap().moves_left = 2.0;
    g.apply(0, &Action::FoundCity { unit: uid }).unwrap();
    assert!(g.city_at(target).is_some());
}

#[test]
fn genuinely_cut_off_site_returns_when_the_refusal_expires() {
    let (g, mut ai, uid, blocked, _) = corridor();
    ai.settler_targets.insert(uid, blocked);
    ai.retire_frozen_settler_targets(&g);
    assert!(!ai.settler_targets.contains_key(&uid));
    assert_eq!(
        ai.settler_dead_sites[&uid][&blocked],
        ai.base.move_refusal_blocks[&uid].1
    );
}

#[test]
fn inactive_or_expired_refusal_keeps_ordinary_targeting() {
    let (mut g, mut ai, uid, _, target) = corridor();
    ai.live_move_refusal_break = false;
    assert_eq!(ai.settler_refusal_waypoint(&g, uid, target), Some(target));
    ai.live_move_refusal_break = true;
    g.turn = ai.base.move_refusal_blocks[&uid].1;
    assert_eq!(ai.settler_refusal_waypoint(&g, uid, target), Some(target));
}

#[test]
fn refused_route_detour_does_not_walk_into_barbarian_capture() {
    let (mut g, mut ai, uid, _, target) = corridor();
    let barb = g
        .players
        .iter()
        .position(|player| player.is_barbarian)
        .unwrap();
    g.spawn_test_unit("warrior", barb, (5, 8));
    let here = g.units[&uid].pos;
    assert_eq!(ai.settler_refusal_waypoint(&g, uid, target), Some((5, 7)));
    assert!(!ai.settler_step_out_of_reach(&mut g, 0, uid, target));
    assert_eq!(g.units[&uid].pos, here);
}

#[test]
#[ignore = "requires CIVVIS_SETTLER_REPLAY events from civvis-20260909T013436Z"]
fn replay_turn48_refused_settler_route() {
    let path = std::path::PathBuf::from(std::env::var("CIVVIS_SETTLER_REPLAY").unwrap());
    let snapshot = crate::mirror::snapshot_from_events_at(&path, Some(48)).unwrap();
    let state = crate::mirror::state_from_events(&path, Some(48)).unwrap();
    let mut mirror = crate::mirror::LiveMirror::new(&snapshot, &state, 6, 1, 250, 6);
    let uid = mirror.uid_of[&720902];
    let g = &mut mirror.game;
    let mut ai = AdvancedAi::new();
    ai.enable_live_move_refusal_break();
    ai.settlement_safety = true;
    let target = (-4, 25);
    let blocked = (-5, 26);
    let here = g.units[&uid].pos;
    assert_eq!(here, (-6, 26));
    ai.base
        .move_refusal_blocks
        .insert(uid, (blocked, g.turn + 4));
    ai.settler_targets.insert(uid, target);
    assert!(!ai.settlement_unit_step_toward_safe(g, 0, uid, Some(uid), false, target));
    ai.retire_frozen_settler_targets(g);
    assert_eq!(ai.settler_targets.get(&uid), Some(&target));
    let waypoint = ai.settler_refusal_waypoint(g, uid, target);
    eprintln!("turn48 from={here:?}, target={target:?}, refused={blocked:?}, detour={waypoint:?}");
    assert!(ai.settler_step_toward_safe(g, 0, uid, target));
    assert_ne!(g.units[&uid].pos, here);
    assert_ne!(g.units[&uid].pos, blocked);
    eprintln!("turn48 moved to {:?}", g.units[&uid].pos);
}
