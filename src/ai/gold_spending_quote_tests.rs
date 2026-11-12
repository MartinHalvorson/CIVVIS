//! `gold-unit-buys-read-the-host-quote`: `buy_gold_unit` tests each city's
//! own Gold quote against the reserve instead of the list price `cost * 4`.

use super::*;
use crate::game::HostPurchaseEntry;
use std::sync::Arc;

/// Two majors on grassland, our two cities at (4, 4) and (12, 4), 420 Gold:
/// the live t192 treasury.
fn fixture() -> (Game, u32, u32) {
    let mut g = Game::new_full(2, 24, 18, 59_010_625, 250, 0, false);
    g.units.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let first = g.found_city_for(0, (4, 4), None);
    let second = g.found_city_for(0, (12, 4), None);
    g.found_city_for(1, (14, 12), None);
    g.current = 0;
    g.turn = 89;
    g.players[0].gold = 420.0;
    (g, first, second)
}

/// The host's Gold menu for `city`.
fn menu(g: &mut Game, city: u32, entries: &[(&str, f64)]) {
    Arc::make_mut(&mut g.host_purchasable).insert(
        city,
        entries
            .iter()
            .map(|(name, price)| {
                (
                    format!("unit:{name}"),
                    HostPurchaseEntry {
                        gold: Some(*price),
                        faith: None,
                    },
                )
            })
            .collect(),
    );
}

fn bought(g: &Game) -> Vec<(u32, Name)> {
    g.log
        .iter()
        .filter_map(|(_, action)| match action {
            Action::Buy {
                city,
                unit,
                currency,
                ..
            } if currency == "gold" => Some((*city, *unit)),
            _ => None,
        })
        .collect()
}

fn arm(on: bool) -> BasicAi {
    let mut ai = BasicAi::new();
    ai.gold_unit_buys_read_the_host_quote = on;
    ai
}

/// The live t192 shape: a Builder listed at 200 and quoted at 400, 420 Gold
/// and a 162 reserve. Off, the list price passes and the buy leaves 20 Gold;
/// on, the quote does not fit and nothing is bought.
#[test]
fn a_quote_above_the_treasury_less_the_reserve_is_not_bought() {
    let (mut g, city, _) = fixture();
    assert_eq!(
        g.rules.units.get("builder").map(|spec| spec.cost * 4.0),
        Some(200.0)
    );
    menu(&mut g, city, &[("builder", 400.0)]);

    let mut off = g.clone();
    assert!(arm(false).buy_gold_unit(&mut off, 0, &[city], "builder", 162.0));
    assert_eq!(bought(&off), vec![(city, crate::name!("builder"))]);
    assert_eq!(off.players[0].gold, 20.0, "off: the reserve is not held");

    let mut on = g.clone();
    assert!(!arm(true).buy_gold_unit(&mut on, 0, &[city], "builder", 162.0));
    assert!(bought(&on).is_empty());
    assert_eq!(on.players[0].gold, 420.0);
}

/// A quote inside the treasury less the reserve is bought, and the reserve
/// is still there afterwards.
#[test]
fn a_quote_inside_the_treasury_less_the_reserve_is_bought() {
    let (mut g, city, _) = fixture();
    menu(&mut g, city, &[("builder", 250.0)]);
    for on in [false, true] {
        let mut board = g.clone();
        assert!(
            arm(on).buy_gold_unit(&mut board, 0, &[city], "builder", 162.0),
            "on={on}"
        );
        assert_eq!(bought(&board), vec![(city, crate::name!("builder"))]);
        assert_eq!(board.players[0].gold, 170.0, "on={on}");
    }
}

/// A quote below the list price (a Scout listed at 120, quoted at 60): off,
/// the list price refuses a buy the treasury covers; on, the quote buys it.
#[test]
fn a_quote_below_the_list_price_is_bought_on_the_quote() {
    let (mut g, city, _) = fixture();
    g.players[0].gold = 200.0;
    assert_eq!(
        g.rules.units.get("scout").map(|spec| spec.cost * 4.0),
        Some(120.0)
    );
    menu(&mut g, city, &[("scout", 60.0)]);

    let mut off = g.clone();
    assert!(!arm(false).buy_gold_unit(&mut off, 0, &[city], "scout", 110.0));
    assert!(bought(&off).is_empty());

    let mut on = g.clone();
    assert!(arm(true).buy_gold_unit(&mut on, 0, &[city], "scout", 110.0));
    assert_eq!(bought(&on), vec![(city, crate::name!("scout"))]);
    assert_eq!(on.players[0].gold, 140.0);
}

/// Each city is priced on its own quote: the first city's 400 does not fit,
/// the second city's 250 does, and the Builder is bought there.
#[test]
fn each_city_is_tested_on_its_own_quote() {
    let (mut g, first, second) = fixture();
    menu(&mut g, first, &[("builder", 400.0)]);
    menu(&mut g, second, &[("builder", 250.0)]);
    let mut on = g.clone();
    assert!(arm(true).buy_gold_unit(&mut on, 0, &[first, second], "builder", 162.0));
    assert_eq!(bought(&on), vec![(second, crate::name!("builder"))]);
    assert_eq!(on.players[0].gold, 170.0);
}
