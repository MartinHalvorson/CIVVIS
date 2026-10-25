use super::*;
use crate::game::HostPurchaseEntry;
use std::sync::Arc;

fn fixture() -> (Game, BasicAi, u32) {
    let mut g = Game::new_full(2, 24, 18, 59_010_625, 250, 0, false);
    g.units.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let city = g.found_city_for(0, (4, 4), None);
    g.found_city_for(1, (18, 12), None);
    g.current = 0;
    g.turn = 89;
    g.players[0]
        .techs
        .extend([crate::name!("archery"), crate::name!("bronze_working")]);
    g.players[0].gold = 245.0;
    g.players[0].gold_per_turn = 26.4062;
    menu(
        &mut g,
        city,
        &[("scout", 60.0), ("archer", 120.0), ("spearman", 125.0)],
    );
    (g, BasicAi::new(), city)
}

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

#[test]
fn native_quote_funds_a_real_melee_defender_instead_of_a_scout() {
    let (mut g, ai, city) = fixture();
    assert!(g.can_produce(
        0,
        city,
        &Item::Unit {
            unit: crate::name!("spearman")
        }
    ));
    assert_eq!(
        g.unit_purchase_cost(0, city, "spearman", "gold"),
        Some(125.0)
    );
    assert!(ai.buy_gold_military(&mut g, 0, &[city], 110.0, false));
    assert_eq!(bought(&g), vec![(city, crate::name!("spearman"))]);
    assert_eq!(g.players[0].gold, 120.0);
}

#[test]
fn scout_only_affordability_does_not_count_as_a_defender() {
    let (mut g, ai, city) = fixture();
    menu(&mut g, city, &[("scout", 60.0)]);
    assert!(!ai.buy_gold_military(&mut g, 0, &[city], 110.0, false));
    assert_eq!(g.players[0].gold, 245.0);
    assert!(bought(&g).is_empty());
}

#[test]
fn a_ranged_defender_keeps_its_requested_role_at_the_native_price() {
    let (mut g, ai, city) = fixture();
    assert!(ai.buy_gold_military(&mut g, 0, &[city], 110.0, true));
    assert_eq!(bought(&g), vec![(city, crate::name!("archer"))]);
    assert_eq!(g.players[0].gold, 125.0);
}

#[test]
fn a_more_expensive_native_quote_cannot_spend_the_reserve() {
    let (mut g, ai, city) = fixture();
    g.players[0].gold = 400.0;
    menu(&mut g, city, &[("archer", 350.0)]);
    assert!(!ai.buy_gold_military(&mut g, 0, &[city], 110.0, true));
    assert_eq!(g.players[0].gold, 400.0);
}

#[test]
fn no_affordable_field_unit_leaves_cash_and_units_untouched() {
    let (mut g, ai, city) = fixture();
    g.players[0].gold = 169.0;
    assert!(!ai.buy_gold_military(&mut g, 0, &[city], 110.0, false));
    assert!(bought(&g).is_empty());
    assert!(g.player_unit_ids(0).is_empty());
    assert_eq!(g.players[0].gold, 169.0);
}

#[test]
fn native_refusals_are_preserved() {
    let (mut g, ai, city) = fixture();
    Arc::make_mut(&mut g.blocked_purchases).insert(
        city,
        ["unit:scout", "unit:archer", "unit:spearman"]
            .into_iter()
            .map(String::from)
            .collect(),
    );
    assert!(!ai.buy_gold_military(&mut g, 0, &[city], 110.0, false));
    assert_eq!(g.players[0].gold, 245.0);
    assert!(bought(&g).is_empty());
}

#[test]
fn an_unoffered_ranged_unit_yields_to_a_host_legal_field_unit() {
    let (mut g, ai, city) = fixture();
    g.players[0].gold = 1000.0;
    menu(&mut g, city, &[("spearman", 125.0)]);
    assert!(ai.buy_gold_military(&mut g, 0, &[city], 110.0, true));
    assert_eq!(bought(&g), vec![(city, crate::name!("spearman"))]);
}

#[test]
fn equally_strong_defenders_use_the_cheaper_native_city_quote() {
    let (mut g, ai, city) = fixture();
    let second = g.found_city_for(0, (12, 4), None);
    g.players[0].gold = 1000.0;
    menu(&mut g, city, &[("spearman", 170.0)]);
    menu(&mut g, second, &[("spearman", 125.0)]);
    assert!(ai.buy_gold_military(&mut g, 0, &[city, second], 110.0, false));
    assert_eq!(bought(&g), vec![(second, crate::name!("spearman"))]);
    assert_eq!(g.players[0].gold, 875.0);
}

#[test]
fn ordinary_model_purchases_still_pay_the_engine_quote() {
    let (mut g, ai, city) = fixture();
    Arc::make_mut(&mut g.host_purchasable).clear();
    g.players[0].gold = 1000.0;
    let price = g.unit_purchase_cost(0, city, "spearman", "gold").unwrap();
    assert!(ai.buy_gold_military(&mut g, 0, &[city], 110.0, false));
    assert_eq!(bought(&g), vec![(city, crate::name!("spearman"))]);
    assert_eq!(g.players[0].gold, 1000.0 - price);
}

#[test]
fn a_separate_exploration_purchase_can_still_buy_a_scout() {
    let (mut g, ai, city) = fixture();
    g.players[0].gold = 1000.0;
    assert!(ai.buy_gold_unit(&mut g, 0, &[city], "scout", 110.0));
    assert_eq!(bought(&g), vec![(city, crate::name!("scout"))]);
    assert_eq!(g.players[0].gold, 940.0);
}
