//! Focused tests for `first-capital-walks-the-one-landmass`.
//!
//! The seat is the live Domination seat's opening (the parent gene's tests
//! build it the same way), and the board is the live one after the
//! first-meeting capital exchange: the rival capital charted, the ground
//! between not.

use super::super::first_capital_before_the_walls::{
    FIRST_CAPITAL_REACH_TILES, FIRST_CAPITAL_WALK_SLACK,
};
use super::super::test_support::opt_in_off_in_both_controllers;
use super::super::VictoryTarget;
use super::*;
use crate::name;

fn at(col: i32, row: i32) -> Pos {
    (col, row)
}

/// A flat 60x30 grassland board of `capitals.len()` empires on `script`,
/// every starting unit cleared, nobody at war, Online speed, turn 45.
fn board(capitals: &[Pos], script: MapScript) -> Game {
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
    game.map_script = script;
    game.turn = 45;
    game.current = 0;
    game
}

fn capital_of(game: &Game, pid: usize) -> u32 {
    game.player_city_ids(pid)
        .into_iter()
        .find(|cid| game.cities[cid].is_capital)
        .unwrap()
}

/// Our capital at (6, 12), theirs 18 tiles east, met; their capital and its
/// ring charted, the ground between not.
fn fogged(script: MapScript) -> (Game, u32) {
    let mut game = board(&[at(6, 12), at(24, 12)], script);
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    let ring = game.wdisk(at(24, 12), 1);
    game.players[0].explored.extend(ring);
    let capital = capital_of(&game, 1);
    (game, capital)
}

/// The live Domination seat's opening genes with
/// `first-capital-before-the-walls`, with or without this one.
fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_early_conquest_opening();
    ai.enable_conquest_opening_stays_near();
    ai.enable_early_conquest_stands_down();
    ai.enable_declaration_needs_production_parity();
    ai.conquest_opening_needs_the_production = false;
    ai.enable_first_capital_before_the_walls();
    ai.base.explore_commit = true;
    if gene {
        ai.enable_first_capital_walks_the_one_landmass();
    }
    ai
}

#[test]
fn first_capital_walks_the_one_landmass_is_a_native_opt_in_off_in_both_controllers() {
    opt_in_off_in_both_controllers("first-capital-walks-the-one-landmass", |ai| {
        ai.first_capital_walks_the_one_landmass
    });
    let game = board(&[at(6, 12), at(24, 12)], MapScript::Pangaea);
    assert!(seat(true).first_capital_walks_the_one_landmass_on(&game));
    assert!(!seat(false).first_capital_walks_the_one_landmass_on(&game));
    let mut bare = AdvancedAi::targeting(VictoryTarget::Domination);
    bare.enable_first_capital_walks_the_one_landmass();
    assert!(
        !bare.first_capital_walks_the_one_landmass_on(&game),
        "rides on first-capital-before-the-walls"
    );
}

#[test]
fn pangaea_land_only_and_lakes_are_the_one_landmass_maps() {
    let one: Vec<MapScript> = crate::setup::CIV6_MAP_SCRIPTS
        .iter()
        .map(|spec| spec.script)
        .filter(|script| map_script_is_one_landmass(*script))
        .collect();
    assert_eq!(
        one,
        vec![MapScript::LandOnly, MapScript::Lakes, MapScript::Pangaea]
    );
    for script in [
        MapScript::InlandSea,
        MapScript::TeninsBall,
        MapScript::GrandCanals,
        MapScript::Continents,
        MapScript::SmallContinents,
        MapScript::Islands,
        MapScript::TacticsPlanet,
    ] {
        assert!(!map_script_is_one_landmass(script), "{script:?}");
    }
}

/// The live refusal (Washington, 12 tiles, walls 0, t12-t62): off, no
/// charted walk reaches the capital; on a Pangaea the gene names it and says
/// so once at naming.
#[test]
fn on_a_pangaea_the_capital_across_uncharted_ground_is_named() {
    let (mut game, capital) = fogged(MapScript::Pangaea);
    assert_eq!(
        seat(false).conquest_target(&game, 0),
        None,
        "off: no charted walk"
    );

    let mut on = seat(true);
    assert_eq!(on.conquest_target(&game, 0), Some((1, capital)));
    on.base.journal = crate::reasoning::Journal::recording();
    on.maintain_conquest_opening(&mut game, 0);
    let opening = on
        .conquest_opening
        .as_ref()
        .expect("on: the opening stands");
    assert_eq!((opening.target, opening.city), (1, capital));
    game.turn += 1;
    on.maintain_conquest_opening(&mut game, 0);
    let notes: Vec<_> = on
        .journal()
        .since(0)
        .thoughts
        .into_iter()
        .filter(|thought| thought.headline.contains("over uncharted ground"))
        .collect();
    assert_eq!(notes.len(), 1, "journalled once, at naming");
    assert!(
        notes[0]
            .detail
            .contains("uncharted tiles; a Pangaea map has one landmass"),
        "{}",
        notes[0].detail
    );

    // A charted walk needs no note.
    let (mut game, _) = fogged(MapScript::Pangaea);
    let everything: Vec<Pos> = game.map.tiles.keys().copied().collect();
    game.players[0].explored.extend(everything);
    let mut on = seat(true);
    on.base.journal = crate::reasoning::Journal::recording();
    on.maintain_conquest_opening(&mut game, 0);
    assert!(on.conquest_opening.is_some());
    assert!(on
        .journal()
        .since(0)
        .thoughts
        .iter()
        .all(|thought| !thought.headline.contains("over uncharted ground")));
}

