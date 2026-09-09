//! Focused tests for `early-conquest-opening`.
//!
//! Every behavioural test asserts the OFF behaviour on the same board first,
//! so each one is also a proof that the gene changes nothing while it ships
//! off.

use super::super::test_support::opt_in_off_in_both_controllers;
use super::*;
use crate::game::Game;
use crate::name;

/// A flat board of `capitals.len()` empires with every starting unit
/// cleared, nobody at war, on standard-speed turn 20 — inside the opening
/// window. Nothing is explored and nobody has met: each test grants exactly
/// the knowledge it is about.
fn board(capitals: &[Pos]) -> Game {
    let mut game = Game::new_full(capitals.len(), 40, 24, 90_909, 1_000, 0, false);
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
        assert!(
            game.map.tiles.get(pos).is_some(),
            "capital {pos:?} is on the map"
        );
        game.found_city_for(pid, *pos, None);
    }
    game.at_war.clear();
    game.turn = 20;
    game.current = 0;
    game
}

/// Seat 0 has met `rival` and explored every tile of the board, which is the
/// most generous knowledge state a fog-honest scan can be given.
fn meet_and_explore(game: &mut Game, rival: usize) {
    game.players[0].met.insert(rival);
    game.players[rival].met.insert(0);
    let tiles: Vec<Pos> = game.map.tiles.keys().copied().collect();
    game.players[0].explored.extend(tiles);
}

fn at(col: i32, row: i32) -> Pos {
    (col, row)
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_early_conquest_opening();
    ai
}

fn fresh(game: &mut Game, uid: u32) {
    let moves = game.unit_max_moves(uid);
    let unit = game.units.get_mut(&uid).unwrap();
    unit.moves_left = moves;
    unit.attacks_left = 1;
    unit.moved = false;
    unit.acted = false;
}

/// `count` of our warriors on the ring `radius` around `around`.
fn bodies(
    game: &mut Game,
    pid: usize,
    kind: &str,
    around: Pos,
    radius: i32,
    count: usize,
) -> Vec<u32> {
    let ring: Vec<Pos> = game
        .wring(around, radius)
        .into_iter()
        .filter(|pos| game.city_at(*pos).is_none() && game.unit_ids_at(*pos).is_empty())
        .collect();
    let mut spawned = Vec::new();
    for pos in ring.into_iter().take(count) {
        let uid = game.spawn_test_unit(kind, pid, pos);
        fresh(game, uid);
        spawned.push(uid);
    }
    assert_eq!(spawned.len(), count, "the ring had room for the force");
    spawned
}

// ----------------------------------------------------------------- registry

#[test]
fn early_conquest_opening_is_a_native_opt_in_off_in_both_controllers() {
    opt_in_off_in_both_controllers("early-conquest-opening", |ai| ai.early_conquest_opening);
}

#[test]
fn the_named_constants_are_the_ones_the_design_states() {
    assert_eq!(CONQUEST_REACH_TILES, 12);
    assert_eq!(CONQUEST_MAX_RIVAL_CITIES, 3);
    assert_eq!(CONQUEST_COMMIT_DEADLINE, 60);
    assert_eq!(CONQUEST_RANGED, 3);
    assert_eq!(CONQUEST_MELEE, 2);
    assert_eq!(CONQUEST_ABANDON_TURNS, 20);
    assert_eq!(CONQUEST_KILLS_PER_LOSS_FLOOR, 1.0);
}

// ------------------------------------------------------------------- target

