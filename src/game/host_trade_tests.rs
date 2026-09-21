use super::*;

fn trade_game(observed: bool) -> Game {
    let mut g = Game::new_full(2, 24, 16, 365_300, 120, 0, false);
    g.record_contact(0, 1);
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.found_city_for(pid, g.units[&settler].pos, None);
        g.players[pid].gold = 500.0;
        g.players[pid].diplomatic_favor = 100.0;
        g.players[pid].civics.insert(crate::name!("early_empire"));
    }
    for (pid, resource) in [(0, "silk"), (1, "wine")] {
        let city = g.player_city_ids(pid)[0];
        let positions = g.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .filter(|pos| g.city_at(*pos).is_none())
            .take(2)
            .collect::<Vec<_>>();
        assert_eq!(positions.len(), 2);
        for pos in positions {
            let tile = g.map.tiles.get_mut(&pos).unwrap();
            tile.resource = Some(Name::new(resource));
            tile.improvement = Some(crate::name!("plantation"));
            tile.pillaged = false;
        }
    }
    if observed {
        let pos = g.cities[&g.player_city_ids(0)[0]].pos;
        Arc::make_mut(&mut g.host_observed).insert(pos);
    }
    g
}

fn resource_sale(g: &Game, pid: usize) -> Action {
    let deal = g
        .quick_deals(pid)
        .into_iter()
        .find(|d| d.direction == "sell" && d.item == if pid == 0 { "silk" } else { "wine" })
        .expect("mutually beneficial surplus luxury quote");
    Action::Trade {
        player: deal.partner,
        offer: Box::new(deal.offer),
        request: Box::new(deal.request),
    }
}

#[test]
fn native_sale_is_logged_without_settling_its_proceeds() {
    let mut g = trade_game(true);
    let action = resource_sale(&g, 0);
    let before = serde_json::to_value(&g.players).unwrap();
    g.apply(0, &action).unwrap();
    assert_eq!(serde_json::to_value(&g.players).unwrap(), before);
    assert!(matches!(g.log.last(), Some((0, Action::Trade { .. }))));
    assert!(g.active_trade_deals.is_empty());
}

#[test]
fn simulator_sale_still_settles() {
    let mut g = trade_game(false);
    let action = resource_sale(&g, 0);
    g.apply(0, &action).unwrap();
    assert!(g.players[0].gold > 500.0);
    assert_eq!(g.active_trade_deals.len(), 1);
    assert_eq!(g.players[0].counters["trades_completed"], 1);
}

#[test]
fn invalid_native_offer_is_not_logged() {
    let mut g = trade_game(true);
    let before = g.log.len();
    let action = Action::Trade {
        player: 1,
        offer: Box::new(DealItems {
            gold: 501.0,
            ..Default::default()
        }),
        request: Box::default(),
    };
    assert!(g.apply(0, &action).is_err());
    assert_eq!(g.log.len(), before);
    assert_eq!(g.players[0].gold, 500.0);
}

#[test]
fn native_sale_cannot_fund_a_purchase_until_observed() {
    let mut g = trade_game(true);
    let city = g.player_city_ids(0)[0];
    g.players[0].gold = 0.0;
    let sale = resource_sale(&g, 0);
    g.apply(0, &sale).unwrap();
    let buy = Action::BuyBuilding {
        city,
        building: crate::name!("monument"),
        currency: "gold".to_string(),
    };
    assert!(g.apply(0, &buy).is_err());
    // A subsequent host snapshot can supply real proceeds normally.
    g.players[0].gold = 500.0;
    g.apply(0, &buy).unwrap();
    assert!(g.cities[&city]
        .buildings
        .contains(&crate::name!("monument")));
}

#[test]
fn other_seat_on_observed_board_retains_simulator_settlement() {
    let mut g = trade_game(true);
    g.current = 1;
    let sale = resource_sale(&g, 1);
    g.apply(1, &sale).unwrap();
    assert!(g.players[1].gold > 500.0);
}

#[test]
fn native_open_borders_remains_unconfirmed() {
    let mut g = trade_game(true);
    let deal = g
        .quick_deals(0)
        .into_iter()
        .find(|d| d.item == "open_borders" && d.direction == "buy")
        .expect("legal passage quote");
    let before = g.players[0].gold;
    g.apply(
        0,
        &Action::Trade {
            player: deal.partner,
            offer: Box::new(deal.offer),
            request: Box::new(deal.request),
        },
    )
    .unwrap();
    assert!(g.active_trade_deals.is_empty());
    assert_eq!(g.players[0].gold, before);
}
