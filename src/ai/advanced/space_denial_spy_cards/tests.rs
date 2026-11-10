use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

/// Communism (one Diplomatic seat, one wildcard) with Gunboat Diplomacy in
/// the Diplomatic seat and Cryptography in the wildcard, both cards the
/// Culture portfolio wants; Diplomatic Service and Cold War adopted, so
/// Machiavellianism and Cryptography are on the menu; seat 1 a met rival
/// with `projects` space projects landed.
fn board(target: VictoryTarget, projects: usize) -> (Game, AdvancedAi) {
    let mut game = Game::new(2, 24, 16, 936_017, 200, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|id| game.units[id].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    game.players[0].government = Some("communism".to_string());
    game.players[0].civics.extend([
        crate::name!("code_of_laws"),
        crate::name!("political_philosophy"),
        crate::name!("ideology"),
        crate::name!("class_struggle"),
        crate::name!("diplomatic_service"),
        crate::name!("cold_war"),
    ]);
    game.players[0].policies.clear();
    game.players[0].policies.extend([
        crate::name!("gunboat_diplomacy"),
        crate::name!("cryptography"),
    ]);
    game.record_contact(0, 1);
    for project in ["launch_earth_satellite", "launch_moon_landing"]
        .into_iter()
        .take(projects)
    {
        game.players[1].science_projects.insert(project.to_string());
    }
    let mut ai = AdvancedAi::targeting(target);
    let strategy = match target {
        VictoryTarget::Science => GrandStrategy::Science,
        VictoryTarget::Domination => GrandStrategy::Conquest,
        _ => GrandStrategy::Culture,
    };
    ai.plan = Some(StrategicPlan {
        strategy,
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

fn deck(game: &mut Game, ai: &AdvancedAi) {
    let strategy = ai.plan.as_ref().unwrap().strategy;
    ai.strategic_policies(game, 0, strategy);
}

#[test]
fn the_gene_ships_off() {
    opt_in_off_in_both_controllers("space-denial-slots-the-spy-cards", |ai| {
        ai.space_denial_slots_the_spy_cards
    });
}

#[test]
fn off_the_culture_deck_keeps_gunboat_diplomacy_and_never_asks_for_machiavellianism() {
    let (mut game, ai) = board(VictoryTarget::Culture, 2);
    assert!(game
        .available_policies(0)
        .contains(&crate::name!("machiavellianism")));
    assert!(ai.denial_spy_cards(&game, 0).is_empty());
    deck(&mut game, &ai);
    assert!(slotted(&game, "gunboat_diplomacy") && slotted(&game, "cryptography"));
    assert!(!slotted(&game, "machiavellianism"));
}

#[test]
fn on_a_culture_seat_against_a_decisive_racer_slots_both_spy_cards() {
    let (mut game, mut ai) = board(VictoryTarget::Culture, 2);
    ai.enable_space_denial_slots_the_spy_cards();
    assert_eq!(
        ai.denial_spy_cards(&game, 0),
        vec!["machiavellianism", "cryptography"]
    );
    deck(&mut game, &ai);
    assert!(
        slotted(&game, "machiavellianism") && slotted(&game, "cryptography"),
        "{:?}",
        game.players[0].policies
    );
    assert!(!slotted(&game, "gunboat_diplomacy"));
    // A second pass is stable: the spy cards hold their seats.
    deck(&mut game, &ai);
    assert!(slotted(&game, "machiavellianism") && slotted(&game, "cryptography"));
}

#[test]
fn on_a_domination_seat_slots_them_too() {
    let (mut game, mut ai) = board(VictoryTarget::Domination, 2);
    ai.enable_space_denial_slots_the_spy_cards();
    deck(&mut game, &ai);
    assert!(
        slotted(&game, "machiavellianism"),
        "{:?}",
        game.players[0].policies
    );
}

#[test]
fn on_a_science_seat_reads_as_off() {
    let (mut game, mut ai) = board(VictoryTarget::Science, 2);
    ai.enable_space_denial_slots_the_spy_cards();
    assert!(ai.denial_spy_cards(&game, 0).is_empty());
    let (mut control, off) = board(VictoryTarget::Science, 2);
    deck(&mut game, &ai);
    deck(&mut control, &off);
    assert_eq!(game.players[0].policies, control.players[0].policies);
    assert!(!slotted(&game, "machiavellianism"));
}

#[test]
fn on_without_a_racer_reads_as_off() {
    let (mut game, mut ai) = board(VictoryTarget::Culture, 1);
    ai.enable_space_denial_slots_the_spy_cards();
    assert!(ai.denial_spy_racers(&game, 0).is_empty());
    assert!(ai.denial_spy_cards(&game, 0).is_empty());
    deck(&mut game, &ai);
    assert!(slotted(&game, "gunboat_diplomacy"));
    assert!(!slotted(&game, "machiavellianism"));
}

#[test]
fn on_with_the_science_victory_off_reads_as_off() {
    let (mut game, mut ai) = board(VictoryTarget::Culture, 2);
    game.victory_conditions.science = false;
    ai.enable_space_denial_slots_the_spy_cards();
    assert!(ai.denial_spy_cards(&game, 0).is_empty());
}

#[test]
fn the_cards_go_just_ahead_of_the_first_ordinary_diplomatic_card() {
    let (game, _) = board(VictoryTarget::Culture, 2);
    let mut desired = vec![
        "music_censorship",
        "heritage_tourism",
        "gunboat_diplomacy",
        "cryptography",
        "levee_en_masse",
    ];
    let protected = |card: &str| card == "music_censorship";
    AdvancedAi::place_denial_spy_cards(
        &game,
        &mut desired,
        &["machiavellianism", "cryptography"],
        &protected,
    );
    assert_eq!(
        desired,
        vec![
            "music_censorship",
            "heritage_tourism",
            "machiavellianism",
            "cryptography",
            "gunboat_diplomacy",
            "levee_en_masse",
        ]
    );
}

#[test]
fn collective_activism_keeps_its_place_ahead_of_the_spy_cards() {
    let (game, _) = board(VictoryTarget::Culture, 2);
    let mut desired = vec![
        "heritage_tourism",
        "collective_activism",
        "gunboat_diplomacy",
    ];
    AdvancedAi::place_denial_spy_cards(
        &game,
        &mut desired,
        &["machiavellianism", "cryptography"],
        &|_: &str| false,
    );
    assert_eq!(
        desired,
        vec![
            "heritage_tourism",
            "collective_activism",
            "machiavellianism",
            "cryptography",
            "gunboat_diplomacy",
        ]
    );
    assert!(!AdvancedAi::denial_spy_card_may_displace(
        &game,
        &desired,
        2,
        "machiavellianism",
        "collective_activism",
        &|_: &str| false,
    ));
}

#[test]
fn a_spy_card_displaces_only_a_later_ordinary_diplomatic_card() {
    let (game, _) = board(VictoryTarget::Culture, 2);
    let desired = vec![
        "music_censorship",
        "merchant_confederation",
        "machiavellianism",
        "gunboat_diplomacy",
        "ecommerce",
    ];
    let protected = |card: &str| card == "music_censorship";
    let may = |current: &str| {
        AdvancedAi::denial_spy_card_may_displace(
            &game,
            &desired,
            2,
            "machiavellianism",
            current,
            &protected,
        )
    };
    assert!(may("gunboat_diplomacy"), "ranked after it");
    assert!(
        may("charismatic_leader"),
        "a Diplomatic card the deck does not want"
    );
    assert!(!may("merchant_confederation"), "ranked ahead of it");
    assert!(!may("music_censorship"), "protected");
    assert!(!may("ecommerce"), "not a Diplomatic card");
    assert!(!may("cryptography"), "the other spy card");
}
