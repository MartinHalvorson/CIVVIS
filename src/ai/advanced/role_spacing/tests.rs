use super::super::{AdvancedAi, ForceDomain, ForceGroup, ForcePosture};
use crate::game::Game;
use crate::name;

fn clear_approach() -> (Game, u32, ForceGroup) {
    let mut g = Game::new_full(2, 36, 22, 379140, 1_000, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    for player in g.players.iter_mut() {
        player.explored.extend(g.map.tiles.keys().copied());
    }
    g.at_war.clear();
    g.turn = 60;
    g.current = 0;
    let objective = (15, 8);
    let uid = g.spawn_test_unit("archer", 0, (9, 12));
    let group = ForceGroup {
        id: 1,
        domain: ForceDomain::Land,
        units: vec![uid],
        anchor: g.units[&uid].pos,
        objective,
        focus_target: None,
        posture: ForcePosture::Advance,
        readiness: 1.0,
        local_strength_ratio: 1.0,
    };
    assert_eq!(g.wdist(g.units[&uid].pos, objective), 6);
    (g, uid, group)
}

#[test]
fn an_ordinary_advancing_archer_crosses_the_clear_six_to_five_approach() {
    let (mut g, uid, group) = clear_approach();
    let mut ai = AdvancedAi::new();
    ai.enable_role_spacing_continuity();
    let next = g.route_step(uid, group.objective, 2).unwrap();
    assert_eq!(ai.base.projected_counter_damage(&g, uid, next, &[]), 0.0);
    assert!(ai.coordinated_tactical_step(&mut g, 0, uid, &group, &[1], false));
    assert!(
        g.wdist(g.units[&uid].pos, group.objective) < 6,
        "an Advance order must actually close through this clear corridor"
    );
}

#[test]
fn the_approach_experiment_is_registered_and_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers("role-spacing-continuity", |ai| {
        ai.role_spacing_continuity
    });
}

#[test]
fn off_and_legacy_keep_the_historical_six_hex_stand() {
    for mut ai in [AdvancedAi::new(), AdvancedAi::legacy()] {
        let (mut g, uid, group) = clear_approach();
        if ai.base.legacy_movement {
            ai.enable_role_spacing_continuity();
        }
        assert!(ai.coordinated_tactical_step(&mut g, 0, uid, &group, &[1], false));
        assert!(g.wdist(g.units[&uid].pos, group.objective) >= 6);
    }
}

#[test]
fn hold_and_recovery_keep_their_full_orders_and_unit_state() {
    for posture in [ForcePosture::Hold, ForcePosture::Recover] {
        let (mut g, uid, mut group) = clear_approach();
        group.posture = posture;
        group.anchor = group.objective;
        g.units.get_mut(&uid).unwrap().hp = 30;
        let mut off_game = g.clone();
        let off = AdvancedAi::new();
        let mut on = off.clone();
        on.enable_role_spacing_continuity();
        assert_eq!(
            on.coordinated_tactical_step(&mut g, 0, uid, &group, &[1], false),
            off.coordinated_tactical_step(&mut off_game, 0, uid, &group, &[1], false)
        );
        assert_eq!(g.log, off_game.log);
        assert!(g.units[&uid] == off_game.units[&uid]);
    }
}

#[test]
fn already_inside_the_ring_the_full_advance_orders_stay_identical() {
    let (mut g, uid, group) = clear_approach();
    g.relocate(uid, (11, 12));
    assert_eq!(g.wdist(g.units[&uid].pos, group.objective), 4);
    let mut off_game = g.clone();
    let off = AdvancedAi::new();
    let mut on = off.clone();
    on.enable_role_spacing_continuity();
    assert_eq!(
        on.coordinated_tactical_step(&mut g, 0, uid, &group, &[1], false),
        off.coordinated_tactical_step(&mut off_game, 0, uid, &group, &[1], false)
    );
    assert_eq!(g.log, off_game.log);
    assert!(g.units[&uid] == off_game.units[&uid]);
}
