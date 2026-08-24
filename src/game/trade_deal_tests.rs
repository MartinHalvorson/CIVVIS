use super::*;
use crate::ai::BasicAi;

fn trade_game() -> Game {
    let mut game = Game::new_full(2, 24, 16, 7711, 120, 0, false);
    // Two capitals dropped on opposite ends of a map have not met, and
    // there is nothing to trade until they have.
    game.record_contact(0, 1);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.players[pid].gold = 500.0;
        game.players[pid].diplomatic_favor = 100.0;
        game.players[pid].civics.insert(crate::name!("early_empire"));
        for city in game.player_city_ids(pid) {
            for position in game.cities[&city].owned_tiles.clone() {
                let tile = game.map.tiles.get_mut(&position).unwrap();
                tile.resource = None;
                tile.improvement = None;
                tile.pillaged = false;
            }
        }
    }
    for (pid, resource) in [(0, "silk"), (1, "wine")] {
        let positions: Vec<Pos> = game
            .player_city_ids(pid)
            .into_iter()
            .flat_map(|city| game.cities[&city].owned_tiles.clone())
            .filter(|position| game.city_at(*position).is_none())
            .take(2)
            .collect();
        assert_eq!(positions.len(), 2);
        for position in positions {
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.resource = Some(Name::new(resource));
            tile.improvement = Some(crate::name!("plantation"));
        }
    }
    game
}

#[test]
fn quick_deals_compare_partners_and_every_quote_benefits_both_sides() {
    let game = trade_game();
    let deals = game.quick_deals(0);
    assert!(!deals.is_empty());
    assert!(deals
        .iter()
        .any(|deal| deal.direction == "sell" && deal.item == "silk"));
    assert!(deals
        .iter()
        .any(|deal| deal.direction == "buy" && deal.item == "wine"));
    assert!(deals
        .iter()
        .all(|deal| deal.my_value > 0.25 && deal.partner_value > 0.25));
    assert!(deals
        .windows(2)
        .all(|pair| pair[0].my_value >= pair[1].my_value));
}

#[test]
fn great_work_trades_are_permanent_and_require_a_compatible_empty_slot() {
    let mut game = trade_game();
    for pid in 0..2 {
        let city = game.player_city_ids(pid)[0];
        install_test_district(&mut game, city, "theater_square");
        game.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(crate::name!("amphitheater"));
    }
    game.players[1]
        .counters
        .insert("great_work:writing".to_string(), 2);

    let deal = game
        .quick_deals(0)
        .into_iter()
        .find(|deal| {
            deal.category == "great_work" && deal.direction == "buy" && deal.item == "writing"
        })
        .unwrap();
    assert_eq!(deal.request.great_works["writing"], 1);
    game.do_trade(0, 1, &deal.offer, &deal.request).unwrap();
    assert_eq!(game.great_work_inventory(0, "writing"), 1);
    assert_eq!(game.great_work_inventory(1, "writing"), 1);
    assert_eq!(game.housed_great_work_count(0, "writing"), 1);
    assert!(game.active_trade_deals.iter().all(|contract| contract
        .offer
        .great_works
        .is_empty()
        && contract.request.great_works.is_empty()));
    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(restored.great_work_inventory(0, "writing"), 1);
    assert_eq!(restored.great_work_inventory(1, "writing"), 1);

    let mut blocked = trade_game();
    let seller = blocked.player_city_ids(1)[0];
    install_test_district(&mut blocked, seller, "theater_square");
    blocked
        .cities
        .get_mut(&seller)
        .unwrap()
        .buildings
        .push(crate::name!("amphitheater"));
    blocked.players[1]
        .counters
        .insert("great_work:writing".to_string(), 2);
    blocked.players[0]
        .counters
        .insert("great_work:relic".to_string(), 1);
    assert!(!blocked.quick_deals(0).into_iter().any(|deal| {
        deal.category == "great_work" && deal.direction == "buy" && deal.item == "writing"
    }));
}

