use std::collections::BTreeSet;

use civvis::game::{Game, GameOptions};
use civvis::mapgen::generate_with_script;
use civvis::rng::Rng;
use civvis::rules::Rules;
use civvis::setup::{
    MapPoles, MapScript, MapTopology, BATTLEFIELD_SIZES, CIV6_MAP_SCRIPTS, CIV6_MAP_SIZES,
    MAP_POLES,
};
use civvis::world::Topology;

/// The rectangle a script is generated into, and how many majors it seats.
///
/// Every world type takes whatever size and seat count the matrix hands it. A
/// scenario cannot: it is drawn at the size of its own chart and fought by the
/// two fleets that were there, and a Trafalgar generated at "tiny" with four
/// seats would be a different map with the same name. So a scenario stays in
/// the matrix — it is exercised on both shapes and both climates like
/// everything else — at the one configuration it has.
fn shape_of(script: MapScript, fallback: (i32, i32), majors: usize) -> ((i32, i32), usize) {
    match BATTLEFIELD_SIZES
        .iter()
        .find(|size| size.script == script && script.is_scenario())
    {
        Some(size) => ((size.width, size.height), 2),
        None => (fallback, majors),
    }
}

/// Shape, climate and map script are independent setup choices. Keep every map
/// type in the catalogue on both generation paths, including fixed-geography
/// Earth, under both thermal distributions; adding a type automatically adds it
/// to this matrix, and the climate list is `MAP_POLES` itself so adding or
/// retiring a climate does too.
#[test]
fn every_world_type_generates_a_playable_world_on_either_shape_and_climate() {
    let rules = Rules::embedded();
    let size = CIV6_MAP_SIZES
        .iter()
        .find(|size| size.id == "tiny")
        .unwrap();

    for (script_index, spec) in CIV6_MAP_SCRIPTS.iter().enumerate() {
        // Tactics maps are in the matrix with their own promises: the bounded
        // Battlefield refuses the globe, while the Planet entry keeps it, and
        // both seat no city-states whatever the caller asked for.
        let ((width, height), majors) =
            shape_of(spec.script, (size.width, size.height), size.default_players);
        let seats = if spec.script.is_battlefield() {
            majors
        } else {
            majors + size.default_city_states
        };
        for (shape_index, shape) in [MapTopology::Flat, MapTopology::Planet]
            .into_iter()
            .enumerate()
        {
            for (poles_index, poles) in MAP_POLES.into_iter().map(|spec| spec.poles).enumerate() {
                let seed = 71_000
                    + 101 * script_index as u64
                    + 7 * shape_index as u64
                    + poles_index as u64;
                let mut rng = Rng::new(seed);
                let (world, spawns) = generate_with_script(
                    &rules,
                    width,
                    height,
                    majors,
                    size.default_city_states,
                    size.natural_wonders,
                    size.continents,
                    spec.script,
                    shape,
                    poles,
                    &mut rng,
                );
                let case = format!("{} / {} / {}", spec.id, shape.id(), poles.id());
                let shape_built = if spec.script.is_planet_battlefield() {
                    MapTopology::Planet
                } else if spec.script.is_battlefield() {
                    MapTopology::Flat
                } else {
                    shape
                };

                match shape_built {
                    MapTopology::Flat => {
                        // A world's flat map is a cylinder: its east and west
                        // edges are the same edge. An arena's is a bounded
                        // rectangle with a wall on all four sides, which is
                        // what stops a unit walking off the field.
                        assert_eq!(
                            world.topology,
                            if spec.script.is_battlefield() {
                                Topology::Rectangle
                            } else {
                                Topology::Cylinder
                            },
                            "{case} shape"
                        );
                        assert_eq!(
                            (world.width, world.height),
                            (width, height),
                            "{case} used planet-map storage dimensions"
                        );
                        assert_eq!(
                            world.tiles.len(),
                            (width * height) as usize,
                            "{case} did not cover the whole rectangle"
                        );
                    }
                    MapTopology::Planet => {
                        assert_eq!(
                            world.topology,
                            Topology::Globe(size.globe_frequency),
                            "{case} did not build the requested globe"
                        );
                        assert_eq!(
                            (world.width, world.height),
                            (size.globe_width(), size.globe_height()),
                            "{case} used flat-map storage dimensions"
                        );
                        assert_eq!(
                            world.tiles.len(),
                            (10 * size.globe_frequency * size.globe_frequency + 2) as usize,
                            "{case} did not cover the whole sphere"
                        );
                    }
                }

                let mut pentagons = 0;
                for (position, _) in world.tiles.iter() {
                    let neighbors = world.neighbors(*position);
                    if shape_built == MapTopology::Planet {
                        match neighbors.len() {
                            5 => pentagons += 1,
                            6 => {}
                            degree => panic!("{case}: {position:?} has {degree} neighbours"),
                        }
                    }
                    for neighbor in neighbors {
                        assert!(
                            world.tiles.contains_key(&neighbor),
                            "{case}: {position:?} points outside the world to {neighbor:?}"
                        );
                        assert!(
                            world.neighbors(neighbor).contains(position),
                            "{case}: adjacency between {position:?} and {neighbor:?} is one-way"
                        );
                    }
                }
                assert_eq!(
                    pentagons,
                    if shape_built == MapTopology::Planet {
                        12
                    } else {
                        0
                    },
                    "{case} has the wrong number of pentagons"
                );

                let land = world
                    .tiles
                    .values()
                    .filter(|tile| !rules.is_water(tile))
                    .count();
                assert!(land > 0, "{case} generated no land");
                if spec.script == civvis::setup::MapScript::Battlefield {
                    // The arena is the one entry here that is not a world:
                    // every hex of it is ground both sides can walk, because
                    // a lake on a field a dozen tiles across is not terrain
                    // to fight over, it is a wall that decides the fight.
                    assert_eq!(land, world.tiles.len(), "{case} put water on an arena");
                } else {
                    assert!(land < world.tiles.len(), "{case} generated no water");
                }
                assert_eq!(spawns.len(), seats, "{case} did not seat every player");
                assert_eq!(
                    spawns.iter().copied().collect::<BTreeSet<_>>().len(),
                    seats,
                    "{case} reused a starting tile"
                );
                for start in &spawns {
                    let tile = world
                        .get(*start)
                        .unwrap_or_else(|| panic!("{case}: start {start:?} is outside the world"));
                    // A start is where a Settler is put down, so on a world it
                    // has to be dry. A naval scenario seats fleets instead,
                    // and its two seats are the flagships' own water.
                    assert_eq!(
                        rules.is_water(tile),
                        spec.script.is_scenario(),
                        "{case}: start {start:?} is on the wrong element"
                    );
                    assert!(
                        rules.is_passable(tile),
                        "{case}: start {start:?} is impassable"
                    );
                }
                for (left_index, left) in spawns.iter().enumerate() {
                    for right in &spawns[left_index + 1..] {
                        let forward = world.distance(*left, *right);
                        assert!(forward > 0, "{case}: distinct starts have zero distance");
                        assert_eq!(
                            forward,
                            world.distance(*right, *left),
                            "{case}: world distance is asymmetric"
                        );
                    }
                }

                // Randomized heat keeps cold terrain but takes away the cold
                // *ends*: no world type grows an ice cap, on either shape.
                if poles == MapPoles::Randomized {
                    for tile in world.tiles.values() {
                        assert!(
                            tile.feature.as_deref() != Some("ice"),
                            "{case}: a randomized world grew a polar ice cap at {:?}",
                            tile.pos
                        );
                    }
                }
            }
        }
    }
}

