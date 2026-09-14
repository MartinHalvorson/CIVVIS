use super::*;
use crate::Pos;

fn board() -> (Game, AdvancedAi, u32, Pos, u32) {
    let mut g = Game::new_full(2, 24, 16, 7925, 250, 1, true);
    g.current = 0;
    g.clear_mirror_cities();
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.river_edges = [true; 6];
    }
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    let capital = g.spawn_test_unit("settler", 0, (3, 6));
    g.apply(0, &Action::FoundCity { unit: capital }).unwrap();
    let here = (8, 6);
    let uid = g.spawn_test_unit("settler", 0, here);
    let raider = g.spawn_test_unit("warrior", g.barb_pid.unwrap(), (9, 6));
    let mut ai = AdvancedAi::new();
    ai.enable_live_settler_capture_lessons();
    ai.settlement_safety = true;
    ai.settler_targets.insert(uid, (10, 7));
    (g, ai, uid, here, raider)
}

#[test]
fn worthwhile_city_short_of_target_is_founded_before_retreat() {
    let (mut g, mut ai, uid, here, raider) = board();
    assert!(g.can_found_city(uid));
    assert!(ai.advanced_settler_step(&mut g, 0, uid));
    assert!(
        g.city_at(here).is_some(),
        "city replaces the exposed settler"
    );
    assert!(!g.units.contains_key(&uid));
    assert!(!ai.settler_targets.contains_key(&uid));
    let city = g.city_at(here).unwrap();
    let barb = g.barb_pid.unwrap();
    g.current = barb;
    g.apply(
        barb,
        &Action::Attack {
            unit: raider,
            target: here,
        },
    )
    .unwrap();
    assert_eq!(
        g.cities[&city].owner, 0,
        "the new city survives the nearby warrior"
    );
    assert!(g.cities[&city].hp > 0);
}

#[test]
fn wasteland_is_not_founded_for_shelter() {
    let (mut g, mut ai, uid, here, _) = board();
    for pos in g.wdisk(here, 2) {
        if let Some(tile) = g.map.tiles.get_mut(&pos) {
            tile.terrain = crate::name!("snow");
            tile.river_edges = [false; 6];
        }
    }
    assert!(!ai.settler_founds_for_shelter(&mut g, 0, uid));
    assert!(g.units.contains_key(&uid));
    assert!(g.city_at(here).is_none());
}

#[test]
fn overwhelming_army_keeps_retreat_available() {
    let (mut g, mut ai, uid, here, raider) = board();
    g.remove_unit(raider);
    for pos in [(9, 6), (8, 7), (7, 6)] {
        g.spawn_test_unit("modern_armor", g.barb_pid.unwrap(), pos);
    }
    assert!(!ai.settler_founds_for_shelter(&mut g, 0, uid));
    assert!(g.units.contains_key(&uid));
    assert!(g.city_at(here).is_none());
}

#[test]
fn quiet_illegal_and_native_sites_do_not_take_emergency_action() {
    let (mut g, mut ai, uid, _, raider) = board();
    ai.live_settler_capture_lessons = false;
    assert!(!ai.settler_founds_for_shelter(&mut g, 0, uid));
    ai.live_settler_capture_lessons = true;
    g.units.get_mut(&uid).unwrap().moves_left = 0.0;
    assert!(!ai.settler_founds_for_shelter(&mut g, 0, uid));
    g.units.get_mut(&uid).unwrap().moves_left = 2.0;
    g.remove_unit(raider);
    assert!(!ai.settler_founds_for_shelter(&mut g, 0, uid));
}

#[test]
fn rejected_site_is_not_revived_by_the_emergency() {
    let (mut g, mut ai, uid, here, _) = board();
    ai.settler_dead_sites
        .entry(uid)
        .or_default()
        .insert(here, g.turn + 10);
    assert!(!ai.settler_founds_for_shelter(&mut g, 0, uid));
}

#[test]
#[ignore = "manual live-controller soak"]
fn live_controller_shelter_soak() {
    for seed in 7900..7904 {
        let mut g = Game::new_full(2, 24, 16, seed, 120, 0, true);
        let mut ais = AdvancedAi::fleet(&g);
        for ai in &mut ais {
            ai.enable_live_bridge_universe();
        }
        crate::ai::run_game(&mut g, &mut ais);
        assert!(
            g.turn >= 120 || g.winner.is_some(),
            "game reaches a terminal outcome"
        );
    }
}
