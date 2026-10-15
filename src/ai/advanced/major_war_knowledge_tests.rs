use super::*;
use std::sync::Arc;

fn joined_wars() -> (Game, usize) {
    let mut game = Game::new_full(2, 24, 16, 97, 80, 1, false);
    let minor = game
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_barbarian)
        .expect("fixture has one city-state")
        .id;
    assert!(game.player_city_ids(1).is_empty());
    assert!(!game.player_city_ids(minor).is_empty());
    game.at_war.insert((0, 1));
    game.at_war.insert((0, minor));
    game.record_contact(0, 1);
    game.record_contact(0, minor);
    (game, minor)
}

fn report_unlocated_cities(game: &mut Game, owner: usize, count: usize) {
    Arc::make_mut(&mut game.observed_public_empire_stats)
        .entry(owner)
        .or_default()
        .city_count = Some(count);
    // This is the mirror's derived guard: a public total above the number
    // of located cities proves there are cities whose locations stay hidden.
    if count > game.player_city_ids(owner).len() {
        game.unseen_city_owners.insert(owner);
    } else {
        game.unseen_city_owners.remove(&owner);
    }
}

#[test]
fn an_observed_living_major_stays_ahead_of_its_joined_minor_without_a_known_city() {
    let (mut game, minor) = joined_wars();
    report_unlocated_cities(&mut game, 1, 3);
    assert!(game.players[1].alive);
    assert!(game.is_at_war(0, 1) && game.is_at_war(0, minor));
    assert_eq!(game.observed_public_empire_stats[&1].city_count, Some(3));
    assert!(game.unseen_city_owners.contains(&1));
    assert!(game.player_city_ids(1).is_empty());

    let plan = AdvancedAi::new().assess(&game, 0);
    assert_eq!(
        plan.target_player,
        Some(1),
        "the public count proves the major war is still live; its joined minor must not become the primary conquest"
    );
    assert_eq!(plan.target_city, None, "no hidden city is manufactured");
    assert!(game.player_city_ids(1).is_empty());
}

#[test]
fn missing_public_evidence_keeps_the_existing_minor_war_fallback() {
    let (game, minor) = joined_wars();
    assert!(!game.unseen_city_owners.contains(&1));
    assert_eq!(
        AdvancedAi::new().assess(&game, 0).target_player,
        Some(minor)
    );
}

#[test]
fn a_reported_zero_does_not_pin_the_front_on_an_empty_major() {
    let (mut game, minor) = joined_wars();
    report_unlocated_cities(&mut game, 1, 0);
    assert_eq!(
        AdvancedAi::new().assess(&game, 0).target_player,
        Some(minor)
    );
}

#[test]
fn a_peaceful_major_with_unlocated_cities_does_not_replace_the_current_war() {
    let (mut game, minor) = joined_wars();
    report_unlocated_cities(&mut game, 1, 3);
    game.at_war.remove(&(0, 1));
    assert!(!game.is_at_war(0, 1));
    assert_eq!(
        AdvancedAi::new().assess(&game, 0).target_player,
        Some(minor)
    );
}

#[test]
fn revealing_a_major_city_retains_the_existing_major_war_priority() {
    let (mut game, _) = joined_wars();
    game.current = 1;
    let settler = game
        .player_unit_ids(1)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("fixture major has its starting settler");
    game.apply(1, &Action::FoundCity { unit: settler })
        .expect("fixture major can found its city");
    game.current = 0;
    let city = game.player_city_ids(1)[0];
    let plan = AdvancedAi::new().assess(&game, 0);
    assert_eq!(plan.target_player, Some(1));
    assert_eq!(plan.target_city, Some(city));
}

#[test]
fn an_eliminated_major_is_not_revived_by_an_old_public_count() {
    let (mut game, minor) = joined_wars();
    report_unlocated_cities(&mut game, 1, 3);
    game.players[1].alive = false;
    assert_eq!(
        AdvancedAi::new().assess(&game, 0).target_player,
        Some(minor)
    );
}
