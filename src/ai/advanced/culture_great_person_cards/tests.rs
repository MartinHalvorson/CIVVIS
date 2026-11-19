//! `culture-deck-seats-the-great-person-cards`: unit tests.
use super::*;
use crate::ai::advanced::{GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::{Action, Item};
use crate::name::Name;
use crate::rules::Yields;
use std::sync::Arc;

const TAG: &str = "culture-deck-seats-the-great-person-cards";

/// One city, Drama and Poetry and Humanism known, at peace. `government`
/// sets the slots; `cards` is the deck already held.
fn board(government: &str, cards: &[&str], art_museum: bool) -> Game {
    let mut g = Game::new(2, 40, 16, 936_017, 200, 0);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = g.player_city_ids(0)[0];
    if art_museum {
        let pos = g.cities[&cid].pos;
        let city = g.cities.get_mut(&cid).unwrap();
        city.districts.insert(crate::name!("theater_square"), pos);
        city.buildings.push(crate::name!("amphitheater"));
        city.buildings.push(crate::name!("art_museum"));
    }
    let player = &mut g.players[0];
    player.government = Some(government.to_string());
    for civic in [
        "code_of_laws",
        "craftsmanship",
        "military_tradition",
        "recorded_history",
        "political_philosophy",
        "early_empire",
        "drama_poetry",
        "humanism",
    ] {
        player.civics.insert(Name::new(civic));
    }
    for card in cards {
        player.policies.insert(Name::new(card));
    }
    g
}

/// Monarchy: 2 military, 1 economic, 1 diplomatic, 2 wildcard. Natural
/// Philosophy and Maneuver overflow into the two wildcard seats.
fn monarchy(art_museum: bool) -> Game {
    board(
        "monarchy",
        &[
            "discipline",
            "agoge",
            "maneuver",
            "urban_planning",
            "natural_philosophy",
            "charismatic_leader",
        ],
        art_museum,
    )
}

fn seat(target: VictoryTarget, on: bool, g: &Game) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if on {
        ai.enable_culture_deck_seats_the_great_person_cards();
    }
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Culture,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    });
    ai.base.journal = crate::reasoning::Journal::recording();
    ai
}

fn slotted(g: &Game, card: &str) -> bool {
    g.players[0].policies.contains(&Name::new(card))
}

fn gene_lines(ai: &AdvancedAi) -> Vec<String> {
    ai.base
        .journal
        .since(0)
        .thoughts
        .iter()
        .map(|thought| format!("{} | {}", thought.headline, thought.detail))
        .filter(|line| line.contains(TAG))
        .collect()
}

#[test]
fn it_is_an_opt_in_off_by_default() {
    assert!(
        !AdvancedAi::targeting(VictoryTarget::Culture).culture_deck_seats_the_great_person_cards
    );
}

#[test]
fn both_cards_take_the_wildcards_from_cards_the_plan_does_not_want() {
    let mut g = monarchy(false);
    for card in ["literary_tradition", "frescoes"] {
        assert!(g.available_policies(0).contains(&Name::new(card)), "{card}");
    }
    let ai = seat(VictoryTarget::Culture, true, &g);
    assert_eq!(
        ai.culture_great_person_cards(&g, &g.player_city_ids(0)),
        vec!["literary_tradition", "frescoes"]
    );
    ai.strategic_policies(&mut g, 0, GrandStrategy::Culture);
    assert!(
        slotted(&g, "literary_tradition"),
        "{:?}",
        g.players[0].policies
    );
    assert!(slotted(&g, "frescoes"), "{:?}", g.players[0].policies);
    assert_eq!(
        g.players[0].policies.len(),
        6,
        "{:?}",
        g.players[0].policies
    );
    assert!(!gene_lines(&ai).is_empty());

    // Held, both are wanted: a second pass keeps them.
    ai.strategic_policies(&mut g, 0, GrandStrategy::Culture);
    assert!(slotted(&g, "literary_tradition") && slotted(&g, "frescoes"));
}