#[test]
fn a_charted_sea_or_mountain_range_still_blocks_the_walk() {
    for mountains in [false, true] {
        let (mut game, _) = fogged(MapScript::Pangaea);
        for row in 0..30 {
            if let Some(tile) = game.map.tiles.get_mut(&at(15, row)) {
                tile.terrain = if mountains {
                    name!("mountain")
                } else {
                    name!("coast")
                };
                game.players[0].explored.insert(at(15, row));
            }
        }
        assert_eq!(
            seat(true).conquest_target(&game, 0),
            None,
            "a charted barrier, mountains {mountains}"
        );
    }
}

#[test]
fn the_uncharted_walk_starts_home_stays_in_its_bound_and_ends_beside_the_capital() {
    let game = board(&[at(6, 12), at(24, 12)], MapScript::Pangaea);
    let home = at(6, 12);
    let walk = AdvancedAi::first_capital_walk(&game, 0, 1, home, at(24, 12), true)
        .expect("uncharted ground is land");
    assert_eq!(walk[0], home);
    assert_eq!(game.wdist(walk[walk.len() - 1], at(24, 12)), 1);
    assert_eq!(walk.len(), 18, "the shortest walk: 17 steps");
    assert!(
        AdvancedAi::first_capital_walk(&game, 0, 1, home, at(24, 12), false).is_none(),
        "strict: nothing between is charted"
    );
    let far = at(
        6 + FIRST_CAPITAL_REACH_TILES + FIRST_CAPITAL_WALK_SLACK + 2,
        12,
    );
    assert!(
        AdvancedAi::first_capital_walk(&game, 0, 1, home, far, true).is_none(),
        "past the bound"
    );
}

#[test]
fn on_any_other_map_the_walk_stays_charted() {
    for script in [
        MapScript::Continents,
        MapScript::SmallContinents,
        MapScript::InlandSea,
        MapScript::TeninsBall,
    ] {
        let (game, capital) = fogged(script);
        let on = seat(true);
        assert_eq!(on.conquest_target(&game, 0), None, "{script:?}");
        assert!(on
            .first_capital_uncharted_corridor(&game, 0, 1, capital)
            .is_empty());
    }
}

#[test]
fn the_nearest_free_scout_charts_the_walk() {
    let (mut game, capital) = fogged(MapScript::Pangaea);
    let near = game.spawn_test_unit("scout", 0, at(8, 12));
    let far = game.spawn_test_unit("scout", 0, at(6, 16));

    assert_eq!(
        seat(false).rival_lead_goal(&game, 0, near),
        None,
        "off: no corridor"
    );

    // Before naming, the capital the scan would name.
    let mut on = seat(true);
    let corridor = on.first_capital_uncharted_corridor(&game, 0, 1, capital);
    assert!(!corridor.is_empty());
    assert!(corridor
        .iter()
        .all(|pos| !game.players[0].explored.contains(pos)));
    assert_eq!(on.rival_lead_goal(&game, 0, near), Some(corridor[0]));
    assert_eq!(
        on.rival_lead_goal(&game, 0, far),
        None,
        "one scout charts it"
    );

    // Named and undeclared: the same corridor.
    on.maintain_conquest_opening(&mut game, 0);
    assert!(on.conquest_opening.is_some());
    assert_eq!(on.rival_lead_goal(&game, 0, near), Some(corridor[0]));

    // Charted: the walk is charted and the scout goes back to its sweep.
    let everything: Vec<Pos> = game.map.tiles.keys().copied().collect();
    game.players[0].explored.extend(everything);
    assert!(on
        .first_capital_uncharted_corridor(&game, 0, 1, capital)
        .is_empty());
    assert_eq!(on.rival_lead_goal(&game, 0, near), None);

    // On Continents the scouts keep their sweep.
    let (mut game, _) = fogged(MapScript::Continents);
    let scout = game.spawn_test_unit("scout", 0, at(8, 12));
    assert_eq!(seat(true).rival_lead_goal(&game, 0, scout), None);
}
