//! `deck-follows-the-queue`: the queue-card signature and the off-cadence
//! deck review it triggers.
use super::deck_queue::{is_queue_card, queue_card_signature};
use super::*;
use crate::rules::Yields;
use std::sync::Arc;

/// One founded city under Chiefdom (one Military and one Economic slot) with
/// Agoge, Colonization, Ilkum and the Code of Laws cards unlocked, twelve
/// Production a turn, and `head` at the front of its queue.
fn city_building(head: Item) -> (Game, u32) {
    let mut g = Game::new_full(2, 24, 16, 91_781, 250, 0, false);
    g.current = 0;
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = g.player_city_ids(0)[0];
    g.players[0].civics.extend(
        ["code_of_laws", "craftsmanship", "early_empire"]
            .into_iter()
            .map(Name::new),
    );
    g.players[0].government = Some("chiefdom".to_string());
    Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        cid,
        Yields {
            production: 10.0,
            ..Default::default()
        },
    );
    g.cities.get_mut(&cid).unwrap().queue = vec![head];
    (g, cid)
}

fn settler() -> Item {
    Item::Unit {
        unit: crate::name!("settler"),
    }
}

fn warrior() -> Item {
    Item::Unit {
        unit: crate::name!("warrior"),
    }
}

fn monument() -> Item {
    Item::Building {
        building: crate::name!("monument"),
    }
}

fn names(cards: &[&str]) -> BTreeSet<Name> {
    cards.iter().map(|card| Name::new(card)).collect()
}

#[test]
fn queue_cards_are_the_pure_production_cards() {
    let (g, _) = city_building(monument());
    for card in [
        "agoge",
        "feudal_contract",
        "maritime_industries",
        "colonization",
        "ilkum",
        "limes",
        "grande_armee",
        "international_waters",
    ] {
        assert!(is_queue_card(&g, Name::new(card)), "{card} is a queue card");
    }
    // Urban Planning is flat Production; Expropriation and Public Works do
    // something besides boost an item; the rest move no production at all.
    for card in [
        "urban_planning",
        "expropriation",
        "public_works",
        "god_king",
        "discipline",
        "conscription",
        "colonial_taxes",
    ] {
        assert!(
            !is_queue_card(&g, Name::new(card)),
            "{card} is not a queue card"
        );
    }
}

#[test]
fn the_signature_names_the_card_the_head_wants() {
    let (mut g, cid) = city_building(settler());
    assert!(
        g.city_yields(cid).production >= 10.0,
        "fixture: the city builds"
    );
    g.players[0].policies.insert(Name::new("agoge"));
    let held = g.players[0].policies.clone();

    assert_eq!(
        queue_card_signature(&mut g, 0),
        names(&["colonization"]),
        "a Settler wants Colonization; a held Agoge boosts nothing"
    );
    assert_eq!(
        g.players[0].policies, held,
        "the counterfactual puts the deck back"
    );

    g.cities.get_mut(&cid).unwrap().queue = vec![warrior()];
    assert_eq!(
        queue_card_signature(&mut g, 0),
        names(&["agoge"]),
        "a held card that boosts the head is in the signature"
    );

    g.cities.get_mut(&cid).unwrap().queue = vec![monument()];
    assert!(
        queue_card_signature(&mut g, 0).is_empty(),
        "no card boosts a Monument"
    );
}

#[test]
fn a_city_too_small_to_gain_a_production_point_is_ignored() {
    let (mut g, cid) = city_building(settler());
    Arc::make_mut(&mut g.observed_city_yield_adjustments).remove(&cid);
    let production = g.city_yields(cid).production;
    assert!(
        production * 0.5 < super::deck_queue::DECK_QUEUE_MIN_BOOST,
        "fixture: a fresh city makes {production} Production"
    );
    assert!(queue_card_signature(&mut g, 0).is_empty());
}

#[test]
fn the_full_deck_is_reviewed_off_cadence_only_when_forced() {
    let (mut g, _) = city_building(settler());
    g.players[0].policies = names(&["discipline", "urban_planning"]);
    g.turn = POLICY_REVIEW_EVERY + 1;
    let weights = Weights {
        policy_deck: PolicyDeck::Live,
        ..Weights::default()
    };

    let mut stock = g.clone();
    revise_policy_deck(&mut stock, 0, &weights, None, false, false, false);
    assert_eq!(
        stock.players[0].policies,
        names(&["discipline", "urban_planning"]),
        "off the cadence a full deck is left alone"
    );

    revise_policy_deck(&mut g, 0, &weights, None, false, false, true);
    assert!(
        g.players[0].policies.contains(&Name::new("colonization")),
        "a forced review slots the card the Settler wants: {:?}",
        g.players[0].policies
    );
}

#[test]
fn the_gene_asks_for_one_review_per_change() {
    let (mut g, cid) = city_building(settler());
    let mut ai = BasicAi::new();
    assert!(!ai.deck_follows_the_queue, "an opt-in ships off");
    assert!(!ai.deck_queue_review_due(&mut g, 0), "off: never due");
    assert!(
        ai.deck_queue_signature.borrow().is_empty(),
        "off: nothing recorded"
    );

    ai.enable_deck_follows_the_queue();
    assert!(
        ai.deck_queue_review_due(&mut g, 0),
        "the first Settler asks for a review"
    );
    assert!(
        !ai.deck_queue_review_due(&mut g, 0),
        "the same queue does not ask again"
    );

    g.cities.get_mut(&cid).unwrap().queue = vec![monument()];
    assert!(
        ai.deck_queue_review_due(&mut g, 0),
        "the Settler leaving asks again"
    );
    assert!(!ai.deck_queue_review_due(&mut g, 0));

    ai.disable_deck_follows_the_queue();
    assert!(
        ai.deck_queue_signature.borrow().is_empty(),
        "disabling forgets the queue"
    );
}
