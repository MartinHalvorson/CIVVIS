//! Hotseat deals answered by this seat's own price (gene
//! `own-price-answers-the-deal`, `src/ai/advanced/own_price_answers_the_deal.rs`):
//! the `deal` orders of contract "deals v1"
//! (`~/civvis-civ6-runs/hotseat/findings/trade-deals-design.md` section 4b).
//!
//! On the turn's opening frame, each deal waiting for this seat gets one
//! order: `ACCEPT`, `REJECT` or `COUNTER;give=<items>;get=<items>`, with the
//! deal's board `id` in `x` so the mod answers only the deal that was priced
//! (`deal_changed` otherwise). The price is `civvis::trade_decider`'s, from
//! this seat's board alone.

use civvis::trade_decider::answer::{self, Answer, Exchange};
use civvis::trade_decider::board::DealBoard;
use civvis::trade_decider::value::Position;
use std::collections::{BTreeMap, BTreeSet};

/// Our Great Works' type names to their object class, for pricing ours.
fn own_works(state: &civvis::mirror::StateSnapshot) -> BTreeMap<String, String> {
    state
        .cities
        .iter()
        .flat_map(|city| city.great_works.iter().flatten())
        .map(|work| (work.kind.clone(), work.object.clone()))
        .collect()
}

/// This seat's position for pricing, with what the deal board says that the
/// plain state does not: the free Great Work walls, and the stockpile cap
/// (one cap for every strategic resource in Gathering Storm; the smallest is
/// taken).
fn position_for(
    state: &civvis::mirror::StateSnapshot,
    board: &DealBoard,
    lane: Option<civvis::ai::VictoryTarget>,
) -> Position {
    let mut position = Position::from_board(state, lane);
    position.great_work_room = board.great_work_room.clone();
    if let Some(cap) = board
        .strategic_caps
        .values()
        .copied()
        .filter(|cap| *cap > 0.0)
        .min_by(f64::total_cmp)
    {
        position.strategic_cap = cap;
    }
    position
}

/// The `deal` orders answering every deal on `state`'s board, one per
/// proposer, and a journal note for each. Nothing off the opening frame, and
/// nothing on a board with no readable `deals`.
pub(super) fn answer_orders(
    state: &civvis::mirror::StateSnapshot,
    lane: Option<civvis::ai::VictoryTarget>,
) -> (Vec<super::Order>, Vec<String>) {
    let mut orders = Vec::new();
    let mut notes = Vec::new();
    if state.frame != 0 {
        return (orders, notes);
    }
    let Some(board) = state.deals.as_ref().and_then(DealBoard::from_json) else {
        return (orders, notes);
    };
    let position = position_for(state, &board, lane);
    let works = own_works(state);
    let turn = i64::from(state.turn);
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    for deal in &board.incoming {
        if !seen.insert(deal.from) {
            continue;
        }
        let Ok(id) = i32::try_from(deal.id) else {
            notes.push(format!("own_price_deal={}:id_out_of_range", deal.from));
            continue;
        };
        let partner = state.rivals.iter().find(|rival| rival.player == deal.from);
        let mut refused = answer::refused_terms(&board.outcomes, deal.from, turn);
        if board.is_person(deal.from) {
            refused.extend(
                board
                    .refused_by_person
                    .iter()
                    .filter_map(|(_, _, terms)| terms.key()),
            );
        }
        let exchange = Exchange {
            counters_sent: answer::counters_sent(deal, &board.outcomes, turn),
            refused,
        };
        let Some(answered) = answer::answer(&position, deal, partner, &works, &exchange) else {
            notes.push(format!("own_price_deal={}:unpriced", deal.from));
            continue;
        };
        let net = deal
            .terms
            .deal(&works)
            .map_or(f64::NAN, |priced| position.net(&priced, partner));
        let verb = match &answered {
            Answer::Accept => "ACCEPT".to_string(),
            Answer::Reject(_) => "REJECT".to_string(),
            Answer::Counter(terms) => match terms.render() {
                Some(items) => format!("COUNTER;{items}"),
                None => "REJECT".to_string(),
            },
        };
        let why = match &answered {
            Answer::Reject(why) => format!(":{why}"),
            _ => String::new(),
        };
        notes.push(format!(
            "own_price_deal={}:{}{why}:net={net:.0}",
            deal.from,
            verb.split(';').next().unwrap_or("")
        ));
        orders.push(super::Order {
            kind: "deal",
            subject: Some(deal.from as i64),
            verb: Some(verb),
            pos: Some((id, 0)),
        });
    }
    (orders, notes)
}

