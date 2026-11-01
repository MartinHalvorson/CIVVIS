use super::*;
use crate::game::{Action, Item};

fn culture_seat_with_a_city(enabled: bool) -> (AdvancedAi, Game, u32) {
    let mut game = Game::new_full(2, 30, 16, 79_104, 200, 0, false);
    let position = game
        .map
        .tiles
        .values()
        .find(|tile| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.unit_ids_at(tile.pos).is_empty()
        })
        .map(|tile| tile.pos)
        .expect("a land tile");
    let settler = game.spawn_test_unit("settler", 0, position);
    game.current = 0;
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.city_at(position).unwrap();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    if enabled {
        ai.enable_culture_digs_one_museum();
    }
    (ai, game, city)
}

#[test]
fn the_first_museum_is_the_dig_museum() {
    let (ai, game, _) = culture_seat_with_a_city(true);
    let dig = crate::name!("archaeological_museum");
    let art = crate::name!("art_museum");

    assert_eq!(ai.dig_museum_bonus(&game, 0, dig), DIG_MUSEUM_BONUS);
    assert_eq!(ai.dig_museum_bonus(&game, 0, art), 0.0);
}

#[test]
fn one_dig_museum_built_or_queued_ends_the_bonus() {
    let dig = crate::name!("archaeological_museum");

    let (ai, mut game, city) = culture_seat_with_a_city(true);
    game.cities
        .get_mut(&city)
        .unwrap()
        .queue
        .push(Item::Building { building: dig });
    assert_eq!(ai.dig_museum_bonus(&game, 0, dig), 0.0, "queued");

    let (ai, mut game, city) = culture_seat_with_a_city(true);
    game.cities.get_mut(&city).unwrap().buildings.push(dig);
    assert_eq!(ai.dig_museum_bonus(&game, 0, dig), 0.0, "built");
}

#[test]
fn off_or_another_lane_adds_nothing() {
    let dig = crate::name!("archaeological_museum");

    let (ai, game, _) = culture_seat_with_a_city(false);
    assert_eq!(ai.dig_museum_bonus(&game, 0, dig), 0.0, "gene off");

    let (_, game, _) = culture_seat_with_a_city(true);
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_culture_digs_one_museum();
    assert_eq!(science.dig_museum_bonus(&game, 0, dig), 0.0, "science seat");
}

#[test]
fn the_delegated_governor_lends_the_flag_only_to_a_culture_seat() {
    let (ai, game, _) = culture_seat_with_a_city(true);
    assert!(ai.dig_museum_lane_active(&game));

    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_culture_digs_one_museum();
    assert!(!science.dig_museum_lane_active(&game));
}
