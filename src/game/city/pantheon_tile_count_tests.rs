//! Unused pantheon counts stay lazy; every nonzero data row still gets a census.
use super::*;

const EFFECTS: [&str; 9] = [
    "pasture_culture",
    "fishing_boats_production",
    "camp_food",
    "camp_production",
    "quarry_faith",
    "plantation_culture",
    "resource_mine_faith",
    "strategic_improved_production",
    "strategic_improved_faith",
];

fn fixture() -> (Game, u32) {
    let mut game = Game::new_full(2, 24, 16, 91_172, 100, 0, false);
    let position = game
        .units
        .values()
        .find(|unit| unit.owner == 0 && unit.kind == "settler")
        .unwrap()
        .pos;
    let city = game.place_city(0, position, None);
    game.players[0].pantheon = Some("god_of_the_forge".to_string());
    Arc::make_mut(&mut game.rules)
        .beliefs
        .pantheon
        .get_mut("god_of_the_forge")
        .unwrap()
        .effects
        .clear();
    (game, city)
}

fn set_effect(game: &mut Game, effect: &str, coefficient: f64) {
    Arc::make_mut(&mut game.rules)
        .beliefs
        .pantheon
        .get_mut("god_of_the_forge")
        .unwrap()
        .effects
        .insert(effect.to_string(), coefficient);
}

fn read(game: &Game, city: u32) -> (Yields, usize) {
    PANTHEON_TILE_CENSUSES.with(|count| count.set(0));
    let yields = game.city_yields_model(city);
    let censuses = PANTHEON_TILE_CENSUSES.with(std::cell::Cell::get);
    (yields, censuses)
}

fn bits(yields: Yields) -> [u64; 6] {
    [
        yields.food,
        yields.production,
        yields.gold,
        yields.science,
        yields.culture,
        yields.faith,
    ]
    .map(f64::to_bits)
}

#[test]
fn no_pantheon_and_unrelated_pantheon_do_not_scan_city_tiles() {
    let (mut game, city) = fixture();
    assert_eq!(read(&game, city).1, 0);
    game.players[0].pantheon = None;
    assert_eq!(read(&game, city).1, 0);
}

#[test]
fn both_signed_zeros_preserve_yield_bits_without_a_census() {
    let (mut game, city) = fixture();
    let baseline = bits(read(&game, city).0);
    for coefficient in [0.0, -0.0] {
        for effect in EFFECTS {
            set_effect(&mut game, effect, coefficient);
        }
        let (actual, censuses) = read(&game, city);
        assert_eq!(bits(actual), baseline);
        assert_eq!(censuses, 0);
    }
}

#[test]
fn every_nonzero_coefficient_still_requests_its_original_census() {
    for effect in EFFECTS {
        let (mut game, city) = fixture();
        for coefficient in [1.5, -2.25, f64::MIN_POSITIVE, f64::INFINITY, f64::NAN] {
            set_effect(&mut game, effect, coefficient);
            assert_eq!(read(&game, city).1, 1, "{effect}: {coefficient}");
        }
    }
}

#[test]
fn the_strategic_resource_census_is_shared_by_both_nonzero_bonuses() {
    let (mut game, city) = fixture();
    set_effect(&mut game, "strategic_improved_production", 1.0);
    set_effect(&mut game, "strategic_improved_faith", 1.0);
    assert_eq!(read(&game, city).1, 1);
}