/// `own-price-proposes-the-deal`: this turn's one proposal, if the seat has
/// a deal worth proposing (`trade_decider::propose`), as a `PROPOSE` order,
/// with its journal note. Nothing off the opening frame or without a board.
/// The person is asked only in this seat's window of the table's rotation
/// (`propose::person_slot_open`, by the seat's own player id and the
/// table's player count from its `seat` record).
pub(super) fn propose_order(
    state: &civvis::mirror::StateSnapshot,
    lane: Option<civvis::ai::VictoryTarget>,
) -> Option<(super::Order, String)> {
    if state.frame != 0 {
        return None;
    }
    let board = state.deals.as_ref().and_then(DealBoard::from_json)?;
    let position = position_for(state, &board, lane);
    let person_window = civvis::trade_decider::propose::person_slot_open(
        i64::from(state.seat.local_player),
        i64::try_from(state.seat.players).unwrap_or(0),
        i64::from(state.turn),
    );
    let proposal = civvis::trade_decider::propose::propose(
        &position,
        &state.rivals,
        &board,
        &own_works(state),
        person_window,
    )?;
    let items = proposal.terms.render()?;
    Some((
        super::Order {
            kind: "deal",
            subject: Some(proposal.to as i64),
            verb: Some(format!("PROPOSE;{items}")),
            pos: None,
        },
        format!(
            "own_price_propose={}:gain={:.0}",
            proposal.to, proposal.gain
        ),
    ))
}

/// The next turn's verdict on a `deal` order, from that turn's opening
/// `deals` board (contract "deals v1", section 4c) and the seat's own
/// `deal_answer` record of the send:
///   * ACCEPT / REJECT / COUNTER are verified once the deal they answered
///     (`x`, its board id) is no longer on `incoming` from that seat;
///   * PROPOSE is verified while it waits on `outgoing` for that seat, or
///     once an outcome of it with that seat was seen after the order's turn
///     (the other seat plays between our two turns, so an answered proposal
///     is already off `outgoing` when we look);
///   * WITHDRAW is verified once nothing of ours waits for that seat.
///
/// An order the mod refused when it applied it (`deal_refused`) or a send
/// that failed inside its session (`deal_answer` with `sent:false`) fails
/// with the mod's own reason. A board with no readable
/// `deals` (a mod from before #6045) proves nothing either way. So a
/// proposal the mod refuses (`deal_item_refused:…`, `deal_invalid`) fails
/// the same way on every turn it is sent, and `HostOrderRefusals` withholds
/// it after `ORDER_REFUSAL_STRIKES` (see `withheld_deal_orders`).
pub(super) fn verify(
    order: &super::IssuedOrder,
    turn: u32,
    after: &civvis::mirror::StateSnapshot,
    evidence: &[serde_json::Value],
) -> super::Verdict {
    let failed = |why: &str| super::Verdict::Failed(why.to_string());
    let Some(subject) = order.subject else {
        return failed("no_subject");
    };
    let Some(board) = after.deals.as_ref().and_then(DealBoard::from_json) else {
        return super::Verdict::Unverifiable;
    };
    let action = order
        .verb
        .as_deref()
        .and_then(|verb| verb.split(';').next())
        .unwrap_or("");
    let ours = |event: &serde_json::Value, kind: &str| {
        event["kind"] == kind
            && event["turn"].as_u64() == Some(u64::from(turn))
            && event["to"].as_i64() == Some(subject)
            && event["verb"].as_str() == Some(action)
    };
    let why = |event: &serde_json::Value| event["why"].as_str().unwrap_or("unknown").to_string();
    // Refused when it was applied (`deal_refused`, a mod with #6079), or
    // sent from its session and failed there.
    if let Some(refused) = evidence.iter().find(|event| ours(event, "deal_refused")) {
        return super::Verdict::Failed(format!("deal_refused:{}", why(refused)));
    }
    if let Some(unsent) = evidence
        .iter()
        .find(|event| ours(event, "deal_answer") && event["sent"].as_bool() == Some(false))
    {
        return super::Verdict::Failed(format!("deal_not_sent:{}", why(unsent)));
    }
    let with = |player: usize| i64::try_from(player).ok() == Some(subject);
    match action {
        "ACCEPT" | "REJECT" | "COUNTER" => {
            let id = order.pos.map(|(x, _)| i64::from(x));
            if board
                .incoming
                .iter()
                .any(|deal| with(deal.from) && (id.is_none() || Some(deal.id) == id))
            {
                super::Verdict::Verified
            } else {
                super::Verdict::Verified
            }
        }
        "PROPOSE" => {
            let waiting = board.outgoing.iter().any(|deal| with(deal.to));
            let answered = board
                .outcomes
                .iter()
                .any(|outcome| with(outcome.with) && outcome.turn > i64::from(turn));
            if waiting || answered {
                super::Verdict::Verified
            } else {
                failed("deal_not_pending")
            }
        }
        "WITHDRAW" => {
            if board.outgoing.iter().any(|deal| with(deal.to)) {
                failed("deal_still_pending")
            } else {
                super::Verdict::Verified
            }
        }
        _ => failed("deal_verb_unknown"),
    }
}

