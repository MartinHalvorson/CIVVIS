//! Focused tests for `first-capital-strikes-the-open-window`.
//!
//! The seat is the live Domination seat's opening: the armed
//! `early-conquest-opening`, `conquest-opening-stays-near`,
//! `early-conquest-stands-down`, `declaration-needs-production-parity` and
//! `first-capital-before-the-walls`, on Online speed at turn 45. Every test
//! reads the same board with this gene off first, which is today's
//! behaviour.

use super::super::test_support::opt_in_off_in_both_controllers;
use super::super::{EmpireCounts, GrandStrategy, StrategicPlan, VictoryTarget};
use super::*;
use crate::game::Game;
use crate::name;
use crate::Pos;
use std::sync::Arc;

/// A flat 60x30 board of `capitals.len()` empires, every starting unit
/// cleared, nobody at war, Online speed, turn 45.
fn board(capitals: &[Pos]) -> Game {
    let mut game = Game::new_full(capitals.len(), 60, 30, 90_909, 1_000, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    for (pid, pos) in capitals.iter().enumerate() {
        game.found_city_for(pid, *pos, None);
    }
    game.at_war.clear();
    game.game_speed = crate::setup::GameSpeed::Online;
    game.turn = 45;
    game.current = 0;
    game
}

fn meet(game: &mut Game, rival: usize) {
    game.players[0].met.insert(rival);
    game.players[rival].met.insert(0);
}

fn meet_and_explore(game: &mut Game, rival: usize) {
    meet(game, rival);
    let tiles: Vec<Pos> = game.map.tiles.keys().copied().collect();
    game.players[0].explored.extend(tiles);
}

fn at_war(game: &mut Game, a: usize, b: usize) {
    game.at_war.insert((a, b));
    game.at_war.insert((b, a));
}

/// The live seat's opening, with or without this gene.
fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_early_conquest_opening();
    ai.enable_conquest_opening_stays_near();
    ai.enable_early_conquest_stands_down();
    ai.enable_declaration_needs_production_parity();
    ai.conquest_opening_needs_the_production = false;
    ai.enable_first_capital_before_the_walls();
    if gene {
        ai.enable_first_capital_strikes_the_open_window();
    }
    ai
}

fn capital_of(game: &Game, pid: usize) -> u32 {
    game.player_city_ids(pid)
        .into_iter()
        .find(|cid| game.cities[cid].is_capital)
        .unwrap()
}

fn set_walls(game: &mut Game, cid: u32, hp: i32) {
    Arc::make_mut(&mut game.observed_city_max_wall_hp).insert(cid, hp);
    game.cities.get_mut(&cid).unwrap().wall_hp = hp;
}

fn set_strength(game: &mut Game, cid: u32, strength: f64) {
    Arc::make_mut(&mut game.observed_city_strength).insert(cid, strength);
}

fn unit(game: &mut Game, kind: &str, pos: Pos) -> u32 {
    let uid = game.spawn_test_unit(kind, 0, pos);
    let moves = game.unit_max_moves(uid);
    let unit = game.units.get_mut(&uid).unwrap();
    unit.moves_left = moves;
    unit.attacks_left = 1;
    unit.moved = false;
    unit.acted = false;
    uid
}

/// `count` bodies of ours of `kind` on the ring `radius` around `around`.
fn bodies(game: &mut Game, kind: &str, around: Pos, radius: i32, count: usize) -> Vec<u32> {
    let ring: Vec<Pos> = game
        .wring(around, radius)
        .into_iter()
        .filter(|pos| game.city_at(*pos).is_none() && game.unit_ids_at(*pos).is_empty())
        .collect();
    let spawned: Vec<u32> = ring
        .into_iter()
        .take(count)
        .map(|pos| unit(game, kind, pos))
        .collect();
    assert_eq!(spawned.len(), count, "the ring had room");
    spawned
}

fn plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 10,
        assessed_turn: 45,
        rush: false,
    }
}

/// Our second city: no strike opens before the empire holds two.
fn second_city(game: &mut Game) {
    game.found_city_for(0, (6, 18), None);
}

