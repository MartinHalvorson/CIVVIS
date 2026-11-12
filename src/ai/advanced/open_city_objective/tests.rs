//! Focused tests for `open-city-is-the-objective`
//! (`advanced/open_city_objective.rs`).
//!
//! The board is the live shape: our capital in the west; player 1, at war
//! with us, holds a walled city and, six tiles east of it, an open city with
//! three of our Swordsmen two and three tiles short of it.

use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::VictoryTarget;
use crate::Pos;
use std::sync::Arc;

const OPEN: Pos = (18, 8);

/// Open grassland: we hold (2, 8); player 1, at war with us, holds a walled
/// city at (12, 8) and the open city at [`OPEN`], read at a centre of 25.
/// Returns the board, the walled city and the open city.
fn front() -> (Game, u32, u32) {
    let mut g = Game::new_full(2, 36, 22, 4_175, 300, 0, false);
    let ids: Vec<_> = g.units.keys().copied().collect();
    for id in ids {
        g.remove_unit(id);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    g.found_city_for(0, (2, 8), None);
    let walled = g.found_city_for(1, (12, 8), None);
    let open = g.found_city_for(1, OPEN, None);
    {
        let city = g.cities.get_mut(&walled).unwrap();
        city.buildings.push(crate::name!("walls"));
        city.wall_hp = 100;
    }
    Arc::make_mut(&mut g.observed_city_strength).insert(open, 25.0);
    g.record_contact(0, 1);
    g.at_war.insert((0, 1));
    g.current = 0;
    g.turn = 80;
    (g, walled, open)
}

/// Three Swordsmen two and three tiles west of the open city.
fn takers(g: &mut Game) -> Vec<u32> {
    [(16, 8), (15, 8), (16, 9)]
        .into_iter()
        .map(|pos| g.spawn_test_unit("swordsman", 0, pos))
        .collect()
}

fn seat(on: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    if on {
        ai.enable_open_city_is_the_objective();
    }
    ai
}

#[test]
fn ships_off() {
    opt_in_off_in_both_controllers("open-city-is-the-objective", |ai| {
        ai.open_city_is_the_objective
    });
}

#[test]
fn the_reckoning_counts_blows_return_blows_healing_and_the_march() {
    // Three Warriors against a centre of 13: about 40 a blow, 119 a turn
    // against 200 health and 20 healing, the second turn takes it.
    let three = [(20.0, 100, 1), (20.0, 100, 1), (20.0, 100, 1)];
    assert_eq!(open_city_take_turns(&three, 13.0, 200, 5), Some(2));
    // One Warrior against a centre of 30 never gets there: 25 a blow under
    // 20 of healing, and it is spent after two.
    assert_eq!(open_city_take_turns(&[(20.0, 100, 1)], 30.0, 200, 5), None);
    // The same three arriving on the fourth turn miss a three-turn horizon.
    let late = [(20.0, 100, 4), (20.0, 100, 4), (20.0, 100, 4)];
    assert_eq!(open_city_take_turns(&late, 13.0, 200, 3), None);
    assert_eq!(open_city_take_turns(&late, 13.0, 200, 5), Some(5));
    // A city already at nothing falls to the first taker.
    assert_eq!(open_city_take_turns(&three, 13.0, 0, 5), Some(1));
    assert_eq!(open_city_take_turns(&[], 13.0, 0, 5), None);
}

#[test]
fn three_swordsmen_take_the_open_city_only_with_the_gene() {
    let (mut g, _, open) = front();
    takers(&mut g);
    assert_eq!(
        seat(false).open_city_take_of(&g, 0, 1),
        None,
        "off: nothing"
    );
    let take = seat(true)
        .open_city_take_of(&g, 0, 1)
        .expect("three Swordsmen beside an open centre of 25");
    assert_eq!((take.rival, take.city, take.takers), (1, open, 3));
    assert!(take.turns <= OPEN_CITY_TURNS, "{take:?}");
    assert_eq!(seat(true).open_city_take_best(&g, 0), Some(take));
}

#[test]
fn each_condition_holds_the_capture() {
    let ai = seat(true);
    // Walls.
    let (mut g, _, open) = front();
    takers(&mut g);
    g.cities.get_mut(&open).unwrap().wall_hp = 100;
    assert_eq!(ai.open_city_take_of(&g, 0, 1), None, "walls");
    // No war.
    let (mut g, _, _) = front();
    takers(&mut g);
    g.at_war.clear();
    assert_eq!(ai.open_city_take_of(&g, 0, 1), None, "at peace");
    // A centre past the tier floor.
    let (mut g, _, open) = front();
    takers(&mut g);
    Arc::make_mut(&mut g.observed_city_strength).insert(open, 60.0);
    assert_eq!(ai.open_city_take_of(&g, 0, 1), None, "tier floor");
    // Takers past the march.
    let (mut g, _, _) = front();
    for pos in [(8, 8), (8, 9), (7, 8)] {
        g.spawn_test_unit("swordsman", 0, pos);
    }
    assert_eq!(ai.open_city_take_of(&g, 0, 1), None, "past the march");
    // One wounded Warrior is no capture.
    let (mut g, _, _) = front();
    let lone = g.spawn_test_unit("warrior", 0, (16, 8));
    g.units.get_mut(&lone).unwrap().hp = 60;
    assert_eq!(ai.open_city_take_of(&g, 0, 1), None, "too little");
    // The owner's soldiers in sight, three tiles from the city (no
    // garrison), outweigh the takers.
    let (mut g, _, _) = front();
    let ours = takers(&mut g);
    let near_ours: Vec<Pos> = ours.iter().map(|uid| g.units[uid].pos).collect();
    let mut tiles: Vec<Pos> = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| g.wdist(*pos, OPEN) == 3)
        .filter(|pos| near_ours.iter().any(|at| g.wdist(*at, *pos) <= 2))
        .filter(|pos| g.unit_ids_at(*pos).is_empty())
        .collect();
    tiles.sort_unstable();
    assert!(tiles.len() >= 4, "{tiles:?}");
    for pos in tiles.into_iter().take(4) {
        g.spawn_test_unit("swordsman", 1, pos);
    }
    assert_eq!(ai.open_city_take_of(&g, 0, 1), None, "the local check");
    // Off the Domination lane.
    let (mut g, _, _) = front();
    takers(&mut g);
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_open_city_is_the_objective();
    assert_eq!(science.open_city_take_of(&g, 0, 1), None, "lane");
}

