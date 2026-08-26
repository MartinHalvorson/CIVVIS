//! A Tactics arena is decided by the fighting and by nothing else.
//!
//! Before 2026-08-26 a person seated on an arena was asked, every turn, to
//! choose research and a civic they could never study, to choose production
//! for a city granted nothing to build with, to form a government and seat
//! governors, to vote in a World Congress that convened every thirty turns
//! from the Medieval era, and to dedicate an age — the whole empire game,
//! standing between them and End Turn on a battlefield where the only thing
//! to do is fight. These tests pin the rule that replaced it: an arena
//! enumerates orders and nothing else, refuses the empire game from every
//! seat, climbs the technology tree on its own and alike for both sides,
//! seats no Congress, and keeps a captured city without asking.

use super::*;
use crate::setup::{MapScript, TacticsRules};

/// Medieval, in `ERA_NAMES`' numbering: the first era whose arena would
/// convene a Congress (`world_era >= 2`).
const MEDIEVAL: usize = 2;

fn arena(start_era: usize, rules: TacticsRules) -> Game {
    let game = Game::new_with(GameOptions {
        map_script: MapScript::Battlefield,
        start_era,
        tactics: rules,
        ..GameOptions::new(2, 12, 12, 260_826, 250, 0)
    });
    assert!(game.is_arena());
    game
}

fn cheapest_open_technology(game: &Game, pid: usize) -> Name {
    game.available_techs(pid)
        .into_iter()
        .min_by(|a, b| {
            game.tech_cost(a.as_str())
                .total_cmp(&game.tech_cost(b.as_str()))
                .then_with(|| a.cmp(b))
        })
        .expect("an opening arena has a tree left to climb")
}

fn city_of(game: &Game, pid: usize) -> u32 {
    game.cities
        .values()
        .find(|city| city.owner == pid)
        .map(|city| city.id)
        .expect("the stock arena seats one city a side")
}

/// The stock arena, played from the Medieval era for sixty-five turns by
/// two seats that only ever end their turn: every action either is offered
/// is an order, the Congress never sits, and the empire's own decisions —
/// research, civics, production for a city with nothing to build with —
/// are never put to anybody.
#[test]
fn a_medieval_arena_puts_nothing_but_orders_to_its_seats() {
    let mut game = arena(MEDIEVAL, TacticsRules::default());
    // The start era deals its civics along with its techs; what an arena
    // never does is add to them.
    let opening_civics: Vec<usize> = (0..2).map(|seat| game.players[seat].civics.len()).collect();
    let mut offered_types = BTreeSet::new();
    let mut turns_seen = 0;
    while game.turn <= 65 && !game.is_finished() {
        let seat = game.current;
        let offered = game.legal_actions(seat);
        for action in &offered {
            let label = format!("{action:?}");
            let kind = label.split([' ', '{']).next().unwrap_or("").to_string();
            offered_types.insert(kind);
            assert!(
                !action.off_the_battlefield(),
                "turn {} seat {seat} was offered the empire game: {label}",
                game.turn
            );
            assert!(
                !matches!(action, Action::Produce { .. }),
                "turn {} seat {seat} was asked to choose production with no Production to build with",
                game.turn
            );
        }
        assert!(
            game.congress.is_none(),
            "a Congress convened on a battlefield at turn {}",
            game.turn
        );
        assert!(
            game.pending_emergencies.is_empty(),
            "an Emergency was queued on a battlefield"
        );
        turns_seen += 1;
        game.apply(seat, &Action::EndTurn)
            .expect("ending a turn is always allowed");
    }
    assert!(
        turns_seen >= 120,
        "both seats took their sixty-five turns: {turns_seen}"
    );
    // The tree still moves — a technology every five turns, the stock pace —
    // and it moves the same way for both sides, with nobody having chosen.
    for seat in 0..2 {
        assert!(
            game.players[seat].techs.len() >= 8,
            "seat {seat} climbed the tree on its own: {} technologies",
            game.players[seat].techs.len()
        );
        assert!(
            game.players[seat].civic.is_none()
                && game.players[seat].civics.len() == opening_civics[seat],
            "seat {seat} has no civics tree to climb"
        );
    }
    assert_eq!(
        game.players[0].techs, game.players[1].techs,
        "both sides are armed alike: the arena picked the same technologies for each"
    );
    assert!(
        offered_types.contains("EndTurn") && offered_types.iter().any(|kind| kind != "EndTurn"),
        "orders were still offered: {offered_types:?}"
    );
}

