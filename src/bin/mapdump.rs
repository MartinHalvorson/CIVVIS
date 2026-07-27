//! Prints a generated map, and the composition numbers behind it.
//!
//! Map generation is the one system whose defects are obvious to a player and
//! invisible to a test suite: a map can satisfy every invariant and still look
//! like static. This renders a world as text and reports the shares that Civ
//! VI's own generator targets, so a change can be judged by eye and by number.
//!
//! Usage: mapdump [--seed N] [--width N] [--height N]
//!                 [--script land_only|lakes|inland_sea|grand_canals|pangaea|
//!                  continents|small_continents|islands|water_world|
//!                  true_start_earth]
//!                 [--shape flat|planet] [--poles poles|no_poles|randomized]
//!                 [--maps N] [--quiet]
//!
//! `--shape planet` is a globe, and its rectangle is the storage the sphere is
//! laid out in rather than a picture of the world: a row is not a parallel of
//! latitude and the two rows holding the poles are one tile wide. The shares
//! below are counted over the tiles themselves, so they read the same either
//! way — which is what makes the same world type comparable across the two
//! shapes.
use std::collections::BTreeMap;

use civvis::rng::Rng;
use civvis::rules::Rules;
use civvis::setup::{MapPoles, MapScript, MapTopology};
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
    let start_quality = args.iter().any(|arg| arg == "--start-quality");
    let text = |flag: &str, default: &str| -> String {
        args.iter()
            .position(|arg| arg == flag)
            .and_then(|index| args.get(index + 1))
            .cloned()
            .unwrap_or_else(|| default.to_string())
    };
    let requested = text("--script", "pangaea");
    let script = MapScript::from_id(&requested).unwrap_or(MapScript::Pangaea);
    // `--script planet` named a world type before the globe became a shape of
    // its own, and still asks for both halves of what it used to mean.
    let default_shape = if requested == "planet" {
        MapTopology::Planet
    } else {
        MapTopology::Flat
    };
    let topology =
        MapTopology::from_id(&text("--shape", default_shape.id())).unwrap_or(default_shape);
    let poles = MapPoles::from_id(&text("--poles", "poles")).unwrap_or_default();
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
            topology,
            poles,
            &mut rng,
        );

        if !quiet {
            println!(
                "--- seed {} {script:?} {}x{} ({} tiles)",
                seed + map,
                world.width,
                world.height,
                world.tiles.len()
            );
            for row in 0..world.height {
                let mut line = String::new();
                if row % 2 == 1 {
                    line.push(' ');
                }
                for col in 0..world.width {
                    let pos = hex::offset_to_axial(col, row);
                    let Some(tile) = world.get(pos) else {
                        line.push_str("  ");
                        continue;
                    };
                    let glyph = match (tile.terrain.as_str(), tile.feature.as_deref()) {
                        (_, Some("ice")) => '*',
                        (_, Some("reef")) => ':',
                        ("ocean", _) => ' ',
                        ("coast", _) => '.',
                        ("lake", _) => '~',
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
        if start_quality {
            // The land a capital actually works, measured independently of the
            // generator's own `start_quality` scorer: the best twelve tiles a
            // city can reach at radius 3, by raw yield total. Using the
            // generator's scorer here would flatter the generator, since that
            // is the number it optimizes.
            let worked: Vec<f64> = spawns
                .iter()
                .take(players)
                .map(|start| {
                    let mut tiles: Vec<f64> = world
                        .tiles
                        .iter()
                        .filter(|(pos, _)| world.distance(**pos, *start) <= 3)
                        .map(|(_, tile)| rules.tile_yields(tile).total())
                        .collect();
                    tiles.sort_by(|left, right| right.total_cmp(left));
                    tiles.iter().take(12).sum()
                })
                .collect();
            let best = worked.iter().copied().fold(f64::MIN, f64::max);
            let worst = worked.iter().copied().fold(f64::MAX, f64::min);
            let mean = worked.iter().sum::<f64>() / worked.len().max(1) as f64;
            println!(
                "start quality (best twelve worked tiles) mean {mean:.1} best {best:.1} worst {worst:.1} spread {:.1} ({:.1}% of mean)",
                best - worst,
                100.0 * (best - worst) / mean.max(1.0)
            );

            // Minor placement against the shipped START_DISTANCE_MINOR_*
            // targets: 6 from a major, 5 from another city-state.
            let majors: Vec<_> = spawns.iter().take(players).copied().collect();
            let minors: Vec<_> = spawns.iter().skip(players).copied().collect();
            let mut to_major = Vec::new();
            let mut to_minor = Vec::new();
            for (index, minor) in minors.iter().enumerate() {
                if let Some(d) = majors
                    .iter()
                    .map(|m| hex::wdistance(*minor, *m, world.width))
                    .min()
                {
                    to_major.push(d);
                }
                if let Some(d) = minors
                    .iter()
                    .enumerate()
                    .filter(|(other, _)| *other != index)
                    .map(|(_, m)| hex::wdistance(*minor, *m, world.width))
                    .min()
                {
                    to_minor.push(d);
                }
            }
            let mean_of = |v: &Vec<i32>| {
                if v.is_empty() {
                    0.0
                } else {
                    v.iter().sum::<i32>() as f64 / v.len() as f64
                }
            };
            println!(
                "minor distance to nearest major mean {:.1} min {} max {} | to nearest minor mean {:.1} min {} max {}",
                mean_of(&to_major),
                to_major.iter().min().copied().unwrap_or(0),
                to_major.iter().max().copied().unwrap_or(0),
                mean_of(&to_minor),
                to_minor.iter().min().copied().unwrap_or(0),
                to_minor.iter().max().copied().unwrap_or(0)
            );
        }

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
                let neighbors = hex::neighbors(*pos)
                    .into_iter()
                    .map(|neighbor| hex::canon(neighbor, world.width))
                    .filter(|neighbor| world.get(*neighbor).is_some_and(matches))
                    .count();
                if neighbors >= 2 {
                    clustered += 1;
                }
            }
            println!("{kind}: {total} tiles, {}% in clusters", share(clustered, total));
        }
    }
}
