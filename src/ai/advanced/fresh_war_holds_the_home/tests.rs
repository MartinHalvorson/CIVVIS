//! `fresh-war-holds-the-home`: a war declared on us opens a window that, at
//! parity, keeps Settlers off the declarer's ground and buys one shooter;
//! a war we declared, a stronger seat and an expired window do neither.
//!
//! The boards copy the live one of `hotseat-20261128T031456Z` p1 turns
//! 12-16: one Warrior home, the declarer's soldiers stronger, Gold for a
//! Slinger in the treasury.

use super::*;
use crate::game::Game;
use crate::Pos;

fn at(col: i32, row: i32) -> Pos {
    crate::hex::offset_to_axial(col, row)
}

/// A flat grassland world of two met empires at peace, turn 60, our turn:
/// our capital at (6, 8) and our one Warrior at (11, 8); their capital far
/// away at (30, 16) and two of their Warriors at (12, 8) and (13, 8), in
/// sight of ours.
fn world() -> Game {
    let mut g = Game::new_full(2, 36, 22, 7_511, 1_000, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    g.found_city_for(0, at(6, 8), None);
    g.found_city_for(1, at(30, 16), None);
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    g.players[0]
        .explored
        .extend(g.map.tiles.keys().copied().collect::<Vec<_>>());
    g.at_war.clear();
    g.turn = 60;
    g.current = 0;
    g.spawn_test_unit("warrior", 0, at(11, 8));
    g.spawn_test_unit("warrior", 1, at(12, 8));
    g.spawn_test_unit("warrior", 1, at(13, 8));
    g
}

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    if gene {
        ai.enable_fresh_war_holds_the_home();
    }
    ai
}

/// Our turn ends at peace; on their turn they declare; our next turn
/// starts at war.
fn declared_on(ai: &mut AdvancedAi, g: &mut Game) {
    ai.note_fresh_war_peace(g, 0);
    g.at_war.insert((0, 1));
    g.turn += 1;
    ai.observe_fresh_wars(g, 0);
}

fn shooters(g: &Game) -> usize {
    g.player_unit_ids(0)
        .into_iter()
        .filter(|uid| g.rules.units[g.units[uid].kind].has_ranged_attack())
        .count()
}

#[test]
fn fresh_war_holds_the_home_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(!ai.fresh_war_holds_the_home, "an opt-in ships off");
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "fresh_war_holds_the_home"));
    super::super::test_support::opt_in_off_in_both_controllers("fresh-war-holds-the-home", |ai| {
        ai.fresh_war_holds_the_home
    });
}

/// A declaration between our turns opens the window; a war already on at
/// the end of our turn (ours) opens none; with the gene off nothing opens.
#[test]
fn a_war_declared_on_us_opens_the_window_and_ours_does_not() {
    let mut g = world();
    assert!(g.military_power(1) >= g.military_power(0));
    let mut ai = seat(true);
    declared_on(&mut ai, &mut g);
    assert_eq!(ai.fresh_war_window(&g, 0), Some((1, 61)));

    // Our own declaration: at war when our turn ends.
    let mut g = world();
    let mut ours = seat(true);
    g.at_war.insert((0, 1));
    ours.note_fresh_war_peace(&g, 0);
    g.turn += 1;
    ours.observe_fresh_wars(&g, 0);
    assert_eq!(ours.fresh_war_window(&g, 0), None, "a war we chose");

    // A war already on when the seat first looked: no peace was seen.
    let mut g = world();
    g.at_war.insert((0, 1));
    let mut late = seat(true);
    late.observe_fresh_wars(&g, 0);
    assert_eq!(late.fresh_war_window(&g, 0), None);

    let mut g = world();
    let mut off = seat(false);
    declared_on(&mut off, &mut g);
    assert_eq!(off.fresh_war_window(&g, 0), None);
    assert!(off.fresh_war_onsets.is_empty() && off.fresh_war_peace_seen.is_empty());
}

