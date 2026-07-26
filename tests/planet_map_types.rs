use std::collections::BTreeSet;

use civvis::game::{Game, GameOptions};
use civvis::mapgen::generate_with_script;
use civvis::rng::Rng;
use civvis::rules::Rules;
use civvis::setup::{MapPoles, MapTopology, CIV6_MAP_SCRIPTS, CIV6_MAP_SIZES};
use civvis::world::Topology;

/// Shape, climate and map script are independent setup choices. Keep every map
/// type in the catalogue on both generation paths, including fixed-geography
/// Earth and the warm no-poles variant; adding a type automatically adds it to
/// this matrix.
#[test]
fn every_world_type_generates_a_playable_world_on_either_shape_and_climate() {
    let rules = Rules::embedded();
    let size = CIV6_MAP_SIZES
        .iter()
        .find(|size| size.id == "tiny")
        .unwrap();
    let seats = size.default_players + size.default_city_states;

    for (script_index, spec) in CIV6_MAP_SCRIPTS.iter().enumerate() {
        for (shape_index, shape) in [MapTopology::Flat, MapTopology::Planet]
            .into_iter()
            .enumerate()
        {
            for (poles_index, poles) in [MapPoles::Poles, MapPoles::NoPoles].into_iter().enumerate()
            {
                let seed = 71_000
                    + 101 * script_index as u64
                    + 7 * shape_index as u64
                    + poles_index as u64;
                let mut rng = Rng::new(seed);
                let (world, spawns) = generate_with_script(
                    &rules,
                    size.width,
                    size.height,
                    size.default_players,
                    size.default_city_states,
                    size.natural_wonders,
                    size.continents,
                    spec.script,
                    shape,
                    poles,
                    &mut rng,
                );
                let case = format!("{} / {} / {}", spec.id, shape.id(), poles.id());

                match shape {
                    MapTopology::Flat => {
                        assert_eq!(world.topology, Topology::Cylinder, "{case} shape");
                        assert_eq!(
                            (world.width, world.height),
                            (size.width, size.height),
                            "{case} used planet-map storage dimensions"
                        );
                        assert_eq!(
                            world.tiles.len(),
                            (size.width * size.height) as usize,
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
                    if shape == MapTopology::Planet {
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
                    if shape == MapTopology::Planet { 12 } else { 0 },
                    "{case} has the wrong number of pentagons"
                );

                let land = world
                    .tiles
                    .values()
                    .filter(|tile| !rules.is_water(tile))
                    .count();
                assert!(land > 0, "{case} generated no land");
                assert!(land < world.tiles.len(), "{case} generated no water");
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
                    assert!(!rules.is_water(tile), "{case}: start {start:?} is in water");
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

                if poles == MapPoles::NoPoles {
                    for tile in world.tiles.values() {
                        assert!(
                            !matches!(tile.terrain.as_str(), "snow" | "tundra")
                                && tile.feature.as_deref() != Some("ice"),
                            "{case}: a no-poles world contains polar terrain at {:?}",
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
            let (width, height) = size.dimensions(shape);
            let game = Game::new_with(GameOptions {
                barbarians: false,
                map_script: spec.script,
                map_topology: shape,
                map_poles: MapPoles::Poles,
                ..GameOptions::new(
                    size.default_players,
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
                if shape.is_globe() {
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
                size.default_players,
                "{case} lost a major player during setup"
            );
            assert_eq!(
                game.players
                    .iter()
                    .filter(|player| player.is_minor && !player.is_barbarian)
                    .count(),
                size.default_city_states,
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
