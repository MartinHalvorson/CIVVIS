//! `medina-quarter-houses-the-districts`: unit tests.
use super::*;
use crate::ai::advanced::{GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::Action;
use crate::name::Name;

/// Monarchy (2 military, 1 economic, 1 diplomatic, 2 wildcard) with Medieval
/// Faires adopted, `cities` cities of ours, and a Conquest plan. The first
/// `qualifying` cities hold three specialty districts and are housing-bound.
fn empire(cities: usize, qualifying: usize) -> (Game, AdvancedAi) {
    let mut game = Game::new(2, 40, 16, 936_017, 200, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|id| game.units[id].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let capital = game.player_city_ids(0)[0];
    let origin = game.cities[&capital].pos;
    let mut ids = vec![capital];
    for step in 1..cities {
        let pos = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| {
                let tile = &game.map.tiles[pos];
                !game.rules.is_water(tile)
                    && tile.terrain != "mountain"
                    && game
                        .cities
                        .values()
                        .all(|city| game.wdist(city.pos, *pos) >= 5)
                    && game.wdist(origin, *pos) <= 6 + 4 * step as i32
            })
            .min()
            .expect("room for another city");
        ids.push(game.found_city_for(0, pos, None));
    }
    for (index, cid) in ids.iter().enumerate() {
        let city = game.cities.get_mut(cid).unwrap();
        if index < qualifying {
            for district in ["campus", "commercial_hub", "theater_square"] {
                city.districts.insert(Name::new(district), city.pos);
            }
            city.pop = 20;
        } else {
            city.pop = 1;
        }
    }
    game.players[0].government = Some("monarchy".to_string());
    game.players[0].civics.extend([
        crate::name!("code_of_laws"),
        crate::name!("political_philosophy"),
        crate::name!("divine_right"),
        crate::name!("feudalism"),
        crate::name!("medieval_faires"),
    ]);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    });
    (game, ai)
}

/// Fill every Monarchy slot: two Economic cards and Urban Planning (a
/// production card) on the economic and wildcard seats, Charismatic Leader
/// on the diplomatic seat, two military cards.
fn full_deck(game: &mut Game) {
    for card in [
        "natural_philosophy",
        "town_charters",
        "urban_planning",
        "charismatic_leader",
        "discipline",
        "survey",
    ] {
        game.players[0].policies.insert(Name::new(card));
    }
}

/// The held cards the housing card may displace, weakest first, with
/// `protected` standing in for the deck's own protections.
fn victims(game: &Game, desired: &[&str], protected: &dyn Fn(&str) -> bool) -> Vec<Name> {
    let mut cards: Vec<Name> = game.players[0]
        .policies
        .iter()
        .filter(|card| {
            AdvancedAi::medina_quarter_may_displace(game, card.as_str())
                && !protected(card.as_str())
        })
        .cloned()
        .collect();
    AdvancedAi::medina_quarter_weakest_first(game, desired, &mut cards);
    cards
}

fn slotted(game: &Game, card: &str) -> bool {
    game.players[0].policies.contains(&Name::new(card))
}

fn medina_lines(ai: &AdvancedAi) -> Vec<String> {
    ai.base
        .journal
        .since(0)
        .thoughts
        .iter()
        .filter(|thought| thought.headline.starts_with("Slotted medina quarter"))
        .map(|thought| format!("{} | {}", thought.headline, thought.detail))
        .collect()
}

#[test]
fn it_is_an_opt_in_off_by_default() {
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(!ai.medina_quarter_houses_the_districts);
}

#[test]
fn off_qualifying_cities_leave_the_deck_alone() {
    let (mut game, ai) = empire(2, 2);
    full_deck(&mut game);
    assert_eq!(AdvancedAi::medina_quarter_qualifying_cities(&game, 0), 2);
    assert!(game
        .available_policies(0)
        .contains(&crate::name!("medina_quarter")));
    assert_eq!(ai.medina_quarter_wanted(&game, 0), None);
    let before = game.players[0].policies.clone();
    ai.strategic_policies(&mut game, 0, GrandStrategy::Conquest);
    assert!(!slotted(&game, "medina_quarter"));
    assert!(before.contains(&crate::name!("natural_philosophy")));
}

#[test]
fn on_two_housing_bound_district_cities_displace_the_weakest_economic_card() {
    let (mut game, mut ai) = empire(2, 2);
    full_deck(&mut game);
    ai.enable_medina_quarter_houses_the_districts();
    ai.base.journal = crate::reasoning::Journal::recording();
    assert_eq!(ai.medina_quarter_wanted(&game, 0), Some(2));
    ai.strategic_policies(&mut game, 0, GrandStrategy::Conquest);
    assert!(
        slotted(&game, "medina_quarter"),
        "{:?}",
        game.players[0].policies
    );
    // The production card and the other slot types are untouched; exactly one
    // of the two ordinary Economic cards made way.
    assert!(slotted(&game, "urban_planning"));
    assert!(slotted(&game, "charismatic_leader"));
    let kept = ["natural_philosophy", "town_charters"]
        .iter()
        .filter(|card| slotted(&game, card))
        .count();
    assert_eq!(kept, 1, "{:?}", game.players[0].policies);
    let lines = medina_lines(&ai);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].starts_with("Slotted medina quarter over "),
        "{lines:?}"
    );
    assert!(
        lines[0].contains("2 housing-limited cities with 3 or more specialty districts"),
        "{lines:?}"
    );

    // Held, the card reads its cities without its own +2 and stays wanted.
    assert_eq!(AdvancedAi::medina_quarter_qualifying_cities(&game, 0), 2);
    ai.strategic_policies(&mut game, 0, GrandStrategy::Conquest);
    assert!(slotted(&game, "medina_quarter"));
}

