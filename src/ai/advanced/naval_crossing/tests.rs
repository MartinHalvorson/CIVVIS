use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

/// The `naval-escort-patience` board: a one-seat map of coast with our city
/// on a plains tile and a plains site more than eight tiles across the water.
fn island_colony_game() -> (Game, Pos, Pos) {
    let mut g = Game::new_full(1, 18, 10, 92, 120, 0, false);
    let founding_settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    let source = g.units[&founding_settler].pos;
    let target = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| g.wdisk(*pos, 2).len() == 19)
        .max_by_key(|pos| (g.wdist(source, *pos), *pos))
        .unwrap();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("coast");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.owner_city = None;
    }
    g.map.tiles.get_mut(&source).unwrap().terrain = crate::name!("plains");
    g.map.tiles.get_mut(&target).unwrap().terrain = crate::name!("plains");
    g.apply(
        0,
        &Action::FoundCity {
            unit: founding_settler,
        },
    )
    .unwrap();
    (g, source, target)
}

/// An embarked Settler with its land guard on the first water tile, and an
/// agent on the live capture lessons with `naval-escort-patience`.
fn embarked() -> (Game, AdvancedAi, u32, u32, Pos, Pos) {
    let (mut game, source, target) = island_colony_game();
    game.players[0]
        .techs
        .extend([crate::name!("sailing"), crate::name!("shipbuilding")]);
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let probe = game.spawn_test_unit("settler", 0, source);
    let first_water = game.route_step(probe, target, 0).unwrap();
    game.remove_unit(probe);
    let settler = game.spawn_test_unit("settler", 0, first_water);
    let guard = game.spawn_test_unit("warrior", 0, first_water);
    let mut ai = AdvancedAi::new();
    ai.enable_live_formationless_settler_shadow();
    ai.enable_live_settler_capture_lessons();
    ai.enable_naval_escort_patience();
    ai.settler_targets.insert(settler, target);
    ai.settler_escort_journeys.insert(
        settler,
        SettlerEscortJourney {
            home: source,
            target,
        },
    );
    ai.settler_guards.insert(settler, guard);
    (game, ai, settler, guard, first_water, target)
}

/// A new turn: the Settler and its guard get their movement back.
fn next_turn(game: &mut Game, units: [u32; 2]) {
    game.turn += 1;
    for uid in units {
        let moves = game.rules.units[game.units[&uid].kind].moves;
        let unit = game.units.get_mut(&uid).unwrap();
        unit.moves_left = moves;
        unit.acted = false;
    }
}

#[test]
fn naval_crossing_keeps_going_is_opt_in() {
    opt_in_off_in_both_controllers("naval-crossing-keeps-going", |ai| {
        ai.naval_crossing_keeps_going
    });
}

/// Stock: after its first crossing alone the Settler holds again at the next
/// water tile. Under the gene the voyage is under way and steps on.
#[test]
fn a_voyage_that_crossed_alone_steps_on() {
    for gene in [false, true] {
        let (mut game, mut ai, settler, guard, first_water, target) = embarked();
        if gene {
            ai.enable_naval_crossing_keeps_going();
        }
        for _ in 0..2 {
            assert!(!ai.settler_step_out_of_reach(&mut game, 0, settler, target));
            next_turn(&mut game, [settler, guard]);
        }
        assert!(
            ai.settler_step_out_of_reach(&mut game, 0, settler, target),
            "the third turn crosses alone (patience)"
        );
        let after_first = game.units[&settler].pos;
        assert_ne!(after_first, first_water);
        assert!(game.rules.is_water(&game.map.tiles[&after_first]));
        next_turn(&mut game, [settler, guard]);
        let moved = ai.settler_step_out_of_reach(&mut game, 0, settler, target);
        if gene {
            assert!(moved, "under way: no second wait on a quiet sea");
            assert_ne!(game.units[&settler].pos, after_first);
        } else {
            assert!(!moved, "stock: the next water tile starts the wait again");
            assert_eq!(game.units[&settler].pos, after_first);
        }
    }
}

/// A hostile ship near the voyage stops it, under way or not.
#[test]
fn a_hostile_ship_holds_a_voyage_under_way() {
    let (mut game, mut ai, settler, guard, _first_water, target) = embarked();
    ai.enable_naval_crossing_keeps_going();
    for _ in 0..2 {
        assert!(!ai.settler_step_out_of_reach(&mut game, 0, settler, target));
        next_turn(&mut game, [settler, guard]);
    }
    assert!(ai.settler_step_out_of_reach(&mut game, 0, settler, target));
    let here = game.units[&settler].pos;
    assert!(ai.naval_crossing_underway(&game, settler));
    let barbarian = game.players.iter().position(|p| p.is_barbarian).unwrap();
    let ship_tile = game
        .wdisk(here, 3)
        .into_iter()
        .find(|pos| {
            *pos != here
                && game
                    .map
                    .get(*pos)
                    .is_some_and(|tile| game.rules.is_water(tile))
                && game.unit_ids_at(*pos).is_empty()
        })
        .unwrap();
    game.spawn_test_unit("galley", barbarian, ship_tile);
    next_turn(&mut game, [settler, guard]);
    assert!(!ai.settler_step_out_of_reach(&mut game, 0, settler, target));
    assert_eq!(game.units[&settler].pos, here, "the ship keeps the hold");
}

/// Live 080511Z: a Settler afloat is retargeted between turns. Its voyage is
/// keyed by the unit, so it still steps on toward the new site.
#[test]
fn a_retargeted_voyage_still_steps_on() {
    let (mut game, mut ai, settler, guard, _first_water, target) = embarked();
    ai.enable_naval_crossing_keeps_going();
    let other = game
        .nbrs(target)
        .into_iter()
        .find(|pos| game.map.get(*pos).is_some())
        .expect("a tile beside the first site");
    game.map.tiles.get_mut(&other).unwrap().terrain = crate::name!("plains");
    for _ in 0..2 {
        assert!(!ai.settler_step_out_of_reach(&mut game, 0, settler, target));
        next_turn(&mut game, [settler, guard]);
    }
    assert!(ai.settler_step_out_of_reach(&mut game, 0, settler, target));
    let here = game.units[&settler].pos;
    assert!(game.rules.is_water(&game.map.tiles[&here]));
    next_turn(&mut game, [settler, guard]);
    ai.settler_targets.insert(settler, other);
    if let Some(journey) = ai.settler_escort_journeys.get_mut(&settler) {
        journey.target = other;
    }
    assert!(
        ai.settler_step_out_of_reach(&mut game, 0, settler, other),
        "the voyage under way survives the new target"
    );
    assert_ne!(game.units[&settler].pos, here);
}

/// The voyage mark follows the Settler through a unit-id remap.
#[test]
fn the_voyage_mark_follows_a_remapped_settler() {
    let mut ai = AdvancedAi::new();
    ai.enable_naval_crossing_keeps_going();
    ai.naval_crossing_units.insert(7, 40);
    ai.remap_unit_memory(&BTreeMap::from([(7, 70)]));
    assert_eq!(ai.naval_crossing_units.get(&70), Some(&40));
    assert!(!ai.naval_crossing_units.contains_key(&7));
}
