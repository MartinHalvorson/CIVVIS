//! Focused tests for `first-capital-before-the-walls`.
//!
//! The seat is built the way the live Domination seat runs the opening: the
//! armed `early-conquest-opening`, `conquest-opening-stays-near`,
//! `early-conquest-stands-down` and `declaration-needs-production-parity`,
//! on Online speed, past the shipped turn-40 naming deadline.

use super::super::test_support::opt_in_off_in_both_controllers;
use super::*;
use crate::ai::advanced::early_conquest::ConquestOpening;
use crate::game::Game;
use crate::name;
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

/// The live Domination seat's opening genes, with or without this one.
fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_early_conquest_opening();
    ai.enable_conquest_opening_stays_near();
    ai.enable_early_conquest_stands_down();
    ai.enable_declaration_needs_production_parity();
    ai.conquest_opening_needs_the_production = false;
    if gene {
        ai.enable_first_capital_before_the_walls();
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
}

fn unit(game: &mut Game, kind: &str, pos: Pos) -> u32 {
    let uid = game.spawn_test_unit(kind, 0, pos);
    let moves = game.unit_max_moves(uid);
    let unit = game.units.get_mut(&uid).unwrap();
    unit.moves_left = moves;
    unit.moved = false;
    uid
}

#[test]
fn first_capital_before_the_walls_is_a_native_opt_in_off_in_both_controllers() {
    opt_in_off_in_both_controllers("first-capital-before-the-walls", |ai| {
        ai.first_capital_before_the_walls
    });
    assert!(!seat(false).first_capital_active());
    assert!(seat(true).first_capital_active());
    let mut culture = AdvancedAi::targeting(VictoryTarget::Culture);
    culture.enable_early_conquest_opening();
    culture.enable_first_capital_before_the_walls();
    assert!(!culture.first_capital_active(), "Domination lane only");
    let mut bare = AdvancedAi::targeting(VictoryTarget::Domination);
    bare.enable_first_capital_before_the_walls();
    assert!(
        !bare.first_capital_active(),
        "rides on early-conquest-opening"
    );
    assert_eq!(
        seat(true).conquest_quota().0 + seat(true).conquest_quota().1,
        seat(false).conquest_quota().0 + seat(false).conquest_quota().1
    );
}

/// The live refusals: 18 tiles is past the 10-tile near reach, the
/// stand-down shuts the opening anyway, and turn 45 is past the shipped
/// turn-40 deadline on Online. The gene names the capital.
#[test]
fn a_domination_seat_opens_on_a_land_capital_eighteen_tiles_out_at_turn_45() {
    let mut game = board(&[at(6, 12), at(24, 12)]);
    meet_and_explore(&mut game, 1);
    let capital = capital_of(&game, 1);
    assert_eq!(game.wdist(at(6, 12), at(24, 12)), 18);

    let mut off = seat(false);
    off.maintain_conquest_opening(&mut game, 0);
    assert!(off.conquest_opening.is_none(), "off: no opening");

    let mut culture = AdvancedAi::targeting(VictoryTarget::Culture);
    culture.enable_early_conquest_opening();
    culture.enable_conquest_opening_stays_near();
    culture.enable_first_capital_before_the_walls();
    culture.maintain_conquest_opening(&mut game, 0);
    assert!(
        culture.conquest_opening.is_none(),
        "another lane: no opening"
    );

    let mut on = seat(true);
    assert_eq!(on.conquest_target(&game, 0), Some((1, capital)));
    on.maintain_conquest_opening(&mut game, 0);
    let opening = on
        .conquest_opening
        .as_ref()
        .expect("on: the opening stands");
    assert_eq!((opening.target, opening.city), (1, capital));
    assert!(
        on.conquest_reservation_open(&game),
        "the reservation is open past the shipped deadline"
    );

    // The capital keeps its Settlers until the empire holds three cities.
    let home = capital_of(&game, 0);
    let counts = EmpireCounts::default();
    assert!(!on.conquest_defers_the_settler(&game, 0, home, &counts, 2, false));
    assert!(on.conquest_defers_the_settler(&game, 0, home, &counts, 3, false));
}

fn at(col: i32, row: i32) -> Pos {
    (col, row)
}