#[test]
fn on_one_qualifying_city_does_not_bring_the_card_but_keeps_a_held_one() {
    let (mut game, mut ai) = empire(2, 1);
    full_deck(&mut game);
    ai.enable_medina_quarter_houses_the_districts();
    assert_eq!(AdvancedAi::medina_quarter_qualifying_cities(&game, 0), 1);
    assert_eq!(ai.medina_quarter_wanted(&game, 0), None);
    ai.strategic_policies(&mut game, 0, GrandStrategy::Conquest);
    assert!(!slotted(&game, "medina_quarter"));

    // Already held, one qualifying city is enough to keep it.
    game.players[0]
        .policies
        .remove(&crate::name!("town_charters"));
    game.players[0]
        .policies
        .insert(crate::name!("medina_quarter"));
    assert_eq!(ai.medina_quarter_wanted(&game, 0), Some(1));
}

#[test]
fn on_no_qualifying_city_leaves_the_deck_alone() {
    let (mut game, mut ai) = empire(2, 0);
    full_deck(&mut game);
    ai.enable_medina_quarter_houses_the_districts();
    assert_eq!(AdvancedAi::medina_quarter_qualifying_cities(&game, 0), 0);
    assert_eq!(ai.medina_quarter_wanted(&game, 0), None);
    ai.strategic_policies(&mut game, 0, GrandStrategy::Conquest);
    assert!(!slotted(&game, "medina_quarter"));
    assert!(slotted(&game, "natural_philosophy"));
    assert!(slotted(&game, "town_charters"));
}

#[test]
fn on_a_city_with_housing_room_does_not_qualify() {
    let (mut game, mut ai) = empire(2, 2);
    for cid in game.player_city_ids(0) {
        game.cities.get_mut(&cid).unwrap().pop = 1;
    }
    ai.enable_medina_quarter_houses_the_districts();
    assert_eq!(AdvancedAi::medina_quarter_qualifying_cities(&game, 0), 0);
    assert_eq!(ai.medina_quarter_wanted(&game, 0), None);
}

#[test]
fn on_only_production_cards_to_spare_it_waits() {
    let (mut game, mut ai) = empire(2, 2);
    for card in [
        "urban_planning",
        "serfdom",
        "ilkum",
        "charismatic_leader",
        "discipline",
        "survey",
    ] {
        game.players[0].policies.insert(Name::new(card));
    }
    ai.enable_medina_quarter_houses_the_districts();
    assert_eq!(ai.medina_quarter_wanted(&game, 0), Some(2));
    assert!(
        victims(&game, &[], &|_| false).is_empty(),
        "production, Builder and Settler cards are spared"
    );
    ai.strategic_policies(&mut game, 0, GrandStrategy::Conquest);
    assert!(
        !slotted(&game, "medina_quarter"),
        "{:?}",
        game.players[0].policies
    );
    assert!(slotted(&game, "urban_planning"));
}

#[test]
fn on_a_free_slot_it_simply_slots() {
    let (mut game, mut ai) = empire(2, 2);
    game.players[0]
        .policies
        .insert(crate::name!("natural_philosophy"));
    ai.enable_medina_quarter_houses_the_districts();
    ai.base.journal = crate::reasoning::Journal::recording();
    ai.strategic_policies(&mut game, 0, GrandStrategy::Conquest);
    assert!(slotted(&game, "medina_quarter"));
    let lines = medina_lines(&ai);
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("Slotted medina quarter | ")),
        "{lines:?}"
    );
}

#[test]
fn insulae_makes_way_first_and_nothing_retires_medina_quarter() {
    // No shipped row names Medina Quarter as its successor's predecessor, so
    // the deck's successor retirement cannot drop it.
    let game = empire(1, 1).0;
    assert!(game
        .rules
        .policies
        .values()
        .all(|policy| !policy.replaces.iter().any(|card| *card == "medina_quarter")));

    // A slotted Insulae is an ordinary Economic card and goes first.
    let (mut game, _) = empire(2, 2);
    for card in ["insulae", "natural_philosophy", "urban_planning"] {
        game.players[0].policies.insert(Name::new(card));
    }
    let desired = ["natural_philosophy"];
    let order = victims(&game, &desired, &|_| false);
    assert_eq!(
        order,
        vec![crate::name!("insulae"), crate::name!("natural_philosophy")],
        "an unwanted card before a wanted one; Urban Planning spared"
    );
    // A protected card is never offered up.
    let protected = victims(&game, &desired, &|card| card == "insulae");
    assert_eq!(protected, vec![crate::name!("natural_philosophy")]);
}
