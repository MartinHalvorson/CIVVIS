use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::setup::GameSpeed;

const TAG: &str = "score-seat-settles-what-pays:";

/// A founded capital at population 4 on an Online game of `max_turns`
/// turns, the Score victory on, every unit removed.
fn score_board(max_turns: u32) -> (Game, u32) {
    let mut game = Game::new_full(1, 32, 20, 936_222, max_turns, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the player opens with a settler");
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    game.game_speed = GameSpeed::Online;
    game.victory_conditions.score = true;
    let capital = game.player_city_ids(0)[0];
    game.cities.get_mut(&capital).unwrap().pop = 4;
    (game, capital)
}

/// The Score seat with `no-city-limit` and the gene on.
fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Score);
    ai.enable_no_city_limit();
    ai.enable_score_seat_settles_what_pays();
    ai
}

#[test]
fn ships_off() {
    opt_in_off_in_both_controllers("score-seat-settles-what-pays", |ai| {
        ai.score_seat_settles_what_pays
    });
}

/// The live Rome lane: 170 turns. `no-city-limit`'s Science test finds no
/// seat that pays from about turn 106; in points every city founded before
/// the end pays, until the last fifteen turns.
#[test]
fn a_score_seat_pays_for_a_city_until_its_last_turns() {
    let (mut game, _) = score_board(170);
    let ai = armed();
    let mut off = ai.clone();
    off.disable_score_seat_settles_what_pays();
    for turn in [40, 100, 120, 155] {
        game.turn = turn;
        assert_eq!(ai.no_city_limit_pay_floor(&game, 0), 0.0, "turn {turn}");
        assert!(ai.score_seat_stops(&game, 190.0).is_none(), "turn {turn}");
    }
    for turn in [110, 140] {
        game.turn = turn;
        assert!(
            off.no_city_limit_pay_floor(&game, 0).is_infinite(),
            "the Science test at turn {turn}"
        );
    }
    game.turn = 156;
    assert!(ai.no_city_limit_pay_floor(&game, 0).is_infinite());
    assert!(ai
        .score_seat_stops(&game, 190.0)
        .is_some_and(|why| why.contains("last 15 turns")));
    assert_eq!(ai.score_seat_last_settler_turn(&game), Some(155));
}

/// What a Settler buys: a city at turn 155 stands about ten turns before
/// the end and is worth more than a 190-Production Settler in points; one
/// costing 600 is not.
#[test]
fn the_points_test_weighs_the_city_against_the_settler() {
    let (mut game, _) = score_board(170);
    game.turn = 155;
    let late = AdvancedAi::score_seat_settler(&game, 190.0);
    assert!(
        late.founded > 160.0 && late.founded < 170.0,
        "about ten Online turns: {late:?}"
    );
    assert!(late.worth > late.price, "{late:?}");
    let ai = armed();
    assert!(ai.score_seat_stops(&game, 600.0).is_some());
    game.turn = 60;
    let early = AdvancedAi::score_seat_settler(&game, 190.0);
    assert!(early.worth > late.worth, "an older city is worth more");
}

#[test]
fn holdable_land_stays_open_to_the_last_turns_on_a_score_seat() {
    let (mut game, _) = score_board(170);
    let ai = armed();
    let mut off = ai.clone();
    off.disable_score_seat_settles_what_pays();
    game.turn = 140;
    assert!(ai.holdable_land_window(&game));
    assert!(
        !off.holdable_land_window(&game),
        "100 standard turns of payback"
    );
    game.turn = 156;
    assert!(!ai.holdable_land_window(&game));
}

#[test]
fn another_target_or_no_city_limit_off_changes_nothing() {
    let (mut game, _) = score_board(170);
    game.turn = 120;
    let mut domination = armed();
    domination.victory_target = Some(VictoryTarget::Domination);
    assert_eq!(domination.score_seat_last_settler_turn(&game), None);
    assert!(domination.score_seat_pay_floor(&game, 190.0).is_none());
    let mut stock = armed();
    stock.disable_no_city_limit();
    assert!(!stock.score_seat_settles(&game));
    assert_eq!(stock.no_city_limit_pay_floor(&game, 0), 0.0);
}

#[test]
fn a_trained_settler_writes_its_points() {
    let (mut game, capital) = score_board(170);
    game.turn = 120;
    let mut ai = armed();
    let journal = crate::reasoning::Journal::recording();
    journal.begin_turn(game.turn, 0);
    ai.attach_journal(journal.handle());
    ai.explain_score_seat_settler(&game, 0, capital);
    ai.explain_score_seat_count(&game, 0, 1, 0, 3);
    game.turn = 160;
    ai.explain_score_seat_count(&game, 0, 1, 0, 0);
    let details: Vec<String> = journal
        .since(0)
        .thoughts
        .into_iter()
        .map(|thought| thought.detail)
        .collect();
    assert!(
        details
            .iter()
            .any(|line| line.starts_with(TAG) && line.contains("trains a Settler")),
        "{details:?}"
    );
    assert!(
        details
            .iter()
            .any(|line| line.starts_with(TAG) && line.contains("3 free seat(s) pay")),
        "{details:?}"
    );
    assert!(
        details
            .iter()
            .any(|line| line.starts_with(TAG) && line.contains("no Settler pays")),
        "{details:?}"
    );
}

/// A seat four tiles from a rival city of twelve citizens and eight from our
/// four-citizen capital would revolt before the tally in a Dark Age: the gene
/// refuses it, and the walker's verdict says so. A seat beside our capital
/// with no rival near holds. Off, nothing is refused for this reason.
#[test]
fn a_seat_that_would_revolt_before_the_tally_in_a_dark_age_is_refused() {
    let mut game = Game::new_full(2, 40, 24, 936_223, 170, 0, false);
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
    }
    game.game_speed = GameSpeed::Online;
    game.victory_conditions.score = true;
    game.turn = 100;
    let home = (8, 10);
    let capital = game.found_city_for(0, home, None);
    game.cities.get_mut(&capital).unwrap().pop = 4;
    let frontier = (16, 10);
    let rival_at = (20, 10);
    let rival = game.found_city_for(1, rival_at, None);
    game.cities.get_mut(&rival).unwrap().pop = 12;
    assert_eq!(game.wdist(frontier, rival_at), 4);
    let near_home = (4, 10);

    let ai = armed();
    let why = ai.score_seat_seat_refusal(&game, 0, frontier);
    assert!(
        why.as_deref()
            .is_some_and(|why| why.starts_with(TAG) && why.contains("Dark Age")),
        "{why:?}"
    );
    assert!(ai
        .settle_site_loyalty_verdict(&game, 0, frontier)
        .is_some_and(|why| why.starts_with(TAG)));
    assert!(ai.holdable_land_loyalty_refuses(&game, 0, frontier));
    assert_eq!(ai.score_seat_seat_refusal(&game, 0, near_home), None);

    let mut off = ai.clone();
    off.disable_score_seat_settles_what_pays();
    assert_eq!(off.score_seat_seat_refusal(&game, 0, frontier), None);
}
