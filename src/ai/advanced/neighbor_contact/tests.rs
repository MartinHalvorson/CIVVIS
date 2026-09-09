use super::*;
use crate::name;

fn opening() -> (Game, u32, u32) {
    let mut g = Game::new_full(2, 28, 18, 91_090_001, 250, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    let pos = g.units[&settler].pos;
    g.found_city_for(0, pos, None);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.map.clear_rivers();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    let a = g.spawn_test_unit("scout", 0, pos);
    let b = g.spawn_test_unit("scout", 0, pos);
    g.players[0].explored.clear();
    let visible = g.wdisk(pos, 2);
    g.players[0].explored.extend(visible);
    (g, a, b)
}

#[test]
fn colocated_scouts_choose_different_frontiers_and_can_walk_them() {
    let (mut g, a, b) = opening();
    let mut ai = AdvancedAi::new();
    assert!(ai.neighbor_contact_targets(&g, 0, a).is_empty());
    ai.enable_early_contact_window_2();
    let first = ai.neighbor_contact_targets(&g, 0, a);
    let second = ai.neighbor_contact_targets(&g, 0, b);
    assert!(!first.is_empty() && !second.is_empty());
    assert_ne!(
        first[0], second[0],
        "recon must not choose the same direction from the same city"
    );
    let before = g.units[&a].pos;
    assert!(ai.neighbor_contact_step(&mut g, 0, a));
    assert_ne!(g.units[&a].pos, before);
}

#[test]
fn contact_closed_borders_or_a_settler_guard_ends_the_search() {
    let (mut g, a, _) = opening();
    let mut ai = AdvancedAi::new();
    ai.enable_early_contact_window_2();
    assert!(!ai.neighbor_contact_targets(&g, 0, a).is_empty());
    ai.settler_guards.insert(999, a);
    assert!(ai.neighbor_contact_targets(&g, 0, a).is_empty());
    ai.settler_guards.clear();
    g.players[0].civics.insert(name!("early_empire"));
    assert!(ai.neighbor_contact_targets(&g, 0, a).is_empty());
    g.players[0].civics.remove(&name!("early_empire"));
    g.record_contact(0, 1);
    assert!(ai.neighbor_contact_targets(&g, 0, a).is_empty());
}

#[test]
fn routing_version_preserves_the_original_scout_production_value() {
    let (mut g, _, _) = opening();
    g.players[1].is_minor = true;
    let mut ai = AdvancedAi::new();
    let counts = ai.counts(&g, 0);
    ai.enable_early_contact_window();
    let original = ai.early_contact_value(&g, 0, &counts);
    assert!(original > 0.0);
    ai.enable_early_contact_window_2();
    assert_eq!(ai.early_contact_value(&g, 0, &counts), original);
}

#[test]
fn neighbor_contact_is_an_independent_opt_in_version() {
    super::super::test_support::opt_in_off_in_both_controllers("early-contact-window-2", |ai| {
        ai.early_contact_window_2
    });
    let mut ai = AdvancedAi::new();
    ai.enable_early_contact_window_2();
    ai.disable_early_contact_window();
    assert!(ai.early_contact_window_2);
    ai.enable_early_contact_window();
    assert!(!ai.early_contact_window_2);
}
