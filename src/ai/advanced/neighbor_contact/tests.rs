use super::*;
use crate::name;

fn opening() -> Game {
    let mut g = Game::new_full(2, 28, 18, 91_090_001, 250, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    let pos = g.units[&settler].pos;
    g.found_city_for(0, pos, None);
    g.remove_unit(settler);
    g
}

#[test]
fn first_major_contact_gets_a_scout_incentive_without_city_states() {
    let g = opening();
    let mut ai = AdvancedAi::new();
    let counts = ai.counts(&g, 0);
    ai.enable_early_contact_window();
    assert_eq!(ai.early_contact_value(&g, 0, &counts), 0.0);
    ai.enable_early_contact_window_2();
    assert!(ai.early_contact_value(&g, 0, &counts) > 0.0);
    assert!(!ai.early_contact_window);
}

#[test]
fn contact_or_two_scouts_ends_the_major_incentive() {
    let mut g = opening();
    let ai = AdvancedAi::new();
    assert!(ai.neighbor_contact_value(&g, 0, 1) > 0.0);
    assert_eq!(ai.neighbor_contact_value(&g, 0, 2), 0.0);
    g.record_contact(0, 1);
    assert_eq!(ai.neighbor_contact_value(&g, 0, 1), 0.0);
}

#[test]
fn explored_neighborhood_and_closed_window_do_not_buy_more_scouts() {
    let mut g = opening();
    let mut ai = AdvancedAi::new();
    ai.enable_early_contact_window_2();
    let counts = ai.counts(&g, 0);
    g.players[0].civics.insert(name!("early_empire"));
    assert_eq!(ai.early_contact_value(&g, 0, &counts), 0.0);
    for pos in g.map.tiles.keys().copied().collect::<Vec<_>>() {
        g.players[0].explored.insert(pos);
    }
    assert_eq!(ai.neighbor_contact_value(&g, 0, 1), 0.0);
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
    ai.disable_early_contact_window_2();
    assert!(ai.early_contact_window);
}

#[test]
fn the_original_city_state_incentive_survives_in_version_two() {
    let mut g = opening();
    g.players[1].is_minor = true;
    let mut ai = AdvancedAi::new();
    let counts = ai.counts(&g, 0);
    ai.enable_early_contact_window();
    let original = ai.early_contact_value(&g, 0, &counts);
    assert!(original > 0.0);
    ai.enable_early_contact_window_2();
    assert_eq!(ai.early_contact_value(&g, 0, &counts), original);
}
