use super::*;
use crate::ai::VictoryTarget;
use crate::trade_decider::board::{BoardItem, DealBoard};
use crate::trade_decider::fixtures::{board, CANADA_T32, OTTOMAN_T32};

/// Seat p1 (holy, Ottoman) and seat p3 (moonshot, Canada) of the owner's
/// game `hotseat-20261128T045523Z` at turn 32, from their own boards.
fn ottoman() -> Position {
    Position::from_board(&board(OTTOMAN_T32), Some(VictoryTarget::Religion))
}

fn canada() -> Position {
    Position::from_board(&board(CANADA_T32), Some(VictoryTarget::Science))
}

/// Seat `player` as `seat_json`'s board sees it.
fn seen(seat_json: &str, player: usize) -> StateRival {
    board(seat_json)
        .rivals
        .into_iter()
        .find(|rival| rival.player == player)
        .expect("met")
}

fn horses(amount: i64) -> BoardItem {
    BoardItem {
        kind: "RESOURCE".to_string(),
        name: Some("RESOURCE_HORSES".to_string()),
        amount,
        class: Some("strategic".to_string()),
        ..BoardItem::default()
    }
}

fn luxury(name: &str) -> BoardItem {
    BoardItem {
        kind: "RESOURCE".to_string(),
        name: Some(name.to_string()),
        amount: 1,
        duration: 30,
        ..BoardItem::default()
    }
}

fn offer(from: usize, give: Vec<BoardItem>, get: Vec<BoardItem>) -> IncomingDeal {
    IncomingDeal {
        from,
        from_kind: "civvis".to_string(),
        id: 4242,
        pending_turns: 0,
        terms: Terms { give, get },
        counter_of: None,
        round: None,
    }
}

fn none() -> BTreeMap<String, String> {
    BTreeMap::new()
}

#[test]
fn a_deal_worth_it_by_our_own_price_is_accepted() {
    // The person offers the Ottomans Silver for 2 Gold a turn and 20 Gold:
    // about 137 Gold of Amenities for about 59.
    let deal = offer(
        0,
        vec![BoardItem::gold_per_turn(2), BoardItem::gold(20)],
        vec![luxury("RESOURCE_SILVER")],
    );
    let person = seen(OTTOMAN_T32, 0);
    assert_eq!(
        answer(&ottoman(), &deal, Some(&person), &none(), &Exchange::default()),
        Some(Answer::Accept)
    );
}

#[test]
fn a_deal_short_of_the_margin_is_countered_with_gold_the_other_side_holds() {
    // The Ottomans ask Canada for 10 Horses and offer nothing. The Horses
    // cost Canada nothing (its cap would waste them), but a deal must gain
    // the margin: Canada asks for Gold. Its board says the Ottomans hold 2.5
    // Gold and earn 2 a turn, so the ask is a Gold a turn, not a lump.
    let deal = offer(1, vec![horses(10)], vec![]);
    let ottomans = seen(CANADA_T32, 1);
    let answered = answer(&canada(), &deal, Some(&ottomans), &none(), &Exchange::default());
    let Some(Answer::Counter(terms)) = answered else {
        panic!("{answered:?}");
    };
    assert_eq!(terms.give, vec![horses(10)]);
    assert_eq!(terms.get, vec![BoardItem::gold_per_turn(1)]);
    assert_eq!(terms.render().as_deref(), Some("give=RESOURCE_HORSES=10;get=GOLD=1@30"));
}

#[test]
fn the_only_luxury_is_let_go_only_for_what_it_is_worth() {
    // The person offers Canada 20 Gold for its only Furs: Halifax and
    // Ottawa would fall Displeased. Canada counters with Gold a turn on top
    // (the person's 26 Gold cannot carry a larger lump), and the counter
    // still gives the Furs: dropping our only item would be a demand.
    let deal = offer(0, vec![luxury("RESOURCE_FURS")], vec![BoardItem::gold(20)]);
    let person = seen(CANADA_T32, 0);
    let seat = canada();
    let answered = answer(&seat, &deal, Some(&person), &none(), &Exchange::default());
    let Some(Answer::Counter(terms)) = answered else {
        panic!("{answered:?}");
    };
    assert_eq!(terms.give, vec![luxury("RESOURCE_FURS")]);
    let priced = terms.deal(&none()).expect("priced");
    let net = seat.net(&priced, Some(&person));
    assert!(net >= margin(&seat, &priced, Some(&person)), "{net}");
    assert!(terms.gold_from_them(true) > 0, "{terms:?}");
    // Asked of a seat with nothing to pay, there is no counter.
    let mut broke = person.clone();
    broke.gold = 0.0;
    broke.gold_per_turn = 0.0;
    assert_eq!(
        answer(&seat, &deal, Some(&broke), &none(), &Exchange::default()),
        Some(Answer::Reject("no_terms_worth_it"))
    );
}

