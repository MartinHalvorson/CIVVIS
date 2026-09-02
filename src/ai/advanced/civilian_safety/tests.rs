use super::super::GrandStrategy;
use super::*;
use crate::game::Action;

fn open_land(g: &Game, pos: Pos) -> bool {
    g.city_at(pos).is_none()
        && g.unit_ids_at(pos).is_empty()
        && g.map
            .get(pos)
            .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
}

/// The live game can hand a Settler to any hostile military owner, not only
/// the Barbarian seat. Keep the native/evaluator envelope frozen, but make
/// the host lessons model the same `resolve_entered_units` rule.
#[test]
fn live_capture_reach_includes_an_at_war_major_without_barbarians() {
    let mut game = Game::new_full(2, 20, 14, 91_401, 60, 0, false);
    game.current = 0;
    let founding_settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("player 0 has a starting Settler");
    game.apply(
        0,
        &Action::FoundCity {
            unit: founding_settler,
        },
    )
    .expect("the starting Settler founds the capital");
    let home = game
        .cities
        .values()
        .find(|city| city.owner == 0)
        .expect("player 0 has a capital")
        .pos;
    let start = game
        .nbrs(home)
        .into_iter()
        .find(|pos| open_land(&game, *pos))
        .expect("open land beside the capital");
    let hostile_pos = game
        .nbrs(start)
        .into_iter()
        .find(|pos| open_land(&game, *pos))
        .expect("open land beside the Settler");
    let settler = game.spawn_test_unit("settler", 0, start);
    let hostile = game.spawn_test_unit("warrior", 1, hostile_pos);
    game.at_war.insert((0, 1));
    game.at_war.insert((1, 0));

    let mut native = AdvancedAi::new();
    native.enable_civilian_out_of_reach();
    assert!(
        !native
            .barbarian_reach(&game, 0, start, REACH_SCAN_RADIUS)
            .covers(&game, start),
        "native screens retain the barbarian-only envelope"
    );

    let mut live = AdvancedAi::new();
    live.enable_live_settler_capture_lessons();
    let reach = live.barbarian_reach(&game, 0, start, REACH_SCAN_RADIUS);
    assert!(
        reach.covers(&game, start),
        "the visible at-war major's Warrior can capture the Settler"
    );
    assert!(
        reach.raiders.iter().any(|raider| raider.pos == hostile_pos),
        "the major's unit is represented in the live reach"
    );

    game.at_war.clear();
    assert!(
        live.barbarian_reach(&game, 0, start, REACH_SCAN_RADIUS)
            .is_empty(),
        "a peaceful rival is not a capture threat"
    );
    let _ = settler;
    let _ = hostile;
}

/// A locally clear live board with one Builder inside a Warrior's full
/// movement envelope. The path's first hex is still covered, while the second
/// is safe; this catches a one-step order that strands the Builder in reach.
fn live_builder_escape_fixture() -> (Game, u32, Pos, Pos) {
    let mut game = Game::new_full(2, 20, 14, 91_402, 60, 0, false);
    game.current = 0;
    let founding_settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("player 0 has a starting Settler");
    game.apply(
        0,
        &Action::FoundCity {
            unit: founding_settler,
        },
    )
    .expect("the starting Settler founds the capital");
    let home = game.player_city_ids(0)[0];
    let home = game.cities[&home].pos;
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.pillaged = false;
    }
    game.players[0]
        .explored
        .extend(game.map.tiles.keys().copied());
    game.players[1].is_barbarian = true;
    game.barb_pid = Some(1);
    game.at_war.insert((0, 1));
    game.at_war.insert((1, 0));

    let (current, middle, refuge, raider_at) = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|current| open_land(&game, *current) && game.wdist(*current, home) >= 4)
        .find_map(|current| {
            game.nbrs(current)
                .into_iter()
                .filter(|raider| open_land(&game, *raider))
                .find_map(|raider| {
                    game.nbrs(current)
                        .into_iter()
                        .filter(|middle| {
                            open_land(&game, *middle) && game.wdist(*middle, raider) == 2
                        })
                        .find_map(|middle| {
                            game.nbrs(middle)
                                .into_iter()
                                .find(|refuge| {
                                    open_land(&game, *refuge)
                                        && game.wdist(*refuge, current) == 2
                                        && game.wdist(*refuge, raider) == 3
                                })
                                .map(|refuge| (current, middle, refuge, raider))
                        })
                })
        })
        .expect("a three-hex land line away from the capital");
    let builder = game.spawn_test_unit("builder", 0, current);
    let raider = game.spawn_test_unit("warrior", 1, raider_at);
    assert!(
        game.player_can_see(0, raider_at),
        "the Builder's own vision exposes the nearby Barbarian"
    );
    let reach = game.threat_reach(raider);
    assert!(reach.contains(&current) && reach.contains(&middle));
    assert!(
        !reach.contains(&refuge),
        "the second Builder step must be outside the Warrior's reach"
    );
    (game, builder, middle, refuge)
}

