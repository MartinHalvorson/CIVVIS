use super::*;
use crate::game::install_test_district as install_ai_test_district;

/// The pass's ordinary reserve (`AdvancedAi::religious_spending`).
const RESERVE: f64 = 80.0;

/// A founder with one Temple city, one unconverted rival city and a corps
/// already at both caps (two Apostles and three Missionaries for one target,
/// and its Guru), holding `faith`. Returns the board and the Temple city.
fn capped_corps(faith: f64) -> (Game, u32) {
    let mut game = Game::new_full(2, 30, 18, 7_105, 200, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.current = pid;
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    game.current = 0;
    let home = game.player_city_ids(0)[0];
    let target = game.player_city_ids(1)[0];
    install_ai_test_district(&mut game, home, "holy_site");
    game.cities.get_mut(&home).unwrap().buildings =
        vec![crate::name!("shrine"), crate::name!("temple")];
    game.players[0].techs.insert(crate::name!("astrology"));
    game.players[0].civics.insert(crate::name!("theology"));
    game.players[0].religion = Some("Our Faith".to_string());
    game.players[0].faith = faith;
    game.cities
        .get_mut(&home)
        .unwrap()
        .pressure
        .insert("Our Faith".to_string(), 1_000.0);
    game.cities
        .get_mut(&target)
        .unwrap()
        .pressure
        .insert("Rival Faith".to_string(), 1_000.0);
    for kind in [
        "apostle",
        "apostle",
        "guru",
        "missionary",
        "missionary",
        "missionary",
    ] {
        let unit = game.spawn_test_unit(kind, 0, game.cities[&home].pos);
        game.units.get_mut(&unit).unwrap().religion = Some("Our Faith".to_string());
    }
    (game, home)
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_religion_seat_spends_the_whole_bank();
    }
    ai
}

fn count(game: &Game, kind: &str) -> usize {
    game.player_unit_ids(0)
        .into_iter()
        .filter(|unit| game.units[unit].kind == kind)
        .count()
}

/// The two quotes in the Temple city and the spreader the gene would take.
fn quotes(game: &Game, home: u32) -> (f64, f64, &'static str) {
    let missionary = game
        .unit_purchase_cost(0, home, "missionary", "faith")
        .expect("fixture: the city sells a Missionary");
    let apostle = game
        .unit_purchase_cost(0, home, "apostle", "faith")
        .expect("fixture: the city sells an Apostle");
    let best = if AdvancedAi::spread_per_faith(game, "apostle", apostle)
        >= AdvancedAi::spread_per_faith(game, "missionary", missionary)
    {
        "apostle"
    } else {
        "missionary"
    };
    (missionary, apostle, best)
}

/// ★ The 094102Z shape: the corps is at its caps, a rival still owes our
/// faith its majority, and the bank holds less than the two Apostle prices
/// `religion-seat-spends-the-bank` asks for. Off, the pass buys nothing; on,
/// the bank buys the spreader that is the most spread for its price.
#[test]
fn the_bank_the_caps_leave_buys_the_most_spread_for_its_price() {
    let (board, home) = capped_corps(0.0);
    let (missionary, apostle, best) = quotes(&board, home);
    assert!(missionary > 0.0 && apostle > missionary);
    assert_eq!(AdvancedAi::majorities_owed(&board, 0, "Our Faith"), 1);
    // Enough for either spreader and the reserve, short of two Apostles.
    let faith = apostle + RESERVE + 1.0;
    assert!(faith < 2.0 * apostle);
    let price = if best == "apostle" {
        apostle
    } else {
        missionary
    };

    let (mut off, _) = capped_corps(faith);
    let units = off.player_unit_ids(0).len();
    seat(VictoryTarget::Religion, false).religious_spending(&mut off, 0, true);
    assert_eq!(off.player_unit_ids(0).len(), units, "off: the caps hold");
    assert_eq!(off.players[0].faith, faith);

    let (mut on, _) = capped_corps(faith);
    let before = count(&on, best);
    seat(VictoryTarget::Religion, true).religious_spending(&mut on, 0, true);
    assert_eq!(on.player_unit_ids(0).len(), units + 1, "on: one spreader");
    assert_eq!(
        count(&on, best),
        before + 1,
        "the most spread for its price"
    );
    assert!((on.players[0].faith - (faith - price)).abs() < 1e-6);
}