#[test]
fn the_target_is_a_met_rivals_explored_city_in_reach_and_nothing_else() {
    // Seat 1's capital is eight tiles away; seat 2's is sixteen, which is
    // past CONQUEST_REACH_TILES even around the cylinder.
    let mut game = board(&[at(6, 12), at(14, 12), at(30, 12)]);
    let ai = armed();
    let off = AdvancedAi::new();

    // Nothing met, nothing explored: no target for either controller.
    assert_eq!(
        ai.conquest_target(&game, 0),
        None,
        "an unmet rival is no target"
    );
    assert_eq!(off.conquest_target(&game, 0), None);

    meet_and_explore(&mut game, 1);
    let city = game.player_city_ids(1)[0];
    assert_eq!(
        ai.conquest_target(&game, 0),
        Some((1, city)),
        "the met rival's explored capital in reach is the target"
    );
    assert_eq!(
        off.conquest_target(&game, 0),
        None,
        "off, there is never a target"
    );

    // The far rival is met and explored too, and is still out of reach.
    meet_and_explore(&mut game, 2);
    assert_eq!(
        ai.conquest_target(&game, 0),
        Some((1, city)),
        "a rival beyond CONQUEST_REACH_TILES is not opened against"
    );
}

#[test]
fn an_unexplored_city_does_not_exist_and_a_fourth_city_takes_the_rival_off_the_board() {
    let mut game = board(&[at(6, 12), at(14, 12)]);
    let ai = armed();
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);

    // Met, but nothing of theirs explored: the fog-honest scan finds nothing.
    assert_eq!(
        ai.conquest_target(&game, 0),
        None,
        "a city on a tile we have never seen is not a known city"
    );

    let capital = game.player_city_ids(1)[0];
    game.players[0].explored.insert(game.cities[&capital].pos);
    assert_eq!(ai.conquest_target(&game, 0), Some((1, capital)));

    // Three known cities is still an opening; the fourth is a war.
    for (index, pos) in [at(13, 9), at(16, 14), at(12, 15)].into_iter().enumerate() {
        let cid = game.found_city_for(1, pos, None);
        game.players[0].explored.insert(game.cities[&cid].pos);
        let known = AdvancedAi::conquest_known_cities(&game, 0, 1).len();
        assert_eq!(known, index + 2);
        if known <= CONQUEST_MAX_RIVAL_CITIES {
            assert_eq!(
                ai.conquest_target(&game, 0),
                Some((1, capital)),
                "{known} known cities is still a small neighbour"
            );
        } else {
            assert_eq!(
                ai.conquest_target(&game, 0),
                None,
                "{known} known cities is past CONQUEST_MAX_RIVAL_CITIES"
            );
        }
    }
}

#[test]
fn the_capital_is_preferred_and_the_lighter_visible_garrison_breaks_the_tie() {
    let mut game = board(&[at(6, 12), at(14, 12)]);
    meet_and_explore(&mut game, 1);
    let capital = game.player_city_ids(1)[0];
    let second = game.found_city_for(1, at(12, 15), None);
    let third = game.found_city_for(1, at(16, 15), None);
    let ai = armed();

    assert_eq!(
        ai.conquest_target(&game, 0),
        Some((1, capital)),
        "the capital outranks a lighter second city"
    );

    // Take the capital out of the running by giving it to a third party's
    // reach test: demote it instead, so the ranking falls to the garrisons.
    game.cities.get_mut(&capital).unwrap().is_capital = false;
    let heavy = game.cities[&second].pos;
    let mut ring = game
        .wring(heavy, 1)
        .into_iter()
        .filter(|pos| game.unit_ids_at(*pos).is_empty());
    let post = ring.next().expect("a tile beside the second city");
    let eyes = ring.next().expect("a second tile beside the second city");
    let uid = game.spawn_test_unit("warrior", 1, post);
    fresh(&mut game, uid);
    // A scout of ours beside it, so the garrison is one we can actually SEE:
    // the ranking reads the visibility frame, not the board.
    let scout = game.spawn_test_unit("scout", 0, eyes);
    fresh(&mut game, scout);
    assert!(
        game.player_can_see(0, post),
        "the defender is inside our own vision"
    );

    let (_, chosen) = ai.conquest_target(&game, 0).expect("a target remains");
    assert_ne!(
        chosen, second,
        "the city with a visible defender beside it is not the lightest"
    );
    assert!(chosen == capital || chosen == third);
}

// -------------------------------------------------------------- reservation

/// An opening committed against seat 1, before the deadline.
fn opened(game: &mut Game) -> AdvancedAi {
    meet_and_explore(game, 1);
    let mut ai = armed();
    ai.maintain_conquest_opening(game, 0);
    assert!(ai.conquest_opening.is_some(), "the opening committed");
    ai
}

