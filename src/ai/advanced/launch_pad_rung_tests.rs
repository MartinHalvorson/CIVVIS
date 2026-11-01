//! `launch-pad-keeps-its-rung`: the city holding the empire's unfinished
//! first Spaceport keeps the 3,000 first-pad rung after a claim borrows its
//! queue. See `AdvancedAi::unfinished_launch_pad_site`.
use super::*;
use crate::game::install_test_district;
use crate::world::DistrictFoundation;

#[test]
fn launch_pad_keeps_its_rung_is_a_native_opt_in_off_in_both_controllers() {
    super::test_support::opt_in_off_in_both_controllers("launch-pad-keeps-its-rung", |ai| {
        ai.launch_pad_keeps_its_rung
    });
}

fn found_second_city(game: &mut Game) -> u32 {
    let position = game
        .map
        .tiles
        .values()
        .filter(|tile| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && tile.district.is_none()
                && tile.wonder.is_none()
                && tile.owner_city.is_none()
                && game.city_at(tile.pos).is_none()
                && game.unit_ids_at(tile.pos).is_empty()
                && game
                    .cities
                    .values()
                    .all(|city| game.wdist(city.pos, tile.pos) >= 4)
        })
        .map(|tile| tile.pos)
        .next()
        .expect("test map needs another legal city site");
    let settler = game.spawn_test_unit("settler", 0, position);
    game.current = 0;
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    game.city_at(position).unwrap()
}

/// Two Science cities past the halfway mark, one out-producing the other.
/// The better producer places the first pad, then a claim takes its queue:
/// Civilization VI keeps the foundation on the tile and the queue is empty.
/// Stock, resuming that foundation reads as a second pad in the city and is
/// vetoed; with the gene it is resumable at the first-pad rung, the slower
/// city still cannot start a second pad, and the rung ends once a pad
/// stands.
#[test]
fn the_placed_pad_keeps_the_first_pad_rung_after_its_queue_is_borrowed() {
    let mut game = Game::new(2, 32, 24, 5_414, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let racer = game.player_city_ids(0)[0];
    let slower = found_second_city(&mut game);
    game.players[0].techs = game.rules.techs.keys().cloned().collect();
    game.max_turns = 100_000;
    game.turn = game.max_turns / 2;
    install_test_district(&mut game, racer, "industrial_zone");
    game.cities.get_mut(&racer).unwrap().pop = 14;
    game.cities.get_mut(&slower).unwrap().pop = 4;

    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };
    let site = *game
        .district_sites(racer, crate::name!("spaceport"))
        .first()
        .expect("the racer can place a Spaceport");
    let pad = Item::District {
        district: crate::name!("spaceport"),
        pos: site,
    };

    let mut stock = AdvancedAi::targeting(VictoryTarget::Science);
    stock.enable_live_bridge_universe();
    stock.refresh_research_weight(&game);
    let mut treated = stock.clone();
    treated.enable_launch_pad_keeps_its_rung();
    let counts = stock.counts(&game, 0);
    assert_eq!(stock.launch_pad_site(&game, 0), Some(racer));
    let first_claim = stock.production_value(&game, 0, racer, &pad, &plan, &counts);

    // The pad is placed, then a Spy borrows the queue: the foundation stays
    // on the tile and nothing of the pad is left in the queue.
    game.map.tiles.get_mut(&site).unwrap().district_foundation = Some(DistrictFoundation {
        district: crate::name!("spaceport"),
        cost: 900.0,
    });
    game.cities.get_mut(&racer).unwrap().queue.clear();
    assert_eq!(stock.launch_pad_site(&game, 0), None);
    assert_eq!(
        AdvancedAi::unfinished_launch_pad_site(&game, 0),
        Some(racer)
    );

    let lapsed = stock.production_value(&game, 0, racer, &pad, &plan, &counts);
    let kept = treated.production_value(&game, 0, racer, &pad, &plan, &counts);
    assert!(
        lapsed < first_claim,
        "stock, the placed pad cannot be resumed: {lapsed} vs {first_claim}"
    );
    assert!(
        kept > lapsed + 2_000.0,
        "the treated seat keeps the rung on its own pad: {kept} vs {lapsed}"
    );

    let fresh = Item::District {
        district: crate::name!("spaceport"),
        pos: *game
            .district_sites(slower, crate::name!("spaceport"))
            .first()
            .expect("the slower city can place a Spaceport"),
    };
    assert_eq!(
        treated.production_value(&game, 0, slower, &fresh, &plan, &counts),
        stock.production_value(&game, 0, slower, &fresh, &plan, &counts),
        "the rung stays with the pad already begun; a second city gains nothing"
    );

    // A standing pad ends it.
    game.map.tiles.get_mut(&site).unwrap().district_foundation = None;
    install_test_district(&mut game, racer, "spaceport");
    assert_eq!(AdvancedAi::unfinished_launch_pad_site(&game, 0), None);
}
