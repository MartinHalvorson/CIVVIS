use super::*;
use serde_json::json;

fn game_with_capitals(players: usize, seed: u64, max_turns: u32) -> Game {
    let mut game = Game::new_full(players, 26, 16, seed, max_turns, 0, false);
    for pid in 0..players {
        let position = game
            .player_unit_ids(pid)
            .into_iter()
            .find_map(|unit| {
                let unit = &game.units[&unit];
                (unit.kind == "settler").then_some(unit.pos)
            })
            .unwrap();
        game.found_city_for(pid, position, None);
    }
    game
}

fn rules_with_runtime_modifier() -> Rules {
    let mut files = Rules::shipped_values();
    files.insert(
        "modifiers".to_string(),
        json!({
            "congress_public_works": {
                "effects": {"builder_production_pct": 25}
            }
        }),
    );
    Rules::from_values(files).unwrap()
}

fn install_runtime_modifier(game: &mut Game) {
    let modifiers = rules_with_runtime_modifier().modifiers;
    let mut rules = (*game.rules).clone();
    rules.modifiers = modifiers;
    game.rules = Arc::new(rules);
}

#[test]
fn player_type_attachment_targets_only_that_class_and_reaches_engine_consumers() {
    let mut game = game_with_capitals(2, 86_001, 80);
    install_runtime_modifier(&mut game);
    let city = game.player_city_ids(0)[0];
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    let baseline = game.item_prod_mult(0, city, Some(&builder));

    assert_eq!(
        game.attach_modifier_to_player_type(
            "PLAYERTYPE_MAJOR",
            "congress_public_works"
        )
        .unwrap(),
        2
    );
    assert_eq!(game.player_modifier_effect(0, "builder_production_pct"), 25.0);
    assert_eq!(game.policy_effect(0, "builder_production_pct"), 25.0);
    assert_eq!(game.item_prod_mult(0, city, Some(&builder)), baseline + 0.25);
    for player in game.players.iter().filter(|player| {
        player.is_minor || player.is_barbarian || player.is_free_city
    }) {
        assert!(player.attached_modifiers.is_empty());
    }

    // Modifier identity is stable: applying one resolution twice does
    // not stack it, and expiry removes exactly the original targets.
    assert_eq!(
        game.attach_modifier_to_player_type("major", "congress_public_works")
            .unwrap(),
        0
    );
    assert_eq!(
        game.detach_modifier_from_player_type("player_major", "congress_public_works")
            .unwrap(),
        2
    );
    assert_eq!(game.item_prod_mult(0, city, Some(&builder)), baseline);
}

#[test]
fn player_type_attachment_supports_minor_barbarian_and_free_city_seats() {
    let mut game = Game::new_full(2, 24, 16, 86_002, 80, 1, true);
    install_runtime_modifier(&mut game);
    let count = |game: &Game, kind: &str| {
        game.players
            .iter()
            .filter(|player| Game::player_has_type(player, kind))
            .count()
    };

    for (selector, kind) in [
        ("city_state", "minor"),
        ("barbarian", "barbarian"),
        ("free_cities", "free_cities"),
    ] {
        let expected = count(&game, kind);
        assert!(expected > 0, "test game has no {kind} seat");
        assert_eq!(
            game.attach_modifier_to_player_type(selector, "congress_public_works")
                .unwrap(),
            expected
        );
        assert_eq!(
            game.detach_modifier_from_player_type(selector, "congress_public_works")
                .unwrap(),
            expected
        );
    }
    assert!(game
        .attach_modifier_to_player_type("spectator", "congress_public_works")
        .unwrap_err()
        .contains("unknown player type"));
    assert!(game
        .attach_modifier_to_player_type("major", "missing")
        .unwrap_err()
        .contains("unknown modifier"));
}

#[test]
fn runtime_modifier_attachments_survive_save_round_trip() {
    let mut game = game_with_capitals(2, 86_003, 80);
    install_runtime_modifier(&mut game);
    game.attach_modifier_to_player(0, "congress_public_works")
        .unwrap();

    let restored: Game =
        serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert!(restored.players[0]
        .attached_modifiers
        .contains("congress_public_works"));
    assert!(restored.players[1].attached_modifiers.is_empty());
}