#[test]
fn off_the_deck_keeps_its_wildcards() {
    let mut g = monarchy(false);
    let ai = seat(VictoryTarget::Culture, false, &g);
    assert!(ai
        .culture_great_person_cards(&g, &g.player_city_ids(0))
        .is_empty());
    ai.strategic_policies(&mut g, 0, GrandStrategy::Culture);
    assert!(!slotted(&g, "literary_tradition"));
    assert!(!slotted(&g, "frescoes"));
    assert!(gene_lines(&ai).is_empty());
}

#[test]
fn one_wildcard_goes_to_frescoes_once_an_art_museum_stands() {
    // Classical Republic: 2 economic, 1 diplomatic, 1 wildcard, which
    // Discipline holds.
    let deck = [
        "urban_planning",
        "natural_philosophy",
        "charismatic_leader",
        "discipline",
    ];
    let mut g = board("classical_republic", &deck, true);
    let ai = seat(VictoryTarget::Culture, true, &g);
    assert_eq!(
        ai.culture_great_person_cards(&g, &g.player_city_ids(0)),
        vec!["frescoes", "literary_tradition"]
    );
    ai.strategic_policies(&mut g, 0, GrandStrategy::Culture);
    assert!(slotted(&g, "frescoes"), "{:?}", g.players[0].policies);
    assert!(!slotted(&g, "literary_tradition"));
    assert!(!slotted(&g, "discipline"));

    // Without a museum Literary Tradition is first and takes the seat.
    let mut g = board("classical_republic", &deck, false);
    let ai = seat(VictoryTarget::Culture, true, &g);
    ai.strategic_policies(&mut g, 0, GrandStrategy::Culture);
    assert!(
        slotted(&g, "literary_tradition"),
        "{:?}",
        g.players[0].policies
    );
    assert!(!slotted(&g, "frescoes"));
}

#[test]
fn another_lane_or_a_released_lane_wants_neither_card() {
    let g = monarchy(true);
    let ids = g.player_city_ids(0);
    assert!(seat(VictoryTarget::Science, true, &g)
        .culture_great_person_cards(&g, &ids)
        .is_empty());
    let mut released = seat(VictoryTarget::Culture, true, &g);
    released.lane_lost = true;
    assert!(released.culture_great_person_cards(&g, &ids).is_empty());
}

#[test]
fn a_production_card_boosting_a_queue_head_keeps_its_seat() {
    // Classical Republic with Colonization overflowing into the wildcard.
    let deck = [
        "urban_planning",
        "natural_philosophy",
        "charismatic_leader",
        "colonization",
    ];
    let settler = || {
        let mut g = board("classical_republic", &deck, false);
        let cid = g.player_city_ids(0)[0];
        Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
            cid,
            Yields {
                production: 10.0,
                ..Default::default()
            },
        );
        g.cities.get_mut(&cid).unwrap().queue = vec![Item::Unit {
            unit: crate::name!("settler"),
        }];
        g
    };
    // A Settler at the head: Colonization stays, Urban Planning makes way.
    let mut g = settler();
    let ai = seat(VictoryTarget::Culture, true, &g);
    ai.strategic_policies(&mut g, 0, GrandStrategy::Culture);
    assert!(
        slotted(&g, "literary_tradition"),
        "{:?}",
        g.players[0].policies
    );
    assert!(slotted(&g, "colonization"), "{:?}", g.players[0].policies);
    assert!(!slotted(&g, "urban_planning"));

    // Nothing it boosts in the queue: Colonization, the oldest unwanted
    // card, is the one that goes.
    let mut g = settler();
    let cid = g.player_city_ids(0)[0];
    g.cities.get_mut(&cid).unwrap().queue = vec![Item::Building {
        building: crate::name!("monument"),
    }];
    let ai = seat(VictoryTarget::Culture, true, &g);
    ai.strategic_policies(&mut g, 0, GrandStrategy::Culture);
    assert!(
        slotted(&g, "literary_tradition"),
        "{:?}",
        g.players[0].policies
    );
    assert!(!slotted(&g, "colonization"), "{:?}", g.players[0].policies);
}
