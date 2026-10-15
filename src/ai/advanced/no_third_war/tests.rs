//! `no-third-war`: unit tests.
use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

/// Four majors on open grassland: us at seat 0 with one city at (4, 8),
/// rivals 1-3 with a city each, all met, nobody at war, turn 120.
fn four_majors() -> (Game, AdvancedAi) {
    let mut g = Game::new_full(4, 40, 24, 581145, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    g.found_city_for(0, (4, 8), None);
    g.found_city_for(1, (14, 8), None);
    g.found_city_for(2, (24, 8), None);
    g.found_city_for(3, (34, 8), None);
    for rival in 1..4 {
        g.record_contact(0, rival);
    }
    g.at_war.clear();
    g.current = 0;
    g.turn = 120;
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    (g, ai)
}

#[test]
fn the_gene_is_opt_in() {
    opt_in_off_in_both_controllers("no-third-war", |ai| ai.no_third_war);
}

/// At war with two majors, the war on a third is held under the gene and
/// journalled once; off, nothing is held.
#[test]
fn a_third_war_is_held_beside_two() {
    let (mut g, mut ai) = four_majors();
    g.at_war.insert((0, 1));
    g.at_war.insert((0, 2));
    assert!(!ai.third_war_held(&g, 0, 3), "off: unchanged");

    ai.enable_no_third_war();
    assert!(ai.third_war_would_open(&g, 0, 3));
    assert!(ai.third_war_held(&g, 0, 3));
    assert_eq!(ai.no_third_war_noted.get(&3), Some(&(true, 120)));
    // Already at war with the target: nothing to open.
    assert!(!ai.third_war_held(&g, 0, 1));
}

/// One war running leaves the second open; the gene only counts majors at
/// war with us.
#[test]
fn a_second_war_is_not_held() {
    let (mut g, mut ai) = four_majors();
    ai.enable_no_third_war();
    g.at_war.insert((0, 1));
    assert!(!ai.third_war_would_open(&g, 0, 2));
    assert!(!ai.third_war_held(&g, 0, 2));
}

/// A rival whose clock is urgent (a Diplomatic Victory one point away) is
/// exempt, and the exemption is journalled as such.
#[test]
fn an_urgent_clock_exempts_the_third_war() {
    let (mut g, mut ai) = four_majors();
    ai.enable_no_third_war();
    ai.deny_leaders = true;
    g.at_war.insert((0, 1));
    g.at_war.insert((0, 2));
    g.players[3].dvp = 19;
    assert!(ai.urgent_victory_threat(&g, 3));
    assert!(ai.third_war_would_open(&g, 0, 3));
    assert_eq!(
        ai.third_war_exemption(&g, 0, 3),
        Some("its victory clock is urgent")
    );
    assert!(!ai.third_war_held(&g, 0, 3));
    assert_eq!(ai.no_third_war_noted.get(&3), Some(&(false, 120)));
}
