//! `capital-founds-before-it-flees`: a cityless seat's Settler founds where
//! it stands when a raider is in reach (seen or remembered), when its walk
//! runs past the deadline, and rather than walk more than two turns; a seat
//! with a city, an army that would break the city and the gene off do not.
//!
//! The boards copy the recorded one of hotseat check game `crash-p4-fa`
//! seat p1 (Greece), shifted by (-10, -24) in Civilization VI offset
//! coordinates (an even row shift keeps the hex layout): the Settler on its
//! site (25, 34) on turn 3 with a barbarian Scout at (22, 34), three tiles
//! off, and its Warrior far to the south-west at (20, 37); on turn 4 the
//! Settler one tile on at (26, 34) with that Scout out of sight and
//! remembered from turn 3.

use super::*;
use crate::ai::advanced::RememberedHostile;
use crate::game::Game;
use crate::Pos;
use std::collections::BTreeSet;
use std::sync::Arc;

fn at(col: i32, row: i32) -> Pos {
    crate::hex::offset_to_axial(col, row)
}

/// The site the turn-1 walk chose, recorded (25, 34).
fn site() -> Pos {
    at(15, 10)
}

/// The barbarian Scout's turn-3 tile, recorded (22, 34).
fn scout_tile() -> Pos {
    at(12, 10)
}

/// A flat grassland world on turn 3: seat 0 holds no city, every tile is in
/// sight (the live board's visible set), and its Warrior stands far off.
fn world() -> Game {
    let mut g = Game::new_full(2, 30, 24, 61_128, 250, 0, true);
    g.current = 0;
    g.turn = 3;
    g.clear_mirror_cities();
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.river_edges = [false; 6];
    }
    let everywhere: Vec<Pos> = g.map.tiles.keys().copied().collect();
    g.players[0].explored.extend(everywhere.iter().copied());
    g.host_observed = Arc::new(everywhere.into_iter().collect::<BTreeSet<Pos>>());
    g.spawn_test_unit("warrior", 0, at(10, 13));
    g
}

fn settler_at(g: &mut Game, pos: Pos) -> u32 {
    let uid = g.spawn_test_unit("settler", 0, pos);
    g.units.get_mut(&uid).unwrap().moves_left = 2.0;
    uid
}

/// The live seat's settler policy: the capture lessons (with the host's
/// capturing barbarian Scouts), the formationless escort and its settlement
/// safety, and the threat detour; `on` adds the gene.
fn seat(on: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_live_settler_capture_lessons();
    ai.enable_live_barbarian_scouts_capture();
    ai.enable_live_formationless_settler_shadow();
    ai.enable_settler_threat_detour();
    ai.settlement_safety = true;
    if on {
        ai.enable_capital_founds_before_it_flees();
    }
    ai
}

/// Turn 3: the Settler on its chosen site, the Scout three tiles off.
fn turn_three(on: bool) -> (Game, AdvancedAi, u32, crate::reasoning::Journal) {
    let mut g = world();
    let barbarian = g.barb_pid.unwrap();
    g.spawn_test_unit("scout", barbarian, scout_tile());
    let uid = settler_at(&mut g, site());
    let mut ai = seat(on);
    ai.settler_targets.insert(uid, site());
    ai.capital_founds_clock = Some(1);
    let journal = crate::reasoning::Journal::recording();
    ai.attach_journal(journal.handle());
    (g, ai, uid, journal)
}

fn founded_lines(journal: &crate::reasoning::Journal) -> usize {
    journal
        .since(0)
        .thoughts
        .iter()
        .filter(|thought| thought.headline.starts_with("Founding the capital at"))
        .filter(|thought| thought.detail.contains("capital-founds-before-it-flees"))
        .count()
}

#[test]
fn the_gene_is_registered_opt_in_and_ships_off() {
    let gene = GENES
        .iter()
        .find(|gene| gene.tag == "capital-founds-before-it-flees")
        .expect("registered");
    assert!(gene.opt_in());
    assert!(!AdvancedAi::new().capital_founds_before_it_flees);
    let mut ai = AdvancedAi::new();
    ai.enable_capital_founds_before_it_flees();
    assert!(ai.capital_founds_before_it_flees);
    ai.disable_capital_founds_before_it_flees();
    assert!(!ai.capital_founds_before_it_flees);
}

