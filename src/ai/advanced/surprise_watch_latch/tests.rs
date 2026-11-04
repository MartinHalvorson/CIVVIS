use super::*;
use crate::ai::advanced::capital_walls_before_the_surprise::{
    CAPITAL_WATCH_STANDARD, CAPITAL_WATCH_TILES,
};
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

/// Two majors who have met: our founded capital (population 4, idle), the
/// rival's city 11-13 tiles off, every unit removed, everything within
/// eleven tiles of the capital in sight, and the rival at 2.5 times our
/// power. Returns the game and our capital.
fn watched() -> Option<(Game, u32)> {
    let mut game = Game::new_full(2, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")?;
    game.apply(0, &Action::FoundCity { unit: settler }).ok()?;
    for pid in [0, 1] {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            (11..=13).contains(&game.wdist(**pos, home)) && !game.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    game.place_city(1, *sites.first()?, None);
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    let city = game.cities.get_mut(&capital).unwrap();
    city.pop = 4;
    city.queue.clear();
    game.turn = 20;
    game.host_observed = Arc::new(game.wdisk(home, 11).into_iter().collect());
    power(&mut game, 0, 100.0);
    power(&mut game, 1, 250.0);
    Some((game, capital))
}

fn power(game: &mut Game, pid: usize, value: f64) {
    Arc::make_mut(&mut game.observed_military_power).insert(pid, value);
}

/// A land tile exactly `gap` tiles from the capital not yet holding a unit.
fn ground(game: &Game, capital: u32, gap: i32) -> Pos {
    let home = game.cities[&capital].pos;
    let mut tiles: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            game.wdist(**pos, home) == gap
                && !game.rules.is_water(tile)
                && !game.units.values().any(|unit| unit.pos == **pos)
        })
        .map(|(pos, _)| *pos)
        .collect();
    tiles.sort();
    tiles[0]
}

fn armed(latch: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_capital_walls_before_the_surprise();
    if latch {
        ai.enable_surprise_watch_holds_its_ratio();
    }
    ai
}

#[test]
fn surprise_watch_holds_its_ratio_is_opt_in() {
    opt_in_off_in_both_controllers("surprise-watch-holds-its-ratio", |ai| {
        ai.surprise_watch_holds_its_ratio
    });
}

/// 001453Z: a 2.5x neighbour's soldier seen near the capital, then our
/// bought warriors lift us to 1.7x. The walls' live reading stands down;
/// the latch keeps them armed through the window, and no longer.
#[test]
fn the_capital_walls_stay_armed_when_our_army_closes_the_ratio() {
    let Some((mut game, capital)) = watched() else {
        panic!("fixture: two majors with a capital each");
    };
    let at = ground(&game, capital, CAPITAL_WATCH_TILES);
    let soldier = game.spawn_unit("warrior", 1, at);

    let mut stock = armed(false);
    stock.observe_turn_start_hostiles(&game, 0);
    let mut ai = armed(true);
    ai.observe_turn_start_hostiles(&game, 0);
    assert!(
        stock.capital_walls_threat(&game, 0).is_some(),
        "2.5x: both arm"
    );
    assert!(ai.capital_walls_threat(&game, 0).is_some());

    // The soldier walks off and our army reaches 1.7x their power's share.
    game.remove_unit(soldier);
    game.turn += 2;
    power(&mut game, 0, 150.0);
    stock.observe_turn_start_hostiles(&game, 0);
    ai.observe_turn_start_hostiles(&game, 0);
    assert!(
        stock.capital_walls_threat(&game, 0).is_none(),
        "off: the live ratio stands the walls down"
    );
    assert_eq!(
        ai.capital_walls_threat(&game, 0).map(|(rival, _)| rival),
        Some(1),
        "the latch holds"
    );

    // Past the window, or at war, the latch is gone.
    let mut later = game.clone();
    later.turn += later.standard_duration(CAPITAL_WATCH_STANDARD);
    assert!(
        ai.capital_walls_threat(&later, 0).is_none(),
        "past the window"
    );
    let mut war = game.clone();
    war.at_war.insert((0, 1));
    war.at_war.insert((1, 0));
    assert!(ai.capital_surprise_latched(&war, 0).is_none(), "at war");
}

/// 005343Z: a 2x neighbour's soldier at nine or ten tiles, outside the walls'
/// own eight-tile watch, latches; at eleven, or under 2x when seen, it does
/// not.
#[test]
fn the_latch_reads_ten_tiles_and_the_ratio_when_seen() {
    let Some((mut game, capital)) = watched() else {
        panic!("fixture: two majors with a capital each");
    };
    let at = ground(&game, capital, LATCH_TILES);
    let soldier = game.spawn_unit("warrior", 1, at);

    let mut stock = armed(false);
    stock.observe_turn_start_hostiles(&game, 0);
    assert!(
        stock.capital_walls_threat(&game, 0).is_none(),
        "off: ten tiles is outside the watch"
    );
    let mut ai = armed(true);
    ai.observe_turn_start_hostiles(&game, 0);
    assert!(
        ai.capital_walls_threat(&game, 0).is_some(),
        "ten tiles latches"
    );

    game.remove_unit(soldier);
    let far = ground(&game, capital, LATCH_TILES + 1);
    game.spawn_unit("warrior", 1, far);
    let mut beyond = armed(true);
    beyond.observe_turn_start_hostiles(&game, 0);
    assert!(beyond.surprise_watch_latches.is_empty(), "eleven tiles: no");

    let Some((mut level, capital)) = watched() else {
        panic!("fixture");
    };
    power(&mut level, 1, 150.0);
    let near = ground(&level, capital, LATCH_TILES);
    level.spawn_unit("warrior", 1, near);
    let mut even = armed(true);
    even.observe_turn_start_hostiles(&level, 0);
    assert!(even.surprise_watch_latches.is_empty(), "1.5x when seen: no");
}
