use std::collections::BTreeSet;

use civvis::game::{Game, GameOptions};
use civvis::mapgen::generate_with_script;
use civvis::rng::Rng;
use civvis::rules::Rules;
use civvis::setup::{MapPoles, MapTopology, CIV6_MAP_SCRIPTS, CIV6_MAP_SIZES};
use civvis::world::Topology;

/// "Planet" is a world shape, not a map script. Keep every map type in the
/// setup catalogue on the same closed-world generation path, including the
/// warm no-poles variant; adding a type to the catalogue automatically adds
/// it to this matrix.
#[test]
fn every_world_type_generates_a_playable_planet_with_either_climate() {
    let rules = Rules::embedded();
    let size = CIV6_MAP_SIZES
        .iter()
        .find(|size| size.id == "tiny")
        .unwrap();
    let seats = size.default_players + size.default_city_states;

    for (script_index, spec) in CIV6_MAP_SCRIPTS.iter().enumerate() {
        for (poles_index, poles) in [MapPoles::Poles, MapPoles::NoPoles].into_iter().enumerate() {
            let seed = 71_000 + 101 * script_index as u64 + poles_index as u64;
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
                MapTopology::Planet,
                poles,
                &mut rng,
            );
            let case = format!("{} / {}", spec.id, poles.id());

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

            let mut pentagons = 0;
            for (position, _) in world.tiles.iter() {
                let neighbors = world.neighbors(*position);
                match neighbors.len() {
                    5 => pentagons += 1,
                    6 => {}
                    degree => panic!("{case}: {position:?} has {degree} neighbours"),
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
                pentagons, 12,
                "{case} did not close as an icosahedral globe"
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
                    .unwrap_or_else(|| panic!("{case}: start {start:?} is outside the globe"));
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
                        "{case}: globe distance is asymmetric"
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

/// Exercise the public setup path as well as raw generation. This catches a
/// lobby/constructor regression that a map-generator-only test would miss,
/// such as silently replacing Planet with Flat for one script.
#[test]
fn every_world_type_starts_a_game_on_planet() {
    let size = CIV6_MAP_SIZES
        .iter()
        .find(|size| size.id == "duel")
        .unwrap();

    for (index, spec) in CIV6_MAP_SCRIPTS.iter().enumerate() {
        let game = Game::new_with(GameOptions {
            barbarians: false,
            map_script: spec.script,
            map_topology: MapTopology::Planet,
            map_poles: MapPoles::Poles,
            ..GameOptions::new(
                size.default_players,
                size.width,
                size.height,
                81_000 + index as u64,
                20,
                size.default_city_states,
            )
        });
        let case = spec.id;

        assert_eq!(game.map_script, spec.script, "{case} was replaced at setup");
        assert_eq!(
            game.map.topology,
            Topology::Globe(size.globe_frequency),
            "{case} did not start on Planet"
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
                "{case}: starting unit {} is outside the globe",
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
