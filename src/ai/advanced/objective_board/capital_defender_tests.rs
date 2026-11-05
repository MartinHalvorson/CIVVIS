//! `capital-keeps-its-defender`: the capital's Defend row ranks first.

use super::tests::{at, conquest, flat_board, on};
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

#[test]
fn version_two_ships_off_and_turns_version_one_off() {
    let ai = AdvancedAi::new();
    assert!(!ai.capital_keeps_its_defender_2, "an opt-in ships off");
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "capital_keeps_its_defender_2"));
    super::super::test_support::opt_in_off_in_both_controllers(
        "capital-keeps-its-defender-2",
        |ai| ai.capital_keeps_its_defender_2,
    );
    let mut on = AdvancedAi::new();
    on.enable_capital_keeps_its_defender();
    on.enable_capital_keeps_its_defender_2();
    assert_eq!(
        (
            on.capital_keeps_its_defender,
            on.capital_keeps_its_defender_2
        ),
        (false, true),
        "one version of the family plays"
    );
}

/// Our capital and three more founded cities: the first three by id are the
/// early cities, the fourth is not.
fn four_cities() -> (Game, [u32; 4]) {
    let mut g = flat_board(17, &[at(6, 8), at(30, 8)], true);
    let capital = g.city_at(at(6, 8)).expect("our capital");
    let second = g.found_city_for(0, at(6, 20), None);
    let third = g.found_city_for(0, at(12, 4), None);
    let fourth = g.found_city_for(0, at(14, 12), None);
    (g, [capital, second, third, fourth])
}

#[test]
fn the_early_cities_are_the_first_three_we_founded() {
    let (g, [capital, second, third, fourth]) = four_cities();
    let early = early_cities(&g, 0);
    assert_eq!(early, vec![capital, second, third]);
    assert!(!early.contains(&fourth));
}

/// The second city faces three Swordsmen, the capital one Warrior: under
/// version two the second city's Defend row leads, the capital's follows,
/// both urgent, and the fourth city and the Escort keep their order behind.
#[test]
fn the_early_city_most_at_risk_leads_not_the_capital() {
    let (mut g, [capital, second, _third, fourth]) = four_cities();
    super::tests::war(&mut g, 0, 1);
    for row in [19, 21] {
        g.spawn_test_unit("swordsman", 1, at(7, row));
    }
    g.spawn_test_unit("swordsman", 1, at(5, 20));
    assert!(
        g.wdist(at(6, 20), at(6, 8)) > THREAT_RELIEF_RADIUS + 2,
        "apart"
    );
    g.spawn_test_unit("warrior", 1, at(8, 8));
    let mut rows = vec![
        row(
            ObjectiveKind::Escort,
            ObjectiveKey::Escort(77),
            280.0,
            Some(1),
        ),
        row(
            ObjectiveKind::Defend,
            ObjectiveKey::Defend(fourth),
            300.0,
            Some(2),
        ),
        row(
            ObjectiveKind::Defend,
            ObjectiveKey::Defend(capital),
            700.0,
            Some(2),
        ),
        row(
            ObjectiveKind::Defend,
            ObjectiveKey::Defend(second),
            400.0,
            Some(2),
        ),
    ];
    let mut ai = AdvancedAi::new();
    ai.enable_capital_keeps_its_defender_2();
    let visible = ai.battlefront_visibility(&g, 0);
    ai.early_cities_defend_first(&g, 0, &visible, &mut rows);
    assert_eq!(
        rows.iter().map(|row| row.key).collect::<Vec<_>>(),
        vec![
            ObjectiveKey::Defend(second),
            ObjectiveKey::Defend(capital),
            ObjectiveKey::Escort(77),
            ObjectiveKey::Defend(fourth),
        ]
    );
    assert!(rows[0].urgent && rows[1].urgent);
    assert!(!rows[3].urgent, "a later city keeps its own urgency");
}