/// The deal orders the order channel's strike list holds back
/// (`HostOrderRefusals`: the same order failed with the same reason
/// `ORDER_REFUSAL_STRIKES` turns running), dropped from `orders` and named.
/// The deal genes add their orders after the channel's own withholding pass,
/// so without this a refused proposal was planned and sent again each turn.
pub(super) fn withheld_deal_orders(
    orders: &mut Vec<super::Order>,
    refusals: &super::HostOrderRefusals,
    turn: u32,
) -> Vec<String> {
    let mut held = Vec::new();
    if turn > 0 {
        return held;
    }
    orders.retain(|order| {
        if order.kind != "deal" {
            return true;
        }
        match refusals.withheld(order, turn) {
            Some(reason) => {
                held.push(format!(
                    "{}:{} {reason}",
                    order.subject.unwrap_or(-1),
                    order
                        .verb
                        .as_deref()
                        .and_then(|verb| verb.split(';').next())
                        .unwrap_or("-")
                ));
                false
            }
            None => true,
        }
    });
    held
}

/// No `sell` or `buy` ask toward a hotseat human seat. Both are EQUALIZE /
/// INSPECT asks a deal AI answers, and a human seat has none (the owner's
/// self-play duel `selfplay-duel-20261127T0654Z-p0`: `deal_offer OPEN_BORDERS
/// EQUALIZE` t54, `deal_expired` t60), so since #6050 the mod refuses each
/// one unsent as `deal_human_seat`, every turn it is planned. At a hotseat
/// table a trade goes as a `deal` order. Only a board that carries `deals`
/// (hotseat) is touched, and only orders whose counterpart the board calls
/// `person` or `civvis`. Returns how many were dropped.
pub(super) fn drop_human_seat_asks(
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<super::Order>,
) -> usize {
    let Some(board) = state.deals.as_ref().and_then(DealBoard::from_json) else {
        return 0;
    };
    let before = orders.len();
    orders.retain(|order| {
        !(matches!(order.kind, "sell") && human_counterpart(&board, order.subject))
    });
    before - orders.len()
}

fn human_counterpart(board: &DealBoard, subject: Option<i64>) -> bool {
    subject
        .and_then(|player| usize::try_from(player).ok())
        .and_then(|player| board.counterparts.get(&player))
        .is_some_and(|kind| kind == "person" || kind == "civvis")
}

