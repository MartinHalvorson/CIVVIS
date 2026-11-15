use super::*;

/// Four majors on a fresh board at turn 20, no religion founded, our capital
/// standing, at the given difficulty.
fn board(difficulty: &str) -> Game {
    let mut game = Game::new_full(4, 44, 26, 936_431, 250, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("fixture: a settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("fixture: the capital founds");
    game.turn = 20;
    game.difficulty = difficulty.to_string();
    game
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_immortal_skips_the_prophet_race();
    }
    ai
}

/// At Immortal and Deity a Culture seat with the gene reads the race closed;
/// off, below Immortal, or aiming at Religion, the race reads as before.
#[test]
fn a_high_tier_culture_seat_sits_the_race_out() {
    for difficulty in ["immortal", "deity"] {
        let game = board(difficulty);
        assert!(
            seat(VictoryTarget::Culture, false).prophet_race_open_for(&game, 0),
            "fixture: the race is open at {difficulty}"
        );
        assert!(
            !seat(VictoryTarget::Culture, true).prophet_race_open_for(&game, 0),
            "{difficulty}: the Culture seat sits it out"
        );
        assert!(
            seat(VictoryTarget::Religion, true).prophet_race_open_for(&game, 0),
            "{difficulty}: a Religion seat still races"
        );
    }
    let king = board("king");
    assert!(
        seat(VictoryTarget::Culture, true).prophet_race_open_for(&king, 0),
        "below Immortal nothing changes"
    );
}

/// A Prophet already pending still founds, whatever the difficulty.
#[test]
fn a_pending_prophet_still_founds() {
    let mut game = board("immortal");
    game.players[0].prophet_pending = true;
    assert!(seat(VictoryTarget::Culture, true).prophet_race_open_for(&game, 0));
}

/// At Immortal a Diplomatic seat keeps the race: the Mahabodhi Temple and the
/// founder's Inquisition both need a religion of our own. At Deity the last
/// Prophet is gone by t46 (73 of 73 live games), so it sits the race out
/// there like every seat but a Religion one.
#[test]
fn a_diplomatic_seat_keeps_the_race_at_immortal_and_sits_it_out_at_deity() {
    let immortal = board("immortal");
    assert!(seat(VictoryTarget::Diplomacy, true).prophet_race_open_for(&immortal, 0));
    let deity = board("deity");
    assert!(
        seat(VictoryTarget::Diplomacy, false).prophet_race_open_for(&deity, 0),
        "fixture: the race is open at Deity without the gene"
    );
    assert!(!seat(VictoryTarget::Diplomacy, true).prophet_race_open_for(&deity, 0));
    assert!(
        seat(VictoryTarget::Religion, true).prophet_race_open_for(&deity, 0),
        "a Religion seat still races at Deity"
    );
    let mut pending = board("deity");
    pending.players[0].prophet_pending = true;
    assert!(
        seat(VictoryTarget::Diplomacy, true).prophet_race_open_for(&pending, 0),
        "a Prophet already pending still founds"
    );
}