#[test]
fn resource_contract_changes_access_and_expires_after_thirty_turns() {
    let mut game = trade_game();
    let deal = game
        .quick_deals(0)
        .into_iter()
        .find(|deal| deal.direction == "sell" && deal.item == "silk")
        .unwrap();
    game.do_trade(0, 1, &deal.offer, &deal.request).unwrap();
    assert_eq!(game.resource_access_count(0, "silk"), 1);
    assert_eq!(game.resource_access_count(1, "silk"), 1);
    assert!(game.empire_luxury_names(1).contains(&Name::new("silk")));
    assert_eq!(game.active_trade_deals.len(), 1);

    let ends = game.active_trade_deals[0].ends;
    game.turn = ends;
    game.process_trade_deals(0);
    assert_eq!(game.resource_access_count(0, "silk"), 2);
    assert_eq!(game.resource_access_count(1, "silk"), 0);
    assert!(game.active_trade_deals.is_empty());
}

#[test]
fn active_trade_contract_survives_a_save_round_trip() {
    let mut game = trade_game();
    let deal = game
        .quick_deals(0)
        .into_iter()
        .find(|deal| deal.direction == "sell" && deal.item == "silk")
        .unwrap();
    game.do_trade(0, 1, &deal.offer, &deal.request).unwrap();

    let encoded = serde_json::to_value(&game).unwrap();
    let restored: Game = serde_json::from_value(encoded).unwrap();
    assert_eq!(restored.active_trade_deals, game.active_trade_deals);
    assert_eq!(restored.resource_access_count(0, "silk"), 1);
    assert_eq!(restored.resource_access_count(1, "silk"), 1);
}

#[test]
fn gpt_pays_each_turn_and_war_cancels_the_remaining_contract() {
    let mut game = trade_game();
    let mut silk = DealItems::default();
    silk.resources.insert("silk".to_string(), 1);
    let payment = DealItems {
        gold_per_turn: 1.0,
        ..DealItems::default()
    };
    let before = game.players[0].gold;
    game.do_trade(0, 1, &silk, &payment).unwrap();
    assert_eq!(game.players[0].gold, before + 1.0);
    game.turn += 1;
    game.process_trade_deals(1);
    assert_eq!(game.players[0].gold, before + 2.0);

    game.do_declare_war(0, 1).unwrap();
    assert!(game.active_trade_deals.is_empty());
    assert_eq!(game.resource_access_count(0, "silk"), 2);
    assert_eq!(game.resource_access_count(1, "silk"), 0);
}

#[test]
fn war_eve_quotes_offer_only_what_the_declaration_hands_back() {
    let game = trade_game();
    let deals = game.war_eve_deals(0, 1);
    assert!(!deals.is_empty(), "a spare luxury copy is a cancellable promise");
    for deal in &deals {
        assert_eq!(deal.partner, 1);
        assert_eq!(deal.direction, "sell");
        // Everything offered is something the declaration takes back.
        assert_eq!(deal.offer.gold, 0.0);
        assert_eq!(deal.offer.diplomatic_favor, 0.0);
        assert!(deal.offer.great_works.is_empty());
        assert!(deal.offer.captured_spies.is_empty());
        assert!(deal.offer.cities.is_empty());
        for resource in deal.offer.resources.keys() {
            assert_eq!(
                game.rules.resources[resource].class, "luxury",
                "a strategic stockpile transfers on signature and no war returns it"
            );
        }
        // Everything asked for has already settled when the war lands.
        assert!(deal.request.gold > 0.0);
        assert_eq!(deal.request.gold_per_turn, 0.0);
        assert!(deal.request.resources.is_empty());
        assert!(!deal.request.open_borders);
        assert!(deal.request.great_works.is_empty());
        assert!(deal.request.cities.is_empty());
        // And it is still an honest contract at the moment it is signed.
        assert!(deal.my_value > 0.0 && deal.partner_value > 0.0);
        assert!(Game::war_eve_net_gold(deal) > 0.0);
    }
    assert!(deals
        .windows(2)
        .all(|pair| Game::war_eve_net_gold(&pair[0]) >= Game::war_eve_net_gold(&pair[1])));
    assert!(
        game.war_eve_deals(1, 1).is_empty()
            && game.war_eve_deals(0, 9).is_empty(),
        "there is no war-eve market with yourself or with a player who does not exist"
    );
}