#[test]
fn walls_past_ancient_the_reach_water_and_fog_each_keep_the_capital_off() {
    let on = seat(true);

    // Walls: Ancient is in, Medieval is out.
    let mut game = board(&[at(6, 12), at(24, 12)]);
    meet_and_explore(&mut game, 1);
    let capital = capital_of(&game, 1);
    set_walls(&mut game, capital, 100);
    assert_eq!(on.conquest_target(&game, 0), Some((1, capital)));
    set_walls(&mut game, capital, 200);
    assert_eq!(
        on.conquest_target(&game, 0),
        None,
        "Medieval Walls, no tower"
    );
    let tower_tech = game.rules.units["siege_tower"].tech.unwrap();
    game.players[0].techs.insert(tower_tech);
    assert_eq!(
        on.conquest_target(&game, 0),
        Some((1, capital)),
        "Medieval Walls, a Siege Tower can be trained"
    );
    set_walls(&mut game, capital, 300);
    assert_eq!(on.conquest_target(&game, 0), None, "Renaissance Walls");

    // Reach: 22 tiles is past twenty.
    let mut game = board(&[at(6, 12), at(28, 12)]);
    meet_and_explore(&mut game, 1);
    assert_eq!(on.conquest_target(&game, 0), None, "past the reach");

    // Water: a strait between the two capitals.
    let mut game = board(&[at(6, 12), at(24, 12)]);
    meet_and_explore(&mut game, 1);
    for row in 0..30 {
        if let Some(tile) = game.map.tiles.get_mut(&at(15, row)) {
            tile.terrain = name!("coast");
        }
    }
    assert_eq!(on.conquest_target(&game, 0), None, "across water");

    // Fog: the capital is charted but the ground between is not.
    let mut game = board(&[at(6, 12), at(24, 12)]);
    meet(&mut game, 1);
    let capital = capital_of(&game, 1);
    game.players[0].explored.insert(at(24, 12));
    assert_eq!(on.conquest_target(&game, 0), None, "no charted walk");
    let walk: Vec<Pos> = (6..=24).map(|col| at(col, 12)).collect();
    game.players[0].explored.extend(walk);
    assert_eq!(
        on.conquest_target(&game, 0),
        Some((1, capital)),
        "a charted walk"
    );

    // Not their original capital any more: nothing to open on.
    let mut game = board(&[at(6, 12), at(24, 12)]);
    meet_and_explore(&mut game, 1);
    let capital = capital_of(&game, 1);
    game.cities.get_mut(&capital).unwrap().original_owner = 2;
    assert_eq!(
        on.conquest_target(&game, 0),
        None,
        "not an original capital"
    );
}

#[test]
fn the_force_leads_with_melee_and_leaves_each_city_its_guard() {
    let mut game = board(&[at(6, 12), at(24, 12)]);
    meet_and_explore(&mut game, 1);
    game.found_city_for(0, at(6, 18), None);
    let archer = unit(&mut game, "archer", at(6, 12));
    let warrior_in_capital = unit(&mut game, "warrior", at(7, 12));
    let second_guard = unit(&mut game, "warrior", at(7, 18));
    let free_a = unit(&mut game, "warrior", at(11, 12));
    let free_b = unit(&mut game, "warrior", at(12, 13));

    let off = seat(false);
    assert!(off.first_capital_home_guards(&game, 0).is_empty());
    let on = seat(true);
    assert_eq!(
        on.first_capital_home_guards(&game, 0),
        BTreeSet::from([archer, second_guard]),
        "the capital keeps its shooter, the second city its warrior"
    );
    let counts = on.counts(&game, 0);
    assert_eq!(
        AdvancedAi::conquest_reservation_shortfall(&counts),
        (3, 0),
        "shipped: four shooters and two melee bodies, guards counted"
    );
    assert_eq!(
        on.first_capital_shortfall(&game, 0, &counts),
        (2, 1),
        "two shooters and four melee bodies on top of the guards"
    );

    let mut on = seat(true);
    on.maintain_conquest_opening(&mut game, 0);
    let force = &on.conquest_opening.as_ref().expect("opened").force;
    assert_eq!(
        *force,
        BTreeSet::from([warrior_in_capital, free_a, free_b]),
        "no guard marches"
    );
}