#[test]
fn the_reservation_prices_the_missing_bodies_in_the_capital_and_nowhere_else() {
    let mut game = board(&[at(6, 12), at(14, 12)]);
    let second = game.found_city_for(0, at(6, 17), None);
    let ai = opened(&mut game);
    let off = AdvancedAi::new();
    let capital = game.player_city_ids(0)[0];
    let capital = if game.cities[&capital].is_capital {
        capital
    } else {
        second
    };
    let counts = EmpireCounts::default();
    let warrior = &game.rules.units[&name!("warrior")];
    let slinger = &game.rules.units[&name!("slinger")];
    let builder = &game.rules.units[&name!("builder")];

    assert!(
        ai.conquest_reservation(&game, 0, capital, warrior, &counts, false) > 0.0,
        "a melee body is reserved"
    );
    assert!(
        ai.conquest_reservation(&game, 0, capital, slinger, &counts, false) > 0.0,
        "a Slinger counts as the force's shooter"
    );
    assert_eq!(
        ai.conquest_reservation(&game, 0, capital, builder, &counts, false),
        0.0,
        "a civilian is not a strike body"
    );
    assert_eq!(
        ai.conquest_reservation(&game, 0, second, warrior, &counts, false),
        0.0,
        "only the capital reserves"
    );
    assert_eq!(
        ai.conquest_reservation(&game, 0, capital, warrior, &counts, true),
        0.0,
        "the defence sentinel outranks the whole opening"
    );
    assert_eq!(
        off.conquest_reservation(&game, 0, capital, warrior, &counts, false),
        0.0,
        "off, nothing is reserved"
    );

    // Once the force is complete the reservation stops asking.
    let full = EmpireCounts {
        ranged: CONQUEST_RANGED,
        melee: CONQUEST_MELEE,
        ..EmpireCounts::default()
    };
    assert_eq!(
        ai.conquest_reservation(&game, 0, capital, warrior, &full, false),
        0.0
    );
    assert_eq!(
        ai.conquest_reservation(&game, 0, capital, slinger, &full, false),
        0.0
    );
}

#[test]
fn the_census_scouts_do_not_fill_the_melee_half() {
    let scouted = EmpireCounts {
        melee: 2,
        scouts: 2,
        ..EmpireCounts::default()
    };
    assert_eq!(
        AdvancedAi::conquest_reservation_shortfall(&scouted),
        (CONQUEST_RANGED, CONQUEST_MELEE),
        "two Scouts filed under melee are not two strike bodies"
    );
    let real = EmpireCounts {
        melee: 3,
        scouts: 1,
        ranged: 1,
        ..EmpireCounts::default()
    };
    assert_eq!(
        AdvancedAi::conquest_reservation_shortfall(&real),
        (CONQUEST_RANGED - 1, 0)
    );
}

#[test]
fn the_second_settler_waits_and_the_first_one_never_does() {
    let mut game = board(&[at(6, 12), at(14, 12)]);
    let ai = opened(&mut game);
    let off = AdvancedAi::new();
    let capital = game.player_city_ids(0)[0];
    let counts = EmpireCounts::default();

    assert!(
        !ai.conquest_defers_the_settler(&game, 0, capital, &counts, 1, false),
        "an empire of one city never defers its first Settler"
    );
    assert!(
        ai.conquest_defers_the_settler(&game, 0, capital, &counts, 2, false),
        "with a second city standing, the reservation holds the next Settler"
    );
    assert!(
        !ai.conquest_defers_the_settler(&game, 0, capital, &counts, 2, true),
        "a threatened capital builds what defends it"
    );
    assert!(
        !off.conquest_defers_the_settler(&game, 0, capital, &counts, 2, false),
        "off, no Settler is ever deferred"
    );

    let full = EmpireCounts {
        ranged: CONQUEST_RANGED,
        melee: CONQUEST_MELEE,
        ..EmpireCounts::default()
    };
    assert!(
        !ai.conquest_defers_the_settler(&game, 0, capital, &full, 2, false),
        "a filled reservation releases the Settler"
    );

    // And the window closes: past the deadline nothing is reserved at all.
    let mut late = game.clone();
    late.turn = late.standard_duration(CONQUEST_COMMIT_DEADLINE);
    assert!(!ai.conquest_defers_the_settler(&late, 0, capital, &counts, 2, false));
}

