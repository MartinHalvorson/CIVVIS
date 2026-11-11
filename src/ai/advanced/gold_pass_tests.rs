//! The gold pass's shortcuts refuse nothing the full pass would have bought.
//!
//! `gold_purchase_score` refuses on the quoted price before it values an
//! item, and `advanced_gold_spending` stops before listing a menu when the
//! treasury is already under its reserve. Both stand on one fact about the
//! engine — a purchase charges its quote in one subtraction and moves no
//! other Gold — and the first test pins that fact on boards from a played
//! game. The second compares the two scorers over those boards at a spread
//! of treasuries and reserves.

use super::*;
use crate::ai::{run_game_observed, Ai, BasicAi};

/// The world at turns 30, 55 and 80 of one game: three majors seated as the
/// site seats them, two city-states.
fn played_boards() -> Vec<Game> {
    let mut game = Game::new_with(crate::game::GameOptions::new(3, 40, 26, 20_261_111, 80, 2));
    let mut ais: Vec<Box<dyn Ai + Send>> = game
        .players
        .iter()
        .map(|player| -> Box<dyn Ai + Send> {
            if player.is_minor || player.is_barbarian {
                Box::new(BasicAi::new())
            } else {
                let mut ai = AdvancedAi::new();
                ai.enable_site_deployment();
                Box::new(ai)
            }
        })
        .collect();
    let mut boards = Vec::new();
    run_game_observed(&mut game, &mut ais, |board| {
        if [30, 55, 80].contains(&board.turn) {
            boards.push(board.clone());
        }
    });
    assert_eq!(boards.len(), 3, "fixture: the game reaches turn 80");
    boards
}

fn majors(game: &Game) -> Vec<usize> {
    game.players
        .iter()
        .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .collect()
}

/// A Gold purchase of a unit, a building or a district, as the pass reads it.
fn gold_option(action: &Action) -> Option<(u32, Item)> {
    let (city, item, currency) = match action {
        Action::Buy {
            city,
            unit,
            formation,
            currency,
        } => (
            *city,
            if *formation == 0 {
                Item::Unit { unit: *unit }
            } else {
                Item::Formation {
                    unit: *unit,
                    formation: *formation,
                }
            },
            currency.as_str(),
        ),
        Action::BuyBuilding {
            city,
            building,
            currency,
        } => (
            *city,
            Item::Building {
                building: *building,
            },
            currency.as_str(),
        ),
        Action::BuyDistrict {
            city,
            district,
            pos,
            currency,
        } => (
            *city,
            Item::District {
                district: *district,
                pos: *pos,
            },
            currency.as_str(),
        ),
        _ => return None,
    };
    (currency == "gold").then_some((city, item))
}

/// `apply` charges exactly what `purchase_quote` quotes, in one subtraction,
/// and nothing else about a purchase moves the buyer's Gold. Everything the
/// shortcuts refuse, they refuse on this.
#[test]
fn a_gold_purchase_charges_its_quote_and_moves_no_other_gold() {
    let mut checked = 0;
    for board in played_boards() {
        for pid in majors(&board) {
            let mut rich = board.clone();
            rich.players[pid].gold = 50_000.0;
            let bank = rich.players[pid].gold;
            for action in rich.legal_purchase_actions(pid) {
                if gold_option(&action).is_none() && !matches!(action, Action::BuyPlot { .. }) {
                    continue;
                }
                let quote = rich
                    .purchase_quote(pid, &action)
                    .unwrap_or_else(|| panic!("a listed purchase has a price: {action:?}"));
                assert!(quote >= 0.0, "{action:?} quotes {quote}");
                let mut after = rich.speculative_clone();
                if after.apply(pid, &action).is_err() {
                    continue;
                }
                assert_eq!(
                    after.players[pid].gold.to_bits(),
                    (bank - quote).to_bits(),
                    "turn {} seat {pid}: {action:?} quoted {quote} of {bank} and left {}",
                    board.turn,
                    after.players[pid].gold
                );
                checked += 1;
            }
        }
    }
    assert!(checked >= 30, "fixture: only {checked} purchases were made");
}

/// Over real boards and a spread of treasuries and reserves, the scorer that
/// reads the price first returns what the full scorer returns: the same
/// refusals and, for what survives, the same score to the bit.
#[test]
fn the_price_first_refusal_agrees_with_the_full_score() {
    let mut ai = AdvancedAi::new();
    ai.enable_site_deployment();
    let (mut priced_out, mut scored, mut compared) = (0, 0, 0);
    for board in played_boards() {
        for pid in majors(&board) {
            for bank in [60.0, 180.0, 420.0, 950.0, 2_600.0] {
                let mut game = board.clone();
                game.players[pid].gold = bank;
                let plan = ai.assess(&game, pid);
                let counts = ai.counts(&game, pid);
                let options: Vec<(Action, u32, Item)> = ai
                    .legal_purchase_actions(&game, pid)
                    .into_iter()
                    .filter_map(|action| {
                        gold_option(&action).map(|(city, item)| (action, city, item))
                    })
                    .collect();
                for reserve in [0.0, 75.0, 300.0, 800.0] {
                    let context = PurchaseScoreContext {
                        g: &game,
                        pid,
                        plan: &plan,
                        counts: &counts,
                        bank,
                        reserve,
                    };
                    let _air_surge = ai.open_air_surge_status_frame();
                    let _memo = game.query_memo();
                    for (action, city, item) in &options {
                        let quick = ai.gold_purchase_score(context, action, *city, item);
                        let full = ai.gold_purchase_score_in_full(context, action, *city, item);
                        assert_eq!(
                            quick.map(f64::to_bits),
                            full.map(f64::to_bits),
                            "turn {} seat {pid}, {bank} Gold over {reserve}: {action:?}",
                            board.turn
                        );
                        compared += 1;
                        scored += usize::from(full.is_some());
                        priced_out += usize::from(
                            game.purchase_quote(pid, action)
                                .is_some_and(|price| bank - price + f64::EPSILON < reserve),
                        );
                    }
                }
            }
        }
    }
    assert!(compared >= 200, "fixture: only {compared} comparisons");
    assert!(priced_out > 0, "fixture: nothing was ever priced out");
    assert!(scored > 0, "fixture: nothing was ever worth buying");
    assert!(priced_out < compared, "fixture: everything was priced out");
}