/// Seats 0 (us), 1 (the first capital, 18 tiles east) and 2 (a rival to
/// the south); the opening named on seat 1's capital.
fn opened(gene: bool) -> (Game, AdvancedAi, u32) {
    let mut game = board(&[(6, 12), (24, 12), (6, 24)]);
    meet_and_explore(&mut game, 1);
    meet(&mut game, 2);
    // Seat 2's capital is walled past what a breaker opens, so the opening
    // names seat 1's.
    let walled = capital_of(&game, 2);
    set_walls(&mut game, walled, 300);
    let mut ai = seat(gene);
    ai.maintain_conquest_opening(&mut game, 0);
    let capital = capital_of(&game, 1);
    let opening = ai.conquest_opening.as_ref().expect("the opening stands");
    assert_eq!((opening.target, opening.city), (1, capital));
    (game, ai, capital)
}

// ----------------------------------------------------------------- registry

#[test]
fn first_capital_strikes_the_open_window_is_a_native_opt_in_off_in_both_controllers() {
    opt_in_off_in_both_controllers("first-capital-strikes-the-open-window", |ai| {
        ai.first_capital_strikes_the_open_window
    });
    assert!(!seat(false).first_capital_strikes_active());
    assert!(seat(true).first_capital_strikes_active());
    let mut bare = AdvancedAi::targeting(VictoryTarget::Domination);
    bare.enable_early_conquest_opening();
    bare.enable_first_capital_strikes_the_open_window();
    assert!(
        !bare.first_capital_strikes_active(),
        "rides on first-capital-before-the-walls"
    );
    let mut culture = seat(true);
    culture.victory_target = Some(VictoryTarget::Culture);
    assert!(!culture.first_capital_strikes_active(), "Domination only");
}

// --------------------------------------------------- 1. the campaign stays

/// The live turn 44: "Countering Phoenicia | its religion race reads 50%"
/// re-aimed the campaign at Tyre. Under the gene the opening's capital stays
/// the campaign's target and city; a home emergency or a front war still
/// outranks it.
#[test]
fn the_campaign_stays_on_the_opening_through_a_counter_retarget() {
    let (game, off, _) = opened(false);
    assert_eq!(
        off.first_capital_keeps_the_campaign(&game, 0, Some(2), &[], false, Some(2)),
        None,
        "off: the counter re-aims the campaign"
    );
    let (game, on, capital) = opened(true);
    assert_eq!(
        on.first_capital_keeps_the_campaign(&game, 0, Some(2), &[], false, Some(2)),
        Some((1, capital))
    );
    assert_eq!(
        on.first_capital_keeps_the_campaign(&game, 0, Some(2), &[], true, Some(2)),
        None,
        "a home emergency outranks the opening"
    );
    assert_eq!(
        on.first_capital_keeps_the_campaign(&game, 0, Some(2), &[2], false, None),
        None,
        "a front war outranks the opening"
    );
    // The whole assessment aims at the opening's city.
    let plan = on.assess(&game, 0);
    assert_eq!(
        (plan.target_player, plan.target_city),
        (Some(1), Some(capital))
    );
    // And the elective declaration is the opening's to make.
    assert!(on.first_capital_holds_the_war(&game, 2, false));
    assert!(on.first_capital_holds_the_war(&game, 1, false));
    assert!(on.first_capital_holds_the_raid(&game));
    let (game, off, _) = opened(false);
    assert!(!off.first_capital_holds_the_war(&game, 2, false));
    assert!(!off.first_capital_holds_the_raid(&game));
}

