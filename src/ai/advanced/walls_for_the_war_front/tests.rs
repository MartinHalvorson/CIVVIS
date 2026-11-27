use super::*;
use crate::Pos;

/// Our capital and a second city, a rival city to the east, at war, and one
/// rival warrior seven tiles from the second city.
fn war_front() -> (Game, u32, u32) {
    front_with(true, (31, 10))
}

fn front_with(at_war: bool, soldier: Pos) -> (Game, u32, u32) {
    let mut game = Game::new_full(2, 48, 24, 4063, 650, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.clear_mirror_cities();
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let capital = game.found_city_for(0, (12, 10), None);
    let front = game.found_city_for(0, (24, 10), None);
    game.found_city_for(1, (35, 10), None);
    assert!(!game.cities[&front].is_capital);
    game.cities.get_mut(&front).unwrap().pop = 4;
    game.current = 0;
    game.turn = 70;
    game.record_contact(0, 1);
    game.players[0].techs.insert(crate::name!("mining"));
    game.players[0].techs.insert(crate::name!("masonry"));
    Arc::make_mut(&mut game.observed_military_power).insert(0, 100.0);
    Arc::make_mut(&mut game.observed_military_power).insert(1, 250.0);
    if at_war {
        game.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
    }
    assert_eq!(game.is_at_war(0, 1), at_war);
    assert!(game.wdist(game.cities[&capital].pos, soldier) > 8);
    let unit = game.spawn_test_unit("warrior", 1, soldier);
    game.host_observed = Arc::new(game.wdisk((24, 10), 8).into_iter().collect());
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    game.apply(
        0,
        &Action::Produce {
            city: front,
            item: builder,
        },
    )
    .unwrap();
    (game, front, unit)
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_walls_for_the_war_front();
    ai
}

fn walls() -> Item {
    Item::Building {
        building: crate::name!("walls"),
    }
}

fn builder() -> Item {
    Item::Unit {
        unit: crate::name!("builder"),
    }
}

#[test]
fn an_unwalled_city_the_enemy_was_seen_near_starts_its_walls() {
    let (mut game, front, _) = war_front();
    assert!(game.can_produce(0, front, &walls()));

    let mut off = AdvancedAi::new();
    off.observe_turn_start_hostiles(&game, 0);
    let mut unarmed = game.clone();
    off.claim_walls_for_the_war_front(&mut unarmed, 0);
    assert_eq!(unarmed.cities[&front].queue.first(), Some(&builder()));

    let mut ai = armed();
    ai.observe_turn_start_hostiles(&game, 0);
    assert!(ai.war_front_walls_threat(&game, 0, front).is_some());
    ai.claim_walls_for_the_war_front(&mut game, 0);
    assert_eq!(
        game.cities[&front].queue.first(),
        Some(&walls()),
        "the city on the war front builds its walls before the siege"
    );
}

#[test]
fn a_peaceful_neighbour_a_distant_soldier_or_a_weak_enemy_starts_nothing() {
    // At peace: the frontier gene's job, not this one.
    let (mut game, front, _) = front_with(false, (31, 10));
    let mut ai = armed();
    ai.remember_war_front_sightings(&game, 0);
    ai.claim_walls_for_the_war_front(&mut game, 0);
    assert_eq!(game.cities[&front].queue.first(), Some(&builder()));

    // The soldier stands nine tiles off.
    let (mut game, front, _) = front_with(true, (33, 10));
    assert!(game.wdist(game.cities[&front].pos, (33, 10)) > WAR_FRONT_TILES);
    let mut ai = armed();
    ai.remember_war_front_sightings(&game, 0);
    ai.claim_walls_for_the_war_front(&mut game, 0);
    assert_eq!(game.cities[&front].queue.first(), Some(&builder()));

    // The enemy fields under half our power.
    let (mut game, front, _) = war_front();
    Arc::make_mut(&mut game.observed_military_power).insert(1, 40.0);
    let mut ai = armed();
    ai.remember_war_front_sightings(&game, 0);
    ai.claim_walls_for_the_war_front(&mut game, 0);
    assert_eq!(game.cities[&front].queue.first(), Some(&builder()));
}

#[test]
fn the_sighting_outlasts_the_soldier_and_ends_with_its_window() {
    let (game, front, unit) = war_front();
    let mut ai = armed();
    ai.remember_war_front_sightings(&game, 0);

    let mut later = game.clone();
    later.remove_unit(unit);
    later.turn += 3;
    ai.remember_war_front_sightings(&later, 0);
    let mut claimed = later.clone();
    ai.claim_walls_for_the_war_front(&mut claimed, 0);
    assert_eq!(claimed.cities[&front].queue.first(), Some(&walls()));

    let mut expired = game.clone();
    expired.remove_unit(unit);
    expired.turn += game.standard_duration(WAR_FRONT_STANDARD) + 1;
    ai.remember_war_front_sightings(&expired, 0);
    assert!(ai.war_front_walls_threat(&expired, 0, front).is_none());
    ai.claim_walls_for_the_war_front(&mut expired, 0);
    assert_eq!(expired.cities[&front].queue.first(), Some(&builder()));
}

#[test]
fn a_walled_city_is_left_alone() {
    let (mut game, front, _) = war_front();
    let walls_name = crate::name!("walls");
    game.cities
        .get_mut(&front)
        .unwrap()
        .buildings
        .push(walls_name);
    assert!(game.city_max_wall_hp(&game.cities[&front]) > 0);
    let mut ai = armed();
    ai.remember_war_front_sightings(&game, 0);
    assert!(ai.war_front_sightings.is_empty());
    ai.claim_walls_for_the_war_front(&mut game, 0);
    assert_eq!(game.cities[&front].queue.first(), Some(&builder()));
}
