use super::*;
use std::collections::BTreeSet;
use std::sync::Arc;

fn tour() -> (Game, u32, Pos, Pos) {
    let mut game = Game::new_full(2, 40, 16, 472_501, 250, 0, false);
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.owner_city = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
    }
    game.found_city_for(0, (4, 6), None);
    let nearby_city = game.found_city_for(1, (10, 6), None);
    let distant_city = game.found_city_for(1, (25, 6), None);
    let nearby = (9, 6);
    let distant = (24, 6);
    game.cities
        .get_mut(&nearby_city)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), nearby);
    game.cities
        .get_mut(&nearby_city)
        .unwrap()
        .buildings
        .push(crate::name!("amphitheater"));
    game.cities
        .get_mut(&nearby_city)
        .unwrap()
        .buildings
        .push(crate::name!("art_museum"));
    let tile = game.map.tiles.get_mut(&nearby).unwrap();
    tile.owner_city = Some(nearby_city);
    tile.district = Some(crate::name!("theater_square"));
    let tile = game.map.tiles.get_mut(&distant).unwrap();
    tile.owner_city = Some(distant_city);
    tile.wonder = Some(crate::name!("pyramids"));
    game.players[1].open_borders_until.insert(0, 999);
    game.record_contact(0, 1);
    game.players[0]
        .explored
        .extend(game.map.tiles.keys().copied());
    let band = game.spawn_test_unit("rock_band", 0, (8, 6));
    game.units
        .get_mut(&band)
        .unwrap()
        .promotions
        .insert(crate::name!("roadies"));
    (game, band, nearby, distant)
}

#[test]
fn a_nearby_concert_beats_a_long_trip_to_a_premium_venue() {
    let (mut game, band, nearby, distant) = tour();
    assert!(
        game.rock_concert_ai_value(0, band, distant).unwrap()
            > game.rock_concert_ai_value(0, band, nearby).unwrap()
    );
    assert!(game.route_distance(band, distant, 0).unwrap() > 10);
    assert_eq!(
        BasicAi::rock_band_tour_destination(&game, 0, band),
        Some(nearby)
    );
    assert!(BasicAi::new().rock_band_step(&mut game, 0, band));
    assert_eq!(game.units[&band].pos, nearby);
}

#[test]
fn a_premium_concert_can_still_repay_a_longer_journey() {
    let (mut game, band, nearby, distant) = tour();
    game.units.get_mut(&band).unwrap().pos = (14, 6);
    assert!(
        game.route_distance(band, distant, 0).unwrap()
            > game.route_distance(band, nearby, 0).unwrap()
    );
    assert_eq!(
        BasicAi::rock_band_tour_destination(&game, 0, band),
        Some(distant)
    );
}

#[test]
fn tour_selection_respects_native_venues_routes_and_war() {
    let (mut game, band, nearby, distant) = tour();
    Arc::make_mut(&mut game.host_unit_facts).insert(
        band,
        crate::game::HostUnitFacts {
            concert_plots: Some(BTreeSet::from([distant])),
            ..Default::default()
        },
    );
    assert_eq!(
        BasicAi::rock_band_tour_destination(&game, 0, band),
        Some(distant)
    );
    Arc::make_mut(&mut game.host_unit_facts)
        .get_mut(&band)
        .unwrap()
        .concert_plots = Some(BTreeSet::new());
    assert_eq!(BasicAi::rock_band_tour_destination(&game, 0, band), None);
    Arc::make_mut(&mut game.host_unit_facts).clear();
    game.at_war.insert((0, 1));
    assert_eq!(BasicAi::rock_band_tour_destination(&game, 0, band), None);
    game.at_war.clear();
    game.map.tiles.get_mut(&distant).unwrap().terrain = crate::name!("mountain");
    assert!(game.route_distance(band, distant, 0).is_none());
    assert_eq!(
        BasicAi::rock_band_tour_destination(&game, 0, band),
        Some(nearby)
    );
}

#[test]
fn a_competitive_current_venue_is_played_without_another_trip() {
    let (mut game, band, nearby, _) = tour();
    game.units.get_mut(&band).unwrap().pos = nearby;
    assert!(game.rock_concert_tourism(0, band).is_some());
    assert_eq!(
        BasicAi::rock_band_tour_destination(&game, 0, band),
        Some(nearby)
    );
    assert!(BasicAi::new().rock_band_step(&mut game, 0, band));
    assert!(game.players[0]
        .counters
        .get(&format!("rock_concert:{band}:{},{}", nearby.0, nearby.1))
        .is_some_and(|count| *count > 0));
}

fn adjacent_matching_wonder() -> (Game, u32, Pos, Pos) {
    let (mut game, band, theater, old_wonder) = tour();
    let owner_city = game.map.tiles[&theater].owner_city.unwrap();
    game.cities
        .get_mut(&owner_city)
        .unwrap()
        .buildings
        .retain(|building| *building != "art_museum");
    game.map.tiles.get_mut(&old_wonder).unwrap().wonder = None;
    let wonder = (theater.0, theater.1 + 1);
    let tile = game.map.tiles.get_mut(&wonder).unwrap();
    tile.owner_city = Some(owner_city);
    tile.wonder = Some(crate::name!("pyramids"));
    let unit = game.units.get_mut(&band).unwrap();
    unit.pos = theater;
    unit.level = 2;
    unit.xp = 15;
    unit.promotions = BTreeSet::from([crate::name!("album_cover_art")]);
    Arc::make_mut(&mut game.host_unit_facts).insert(
        band,
        crate::game::HostUnitFacts {
            concert_plots: Some(BTreeSet::from([theater, wonder])),
            ..Default::default()
        },
    );
    (game, band, theater, wonder)
}

#[test]
fn a_passing_band_keeps_its_better_reachable_promotion_matching_tour() {
    let (mut game, band, theater, wonder) = adjacent_matching_wonder();
    assert!(game.rock_concert_tourism(0, band).is_some());
    let distance = game.route_distance(band, wonder, 0).unwrap();
    assert_eq!(distance, 1);
    let value = game.rock_concert_ai_value(0, band, wonder).unwrap()
        / (1.0 + distance as f64 / game.unit_max_moves(band));
    assert!(value > game.rock_concert_ai_value(0, band, theater).unwrap());
    assert!(BasicAi::new().rock_band_step(&mut game, 0, band));
    assert_eq!(game.units[&band].pos, wonder);
    assert_eq!(
        game.players[0]
            .counters
            .get(&format!("rock_concert:{band}:{},{}", theater.0, theater.1)),
        None
    );
    assert!(BasicAi::new().rock_band_step(&mut game, 0, band));
    assert!(game.players[0]
        .counters
        .get(&format!("rock_concert:{band}:{},{}", wonder.0, wonder.1))
        .is_some_and(|count| *count > 0));
}

#[test]
fn a_premium_venue_the_host_does_not_allow_cannot_delay_the_current_concert() {
    let (mut game, band, theater, _) = adjacent_matching_wonder();
    Arc::make_mut(&mut game.host_unit_facts)
        .get_mut(&band)
        .unwrap()
        .concert_plots = Some(BTreeSet::from([theater]));
    assert!(BasicAi::new().rock_band_step(&mut game, 0, band));
    assert!(game.players[0]
        .counters
        .get(&format!("rock_concert:{band}:{},{}", theater.0, theater.1))
        .is_some_and(|count| *count > 0));
}