#[test]
fn the_research_credit_chases_the_shooter_node_only_while_the_reservation_is_open() {
    let mut game = board(&[at(6, 12), at(14, 12)]);
    let ai = opened(&mut game);
    let off = AdvancedAi::new();

    assert_eq!(
        ai.conquest_research_value(&game, 0, "archery"),
        CONQUEST_RESEARCH,
        "the node that unlocks the shooter is chased"
    );
    assert_eq!(off.conquest_research_value(&game, 0, "archery"), 0.0);
    assert_eq!(
        ai.conquest_research_value(&game, 0, "pottery"),
        0.0,
        "a node that does not lead to the shooter earns nothing"
    );

    game.players[0].techs.insert(name!("archery"));
    assert_eq!(
        ai.conquest_research_value(&game, 0, "archery"),
        0.0,
        "the credit stops once the node is held"
    );
}

// -------------------------------------------------------------- declaration

#[test]
fn the_declaration_waits_for_the_assembly_share_and_then_for_the_bill() {
    let mut game = board(&[at(6, 12), at(14, 12)]);
    let mut ai = opened(&mut game);
    let city = game.player_city_ids(1)[0];
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    assert!(
        (CONQUEST_RALLY_MIN..=CONQUEST_RALLY_MAX)
            .contains(&game.wdist(rally, game.cities[&city].pos)),
        "the rally stands on the target's own ring"
    );

    // No army at all: nothing is assembled and nothing is declared.
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(!ai.conquest_declaration(&mut game, 0));
    assert!(!game.is_at_war(0, 1));
    assert_eq!(ai.conquest_opening.as_ref().unwrap().assembled, None);

    // A force standing on the rally: assembled, and the bill is covered
    // because the rival's capital is empty.
    bodies(
        &mut game,
        0,
        "warrior",
        rally,
        1,
        CONQUEST_RANGED + CONQUEST_MELEE,
    );
    ai.maintain_conquest_opening(&mut game, 0);
    let opening = ai.conquest_opening.as_ref().unwrap();
    assert!(opening.assembled.is_some(), "the force stands at the rally");
    assert!(AdvancedAi::conquest_assembled_share(&game, opening) >= CONQUEST_ASSEMBLY_SHARE);
    assert!(
        ai.conquest_preview_takes_the_city(&game, 0, opening),
        "an undefended capital's bill is covered by five bodies"
    );

    assert!(ai.conquest_declaration(&mut game, 0), "the war opens");
    assert!(game.is_at_war(0, 1));
    let opening = ai.conquest_opening.as_ref().unwrap();
    assert_eq!(opening.declared, Some(game.turn));

    // And the campaign is pinned, which is the whole handoff.
    assert_eq!(ai.campaign_target(&game, 0), Some(1));
    assert_eq!(ai.campaign_objective_city(&game, 0, Some(1)), Some(city));
    assert!(ai.conquest_owns_the_campaign());
}

#[test]
fn a_bill_the_force_cannot_cover_is_not_declared_on() {
    let mut game = board(&[at(6, 12), at(14, 12)]);
    let mut ai = opened(&mut game);
    let city = game.player_city_ids(1)[0];
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    bodies(
        &mut game,
        0,
        "warrior",
        rally,
        1,
        CONQUEST_RANGED + CONQUEST_MELEE,
    );
    // A garrison the five bodies cannot pay for.
    let centre = game.cities[&city].pos;
    bodies(&mut game, 1, "warrior", centre, 1, 6);

    ai.maintain_conquest_opening(&mut game, 0);
    let opening = ai.conquest_opening.as_ref().unwrap();
    assert!(opening.assembled.is_some());
    assert!(
        !ai.conquest_preview_takes_the_city(&game, 0, opening),
        "the preview refuses a city the force cannot take"
    );
    assert!(!ai.conquest_declaration(&mut game, 0));
    assert!(
        !game.is_at_war(0, 1),
        "no war is opened on a bill we cannot pay"
    );
}

