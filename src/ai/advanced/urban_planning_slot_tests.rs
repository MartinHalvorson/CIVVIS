//! `urban-planning-fills-the-slot`: Urban Planning at the tail of the policy
//! portfolio takes a free slot or an orphaned timed card's slot, and nothing
//! else.

use super::*;

/// A founded capital in Chiefdom (one military and one economic slot) with
/// Code of Laws and Feudalism known, at peace, and no Builder or Settler in
/// any queue: the Builder window and the Settler waves are closed.
fn peaceful_capital() -> (Game, AdvancedAi) {
    let mut game = Game::new(2, 24, 16, 936015, 200, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|id| game.units[id].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    game.players[0].government = Some("chiefdom".to_string());
    game.players[0].civics.extend([
        crate::name!("code_of_laws"),
        crate::name!("craftsmanship"),
        crate::name!("early_empire"),
        crate::name!("feudalism"),
    ]);
    for cid in game.player_city_ids(0) {
        game.cities.get_mut(&cid).unwrap().queue.clear();
    }
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    (game, ai)
}

fn holds(game: &Game, card: &str) -> bool {
    game.players[0].policies.contains(&Name::new(card))
}

#[test]
fn urban_planning_takes_a_closed_builder_windows_serfdom_slot() {
    let run = |gene: bool| {
        let (mut game, mut ai) = peaceful_capital();
        game.players[0].policies.insert(crate::name!("serfdom"));
        if gene {
            ai.enable_urban_planning_fills_the_slot();
        }
        ai.strategic_policies(&mut game, 0, GrandStrategy::Conquest);
        game
    };
    let stock = run(false);
    assert!(
        holds(&stock, "serfdom"),
        "stock: the orphaned Serfdom keeps the slot"
    );
    assert!(
        !holds(&stock, "urban_planning"),
        "stock: nothing asks for Urban Planning"
    );
    let gene = run(true);
    assert!(
        holds(&gene, "urban_planning"),
        "gene: Urban Planning takes the slot"
    );
    assert!(
        !holds(&gene, "serfdom"),
        "gene: the closed window's Serfdom leaves"
    );
}

#[test]
fn urban_planning_fills_an_empty_economic_slot() {
    let run = |gene: bool| {
        let (mut game, mut ai) = peaceful_capital();
        if gene {
            ai.enable_urban_planning_fills_the_slot();
        }
        ai.strategic_policies(&mut game, 0, GrandStrategy::Conquest);
        game
    };
    assert!(
        !holds(&run(false), "urban_planning"),
        "stock: the economic slot stays empty"
    );
    assert!(
        holds(&run(true), "urban_planning"),
        "gene: Urban Planning fills it"
    );
}

#[test]
fn urban_planning_leaves_a_live_builder_window_to_serfdom() {
    let (mut game, mut ai) = peaceful_capital();
    game.players[0].policies.insert(crate::name!("serfdom"));
    ai.enable_builder_charge_window();
    ai.enable_urban_planning_fills_the_slot();
    // A Builder one production short of done reopens the window.
    let cid = game.player_city_ids(0)[0];
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    game.cities
        .get_mut(&cid)
        .unwrap()
        .queue
        .push(builder.clone());
    let cost = game.item_remaining_cost_for_city(0, cid, &builder);
    game.cities.get_mut(&cid).unwrap().production = (cost - 1.0).max(0.0);
    assert_eq!(
        ai.builder_charge_window_card(&game, 0),
        Some("serfdom"),
        "fixture: the Builder window is open"
    );
    ai.strategic_policies(&mut game, 0, GrandStrategy::Conquest);
    assert!(holds(&game, "serfdom"), "a live window keeps Serfdom");
    assert!(!holds(&game, "urban_planning"));
}

#[test]
fn urban_planning_never_evicts_a_card_that_is_not_a_timed_one() {
    let (mut game, mut ai) = peaceful_capital();
    game.players[0]
        .civics
        .insert(crate::name!("recorded_history"));
    // A card the Conquest deck does not want, but not a timed Settler or
    // Builder card either: the tail entry leaves it alone.
    game.players[0]
        .policies
        .insert(crate::name!("natural_philosophy"));
    ai.enable_urban_planning_fills_the_slot();
    ai.strategic_policies(&mut game, 0, GrandStrategy::Conquest);
    assert!(
        holds(&game, "natural_philosophy"),
        "the non-timed card keeps its slot"
    );
    assert!(
        !holds(&game, "urban_planning"),
        "no free slot is left for the tail entry"
    );
}

#[test]
fn urban_planning_fills_the_slot_is_an_independently_reversible_opt_in() {
    assert!(!AdvancedAi::new().urban_planning_fills_the_slot);
    let mut ai = AdvancedAi::new();
    ai.enable_urban_planning_fills_the_slot();
    assert!(ai.urban_planning_fills_the_slot);
    ai.disable_urban_planning_fills_the_slot();
    assert!(!ai.urban_planning_fills_the_slot);
}
