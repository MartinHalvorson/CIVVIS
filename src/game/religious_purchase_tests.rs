//! Faith-purchased religious units adopt their city's majority religion —
//! the stock rule that lets civilizations without a founded religion field
//! Missionaries of an adopted faith.
use super::{Action, Game, Pos};
use crate::name::Name;

fn founded_two_cities() -> (Game, u32) {
    let mut game = Game::new_full(2, 30, 18, 4_242, 200, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.current = pid;
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    game.current = 0;
    let city = game.player_city_ids(0)[0];
    (game, city)
}

fn enable_faith_purchase(game: &mut Game, city: u32) -> Pos {
    let pos = game.cities[&city].pos;
    let site = game
        .wdisk(pos, 2)
        .into_iter()
        .find(|p| {
            game.wdist(*p, pos) == 2 && game.map.get(*p).is_some_and(|t| t.district.is_none())
        })
        .expect("open tile for holy site");
    let tile = game.map.tiles.get_mut(&site).unwrap();
    tile.district = Some(crate::name!("holy_site"));
    tile.owner_city = Some(city);
    tile.terrain = crate::name!("grassland");
    tile.feature = None;
    let c = game.cities.get_mut(&city).unwrap();
    c.districts.insert(crate::name!("holy_site"), site);
    c.buildings.push(crate::name!("shrine"));
    game.players[0].techs.insert(crate::name!("astrology"));
    game.players[0].faith = 1_000.0;
    site
}

#[test]
fn religious_purchases_use_holy_site_then_center_with_source_city_bonuses() {
    for district in ["holy_site", "lavra"] {
        let (mut game, city) = founded_two_cities();
        let site = enable_faith_purchase(&mut game, city);
        let center = game.cities[&city].pos;
        game.map.tiles.get_mut(&site).unwrap().district = Some(Name::new(district));
        let c = game.cities.get_mut(&city).unwrap();
        c.districts.clear();
        c.districts.insert(Name::new(district), site);
        c.buildings.push(crate::name!("mosque"));
        c.pressure.insert("Adopted Faith".to_string(), 1_000.0);
        game.players[1].religion = Some("Adopted Faith".to_string());
        // A friendly military unit can share the religious unit's district.
        game.spawn_unit("warrior", 0, site);
        for expected in [site, center] {
            let before = game.player_unit_ids(0);
            game.apply(
                0,
                &Action::Buy {
                    city,
                    unit: crate::name!("missionary"),
                    formation: 0,
                    currency: "faith".to_string(),
                },
            )
            .expect("religious purchase");
            let uid = game
                .player_unit_ids(0)
                .into_iter()
                .find(|uid| !before.contains(uid))
                .unwrap();
            let purchased = &game.units[&uid];
            assert_eq!(purchased.pos, expected, "{district} placement");
            assert_eq!(purchased.religion.as_deref(), Some("Adopted Faith"));
            assert_eq!(
                purchased.charges,
                game.rules.units["missionary"].charges + 1
            );
        }
    }
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
    assert_eq!(
        game.city_religion(&game.cities[&city]),
        Some("Adopted Faith")
    );

    game.apply(
        0,
        &Action::Buy {
            city,
            unit: crate::name!("missionary"),
            formation: 0,
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
            unit: crate::name!("missionary"),
            formation: 0,
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

#[test]
fn warrior_monks_require_the_belief_and_temple_and_use_faith() {
    let (mut game, city) = founded_two_cities();
    enable_faith_purchase(&mut game, city);
    game.players[0].religion = Some("Home Faith".to_string());
    game.players[0].religion_beliefs = vec!["warrior_monks".to_string()];
    game.cities
        .get_mut(&city)
        .unwrap()
        .pressure
        .insert("Home Faith".to_string(), 1_000.0);
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("temple"));

    for uid in game
        .player_unit_ids(0)
        .into_iter()
        .filter(|uid| game.units[uid].kind == "warrior")
        .collect::<Vec<_>>()
    {
        game.remove_unit(uid);
    }

    assert_eq!(game.rules.units["warrior_monk"].cost, 100.0);
    assert_eq!(
        game.unit_purchase_cost(0, city, "warrior_monk", "faith"),
        Some(200.0)
    );
    assert_eq!(
        game.unit_purchase_cost(0, city, "warrior_monk", "gold"),
        None
    );

    game.players[0].religion_beliefs.clear();
    assert_eq!(
        game.unit_purchase_cost(0, city, "warrior_monk", "faith"),
        None
    );
    game.players[0].religion_beliefs = vec!["warrior_monks".to_string()];
    game.cities.get_mut(&city).unwrap().buildings.clear();
    assert_eq!(
        game.unit_purchase_cost(0, city, "warrior_monk", "faith"),
        None
    );
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("temple"));

    game.apply(
        0,
        &Action::Buy {
            city,
            unit: crate::name!("warrior_monk"),
            formation: 0,
            currency: "faith".to_string(),
        },
    )
    .expect("faith purchase");
    let monk = game
        .units
        .values()
        .find(|unit| unit.owner == 0 && unit.kind == "warrior_monk")
        .expect("Warrior Monk spawned");
    assert_eq!(monk.religion.as_deref(), Some("Home Faith"));
    assert_eq!(game.players[0].faith, 800.0);
}