#[test]
fn a_force_that_never_covers_the_bill_releases_after_the_patience_window() {
    let mut game = board(&[at(6, 12), at(14, 12)]);
    let mut ai = opened(&mut game);
    let city = game.player_city_ids(1)[0];
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    bodies(
        &mut game,
        0,
        "warrior",
        rally,
        1,
        CONQUEST_RANGED + CONQUEST_MELEE,
    );
    let centre = game.cities[&city].pos;
    bodies(&mut game, 1, "warrior", centre, 1, 6);
    ai.maintain_conquest_opening(&mut game, 0);
    let assembled = ai
        .conquest_opening
        .as_ref()
        .unwrap()
        .assembled
        .expect("the force assembled");

    game.turn = assembled + game.standard_duration(CONQUEST_ABANDON_TURNS) - 1;
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(
        ai.conquest_opening.is_some(),
        "one turn inside the window the opening stands"
    );

    game.turn = assembled + game.standard_duration(CONQUEST_ABANDON_TURNS);
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(
        ai.conquest_opening.is_none(),
        "the reservation is released when the patience window closes"
    );
    assert!(!game.is_at_war(0, 1));

    // And that was the game's one attempt: the next turn, still inside the
    // commit window, does not name the same city again and re-open the
    // reservation that was just released.
    assert!(
        ai.conquest_closed,
        "an assembled opening that released is closed"
    );
    game.turn += 1;
    assert!(game.turn < game.standard_duration(CONQUEST_COMMIT_DEADLINE));
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(
        ai.conquest_opening.is_none(),
        "a released opening does not come back and hold the Settler again"
    );
    let capital = game.player_city_ids(0)[0];
    assert!(!ai.conquest_defers_the_settler(&game, 0, capital, &EmpireCounts::default(), 2, false));
}

/// A force that assembled, declared and then died: the opening asks for
/// terms and stands down, so a dead force cannot pin the campaign to a city
/// nobody is marching on for the rest of the game.
#[test]
fn a_force_that_is_wholly_lost_asks_for_terms_and_releases() {
    let mut game = board(&[at(6, 12), at(14, 12)]);
    let mut ai = opened(&mut game);
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    let force = bodies(
        &mut game,
        0,
        "warrior",
        rally,
        1,
        CONQUEST_RANGED + CONQUEST_MELEE,
    );
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.conquest_declaration(&mut game, 0));
    assert!(ai.conquest_owns_the_campaign());

    for uid in force {
        game.remove_unit(uid);
    }
    ai.maintain_conquest_opening(&mut game, 0);
    assert_eq!(
        ai.conquest_opening.as_ref().map(|opening| opening.losses),
        None,
        "the opening is released once the whole force is gone"
    );
    assert!(
        ai.peace_offers.contains(&1),
        "and it asks the peace desk for terms"
    );
    assert!(!ai.conquest_owns_the_campaign());
    assert!(ai.conquest_closed);
}

/// The war ends by a road the opening did not choose — a peace accepted by
/// the shipped desk, a truce — and the opening ends with it, instead of
/// owning the campaign plan forever with no war to fight.
#[test]
fn an_opening_whose_war_has_ended_releases() {
    let mut game = board(&[at(6, 12), at(14, 12)]);
    let mut ai = opened(&mut game);
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    bodies(
        &mut game,
        0,
        "warrior",
        rally,
        1,
        CONQUEST_RANGED + CONQUEST_MELEE,
    );
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.conquest_declaration(&mut game, 0));
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(
        ai.conquest_owns_the_campaign(),
        "at war, the opening owns the plan"
    );

    game.at_war.remove(&(0, 1));
    game.at_war.remove(&(1, 0));
    assert!(!game.is_at_war(0, 1));
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.conquest_opening.is_none(), "peace releases the opening");
    assert!(!ai.conquest_owns_the_campaign());
    assert!(ai.conquest_closed, "and the game has had its attempt");
}

