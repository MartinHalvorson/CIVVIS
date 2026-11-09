use super::*;

#[test]
fn unlimited_worlds_age_on_their_own_game_speed() {
    let mut game = even_table(2, "prince", &[]);
    for pid in 0..2 {
        found_capital(&mut game, pid);
    }
    game.players[1].era_score += 100;
    game.max_turns = 0;
    game.speed = "standard".to_string();
    game.turn = 250;
    let standard = table(&game, flat);
    assert_eq!(clock_progress(&game), 0.5);
    for (speed, turn) in [
        ("online", 125),
        ("quick", 165),
        ("epic", 375),
        ("marathon", 750),
    ] {
        game.speed = speed.to_string();
        game.turn = turn;
        assert_eq!(clock_progress(&game), 0.5, "{speed}");
        let scaled = table(&game, flat);
        assert_eq!(scaled[&1].now, standard[&1].now, "{speed}");
    }
}

#[test]
fn explicit_caps_and_the_unlimited_sentinel_keep_their_actual_clock() {
    let mut game = even_table(2, "prince", &[]);
    game.speed = "online".to_string();
    game.max_turns = 500;
    game.turn = 125;
    assert_eq!(clock_progress(&game), 0.25);
    game.turn = 1000;
    assert_eq!(clock_progress(&game), 1.0);
    game.max_turns = 100_000;
    game.turn = 125;
    assert_eq!(clock_progress(&game), 0.5);
}
