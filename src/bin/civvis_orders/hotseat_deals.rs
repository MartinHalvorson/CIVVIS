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
    let position = Position::from_board(state, lane);
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
        assert!(notes[0].starts_with("own_price_deal=1:COUNTER:net="), "{notes:?}");
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
}
