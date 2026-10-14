//! `merchant-confederation-earns-its-slot`: the Conquest portfolio never names
//! Merchant Confederation, so a deck at 41 to 63 placed Envoys kept
//! Charismatic Leader and then Gunboat Diplomacy in the diplomatic slot while
//! income ran negative (live Emperor civvis-20261014T083713Z, t100-t205).
use super::*;

/// Monarchy (one diplomatic slot) with Charismatic Leader slotted, Medieval
/// Faires adopted so Merchant Confederation is on the menu, `envoys` Envoys
/// placed, and a Conquest plan.
fn monarchy_with_envoys(envoys: i64) -> (Game, AdvancedAi) {
    let mut game = Game::new(2, 24, 16, 936_017, 200, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|id| game.units[id].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    game.players[0].government = Some("monarchy".to_string());
    game.players[0].civics.extend([
        crate::name!("code_of_laws"),
        crate::name!("political_philosophy"),
        crate::name!("divine_right"),
        crate::name!("medieval_faires"),
    ]);
    game.players[0]
        .policies
        .insert(crate::name!("charismatic_leader"));
    game.players[0].envoys = vec![(5, envoys)];
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

fn slotted(game: &Game, card: &str) -> bool {
    game.players[0].policies.contains(&Name::new(card))
}

#[test]
fn off_the_conquest_deck_never_asks_for_merchant_confederation() {
    let (mut game, ai) = monarchy_with_envoys(12);
    assert!(game
        .available_policies(0)
        .contains(&crate::name!("merchant_confederation")));
    assert_eq!(ai.merchant_confederation_pays(&game, 0), None);
    ai.strategic_policies(&mut game, 0, GrandStrategy::Conquest);
    assert!(!slotted(&game, "merchant_confederation"));
}

#[test]
fn on_twelve_envoys_slot_merchant_confederation() {
    let (mut game, mut ai) = monarchy_with_envoys(12);
    ai.enable_merchant_confederation_earns_its_slot();
    assert_eq!(ai.merchant_confederation_pays(&game, 0), Some((12, 12.0)));
    ai.strategic_policies(&mut game, 0, GrandStrategy::Conquest);
    assert!(
        slotted(&game, "merchant_confederation"),
        "{:?}",
        game.players[0].policies
    );
}

#[test]
fn on_below_the_gold_floor_reads_as_off() {
    let (mut game, mut ai) = monarchy_with_envoys(9);
    ai.enable_merchant_confederation_earns_its_slot();
    assert_eq!(ai.merchant_confederation_pays(&game, 0), None);
    ai.strategic_policies(&mut game, 0, GrandStrategy::Conquest);
    assert!(!slotted(&game, "merchant_confederation"));
}

#[test]
fn on_a_government_without_a_diplomatic_slot_reads_as_off() {
    let (game, mut ai) = {
        let (mut game, ai) = monarchy_with_envoys(30);
        game.players[0].government = Some("oligarchy".to_string());
        (game, ai)
    };
    ai.enable_merchant_confederation_earns_its_slot();
    assert_eq!(g_slots_diplomatic(&game), 0);
    assert_eq!(ai.merchant_confederation_pays(&game, 0), None);
}

fn g_slots_diplomatic(game: &Game) -> i64 {
    game.gov_slots(0).diplomatic
}

#[test]
fn on_gunboat_diplomacy_does_not_evict_it() {
    let (mut game, mut ai) = monarchy_with_envoys(40);
    ai.enable_merchant_confederation_earns_its_slot();
    ai.strategic_policies(&mut game, 0, GrandStrategy::Conquest);
    assert!(slotted(&game, "merchant_confederation"));
    // Gunboat Diplomacy, on the Conquest list, comes onto the menu.
    game.players[0].civics.insert(crate::name!("ideology"));
    assert!(game
        .available_policies(0)
        .contains(&crate::name!("gunboat_diplomacy")));
    ai.strategic_policies(&mut game, 0, GrandStrategy::Conquest);
    assert!(
        slotted(&game, "merchant_confederation"),
        "{:?}",
        game.players[0].policies
    );
}

#[test]
fn the_toggle_sets_the_base_deck_reading_too() {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(!ai.base.deck_reads_player_income);
    ai.enable_merchant_confederation_earns_its_slot();
    assert!(ai.base.deck_reads_player_income);
    ai.disable_merchant_confederation_earns_its_slot();
    assert!(!ai.base.deck_reads_player_income);
}
