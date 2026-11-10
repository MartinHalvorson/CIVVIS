//! The final war's doctrine: no defensive plan, a larger army, every idle
//! city arming, and none of it outside a world played on to its last civ.

use super::*;
use crate::game::PlayOnMode;

/// Two founded capitals, every unit removed, seat 0 to move, at war.
fn two_capitals_at_war() -> Game {
    let mut game = Game::new_full(2, 30, 18, 7_218, 300, 0, false);
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
    game.turn = 200;
    game.at_war.insert((0, 1));
    game
}

/// "One more turn" on a decided world: only the survivor can win now.
fn play_to_the_last_civ(game: &mut Game) {
    game.winner = Some(1);
    game.victory_type = Some("score".to_string());
    assert!(game.play_on(PlayOnMode::LastCivStanding));
    assert!(game.plays_to_the_last_civ());
    game.current = 0;
}

/// Five hostile soldiers three tiles from seat 0's capital, in its sight,
/// with its own four tanks nine tiles away: the shipped Recovery fixture.
fn besiege_the_capital(game: &mut Game) -> u32 {
    let home = game.player_city_ids(0)[0];
    let home_pos = game.cities[&home].pos;
    let intruder_pos = game
        .wdisk(home_pos, 6)
        .into_iter()
        .find(|position| game.wdist(*position, home_pos) == 3 && game.city_at(*position).is_none())
        .unwrap();
    let observer_pos = game
        .nbrs(intruder_pos)
        .into_iter()
        .find(|position| {
            *position != home_pos
                && game.city_at(*position).is_none()
                && game.unit_ids_at(*position).is_empty()
        })
        .unwrap();
    let far_pos = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.wdist(*position, home_pos) >= 9 && game.city_at(*position).is_none())
        .unwrap();
    for position in [intruder_pos, observer_pos, far_pos] {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    game.spawn_test_unit("scout", 0, observer_pos);
    for _ in 0..4 {
        game.spawn_test_unit("modern_armor", 0, far_pos);
    }
    game.spawn_test_unit("warrior", 1, intruder_pos);
    for _ in 0..4 {
        game.spawn_test_unit("modern_armor", 1, intruder_pos);
    }
    home
}

#[test]
fn a_threatened_seat_keeps_attacking_in_the_final_war() {
    let mut game = two_capitals_at_war();
    let home = besiege_the_capital(&mut game);
    let mut ai = AdvancedAi::new();
    let ordinary = ai.assess(&game, 0);
    assert_eq!(ordinary.strategy, GrandStrategy::Recovery);
    assert_eq!(ordinary.threatened_city, Some(home));

    play_to_the_last_civ(&mut game);
    ai.plays_for_the_last_civ(&game, 0);
    let last = ai.assess(&game, 0);
    assert_ne!(
        last.strategy,
        GrandStrategy::Recovery,
        "there is no later to recover for"
    );
    assert_eq!(
        last.threatened_city,
        Some(home),
        "the threat is still named, so the board still defends the city"
    );
}

#[test]
fn the_final_war_wants_a_larger_army_while_the_treasury_carries_it() {
    let mut game = two_capitals_at_war();
    let ai = AdvancedAi::new();
    assert_eq!(
        ai.final_war_army_target(&game, 0, 3, 6),
        6,
        "an ordinary war keeps the shipped target"
    );
    play_to_the_last_civ(&mut game);
    game.players[0].gold = 0.0;
    game.players[0].gold_per_turn = 5.0;
    assert_eq!(
        ai.final_war_army_target(&game, 0, 3, 6),
        3 * FINAL_WAR_BODIES_PER_CITY
    );
    assert_eq!(
        ai.final_war_army_target(&game, 0, 1, 9),
        9,
        "never under the shipped target"
    );
    game.players[0].gold_per_turn = -5.0;
    game.players[0].gold = 5.0 * FINAL_WAR_DEFICIT_RUNWAY - 1.0;
    assert_eq!(
        ai.final_war_army_target(&game, 0, 3, 6),
        6,
        "a deficit the treasury cannot carry stops the arming"
    );
    game.players[0].gold = 5.0 * FINAL_WAR_DEFICIT_RUNWAY;
    assert_eq!(
        ai.final_war_army_target(&game, 0, 3, 6),
        3 * FINAL_WAR_BODIES_PER_CITY,
        "a treasury with the runway carries it"
    );
}

#[test]
fn an_idle_city_trains_a_soldier_only_in_the_final_war() {
    let mut game = two_capitals_at_war();
    let home = game.player_city_ids(0)[0];
    let mut ai = AdvancedAi::new();
    let plan = ai.assess(&game, 0);
    game.cities.get_mut(&home).unwrap().queue.clear();
    ai.arm_for_the_final_war(&mut game, 0, &plan);
    assert!(
        game.cities[&home].queue.is_empty(),
        "an ordinary war leaves the queue to the governors"
    );

    play_to_the_last_civ(&mut game);
    ai.plays_for_the_last_civ(&game, 0);
    let plan = ai.assess(&game, 0);
    game.cities.get_mut(&home).unwrap().queue.clear();
    game.players[0].gold_per_turn = 5.0;
    ai.arm_for_the_final_war(&mut game, 0, &plan);
    let head = game.cities[&home].queue.first().cloned();
    assert!(
        matches!(&head, Some(Item::Unit { unit }) if game.rules.units[unit].class == "military"),
        "the idle capital trains a soldier, got {head:?}"
    );

    // A queue already at work is not interrupted.
    let busy = game.cities[&home].queue.len();
    ai.arm_for_the_final_war(&mut game, 0, &plan);
    assert_eq!(game.cities[&home].queue.len(), busy);

    // Nor is anything started on a deficit the treasury cannot carry.
    game.cities.get_mut(&home).unwrap().queue.clear();
    game.players[0].gold = 0.0;
    game.players[0].gold_per_turn = -5.0;
    ai.arm_for_the_final_war(&mut game, 0, &plan);
    assert!(game.cities[&home].queue.is_empty());
}

