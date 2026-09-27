use super::*;

fn rival_amani_fixture() -> (Game, usize, u32) {
    let mut game = Game::new_full(2, 40, 26, 379_301, 500, 1, false);
    game.turn = 10;
    game.current = 0;
    let minor = game
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_barbarian)
        .unwrap()
        .id;
    let city = game.player_city_ids(minor)[0];
    game.record_contact(0, 1);
    game.record_contact(0, minor);
    game.record_contact(1, minor);
    game.players[0].envoys = vec![(minor, 18)];
    game.players[0].envoys_free = 10;
    game.players[1].envoys = vec![(minor, 18)];
    game.players[1].governor_roster.insert(
        "amani".into(),
        GovernorState {
            city: Some(city),
            assigned_turn: 0,
            disabled_until: 0,
            promotions: Default::default(),
        },
    );
    game.sync_governor_cities(1);
    game.at_war.insert((0, 1));
    assert!(game.governor_established(1, "amani"));
    assert_eq!(game.envoys_at(1, minor), 20);
    assert_eq!(game.suzerain_of(minor), Some(1));
    assert!(game.is_at_war(0, minor));
    (game, minor, city)
}

#[test]
fn rival_effective_envoys_preserve_suzerainty_and_derived_war() {
    let (game, minor, _) = rival_amani_fixture();
    let mut view = game.player_decision_view(0);
    assert!(view.players[1].governor_roster.is_empty());
    assert_eq!(view.envoys_at(1, minor), 20);
    assert_eq!(view.suzerain_of(minor), Some(1));
    assert!(view.is_at_war(0, minor));
    assert!(!view.can_send_envoy(0, minor));
    assert!(!view
        .legal_actions_within(0, ActionFamilies::EMPIRE)
        .contains(&Action::SendEnvoy { player: minor }));
    assert_eq!(
        view.apply(0, &Action::SendEnvoy { player: minor }),
        Err("invalid city-state".into())
    );
    assert_eq!(view.players[0].envoys_free, 10);
}

#[test]
fn puppeteer_without_any_raw_envoy_entry_is_still_public() {
    let (mut game, minor, _) = rival_amani_fixture();
    game.players[0].envoys.clear();
    game.players[1].envoys.clear();
    game.players[1]
        .governor_roster
        .get_mut("amani")
        .unwrap()
        .promotions
        .insert("puppeteer".into());
    assert_eq!(game.envoys_at(1, minor), 4);
    let view = game.player_decision_view(0);
    assert_eq!(view.envoys_at(1, minor), 4);
    assert_eq!(view.suzerain_of(minor), Some(1));
    assert!(view.is_at_war(0, minor));
    assert!(view.players[1].governor_roster.is_empty());
}

#[test]
fn reading_public_envoys_does_not_mutate_world_or_disclose_rival_governors() {
    let (game, minor, _) = rival_amani_fixture();
    let before = serde_json::to_value(&game).unwrap();
    let view = game.player_decision_view(0);
    assert!(view.players[1].governor_roster.is_empty());
    assert!(view.players[1].governors.is_empty());
    assert_eq!(view.envoys_at(1, minor), game.envoys_at(1, minor));
    assert_eq!(serde_json::to_value(&game).unwrap(), before);
}

#[test]
fn own_raw_envoys_and_amani_are_not_counted_twice() {
    let (mut game, minor, city) = rival_amani_fixture();
    let amani = game.players[1].governor_roster.remove("amani").unwrap();
    game.sync_governor_cities(1);
    game.players[0]
        .governor_roster
        .insert("amani".into(), amani);
    game.sync_governor_cities(0);
    let remembered = game.remember_city(&game.cities[&city]);
    game.players[0].remembered_cities.insert(city, remembered);
    assert_eq!(game.envoys_at(0, minor), 20);
    let view = game.player_decision_view(0);
    assert_eq!(view.players[0].envoys, game.players[0].envoys);
    assert_eq!(view.envoys_at(0, minor), 20);
    assert_eq!(view.suzerain_of(minor), Some(0));
}

#[test]
fn an_unmet_city_states_delegation_is_not_disclosed() {
    let (mut game, minor, _) = rival_amani_fixture();
    game.players[0].met.remove(&minor);
    assert!(!game.has_met(0, minor));
    let view = game.player_decision_view(0);
    assert!(!view.players[1]
        .envoys
        .iter()
        .any(|(seat, _)| *seat == minor));
    assert!(view.players[1].governor_roster.is_empty());
}

#[test]
fn public_effective_totals_survive_repeated_observation_without_inflation() {
    let (game, minor, _) = rival_amani_fixture();
    let view = game.player_decision_view(0);
    let repeated = view.player_decision_view(0);
    assert_eq!(view.envoys_at(1, minor), 20);
    assert_eq!(repeated.envoys_at(1, minor), 20);
    assert_eq!(repeated.suzerain_of(minor), Some(1));
    assert!(repeated.is_at_war(0, minor));
    assert!(repeated.players[1].governor_roster.is_empty());
}

#[test]
fn refreshed_observation_tracks_a_neutralized_amani_and_real_tie() {
    let (mut game, minor, _) = rival_amani_fixture();
    let before = game.player_decision_view(0);
    game.players[1]
        .governor_roster
        .get_mut("amani")
        .unwrap()
        .disabled_until = game.turn + 1;
    assert_eq!(game.envoys_at(1, minor), 18);
    let after = game.player_decision_view(0);
    assert_eq!(before.envoys_at(1, minor), 20);
    assert_eq!(after.envoys_at(1, minor), 18);
    assert_eq!(after.suzerain_of(minor), None);
    assert!(!after.is_at_war(0, minor));
    assert!(after.can_send_envoy(0, minor));
}

#[test]
fn observed_dispatcher_does_not_repeat_a_ghost_wartime_envoy() {
    use crate::ai::{AdvancedAi, Ai, VictoryTarget};
    let (mut game, minor, _) = rival_amani_fixture();
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.found_city_for(0, game.units[&settler].pos, None);
    game.remove_unit(settler);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_engine_repairs();
    assert!(ai.uses_player_observation());
    let start = game.log.len();
    ai.take_turn(&mut game, 0);
    assert!(!game.log.since(start).any(|(seat, action)| {
        *seat == 0 && matches!(action, Action::SendEnvoy { player } if *player == minor)
    }));
    assert_eq!(
        game.players[0]
            .counters
            .get("player:refused")
            .copied()
            .unwrap_or(0),
        0,
        "the real observed dispatcher must not spend its refresh frames on the false Envoy offer"
    );
    assert!(
        game.log.since(start).any(|(seat, action)| {
            *seat == 0 && matches!(action, Action::Move { .. } | Action::MoveTo { .. })
        }),
        "the independent army must still act"
    );
}
