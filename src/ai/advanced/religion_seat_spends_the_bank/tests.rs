use super::*;
use crate::game::install_test_district as install_ai_test_district;

/// A founder with a Temple city, one unconverted rival city and a corps
/// already at both caps (the shipped two Apostles and three Missionaries for
/// one target), holding `faith`.
fn capped_corps(faith: f64) -> Game {
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
    game
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_religion_seat_spends_the_bank();
    }
    ai
}

fn count(game: &Game, kind: &str) -> usize {
    game.player_unit_ids(0)
        .into_iter()
        .filter(|unit| game.units[unit].kind == kind)
        .count()
}

/// ★ The 012017Z shape: the corps is at its caps and the bank is many
/// Apostles deep. Off, the pass buys nothing; on, it buys an Apostle.
#[test]
fn a_deep_bank_buys_an_apostle_past_the_corps_caps() {
    let off = seat(VictoryTarget::Religion, false);
    let mut game = capped_corps(10_000.0);
    assert!(!off.religion_seat_bank_is_deep(&game, 0, true, "Our Faith"));
    let (units, faith) = (game.player_unit_ids(0).len(), game.players[0].faith);
    seat(VictoryTarget::Religion, false).religious_spending(&mut game, 0, true);
    assert_eq!(game.player_unit_ids(0).len(), units, "off: the caps hold");
    assert_eq!(game.players[0].faith, faith);

    let mut on = seat(VictoryTarget::Religion, true);
    let mut game = capped_corps(10_000.0);
    assert!(on.religion_seat_bank_is_deep(&game, 0, true, "Our Faith"));
    let apostles = count(&game, "apostle");
    on.religious_spending(&mut game, 0, true);
    assert_eq!(
        count(&game, "apostle"),
        apostles + 1,
        "on: one more Apostle"
    );
    assert!(game.players[0].faith < 10_000.0);
}

/// The caps hold below two Apostle prices, on the defensive, and for a seat
/// aiming elsewhere.
#[test]
fn the_caps_hold_without_a_deep_bank_or_a_religion_target() {
    let on = seat(VictoryTarget::Religion, true);
    let rich = capped_corps(10_000.0);
    let price = on
        .religion_seat_apostle_price(&rich, 0, "Our Faith")
        .expect("fixture: the Temple city can field an Apostle");
    assert!(price > 0.0);

    let mut thin = capped_corps(DEEP_BANK_APOSTLES * price - 1.0);
    assert!(!on.religion_seat_bank_is_deep(&thin, 0, true, "Our Faith"));
    let units = thin.player_unit_ids(0).len();
    seat(VictoryTarget::Religion, true).religious_spending(&mut thin, 0, true);
    assert_eq!(thin.player_unit_ids(0).len(), units);

    let exact = capped_corps(DEEP_BANK_APOSTLES * price);
    assert!(on.religion_seat_bank_is_deep(&exact, 0, true, "Our Faith"));

    assert!(!on.religion_seat_bank_is_deep(&rich, 0, false, "Our Faith"));
    assert!(!on.religion_seat_bank_is_deep(&rich, 0, true, "Rival Faith"));
    assert!(
        !seat(VictoryTarget::Culture, true).religion_seat_bank_is_deep(&rich, 0, true, "Our Faith")
    );
    let mut elsewhere = capped_corps(10_000.0);
    let units = elsewhere.player_unit_ids(0).len();
    seat(VictoryTarget::Culture, true).religious_spending(&mut elsewhere, 0, true);
    assert_eq!(elsewhere.player_unit_ids(0).len(), units);
}