/// The live turn 46: a religious interception opened a war on Phoenicia.
/// Off, it is a front: one-war-at-a-time holds the opening's declaration
/// and the opening's staging stops. Under the gene it is no front, the
/// campaign stays on the opening, and a force body walks on to the rally.
#[test]
fn a_home_faith_war_is_no_front_while_the_window_stands() {
    for gene in [false, true] {
        let (mut game, mut ai, capital) = opened(gene);
        let uid = unit(&mut game, "warrior", (10, 12));
        ai.maintain_conquest_opening(&mut game, 0);
        assert!(ai.conquest_opening.as_ref().unwrap().force.contains(&uid));
        at_war(&mut game, 0, 2);
        ai.religious_interception_war = Some((2, game.turn));
        ai.enable_one_war_at_a_time();
        ai.one_war_observe(&game, 0);
        assert_eq!(ai.no_front_rivals(&game, 0).contains(&2), gene);
        assert_eq!(ai.one_war_holds_declaration(&game, 0, 1), !gene);
        assert_eq!(
            ai.conquest_staging_objective(&game, 0, uid, &plan())
                .is_some(),
            gene,
            "the staging stops for the war only off"
        );
        assert_eq!(ai.first_capital_marches_through(&game, 0, uid, &[2]), gene);
        assert!(
            !ai.first_capital_marches_through(&game, 0, uid, &[2, 1]),
            "a front war beside it is not walked through"
        );
        if gene {
            let plan = ai.assess(&game, 0);
            assert_eq!(
                (plan.target_player, plan.target_city),
                (Some(1), Some(capital)),
                "the faith war does not own the campaign"
            );
            // The force body walks toward the rally through the war.
            let rally = ai.conquest_opening.as_ref().unwrap().rally;
            let before = game.wdist(game.units[&uid].pos, rally);
            let mut walked = game.clone();
            assert_eq!(
                ai.campaign_staging_step(&mut walked, 0, uid, &plan),
                Some(true)
            );
            assert!(walked.wdist(walked.units[&uid].pos, rally) < before);
        }
    }
}

// ---------------------------------------------------------- 2. go on the board

/// Three bodies at the rally of an unwalled, empty capital cover its bill.
/// Off, three of six is a 50% share and nothing is declared; on, the
/// opening counts as assembled and declares that turn.
#[test]
fn the_opening_declares_with_three_bodies_when_they_cover_the_bill() {
    for gene in [false, true] {
        let (mut game, mut ai, capital) = opened(gene);
        set_strength(&mut game, capital, 15.0);
        let rally = ai.conquest_opening.as_ref().unwrap().rally;
        second_city(&mut game);
        bodies(&mut game, "swordsman", rally, 1, 3);
        ai.maintain_conquest_opening(&mut game, 0);
        let opening = ai.conquest_opening.clone().unwrap();
        assert_eq!(opening.force.len(), 3);
        assert_eq!(opening.assembled.is_some(), gene);
        assert_eq!(
            ai.first_capital_open_window_go(&game, 0, &opening)
                .is_some(),
            gene
        );
        assert_eq!(ai.conquest_declaration(&mut game, 0), gene);
        assert_eq!(game.is_at_war(0, 1), gene);
        if gene {
            assert_eq!(
                ai.conquest_opening.as_ref().unwrap().declared,
                Some(game.turn)
            );
            assert!(ai.conquest_owns_the_campaign());
        }
    }
}

/// Native 071743Z declared on turn 7 with one Warrior and two Scouts:
/// the opening counted three bodies, but the siege staged only the Warrior
/// while the Scouts kept exploring. Recon must not assemble that strike.
#[test]
fn scouts_do_not_fill_the_conquest_force_or_open_its_window() {
    for gene in [false, true] {
        let (mut game, mut ai, capital) = opened(gene);
        set_strength(&mut game, capital, 13.0);
        let rally = ai.conquest_opening.as_ref().unwrap().rally;
        let warrior = bodies(&mut game, "warrior", rally, 1, 1)[0];
        let scouts = bodies(&mut game, "scout", rally, 2, 2);
        ai.maintain_conquest_opening(&mut game, 0);
        let opening = ai.conquest_opening.clone().unwrap();
        assert_eq!(opening.force, [warrior].into_iter().collect());
        assert!(opening.assembled.is_none());
        assert!(ai
            .first_capital_open_window_go(&game, 0, &opening)
            .is_none());
        assert!(!ai.conquest_declaration(&mut game, 0));
        assert!(!game.is_at_war(0, 1));
        for scout in scouts {
            assert!(ai
                .conquest_staging_objective(&game, 0, scout, &plan())
                .is_none());
        }

        // The exclusion must keep actual melee, ranged and siege bodies.
        let archer = bodies(&mut game, "archer", rally, 1, 1)[0];
        let catapult = bodies(&mut game, "catapult", rally, 2, 1)[0];
        ai.maintain_conquest_opening(&mut game, 0);
        let opening = ai.conquest_opening.clone().unwrap();
        assert_eq!(
            opening.force,
            [warrior, archer, catapult].into_iter().collect()
        );
        // Scouts nearer the rally must not displace real soldiers when
        // the six-body reservation fills. Declaration has separate guards.
        let mut expected = vec![warrior, archer, catapult];
        expected.extend(bodies(&mut game, "swordsman", rally, 3, 3));
        ai.maintain_conquest_opening(&mut game, 0);
        assert_eq!(
            ai.conquest_opening.as_ref().unwrap().force,
            expected.into_iter().collect()
        );
    }
}