#[test]
fn the_open_city_is_the_objective_ahead_of_the_walled_one() {
    let (mut g, walled, open) = front();
    takers(&mut g);
    assert_eq!(
        seat(false).open_city_objective(&g, 0, Some(walled), |_| false),
        None,
        "off: the plan stands"
    );
    let ai = seat(true);
    let take = ai
        .open_city_objective(&g, 0, Some(walled), |_| false)
        .expect("the open city outranks the walled one");
    assert_eq!((take.rival, take.city), (1, open));
    // Already the objective: nothing to change.
    assert_eq!(ai.open_city_objective(&g, 0, Some(open), |_| false), None);
    // A home emergency, the finishing capital or another urgent clock.
    assert_eq!(ai.open_city_objective(&g, 0, Some(walled), |_| true), None);
    // A walled objective already being reduced finishes first.
    g.cities.get_mut(&walled).unwrap().hp = 150;
    assert_eq!(ai.open_city_objective(&g, 0, Some(walled), |_| false), None);
}

#[test]
fn a_falling_city_of_ours_holds_the_objective_and_the_peace() {
    let (mut g, walled, _) = front();
    takers(&mut g);
    let home = g.city_at((2, 8)).unwrap();
    {
        let city = g.cities.get_mut(&home).unwrap();
        city.hp = 60;
        city.last_attacked = g.turn;
    }
    let ai = seat(true);
    assert_eq!(ai.open_city_objective(&g, 0, Some(walled), |_| false), None);
    assert!(!ai.open_city_keeps_the_war(&g, 0, 1));
    assert_eq!(ai.open_city_front_swap(&g, 0, None, &[1]), None);
}

#[test]
fn the_war_is_kept_while_the_capture_stands() {
    let (mut g, _, _) = front();
    takers(&mut g);
    assert!(!seat(false).open_city_keeps_the_war(&g, 0, 1), "off");
    assert!(seat(true).open_city_keeps_the_war(&g, 0, 1));
    // Outmatched: the stock offer stands.
    let ours = g.military_power(0);
    Arc::make_mut(&mut g.observed_military_power).insert(1, 2.0 * ours.max(1.0));
    assert!(!seat(true).open_city_keeps_the_war(&g, 0, 1), "outmatched");
    // No taker in reach: nothing keeps it.
    let (g, _, _) = front();
    assert!(!seat(true).open_city_keeps_the_war(&g, 0, 1), "no takers");
}

#[test]
fn the_one_war_rule_still_closes_for_a_threat_a_capital_or_a_rout() {
    use one_war::OneWarPeace;
    for peace in [
        OneWarPeace::VictoryThreat,
        OneWarPeace::CapitalSecured,
        OneWarPeace::CapitalElsewhere,
        OneWarPeace::Rout,
    ] {
        assert!(AdvancedAi::open_city_yields_to(Some(peace)));
    }
    for peace in [OneWarPeace::SecondFront, OneWarPeace::TideTurned] {
        assert!(!AdvancedAi::open_city_yields_to(Some(peace)));
    }
    assert!(!AdvancedAi::open_city_yields_to(None));
}

#[test]
fn its_war_is_a_front() {
    let (mut g, _, _) = front();
    takers(&mut g);
    assert!(!seat(false).open_city_war_is_a_front(&g, 0, 1));
    assert!(seat(true).open_city_war_is_a_front(&g, 0, 1));
    assert_eq!(seat(false).open_city_front_swap(&g, 0, None, &[1]), None);
    assert_eq!(seat(true).open_city_front_swap(&g, 0, None, &[1]), Some(1));
    // The current front already offers the open city: it stays.
    assert_eq!(seat(true).open_city_front_swap(&g, 0, Some(1), &[1]), None);
}

#[test]
fn the_siege_goes_on_the_trains_own_takers() {
    let (mut g, _, open) = front();
    let members = takers(&mut g);
    assert_eq!(seat(false).open_city_siege_go(&g, 0, open, &members), None);
    let take = seat(true)
        .open_city_siege_go(&g, 0, open, &members)
        .expect("the train's three Swordsmen take it");
    assert_eq!(take.takers, 3);
    // Takers outside the train do not count.
    assert_eq!(seat(true).open_city_siege_go(&g, 0, open, &[]), None);
    assert_eq!(
        seat(true).open_city_siege_go(&g, 0, open, &members[..1]),
        None,
        "one Swordsman alone does not take a centre of 25 in time"
    );
}
