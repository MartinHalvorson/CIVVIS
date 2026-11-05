use super::*;
use crate::game::Action;

/// Our capital and player 1's founded capital, every unit removed. Returns
/// the game, our capital's position and the rival capital's position.
fn two_capitals(seed: u64) -> (Game, Pos, Pos) {
    let mut game = Game::new_full(2, 74, 46, seed, 200, 0, false);
    for pid in 0..2 {
        game.current = pid;
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.current = 0;
    let ours = game.cities[&game.player_city_ids(0)[0]].pos;
    let theirs = game.cities[&game.player_city_ids(1)[0]].pos;
    (game, ours, theirs)
}

/// A land tile exactly `distance` from `from`, if the map has one.
fn land_at(game: &Game, from: Pos, distance: i32) -> Option<Pos> {
    game.map
        .tiles
        .values()
        .filter(|tile| {
            !game.rules.is_water(tile)
                && game.rules.is_passable(tile)
                && game.wdist(from, tile.pos) == distance
        })
        .map(|tile| tile.pos)
        .min()
}

fn gene_on() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_early_cities_keep_their_distance();
    ai
}

#[test]
fn off_or_uncharted_the_term_is_zero() {
    let (mut game, _, theirs) = two_capitals(41_105);
    let site = land_at(&game, theirs, 5).expect("a site five tiles out");
    game.players[0].explored.insert(theirs);
    assert_eq!(
        AdvancedAi::new().early_city_rival_penalty(&game, 0, site),
        0.0
    );

    game.players[0].explored.remove(&theirs);
    assert_eq!(
        gene_on().early_city_rival_penalty(&game, 0, site),
        0.0,
        "a rival city the seat has not charted is fog"
    );
}

#[test]
fn a_charted_rival_city_prices_the_site_by_distance() {
    let (mut game, _, theirs) = two_capitals(41_105);
    game.players[0].explored.insert(theirs);
    let ai = gene_on();
    let near = land_at(&game, theirs, 5).expect("a site five tiles out");
    let mid = land_at(&game, theirs, 9).expect("a site nine tiles out");
    let far = land_at(&game, theirs, EARLY_CITY_RIVAL_BUFFER).expect("a site at the buffer");
    let near_pen = ai.early_city_rival_penalty(&game, 0, near);
    let mid_pen = ai.early_city_rival_penalty(&game, 0, mid);
    assert_eq!(near_pen, 5.0 * EARLY_CITY_TILE_PENALTY);
    assert_eq!(mid_pen, EARLY_CITY_TILE_PENALTY);
    assert_eq!(ai.early_city_rival_penalty(&game, 0, far), 0.0);
    assert!(
        ai.settle_value(&game, 0, near)
            <= AdvancedAi::new().settle_value(&game, 0, near) - near_pen + 1e-9,
        "the penalty reaches the settle value"
    );
}

#[test]
fn the_third_city_and_later_are_unpriced() {
    let (mut game, ours, theirs) = two_capitals(41_105);
    game.players[0].explored.insert(theirs);
    let ai = gene_on();
    let site = land_at(&game, theirs, 5).expect("a site five tiles out");
    assert!(ai.early_city_rival_penalty(&game, 0, site) > 0.0);
    for distance in [4, 8] {
        let Some(pos) = game
            .map
            .tiles
            .values()
            .filter(|tile| {
                !game.rules.is_water(tile)
                    && game.rules.is_passable(tile)
                    && game.city_at(tile.pos).is_none()
                    && tile.owner_city.is_none()
                    && game.wdist(ours, tile.pos) == distance
                    && game.wdist(theirs, tile.pos) > 3
            })
            .map(|tile| tile.pos)
            .min()
        else {
            continue;
        };
        let walker = game.spawn_test_unit("settler", 0, pos);
        game.apply(0, &Action::FoundCity { unit: walker }).unwrap();
    }
    assert!(game.player_city_ids(0).len() >= EARLY_CITY_COUNT);
    assert_eq!(
        ai.early_city_rival_penalty(&game, 0, site),
        0.0,
        "the restart rule protects only the first three cities"
    );
}