/// A rival that becomes a friend while the force is still being raised is
/// no longer a target, and the reservation goes with it rather than holding
/// the Settler for a war that can no longer be declared.
#[test]
fn a_rival_that_stops_being_a_legal_target_releases_the_reservation() {
    let mut game = board(&[at(6, 12), at(14, 12)]);
    let mut ai = opened(&mut game);
    let until = game.turn + 30;
    game.players[0].friends_until.insert(1, until);
    game.players[1].friends_until.insert(0, until);
    assert!(game.are_friends(0, 1));
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(
        ai.conquest_opening.is_none(),
        "a friend is not a target, and the reservation is released"
    );
    assert!(
        !ai.conquest_closed,
        "an opening released before it assembled leaves the door open"
    );
}

/// The gene's flag alone does not hand the shared campaign plan to
/// `city_campaign.rs`: until the opening has DECLARED, the shipped
/// maintenance clears the plan exactly as it does with the gene off, and
/// never runs city-campaign v1's own planner.
#[test]
fn the_flag_alone_does_not_switch_on_the_shipped_campaign_planner() {
    let mut game = board(&[at(6, 12), at(14, 12)]);
    let mut ai = opened(&mut game);
    let mut off = AdvancedAi::new();
    assert!(!ai.conquest_owns_the_campaign(), "nothing declared yet");
    assert!(
        !ai.city_campaign_active(),
        "the family predicate is the shipped one until the opening declares"
    );
    assert!(!off.city_campaign_active());

    ai.maintain_city_campaign(&mut game, 0);
    off.maintain_city_campaign(&mut game, 0);
    assert_eq!(
        ai.campaign, None,
        "the opening's flag draws no campaign plan"
    );
    assert_eq!(off.campaign, None);
    assert!(!ai.city_campaign_stands(&game, 0));
    assert_eq!(ai.campaign_target(&game, 0), None);

    // Declared, the opening owns the plan and the predicate accepts it.
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    bodies(
        &mut game,
        0,
        "warrior",
        rally,
        1,
        CONQUEST_RANGED + CONQUEST_MELEE,
    );
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.conquest_declaration(&mut game, 0));
    assert!(ai.city_campaign_active());
    assert!(ai.city_campaign_stands(&game, 0));
    ai.maintain_city_campaign(&mut game, 0);
    assert!(
        ai.campaign.is_some(),
        "the shipped maintenance leaves the pinned plan alone"
    );
}

/// A guard bound to a Settler is never a strike body, so the vision guard
/// never scores an escort's tiles and the assembly share never waits on it.
#[test]
fn a_settlers_bound_guard_is_never_in_the_force() {
    let mut game = board(&[at(6, 12), at(14, 12)]);
    let mut ai = opened(&mut game);
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    // Four bodies on the rally and one more, the nearest of all, that is a
    // Settler's guard.
    bodies(
        &mut game,
        0,
        "warrior",
        rally,
        1,
        CONQUEST_RANGED + CONQUEST_MELEE - 1,
    );
    let settler = game.spawn_test_unit("settler", 0, rally);
    let guard = game.spawn_test_unit("warrior", 0, rally);
    fresh(&mut game, guard);
    ai.settler_guards.insert(settler, guard);

    ai.maintain_conquest_opening(&mut game, 0);
    let force = &ai.conquest_opening.as_ref().unwrap().force;
    assert!(!force.contains(&guard), "the escort is not conscripted");
    assert_eq!(force.len(), CONQUEST_RANGED + CONQUEST_MELEE - 1);
    let blind = crate::world::TileBits::default();
    let lone = game
        .wring(rally, 6)
        .into_iter()
        .find(|pos| game.unit_ids_at(*pos).is_empty() && game.city_at(*pos).is_none())
        .expect("an empty tile far from the force");
    assert_eq!(
        ai.conquest_blind_tile_penalty(&game, 0, guard, lone, &blind),
        0.0,
        "an escort is never scored by the vision guard"
    );
}

