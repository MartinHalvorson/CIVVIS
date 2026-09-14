use super::*;
use std::sync::Arc;

fn board() -> (Game, AdvancedAi, Pos, u32) {
    let mut g = Game::new(3, 24, 16, 914_357_900, 250, 0);
    g.units.clear();
    g.cities.clear();
    g.at_war.clear();
    g.at_war.insert((0, 1));
    g.turn = 21;
    let center = (10, 8);
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    g.host_observed = Arc::new([center].into_iter().collect());
    let target = g.spawn_test_unit("warrior", 0, center);
    let mut ai = AdvancedAi::new();
    ai.enable_hostile_memory_3();
    ai.hostile_last_seen.insert(
        999_999,
        super::super::RememberedHostile {
            pos: center,
            when: 20,
            owner: 1,
            kind: crate::name!("crossbowman"),
        },
    );
    (g, ai, center, target)
}

fn observed_identity(g: &mut Game, owner: usize, pos: Pos) -> u32 {
    let uid = g.spawn_test_unit("crossbowman", owner, pos);
    Arc::make_mut(&mut g.host_unit_facts)
        .entry(uid)
        .or_default()
        .civ6_id = Some(999_999);
    uid
}

#[test]
fn hostile_identity_v3_stops_pricing_a_visible_converted_or_transferred_gun() {
    for owner in [0, 2] {
        let (mut g, ai, center, target) = board();
        observed_identity(&mut g, owner, center);
        let mut old = ai.clone();
        old.enable_hostile_memory_2();
        assert!(
            old.remembered_ranged_reach(&g, 0)
                .strongest_nominal_shot(&g, target, center)
                > 0.0
        );
        let forecast = ai.remembered_ranged_reach(&g, 0);
        assert_eq!(forecast.strongest_nominal_shot(&g, target, center), 0.0);
        assert!(forecast.margin(&g, center) > 0);
        assert!(ai.hostile_last_seen.contains_key(&999_999));
    }
}

#[test]
fn hostile_identity_v3_hidden_transfer_does_not_reveal_the_new_owner() {
    let (mut g, ai, center, target) = board();
    let uid = observed_identity(&mut g, 2, (20, 12));
    assert!(!g.sees(&g.player_vision_frame(0), g.units[&uid].pos));
    let native = ai.remembered_ranged_reach(&g, 0);
    assert!(native.strongest_nominal_shot(&g, target, center) > 0.0);
    let mut exported = g.clone();
    exported.remove_unit(uid);
    let absent = ai.remembered_ranged_reach(&exported, 0);
    assert_eq!(native.margin(&g, center), absent.margin(&exported, center));
    assert_eq!(
        native.strongest_nominal_shot(&g, target, center),
        absent.strongest_nominal_shot(&exported, target, center)
    );
}

#[test]
fn hostile_identity_v3_missing_current_turn_gun_still_has_a_forecast() {
    let (g, mut ai, center, target) = board();
    ai.hostile_last_seen.get_mut(&999_999).unwrap().when = g.turn;
    assert!(
        ai.remembered_ranged_reach(&g, 0)
            .strongest_nominal_shot(&g, target, center)
            > 0.0
    );
}