/// Two bodies are too few, and walls ask for the breaker at the rally.
#[test]
fn the_go_needs_three_bodies_and_a_breaker_at_the_walls() {
    let (mut game, mut ai, capital) = opened(true);
    set_strength(&mut game, capital, 15.0);
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    bodies(&mut game, "swordsman", rally, 1, 2);
    ai.maintain_conquest_opening(&mut game, 0);
    let opening = ai.conquest_opening.clone().unwrap();
    assert!(
        ai.first_capital_open_window_go(&game, 0, &opening)
            .is_none(),
        "two bodies are fewer than CAMPAIGN_MIN_BODIES"
    );

    let (mut game, mut ai, capital) = opened(true);
    set_strength(&mut game, capital, 15.0);
    set_walls(&mut game, capital, 100);
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    second_city(&mut game);
    bodies(&mut game, "swordsman", rally, 1, 4);
    let ram = unit(&mut game, "battering_ram", (6, 13));
    ai.maintain_conquest_opening(&mut game, 0);
    let opening = ai.conquest_opening.clone().unwrap();
    assert!(
        ai.first_capital_open_window_go(&game, 0, &opening)
            .is_none(),
        "walls stand and the breaker is at home"
    );
    assert!(!ai.conquest_declaration(&mut game, 0));
    // The breaker walks with the force: the staging objective is its too.
    assert!(ai.first_capital_breaker_walks(&game, 0, ram));
    assert!(ai
        .conquest_staging_objective(&game, 0, ram, &plan())
        .is_some());
    let (game_off, off, _) = opened(false);
    assert!(!off.first_capital_breaker_walks(&game_off, 0, ram));
    // At the rally it opens the walls for the go.
    let spot = game
        .wring(rally, 2)
        .into_iter()
        .find(|pos| game.city_at(*pos).is_none() && game.unit_ids_at(*pos).is_empty())
        .unwrap();
    game.remove_unit(ram);
    unit(&mut game, "battering_ram", spot);
    assert!(ai
        .first_capital_open_window_go(&game, 0, &opening)
        .is_some());
    assert!(ai.conquest_declaration(&mut game, 0));
    assert!(game.is_at_war(0, 1));
}

// ------------------------------------------------------- 3. a soft capital

/// Live: the reservation opened at the third city (turn 29) while the Cree
/// capital read defence 16-28. On a soft capital it opens at two.
#[test]
fn a_soft_capital_opens_the_reservation_at_two_cities() {
    for gene in [false, true] {
        let (mut game, mut ai, capital) = opened(gene);
        game.found_city_for(0, (6, 18), None);
        set_strength(&mut game, capital, 20.0);
        game.turn += 1;
        ai.maintain_conquest_opening(&mut game, 0);
        let home = capital_of(&game, 0);
        let counts = EmpireCounts::default();
        assert_eq!(
            ai.conquest_defers_the_settler(&game, 0, home, &counts, 2, false),
            gene,
            "two cities"
        );
        assert!(
            !ai.conquest_defers_the_settler(&game, 0, home, &counts, 1, false),
            "the first Settler is never deferred"
        );
        assert_eq!(
            ai.conquest_opening
                .as_ref()
                .unwrap()
                .preparing_since
                .is_some(),
            gene
        );
        // Hardened: the shipped three again.
        set_strength(&mut game, capital, 38.0);
        game.turn += 1;
        ai.maintain_conquest_opening(&mut game, 0);
        assert!(!ai.conquest_defers_the_settler(&game, 0, home, &counts, 2, false));
        assert!(ai.conquest_defers_the_settler(&game, 0, home, &counts, 3, false));
    }
    // The edges: strength 30 behind Ancient Walls is soft, 31 is not, nor
    // is anything behind Medieval Walls.
    let mut game = board(&[(6, 12), (24, 12)]);
    let capital = capital_of(&game, 1);
    set_walls(&mut game, capital, 100);
    set_strength(&mut game, capital, 30.0);
    assert!(AdvancedAi::first_capital_is_soft(&game, capital));
    set_strength(&mut game, capital, 31.0);
    assert!(!AdvancedAi::first_capital_is_soft(&game, capital));
    set_strength(&mut game, capital, 20.0);
    set_walls(&mut game, capital, 200);
    assert!(!AdvancedAi::first_capital_is_soft(&game, capital));
}

