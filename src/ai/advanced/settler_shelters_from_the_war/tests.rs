use super::super::test_support::opt_in_off_in_both_controllers;
use super::*;

/// Two majors with a capital each on flat, explored ground, no units, met.
/// Returns the board, our capital and the rival capital.
fn board() -> (Game, Pos, Pos) {
    let mut game = Game::new_full(2, 30, 20, 8_133, 120, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        let pos = game.units[&settler].pos;
        game.remove_unit(settler);
        game.found_city_for(pid, pos, None);
    }
    for unit in game
        .player_unit_ids(0)
        .into_iter()
        .chain(game.player_unit_ids(1))
    {
        game.remove_unit(unit);
    }
    let positions: Vec<Pos> = game.map.tiles.keys().copied().collect();
    for position in positions {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.improvement = None;
        game.players[0].explored.insert(position);
    }
    game.barb_camps.clear();
    game.record_contact(0, 1);
    game.current = 0;
    game.turn = 98;
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    let rival = game.cities[&game.player_city_ids(1)[0]].pos;
    (game, home, rival)
}

/// The tile `from_home` tiles out from our capital, farthest from the rival.
fn open_tile(game: &Game, home: Pos, rival: Pos, from_home: i32) -> Pos {
    let mut tiles: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|p| game.wdist(*p, home) == from_home && game.city_at(*p).is_none())
        .collect();
    tiles.sort_by_key(|p| (std::cmp::Reverse(game.wdist(*p, rival)), *p));
    tiles[0]
}

/// A tile exactly `distance` from `at`, not a city, not `at` itself.
fn tile_at(game: &Game, at: Pos, distance: i32, avoid: Pos) -> Pos {
    let mut tiles: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|p| {
            game.wdist(*p, at) == distance
                && game.city_at(*p).is_none()
                && game.wdist(*p, avoid) > 1
        })
        .collect();
    tiles.sort_unstable();
    tiles[0]
}

fn sheltering() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_settler_shelters_from_the_war();
    ai
}

#[test]
fn settler_shelters_from_the_war_is_opt_in() {
    opt_in_off_in_both_controllers("settler-shelters-from-the-war", |ai| {
        ai.settler_shelters_from_the_war
    });
}

/// civvis-20261105T044106Z t98-t100: at war, an enemy soldier three tiles
/// from a Settler in the open. Off, the routine keeps the turn; on, the gene
/// takes it, the Settler gets no farther from home, and its target is kept.
#[test]
fn a_settler_in_the_open_near_an_enemy_soldier_heads_home() {
    let (mut game, home, rival) = board();
    game.at_war.insert((0, 1));
    let here = open_tile(&game, home, rival, 3);
    let settler = game.spawn_test_unit("settler", 0, here);
    game.spawn_test_unit("warrior", 1, tile_at(&game, here, 3, home));
    let site = open_tile(&game, home, rival, 6);

    let mut off = AdvancedAi::new();
    off.settler_targets.insert(settler, site);
    assert_eq!(
        off.settler_shelters_from_the_war_step(&mut game.clone(), 0, settler),
        None
    );

    let mut on = sheltering();
    on.settler_targets.insert(settler, site);
    assert_eq!(
        on.settler_shelters_from_the_war_step(&mut game, 0, settler),
        Some(true)
    );
    assert!(game.wdist(game.units[&settler].pos, home) <= 3);
    assert_eq!(on.settler_targets.get(&settler), Some(&site));
}

/// A Settler already in our city holds there while the soldier is near.
#[test]
fn a_settler_in_a_city_stays() {
    let (mut game, home, _rival) = board();
    game.at_war.insert((0, 1));
    let settler = game.spawn_test_unit("settler", 0, home);
    game.spawn_test_unit("warrior", 1, tile_at(&game, home, 3, home));
    let mut on = sheltering();
    assert_eq!(
        on.settler_shelters_from_the_war_step(&mut game, 0, settler),
        Some(true)
    );
    assert_eq!(game.units[&settler].pos, home);
}

/// At peace, beyond the radius, or against a barbarian, the gene stands
/// aside.
#[test]
fn peace_distance_and_barbarians_leave_the_routine_alone() {
    let (base, home, rival) = board();
    let here = open_tile(&base, home, rival, 3);

    let mut peace = base.clone();
    let settler = peace.spawn_test_unit("settler", 0, here);
    peace.spawn_test_unit("warrior", 1, tile_at(&peace, here, 3, home));
    assert_eq!(
        sheltering().settler_shelters_from_the_war_step(&mut peace, 0, settler),
        None
    );

    let mut far = base.clone();
    far.at_war.insert((0, 1));
    let settler = far.spawn_test_unit("settler", 0, here);
    far.spawn_test_unit(
        "warrior",
        1,
        tile_at(&far, here, WAR_SHELTER_RADIUS + 1, home),
    );
    assert_eq!(
        sheltering().settler_shelters_from_the_war_step(&mut far, 0, settler),
        None
    );

    let mut barbarian = base.clone();
    let barb = barbarian
        .players
        .iter()
        .position(|player| player.is_barbarian)
        .expect("the board seats the barbarians");
    let settler = barbarian.spawn_test_unit("settler", 0, here);
    barbarian.spawn_test_unit("warrior", barb, tile_at(&barbarian, here, 2, home));
    assert_eq!(
        sheltering().settler_shelters_from_the_war_step(&mut barbarian, 0, settler),
        None
    );
}

/// Standing on its own target with the city legal, the Settler is left to
/// found: the city is the shelter.
#[test]
fn a_settler_on_its_foundable_site_is_left_to_found() {
    let (mut game, home, rival) = board();
    game.at_war.insert((0, 1));
    let here = open_tile(&game, home, rival, 5);
    let settler = game.spawn_test_unit("settler", 0, here);
    game.spawn_test_unit("warrior", 1, tile_at(&game, here, 3, home));
    assert!(
        game.can_found_city(settler),
        "the test site must be foundable"
    );
    let mut on = sheltering();
    on.settler_targets.insert(settler, here);
    assert_eq!(
        on.settler_shelters_from_the_war_step(&mut game, 0, settler),
        None
    );
}

/// civvis-20261105T074444Z t31: at peace, a rival soldier stood beside the
/// Settler and took it on the turn its empire declared. A non-friend's
/// soldier within two tiles sends the Settler home; a declared friend's
/// does not.
#[test]
fn a_non_friends_soldier_beside_the_settler_at_peace_sends_it_home() {
    let (base, home, rival) = board();
    let here = open_tile(&base, home, rival, 3);

    let mut stranger = base.clone();
    let settler = stranger.spawn_test_unit("settler", 0, here);
    stranger.spawn_test_unit(
        "warrior",
        1,
        tile_at(&stranger, here, SURPRISE_SHELTER_RADIUS, home),
    );
    assert_eq!(
        sheltering().settler_shelters_from_the_war_step(&mut stranger, 0, settler),
        Some(true)
    );

    let mut friend = base.clone();
    let settler = friend.spawn_test_unit("settler", 0, here);
    friend.spawn_test_unit(
        "warrior",
        1,
        tile_at(&friend, here, SURPRISE_SHELTER_RADIUS, home),
    );
    let until = friend.turn + 30;
    friend.players[0].friends_until.insert(1, until);
    friend.players[1].friends_until.insert(0, until);
    assert_eq!(
        sheltering().settler_shelters_from_the_war_step(&mut friend, 0, settler),
        None
    );
}