#[test]
fn a_war_eve_sale_banks_the_gold_and_the_declaration_returns_the_luxury() {
    let mut game = trade_game();
    let deal = game
        .war_eve_deals(0, 1)
        .into_iter()
        .find(|deal| deal.item == "silk")
        .expect("the spare silk copy is sellable");
    let net = Game::war_eve_net_gold(&deal);
    let before = game.players[0].gold;
    assert_eq!(game.resource_access_count(0, "silk"), 2);

    game.do_trade(0, 1, &deal.offer, &deal.request).unwrap();
    assert!((game.players[0].gold - before - net).abs() < 1e-9);
    assert_eq!(game.resource_access_count(0, "silk"), 1);
    assert_eq!(game.resource_access_count(1, "silk"), 1);

    game.do_declare_war(0, 1).unwrap();
    // The Gold raised is the Gold kept: only the instalment settled at
    // signing ever left, and the war stopped the twenty-nine after it.
    assert!(net > 0.0);
    assert!((game.players[0].gold - before - net).abs() < 1e-9);
    assert!(game.active_trade_deals.is_empty());
    assert_eq!(game.resource_access_count(0, "silk"), 2);
    assert_eq!(game.resource_access_count(1, "silk"), 0);
}

#[test]
fn the_war_eve_price_beats_the_market_and_takes_all_of_it_in_lump_gold() {
    let game = trade_game();
    let ordinary = game
        .quick_deals(0)
        .into_iter()
        .find(|deal| deal.direction == "sell" && deal.item == "silk")
        .unwrap();
    let war_eve = game
        .war_eve_deals(0, 1)
        .into_iter()
        .find(|deal| deal.item == "silk")
        .unwrap();
    assert!(
        ordinary.request.gold_per_turn > 0.0,
        "the ordinary quote settles part of its price in instalments a war would cancel"
    );
    assert_eq!(war_eve.request.gold_per_turn, 0.0);
    assert!(war_eve.request.gold > ordinary.request.gold);
    // The rival is not being cheated at the higher price; it is being read.
    // It still profits by its own valuation, with almost nothing to spare.
    assert!(war_eve.partner_value > 0.0);
    assert!(war_eve.partner_value < ordinary.partner_value);
}

#[test]
fn war_eve_riders_never_promise_the_same_income_twice() {
    let mut game = trade_game();
    let mut promised = 0.0;
    for _ in 0..4 {
        let Some(deal) = game.war_eve_deals(0, 1).into_iter().next() else {
            break;
        };
        promised += deal.offer.gold_per_turn;
        game.do_trade(0, 1, &deal.offer, &deal.request).unwrap();
    }
    assert!(
        promised > 0.0,
        "a rider is what reaches a treasury larger than the asset is worth"
    );
    assert!((game.committed_gold_per_turn(0) - promised).abs() < 1e-9);
    assert!(
        promised <= game.empire_gold_per_turn(0) + 1e-9,
        "{promised} Gold per turn promised against an income of {}",
        game.empire_gold_per_turn(0)
    );
}

#[test]
fn the_ai_sells_the_cancellable_promises_only_into_a_real_declaration() {
    let mut game = trade_game();
    let ai = BasicAi::new();
    let before = game.players[0].gold;
    assert_eq!(
        ai.war_eve_liquidation(&mut game, 0, &Action::Denounce { player: 1 }),
        0.0,
        "a denouncement is not a declaration and cancels nothing"
    );
    assert_eq!(game.players[0].gold, before);
    assert!(game.active_trade_deals.is_empty());

    let raised = ai.war_eve_liquidation(&mut game, 0, &Action::DeclareWar { player: 1 });
    assert!(raised > 0.0);
    assert!((game.players[0].gold - before - raised).abs() < 1e-9);
    assert!(
        game.active_trade_deals.len() > 1,
        "the pass re-quotes after every contract and keeps selling while the \
         rival can still pay, rather than taking one quote and stopping"
    );

    game.do_declare_war(0, 1).unwrap();
    assert!(game.active_trade_deals.is_empty());
    assert!((game.players[0].gold - before - raised).abs() < 1e-9);
    assert_eq!(game.resource_access_count(0, "silk"), 2);
}