/// The refusal is the engine's, for every seat: an AI that picks its own
/// research or sues for its own peace gets the same answer a player would.
#[test]
fn an_arena_refuses_the_empire_game_from_every_seat() {
    let mut game = arena(MEDIEVAL, TacticsRules::default());
    let tech = cheapest_open_technology(&game, 0);
    // The arena has already put a technology under study for each side;
    // clear it so the refusal is about the rule, not about "already
    // researching".
    game.players[0].research = None;
    let refused = [
        Action::Research { tech },
        Action::Civic {
            civic: crate::name!("code_of_laws"),
        },
        Action::Government {
            government: crate::name!("chiefdom"),
        },
        Action::MakePeace { player: 1 },
        Action::ProposeDeal {
            player: 1,
            give_gold: 0.0,
            request_gold: 0.0,
            open_borders: false,
            friendship: false,
            peace: true,
            alliance: None,
        },
        Action::ChooseDedication {
            dedication: crate::name!("monumentality"),
        },
    ];
    for action in refused {
        assert!(
            action.off_the_battlefield(),
            "{action:?} belongs to the empire game"
        );
        let err = game
            .apply(0, &action)
            .expect_err(&format!("{action:?} must be refused on a battlefield"));
        assert!(
            err.contains("battlefield"),
            "{action:?} refused for the wrong reason: {err}"
        );
    }
    assert!(
        game.players[0].research.is_none(),
        "the refused pick did not land"
    );
    // Orders are not the empire game.
    for action in [
        Action::EndTurn,
        Action::Fortify { unit: 0 },
        Action::Move {
            unit: 0,
            to: (0, 0),
        },
        Action::Attack {
            unit: 0,
            target: (0, 0),
        },
        Action::Promote {
            unit: 0,
            promotion: crate::name!("battlecry"),
        },
        Action::CityStrike {
            city: 0,
            target: (0, 0),
        },
    ] {
        assert!(!action.off_the_battlefield(), "{action:?} is an order");
    }
}

/// Research on an arena is a pace, not a decision: both sides are studying
/// something from their first turn, the cheapest technology open to them,
/// and a pace of zero freezes the tree instead.
#[test]
fn an_arena_climbs_the_tree_on_its_own_and_alike() {
    let mut game = arena(
        0,
        TacticsRules {
            turns_per_tech: 5,
            ..TacticsRules::default()
        },
    );
    for seat in 0..2 {
        assert_eq!(
            game.players[seat].research.as_deref(),
            Some(cheapest_open_technology(&game, seat).as_str()),
            "seat {seat} is studying the cheapest open technology from turn one"
        );
    }
    let opening: Vec<usize> = (0..2).map(|seat| game.players[seat].techs.len()).collect();
    while game.turn <= 12 {
        let seat = game.current;
        game.apply(seat, &Action::EndTurn).unwrap();
    }
    for (seat, opening) in opening.iter().enumerate() {
        assert!(
            game.players[seat].techs.len() >= opening + 2,
            "a five-turn pace lands two technologies inside twelve turns for seat {seat}"
        );
        assert!(
            game.players[seat].research.is_some(),
            "seat {seat} picked its next study itself"
        );
    }
    assert_eq!(game.players[0].techs, game.players[1].techs);

    let mut frozen = arena(
        0,
        TacticsRules {
            turns_per_tech: 0,
            ..TacticsRules::default()
        },
    );
    let opening: Vec<usize> = (0..2)
        .map(|seat| frozen.players[seat].techs.len())
        .collect();
    while frozen.turn <= 12 {
        let seat = frozen.current;
        frozen.apply(seat, &Action::EndTurn).unwrap();
    }
    for (seat, opening) in opening.iter().enumerate() {
        assert!(
            frozen.players[seat].research.is_none(),
            "a frozen tree studies nothing"
        );
        assert_eq!(
            frozen.players[seat].techs.len(),
            *opening,
            "and gains nothing"
        );
    }
}

/// A build menu needs Production to build with. The stock arena grants
/// none, so its city offers nothing and asks for nothing; raise the grant
/// and the menu is back, fighting units only.
#[test]
fn a_build_menu_needs_production_to_build_with() {
    let stock = arena(0, TacticsRules::default());
    let city = city_of(&stock, 0);
    assert!(
        stock.producible_items(0, city).is_empty(),
        "nothing to build with, nothing to build"
    );
    assert!(
        !stock
            .legal_actions(0)
            .iter()
            .any(|action| matches!(action, Action::Produce { .. })),
        "the stock arena never asks its player to choose production"
    );

    let reinforced = arena(
        0,
        TacticsRules {
            production: 30,
            ..TacticsRules::default()
        },
    );
    let city = city_of(&reinforced, 0);
    let menu = reinforced.producible_items(0, city);
    assert!(!menu.is_empty(), "a granted city builds again");
    assert!(
        menu.iter()
            .all(|item| matches!(item, Item::Unit { .. } | Item::Formation { .. })),
        "and builds only what fights: {menu:?}"
    );
}

/// A captured arena city is kept the moment it falls; no verdict is put to
/// the captor, and the seat is never told to resolve the city's fate first.
#[test]
fn a_captured_arena_city_is_kept_without_asking() {
    let mut game = arena(0, TacticsRules::default());
    let city = city_of(&game, 1);
    game.capture_city(city, 0);
    assert_eq!(game.cities[&city].owner, 0, "the city changed hands");
    assert!(
        game.cities[&city].captured_from.is_none(),
        "and was kept on the spot"
    );
    assert!(
        game.pending_city_capture_actions(0).is_empty(),
        "nothing about its fate is put to the captor"
    );
    assert!(
        !game.legal_actions(0).iter().any(|action| matches!(
            action,
            Action::KeepCity { .. } | Action::RazeCity { .. } | Action::LiberateCity { .. }
        )),
        "and no verdict is enumerated"
    );
    // Taking the other side's only city is the arena's objective, so the
    // keep verdict the engine applied for the captor runs the Domination
    // check at once: the battle is decided on the spot, with no "Keep
    // city?" in between — or, on an arena that seats no cities to decide
    // it, the turn simply goes on.
    assert!(
        (game.is_finished() && game.winner == Some(0)) || game.apply(0, &Action::EndTurn).is_ok(),
        "the turn is not held on the verdict"
    );
}
