use super::*;
use crate::ai::VictoryTarget;
use crate::trade_decider::answer::{answer, Answer, Exchange};
use crate::trade_decider::board::{DealOutcome, IncomingDeal, OutgoingDeal};
use crate::trade_decider::fixtures::{board, CANADA_T32, OTTOMAN_T32};

/// Seat p1 (holy, Ottoman) and seat p3 (moonshot, Canada) of the owner's
/// game `hotseat-20261128T045523Z` at turn 32, from their own boards.
fn ottoman() -> Position {
    Position::from_board(&board(OTTOMAN_T32), Some(VictoryTarget::Religion))
}

fn canada() -> Position {
    Position::from_board(&board(CANADA_T32), Some(VictoryTarget::Science))
}

fn rivals(json: &str) -> Vec<StateRival> {
    board(json).rivals
}

fn seen(json: &str, player: usize) -> StateRival {
    rivals(json)
        .into_iter()
        .find(|rival| rival.player == player)
        .expect("met")
}

/// The deal board of a seat that has nothing pending: who each other seat is.
fn table(counterparts: &[(usize, &str)]) -> DealBoard {
    DealBoard {
        counterparts: counterparts
            .iter()
            .map(|(player, kind)| (*player, kind.to_string()))
            .collect(),
        ..DealBoard::default()
    }
}

fn ottoman_table() -> DealBoard {
    table(&[(0, "person"), (2, "civvis"), (3, "civvis")])
}

fn none() -> BTreeMap<String, String> {
    BTreeMap::new()
}

fn gold_of(items: &[BoardItem], per_turn: bool) -> i64 {
    items
        .iter()
        .filter(|item| item.gold_kind() == Some(per_turn))
        .map(|item| item.amount)
        .sum()
}

#[test]
fn a_payment_is_lump_when_it_can_be_and_the_fewest_gold_a_turn_otherwise() {
    assert_eq!(payment(30.0, 19.6, 47, 2), Some(vec![BoardItem::gold(30)]));
    assert_eq!(
        payment(68.5, 19.6, 47, 2),
        Some(vec![BoardItem::gold_per_turn(2), BoardItem::gold(30)])
    );
    assert_eq!(
        payment(39.0, 19.6, 0, 2),
        Some(vec![BoardItem::gold_per_turn(2)])
    );
    assert_eq!(
        payment(100.0, 19.6, 47, 2),
        None,
        "more than the payer carries"
    );
    assert_eq!(payment(0.4, 19.6, 47, 2), None, "nothing worth a session");
}

#[test]
fn the_ottomans_ask_canada_for_its_furs_with_what_they_can_pay_and_canada_takes_it() {
    // Two Displeased cities and no luxury: a new type is worth about 137
    // Gold to the Ottomans. Canada, a CIVVIS seat, offers Furs; the person
    // offers Silver and Sugar for as much, but a seat is asked first.
    let seat = ottoman();
    let proposal = propose(&seat, &rivals(OTTOMAN_T32), &ottoman_table(), &none())
        .expect("a deal worth proposing");
    assert_eq!(proposal.to, 3);
    assert_eq!(
        proposal.terms.get,
        vec![BoardItem {
            kind: "RESOURCE".to_string(),
            name: Some("RESOURCE_FURS".to_string()),
            amount: 1,
            duration: 30,
            class: Some("luxury".to_string()),
            ..BoardItem::default()
        }]
    );
    // Half its worth to us, in what the Ottomans hold: 2 Gold a turn (their
    // whole income) and the rest from their 47 Gold.
    assert_eq!(gold_of(&proposal.terms.give, true), 2);
    let lump = gold_of(&proposal.terms.give, false);
    assert!((20..=40).contains(&lump), "{lump}");
    assert!(proposal.gain > 50.0, "{}", proposal.gain);
    // Canada answers from its own board: its only Furs cost it about 41.
    let incoming = IncomingDeal {
        from: 1,
        from_kind: "civvis".to_string(),
        id: 1,
        pending_turns: 0,
        terms: Terms {
            give: proposal.terms.get.clone(),
            get: proposal.terms.give.clone(),
        },
        counter_of: None,
        round: Some(1),
    };
    assert_eq!(
        answer(
            &canada(),
            &incoming,
            Some(&seen(CANADA_T32, 1)),
            &none(),
            &Exchange::default()
        ),
        Some(Answer::Accept)
    );
}

#[test]
fn the_person_is_asked_when_no_seat_offers_as_much_and_never_twice_with_the_same_terms() {
    let seat = ottoman();
    let people = table(&[(0, "person"), (2, "civvis")]);
    let first =
        propose(&seat, &rivals(OTTOMAN_T32), &people, &none()).expect("the person's Silver");
    assert_eq!(first.to, 0);
    assert!(first.gain >= PERSON_MIN_GAIN);
    // Refused: the same terms are never sent again; the other luxury is.
    let mut refused = people.clone();
    refused.refused_by_person.push((7, 20, first.terms.clone()));
    let second = propose(&seat, &rivals(OTTOMAN_T32), &refused, &none()).expect("Sugar instead");
    assert_eq!(second.to, 0);
    assert_ne!(second.terms.key(), first.terms.key());
    // Asked seven turns ago: the person is left alone for ten.
    let mut asked = people.clone();
    asked.outcomes.push(DealOutcome {
        with: 0,
        id: 7,
        turn: 25,
        outcome: "rejected".to_string(),
        terms: first.terms.clone(),
    });
    assert_eq!(propose(&seat, &rivals(OTTOMAN_T32), &asked, &none()), None);
}

