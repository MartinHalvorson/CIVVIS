use super::*;
use std::sync::Arc;

/// Two majors who have met: our founded capital, the rival's city 9-12 tiles
/// off, a rival Warrior seen eight tiles from our capital, everything within
/// nine tiles in sight, and the rival at 1.4 times our power (201608Z's
/// Egypt at t50). Returns the game.
fn neighbour(difficulty: &str) -> Game {
    let mut game = Game::new_full(2, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("fixture: a settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("fixture: the capital founds");
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
            (9..=12).contains(&game.wdist(**pos, home)) && !game.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    game.place_city(1, *sites.first().expect("fixture: a rival site"), None);
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    game.turn = 30;
    game.difficulty = difficulty.to_string();
    game.host_observed = Arc::new(game.wdisk(home, 9).into_iter().collect());
    Arc::make_mut(&mut game.observed_military_power).insert(0, 100.0);
    Arc::make_mut(&mut game.observed_military_power).insert(1, 140.0);
    let mut tiles: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| game.wdist(**pos, home) == 8 && !game.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .collect();
    tiles.sort();
    game.spawn_unit("warrior", 1, tiles[0]);
    game
}

fn watch(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_capital_walls_before_the_surprise();
    if gene {
        ai.enable_capital_walls_at_any_ratio();
    }
    ai
}

/// A 1.4x neighbour at our door arms the walls at Immortal and Deity with the
/// gene; off, or at King, the 2.0 bar leaves them unarmed.
#[test]
fn a_neighbour_stronger_than_us_arms_the_walls_at_high_tiers() {
    for difficulty in ["immortal", "deity"] {
        let game = neighbour(difficulty);
        let mut on = watch(true);
        on.observe_turn_start_hostiles(&game, 0);
        assert!(
            on.capital_watched_major_power(&game, 0).is_some(),
            "{difficulty}: a 1.4x neighbour arms the walls"
        );
        let mut off = watch(false);
        off.observe_turn_start_hostiles(&game, 0);
        assert!(
            off.capital_watched_major_power(&game, 0).is_none(),
            "{difficulty}: off, the 2.0 bar stands"
        );
    }
    let king = neighbour("king");
    let mut on = watch(true);
    on.observe_turn_start_hostiles(&king, 0);
    assert!(
        on.capital_watched_major_power(&king, 0).is_none(),
        "below Immortal the 2.0 bar stands"
    );
}

/// A neighbour weaker than us still does not arm them.
#[test]
fn a_weaker_neighbour_does_not() {
    let mut game = neighbour("immortal");
    Arc::make_mut(&mut game.observed_military_power).insert(1, 80.0);
    let mut on = watch(true);
    on.observe_turn_start_hostiles(&game, 0);
    assert!(on.capital_watched_major_power(&game, 0).is_none());
}