/// The window holds only while the declarer's military is at least ours,
/// and closes after `FRESH_WAR_TURNS` standard turns or at peace.
#[test]
fn the_window_needs_parity_and_closes_with_time_and_peace() {
    let mut g = world();
    let mut ai = seat(true);
    declared_on(&mut ai, &mut g);
    assert!(ai.fresh_war_window(&g, 0).is_some());
    // Stronger than the declarer: the home is not held, the window waits.
    let extra: Vec<u32> = (0..3)
        .map(|i| g.spawn_test_unit("warrior", 0, at(5, 6 + i)))
        .collect();
    assert!(g.military_power(0) > g.military_power(1));
    assert_eq!(ai.fresh_war_window(&g, 0), None);
    for uid in extra {
        g.remove_unit(uid);
    }
    assert!(ai.fresh_war_window(&g, 0).is_some());
    // The last turn of the window, then past it.
    let duration = g.standard_duration(FRESH_WAR_TURNS);
    g.turn = 61 + duration - 1;
    ai.observe_fresh_wars(&g, 0);
    assert!(ai.fresh_war_window(&g, 0).is_some(), "still inside");
    g.turn += 1;
    ai.observe_fresh_wars(&g, 0);
    assert_eq!(ai.fresh_war_window(&g, 0), None, "past its time");
    assert!(ai.fresh_war_held_ground.is_empty());

    let mut g = world();
    let mut ai = seat(true);
    declared_on(&mut ai, &mut g);
    g.at_war.clear();
    g.turn += 1;
    ai.observe_fresh_wars(&g, 0);
    assert_eq!(ai.fresh_war_window(&g, 0), None, "peace closes it");
}

/// Ground within `FRESH_WAR_GROUND_RADIUS` of where the declarer's soldiers
/// were seen is no Settler site while the window holds, and only then.
#[test]
fn the_declarers_ground_is_no_settler_site() {
    let site = at(15, 8);
    let far = at(20, 8);
    let mut g = world();
    assert!(g.wdist(at(13, 8), site) <= FRESH_WAR_GROUND_RADIUS);
    assert!(g.wdist(at(13, 8), far) > FRESH_WAR_GROUND_RADIUS);
    let mut ai = seat(true);
    assert!(
        !ai.settler_site_is_dead(0, site),
        "at peace the site is open"
    );
    declared_on(&mut ai, &mut g);
    assert!(ai.fresh_war_holds_the_ground(site));
    assert!(ai.settler_site_is_dead(0, site));
    assert!(!ai.settler_site_is_dead(0, far));
    // The soldiers walk away: the ground they were seen on stays held.
    for uid in g.player_unit_ids(1) {
        g.remove_unit(uid);
    }
    g.spawn_test_unit("warrior", 1, at(28, 16));
    g.spawn_test_unit("warrior", 1, at(29, 16));
    g.turn += 1;
    ai.observe_fresh_wars(&g, 0);
    assert!(ai.settler_site_is_dead(0, site), "last seen there");

    let mut g = world();
    let mut off = seat(false);
    declared_on(&mut off, &mut g);
    assert!(!off.settler_site_is_dead(0, site));
}

/// One shooter, once per window, with the Gold the seat holds; none when we
/// are the stronger, none with the gene off.
#[test]
fn one_shooter_is_bought_once_per_window() {
    let mut g = world();
    g.players[0].gold = 500.0;
    let mut ai = seat(true);
    declared_on(&mut ai, &mut g);
    assert_eq!(shooters(&g), 0);
    assert!(ai.fresh_war_defender_purchase(&mut g, 0));
    assert_eq!(shooters(&g), 1, "a ranged unit joined the capital");
    let city = g.city_at(at(6, 8)).expect("our capital");
    assert!(g
        .unit_ids_at(g.cities[&city].pos)
        .iter()
        .any(|uid| g.rules.units[g.units[uid].kind].has_ranged_attack()));
    assert!(
        !ai.fresh_war_defender_purchase(&mut g, 0),
        "once per window"
    );
    assert_eq!(shooters(&g), 1);

    let mut g = world();
    g.players[0].gold = 500.0;
    let mut off = seat(false);
    declared_on(&mut off, &mut g);
    assert!(!off.fresh_war_defender_purchase(&mut g, 0));
    assert_eq!(shooters(&g), 0);

    let mut g = world();
    g.players[0].gold = 500.0;
    for row in 6..9 {
        g.spawn_test_unit("warrior", 0, at(5, row));
    }
    let mut strong = seat(true);
    declared_on(&mut strong, &mut g);
    assert!(
        !strong.fresh_war_defender_purchase(&mut g, 0),
        "we are stronger"
    );
    assert_eq!(shooters(&g), 0);

    let mut g = world();
    g.players[0].gold = 0.0;
    let mut poor = seat(true);
    declared_on(&mut poor, &mut g);
    assert!(!poor.fresh_war_defender_purchase(&mut g, 0), "no Gold");
}