/// One purchase a city a turn, whatever the bank holds, and none in the city
/// the pass has already bought in.
#[test]
fn each_city_sells_one_spreader_a_turn() {
    let (mut rich, home) = capped_corps(50_000.0);
    let units = rich.player_unit_ids(0).len();
    let ai = seat(VictoryTarget::Religion, true);
    assert_eq!(
        ai.religion_seat_whole_bank_buys(&mut rich, 0, true, "Our Faith", RESERVE, None),
        1
    );
    assert_eq!(rich.player_unit_ids(0).len(), units + 1);

    let (mut sold, _) = capped_corps(50_000.0);
    assert_eq!(
        ai.religion_seat_whole_bank_buys(&mut sold, 0, true, "Our Faith", RESERVE, Some(home)),
        0
    );
    assert_eq!(sold.player_unit_ids(0).len(), units);

    // With `religion-seat-spends-the-bank` lifting the caps too, the pass's
    // own Apostle is the city's purchase for the turn.
    let (mut both, _) = capped_corps(50_000.0);
    let mut deep = seat(VictoryTarget::Religion, true);
    deep.enable_religion_seat_spends_the_bank();
    deep.religious_spending(&mut both, 0, true);
    assert_eq!(both.player_unit_ids(0).len(), units + 1);
}

/// Nothing is bought off the offensive, for a seat aiming elsewhere, for
/// another faith, below a spreader's price and the reserve, or once every
/// living rival follows our faith.
#[test]
fn the_bank_is_left_alone_without_a_majority_to_win() {
    let (board, home) = capped_corps(50_000.0);
    let units = board.player_unit_ids(0).len();
    let on = seat(VictoryTarget::Religion, true);
    for (ai, offensive, religion) in [
        (seat(VictoryTarget::Religion, false), true, "Our Faith"),
        (seat(VictoryTarget::Culture, true), true, "Our Faith"),
        (seat(VictoryTarget::Religion, true), false, "Our Faith"),
        (seat(VictoryTarget::Religion, true), true, "Rival Faith"),
    ] {
        let mut game = board.clone();
        assert!(!ai.spends_the_whole_bank(&game, 0, offensive, religion));
        assert_eq!(
            ai.religion_seat_whole_bank_buys(&mut game, 0, offensive, religion, RESERVE, None),
            0
        );
        assert_eq!(game.player_unit_ids(0).len(), units);
    }

    // A bank that covers the cheaper spreader but not the reserve on top.
    let (missionary, _, _) = quotes(&board, home);
    let (mut thin, _) = capped_corps(missionary + RESERVE - 1.0);
    assert_eq!(
        on.religion_seat_whole_bank_buys(&mut thin, 0, true, "Our Faith", RESERVE, None),
        0
    );
    assert_eq!(thin.player_unit_ids(0).len(), units);

    // The rival's one city follows our faith: no majority is owed.
    let mut won = board.clone();
    let target = won.player_city_ids(1)[0];
    {
        let city = won.cities.get_mut(&target).unwrap();
        city.atheist_pressure = 0.0;
        city.pressure.clear();
        city.pressure.insert("Our Faith".to_string(), 1_000.0);
    }
    assert_eq!(AdvancedAi::majorities_owed(&won, 0, "Our Faith"), 0);
    assert!(!on.spends_the_whole_bank(&won, 0, true, "Our Faith"));
    assert_eq!(
        on.religion_seat_whole_bank_buys(&mut won, 0, true, "Our Faith", RESERVE, None),
        0
    );
    assert_eq!(won.player_unit_ids(0).len(), units);
}

/// The measure: charges times the pressure of one spread, over the price.
#[test]
fn spread_for_a_price_is_charges_times_pressure_over_faith() {
    let (game, _) = capped_corps(0.0);
    let spec = game.rules.units.get("missionary").unwrap();
    let whole = f64::from(spec.charges) * spec.religious_spread;
    assert!(whole > 0.0);
    assert_eq!(
        AdvancedAi::spread_per_faith(&game, "missionary", 100.0),
        whole / 100.0
    );
    assert_eq!(
        AdvancedAi::spread_per_faith(&game, "missionary", 200.0),
        whole / 200.0
    );
    assert_eq!(AdvancedAi::spread_per_faith(&game, "missionary", 0.0), 0.0);
    assert_eq!(AdvancedAi::spread_per_faith(&game, "warrior", 100.0), 0.0);
    assert_eq!(
        AdvancedAi::spread_per_faith(&game, "no_such_unit", 100.0),
        0.0
    );
}
