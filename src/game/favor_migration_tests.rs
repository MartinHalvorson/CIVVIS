use super::*;

/// Seat 0 has earned 120 Favor over the game and spent all of it; seat 1 has
/// earned 40 and holds 15. `counters["diplomatic_favor"]` is the lifetime
/// tally the engine adds every grant to.
fn game_with_favor() -> Game {
    let mut game = Game::new_full(2, 24, 16, 5_763, 300, 0, false);
    game.players[0]
        .counters
        .insert("diplomatic_favor".into(), 120);
    game.players[0].diplomatic_favor = 0.0;
    game.players[1]
        .counters
        .insert("diplomatic_favor".into(), 40);
    game.players[1].diplomatic_favor = 15.0;
    game
}

#[test]
fn a_spent_favor_balance_stays_spent_through_a_save() {
    let game = game_with_favor();
    let back: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(back.players[0].diplomatic_favor, 0.0);
    assert_eq!(back.players[1].diplomatic_favor, 15.0);
}

#[test]
fn a_save_from_before_favor_had_its_own_field_still_migrates() {
    let game = game_with_favor();
    let mut save = serde_json::to_value(&game).unwrap();
    for player in save["players"].as_array_mut().unwrap() {
        player.as_object_mut().unwrap().remove("diplomatic_favor");
    }
    let back: Game = serde_json::from_value(save).unwrap();
    assert_eq!(back.players[0].diplomatic_favor, 120.0);
    assert_eq!(back.players[1].diplomatic_favor, 40.0);
    assert!(back
        .players
        .iter()
        .all(|player| !player.diplomatic_favor.is_nan()));
}