#[test]
fn the_threatened_first_settler_founds_on_its_site() {
    // Off: the recorded turn 3. The Scout puts the site's risk over the step
    // limit and the Settler leaves the site it walked two turns to found.
    let (mut g, mut ai, uid, _) = turn_three(false);
    assert!(g.can_found_city(uid));
    ai.advanced_settler_step(&mut g, 0, uid);
    assert!(g.player_city_ids(0).is_empty(), "off, no capital");
    assert!(
        g.units.contains_key(&uid),
        "off, the Settler is still a Settler"
    );

    // On: the capital stands on the site before any retreat.
    let (mut g, mut ai, uid, journal) = turn_three(true);
    assert!(ai.advanced_settler_step(&mut g, 0, uid));
    assert!(!g.units.contains_key(&uid), "the Settler became the city");
    let city = g
        .city_at(site())
        .expect("the capital is founded on the site");
    assert_eq!(g.cities[&city].owner, 0);
    assert_eq!(founded_lines(&journal), 1, "the founding is journaled");
    assert_eq!(
        ai.capital_founds_clock, None,
        "the clock closes with the city"
    );
}

#[test]
fn a_remembered_raider_alone_is_enough() {
    // The recorded turn 4: the Settler one tile on, the Scout out of sight,
    // remembered on its turn-3 tile; its envelope (three moves and a turn)
    // reaches the Settler, so the capture lessons flee it.
    let board = |on: bool| {
        let mut g = world();
        g.turn = 4;
        let uid = settler_at(&mut g, at(16, 10));
        let mut ai = seat(on);
        ai.settler_targets.insert(uid, at(15, 9));
        ai.capital_founds_clock = Some(1);
        ai.hostile_last_seen.insert(
            393_221,
            RememberedHostile {
                pos: scout_tile(),
                when: 3,
                owner: g.barb_pid.unwrap(),
                kind: crate::name!("scout"),
            },
        );
        (g, ai, uid)
    };
    let (g, ai, uid) = board(true);
    let here = g.units[&uid].pos;
    assert!(
        ai.barbarian_reach(&g, 0, here, REACH_SCAN_RADIUS)
            .covers(&g, here),
        "the remembered Scout's envelope reaches the Settler"
    );

    let (mut g, mut ai, uid) = board(false);
    ai.advanced_settler_step(&mut g, 0, uid);
    assert!(g.player_city_ids(0).is_empty(), "off, the Settler flees");
    assert_ne!(g.units[&uid].pos, here, "off, it steps out of the envelope");

    let (mut g, mut ai, uid) = board(true);
    assert!(ai.advanced_settler_step(&mut g, 0, uid));
    assert!(
        g.city_at(here).is_some(),
        "on, the capital stands where it was"
    );
    assert!(!g.units.contains_key(&uid));
}

#[test]
fn a_walk_longer_than_two_turns_gives_way_to_the_capital() {
    // No threat; the cached site is `distance` tiles east on flat grassland,
    // a turn a tile (`opening_walk_turns`).
    let board = |on: bool, distance: i32| {
        let mut g = world();
        let here = site();
        let uid = settler_at(&mut g, here);
        let mut ai = seat(on);
        ai.settler_targets.insert(uid, (here.0 + distance, here.1));
        let journal = crate::reasoning::Journal::recording();
        ai.attach_journal(journal.handle());
        (g, ai, uid, here, journal)
    };
    let (mut g, mut ai, uid, here, _) = board(false, 4);
    ai.advanced_settler_step(&mut g, 0, uid);
    assert!(
        g.player_city_ids(0).is_empty(),
        "off, it walks the four turns"
    );
    assert_ne!(g.units[&uid].pos, here);

    let (mut g, mut ai, uid, here, journal) = board(true, 4);
    assert!(ai.advanced_settler_step(&mut g, 0, uid));
    assert!(
        g.city_at(here).is_some(),
        "on, a four-turn walk is not taken"
    );
    assert_eq!(founded_lines(&journal), 1);

    // A walk of two turns still goes ahead.
    let (mut g, mut ai, uid, here, journal) = board(true, 2);
    ai.advanced_settler_step(&mut g, 0, uid);
    assert!(g.player_city_ids(0).is_empty(), "a two-turn walk stands");
    assert_ne!(g.units[&uid].pos, here);
    assert_eq!(founded_lines(&journal), 0);
}

