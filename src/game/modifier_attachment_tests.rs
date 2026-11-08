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
    // Merged into the imported catalog rather than replacing it: the shipped
    // ruleset attaches imported bundles by name and will not build without them.
    let files = Rules::shipped_values_with(json!({
        "congress_public_works": {
            "effects": {"builder_production_pct": 25}
        }
    }));
    Rules::from_values(files).unwrap()
}

fn install_runtime_modifier(game: &mut Game) {
    let modifiers = rules_with_runtime_modifier().modifiers;
    let mut rules = (*game.rules).clone();
    rules.modifiers = modifiers;
    game.rules = Arc::new(rules);
}

#[test]
fn an_attachment_reaches_engine_consumers_and_detaches_cleanly() {
    let mut game = game_with_capitals(2, 86_001, 80);
    install_runtime_modifier(&mut game);
    let city = game.player_city_ids(0)[0];
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    let baseline = game.item_prod_mult(0, city, Some(&builder));

    assert!(game
        .attach_modifier_to_player(0, "congress_public_works")
        .unwrap());
    assert_eq!(
        game.player_modifier_effect(0, "builder_production_pct"),
        25.0
    );
    assert_eq!(game.policy_effect(0, "builder_production_pct"), 25.0);
    assert_eq!(
        game.item_prod_mult(0, city, Some(&builder)),
        baseline + 0.25
    );
    assert!(game.players[1].attached_modifiers.is_empty());

    // Modifier identity is stable: applying one resolution twice does
    // not stack it, and expiry removes exactly the original target.
    assert!(!game
        .attach_modifier_to_player(0, "congress_public_works")
        .unwrap());
    assert!(game.detach_modifier_from_player(0, "congress_public_works"));
    assert_eq!(game.item_prod_mult(0, city, Some(&builder)), baseline);
    assert!(game
        .attach_modifier_to_player(0, "missing")
        .unwrap_err()
        .contains("unknown modifier"));
}

#[test]
fn runtime_modifier_attachments_survive_save_round_trip() {
    let mut game = game_with_capitals(2, 86_003, 80);
    install_runtime_modifier(&mut game);
    game.attach_modifier_to_player(0, "congress_public_works")
        .unwrap();

    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert!(restored.players[0]
        .attached_modifiers
        .contains("congress_public_works"));
    assert!(restored.players[1].attached_modifiers.is_empty());
}

#[test]
fn conditional_collections_follow_player_state_without_leaking_scope() {
    let mut game = game_with_capitals(2, 86_004, 80);
    let city = game.player_city_ids(0)[0];
    let files = Rules::shipped_values_with(json!({
        "democracy_cities": {
            "collection": "player_cities",
            "requirements": {"all": [{"government": "democracy"}]},
            "effects": {"city_production": 7}
        },
        "democracy_units": {
            "collection": "player_units",
            "requirements": {"all": [{"government": "democracy"}]},
            "effects": {"infantry_production_pct": 11}
        },
        "democracy_player": {
            "requirements": {"all": [{"government": "democracy"}]},
            "effects": {"city_gold": 3}
        }
    }));
    let mut rules = (*game.rules).clone();
    rules.modifiers = Rules::from_values(files).unwrap().modifiers;
    game.rules = Arc::new(rules);
    for modifier in ["democracy_cities", "democracy_units", "democracy_player"] {
        game.attach_modifier_to_player(0, modifier).unwrap();
    }

    // Requirements are live facts, not an attach-time snapshot. Before the
    // government exists none of the three bundles contributes.
    assert_eq!(game.player_modifier_effect(0, "city_gold"), 0.0);
    assert_eq!(game.policy_effect(0, "city_production"), 0.0);

    game.players[0].government = Some("democracy".to_string());
    assert_eq!(game.player_modifier_effect(0, "city_gold"), 3.0);
    assert_eq!(game.policy_effect(0, "city_production"), 7.0);
    assert_eq!(game.player_modifier_effect(0, "city_production"), 0.0);

    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    let baseline = {
        game.players[0].government = Some("oligarchy".to_string());
        game.item_prod_mult(0, city, Some(&warrior))
    };
    game.players[0].government = Some("democracy".to_string());
    let before = game.item_prod_mult(0, city, Some(&warrior));
    assert!(
        (before - baseline - 0.11).abs() < 1e-9,
        "unit collection was not applied: {before}"
    );

    game.players[0].government = Some("oligarchy".to_string());
    assert_eq!(game.player_modifier_effect(0, "city_gold"), 0.0);
    assert_eq!(game.policy_effect(0, "city_production"), 0.0);
    let after = game.item_prod_mult(0, city, Some(&warrior));
    assert!(
        (after - baseline).abs() < 1e-9,
        "conditional unit modifier leaked: {after}"
    );
}
