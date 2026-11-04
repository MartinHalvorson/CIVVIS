use crate::game::{Game, HostMaintenance};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Two majors who can trade passage, with Gold in hand but a unit bill that
/// drives `empire_gold_per_turn` to zero — the built-up empire of the census.
fn upkept_traders() -> Game {
    let mut game = Game::new_full(2, 18, 10, 79_004, 200, 0, false);
    game.record_contact(0, 1);
    game.turn = 7;
    let mut bills = BTreeMap::new();
    for pid in 0..2 {
        game.players[pid].gold = 1_000.0;
        game.players[pid]
            .civics
            .insert(crate::name!("early_empire"));
        bills.insert(
            pid,
            HostMaintenance {
                units: Some(200.0),
                buildings: None,
                districts: None,
            },
        );
    }
    game.host_maintenance = Arc::new(bills);
    game
}

fn passage_buy(game: &Game) -> Option<crate::game::QuickDeal> {
    game.quick_deals(0)
        .into_iter()
        .find(|deal| deal.item == "open_borders" && deal.direction == "buy")
}

#[test]
fn a_zero_liquidity_proxy_refuses_the_sliver_and_the_lump_restores_the_quote() {
    let mut game = upkept_traders();
    assert!(
        passage_buy(&game).is_none(),
        "the fractional midpoint's 0.1-a-turn remainder fails the zero proxy"
    );

    game.lump_quote_viewer = Some(0);
    let deal = passage_buy(&game).expect("a lump quote");

    assert_eq!(deal.offer.gold_per_turn, 0.0);
    assert_eq!(deal.offer.gold, deal.offer.gold.ceil());
    assert!(deal.my_value > 0.0 && deal.partner_value > 0.0);
}

#[test]
fn only_the_named_viewer_quotes_in_lumps() {
    let mut game = upkept_traders();
    game.lump_quote_viewer = Some(1);

    assert!(passage_buy(&game).is_none());
}

#[test]
fn a_lump_quote_settles_as_an_ordinary_trade() {
    let mut game = upkept_traders();
    game.lump_quote_viewer = Some(0);
    let deal = passage_buy(&game).expect("a lump quote");
    let gold = game.players[0].gold;
    game.current = 0;

    game.apply(
        0,
        &crate::game::Action::Trade {
            player: deal.partner,
            offer: Box::new(deal.offer.clone()),
            request: Box::new(deal.request.clone()),
        },
    )
    .expect("the authoritative rules accept the lump");

    assert!(game.has_open_borders(0, 1));
    assert_eq!(game.players[0].gold, gold - deal.offer.gold);
}
