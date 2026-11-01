use super::*;
use crate::game::Action;
use crate::world::DistrictFoundation;

/// Two cities with Rocketry; the first holds a placed, unfinished Spaceport
/// on its tile and an empty queue, as Civilization VI leaves it once a claim
/// has borrowed the queue. Returns the game, the launch city, the other city
/// and the foundation's tile.
fn placed_pad() -> (Game, u32, u32, crate::Pos) {
    let mut game = Game::new_full(
        1,
        30,
        20,
        crate::rng::fixture_seed("SPACEPORT", 324_006),
        100,
        0,
        false,
    );
    let first_settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(
        0,
        &Action::FoundCity {
            unit: first_settler,
        },
    )
    .unwrap();
    let first_center = game.cities[&game.player_city_ids(0)[0]].pos;
    let second_center = game
        .map
        .tiles
        .iter()
        .filter(|(position, tile)| {
            game.wdist(**position, first_center) >= 4
                && game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.unit_ids_at(**position).is_empty()
        })
        .map(|(position, _)| *position)
        .next()
        .expect("map has a second legal city site");
    let second_settler = game.spawn_test_unit("settler", 0, second_center);
    game.apply(
        0,
        &Action::FoundCity {
            unit: second_settler,
        },
    )
    .unwrap();
    game.players[0].techs.insert(crate::name!("rocketry"));
    let cities = game.player_city_ids(0);
    for city in &cities {
        game.cities.get_mut(city).unwrap().pop = 10;
    }
    let (launch, other) = (cities[0], cities[1]);
    let site = *game
        .district_sites(launch, crate::name!("spaceport"))
        .first()
        .expect("the launch city can place a Spaceport");
    game.map.tiles.get_mut(&site).unwrap().district_foundation = Some(DistrictFoundation {
        district: crate::name!("spaceport"),
        cost: 900.0,
    });
    game.cities.get_mut(&launch).unwrap().queue.clear();
    (game, launch, other, site)
}

#[test]
fn the_idle_city_resumes_its_own_placed_pad() {
    let (game, launch, other, site) = placed_pad();
    let pad = Item::District {
        district: crate::name!("spaceport"),
        pos: site,
    };
    let stock = BasicAi::new();
    assert_eq!(stock.launch_pad_resume_item(&game, 0, launch), None);

    let mut ai = BasicAi::new();
    ai.enable_launch_pad_resumes_its_foundation();
    assert_eq!(
        ai.launch_pad_resume_item(&game, 0, launch),
        Some(pad.clone())
    );
    assert_eq!(
        ai.pick_item(&game, 0, launch, 2, 2, 2, 2, 1, 10, 5, 5),
        Some(pad),
        "the governor's pick resumes the pad"
    );
    assert_eq!(
        ai.launch_pad_resume_item(&game, 0, other),
        None,
        "a city without the foundation is not sent to start a second pad"
    );

    let mut domination = ai.clone();
    domination.exclude_space_race = true;
    assert_eq!(domination.launch_pad_resume_item(&game, 0, launch), None);
}

#[test]
fn a_standing_pad_ends_the_resume() {
    let (mut game, launch, other, _) = placed_pad();
    let mut ai = BasicAi::new();
    ai.enable_launch_pad_resumes_its_foundation();
    crate::game::install_test_district(&mut game, other, "spaceport");
    assert_eq!(
        ai.launch_pad_resume_item(&game, 0, launch),
        None,
        "with a pad standing the race has its launch site"
    );
}