/// Three guarded cities plus exactly the nominal strike quota. One of those
/// bodies shadows a Settler, so it is unavailable to the actual strike.
fn escorted_quota(kind: &str) -> (Game, AdvancedAi, u32, u32) {
    let mut game = board(&[at(6, 12), at(24, 12)]);
    meet_and_explore(&mut game, 1);
    game.found_city_for(0, at(6, 18), None);
    game.found_city_for(0, at(11, 18), None);
    for pos in [at(6, 12), at(6, 18), at(11, 18)] {
        unit(&mut game, "archer", pos);
    }
    let mut bodies = Vec::new();
    for pos in [at(12, 12), at(13, 12), at(14, 12), at(15, 12)] {
        bodies.push(unit(&mut game, "warrior", pos));
    }
    for pos in [at(12, 14), at(13, 14)] {
        bodies.push(unit(&mut game, "archer", pos));
    }
    let escort = bodies
        .into_iter()
        .find(|uid| game.units[uid].kind == kind)
        .unwrap();
    let settler = unit(&mut game, "settler", at(15, 14));
    let mut ai = seat(true);
    ai.settler_guards.insert(settler, escort);
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.conquest_opening.is_some(), "fixture: named opening");
    assert!(
        !ai.conquest_opening
            .as_ref()
            .unwrap()
            .force
            .contains(&escort),
        "the actual strike already excludes its civilian escort"
    );
    let capital = capital_of(&game, 0);
    (game, ai, capital, escort)
}

#[test]
fn a_settler_escort_cannot_fill_the_assault_production_quota() {
    for (kind, expected) in [("warrior", (0, 1)), ("archer", (1, 0))] {
        let (game, ai, capital, _) = escorted_quota(kind);
        let counts = ai.counts(&game, 0);
        assert_eq!(
            ai.first_capital_shortfall(&game, 0, &counts),
            expected,
            "{kind}"
        );
        assert!(
            ai.conquest_reservation(&game, 0, capital, &game.rules.units[kind], &counts, false)
                > 0.0,
            "the capital must still train the missing free {kind}"
        );
    }
}

#[test]
fn queued_replacements_still_count_and_the_off_arm_keeps_its_census() {
    let (mut game, ai, capital, _) = escorted_quota("warrior");
    let counts = ai.counts(&game, 0);
    assert_eq!(
        seat(false).first_capital_shortfall(&game, 0, &counts),
        (0, 0)
    );
    assert_eq!(ai.first_capital_shortfall(&game, 0, &counts), (0, 1));
    game.apply(
        0,
        &crate::game::Action::Produce {
            city: capital,
            item: crate::game::Item::Unit {
                unit: name!("warrior"),
            },
        },
    )
    .unwrap();
    assert_eq!(
        ai.first_capital_shortfall(&game, 0, &ai.counts(&game, 0)),
        (0, 0),
        "the queued replacement prevents another city claiming the same slot"
    );
}

#[test]
fn reserved_bodies_use_the_same_classification_as_the_empire_census() {
    let (mut game, ai, _, escort) = escorted_quota("archer");
    // The census counts a land siege weapon as ranged. Even though the
    // opening would not train it as its Archer, its separate civilian
    // assignment cannot satisfy the free ranged-body quota.
    game.units.get_mut(&escort).unwrap().kind = name!("catapult");
    assert_eq!(
        ai.first_capital_shortfall(&game, 0, &ai.counts(&game, 0)),
        (1, 0)
    );
}

#[test]
fn stale_foreign_and_recon_escort_bindings_do_not_remove_free_soldiers() {
    let (mut game, mut ai, _, _) = escorted_quota("warrior");
    ai.settler_guards.clear();
    let scout = unit(&mut game, "scout", at(15, 15));
    let foreign = game.spawn_test_unit("warrior", 1, at(22, 12));
    ai.settler_guards.insert(900_001, scout);
    ai.settler_guards.insert(900_002, foreign);
    ai.settler_guards.insert(900_003, u32::MAX);
    assert_eq!(
        ai.first_capital_shortfall(&game, 0, &ai.counts(&game, 0)),
        (0, 0),
        "Scouts are already excluded; foreign and missing units never entered our census"
    );
}

#[test]
fn walls_rising_past_ancient_release_the_opening_unless_a_tower_opens_them() {
    let mut game = board(&[at(6, 12), at(24, 12)]);
    meet_and_explore(&mut game, 1);
    let capital = capital_of(&game, 1);
    let mut on = seat(true);
    on.maintain_conquest_opening(&mut game, 0);
    assert!(on.conquest_opening.is_some());

    // Medieval Walls with a Siege Tower held: the opening stands.
    set_walls(&mut game, capital, 200);
    let tower = unit(&mut game, "siege_tower", at(6, 12));
    game.turn += 1;
    on.maintain_conquest_opening(&mut game, 0);
    assert!(
        on.conquest_opening.is_some(),
        "a tower opens Medieval Walls"
    );

    // Without the tower it stands down.
    game.remove_unit(tower);
    game.turn += 1;
    on.maintain_conquest_opening(&mut game, 0);
    assert!(
        on.conquest_opening.is_none(),
        "walls past Ancient, no tower"
    );

    // Renaissance Walls: a tower does not help.
    let mut game = board(&[at(6, 12), at(24, 12)]);
    meet_and_explore(&mut game, 1);
    let capital = capital_of(&game, 1);
    let mut on = seat(true);
    on.maintain_conquest_opening(&mut game, 0);
    let opening = on.conquest_opening.clone().expect("opened");
    unit(&mut game, "siege_tower", at(6, 12));
    set_walls(&mut game, capital, 300);
    assert!(on.first_capital_walled_out(&game, 0, &opening).is_some());
    assert!(
        seat(false)
            .first_capital_walled_out(&game, 0, &opening)
            .is_none(),
        "off: never"
    );
}