#[test]
fn the_deadline_founds_wherever_it_stands() {
    // A two-turn walk (allowed on its own), no threat, a clock started on
    // turn `first`; it is turn 3.
    let board = |first: u32| {
        let mut g = world();
        let here = site();
        let uid = settler_at(&mut g, here);
        let mut ai = seat(true);
        ai.settler_targets.insert(uid, (here.0 + 2, here.1));
        ai.capital_founds_clock = Some(first);
        (g, ai, uid, here)
    };
    let (mut g, mut ai, uid, here) = board(1);
    ai.advanced_settler_step(&mut g, 0, uid);
    assert!(
        g.player_city_ids(0).is_empty(),
        "two turns in, it still walks"
    );
    assert_ne!(g.units[&uid].pos, here);

    let (mut g, mut ai, uid, here) = board(0);
    assert!(ai.advanced_settler_step(&mut g, 0, uid));
    assert!(g.city_at(here).is_some(), "three turns in, it founds here");

    // The first step starts the clock.
    let (mut g, mut ai, uid, _) = board(1);
    ai.capital_founds_clock = None;
    ai.capital_founds_before_it_flees_step(&mut g, 0, uid);
    assert_eq!(ai.capital_founds_clock, Some(3));
}

#[test]
fn a_refused_tile_steps_to_a_plot_it_can_found_this_turn() {
    let (mut g, mut ai, uid, _) = turn_three(true);
    g.blocked_city_sites = Arc::new([site()].into_iter().collect());
    assert!(!g.can_found_city(uid));
    assert!(ai.capital_founds_before_it_flees_step(&mut g, 0, uid));
    assert!(!g.units.contains_key(&uid));
    assert!(g.city_at(site()).is_none(), "not on the refused plot");
    let city = g.player_city_ids(0)[0];
    assert_eq!(
        g.wdist(g.cities[&city].pos, site()),
        1,
        "one step and the city, the same turn"
    );
}

#[test]
fn a_persons_scout_at_peace_cannot_chase_the_first_settler_off_its_site() {
    // Under `person-units-threaten-civilians` a person's Scout three tiles
    // off prices the site at the capture tier at peace: without this gene the
    // Settler steps off its site, as it did from the barbarian Scout.
    let board = |on: bool| {
        let mut g = world();
        g.at_war.remove(&(0, 1));
        g.at_war.remove(&(1, 0));
        assert!(!g.is_at_war(0, 1));
        Arc::make_mut(&mut g.observed_person_rivals).insert(1);
        g.spawn_test_unit("scout", 1, scout_tile());
        let uid = settler_at(&mut g, site());
        let mut ai = seat(on);
        ai.enable_person_units_threaten_civilians();
        ai.settler_targets.insert(uid, site());
        ai.capital_founds_clock = Some(1);
        (g, ai, uid)
    };
    let (mut g, mut ai, uid) = board(false);
    ai.advanced_settler_step(&mut g, 0, uid);
    assert!(
        g.player_city_ids(0).is_empty(),
        "off, the person's Scout wins"
    );
    assert_ne!(g.units[&uid].pos, site());

    let (mut g, mut ai, uid) = board(true);
    assert!(ai.advanced_settler_step(&mut g, 0, uid));
    assert!(g.city_at(site()).is_some(), "on, the capital stands");
}

#[test]
fn a_seat_with_a_city_an_overwhelming_army_and_the_gene_off_are_untouched() {
    // A seat that already holds a city is left to the rules it had.
    let (mut g, mut ai, uid, _) = turn_three(true);
    g.found_city_for(0, at(3, 20), None);
    assert!(!ai.capital_founds_before_it_flees_step(&mut g, 0, uid));
    assert_eq!(ai.capital_founds_clock, None);
    assert!(!ai.capital_founds_instead_of_walking(&mut g, 0, uid, Some(site()), None));
    assert!(g.units.contains_key(&uid));

    // An army that would break the new city keeps the retreat open.
    let (mut g, mut ai, uid, _) = turn_three(true);
    let barbarian = g.barb_pid.unwrap();
    for pos in [at(16, 10), at(14, 10), at(15, 11)] {
        g.spawn_test_unit("modern_armor", barbarian, pos);
    }
    assert!(!ai.capital_founds_before_it_flees_step(&mut g, 0, uid));
    assert!(g.player_city_ids(0).is_empty());

    // A Settler with no movement left cannot found this turn.
    let (mut g, mut ai, uid, _) = turn_three(true);
    g.units.get_mut(&uid).unwrap().moves_left = 0.0;
    assert!(!ai.capital_founds_before_it_flees_step(&mut g, 0, uid));
    assert!(g.player_city_ids(0).is_empty());

    // Off: neither hook acts and the clock never starts.
    let (mut g, mut ai, uid, _) = turn_three(false);
    ai.capital_founds_clock = None;
    assert!(!ai.capital_founds_before_it_flees_step(&mut g, 0, uid));
    assert!(!ai.capital_founds_instead_of_walking(&mut g, 0, uid, Some(site()), None));
    assert_eq!(ai.capital_founds_clock, None);
    assert!(g.player_city_ids(0).is_empty());
}