#[test]
fn counters_stop_after_two_and_refused_terms_are_not_sent_again() {
    let deal = offer(1, vec![horses(10)], vec![]);
    let ottomans = seen(CANADA_T32, 1);
    let spent = Exchange {
        counters_sent: MAX_COUNTERS,
        ..Exchange::default()
    };
    assert_eq!(
        answer(&canada(), &deal, Some(&ottomans), &none(), &spent),
        Some(Answer::Reject("counters_spent"))
    );
    let refused = Exchange {
        counters_sent: 0,
        refused: BTreeSet::from(["give=RESOURCE_HORSES=10;get=GOLD=1@30".to_string()]),
    };
    let again = answer(&canada(), &deal, Some(&ottomans), &none(), &refused);
    assert_ne!(
        again,
        Some(Answer::Counter(Terms {
            give: vec![horses(10)],
            get: vec![BoardItem::gold_per_turn(1)],
        }))
    );
    // With the counters spent a deal worth it is still accepted.
    let fair = offer(1, vec![horses(10)], vec![BoardItem::gold_per_turn(1)]);
    assert_eq!(
        answer(&canada(), &fair, Some(&ottomans), &none(), &spent),
        Some(Answer::Accept)
    );
}

#[test]
fn a_gift_is_taken_and_an_unpriced_item_is_left_alone() {
    let gift = offer(0, vec![], vec![BoardItem::gold(10)]);
    assert_eq!(
        answer(&ottoman(), &gift, None, &none(), &Exchange::default()),
        Some(Answer::Accept)
    );
    let city = BoardItem {
        kind: "CITY".to_string(),
        name: Some("Trabzon".to_string()),
        ..BoardItem::default()
    };
    let for_a_city = offer(0, vec![city], vec![BoardItem::gold(500)]);
    assert_eq!(
        answer(&ottoman(), &for_a_city, None, &none(), &Exchange::default()),
        None,
        "a city is not the valuation's to price"
    );
}

/// Two seats, each answering from its own board, until one accepts or
/// rejects. Returns the accepted terms from `a`'s side (or `None`) and how
/// many deals crossed. Neither side reads the other's valuation: each sees
/// only the terms and its own board's view of the other seat.
fn exchange(
    a: &Position,
    a_sees_b: &StateRival,
    b: &Position,
    b_sees_a: &StateRival,
    opening: Terms,
) -> (Option<Terms>, usize) {
    // `on_table` is from the side of whoever must answer next.
    let mut on_table = Terms {
        give: opening.get.clone(),
        get: opening.give.clone(),
    };
    let mut seats = [(b, b_sees_a, Exchange::default()), (a, a_sees_b, Exchange::default())];
    for crossed in 1..=12 {
        let mover = (crossed - 1) % 2;
        let (seat, partner, history) = &seats[mover];
        let deal = offer(9, on_table.give.clone(), on_table.get.clone());
        let answered = answer(seat, &deal, Some(*partner), &none(), history)
            .expect("every item here is priced");
        match answered {
            Answer::Accept => {
                let from_a = if mover == 1 {
                    on_table
                } else {
                    Terms {
                        give: on_table.get,
                        get: on_table.give,
                    }
                };
                return (Some(from_a), crossed);
            }
            Answer::Reject(_) => return (None, crossed),
            Answer::Counter(terms) => {
                // Had the other side taken them, the exchange would be over:
                // on the board this is the outcome `countered`, and the
                // seat's next answer reads it as refused.
                let key = terms.key().expect("renders");
                let history = &mut seats[mover].2;
                assert!(history.refused.insert(key.clone()), "the same terms sent twice: {key}");
                history.counters_sent += 1;
                on_table = Terms {
                    give: terms.get,
                    get: terms.give,
                };
            }
        }
    }
    panic!("the exchange never ended");
}

