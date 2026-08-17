use super::*;

fn band_of(longitude: f64) -> usize {
    let turns = (longitude / std::f64::consts::TAU).rem_euclid(1.0);
    ((turns * GLOBE_LAP_BANDS as f64) as usize).min(GLOBE_LAP_BANDS - 1)
}

/// A people learn that their world comes back on itself by going round it,
/// and not one step before. Every longitude but one still leaves a gap,
/// and a gap is a direction the world might simply keep going in.
#[test]
fn a_world_is_known_to_come_back_on_itself_only_once_every_longitude_is_seen() {
    let mut game = Game::new(2, 24, 16, 4_242, 25, 0);
    let width = game.map.width;
    game.players[0].explored.clear();
    game.players[0].went_around = false;
    // Setup vision already primed the accumulation; the fixture rebuilds
    // its own explored set from nothing, so the accumulation starts over.
    game.players[0].lap_bands.clear();

    for column in 0..width - 1 {
        game.players[0]
            .explored
            .insert(crate::hex::offset_to_axial(column, 8));
    }
    // An empty accumulation is rebuilt from the whole explored set, which
    // is also how a save from before the cache existed comes up to date.
    game.update_world_lap(0, &[]);
    assert!(
        !game.players[0].went_around,
        "one unseen longitude is a way the world might still continue",
    );

    let last = crate::hex::offset_to_axial(width - 1, 8);
    game.players[0].explored.insert(last);
    // A primed accumulation looks only at the ground that just arrived.
    game.update_world_lap(0, &[last]);
    assert!(game.players[0].went_around, "that was a lap");
    assert!(
        game.events
            .iter()
            .any(|event| event.player == 0 && event.text.contains("whole way around the world")),
        "closing the ring is the sort of thing a chronicle records",
    );

    // Knowing the shape of your world is not something you can un-know:
    // ground can be lost, and this cannot.
    game.players[0].explored.clear();
    game.update_world_lap(0, &[]);
    assert!(game.players[0].went_around);
}

/// Near a pole every longitude is a few steps from every other, so a
/// circuit of the ice crosses all of them without going anywhere. It is
/// not a lap of the world and must not read as one.
#[test]
fn a_circuit_of_the_ice_is_not_a_lap_of_a_globe() {
    let mut game = Game::new(2, 24, 16, 90_210, 25, 0);
    game.map = crate::world::WorldMap::globe(16);
    let cells: Vec<(Pos, f64, f64)> = {
        let sphere = game.map.sphere().expect("a globe is laid out on a sphere");
        sphere
            .positions()
            .map(|pos| (pos, sphere.latitude(pos), sphere.longitude(pos)))
            .collect()
    };
    let polar: Vec<(Pos, f64)> = cells
        .iter()
        .filter(|(_, latitude, _)| latitude.abs() > 70f64.to_radians())
        .map(|(pos, _, longitude)| (*pos, *longitude))
        .collect();
    assert_eq!(
        polar
            .iter()
            .map(|(_, longitude)| band_of(*longitude))
            .collect::<BTreeSet<_>>()
            .len(),
        GLOBE_LAP_BANDS,
        "the fixture only tests anything if the ice does cross every longitude",
    );

    game.players[0].explored.clear();
    game.players[0].went_around = false;
    game.players[0].lap_bands.clear();
    let polar_positions: Vec<Pos> = polar.iter().map(|(pos, _)| *pos).collect();
    for pos in &polar_positions {
        game.players[0].explored.insert(*pos);
    }
    game.update_world_lap(0, &polar_positions);
    assert!(
        !game.players[0].went_around,
        "walking round the ice is not sailing round the world",
    );

    let equator: Vec<Pos> = cells
        .iter()
        .filter(|(_, latitude, _)| latitude.abs() < 20f64.to_radians())
        .map(|(pos, _, _)| *pos)
        .collect();
    for pos in &equator {
        game.players[0].explored.insert(*pos);
    }
    game.update_world_lap(0, &equator);
    assert!(
        game.players[0].went_around,
        "a way round at the equator is the real thing",
    );
}