#[test]
fn a_seat_with_a_deal_pending_either_way_or_inside_the_cooldown_is_not_asked() {
    let seat = ottoman();
    let pending = Terms::default();
    let mut incoming = ottoman_table();
    incoming.incoming.push(IncomingDeal {
        from: 3,
        from_kind: "civvis".to_string(),
        id: 9,
        pending_turns: 0,
        terms: pending.clone(),
        counter_of: None,
        round: None,
    });
    let mut outgoing = ottoman_table();
    outgoing.outgoing.push(OutgoingDeal {
        to: 3,
        id: 9,
        proposed_turn: 31,
        terms: pending,
    });
    let mut cooling = ottoman_table();
    cooling.outcomes.push(DealOutcome {
        with: 3,
        id: 9,
        turn: 29,
        outcome: "accepted".to_string(),
        terms: Terms::default(),
    });
    for board in [incoming, outgoing, cooling] {
        let proposal = propose(&seat, &rivals(OTTOMAN_T32), &board, &none());
        assert_ne!(proposal.as_ref().map(|p| p.to), Some(3), "{proposal:?}");
    }
    // A seat that is not a hotseat human (a Firaxis leader) is the sell/buy
    // lane's, never this one's.
    let leaders = table(&[(0, "ai"), (2, "ai"), (3, "ai")]);
    assert_eq!(
        propose(&seat, &rivals(OTTOMAN_T32), &leaders, &none()),
        None
    );
}

#[test]
fn an_offer_turned_down_comes_back_higher() {
    let mut history = History::default();
    assert_eq!(history.share("RESOURCE_FURS"), OPENING_SHARES[0]);
    history.turned_down.insert("RESOURCE_FURS".to_string(), 1);
    assert_eq!(history.share("RESOURCE_FURS"), OPENING_SHARES[1]);
    history.turned_down.insert("RESOURCE_FURS".to_string(), 9);
    assert_eq!(history.share("RESOURCE_FURS"), OPENING_SHARES[2]);
    // From the board: a refusal six turns ago (outside the five-turn
    // cooldown) raises the next offer for the same item.
    let mut board = ottoman_table();
    board.outcomes.push(DealOutcome {
        with: 3,
        id: 9,
        turn: 26,
        outcome: "rejected".to_string(),
        terms: Terms {
            give: vec![BoardItem::gold(20)],
            get: vec![luxury("RESOURCE_FURS")],
        },
    });
    let read = History::from_board(&board, 3, 32);
    assert_eq!(read.turned_down.get("RESOURCE_FURS"), Some(&1));
    assert_eq!(read.last_proposed, Some(26));
    assert!(read.refused.contains("give=GOLD=20;get=RESOURCE_FURS=1@30"));
}

#[test]
fn canada_sells_its_spare_horses_and_the_two_seats_settle_by_counter() {
    // Canada's 12 Horses rise 2 a turn toward the cap: 10 cost it nothing.
    // It asks the Ottomans 40 Gold, in what its board says they hold
    // (2 Gold a turn and 1 Gold); the Ottomans' own price for 10 Horses is
    // about 45, so they counter without the lump; Canada takes that.
    let seller = canada();
    let buyer = ottoman();
    let proposal = propose(
        &seller,
        &rivals(CANADA_T32),
        &table(&[(0, "person"), (1, "civvis")]),
        &none(),
    )
    .expect("the spare Horses");
    assert_eq!(proposal.to, 1);
    assert_eq!(proposal.terms.give.len(), 1);
    assert_eq!(
        proposal.terms.give[0].name.as_deref(),
        Some("RESOURCE_HORSES")
    );
    let mut on_table = Terms {
        give: proposal.terms.get.clone(),
        get: proposal.terms.give.clone(),
    };
    // Buyer first, then seller, each from its own board.
    let seats = [
        (&buyer, seen(OTTOMAN_T32, 3)),
        (&seller, seen(CANADA_T32, 1)),
    ];
    let mut counters = [0u32; 2];
    let mut closed = None;
    for step in 0..6 {
        let (seat, partner) = (&seats[step % 2].0, &seats[step % 2].1);
        let deal = IncomingDeal {
            from: 9,
            from_kind: "civvis".to_string(),
            id: 1,
            pending_turns: 0,
            terms: on_table.clone(),
            counter_of: None,
            round: None,
        };
        let exchange = Exchange {
            counters_sent: counters[step % 2],
            ..Exchange::default()
        };
        match answer(seat, &deal, Some(partner), &none(), &exchange).expect("priced") {
            Answer::Accept => {
                closed = Some((step, on_table.clone()));
                break;
            }
            Answer::Reject(why) => panic!("rejected at step {step}: {why}"),
            Answer::Counter(terms) => {
                counters[step % 2] += 1;
                on_table = Terms {
                    give: terms.get,
                    get: terms.give,
                };
            }
        }
    }
    let (step, terms) = closed.expect("the deal closes");
    assert_eq!(step, 1, "a counter, then the seller's acceptance");
    // From the seller's side: its Horses for Gold a turn, both sides gaining.
    let for_seller = terms.deal(&none()).unwrap();
    assert!(seller.net(&for_seller, None) > 0.0);
    assert!(buyer.net(&for_seller.mirrored(), None) > 0.0);
}
