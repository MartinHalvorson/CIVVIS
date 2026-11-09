use super::*;

#[test]
fn handicap_exempt_seats_receive_no_fictional_starting_units_or_era_boosts() {
    let mut options = GameOptions::new(4, 32, 22, 91_139, 250, 0);
    options.difficulty = "deity".to_string();
    options.handicap_exempt.insert(0);
    options.human_seats.insert(1);
    let game = Game::new_with(options);
    let odds = table(&game, flat);
    assert_eq!(odds[&0].handicap_elo, 0.0);
    assert_eq!(odds[&1].handicap_elo, 0.0);
    assert_eq!(odds[&0].start, odds[&1].start);
    assert!(odds[&2].handicap_elo > 0.0);
}

#[test]
fn an_exempt_human_does_not_receive_camp_gold_credit() {
    let mut options = GameOptions::new(2, 32, 22, 91_140, 250, 0);
    options.difficulty = "settler".to_string();
    options.human_seats.insert(0);
    options.handicap_exempt.insert(0);
    let game = Game::new_with(options);
    assert_eq!(table(&game, flat)[&0].handicap_elo, 0.0);
}

#[test]
fn a_scheduled_late_handicap_never_rewrites_the_start_forecast() {
    let mut options = GameOptions::new(4, 32, 22, 91_141, 250, 0);
    options.difficulty = "deity".to_string();
    options.human_seats.insert(0);
    options.ai_late_science_pct = Some(50);
    options.ai_yield_rung = Some("king".to_string());
    let mut game = Game::new_with(options);
    let before = table(&game, flat);
    let initial_science = game.handicap_yield_pct(1).science;
    game.turn = 1000;
    assert!(game.handicap_yield_pct(1).science < initial_science);
    let after = table(&game, flat);
    for pid in 0..4 {
        assert_eq!(before[&pid].start, after[&pid].start);
        assert_eq!(before[&pid].handicap_elo, after[&pid].handicap_elo);
    }
}