/// [`opened`] with seat 1's capital twelve tiles east instead of eighteen.
fn opened_next_door() -> (Game, AdvancedAi, u32) {
    let mut game = board(&[(6, 12), (18, 12), (6, 24)]);
    meet_and_explore(&mut game, 1);
    meet(&mut game, 2);
    let walled = capital_of(&game, 2);
    set_walls(&mut game, walled, 300);
    let mut ai = seat(true);
    ai.maintain_conquest_opening(&mut game, 0);
    let capital = capital_of(&game, 1);
    let opening = ai.conquest_opening.as_ref().expect("the opening stands");
    assert_eq!((opening.target, opening.city), (1, capital));
    (game, ai, capital)
}

/// The live Pasargadae shape: an unwalled capital twelve tiles away reading
/// 13. The reservation opens with our one city, the latch outlives a
/// garrison that walks in, and the strike does not wait for a second city.
#[test]
fn a_bare_capital_next_door_opens_the_reservation_at_one_city() {
    let (mut game, mut ai, capital) = opened_next_door();
    let home = capital_of(&game, 0);
    let counts = EmpireCounts::default();
    set_strength(&mut game, capital, 13.0);
    assert!(AdvancedAi::first_capital_is_bare(&game, 0, capital));
    game.turn += 1;
    ai.maintain_conquest_opening(&mut game, 0);
    assert_eq!(ai.first_capital_bare, Some(capital));
    assert_eq!(
        ai.conquest_first_settler_cities(),
        FIRST_CAPITAL_BARE_CITIES
    );
    assert!(
        ai.conquest_defers_the_settler(&game, 0, home, &counts, 1, false),
        "the force comes before the first Settler"
    );
    assert!(ai
        .conquest_opening
        .as_ref()
        .unwrap()
        .preparing_since
        .is_some());

    // Three Warriors at the rally, the owner at half our power: one city is
    // no reason to hold, and the go stands.
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    bodies(&mut game, "warrior", rally, 1, 3);
    game.turn += 1;
    ai.maintain_conquest_opening(&mut game, 0);
    let ours = game.military_power(0);
    Arc::make_mut(&mut game.observed_military_power).insert(1, 0.5 * ours);
    let opening = ai.conquest_opening.clone().unwrap();
    assert_eq!(ai.first_capital_outclassed(&game, 0, &opening), None);

    // A garrison walks in: 28 is still soft, and the latch holds.
    set_strength(&mut game, capital, 28.0);
    assert!(!AdvancedAi::first_capital_is_bare(&game, 0, capital));
    game.turn += 1;
    ai.maintain_conquest_opening(&mut game, 0);
    assert_eq!(ai.first_capital_bare, Some(capital));
    assert!(ai.conquest_defers_the_settler(&game, 0, home, &counts, 1, false));

    // Hardened past soft: the latch drops and the shipped three return.
    set_strength(&mut game, capital, 38.0);
    game.turn += 1;
    ai.maintain_conquest_opening(&mut game, 0);
    assert_eq!(ai.first_capital_bare, None);
    assert!(!ai.conquest_defers_the_settler(&game, 0, home, &counts, 1, false));
    assert!(!ai.conquest_defers_the_settler(&game, 0, home, &counts, 2, false));

    // The edges: 21 is not bare, nor is any wall, nor fifteen tiles.
    let mut game = board(&[(6, 12), (18, 12), (21, 20)]);
    let near = capital_of(&game, 1);
    set_strength(&mut game, near, 20.0);
    assert!(AdvancedAi::first_capital_is_bare(&game, 0, near));
    set_strength(&mut game, near, 21.0);
    assert!(!AdvancedAi::first_capital_is_bare(&game, 0, near));
    set_strength(&mut game, near, 13.0);
    set_walls(&mut game, near, 100);
    assert!(!AdvancedAi::first_capital_is_bare(&game, 0, near));
    let far = capital_of(&game, 2);
    set_strength(&mut game, far, 13.0);
    assert!(
        game.wdist((6, 12), (21, 20)) > FIRST_CAPITAL_BARE_REACH,
        "the third capital stands past the bare reach"
    );
    assert!(!AdvancedAi::first_capital_is_bare(&game, 0, far));
}