/// An outclassed Defend force holds its city under version two; a Relieve
/// still marches, and nothing changes with the gene off or on an arena.
#[test]
fn an_outclassed_defence_holds_the_city() {
    let mut ai = AdvancedAi::new();
    let defend = Some(ObjectiveKind::Defend);
    assert!(!ai.defend_holds_when_outclassed(false, defend, 0.1), "off");
    ai.enable_capital_keeps_its_defender_2();
    assert!(ai.defend_holds_when_outclassed(false, defend, 0.1));
    assert!(ai.defend_holds_when_outclassed(false, defend, DEFEND_HOLD_RATIO - 0.01));
    assert!(!ai.defend_holds_when_outclassed(false, defend, DEFEND_HOLD_RATIO));
    assert!(!ai.defend_holds_when_outclassed(false, Some(ObjectiveKind::Relieve), 0.1));
    assert!(
        !ai.defend_holds_when_outclassed(true, defend, 0.1),
        "an arena"
    );
}

#[test]
fn version_three_ships_off_and_turns_the_others_off() {
    let ai = AdvancedAi::new();
    assert!(!ai.capital_keeps_its_defender_3, "an opt-in ships off");
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "capital_keeps_its_defender_3"));
    super::super::test_support::opt_in_off_in_both_controllers(
        "capital-keeps-its-defender-3",
        |ai| ai.capital_keeps_its_defender_3,
    );
    let mut on = AdvancedAi::new();
    on.enable_capital_keeps_its_defender();
    on.enable_capital_keeps_its_defender_2();
    on.enable_capital_keeps_its_defender_3();
    assert_eq!(
        (
            on.capital_keeps_its_defender,
            on.capital_keeps_its_defender_2,
            on.capital_keeps_its_defender_3
        ),
        (false, false, true),
        "one version of the family plays"
    );
}

/// A rival at peace, far stronger than us, parks two Swordsmen two tiles
/// from our second city. Version two sees nothing (no war); version three
/// reads the parked strength and gives the city its Defend row.
#[test]
fn a_stronger_major_parked_at_peace_raises_the_early_citys_row() {
    let (mut g, [_capital, second, _third, _fourth]) = four_cities();
    for col in 26..32 {
        g.spawn_test_unit("swordsman", 1, at(col, 12));
    }
    let near = [at(8, 20), at(8, 21)];
    for pos in near {
        g.spawn_test_unit("swordsman", 1, pos);
    }
    let second_pos = g.cities[&second].pos;
    assert!(near
        .iter()
        .all(|pos| g.wdist(*pos, second_pos) <= SURPRISE_TILES));
    assert!(!g.is_at_war(0, 1), "at peace");
    let plan = conquest(&g, None);
    let row_for = |ai: &mut AdvancedAi, g: &Game| {
        ai.rebuild_force_groups(g, 0, &plan);
        ai.objective_board()
            .rows
            .iter()
            .any(|row| row.key == ObjectiveKey::Defend(second))
    };
    let mut v3 = on();
    v3.enable_capital_keeps_its_defender_3();
    let (close, relief) = v3.surprise_strength_near(&g, 0, second_pos);
    assert!(
        close > 0.0 && relief >= close,
        "the parked soldiers read: {close} / {relief}"
    );
    assert!(
        row_for(&mut v3, &g),
        "version three raises Defend for the second city"
    );
    let mut v2 = on();
    v2.enable_capital_keeps_its_defender_2();
    assert!(!row_for(&mut v2, &g), "version two counts only a war");
}

/// `outclassed-defenders-hold-the-city` plays version two's hold alone: the
/// same posture, and no early-city ranking.
#[test]
fn the_hold_plays_alone_without_the_early_city_ranking() {
    let mut ai = AdvancedAi::new();
    assert!(
        !ai.outclassed_defenders_hold_the_city,
        "an opt-in ships off"
    );
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "outclassed_defenders_hold_the_city"));
    super::super::test_support::opt_in_off_in_both_controllers(
        "outclassed-defenders-hold-the-city",
        |ai| ai.outclassed_defenders_hold_the_city,
    );
    let defend = Some(ObjectiveKind::Defend);
    ai.enable_outclassed_defenders_hold_the_city();
    assert!(!ai.capital_keeps_its_defender_2 && !ai.capital_keeps_its_defender_3);
    assert!(ai.defend_holds_when_outclassed(false, defend, 0.48));
    assert!(!ai.defend_holds_when_outclassed(false, defend, DEFEND_HOLD_RATIO));
    assert!(!ai.defend_holds_when_outclassed(false, Some(ObjectiveKind::Relieve), 0.1));
    assert!(
        !ai.defend_holds_when_outclassed(true, defend, 0.1),
        "an arena"
    );
}