#[test]
fn the_capital_can_be_named_until_turn_63_on_online() {
    assert_eq!(
        crate::setup::GameSpeed::Online.scale_turns(FIRST_CAPITAL_NAMING_DEADLINE),
        63
    );
    for (turn, opens) in [(62, true), (63, false)] {
        let mut game = board(&[at(6, 12), at(24, 12)]);
        meet_and_explore(&mut game, 1);
        game.turn = turn;
        let mut on = seat(true);
        on.conquest_first_turn = Some(1);
        on.maintain_conquest_opening(&mut game, 0);
        assert_eq!(on.conquest_opening.is_some(), opens, "turn {turn}");
    }
}

/// The capital fell to a war trading at 0 kills for 2 losses. The shipped
/// opening asks for terms; the gene carries the campaign to the next city
/// with the field army near the captured capital.
#[test]
fn after_the_capital_falls_the_campaign_goes_on_at_any_rate() {
    for gene in [false, true] {
        let mut game = board(&[at(6, 12), at(24, 12)]);
        game.found_city_for(1, at(30, 12), None);
        meet_and_explore(&mut game, 1);
        let capital = capital_of(&game, 1);
        let next = game
            .player_city_ids(1)
            .into_iter()
            .find(|cid| *cid != capital)
            .unwrap();
        let survivor = unit(&mut game, "warrior", at(23, 12));
        let reinforcement = unit(&mut game, "warrior", at(20, 12));
        let mut ai = seat(gene);
        ai.conquest_opening = Some(ConquestOpening {
            target: 1,
            city: capital,
            opened: 40,
            preparing_since: Some(40),
            grace_until: None,
            rally: at(21, 12),
            force: BTreeSet::from([survivor]),
            assembled: Some(42),
            declared: Some(43),
            kills_at_war: 0,
            losses: 2,
            taken: 0,
        });
        let city = game.cities.get_mut(&capital).unwrap();
        city.owner = 0;
        city.is_capital = false;
        ai.maintain_conquest_opening(&mut game, 0);
        if gene {
            let opening = ai.conquest_opening.as_ref().expect("the campaign goes on");
            assert_eq!((opening.city, opening.taken), (next, 1));
            assert!(opening.force.contains(&survivor));
            assert!(opening.force.contains(&reinforcement), "reinforced");
        } else {
            assert!(ai.conquest_opening.is_none(), "off: the opening closes");
        }
    }
}

#[test]
fn our_friendship_offer_spares_the_target_only_while_the_window_is_open() {
    let mut game = board(&[at(6, 12), at(24, 12), at(6, 24)]);
    meet_and_explore(&mut game, 1);
    meet(&mut game, 2);
    // Seat 2's capital is nearer but walled past what a Ram or Tower opens.
    let walled = capital_of(&game, 2);
    set_walls(&mut game, walled, 300);
    assert!(!seat(false).first_capital_spares_friendship(&game, 0, 1));
    let mut on = seat(true);
    assert!(
        on.first_capital_spares_friendship(&game, 0, 1),
        "the target"
    );
    assert!(
        !on.first_capital_spares_friendship(&game, 0, 2),
        "a rival whose capital is not the target is not spared"
    );
    on.maintain_conquest_opening(&mut game, 0);
    assert!(
        on.first_capital_spares_friendship(&game, 0, 1),
        "the opening"
    );
    let mut late = seat(true);
    game.turn = 63;
    assert!(
        !late.first_capital_spares_friendship(&game, 0, 1),
        "past t63"
    );
    late.conquest_first_turn = Some(1);
    late.maintain_conquest_opening(&mut game, 0);
    assert!(late.conquest_opening.is_none());
}