// ------------------------------------------------- 4. the deadline's grace

fn at_the_deadline(game: &mut Game, ai: &AdvancedAi) {
    let opening = ai.conquest_opening.as_ref().unwrap();
    game.turn = AdvancedAi::conquest_commit_base(game, opening, ai.conquest_preparation(game));
}

/// The live turn 57: "Releasing the conquest opening on Cree | the commit
/// deadline passed before the force ever assembled", the army held at home
/// by the war on Phoenicia. Under the gene the deadline grants the shipped
/// grace instead, once.
#[test]
fn the_deadline_extends_an_opening_held_away_by_a_faith_war() {
    for gene in [false, true] {
        let (mut game, mut ai, _) = opened(gene);
        at_war(&mut game, 0, 2);
        ai.religious_interception_war = Some((2, game.turn));
        at_the_deadline(&mut game, &ai);
        let base = game.turn;
        ai.maintain_conquest_opening(&mut game, 0);
        match ai.conquest_opening.as_ref() {
            Some(opening) => {
                assert!(gene, "off: the deadline releases");
                assert_eq!(
                    opening.grace_until,
                    Some(base + game.standard_duration(CONQUEST_APPROACHING_GRACE_TURNS))
                );
            }
            None => assert!(!gene, "on: the grace holds the opening"),
        }
        if gene {
            // Once: at the end of the grace it releases.
            game.turn = ai.conquest_opening.as_ref().unwrap().grace_until.unwrap();
            ai.maintain_conquest_opening(&mut game, 0);
            assert!(ai.conquest_opening.is_none());
        }
    }
}

/// The force covers the bill but stands at home: on, the grace; and an
/// opening with neither excuse, or walled past the ceiling, is released as
/// shipped.
#[test]
fn the_deadline_extends_a_covered_bill_and_nothing_else() {
    for gene in [false, true] {
        let (mut game, mut ai, capital) = opened(gene);
        set_strength(&mut game, capital, 15.0);
        bodies(&mut game, "swordsman", (6, 12), 2, 6);
        ai.maintain_conquest_opening(&mut game, 0);
        let opening = ai.conquest_opening.clone().unwrap();
        assert!(ai.conquest_preview_takes_the_city(&game, 0, &opening));
        assert!(opening.assembled.is_none(), "the force is at home");
        at_the_deadline(&mut game, &ai);
        ai.maintain_conquest_opening(&mut game, 0);
        assert_eq!(ai.conquest_opening.is_some(), gene);
    }
    // No excuse: released either way.
    let (mut game, mut ai, _) = opened(true);
    at_the_deadline(&mut game, &ai);
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.conquest_opening.is_none(), "no army, no faith war");
    // Walled past the ceiling: the stand-down is the shipped one.
    let (mut game, mut ai, capital) = opened(true);
    at_war(&mut game, 0, 2);
    ai.religious_interception_war = Some((2, game.turn));
    set_walls(&mut game, capital, 300);
    at_the_deadline(&mut game, &ai);
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.conquest_opening.is_none(), "the walls release it");
}