/// The declaration honours the shipped vetoes: `one-war-at-a-time` holds it
/// while another major war burns, and `war-needs-a-treasury` holds it while
/// the treasury cannot carry a war. Both are exact no-ops off.
#[test]
fn the_declaration_honours_one_war_at_a_time_and_the_treasury() {
    let mut game = board(&[at(6, 12), at(14, 12), at(30, 12)]);
    let mut ai = opened(&mut game);
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    bodies(
        &mut game,
        0,
        "warrior",
        rally,
        1,
        CONQUEST_RANGED + CONQUEST_MELEE,
    );
    ai.maintain_conquest_opening(&mut game, 0);
    let opening = ai.conquest_opening.clone().unwrap();
    assert!(opening.assembled.is_some());
    assert!(ai.conquest_preview_takes_the_city(&game, 0, &opening));

    // A war with seat 2 already burning, and one war at a time.
    game.players[0].met.insert(2);
    game.players[2].met.insert(0);
    game.at_war.insert((0, 2));
    game.at_war.insert((2, 0));
    ai.enable_one_war_at_a_time();
    ai.one_war_observe(&game, 0);
    assert!(ai.one_war_holds_declaration(&game, 0, 1));
    assert!(
        !ai.conquest_declaration(&mut game, 0),
        "held: one war at a time"
    );
    assert!(!game.is_at_war(0, 1));
    ai.disable_one_war_at_a_time();
    game.at_war.remove(&(0, 2));
    game.at_war.remove(&(2, 0));

    // An empty treasury running a deficit, and a war that needs one.
    ai.enable_war_needs_a_treasury();
    game.players[0].gold = 17.0;
    game.players[0].gold_per_turn = -7.0;
    assert!(!ai.war_is_affordable(&game, 0));
    assert!(!ai.conquest_declaration(&mut game, 0), "held: no treasury");
    assert!(!game.is_at_war(0, 1));
    ai.disable_war_needs_a_treasury();

    // Both vetoes lifted: the same board declares.
    assert!(ai.conquest_declaration(&mut game, 0));
    assert!(game.is_at_war(0, 1));
}

#[test]
fn an_opening_that_never_assembles_expires_at_the_commit_deadline() {
    let mut game = board(&[at(6, 12), at(14, 12)]);
    let mut ai = opened(&mut game);
    game.turn = game.standard_duration(CONQUEST_COMMIT_DEADLINE);
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.conquest_opening.is_none());
    // And it does not simply re-open on the next turn.
    game.turn += 1;
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.conquest_opening.is_none());
}

// ------------------------------------------------------------- vision guard

#[test]
fn a_strike_body_is_charged_for_a_tile_beside_unseen_ground_unless_a_friend_stands_with_it() {
    let mut game = board(&[at(6, 12), at(14, 12)]);
    let mut ai = opened(&mut game);
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    let force = bodies(
        &mut game,
        0,
        "warrior",
        rally,
        1,
        CONQUEST_RANGED + CONQUEST_MELEE,
    );
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(
        ai.conquest_opening
            .as_ref()
            .unwrap()
            .force
            .contains(&force[0]),
        "the nearest bodies are the strike force"
    );

    let tile = game.units[&force[0]].pos;
    let seen = game.player_vision_frame(0);
    // A frame in which the seat sees nothing at all: every neighbour of every
    // tile is unseen, so only the friendly screen can waive the charge.
    let blind = crate::world::TileBits::default();

    assert_eq!(
        ai.conquest_blind_tile_penalty(&game, 0, force[0], tile, &seen),
        0.0,
        "a wholly seen ring costs nothing"
    );
    // With only the tile itself visible, every neighbour is unseen — but the
    // rest of the force stands beside it, which is the screen.
    assert_eq!(
        ai.conquest_blind_tile_penalty(&game, 0, force[0], tile, &blind),
        0.0,
        "a friendly body beside it waives the charge"
    );

    // Alone on a tile whose ring is unseen.
    let lone = game
        .wring(rally, 6)
        .into_iter()
        .find(|pos| game.unit_ids_at(*pos).is_empty() && game.city_at(*pos).is_none())
        .expect("an empty tile far from the force");
    assert_eq!(
        ai.conquest_blind_tile_penalty(&game, 0, force[0], lone, &blind),
        CONQUEST_BLIND_TILE_PENALTY,
        "unseen ground and nobody beside it is charged"
    );

    // Nothing of this reaches a unit outside the force, or the gene off.
    let outsider = bodies(&mut game, 0, "warrior", rally, 5, 1)[0];
    assert_eq!(
        ai.conquest_blind_tile_penalty(&game, 0, outsider, lone, &blind),
        0.0,
        "a body outside the force keeps the shipped score"
    );
    let off = AdvancedAi::new();
    assert_eq!(
        off.conquest_blind_tile_penalty(&game, 0, force[0], lone, &blind),
        0.0,
        "off, the term is zero"
    );
}

