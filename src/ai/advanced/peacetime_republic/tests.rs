use super::*;
use crate::ai::advanced::GrandStrategy;

/// Two majors at turn 45: we hold Political Philosophy and run Chiefdom, the
/// window between the first tier-one government and Divine Right.
fn political_philosophy(gene: bool) -> (Game, AdvancedAi) {
    let mut g = Game::new_full(2, 18, 10, 41_045, 200, 0, false);
    g.turn = 45;
    g.players[0].government = Some("chiefdom".to_string());
    g.players[0]
        .civics
        .insert(crate::name!("political_philosophy"));
    let mut ai = AdvancedAi::new();
    if gene {
        ai.enable_peacetime_classical_republic();
    }
    (g, ai)
}

#[test]
fn without_the_gene_the_conquest_lane_takes_oligarchy() {
    let (mut g, ai) = political_philosophy(false);
    ai.strategic_government(&mut g, 0, GrandStrategy::Conquest);
    assert_eq!(g.players[0].government.as_deref(), Some("oligarchy"));
}

#[test]
fn at_peace_the_gene_takes_the_classical_republic() {
    let (mut g, ai) = political_philosophy(true);
    ai.strategic_government(&mut g, 0, GrandStrategy::Conquest);
    assert_eq!(
        g.players[0].government.as_deref(),
        Some("classical_republic")
    );
}

#[test]
fn at_war_with_a_major_the_gene_keeps_oligarchy() {
    let (mut g, ai) = political_philosophy(true);
    g.at_war.insert((0, 1));
    ai.strategic_government(&mut g, 0, GrandStrategy::Conquest);
    assert_eq!(g.players[0].government.as_deref(), Some("oligarchy"));
}

#[test]
fn a_republic_at_war_switches_to_oligarchy_and_stays_there() {
    let (mut g, ai) = political_philosophy(true);
    ai.strategic_government(&mut g, 0, GrandStrategy::Conquest);
    assert_eq!(
        g.players[0].government.as_deref(),
        Some("classical_republic")
    );
    g.at_war.insert((0, 1));
    ai.strategic_government(&mut g, 0, GrandStrategy::Conquest);
    assert_eq!(g.players[0].government.as_deref(), Some("oligarchy"));
    // Peace again: Oligarchy is already ours, and a return to the Republic
    // would be an equal-capacity repeat.
    g.at_war.clear();
    ai.strategic_government(&mut g, 0, GrandStrategy::Conquest);
    assert_eq!(g.players[0].government.as_deref(), Some("oligarchy"));
}

#[test]
fn divine_right_still_takes_monarchy_over_the_republic() {
    let (mut g, ai) = political_philosophy(true);
    ai.strategic_government(&mut g, 0, GrandStrategy::Conquest);
    g.players[0].civics.insert(crate::name!("divine_right"));
    ai.strategic_government(&mut g, 0, GrandStrategy::Conquest);
    assert_eq!(g.players[0].government.as_deref(), Some("monarchy"));
}
