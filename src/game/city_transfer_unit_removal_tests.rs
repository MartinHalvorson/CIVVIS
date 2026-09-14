use super::*;

fn transfer_city_with_carried_aircraft(carrier_first: bool, conquest: bool) {
    let mut game = Game::new_full(2, 26, 16, 914_358_100, 300, 0, false);
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    let center = game
        .map
        .tiles
        .iter()
        .find(|(position, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.nbrs(**position).iter().any(|neighbor| {
                    game.map
                        .get(*neighbor)
                        .is_some_and(|tile| game.rules.is_water(tile))
                })
        })
        .map(|(position, _)| *position)
        .expect("fixture has a coastal city site");
    let city = game.found_city_for(0, center, None);
    let (carrier, aircraft) = if carrier_first {
        let carrier = game.spawn_unit("aircraft_carrier", 0, center);
        let aircraft = game.spawn_unit("fighter", 0, center);
        (carrier, aircraft)
    } else {
        let aircraft = game.spawn_unit("fighter", 0, center);
        let carrier = game.spawn_unit("aircraft_carrier", 0, center);
        (carrier, aircraft)
    };
    let builder = game.spawn_unit("builder", 0, center);
    let settler = game.spawn_unit("settler", 0, center);
    let new_owner_unit = game.spawn_unit("warrior", 1, center);
    let evacuation_snapshot = game.units_at(center);
    assert_eq!(
        evacuation_snapshot.iter().position(|id| *id == carrier)
            < evacuation_snapshot.iter().position(|id| *id == aircraft),
        carrier_first
    );

    game.transfer_city(city, 1, conquest);

    assert_eq!(game.cities[&city].owner, 1);
    assert_eq!(game.cities[&city].captured_from, conquest.then_some(0));
    assert!(!game.units.contains_key(&carrier));
    assert!(!game.units.contains_key(&aircraft));
    assert!(!game.unit_ids_at(center).contains(&carrier));
    assert!(!game.unit_ids_at(center).contains(&aircraft));
    assert_eq!(game.units[&builder].owner, 1);
    assert_eq!(game.units[&settler].owner, 1);
    assert_eq!(game.units[&new_owner_unit].owner, 1);
    assert_eq!(
        game.players[0].unit_lifetimes[&crate::name!("aircraft_carrier")].units,
        1,
        "the carrier is recorded exactly once"
    );
    assert_eq!(
        game.players[0].unit_lifetimes[&crate::name!("fighter")].units,
        1,
        "the aircraft is recorded exactly once, including recursive removal"
    );
    assert!(game
        .unit_ids_at(center)
        .iter()
        .all(|id| game.units.contains_key(id)));
}

#[test]
fn city_transfer_skips_aircraft_already_removed_with_their_carrier() {
    for conquest in [false, true] {
        transfer_city_with_carried_aircraft(true, conquest);
    }
}

#[test]
fn city_transfer_handles_aircraft_before_the_carrier_without_double_accounting() {
    for conquest in [false, true] {
        transfer_city_with_carried_aircraft(false, conquest);
    }
}
