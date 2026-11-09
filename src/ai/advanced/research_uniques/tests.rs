use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

/// Four majors, one turn in, our seat playing `civ`.
fn board(civ: &str) -> Game {
    let mut g = Game::new_full(4, 34, 24, 31_031, 250, 0, false);
    g.players[0].civ = civ.to_string();
    g
}

fn stock() -> AdvancedAi {
    AdvancedAi::new()
}

fn armed() -> AdvancedAi {
    let mut ai = stock();
    ai.enable_research_weighs_our_own_uniques();
    ai
}

fn civic(ai: &AdvancedAi, civ: &str, civic: &str) -> f64 {
    ai.civic_value(&board(civ), 0, civic, GrandStrategy::Culture)
}

fn tech(ai: &AdvancedAi, civ: &str, tech: &str) -> f64 {
    ai.tech_value(&board(civ), 0, tech, GrandStrategy::Science)
}

#[test]
fn research_weighs_our_own_uniques_is_opt_in() {
    opt_in_off_in_both_controllers("research-weighs-our-own-uniques", |ai| {
        ai.research_weighs_our_own_uniques
    });
}

#[test]
fn off_a_civic_prices_no_improvement_and_reads_no_civilization() {
    // Reformed Church unlocks only Scotland's Golf Course among improvements.
    assert_eq!(
        civic(&stock(), "Scotland", "reformed_church"),
        civic(&stock(), "Rome", "reformed_church")
    );
    assert_eq!(
        stock().civic_improvement_value(
            &board("Scotland"),
            0,
            "reformed_church",
            GrandStrategy::Culture
        ),
        0.0
    );
}

#[test]
fn scotland_prices_reformed_church_for_its_golf_course() {
    let scotland = civic(&armed(), "Scotland", "reformed_church");
    assert!(
        scotland > civic(&stock(), "Scotland", "reformed_church"),
        "the Golf Course is Scotland's"
    );
    // Rome cannot build one: armed, Reformed Church is worth what it was.
    assert_eq!(
        civic(&armed(), "Rome", "reformed_church"),
        civic(&stock(), "Rome", "reformed_church")
    );
    assert!(scotland > civic(&armed(), "Rome", "reformed_church"));
}

#[test]
fn masonry_is_not_priced_for_another_civilizations_great_wall() {
    assert!(
        tech(&armed(), "Rome", "masonry") < tech(&stock(), "Rome", "masonry"),
        "the Great Wall is China's"
    );
    assert!(
        tech(&armed(), "China", "masonry") > tech(&stock(), "China", "masonry"),
        "and China's own"
    );
}

#[test]
fn theology_is_not_priced_for_other_civilizations_temples() {
    // The Madrasa, Prasat and Stave Church replace the Temple elsewhere.
    assert!(civic(&armed(), "Rome", "theology") < civic(&stock(), "Rome", "theology"));
    assert!(civic(&armed(), "Khmer", "theology") > civic(&armed(), "Rome", "theology"));
}

#[test]
fn a_civ_blind_seat_counts_its_own_unique_but_takes_no_credit() {
    let mut blind = armed();
    blind.civ_blind = true;
    // Its own Great Wall still counts, as it did off; only the +55 is withheld.
    assert_eq!(
        tech(&blind, "China", "masonry"),
        tech(&stock(), "China", "masonry")
    );
    assert_eq!(
        blind.own_unique_unlock_credit(&board("China"), 0, Some("China")),
        0.0
    );
    assert_eq!(
        armed().own_unique_unlock_credit(&board("China"), 0, Some("China")),
        OWN_UNIQUE_UNLOCK_CREDIT
    );
}

#[test]
fn an_unlock_nobody_owns_is_always_counted() {
    for ai in [stock(), armed()] {
        assert!(ai.research_counts_unlock(&board("Rome"), 0, None));
    }
    assert!(stock().research_counts_unlock(&board("Rome"), 0, Some("China")));
    assert!(!armed().research_counts_unlock(&board("Rome"), 0, Some("China")));
    assert!(armed().research_counts_unlock(&board("China"), 0, Some("China")));
}
