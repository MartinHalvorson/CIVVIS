//! `exploration-replans-need-a-sighting`: ground an order explored re-opens
//! the turn only when the foreign units and cities in the seat's sight
//! changed with it.
use super::*;

/// Player 0's land unit, one legal step it can take, and a board on which
/// that step explores ground: the seat's explored set is cut back to the
/// unit's own tile, so whatever the step reveals is new.
fn exploring_step(seed: u64) -> (Game, u32, Pos) {
    let mut game = Game::new_full(
        2,
        24,
        16,
        crate::rng::fixture_seed("exploration-replans-need-a-sighting", seed),
        250,
        0,
        false,
    );
    let (unit, to) = game
        .player_unit_ids(0)
        .into_iter()
        .find_map(|unit| {
            let from = game.units[&unit].pos;
            game.nbrs(from)
                .into_iter()
                .find(|to| game.can_move(unit, *to) && game.unit_ids_at(*to).is_empty())
                .map(|to| (unit, to))
        })
        .expect("fixture: a unit of player 0 can step");
    let home = game.units[&unit].pos;
    game.players[0].explored = std::iter::once(home).collect();
    (game, unit, to)
}

/// Whether `move unit to` re-opens the turn on `game`, with the gene as `on`.
fn reopens(mut game: Game, unit: u32, to: Pos, on: bool) -> Option<bool> {
    game.exploration_replans_need_a_sighting = on;
    let explored = game.players[0].explored.len();
    let reopened = execute_observed_action_recorded(
        &mut game,
        0,
        &Action::Move { unit, to },
        &mut Vec::new(),
        &mut Vec::new(),
        &mut BTreeSet::new(),
    );
    assert!(
        game.players[0].explored.len() > explored,
        "fixture: the step must explore ground"
    );
    reopened
}

#[test]
fn explored_empty_ground_re_opens_the_turn_only_without_the_gene() {
    let (game, unit, to) = exploring_step(1);
    assert_eq!(
        foreign_in_sight(&game, 0),
        0,
        "fixture: nobody foreign in sight"
    );
    assert_eq!(reopens(game.clone(), unit, to, false), Some(true));
    assert_eq!(reopens(game, unit, to, true), Some(false));
}

/// A board, the step, and a land tile the step brings into sight that was
/// out of it before, from the first fixture seed whose start has one.
fn sighting_step() -> (Game, u32, Pos, Pos) {
    (1..=16)
        .find_map(|seed| {
            let (game, unit, to) = exploring_step(seed);
            let before = game.player_vision_frame(0);
            let mut stepped = game.clone();
            stepped.apply(0, &Action::Move { unit, to }).ok()?;
            let after = stepped.player_vision_frame(0);
            let newly = game
                .map
                .tiles
                .iter()
                .find(|(pos, tile)| {
                    let index = game.map.tiles.index_of(**pos).unwrap();
                    after.contains(index)
                        && !before.contains(index)
                        && game.rules.is_passable(tile)
                        && !game.rules.is_water(tile)
                        && game.unit_ids_at(**pos).is_empty()
                })
                .map(|(pos, _)| *pos)?;
            Some((game, unit, to, newly))
        })
        .expect("fixture: some start sights a new land tile in one step")
}

#[test]
fn explored_ground_showing_a_foreign_unit_re_opens_the_turn() {
    let (game, unit, to, newly) = sighting_step();
    let mut sighted = game;
    let foreign = sighted.player_unit_ids(1)[0];
    sighted.units.get_mut(&foreign).unwrap().pos = newly;
    assert_eq!(reopens(sighted, unit, to, true), Some(true));
}
