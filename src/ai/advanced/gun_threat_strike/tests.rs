use super::super::*;
use super::GUN_THREAT_KILL_BONUS;

/// A grassland field, our trebuchet at (16, 10), a Line Infantry of the
/// enemy's at (20, 10) seen by a warrior of ours behind it, at war: the
/// `guns-stay-out-of-reach` fixture, where (18, 10) is lethal to the gun.
fn fixture() -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new_full(2, 40, 24, 377300, 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (6, 10), None);
    g.current = 0;
    g.at_war.insert((0, 1));
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    let gun = g.spawn_test_unit("trebuchet", 0, (16, 10));
    let hostile = g.spawn_test_unit("line_infantry", 1, (20, 10));
    g.spawn_test_unit("warrior", 0, (22, 10));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.base.recorded_tactical_step = true;
    ai.enable_guns_stay_out_of_reach();
    (g, ai, gun, hostile)
}

#[test]
fn off_the_pinning_hostile_is_priced_as_before() {
    let (g, mut ai, _, hostile) = fixture();
    ai.mark_gun_lethal_tiles(&g, 0);
    assert!(ai.gun_threat_sources.is_empty());
    assert_eq!(ai.strike_priority_bonus(hostile), 0.0);
}

#[test]
fn the_hostile_that_pins_a_gun_is_the_priority_kill() {
    let (g, mut ai, gun, hostile) = fixture();
    ai.enable_gun_threat_draws_the_strike();
    ai.mark_gun_lethal_tiles(&g, 0);
    assert!(
        ai.base.gun_lethal_tiles.contains_key(&gun),
        "the veto still holds"
    );
    assert!(ai.gun_threat_sources.contains(&hostile));
    assert_eq!(ai.strike_priority_bonus(hostile), GUN_THREAT_KILL_BONUS);
    let warrior = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "warrior")
        .unwrap();
    assert_eq!(ai.strike_priority_bonus(warrior), 0.0);
}

#[test]
fn a_hostile_that_cannot_destroy_the_gun_is_no_threat() {
    let (mut g, mut ai, _, hostile) = fixture();
    ai.enable_gun_threat_draws_the_strike();
    g.remove_unit(hostile);
    let weak = g.spawn_test_unit("warrior", 1, (20, 10));
    ai.mark_gun_lethal_tiles(&g, 0);
    assert!(!ai.gun_threat_sources.contains(&weak));
    assert_eq!(ai.strike_priority_bonus(weak), 0.0);
}

#[test]
fn the_veto_gene_off_marks_no_threat() {
    let (g, _, _, hostile) = fixture();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.base.recorded_tactical_step = true;
    ai.enable_gun_threat_draws_the_strike();
    ai.mark_gun_lethal_tiles(&g, 0);
    assert!(ai.gun_threat_sources.is_empty());
    assert_eq!(ai.strike_priority_bonus(hostile), 0.0);
}