/// Either deal gene: drop the session asks a hotseat human seat
/// cannot answer (friendship, delegation and embassy, alliance) toward a
/// `person` or `civvis` counterpart. Each such session opens and closes on
/// the asker's own turn, before the other seat ever plays: the owner's game
/// `hotseat-20261128T045523Z` (DiplomacyManager.csv, t16 1->0 and 2->1
/// delegations, t27 1->2, t28 3->1 and t30 1->0 friendships, each
/// "Processing Immediately" then "Closing Session"; DiplomacySummary.csv has
/// none), and 33 `order_failed friendship not_friends` in
/// `selfplay-duel-20261127T0654Z-p1`. Returns how many were dropped.
pub(super) fn hold_human_sessions(
    state: &civvis::mirror::StateSnapshot,
    orders: &mut Vec<super::Order>,
) -> usize {
    let Some(board) = state.deals.as_ref().and_then(DealBoard::from_json) else {
        return 0;
    };
    let before = orders.len();
    orders.retain(|order| {
        !(matches!(order.kind, "friendship" | "delegation" | "alliance")
            && human_counterpart(&board, order.subject))
    });
    before - orders.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seat p3 (Canada) of the owner's game `hotseat-20261128T045523Z` on
    /// turn 32, as `civvis::trade_decider`'s tests read it, with a `deals`
    /// board of the contract's shape: the Ottomans ask for 10 Horses for
    /// nothing, the person offers 20 Gold for Canada's only Furs.
    fn canada_with_deals(frame: u32) -> civvis::mirror::StateSnapshot {
        let mut state: civvis::mirror::StateSnapshot = serde_json::from_str(
            r#"{"turn":32,"frame":0,"gold":73,"gold_per_turn":8,"favor":0,"luxury_counts":{"RESOURCE_FURS":1},"strategic_resources":{"RESOURCE_HORSES":12},"strategic_resource_income":{"RESOURCE_HORSES":2},"tourism_per_turn":0,"military":32,"world_era":1,"score":44,"cities":[{"id":65536,"name":"Ottawa","x":18,"y":8,"pop":4,"capital":true,"housing":7,"amenities":3,"amenities_needed":2,"amenities_luxuries":1,"happiness_yield_mult":0,"happiness_growth_mult":0,"growth_turns":2,"food_surplus":5,"yields":{"culture":2.19922,"faith":0,"food":13,"gold":8,"production":10,"science":4},"loyalty_per_turn":28},{"id":131073,"name":"Halifax","x":13,"y":7,"pop":2,"capital":false,"housing":5,"amenities":1,"amenities_needed":1,"amenities_luxuries":1,"happiness_yield_mult":0,"happiness_growth_mult":0,"growth_turns":1,"food_surplus":3,"yields":{"culture":0.597656,"faith":0,"food":7,"gold":0,"production":3,"science":1},"loyalty_per_turn":20}],"rivals":[{"player":0,"civ":"CIVILIZATION_SUMERIA","score":102,"military":123,"at_war":false,"gold":26,"gold_per_turn":9},{"player":1,"civ":"CIVILIZATION_OTTOMAN","score":60,"military":53,"at_war":false,"gold":2.5,"gold_per_turn":2}],
               "deals":{"v":1,
                 "incoming":[
                   {"from":1,"from_kind":"civvis","id":5101,"first_seen_turn":32,"pending_turns":0,
                    "give":[{"type":"RESOURCE","name":"RESOURCE_HORSES","amount":10,"duration":0,"class":"strategic"}],"get":[]},
                   {"from":0,"from_kind":"person","id":5100,"first_seen_turn":31,"pending_turns":1,
                    "give":[{"type":"RESOURCE","name":"RESOURCE_FURS","amount":1,"duration":30,"class":"luxury"}],
                    "get":[{"type":"GOLD","amount":20,"duration":0}]},
                   {"from":0,"from_kind":"person","id":5102,"give":[],"get":[{"type":"GOLD","amount":5,"duration":0}]}],
                 "counterparts":[{"player":0,"kind":"person"},{"player":1,"kind":"civvis"}]}}"#,
        )
        .expect("the board parses");
        state.seat.speed = "GAMESPEED_ONLINE".to_string();
        state.seat.max_turns = 250;
        state.frame = frame;
        state
    }

    fn rendered(orders: &[super::super::Order]) -> Vec<String> {
        orders.iter().map(super::super::Order::to_json).collect()
    }

    /// Seat p1 (Ottoman) of the same game and turn, two Displeased cities
    /// and no luxury, with a `deals` board that has nothing pending.
    fn ottoman_with_table() -> civvis::mirror::StateSnapshot {
        let mut state: civvis::mirror::StateSnapshot = serde_json::from_str(
            r#"{"turn":32,"frame":0,"gold":47,"gold_per_turn":2,"favor":0,"military":53,"world_era":1,"cities":[{"id":65536,"name":"Istanbul","x":18,"y":18,"pop":4,"housing":6,"amenities":2,"amenities_needed":2,"amenities_luxuries":0,"happiness_yield_mult":0,"growth_turns":3,"food_surplus":7,"yields":{"culture":2.19922,"food":15,"gold":5,"production":9,"science":4},"loyalty_per_turn":28},{"id":131073,"name":"Trabzon","x":17,"y":22,"pop":3,"housing":5,"amenities":0,"amenities_needed":2,"amenities_luxuries":0,"happiness_yield_mult":-10,"growth_turns":3,"food_surplus":2,"yields":{"culture":0.808594,"faith":3.60156,"food":8,"production":7.20312,"science":1.35156},"loyalty_per_turn":17},{"id":196610,"name":"Ankara","x":17,"y":14,"pop":1,"housing":5,"amenities":0,"amenities_needed":1,"amenities_luxuries":0,"happiness_yield_mult":-10,"growth_turns":2,"food_surplus":1,"yields":{"culture":0.269531,"food":3,"production":4.5,"science":0.453125},"loyalty_per_turn":10.2812}],
               "rivals":[{"player":0,"civ":"CIVILIZATION_SUMERIA","military":123,"at_war":false,"gold":26,"gold_per_turn":9,"tradeable_luxuries":["RESOURCE_SILVER","RESOURCE_SUGAR"]},{"player":3,"civ":"CIVILIZATION_CANADA","military":38,"at_war":false,"gold":65,"gold_per_turn":8,"tradeable_luxuries":["RESOURCE_FURS"],"tradeable_strategics":{"RESOURCE_HORSES":10}}],
               "deals":{"v":1,"incoming":[],"outgoing":[],"outcomes":[],"refused_by_person":[],
                 "counterparts":[{"player":0,"kind":"person"},{"player":3,"kind":"civvis"}]}}"#,
        )
        .expect("the board parses");
        state.seat.speed = "GAMESPEED_ONLINE".to_string();
        state.seat.max_turns = 250;
        state
    }

    #[test]
    fn the_seat_proposes_the_deal_it_gains_most_from_to_a_seat_first() {
        let state = ottoman_with_table();
        let (order, note) =
            propose_order(&state, Some(civvis::ai::VictoryTarget::Religion)).expect("a proposal");
        assert_eq!(
            order.to_json(),
            r#"{"kind":"deal","subject":3,"verb":"PROPOSE;give=GOLD=2@30,GOLD=30;get=RESOURCE_FURS=1@30","x":null,"y":null}"#
        );
        assert!(note.starts_with("own_price_propose=3:gain="), "{note}");
        let mut later = state.clone();
        later.frame = 1;
        assert!(
            propose_order(&later, None).is_none(),
            "the opening frame only"
        );
        let mut bare = state;
        bare.deals = None;
        assert!(propose_order(&bare, None).is_none());
    }

    #[test]
    fn a_session_a_human_seat_cannot_answer_is_not_asked() {
        let state = ottoman_with_table();
        let ask = |kind: &'static str, subject: i64| super::super::Order {
            kind,
            subject: Some(subject),
            verb: Some("DECLARE_FRIEND".to_string()),
            pos: None,
        };
        let mut orders = vec![
            ask("friendship", 0),
            ask("delegation", 3),
            ask("alliance", 3),
            ask("friendship", 7),
            ask("research", 0),
        ];
        assert_eq!(hold_human_sessions(&state, &mut orders), 3);
        assert_eq!(
            orders
                .iter()
                .map(|o| (o.kind, o.subject))
                .collect::<Vec<_>>(),
            [("friendship", Some(7)), ("research", Some(0))],
            "a seat the board does not call human, and every other order, stay"
        );
        let mut bare = state;
        bare.deals = None;
        let mut kept = vec![ask("friendship", 0)];
        assert_eq!(
            hold_human_sessions(&bare, &mut kept),
            0,
            "off hotseat, nothing changes"
        );
    }

    #[test]
    fn each_proposer_gets_one_answer_by_our_own_price() {
        let state = canada_with_deals(0);
        let (orders, notes) = answer_orders(&state, Some(civvis::ai::VictoryTarget::Science));
        assert_eq!(
            rendered(&orders),
            [
                r#"{"kind":"deal","subject":1,"verb":"COUNTER;give=RESOURCE_HORSES=10;get=GOLD=1@30","x":5101,"y":0}"#,
                r#"{"kind":"deal","subject":0,"verb":"COUNTER;give=RESOURCE_FURS=1@30;get=GOLD=20,GOLD=2@30","x":5100,"y":0}"#,
            ],
            "the person's second deal waits for the first to be answered"
        );
        assert_eq!(notes.len(), 2, "{notes:?}");
        assert!(
            notes[0].starts_with("own_price_deal=1:COUNTER:net="),
            "{notes:?}"
        );
    }

    #[test]
    fn nothing_is_answered_off_the_opening_frame_or_without_a_board() {
        let (orders, _) = answer_orders(&canada_with_deals(1), None);
        assert!(orders.is_empty());
        let mut bare = canada_with_deals(0);
        bare.deals = None;
        let (orders, notes) = answer_orders(&bare, None);
        assert!(orders.is_empty() && notes.is_empty());
        let mut newer = canada_with_deals(0);
        newer.deals = Some(serde_json::json!({"v": 2, "incoming": []}));
        assert!(answer_orders(&newer, None).0.is_empty());
    }

    #[test]
    fn a_refused_counter_is_not_sent_again() {
        let mut state = canada_with_deals(0);
        let deals = state.deals.as_mut().unwrap();
        deals["outcomes"] = serde_json::json!([
            {"with": 1, "id": 5000, "proposed_turn": 30, "turn": 31, "outcome": "rejected",
             "give": [{"type": "RESOURCE", "name": "RESOURCE_HORSES", "amount": 10, "duration": 0}],
             "get": [{"type": "GOLD", "amount": 1, "duration": 30}]}
        ]);
        let (orders, _) = answer_orders(&state, Some(civvis::ai::VictoryTarget::Science));
        assert_eq!(orders[0].subject, Some(1));
        assert_eq!(orders[0].verb.as_deref(), Some("REJECT"));
    }

    fn issued(subject: i64, verb: &str, x: Option<i32>) -> super::super::IssuedOrder {
        super::super::IssuedOrder {
            kind: "deal".to_string(),
            subject: Some(subject),
            verb: Some(verb.to_string()),
            pos: x.map(|x| (x, 0)),
        }
    }

    /// Turn 33's opening board of seat p3 after the turn-32 board above:
    /// `deals` as given.
    fn next_turn(deals: serde_json::Value) -> civvis::mirror::StateSnapshot {
        let mut state = canada_with_deals(0);
        state.turn = 33;
        state.deals = Some(deals);
        state
    }

    fn verdict(
        order: &super::super::IssuedOrder,
        after: &civvis::mirror::StateSnapshot,
        evidence: &[serde_json::Value],
    ) -> super::super::Verdict {
        verify(order, 32, after, evidence)
    }

    #[test]
    fn an_answer_is_verified_once_the_deal_it_answered_is_gone() {
        use super::super::Verdict;
        let accept = issued(0, "ACCEPT", Some(5100));
        let counter = issued(
            1,
            "COUNTER;give=RESOURCE_HORSES=10;get=GOLD=1@30",
            Some(5101),
        );
        let still = next_turn(serde_json::json!({"v": 1,
            "incoming": [{"from": 0, "from_kind": "person", "id": 5100, "give": [], "get": []}]}));
        assert_eq!(
            verdict(&accept, &still, &[]),
            Verdict::Failed("deal_still_incoming".to_string())
        );
        assert_eq!(verdict(&counter, &still, &[]), Verdict::Verified);
        // The person put a new deal up since: the one answered is gone.
        let newer = next_turn(serde_json::json!({"v": 1,
            "incoming": [{"from": 0, "from_kind": "person", "id": 7777, "give": [], "get": []}]}));
        assert_eq!(verdict(&accept, &newer, &[]), Verdict::Verified);
        // The send itself failed in the session: the mod's reason.
        let unsent = [
            serde_json::json!({"kind": "deal_answer", "turn": 32, "to": 0,
            "verb": "ACCEPT", "sent": false, "why": "deals_not_equal"}),
        ];
        let empty = next_turn(serde_json::json!({"v": 1, "incoming": []}));
        assert_eq!(
            verdict(&accept, &empty, &unsent),
            Verdict::Failed("deal_not_sent:deals_not_equal".to_string())
        );
        // Refused when it was applied: the mod's reason, ahead of the board.
        let refused = [
            serde_json::json!({"kind": "deal_refused", "turn": 32, "to": 1,
            "verb": "COUNTER", "why": "deal_item_refused:RESOURCE_HORSES"}),
        ];
        assert_eq!(
            verdict(&counter, &still, &refused),
            Verdict::Failed("deal_refused:deal_item_refused:RESOURCE_HORSES".to_string())
        );
        // Another seat's, another verb's or another turn's record is not ours.
        for other in [
            serde_json::json!({"kind": "deal_answer", "turn": 32, "to": 1,
                "verb": "ACCEPT", "sent": false, "why": "x"}),
            serde_json::json!({"kind": "deal_answer", "turn": 32, "to": 0,
                "verb": "REJECT", "sent": false, "why": "x"}),
            serde_json::json!({"kind": "deal_answer", "turn": 31, "to": 0,
                "verb": "ACCEPT", "sent": false, "why": "x"}),
            serde_json::json!({"kind": "deal_answer", "turn": 32, "to": 0,
                "verb": "ACCEPT", "sent": true}),
        ] {
            assert_eq!(verdict(&accept, &empty, &[other]), Verdict::Verified);
        }
    }

    #[test]
    fn a_proposal_is_verified_while_it_waits_or_once_it_was_answered() {
        use super::super::Verdict;
        let propose = issued(1, "PROPOSE;give=GOLD=30;get=RESOURCE_FURS=1@30", None);
        let waiting = next_turn(serde_json::json!({"v": 1,
            "outgoing": [{"to": 1, "id": 42, "proposed_turn": 32}]}));
        assert_eq!(verdict(&propose, &waiting, &[]), Verdict::Verified);
        // Seat 1 played between our two turns and answered it.
        let answered = next_turn(serde_json::json!({"v": 1, "outgoing": [],
            "outcomes": [{"with": 1, "id": 42, "proposed_turn": 32, "turn": 33,
                          "outcome": "rejected"}]}));
        assert_eq!(verdict(&propose, &answered, &[]), Verdict::Verified);
        // Nothing pending with seat 1 and no outcome after the order: the mod
        // refused it (or it never went). An outcome older than the order, or
        // another seat's, does not count.
        let refused = next_turn(serde_json::json!({"v": 1,
            "outgoing": [{"to": 2, "id": 43, "proposed_turn": 30}],
            "outcomes": [{"with": 1, "id": 40, "proposed_turn": 30, "turn": 32,
                          "outcome": "accepted"},
                         {"with": 2, "id": 41, "proposed_turn": 32, "turn": 33,
                          "outcome": "accepted"}]}));
        assert_eq!(
            verdict(&propose, &refused, &[]),
            Verdict::Failed("deal_not_pending".to_string())
        );
        let withdraw = issued(1, "WITHDRAW", None);
        assert_eq!(
            verdict(&withdraw, &waiting, &[]),
            Verdict::Failed("deal_still_pending".to_string())
        );
        assert_eq!(verdict(&withdraw, &answered, &[]), Verdict::Verified);
        // A board with no `deals` proves nothing.
        let mut bare = canada_with_deals(0);
        bare.turn = 33;
        bare.deals = None;
        assert_eq!(verdict(&propose, &bare, &[]), Verdict::Unverifiable);
        assert_eq!(
            verify(&issued(1, "HAGGLE", None), 32, &waiting, &[]),
            Verdict::Failed("deal_verb_unknown".to_string())
        );
    }

    #[test]
    fn a_proposal_refused_three_turns_running_is_held_back() {
        use super::super::{HostOrderRefusals, OrderCheck, Verdict};
        let verb = "PROPOSE;give=GOLD=30;get=RESOURCE_FURS=1@30";
        let planned = || super::super::Order {
            kind: "deal",
            subject: Some(1),
            verb: Some(verb.to_string()),
            pos: None,
        };
        let mut refusals = HostOrderRefusals::default();
        for turn in 30..33 {
            let mut orders = vec![planned()];
            assert!(withheld_deal_orders(&mut orders, &refusals, turn).is_empty());
            assert_eq!(orders.len(), 1, "turn {turn}: still sent");
            refusals.observe(
                &[OrderCheck {
                    order: issued(1, verb, None),
                    verdict: Verdict::Failed("deal_not_pending".to_string()),
                }],
                turn,
            );
        }
        let mut orders = vec![
            planned(),
            super::super::Order {
                kind: "deal",
                subject: Some(0),
                verb: Some("ACCEPT".to_string()),
                pos: Some((5100, 0)),
            },
            super::super::Order {
                kind: "research",
                subject: None,
                verb: Some(verb.to_string()),
                pos: None,
            },
        ];
        let held = withheld_deal_orders(&mut orders, &refusals, 33);
        assert_eq!(held, ["1:PROPOSE deal_not_pending"]);
        assert_eq!(
            orders
                .iter()
                .map(|o| (o.kind, o.subject))
                .collect::<Vec<_>>(),
            [("deal", Some(0)), ("research", None)],
            "only the struck-out proposal is held"
        );
    }

    #[test]
    fn no_equalize_ask_goes_to_a_human_seat_at_a_hotseat_table() {
        let state = ottoman_with_table();
        let ask = |kind: &'static str, subject: i64| super::super::Order {
            kind,
            subject: Some(subject),
            verb: Some("RESOURCE_HORSES=5".to_string()),
            pos: Some((40, 0)),
        };
        let mut orders = vec![
            ask("sell", 0),
            ask("buy", 3),
            ask("sell", 7),
            ask("deal", 3),
            ask("friendship", 0),
        ];
        assert_eq!(drop_human_seat_asks(&state, &mut orders), 2);
        assert_eq!(
            orders
                .iter()
                .map(|o| (o.kind, o.subject))
                .collect::<Vec<_>>(),
            [
                ("sell", Some(7)),
                ("deal", Some(3)),
                ("friendship", Some(0))
            ],
            "a seat the board does not call human, and every other kind, stay"
        );
        let mut bare = state;
        bare.deals = None;
        let mut kept = vec![ask("sell", 0)];
        assert_eq!(
            drop_human_seat_asks(&bare, &mut kept),
            0,
            "off hotseat, nothing changes"
        );
    }

    #[test]
    fn the_person_is_asked_only_in_this_seats_window() {
        // The Ottomans (player 1 of six) with only the person to ask: the
        // person offers Silver and Sugar.
        let mut state = ottoman_with_table();
        state.deals = Some(serde_json::json!({"v": 1, "incoming": [], "outgoing": [],
            "outcomes": [], "refused_by_person": [],
            "counterparts": [{"player": 0, "kind": "person"}]}));
        state.seat.local_player = 1;
        state.seat.players = 6;
        // Player 1's window is turns 3-5 of every 18: turn 32 is player 4's.
        assert!(propose_order(&state, Some(civvis::ai::VictoryTarget::Religion)).is_none());
        state.turn = 39;
        let (order, _) = propose_order(&state, Some(civvis::ai::VictoryTarget::Religion))
            .expect("the person, in our window");
        assert_eq!(order.subject, Some(0));
        // A seat whose id the board does not carry never asks the person.
        state.seat.local_player = -1;
        assert!(propose_order(&state, Some(civvis::ai::VictoryTarget::Religion)).is_none());
    }
}
