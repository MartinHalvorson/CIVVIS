//! `gold-buys-the-idle-trader`.

use super::tests::two_city_trade_empire;
use super::*;

fn traders(game: &Game) -> usize {
    game.units
        .values()
        .filter(|unit| unit.owner == 0 && unit.kind == "trader")
        .count()
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_gold_buys_the_idle_trader();
    ai
}

#[test]
fn an_empty_slot_buys_a_trader_with_the_idle_bank() {
    let Some((mut game, _)) = two_city_trade_empire() else {
        panic!("fixture: two cities and a trade slot");
    };
    game.players[0].gold = 400.0;
    let ai = armed();
    let plan = ai.assess(&game, 0);
    assert!(ai.idle_trader_purchase(&mut game, 0, &plan));
    assert_eq!(traders(&game), 1);
    assert!(game.players[0].gold < 400.0);
    // The slot is answered: no second purchase.
    assert!(!ai.idle_trader_purchase(&mut game, 0, &plan));
    assert_eq!(traders(&game), 1);
}

#[test]
fn off_nothing_is_bought() {
    let Some((mut game, _)) = two_city_trade_empire() else {
        panic!("fixture");
    };
    game.players[0].gold = 400.0;
    let ai = AdvancedAi::new();
    let plan = ai.assess(&game, 0);
    assert!(!ai.idle_trader_purchase(&mut game, 0, &plan));
    assert_eq!(traders(&game), 0);
    assert_eq!(game.players[0].gold, 400.0);
}

#[test]
fn an_empty_bank_buys_nothing() {
    let Some((mut game, _)) = two_city_trade_empire() else {
        panic!("fixture");
    };
    game.players[0].gold = 5.0;
    let ai = armed();
    let plan = ai.assess(&game, 0);
    assert!(!ai.idle_trader_purchase(&mut game, 0, &plan));
    assert_eq!(traders(&game), 0);
}

#[test]
fn a_threatened_city_keeps_the_bank() {
    let Some((mut game, capital)) = two_city_trade_empire() else {
        panic!("fixture");
    };
    game.players[0].gold = 400.0;
    let ai = armed();
    let mut plan = ai.assess(&game, 0);
    plan.threatened_city = Some(capital);
    assert!(!ai.idle_trader_purchase(&mut game, 0, &plan));
    assert_eq!(traders(&game), 0);
}

#[test]
fn a_city_training_a_trader_is_where_it_is_bought() {
    let Some((mut game, _)) = two_city_trade_empire() else {
        panic!("fixture");
    };
    game.players[0].gold = 400.0;
    let cities = game.player_city_ids(0);
    let last = *cities.last().unwrap();
    game.cities.get_mut(&last).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("trader"),
    }];
    let ai = armed();
    let plan = ai.assess(&game, 0);
    assert!(ai.idle_trader_purchase(&mut game, 0, &plan));
    let bought = game
        .units
        .values()
        .find(|unit| unit.owner == 0 && unit.kind == "trader")
        .unwrap();
    assert_eq!(bought.pos, game.cities[&last].pos);
}
