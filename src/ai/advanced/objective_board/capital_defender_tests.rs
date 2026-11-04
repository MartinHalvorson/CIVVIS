//! `capital-keeps-its-defender`: the capital's Defend row ranks first.

use super::tests::{at, flat_board};
use super::*;

fn row(kind: ObjectiveKind, key: ObjectiveKey, value: f64, deadline: Option<u32>) -> Objective {
    Objective {
        kind,
        key,
        at: (0, 0),
        value,
        requirement: ForceNeed::default(),
        deadline,
        state: RowState::Open,
        depends_on: None,
        land: true,
        sea: false,
        label: format!("{key:?}"),
        urgent: false,
    }
}

/// Our capital and a second city, each with a Defend row; the second under
/// fire (urgent, ranked first by `city_under_fire_first`), an Escort above
/// the capital by score, and the capital's Relieve behind its Defend.
fn board() -> (Game, u32, u32, Vec<Objective>) {
    let g = flat_board(17, &[at(6, 8), at(30, 8)], true);
    let capital = g.city_at(at(6, 8)).expect("our capital");
    assert!(g.cities[&capital].is_capital && g.cities[&capital].owner == 0);
    // Any other city id stands in for the secondary city: the gene keys on
    // the capital, not on the other row's city.
    let second = capital + 1_000;
    let mut under_fire = row(
        ObjectiveKind::Defend,
        ObjectiveKey::Defend(second),
        40.0,
        Some(1),
    );
    under_fire.urgent = true;
    let escort = row(
        ObjectiveKind::Escort,
        ObjectiveKey::Escort(77),
        280.0,
        Some(1),
    );
    let defend = row(
        ObjectiveKind::Defend,
        ObjectiveKey::Defend(capital),
        60.0,
        Some(2),
    );
    let mut relieve = row(
        ObjectiveKind::Relieve,
        ObjectiveKey::Relieve(capital),
        60.0,
        Some(2),
    );
    relieve.depends_on = Some(ObjectiveKey::Defend(capital));
    (
        g,
        capital,
        second,
        vec![under_fire, escort, defend, relieve],
    )
}

#[test]
fn capital_keeps_its_defender_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(!ai.capital_keeps_its_defender, "an opt-in ships off");
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "capital_keeps_its_defender"));
    super::super::test_support::opt_in_off_in_both_controllers(
        "capital-keeps-its-defender",
        |ai| ai.capital_keeps_its_defender,
    );
}

#[test]
fn the_capitals_defense_ranks_first_and_urgent() {
    let (g, capital, second, mut rows) = board();
    let mut ai = AdvancedAi::new();
    ai.enable_capital_keeps_its_defender();
    ai.capital_defend_first(&g, 0, &mut rows);
    assert_eq!(rows[0].key, ObjectiveKey::Defend(capital));
    assert!(rows[0].urgent);
    assert_eq!(
        rows.iter().map(|row| row.key).collect::<Vec<_>>(),
        vec![
            ObjectiveKey::Defend(capital),
            ObjectiveKey::Defend(second),
            ObjectiveKey::Escort(77),
            ObjectiveKey::Relieve(capital),
        ],
        "the rest keep their order; the Relieve still follows its Defend"
    );
}

#[test]
fn no_capital_row_leaves_the_board_as_it_was() {
    let (g, capital, _, rows) = board();
    let mut without: Vec<Objective> = rows
        .iter()
        .filter(|row| row.key != ObjectiveKey::Defend(capital))
        .cloned()
        .collect();
    let before = without.clone();
    let mut ai = AdvancedAi::new();
    ai.enable_capital_keeps_its_defender();
    ai.capital_defend_first(&g, 0, &mut without);
    assert_eq!(without, before);
}
