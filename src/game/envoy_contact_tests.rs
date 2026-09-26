use super::*;

fn host_envoy_fixture(allowed: bool, war: bool) -> (Game, usize) {
    let mut game = Game::new_full(2, 24, 16, 90_731, 120, 1, false);
    let minor = game
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_barbarian)
        .unwrap()
        .id;
    game.record_contact(0, minor);
    game.players[0].envoys_free = 1;
    if war {
        game.at_war.insert((0, minor));
    }
    Arc::make_mut(&mut game.host_envoy_permissions).insert((0, minor), (game.turn, allowed));
    (game, minor)
}

#[test]
fn host_envoy_permission_can_allow_a_wartime_placement() {
    let (mut game, minor) = host_envoy_fixture(true, true);
    assert!(game.is_at_war(0, minor));
    assert!(game.can_send_envoy(0, minor));
    assert!(game
        .legal_actions_within(0, ActionFamilies::EMPIRE)
        .iter()
        .any(|action| matches!(action, Action::SendEnvoy { player } if *player == minor)));
    let before = game.raw_envoys_at(0, minor);
    game.apply(0, &Action::SendEnvoy { player: minor }).unwrap();
    assert_eq!(game.raw_envoys_at(0, minor), before + 1);
    assert_eq!(game.players[0].envoys_free, 0);
}

#[test]
fn host_envoy_refusal_blocks_peacetime_enumeration_and_apply() {
    let (mut game, minor) = host_envoy_fixture(false, false);
    assert!(!game.can_send_envoy(0, minor));
    assert!(!game
        .legal_actions_within(0, ActionFamilies::EMPIRE)
        .iter()
        .any(|action| matches!(action, Action::SendEnvoy { player } if *player == minor)));
    assert!(game.apply(0, &Action::SendEnvoy { player: minor }).is_err());
    assert_eq!(game.players[0].envoys_free, 1);
    game.turn += 1;
    assert!(game.can_send_envoy(0, minor), "an old refusal must expire");
}

#[test]
fn host_envoy_grant_is_scoped_to_actor_target_and_turn() {
    let (mut game, minor) = host_envoy_fixture(true, true);
    game.record_contact(1, minor);
    game.players[1].envoys_free = 1;
    game.at_war.insert((1, minor));
    assert!(!game.can_send_envoy(1, minor));
    assert!(!game.can_send_envoy(0, 1));
    game.turn += 1;
    assert!(!game.can_send_envoy(0, minor), "an old grant must expire");
}

#[test]
fn host_envoy_grant_does_not_bypass_contact_supply_or_life() {
    let (mut game, minor) = host_envoy_fixture(true, true);
    game.players[0].envoys_free = 0;
    assert!(!game.can_send_envoy(0, minor));
    game.players[0].envoys_free = 1;
    game.players[minor].alive = false;
    assert!(!game.can_send_envoy(0, minor));
    game.players[minor].alive = true;
    game.players[0].met.clear();
    game.players[minor].met.clear();
    assert!(!game.can_send_envoy(0, minor));
}

#[test]
fn envoys_require_contact_in_both_enumeration_and_apply() {
    let mut game = Game::new_full(1, 24, 16, 90_731, 120, 2, false);
    let city_states: Vec<usize> = game
        .players
        .iter()
        .filter(|player| player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .collect();
    assert_eq!(city_states.len(), 2);
    let hidden = city_states[0];
    let known = city_states[1];

    for player in 0..game.players.len() {
        game.players[player].met.clear();
    }
    game.record_contact(0, known);
    // Keep this legality fixture independent of the automatic Envoy for
    // first discovering the known city-state.
    game.players[0].envoys.clear();
    game.players[0].envoys_free = 1;

    assert!(!game.can_send_envoy(0, hidden));
    assert!(game.can_send_envoy(0, known));
    let actions = game.legal_actions_within(0, ActionFamilies::EMPIRE);
    assert!(!actions
        .iter()
        .any(|action| matches!(action, Action::SendEnvoy { player } if *player == hidden)));
    assert!(actions
        .iter()
        .any(|action| matches!(action, Action::SendEnvoy { player } if *player == known)));

    assert_eq!(
        game.apply(0, &Action::SendEnvoy { player: hidden }),
        Err("invalid city-state".to_string())
    );
    assert_eq!(game.players[0].envoys_free, 1);
    assert_eq!(game.raw_envoys_at(0, hidden), 0);

    game.apply(0, &Action::SendEnvoy { player: known }).unwrap();
    assert_eq!(game.players[0].envoys_free, 0);
    assert_eq!(game.raw_envoys_at(0, known), 1);
}
