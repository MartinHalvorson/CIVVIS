use super::*;
use crate::ai::advanced::{GrandStrategy, VictoryTarget};
use crate::ai::Ai;
use crate::setup::GameSpeed;
use std::collections::BTreeSet;
use std::sync::Arc;

/// Two majors who have met, both holding Early Empire and Gold, on Online
/// speed, at `turn`; the seat fields `ours` military and the rival `theirs`.
fn two_traders(turn: u32, ours: f64, theirs: f64) -> Game {
    let mut game = Game::new_full(2, 18, 10, 79_004, 200, 0, false);
    game.game_speed = GameSpeed::Online;
    game.record_contact(0, 1);
    game.turn = turn;
    for pid in 0..2 {
        game.players[pid].gold = 1_000.0;
        game.players[pid]
            .civics
            .insert(crate::name!("early_empire"));
    }
    let power = Arc::make_mut(&mut game.observed_military_power);
    power.insert(0, ours);
    power.insert(1, theirs);
    game
}

fn seat(target: VictoryTarget, enabled: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if enabled {
        ai.enable_passage_waits_past_the_opening();
    }
    ai
}

#[test]
fn the_hold_reads_the_opening_then_the_rival_military() {
    let ai = seat(VictoryTarget::Culture, true);
    assert_eq!(
        ai.passage_hold(&two_traders(79, 100.0, 100.0), 0, 1),
        Some(PassageHold::Opening { until: 80 })
    );
    assert_eq!(ai.passage_hold(&two_traders(80, 100.0, 100.0), 0, 1), None);
    assert_eq!(
        ai.passage_hold(&two_traders(80, 100.0, 150.0), 0, 1),
        None,
        "exactly 1.5x is not more than 1.5x"
    );
    assert_eq!(
        ai.passage_hold(&two_traders(120, 100.0, 151.0), 0, 1),
        Some(PassageHold::Stronger {
            theirs: 151.0,
            ours: 100.0
        })
    );
    // The opening is the same point of the game at every speed.
    let mut standard = two_traders(120, 100.0, 100.0);
    standard.game_speed = GameSpeed::Standard;
    assert_eq!(
        ai.passage_hold(&standard, 0, 1),
        Some(PassageHold::Opening { until: 160 })
    );
    // Off, nothing is held.
    let off = seat(VictoryTarget::Culture, false);
    assert_eq!(off.passage_hold(&two_traders(6, 100.0, 900.0), 0, 1), None);
    assert!(off
        .passage_holds(&two_traders(6, 100.0, 900.0), 0)
        .is_empty());
}

#[test]
fn the_culture_seat_buys_no_passage_in_the_opening() {
    // Off: the Culture objective's trade slot buys the rival's passage.
    let mut off = two_traders(6, 100.0, 100.0);
    seat(VictoryTarget::Culture, false).strategic_bilateral_trade(
        &mut off,
        0,
        None,
        GrandStrategy::Expansion,
    );
    assert!(
        off.has_open_borders(0, 1),
        "the slot buys passage with the gene off"
    );

    let mut on = two_traders(6, 100.0, 100.0);
    seat(VictoryTarget::Culture, true).strategic_bilateral_trade(
        &mut on,
        0,
        None,
        GrandStrategy::Expansion,
    );
    assert!(!on.has_open_borders(0, 1), "no passage bought at turn 6");
}

#[test]
fn after_the_opening_only_a_rival_that_outguns_us_is_refused() {
    // Turn 84 is the first six-turn trade slot of seat 0 past turn 80.
    let mut even = two_traders(84, 100.0, 120.0);
    seat(VictoryTarget::Culture, true).strategic_bilateral_trade(
        &mut even,
        0,
        None,
        GrandStrategy::Expansion,
    );
    assert!(even.has_open_borders(0, 1), "a rival at 1.2x sells passage");

    let mut outgunned = two_traders(84, 100.0, 300.0);
    seat(VictoryTarget::Culture, true).strategic_bilateral_trade(
        &mut outgunned,
        0,
        None,
        GrandStrategy::Expansion,
    );
    assert!(
        !outgunned.has_open_borders(0, 1),
        "a rival at 3x is not paid for passage"
    );
}

#[test]
fn every_other_objective_trades_without_the_passage_in_the_opening() {
    for target in [VictoryTarget::Science, VictoryTarget::Domination] {
        let mut on = two_traders(6, 100.0, 100.0);
        seat(target, true).strategic_bilateral_trade(&mut on, 0, None, GrandStrategy::Expansion);
        assert!(
            !on.has_open_borders(0, 1),
            "{target:?}: no passage bought at turn 6"
        );
    }
}

#[test]
fn every_border_open_waits_for_the_opening_too() {
    let culture = |enabled: bool| {
        let mut ai = seat(VictoryTarget::Culture, enabled);
        ai.enable_culture_keeps_every_border_open();
        ai
    };
    let tourist_board = || {
        // Off the six-turn slot, so only the lump buy can buy passage.
        let mut game = two_traders(7, 100.0, 100.0);
        Arc::make_mut(&mut game.observed_tourism_per_turn).insert(0, 60.0);
        game
    };
    let mut off = tourist_board();
    assert_eq!(
        culture(false).keep_every_border_open(
            &mut off,
            0,
            None,
            &BTreeSet::new(),
            &BTreeSet::new()
        ),
        1
    );
    let mut on = tourist_board();
    assert_eq!(
        culture(true).keep_every_border_open(&mut on, 0, None, &BTreeSet::new(), &BTreeSet::new()),
        0
    );
    assert!(!on.has_open_borders(0, 1));
}

#[test]
fn a_withheld_purchase_journals_the_rival_the_turn_and_the_reason() {
    let mut game = two_traders(6, 100.0, 100.0);
    let mut ai = seat(VictoryTarget::Culture, true);
    let journal = crate::reasoning::Journal::recording();
    ai.attach_journal(journal.handle());
    ai.strategic_bilateral_trade(&mut game, 0, None, GrandStrategy::Expansion);

    let civ = game.players[1].civ.to_string();
    let lines: Vec<(String, String)> = journal
        .since(0)
        .thoughts
        .iter()
        .filter(|thought| thought.headline.starts_with("Withholding passage from"))
        .map(|thought| (thought.headline.clone(), thought.detail.clone()))
        .collect();
    assert_eq!(
        lines,
        vec![(
            format!("Withholding passage from {civ}"),
            "passage-waits-past-the-opening withheld the trade slot purchase: turn 6 is inside \
             the opening, which ends at turn 80"
                .to_string()
        )]
    );

    // The bridge's lane journals only on the turns it would ask.
    let quiet = crate::reasoning::Journal::recording();
    ai.attach_journal(quiet.handle());
    assert!(ai.passage_withheld_on_the_bridge(&game, 0, 1, false));
    assert!(quiet.since(0).thoughts.is_empty());
    let outgunned = two_traders(91, 100.0, 300.0);
    assert!(ai.passage_withheld_on_the_bridge(&outgunned, 0, 1, true));
    let bridge: Vec<String> = quiet
        .since(0)
        .thoughts
        .iter()
        .map(|thought| thought.detail.clone())
        .collect();
    assert_eq!(
        bridge,
        vec![
            "passage-waits-past-the-opening withheld the passage lane purchase: turn 91: their \
              military 300 is 3.0 times our 100, over 1.5"
                .to_string()
        ]
    );
    assert!(!ai.passage_withheld_on_the_bridge(&two_traders(91, 100.0, 100.0), 0, 1, true));
}