#[test]
fn war_cancels_foreign_routes_but_recalls_both_traders() {
    let mut game = trade_game();
    for player in 0..2 {
        game.players[player]
            .civics
            .insert(crate::name!("foreign_trade"));
    }
    let first_city = game.player_city_ids(0)[0];
    let second_city = game.player_city_ids(1)[0];
    assert!(game.wdist(game.cities[&first_city].pos, game.cities[&second_city].pos) <= 15);
    let first_trader = game.spawn_unit("trader", 0, game.cities[&first_city].pos);
    let second_trader = game.spawn_unit("trader", 1, game.cities[&second_city].pos);
    game.do_trade_route(0, first_trader, second_city).unwrap();
    game.do_trade_route(1, second_trader, first_city).unwrap();
    assert_eq!(game.routes.len(), 2);
    assert!(!game.units.contains_key(&first_trader));
    assert!(!game.units.contains_key(&second_trader));

    game.do_declare_war(0, 1).unwrap();

    assert!(game.routes.is_empty());
    for owner in 0..2 {
        assert_eq!(
            game.units
                .values()
                .filter(|unit| unit.owner == owner && unit.kind == "trader")
                .count(),
            1,
            "war must recall player {owner}'s Trader rather than destroy it"
        );
    }
}

#[test]
fn gathering_storm_merchant_republic_uses_governors_and_district_production() {
    let mut game = trade_game();
    let city = game.player_city_ids(0)[0];
    let site = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    let campus = Item::District {
        district: crate::name!("campus"),
        pos: site,
    };
    let baseline_gold = game.city_yields(city).gold;
    let baseline_district_multiplier = game.item_prod_mult(0, city, Some(&campus));
    let baseline_capacity = game.trade_capacity(0);

    game.players[0].government = Some("merchant_republic".to_string());
    assert_eq!(game.trade_capacity(0), baseline_capacity);
    assert!((game.city_yields(city).gold - baseline_gold).abs() < 1e-9);
    assert!(
        (game.item_prod_mult(0, city, Some(&campus)) - baseline_district_multiplier - 0.15)
            .abs()
            < 1e-9
    );

    game.players[0]
        .civics
        .insert(crate::name!("state_workforce"));
    game.do_appoint_governor(0, "pingala", city).unwrap();
    assert!(
        (game.city_yields(city).gold - baseline_gold).abs() < 1e-9,
        "the Gold bonus waits for the Governor to establish"
    );
    game.turn += game.rules.governors["pingala"].establish_turns;
    assert!(
        (game.city_yields(city).gold - baseline_gold * 1.1).abs() < 1e-9,
        "an established Governor activates Merchant Republic's 10% Gold"
    );
}

/// Civilization VI's rule, read off the game's own database on 2026-08-24:
/// a one-sided deal that gives is a gift and buys nothing (no diplomatic
/// modifier exists for one), and a one-sided deal that takes is a demand,
/// never a trade. The engine used to refuse the gift outright.
#[test]
fn a_gift_is_legal_buys_nothing_and_a_demand_is_refused() {
    let mut game = trade_game();
    let gift = DealItems {
        gold: 25.0,
        ..DealItems::default()
    };
    let opinion_before = game.relationship_opinion(1, 0);
    let gold_before = (game.players[0].gold, game.players[1].gold);
    game.do_trade(0, 1, &gift, &DealItems::default())
        .expect("a gift is a legal one-sided deal");
    assert_eq!(game.players[0].gold, gold_before.0 - 25.0);
    assert_eq!(game.players[1].gold, gold_before.1 + 25.0);
    assert_eq!(game.players[0].counters.get("gifts_given"), Some(&1));
    assert_eq!(game.players[1].counters.get("gifts_received"), Some(&1));
    assert_eq!(
        game.relationship_opinion(1, 0),
        opinion_before,
        "a gift buys no opinion, as in Civilization VI"
    );
    assert_eq!(
        game.do_trade(0, 1, &DealItems::default(), &gift),
        Err("invalid trade terms".to_string()),
        "a one-sided deal that only takes is a demand, not a trade"
    );

    // The same rule through the diplomatic lane: a Gold-only proposal is a
    // gift the recipient may accept, and it lands on the same ledger.
    let before = game.players[1].gold;
    game.apply(
        0,
        &Action::ProposeDeal {
            player: 1,
            give_gold: 40.0,
            request_gold: 0.0,
            open_borders: false,
            friendship: false,
            peace: false,
            alliance: None,
        },
    )
    .expect("a Gold-only proposal is a gift");
    let deal = game.pending_deals.last().map(|deal| deal.id).unwrap();
    game.current = 1;
    game.apply(1, &Action::AcceptDeal { deal }).unwrap();
    assert_eq!(game.players[1].gold, before + 40.0);
    assert_eq!(game.players[0].counters.get("gifts_given"), Some(&2));
    assert_eq!(
        game.apply(
            0,
            &Action::ProposeDeal {
                player: 1,
                give_gold: 0.0,
                request_gold: 40.0,
                open_borders: false,
                friendship: false,
                peace: false,
                alliance: None,
            },
        ),
        Err("economic exchanges must use mutually favorable trade terms".to_string()),
        "a Gold-only ask is a demand, and a demand is `DemandGold`"
    );
}

