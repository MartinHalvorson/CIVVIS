use super::*;

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