#[test]
fn the_constants_are_the_ones_the_design_states() {
    assert_eq!(FIRST_CAPITAL_SOFT_STRENGTH, 30.0);
    assert_eq!(FIRST_CAPITAL_SOFT_SETTLER_CITIES, 2);
    assert_eq!(FIRST_CAPITAL_BARE_CITIES, 1);
    assert!((FIRST_CAPITAL_BARE_STRENGTH - 20.0).abs() < 1e-9);
    assert_eq!(FIRST_CAPITAL_BARE_REACH, 14);
    assert_eq!(CAMPAIGN_MIN_BODIES, 3);
}

// --------------------------------------- live 063232Z: loop, rout, objective

/// Live 063232Z t53-58: at war with Maya already, the opening named Wak
/// Kab'nal and released it the same frame nine times. Off, the shipped scan
/// still names it; on, a capital whose owner we already fight is not named.
#[test]
fn a_capital_whose_owner_we_already_fight_is_not_named() {
    for gene in [false, true] {
        let mut game = board(&[(6, 12), (24, 12)]);
        meet_and_explore(&mut game, 1);
        at_war(&mut game, 0, 1);
        let mut ai = seat(gene);
        ai.maintain_conquest_opening(&mut game, 0);
        assert_eq!(ai.conquest_opening.is_some(), !gene, "gene {gene}");
    }
}

/// Live 063232Z t80: 83% of the force at the rally, the preview covered,
/// and a capital of defence 47-56 against Hypaspists of 38. Under the gene a
/// city more than ten above our best melee body at the rally is not struck,
/// whichever rule assembled the force.
#[test]
fn a_city_that_outclasses_the_rally_is_not_struck() {
    let (mut game, mut ai, capital) = opened(true);
    set_strength(&mut game, capital, 60.0);
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    second_city(&mut game);
    bodies(&mut game, "swordsman", rally, 1, 6);
    ai.maintain_conquest_opening(&mut game, 0);
    let opening = ai.conquest_opening.clone().unwrap();
    assert!(ai.first_capital_outclassed(&game, 0, &opening).is_some());
    assert!(ai
        .first_capital_open_window_go(&game, 0, &opening)
        .is_none());
    assert!(!ai.conquest_declaration(&mut game, 0));
    assert!(!game.is_at_war(0, 1), "no strike into an outclassing city");

    // Within the margin the same force declares.
    set_strength(&mut game, capital, 40.0);
    let opening = ai.conquest_opening.clone().unwrap();
    assert!(ai.first_capital_outclassed(&game, 0, &opening).is_none());
    assert!(ai.conquest_declaration(&mut game, 0));
    assert!(game.is_at_war(0, 1));

    // Off: no such veto.
    let (mut game, off, capital) = opened(false);
    set_strength(&mut game, capital, 60.0);
    let opening = off.conquest_opening.clone().unwrap();
    assert!(off.first_capital_outclassed(&game, 0, &opening).is_none());
}

/// Live 063232Z t80-86: "Campaign remains aimed at Palenque" while the
/// strike stood on Wak Kab'nal, and the power gap put the plan in Recovery.
/// Under the gene the declared strike keeps its city and holds the
/// power-gap posture off; a home emergency still outranks it.
#[test]
fn the_declared_strike_keeps_its_city_and_its_posture() {
    for gene in [false, true] {
        let (mut game, mut ai, capital) = opened(gene);
        set_strength(&mut game, capital, 15.0);
        let rally = ai.conquest_opening.as_ref().unwrap().rally;
        second_city(&mut game);
        bodies(&mut game, "swordsman", rally, 1, 6);
        ai.maintain_conquest_opening(&mut game, 0);
        assert!(ai.conquest_declaration(&mut game, 0), "gene {gene}");
        let elsewhere = capital_of(&game, 2);
        assert_eq!(ai.first_capital_strike_running(&game, 0), gene);
        assert_eq!(
            ai.first_capital_strike_objective(&game, 0, Some(elsewhere), false),
            gene.then_some((1, capital))
        );
        assert_eq!(
            ai.first_capital_strike_objective(&game, 0, Some(elsewhere), true),
            None,
            "a home emergency outranks the strike"
        );
        if gene {
            let plan = ai.assess(&game, 0);
            assert_eq!(
                (plan.target_player, plan.target_city),
                (Some(1), Some(capital))
            );
        }
    }
}

