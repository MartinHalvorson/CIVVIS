use super::*;

/// A lethal area strike on a tile holding a loaded Aircraft Carrier: removing
/// the carrier removes its aircraft, so the strike must skip the aircraft it
/// already took instead of indexing them (the ladder proxy panicked here with
/// "unit N is not present" under a WMD strike on a carrier group).
fn lethal_strike_on_a_loaded_carrier(carrier_first: bool) {
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
    game.found_city_for(0, center, None);
    let (carrier, aircraft) = if carrier_first {
        let carrier = game.spawn_unit("aircraft_carrier", 0, center);
        let aircraft = game.spawn_unit("fighter", 0, center);
        (carrier, aircraft)
    } else {
        let aircraft = game.spawn_unit("fighter", 0, center);
        let carrier = game.spawn_unit("aircraft_carrier", 0, center);
        (carrier, aircraft)
    };
    let snapshot = game.units_at(center);
    assert_eq!(
        snapshot.iter().position(|id| *id == carrier)
            < snapshot.iter().position(|id| *id == aircraft),
        carrier_first
    );

    game.damage_tile_area(center, 1_000, Some(1));

    assert!(!game.units.contains_key(&carrier));
    assert!(!game.units.contains_key(&aircraft));
    assert!(game
        .unit_ids_at(center)
        .iter()
        .all(|id| game.units.contains_key(id)));
    assert_eq!(
        game.players[0].unit_lifetimes[&crate::name!("aircraft_carrier")].units,
        1,
        "the carrier is recorded exactly once"
    );
    assert_eq!(
        game.players[0].unit_lifetimes[&crate::name!("fighter")].units,
        1,
        "the aircraft is recorded exactly once, whichever came first"
    );
}

#[test]
fn a_lethal_strike_skips_aircraft_already_removed_with_their_carrier() {
    lethal_strike_on_a_loaded_carrier(true);
}

#[test]
fn a_lethal_strike_handles_aircraft_before_their_carrier() {
    lethal_strike_on_a_loaded_carrier(false);
}