#[test]
fn one_way_open_borders_are_directional() {
    let mut game = trade_game();
    let borders = game
        .quick_deals(0)
        .into_iter()
        .find(|deal| deal.direction == "sell" && deal.item == "open_borders")
        .unwrap();
    game.do_trade(0, 1, &borders.offer, &borders.request)
        .unwrap();
    assert!(game.has_open_borders(1, 0));
    assert!(!game.has_open_borders(0, 1));
}

#[test]
fn suzerainty_and_gunboat_diplomacy_open_city_state_borders() {
    let mut game = trade_game();
    let minor = game.players.len();
    let mut city_state = Player::new(minor, "Geneva", true);
    city_state.civics.insert(crate::name!("early_empire"));
    game.players.push(city_state);
    assert!(!game.has_open_borders(0, minor));

    game.players[0].envoys.push((minor, 3));
    assert_eq!(game.suzerain_of(minor), Some(0));
    assert!(game.has_open_borders(0, minor));

    game.players[0].envoys.clear();
    game.players[0]
        .policies
        .insert(crate::name!("gunboat_diplomacy"));
    assert!(game.has_open_borders(0, minor));
    assert!(!game.has_open_borders(1, minor));

    game.players[0].policies.clear();
    game.players[0].civ = "Portugal".to_string();
    assert!(
        game.has_open_borders(0, minor),
        "João III has Open Borders with every city-state"
    );
}

#[test]
fn bilateral_open_borders_wait_for_both_early_empire_civics() {
    let mut game = trade_game();
    game.players[1].civics.remove(&Name::new("early_empire"));
    assert_eq!(
        game.do_propose_deal(0, 1, 0.0, 0.0, true, false, false, None),
        Err("invalid diplomatic deal".to_string())
    );
    game.players[1].civics.insert(crate::name!("early_empire"));
    assert!(game
        .do_propose_deal(0, 1, 0.0, 0.0, true, false, false, None)
        .is_ok());
}

#[test]
fn trade_action_round_trips_all_supported_terms() {
    let mut resources = BTreeMap::new();
    resources.insert("iron".to_string(), 1);
    let action = Action::Trade {
        player: 1,
        offer: Box::new(DealItems {
            gold: 12.0,
            gold_per_turn: 2.0,
            diplomatic_favor: 5.0,
            resources,
            great_works: BTreeMap::from([("writing".to_string(), 1)]),
            captured_spies: vec![77],
            cities: vec![42],
            open_borders: true,
        }),
        request: Box::new(DealItems::default()),
    };
    let encoded = serde_json::to_value(&action).unwrap();
    assert_eq!(serde_json::from_value::<Action>(encoded).unwrap(), action);
}

#[test]
fn ai_chooses_a_mutual_quick_deal_instead_of_requesting_a_gift() {
    let mut game = trade_game();
    game.turn = 6;
    BasicAi::new().bilateral_trade(&mut game, 0);
    let (
        _,
        Action::Trade {
            player,
            offer,
            request,
        },
    ) = game.log.last().unwrap()
    else {
        panic!("AI did not execute a trade")
    };
    let values = game.trade_utilities(0, *player, offer, request);
    // The now-active resource lease changes marginal resource values, so
    // replay valuation need not equal the original quote; the completed
    // action still has non-empty consideration on both sides.
    assert!(!offer.is_empty());
    assert!(!request.is_empty());
    assert!(values.0.is_finite() && values.1.is_finite());
    assert_eq!(game.players[0].counters["trades_completed"], 1);
    assert_eq!(game.players[*player].counters["trades_completed"], 1);
}