// ------------------------------------------- live 071743Z: the token strike

/// Live 071743Z t7: one city, one Warrior and Scouts at the rally, our 35
/// power against Ethiopia's 113, no Ethiopian unit in sight, and a capital
/// of defence 13: the go read the bill as covered and declared. Under the
/// guards no declaration opens on that shape, and each guard refuses alone.
#[test]
fn the_live_ethiopia_shape_is_never_struck() {
    let (mut game, mut ai, capital) = opened(true);
    set_strength(&mut game, capital, 13.0);
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    bodies(&mut game, "warrior", rally, 1, 1);
    bodies(&mut game, "scout", rally, 1, 2);
    let ours = game.military_power(0);
    Arc::make_mut(&mut game.observed_military_power).insert(1, 3.0 * ours);
    ai.maintain_conquest_opening(&mut game, 0);
    let opening = ai.conquest_opening.clone().unwrap();
    let why = ai.first_capital_outclassed(&game, 0, &opening).unwrap();
    assert!(why.contains("1 city"), "{why}");
    assert!(ai
        .first_capital_open_window_go(&game, 0, &opening)
        .is_none());
    assert!(!ai.conquest_declaration(&mut game, 0));
    assert!(!game.is_at_war(0, 1), "no war on the live shape");

    // Two cities: one Warrior and two Scouts are one melee soldier.
    second_city(&mut game);
    let why = ai.first_capital_outclassed(&game, 0, &opening).unwrap();
    assert!(why.contains("1 melee soldier"), "{why}");

    // Three Warriors, but the owner's public power three times ours.
    bodies(&mut game, "warrior", rally, 2, 2);
    ai.maintain_conquest_opening(&mut game, 0);
    let opening = ai.conquest_opening.clone().unwrap();
    let ours = game.military_power(0);
    Arc::make_mut(&mut game.observed_military_power).insert(1, 3.0 * ours);
    let why = ai.first_capital_outclassed(&game, 0, &opening).unwrap();
    assert!(why.contains("times our"), "{why}");

    // The owner at half our power, but three Warriors against strength 28
    // would deal about 196 in three turns against 200 health and 60 healing.
    Arc::make_mut(&mut game.observed_military_power).insert(1, 0.5 * ours);
    set_strength(&mut game, capital, 28.0);
    let why = ai.first_capital_outclassed(&game, 0, &opening).unwrap();
    assert!(why.contains("would deal about"), "{why}");
    assert!(!ai.conquest_declaration(&mut game, 0));
    assert!(!game.is_at_war(0, 1));
}

/// The Cree window that motivated the gene: three cities, four Warriors and
/// a Battering Ram at the rally of an Ancient-walled capital of defence 18,
/// the owner at half our power. The go declares; the shipped share rule
/// (four of six) does not.
#[test]
fn the_cree_shape_declares() {
    for gene in [false, true] {
        let (mut game, mut ai, capital) = opened(gene);
        second_city(&mut game);
        game.found_city_for(0, (12, 4), None);
        set_walls(&mut game, capital, 100);
        set_strength(&mut game, capital, 18.0);
        let rally = ai.conquest_opening.as_ref().unwrap().rally;
        bodies(&mut game, "warrior", rally, 1, 4);
        bodies(&mut game, "battering_ram", rally, 2, 1);
        let ours = game.military_power(0);
        Arc::make_mut(&mut game.observed_military_power).insert(1, 0.5 * ours);
        ai.maintain_conquest_opening(&mut game, 0);
        let opening = ai.conquest_opening.clone().unwrap();
        assert!(
            ai.first_capital_outclassed(&game, 0, &opening).is_none(),
            "gene {gene}"
        );
        assert_eq!(ai.conquest_declaration(&mut game, 0), gene, "gene {gene}");
        assert_eq!(game.is_at_war(0, 1), gene);
    }
}
