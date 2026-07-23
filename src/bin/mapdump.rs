//! Prints a generated map, and the composition numbers behind it.
//!
//! Map generation is the one system whose defects are obvious to a player and
//! invisible to a test suite: a map can satisfy every invariant and still look
//! like static. This renders a world as text and reports the shares that Civ
//! VI's own generator targets, so a change can be judged by eye and by number.
//!
//! Usage: mapdump [--seed N] [--width N] [--height N] [--script pangaea|
//!                 continents|small_continents|inland_sea] [--maps N] [--quiet]
use std::collections::BTreeMap;

use civvis::rng::Rng;
use civvis::rules::Rules;
use civvis::setup::MapScript;
use civvis::{hex, mapgen};

fn number(args: &[String], flag: &str, default: i64) -> i64 {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let seed = number(&args, "--seed", 1) as u64;
    let width = number(&args, "--width", 60) as i32;
    let height = number(&args, "--height", 38) as i32;
    let maps = number(&args, "--maps", 1) as u64;
    let players = number(&args, "--players", 4) as usize;
    let city_states = number(&args, "--city-states", 6) as usize;
    let quiet = args.iter().any(|arg| arg == "--quiet");
    let script = match args
        .iter()
        .position(|arg| arg == "--script")
        .and_then(|index| args.get(index + 1))
        .map(String::as_str)
    {
        Some("continents") => MapScript::Continents,
        Some("small_continents") => MapScript::SmallContinents,
        Some("inland_sea") => MapScript::InlandSea,
        _ => MapScript::Pangaea,
    };
    let rules = Rules::embedded();

    for map in 0..maps {
        let mut rng = Rng::new(seed + map);
        let (world, spawns) = mapgen::generate_with_script(
            &rules,
            width,
            height,
            players,
            city_states,
            3,
            2,
            script,
            &mut rng,
        );

        if !quiet {
            println!("--- seed {} {script:?} {width}x{height}", seed + map);
            for row in 0..height {
                let mut line = String::new();
                if row % 2 == 1 {
                    line.push(' ');
                }
                for col in 0..width {
                    let pos = hex::offset_to_axial(col, row);
                    let tile = &world.tiles[&pos];
                    let glyph = match (tile.terrain.as_str(), tile.feature.as_deref()) {
                        (_, Some("ice")) => '*',
                        (_, Some("reef")) => ':',
                        ("ocean", _) => ' ',
                        ("coast", _) => '.',
                        ("mountain", _) => 'A',
                        (_, Some("jungle")) => 'J',
                        (_, Some("forest")) => 'f',
                        (_, Some("marsh")) => 'm',
                        (_, Some("oasis")) => 'o',
                        (_, Some(floodplain)) if floodplain.contains("floodplains") => 'w',
                        ("desert", _) => 'd',
                        ("plains", _) => 'p',
                        ("grassland", _) => 'g',
                        ("tundra", _) => 't',
                        ("snow", _) => 's',
                        _ => '?',
                    };
                    let glyph = if spawns.contains(&pos) {
                        '@'
                    } else if tile.hills && glyph.is_lowercase() {
                        glyph.to_ascii_uppercase()
                    } else {
                        glyph
                    };
                    line.push(glyph);
                    line.push(' ');
                }
                println!("{}", line.trim_end());
            }
        }

        let separations: Vec<i32> = spawns
            .iter()
            .enumerate()
            .map(|(index, start)| {
                spawns
                    .iter()
                    .enumerate()
                    .filter(|(other, _)| *other != index)
                    .map(|(_, other)| world.distance(*start, *other))
                    .min()
                    .unwrap_or(0)
            })
            .collect();
        println!(
            "spawns {:?} nearest-neighbour separations {separations:?}",
            spawns
                .iter()
                .map(|pos| hex::axial_to_offset(pos.0, pos.1))
                .collect::<Vec<_>>()
        );

        let mut terrain: BTreeMap<&str, usize> = BTreeMap::new();
        let mut feature: BTreeMap<&str, usize> = BTreeMap::new();
        let (mut land, mut hills, mut water, mut coast) = (0, 0, 0, 0);
        for tile in world.tiles.values() {
            if rules.is_water(tile) {
                water += 1;
                if tile.terrain == "coast" {
                    coast += 1;
                }
            } else {
                land += 1;
                *terrain.entry(tile.terrain.as_str()).or_default() += 1;
                if tile.hills {
                    hills += 1;
                }
            }
            if let Some(name) = tile.feature.as_deref() {
                *feature.entry(name).or_default() += 1;
            }
        }
        let share = |count: usize, total: usize| count * 100 / total.max(1);
        println!(
            "land {land} ({}% of map)  hills {}%  shallow water {}% of ocean",
            share(land, land + water),
            share(hills, land),
            share(coast, water),
        );
        let terrain_line: Vec<String> = terrain
            .iter()
            .map(|(name, count)| format!("{name} {}%", share(*count, land)))
            .collect();
        println!("terrain: {}", terrain_line.join("  "));
        let feature_line: Vec<String> = feature
            .iter()
            .map(|(name, count)| format!("{name} {}%", share(*count, land)))
            .collect();
        println!("features (share of land): {}", feature_line.join("  "));

        // How clustered the map reads: the share of tiles of each kind that
        // have at least two same-kind neighbours. Independent per-tile rolls
        // sit near the band's own share; regions sit far above it.
        for kind in ["mountain", "desert", "coast"] {
            let mut total = 0;
            let mut clustered = 0;
            for (pos, tile) in &world.tiles {
                let matches = |t: &civvis::world::Tile| match kind {
                    "coast" => t.terrain == "coast",
                    other => t.terrain == other,
                };
                if !matches(tile) {
                    continue;
                }
                total += 1;
                let neighbors = world
                    .neighbors(*pos)
                    .into_iter()
                    .filter(|neighbor| world.get(*neighbor).is_some_and(matches))
                    .count();
                if neighbors >= 2 {
                    clustered += 1;
                }
            }
            println!(
                "{kind}: {total} tiles, {}% in clusters",
                share(clustered, total)
            );
        }
    }
}