/// Exercise the public setup path as well as raw generation. This catches a
/// lobby/constructor regression that a map-generator-only test would miss,
/// such as dropping Planet at a process boundary or forcing Earth back onto a
/// globe after the lobby selected Flat.
#[test]
fn every_world_type_starts_a_game_on_either_shape() {
    let size = CIV6_MAP_SIZES
        .iter()
        .find(|size| size.id == "duel")
        .unwrap();

    for (index, spec) in CIV6_MAP_SCRIPTS.iter().enumerate() {
        for shape in [MapTopology::Flat, MapTopology::Planet] {
            let ((width, height), majors) =
                shape_of(spec.script, size.dimensions(shape), size.default_players);
            let game = Game::new_with(GameOptions {
                barbarians: false,
                map_script: spec.script,
                map_topology: shape,
                map_poles: MapPoles::Poles,
                ..GameOptions::new(
                    majors,
                    width,
                    height,
                    81_000 + 2 * index as u64 + shape.is_globe() as u64,
                    20,
                    size.default_city_states,
                )
            });
            let case = format!("{} / {}", spec.id, shape.id());

            assert_eq!(game.map_script, spec.script, "{case} was replaced at setup");
            assert_eq!(
                game.map.topology,
                // The bounded Battlefield is the scripted exception to shape
                // independence; the Tactics Planet intentionally remains a
                // globe so its cities can face each other around the world.
                if spec.script.is_planet_battlefield() {
                    Topology::Globe(size.globe_frequency)
                } else if spec.script.is_battlefield() {
                    Topology::Rectangle
                } else if shape.is_globe() {
                    Topology::Globe(size.globe_frequency)
                } else {
                    Topology::Cylinder
                },
                "{case} did not start on the selected shape"
            );
            assert_eq!(
                game.players
                    .iter()
                    .filter(|player| !player.is_minor && !player.is_barbarian)
                    .count(),
                majors,
                "{case} lost a major player during setup"
            );
            assert_eq!(
                game.players
                    .iter()
                    .filter(|player| player.is_minor && !player.is_barbarian)
                    .count(),
                // The arena refuses the city-states it was asked for; every
                // world seats them all.
                if spec.script.is_battlefield() {
                    0
                } else {
                    size.default_city_states
                },
                "{case} lost a city-state during setup"
            );
            for unit in game.units.values() {
                assert!(
                    game.map.tiles.contains_key(&unit.pos),
                    "{case}: starting unit {} is outside the world",
                    unit.id
                );
                assert!(
                    game.rules.is_passable(&game.map.tiles[&unit.pos]),
                    "{case}: starting unit {} is on impassable terrain",
                    unit.id
                );
            }
            assert!(
                !game.legal_actions(game.current).is_empty(),
                "{case} starts with no legal action"
            );
        }
    }
}