#[test]
fn live_builder_uses_the_full_turn_escape_before_work() {
    let (mut game, builder, middle, _refuge) = live_builder_escape_fixture();
    let start = game.units[&builder].pos;
    let mut live = AdvancedAi::new();
    live.enable_live_settler_capture_lessons();
    assert!(live.civilian_reach_safety_on());

    assert!(
        live.advanced_builder_step(&mut game, 0, builder, GrandStrategy::Expansion),
        "the live Builder spends its turn escaping rather than starting work"
    );
    let escaped = game.units[&builder].pos;
    assert_ne!(escaped, middle);
    assert_eq!(game.wdist(start, escaped), 2, "the full safe route is used");
    let reach = live.barbarian_reach(&game, 0, escaped, REACH_SCAN_RADIUS);
    assert!(
        !reach.covers(&game, escaped),
        "the Builder ends the turn outside the known capture envelope"
    );
}

#[test]
fn live_builder_does_not_borrow_a_military_stack_as_protection() {
    let (mut game, builder, _middle, _refuge) = live_builder_escape_fixture();
    let current = game.units[&builder].pos;
    game.spawn_test_unit("warrior", 0, current);
    let mut live = AdvancedAi::new();
    live.enable_live_settler_capture_lessons();
    let reach = live.barbarian_reach(&game, 0, current, REACH_SCAN_RADIUS);
    assert!(reach.covers(&game, current));
    assert!(
        !live.civilian_safe_at(&game, 0, builder, current, &reach),
        "the guard is free to move later this turn, so it cannot make a live Builder safe"
    );
}

#[test]
fn live_builder_refuses_a_job_step_a_barbarian_can_take() {
    let mut game = Game::new_full(2, 20, 14, 91_403, 60, 0, false);
    game.current = 0;
    let founding_settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("player 0 has a starting Settler");
    game.apply(
        0,
        &Action::FoundCity {
            unit: founding_settler,
        },
    )
    .expect("the starting Settler founds the capital");
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.pillaged = false;
    }
    game.players[1].is_barbarian = true;
    game.barb_pid = Some(1);
    game.at_war.insert((0, 1));
    game.at_war.insert((1, 0));
    let target = game
        .nbrs(home)
        .into_iter()
        .find(|pos| open_land(&game, *pos))
        .expect("an open Builder job beside the capital");
    let raider_at = game
        .nbrs(target)
        .into_iter()
        .find(|pos| open_land(&game, *pos) && game.wdist(*pos, home) > 1)
        .expect("an open Barbarian tile beside the job but outside the city");
    let builder = game.spawn_test_unit("builder", 0, home);
    let raider = game.spawn_test_unit("warrior", 1, raider_at);
    assert!(game.player_can_see(0, raider_at));

    let mut live = AdvancedAi::new();
    live.enable_live_settler_capture_lessons();
    let reach = live.barbarian_reach(&game, 0, home, REACH_SCAN_RADIUS);
    assert!(reach.covers(&game, target));
    assert!(
        !live.builder_step_out_of_reach(&mut game, 0, builder, target),
        "the live Builder holds in the city rather than take an exposed job step"
    );
    assert_eq!(game.units[&builder].pos, home);

    game.current = 1;
    game.apply(
        1,
        &Action::Move {
            unit: raider,
            to: target,
        },
    )
    .expect("the Barbarian can occupy the job during its hostile phase");
    assert_eq!(
        game.units[&builder].owner, 0,
        "holding in the city preserves the Builder instead of donating it"
    );
}
