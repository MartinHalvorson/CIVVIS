use super::super::*;
use crate::mirror::{HostUnitDeath, StateSnapshot};

fn departure() -> (Game, AdvancedAi, StateSnapshot, u32, u32, Pos) {
    let mut game = Game::new_full(2, 24, 16, 91_412, 150, 0, false);
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
    }
    game.current = 0;
    let home = (4, 4);
    let founder = game.spawn_test_unit("settler", 0, home);
    game.apply(0, &Action::FoundCity { unit: founder }).unwrap();
    let settler = game.spawn_test_unit("settler", 0, home);
    let _guard = game.spawn_test_unit("warrior", 0, (4, 3));
    let hostile = game.spawn_test_unit("spearman", 1, (6, 4));
    Arc::make_mut(&mut game.host_unit_facts).insert(
        hostile,
        crate::game::HostUnitFacts {
            civ6_id: Some(851980),
            ..Default::default()
        },
    );
    game.at_war.insert((0, 1));
    game.turn = 9;
    let mut ai = AdvancedAi::new();
    ai.enable_live_formationless_settler_shadow();
    ai.enable_live_settler_capture_lessons();
    ai.enable_civilian_out_of_reach();
    ai.observe_turn_start_hostiles(&game, 0);
    assert!(ai.hostile_last_seen.contains_key(&851980));
    let mut state = StateSnapshot::default();
    state.seat.players = 2;
    state.confirmed_unit_deaths.push(HostUnitDeath {
        player: 1,
        unit: 851980,
        turn: 9,
    });
    ai.settler_targets.insert(settler, (8, 4));
    (game, ai, state, settler, hostile, home)
}

#[test]
fn confirmed_kill_lets_the_new_settler_depart_without_an_escort_wait() {
    let (mut game, mut ai, mut state, settler, hostile, home) = departure();
    game.remove_unit(hostile);
    game.turn = 12;
    state.turn = 12;
    ai.observe_turn_start_hostiles(&game, 0);
    let mut stale_game = game.clone();
    let mut stale_ai = ai.clone();
    assert!(!stale_ai.advanced_settler_step(&mut stale_game, 0, settler));
    assert_eq!(stale_game.units[&settler].pos, home);

    ai.observe_confirmed_host_deaths(&game, &state);
    assert!(!ai.hostile_last_seen.contains_key(&851980));
    assert!(ai.advanced_settler_step(&mut game, 0, settler));
    assert_ne!(game.units[&settler].pos, home);
    assert!(game.wdist(game.units[&settler].pos, (8, 4)) < 4);
}

#[test]
fn fog_and_unconfirmed_simulated_kills_keep_the_capture_threat() {
    let (mut game, mut ai, mut state, _, hostile, _) = departure();
    game.remove_unit(hostile);
    state.confirmed_unit_deaths.clear();
    ai.observe_confirmed_host_deaths(&game, &state);
    assert!(ai.hostile_last_seen.contains_key(&851980));
    assert!(ai
        .turn_start_hostiles
        .iter()
        .any(|unit| unit.host_key == 851980));
}

#[test]
fn host_confirmation_clears_both_same_turn_threat_caches() {
    let (mut game, mut ai, state, _, hostile, _) = departure();
    game.remove_unit(hostile);
    ai.observe_confirmed_host_deaths(&game, &state);
    assert!(!ai.hostile_last_seen.contains_key(&851980));
    assert!(!ai
        .turn_start_hostiles
        .iter()
        .any(|unit| unit.host_key == 851980));
}

#[test]
fn wrong_owner_future_kill_and_newer_sighting_do_not_erase_memory() {
    let (mut game, ai, mut state, _, hostile, _) = departure();
    game.remove_unit(hostile);
    for (player, turn) in [(0, 9), (1, 10), (1, 8)] {
        let mut tested = ai.clone();
        state.confirmed_unit_deaths[0].player = player;
        state.confirmed_unit_deaths[0].turn = turn;
        tested.observe_confirmed_host_deaths(&game, &state);
        assert!(tested.hostile_last_seen.contains_key(&851980));
    }
}

#[test]
fn an_authoritatively_visible_unit_overrides_an_old_casualty() {
    let (game, mut ai, state, _, _, _) = departure();
    ai.observe_confirmed_host_deaths(&game, &state);
    assert!(ai.hostile_last_seen.contains_key(&851980));
    assert!(ai
        .turn_start_hostiles
        .iter()
        .any(|unit| unit.host_key == 851980));
}