/// Three civilizations on open ground: ours, a rival city five tiles away
/// and another rival's at least fourteen away.
fn three_civs_on_open_ground() -> (Game, u32, u32) {
    let mut game = Game::new_full(3, 40, 24, 7_221, 300, 0, false);
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    let mut tiles: Vec<Pos> = game.map.tiles.keys().copied().collect();
    tiles.sort_unstable();
    let home = tiles[tiles.len() / 2];
    let near = tiles
        .iter()
        .copied()
        .find(|pos| game.wdist(*pos, home) == 5)
        .expect("a tile five from home");
    let far = tiles
        .iter()
        .copied()
        .find(|pos| game.wdist(*pos, home) >= 14 && game.wdist(*pos, near) >= 10)
        .expect("a tile far from both");
    game.found_city_for(0, home, None);
    let near_city = game.found_city_for(1, near, None);
    let far_city = game.found_city_for(2, far, None);
    game.current = 0;
    game.turn = 200;
    for pair in [(0, 1), (0, 2), (1, 2)] {
        game.at_war.insert(pair);
    }
    (game, near_city, far_city)
}

#[test]
fn the_final_war_aims_at_the_nearest_rival_city_and_stays_on_it() {
    let (mut game, near_city, far_city) = three_civs_on_open_ground();
    let mut ai = AdvancedAi::new();
    assert_eq!(
        ai.final_war_objective(&game, 0),
        None,
        "an ordinary war keeps the planner's own target"
    );

    play_to_the_last_civ(&mut game);
    ai.plays_for_the_last_civ(&game, 0);
    assert_eq!(ai.final_war_objective(&game, 0), Some((1, near_city)));
    let plan = ai.assess(&game, 0);
    assert_eq!(plan.strategy, GrandStrategy::Conquest);
    assert_eq!(
        (plan.target_player, plan.target_city),
        (Some(1), Some(near_city)),
        "the plan aims where the objective says"
    );

    // A slightly nearer city does not pull a campaign off its objective;
    // a fresh plan with no objective takes the nearer one.
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    let nearer = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| {
            game.wdist(*pos, home) == 3
                && game
                    .cities
                    .values()
                    .all(|city| game.wdist(city.pos, *pos) >= 3)
        })
        .expect("a tile three from home");
    let nearer_city = game.found_city_for(2, nearer, None);
    ai.plan = Some(plan);
    assert_eq!(ai.final_war_objective(&game, 0), Some((1, near_city)));
    ai.plan = None;
    assert_eq!(ai.final_war_objective(&game, 0), Some((2, nearer_city)));

    // A city that falls to us is no objective: the next nearest is.
    game.cities.get_mut(&nearer_city).unwrap().owner = 0;
    game.cities.get_mut(&near_city).unwrap().owner = 0;
    assert_eq!(ai.final_war_objective(&game, 0), Some((2, far_city)));
}

#[test]
fn the_final_war_trains_a_wall_breaker_for_every_four_soldiers() {
    let mut game = two_capitals_at_war();
    let home = game.player_city_ids(0)[0];
    let home_pos = game.cities[&home].pos;
    game.players[0].techs.insert(crate::name!("engineering"));
    play_to_the_last_civ(&mut game);
    assert_eq!(
        AdvancedAi::final_war_breaker(&game, 0, home),
        None,
        "no army, no breaker owed"
    );

    for _ in 0..FINAL_WAR_SOLDIERS_PER_BREAKER {
        game.spawn_test_unit("warrior", 0, home_pos);
    }
    let breaker = AdvancedAi::final_war_breaker(&game, 0, home);
    assert!(
        matches!(&breaker, Some(Item::Unit { unit }) if game.rules.units[unit].siege),
        "four soldiers and nothing to open a wall: got {breaker:?}"
    );

    // The claim trains it ahead of another soldier.
    let ai = AdvancedAi::new();
    let plan = ai.assess(&game, 0);
    game.cities.get_mut(&home).unwrap().queue.clear();
    game.players[0].gold_per_turn = 50.0;
    ai.arm_for_the_final_war(&mut game, 0, &plan);
    assert_eq!(game.cities[&home].queue.first(), breaker.as_ref());

    // One in the queue answers for the four; a fifth soldier owes another.
    assert_eq!(AdvancedAi::final_war_breaker(&game, 0, home), None);
    game.spawn_test_unit("warrior", 0, home_pos);
    assert!(AdvancedAi::final_war_breaker(&game, 0, home).is_some());
}