#[test]
fn two_seats_reach_a_deal_both_gain_from_by_offer_and_counter() {
    // The Ottomans open: 10 of Canada's Horses, for nothing. Canada counters
    // a Gold a turn; the Ottomans, whose own price for 10 Horses is about
    // 45 Gold, accept.
    let (a, b) = (ottoman(), canada());
    let a_sees_b = seen(OTTOMAN_T32, 3);
    let b_sees_a = seen(CANADA_T32, 1);
    let opening = Terms {
        give: vec![],
        get: vec![horses(10)],
    };
    let (closed, crossed) = exchange(&a, &a_sees_b, &b, &b_sees_a, opening);
    let closed = closed.expect("both gain, so the deal closes");
    assert_eq!(crossed, 2, "an offer, a counter, an acceptance");
    let for_a = closed.deal(&none()).unwrap();
    let for_b = for_a.mirrored();
    let a_gain = a.net(&for_a, Some(&a_sees_b));
    let b_gain = b.net(&for_b, Some(&b_sees_a));
    assert!(a_gain >= margin(&a, &for_a, Some(&a_sees_b)), "{a_gain}");
    assert!(b_gain >= margin(&b, &for_b, Some(&b_sees_a)), "{b_gain}");
}

#[test]
fn a_deal_bad_for_one_side_is_never_closed_and_the_haggling_ends() {
    // A buyer who already holds Furs (so a copy is worth nothing to it)
    // offers 10 Gold for Canada's only copy. No terms work for both: the
    // exchange must end in a rejection, inside the counter limit, without
    // repeating itself.
    let seller = canada();
    let buyer = canada();
    let buyer_sees_seller = seen(OTTOMAN_T32, 3);
    let seller_sees_buyer = seen(CANADA_T32, 0);
    let opening = Terms {
        give: vec![BoardItem::gold(10)],
        get: vec![luxury("RESOURCE_FURS")],
    };
    let (closed, crossed) = exchange(
        &buyer,
        &buyer_sees_seller,
        &seller,
        &seller_sees_buyer,
        opening,
    );
    assert_eq!(closed, None);
    assert!(crossed <= 2 * MAX_COUNTERS as usize + 2, "{crossed}");
}

#[test]
fn the_exchange_history_is_read_from_the_board() {
    let deals = serde_json::json!({
        "v": 1,
        "incoming": [{"from": 0, "from_kind": "person", "id": 184467, "pending_turns": 1,
                      "give": [{"type": "RESOURCE", "name": "RESOURCE_FURS", "amount": 1, "duration": 30}],
                      "get": [{"type": "GOLD", "amount": 20, "duration": 0}],
                      "round": 3}],
        "outcomes": [
            {"with": 0, "id": 9, "turn": 30, "outcome": "rejected",
             "give": [{"type": "RESOURCE", "name": "RESOURCE_FURS", "amount": 1, "duration": 30}],
             "get": [{"type": "GOLD", "amount": 2, "duration": 30}]},
            {"with": 0, "id": 8, "turn": 12, "outcome": "rejected",
             "give": [{"type": "GOLD", "amount": 1, "duration": 0}], "get": []},
            {"with": 2, "id": 7, "turn": 31, "outcome": "countered"}
        ],
        "counterparts": [{"player": 0, "kind": "person"}, {"player": 2, "kind": "civvis"}]
    });
    let parsed = DealBoard::from_json(&deals).expect("deals v1");
    assert!(parsed.is_person(0));
    assert!(!parsed.is_person(2));
    let incoming = &parsed.incoming[0];
    assert_eq!(incoming.terms.give, vec![luxury("RESOURCE_FURS")]);
    assert_eq!(counters_sent(incoming, &parsed.outcomes, 32), 1, "round 3: one of ours");
    let mut unnumbered = incoming.clone();
    unnumbered.round = None;
    unnumbered.from = 2;
    assert_eq!(counters_sent(&unnumbered, &parsed.outcomes, 32), 1);
    assert_eq!(
        refused_terms(&parsed.outcomes, 0, 32),
        BTreeSet::from(["give=RESOURCE_FURS=1@30;get=GOLD=2@30".to_string()]),
        "the turn-12 refusal is older than the window"
    );
    assert_eq!(DealBoard::from_json(&serde_json::json!({"v": 2})), None);
    assert_eq!(
        DealBoard::from_json(&serde_json::json!({"v": 1, "incoming": {}})),
        Some(DealBoard::default()),
        "a Lua empty table where a list was expected is an empty list"
    );
    // Real-world sloppiness: an item without a type is dropped, a float id read.
    let sloppy = serde_json::json!({"v": 1.0, "incoming": [
        {"from": 4, "id": 77.0, "give": [{"amount": 3}], "get": [{"type": "FAVOR", "amount": 10}]}]});
    let sloppy = DealBoard::from_json(&sloppy).expect("v 1.0 is v1");
    assert_eq!(sloppy.incoming[0].id, 77);
    assert!(sloppy.incoming[0].terms.give.is_empty());
    assert_eq!(
        sloppy.incoming[0].terms.deal(&none()),
        Some(Deal {
            give: vec![],
            get: vec![super::super::Item::Favor(10)],
        })
    );
}
