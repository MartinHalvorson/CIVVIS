//! Faith-purchased religious units adopt their city's majority religion —
//! the stock rule that lets civilizations without a founded religion field
//! Missionaries of an adopted faith.
use super::{Action, Game};

fn founded_two_cities() -> (Game, u32) {
    let mut game = Game::new_full(2, 30, 18, 4_242, 200, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.current = pid;
        game.apply(pid, &Action::FoundCity { unit: settler }).unwrap();
    }
    game.current = 0;
    let city = game.player_city_ids(0)[0];
    (game, city)
}

fn enable_faith_purchase(game: &mut Game, city: u32) {
    let pos = game.cities[&city].pos;
    let site = game
        .wdisk(pos, 1)
        .into_iter()
        .find(|p| *p != pos && game.map.get(*p).is_some_and(|t| t.district.is_none()))
        .expect("open tile for holy site");
    let tile = game.map.tiles.get_mut(&site).unwrap();
    tile.district = Some("holy_site".to_string());
    tile.owner_city = Some(city);
    let c = game.cities.get_mut(&city).unwrap();
    c.districts.insert("holy_site".to_string(), site);
    c.buildings.push("shrine".to_string());
    game.players[0].techs.insert("astrology".to_string());
    game.players[0].faith = 1_000.0;
}

#[test]
fn purchased_missionary_adopts_the_city_majority_religion() {
    let (mut game, city) = founded_two_cities();
    enable_faith_purchase(&mut game, city);
    // Player 1 founded "Foreign Faith"; player 0 founded nothing, but their
    // city has been converted to the rival majority's sibling faith.
    game.players[1].religion = Some("Adopted Faith".to_string());
    game.cities
        .get_mut(&city)
        .unwrap()
        .pressure
        .insert("Adopted Faith".to_string(), 1_000.0);
    assert_eq!(game.city_religion(&game.cities[&city]), Some("Adopted Faith"));

    game.apply(
        0,
        &Action::Buy {
            city,
            unit: "missionary".to_string(),
            currency: "faith".to_string(),
        },
    )
    .expect("faith purchase in a majority-religion city");
    let missionary = game
        .units
        .values()
        .find(|unit| unit.owner == 0 && unit.kind == "missionary")
        .expect("missionary spawned");
    assert_eq!(missionary.religion.as_deref(), Some("Adopted Faith"));
}

#[test]
fn founder_purchase_still_prefers_the_city_majority() {
    let (mut game, city) = founded_two_cities();
    enable_faith_purchase(&mut game, city);
    // A founder buying in a converted city gets the city's faith, not their
    // own — matching the stock behavior and keeping the AI guard (never buy
    // in a converted city) meaningful.
    game.players[0].religion = Some("Home Faith".to_string());
    game.players[1].religion = Some("Rival Faith".to_string());
    game.cities
        .get_mut(&city)
        .unwrap()
        .pressure
        .insert("Rival Faith".to_string(), 1_000.0);

    game.apply(
        0,
        &Action::Buy {
            city,
            unit: "missionary".to_string(),
            currency: "faith".to_string(),
        },
    )
    .expect("faith purchase");
    let missionary = game
        .units
        .values()
        .find(|unit| unit.owner == 0 && unit.kind == "missionary")
        .expect("missionary spawned");
    assert_eq!(missionary.religion.as_deref(), Some("Rival Faith"));
}
