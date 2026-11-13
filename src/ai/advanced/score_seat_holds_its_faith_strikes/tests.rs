//! Focused tests for `score-seat-holds-its-faith-strikes`.
//!
//! The board is `faith_strike`'s: our city at (2, 8) leaning to rival 1's
//! Islam, three of our Spearmen beside three of its Missionaries, at peace.

use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::Ai;
use crate::reasoning::Journal;

fn fixture(target: VictoryTarget) -> (Game, AdvancedAi) {
    let mut g = Game::new_full(4, 40, 24, 372101, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let home = g.found_city_for(0, (2, 8), None);
    g.found_city_for(1, (30, 8), None);
    g.found_city_for(2, (18, 18), None);
    g.players[1].religion = Some("Islam".into());
    for rival in 1..4 {
        g.record_contact(0, rival);
    }
    g.at_war.clear();
    g.current = 0;
    g.turn = 83;
    g.players[0].gold = 10000.0;
    g.players[0].gold_per_turn = 10.0;
    // A city of ours leans to Islam, so the faith matters to any seat.
    let pressure = &mut g.cities.get_mut(&home).unwrap().pressure;
    pressure.insert("Buddhism".into(), 100.0);
    pressure.insert("Islam".into(), 70.0);
    for at in [(4, 8), (5, 9), (6, 8)] {
        g.spawn_test_unit("spearman", 0, at);
    }
    for at in [(4, 9), (6, 9), (7, 8)] {
        let uid = g.spawn_test_unit("missionary", 1, at);
        g.units.get_mut(&uid).unwrap().religion = Some("Islam".into());
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
    ai.faith_strike_wipes_the_spreaders = true;
    ai.victory_target = Some(target);
    (g, ai)
}

/// Islam holds every other living major: we are its last holdout.
fn at_match_point(g: &mut Game) {
    g.found_city_for(3, (30, 18), None);
    for pid in [1, 2, 3] {
        for cid in g.player_city_ids(pid) {
            let pressure = &mut g.cities.get_mut(&cid).unwrap().pressure;
            pressure.clear();
            pressure.insert("Islam".into(), 100.0);
        }
    }
}

#[test]
fn the_gene_ships_off() {
    opt_in_off_in_both_controllers("score-seat-holds-its-faith-strikes", |ai| {
        ai.score_seat_holds_its_faith_strikes
    });
}

#[test]
fn off_a_score_seat_strikes_as_before() {
    let (mut g, mut ai) = fixture(VictoryTarget::Score);
    assert!(!ai.score_seat_holds_the_strike(&g, 0, 1));
    assert!(ai.faith_strike_opening(&mut g, 0));
    assert!(g.is_at_war(0, 1));
}

#[test]
fn on_a_score_seat_holds_the_strike_and_says_why() {
    let (mut g, mut ai) = fixture(VictoryTarget::Score);
    ai.enable_score_seat_holds_its_faith_strikes();
    let journal = Journal::recording();
    ai.attach_journal(journal.handle());
    assert!(!ai.faith_strike_opening(&mut g, 0));
    assert!(!g.is_at_war(0, 1), "the strike is held");
    assert_eq!(
        g.units.values().filter(|u| u.owner == 1).count(),
        3,
        "no spreader is condemned"
    );
    // A second turn's hold inside the window writes no second line.
    assert!(!ai.faith_strike_opening(&mut g, 0));
    let civ = g.players[1].civ.clone();
    let lines: Vec<String> = journal
        .since(0)
        .thoughts
        .iter()
        .filter(|t| t.headline == format!("Holding the faith strike on {civ}"))
        .map(|t| t.detail.clone())
        .collect();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].starts_with("score-seat-holds-its-faith-strikes: its faith holds"),
        "{lines:?}"
    );
    assert!(lines[0].contains("of 4 living majors"), "{lines:?}");
}

#[test]
fn a_faith_at_match_point_is_still_struck() {
    let (mut g, mut ai) = fixture(VictoryTarget::Score);
    at_match_point(&mut g);
    ai.enable_score_seat_holds_its_faith_strikes();
    assert!(AdvancedAi::we_are_the_last_holdout(&g, 0, 1));
    assert!(!ai.score_seat_holds_the_strike(&g, 0, 1));
    assert!(ai.faith_strike_opening(&mut g, 0));
    assert!(g.is_at_war(0, 1));
}

#[test]
fn another_target_strikes_as_before() {
    for target in [VictoryTarget::Culture, VictoryTarget::Religion] {
        let (mut g, mut ai) = fixture(target);
        ai.enable_score_seat_holds_its_faith_strikes();
        assert!(!ai.score_seat_holds_the_strike(&g, 0, 1), "{target:?}");
        assert!(ai.faith_strike_opening(&mut g, 0), "{target:?}");
        assert!(g.is_at_war(0, 1), "{target:?}");
    }
}
