//! `campaign-second-front-waits-for-the-staging`: unit tests.
use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use std::collections::VecDeque;

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

fn front_on(target: usize, turn: u32) -> one_war::OneWarFront {
    one_war::OneWarFront {
        target,
        since: turn,
        ledger: (0, 0, 0, 0),
        window: VecDeque::new(),
        tide_against_since: None,
        city_health: BTreeMap::new(),
        sieges_advancing: 0,
        closure_wanted_since: None,
    }
}

#[test]
fn the_gene_is_opt_in() {
    opt_in_off_in_both_controllers("campaign-second-front-waits-for-the-staging", |ai| {
        ai.campaign_second_front_waits_for_the_staging
    });
}

/// The second front waits while the first front has four or fewer units
/// staged and no city was taken; five staged, a recent capture, an urgent
/// clock or a rival already named lift the wait. Off: never waits.
#[test]
fn the_second_front_waits_for_the_staging() {
    let (mut g, mut ai) = four_majors();
    ai.enable_one_war_at_a_time();
    g.at_war.insert((0, 1));
    ai.one_war = Some(front_on(1, 100));
    assert_eq!(ai.campaign_second_front_wait(&g, 0, 2), None, "off: unchanged");

    ai.enable_campaign_second_front_waits_for_the_staging();
    let reason = ai.campaign_second_front_wait(&g, 0, 2).expect("waits");
    assert!(reason.contains("0 unit(s) staged"), "{reason}");

    // Four staged still waits; the fifth opens it.
    for x in 10..14 {
        g.spawn_test_unit("warrior", 0, (x, 9));
    }
    assert_eq!(ai.front_units_staged(&g, 0, 1), 4);
    assert!(ai.campaign_second_front_wait(&g, 0, 2).is_some());
    g.spawn_test_unit("warrior", 0, (11, 7));
    assert_eq!(ai.front_units_staged(&g, 0, 1), 5);
    assert_eq!(ai.campaign_second_front_wait(&g, 0, 2), None);
}

#[test]
fn a_recent_capture_or_an_urgent_clock_opens_the_second_front() {
    let (mut g, mut ai) = four_majors();
    ai.enable_one_war_at_a_time();
    ai.enable_campaign_second_front_waits_for_the_staging();
    g.at_war.insert((0, 1));
    ai.one_war = Some(front_on(1, 100));
    assert!(ai.campaign_second_front_wait(&g, 0, 2).is_some());

    // A city taken eight turns ago lifts the wait; one taken 20 ago does not.
    ai.second_front_captures.insert(77, 112);
    assert_eq!(ai.campaign_second_front_wait(&g, 0, 2), None);
    ai.second_front_captures.insert(77, 100);
    assert!(ai.campaign_second_front_wait(&g, 0, 2).is_some());

    // An urgent clock on the new target.
    ai.deny_leaders = true;
    g.players[2].dvp = 19;
    assert_eq!(ai.campaign_second_front_wait(&g, 0, 2), None);
    g.players[2].dvp = 0;

    // The rival named at the last observation keeps its name.
    ai.one_war_second = Some(2);
    assert_eq!(ai.campaign_second_front_wait(&g, 0, 2), None);
}

/// The observation records a city taken from another owner and forgets it
/// when it is lost; nothing is recorded with the gene off.
#[test]
fn the_observation_records_our_captures() {
    let (mut g, mut ai) = four_majors();
    let taken = g.player_city_ids(1)[0];
    g.cities.get_mut(&taken).unwrap().owner = 0;
    ai.observe_second_front_staging(&g, 0);
    assert!(ai.second_front_captures.is_empty(), "off: unchanged");

    ai.enable_campaign_second_front_waits_for_the_staging();
    ai.observe_second_front_staging(&g, 0);
    assert_eq!(ai.second_front_captures.get(&taken), Some(&120));
    g.turn = 125;
    ai.observe_second_front_staging(&g, 0);
    assert_eq!(ai.second_front_captures.get(&taken), Some(&120), "first turn kept");
    g.cities.get_mut(&taken).unwrap().owner = 1;
    ai.observe_second_front_staging(&g, 0);
    assert!(ai.second_front_captures.is_empty());
}