// ---------------------------------------------------------- the continuation

#[test]
fn a_war_that_pays_continues_and_one_that_does_not_asks_for_terms() {
    let mut game = board(&[at(6, 12), at(14, 12)]);
    let second = game.found_city_for(1, at(16, 14), None);
    let mut ai = opened(&mut game);
    let city = game.player_city_ids(1)[0];
    let rally = ai.conquest_opening.as_ref().unwrap().rally;
    bodies(
        &mut game,
        0,
        "warrior",
        rally,
        1,
        CONQUEST_RANGED + CONQUEST_MELEE,
    );
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.conquest_declaration(&mut game, 0));

    // The objective changes hands to us, and the war has cost nothing.
    game.cities.get_mut(&city).unwrap().owner = 0;
    ai.maintain_conquest_opening(&mut game, 0);
    let opening = ai
        .conquest_opening
        .as_ref()
        .expect("a paying campaign continues");
    assert_eq!(opening.taken, 1);
    assert_eq!(
        opening.city, second,
        "it moves to the rival's next known city"
    );
    assert_eq!(ai.campaign_objective_city(&game, 0, Some(1)), Some(second));

    // Now the same campaign, trading badly: three bodies lost, no kills.
    if let Some(opening) = ai.conquest_opening.as_mut() {
        opening.losses = 3;
    }
    game.cities.get_mut(&second).unwrap().owner = 0;
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(
        ai.conquest_opening.is_none(),
        "a campaign under the kills-per-loss floor closes"
    );
    assert!(
        ai.peace_offers.contains(&1),
        "and it asks the shipped peace desk for terms"
    );
}

#[test]
fn the_war_rate_is_counted_from_the_engines_own_kill_counter() {
    let mut game = board(&[at(6, 12), at(14, 12)]);
    let opening = ConquestOpening {
        target: 1,
        city: 0,
        opened: 10,
        rally: at(10, 12),
        force: BTreeSet::new(),
        assembled: Some(10),
        declared: Some(12),
        kills_at_war: 4,
        losses: 0,
        taken: 0,
    };
    assert_eq!(
        opening.kills_per_loss(&game, 0),
        CONQUEST_KILLS_PER_LOSS_FLOOR,
        "a war that has cost nothing is not evidence against itself"
    );

    game.players[0].counters.insert("kills".to_string(), 7);
    let two_lost = ConquestOpening {
        losses: 2,
        ..opening.clone()
    };
    assert_eq!(
        two_lost.kills_per_loss(&game, 0),
        1.5,
        "three kills since the declaration over two losses"
    );
    let four_lost = ConquestOpening {
        losses: 4,
        ..opening
    };
    assert!(four_lost.kills_per_loss(&game, 0) < CONQUEST_KILLS_PER_LOSS_FLOOR);
}

#[test]
fn the_opening_is_dropped_the_moment_the_gene_is_switched_off() {
    let mut game = board(&[at(6, 12), at(14, 12)]);
    let mut ai = opened(&mut game);
    assert!(ai.conquest_opening.is_some());
    ai.disable_early_conquest_opening();
    ai.maintain_conquest_opening(&mut game, 0);
    assert_eq!(ai.conquest_opening, None);
    assert!(!ai.conquest_owns_the_campaign());
}
