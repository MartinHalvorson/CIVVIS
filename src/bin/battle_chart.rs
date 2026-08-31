//! Print a historical battle's chart as text, for checking a terrain plan
//! against the history without opening a browser.
//!
//! ```text
//! cargo run --features developer-tools --bin battle_chart -- thermopylae
//! cargo run --features developer-tools --bin battle_chart            # every drawn battle
//! ```
//!
//! The legend is one character per tile, chosen so a field reads at a glance:
//! `~` sea, `≈` ocean, `^` mountain, `n` hills, `♣` forest, `,` marsh,
//! `.` grassland, `:` plains, `_` desert, `#` an improvement, `1`/`2` the two
//! sides' opening positions.

use civvis::hex;
use civvis::historical_scenarios;
use civvis::historical_terrain;
use civvis::setup::{MapPoles, MapTopology};

fn glyph(tile: &civvis::world::Tile) -> char {
    if tile.improvement.is_some() {
        return '#';
    }
    match tile.terrain.as_str() {
        "ocean" => '≈',
        "coast" | "lake" => '~',
        "mountain" => '^',
        _ => match tile.feature.as_deref() {
            Some("forest") | Some("jungle") => '♣',
            Some("marsh") => ',',
            Some("floodplains") | Some("grassland_floodplains") | Some("plains_floodplains") => 'v',
            Some("oasis") => 'o',
            Some("geothermal_fissure") => '*',
            Some("reef") => '"',
            _ if tile.hills => 'n',
            Some(_) => '?',
            None => match tile.terrain.as_str() {
                "desert" => '_',
                "plains" => ':',
                "tundra" | "snow" => '-',
                _ => '.',
            },
        },
    }
}

fn draw(id: &str) {
    let Some(scenario) = historical_scenarios::by_id(id) else {
        eprintln!("no catalogue battle called {id}");
        return;
    };
    let Some(plan) = historical_terrain::by_id(id) else {
        eprintln!("{id} has no drawn chart");
        return;
    };
    let rules = civvis::rules::Rules::embedded();
    let mut rng = civvis::rng::Rng::new(7);
    let (map, _) = civvis::mapgen::generate_with_script(
        &rules,
        scenario.width,
        scenario.height,
        2,
        0,
        0,
        1,
        historical_scenarios::script_from_id(id).expect("map script"),
        MapTopology::Flat,
        MapPoles::Poles,
        &mut rng,
    );
    let afloat = historical_terrain::sides_afloat(&rules, scenario);
    let starts = historical_terrain::major_starts(&map, plan, afloat).unwrap_or_default();
    println!(
        "\n{} — {} ({})",
        scenario.name, scenario.location, scenario.date
    );
    println!("  {}", scenario.map);
    println!(
        "  {} vs {}",
        scenario.forces[0].label, scenario.forces[1].label
    );
    for row in 0..map.height {
        let mut line = String::new();
        // Offset rows are staggered on a hex chart; the half-space keeps the
        // printed picture in the same proportions as the drawn one.
        if row % 2 == 1 {
            line.push(' ');
        }
        for col in 0..map.width {
            let pos = hex::offset_to_axial(col, row);
            let mark = match map.get(pos) {
                None => ' ',
                Some(tile) => {
                    if starts.first() == Some(&pos) {
                        '1'
                    } else if starts.get(1) == Some(&pos) {
                        '2'
                    } else {
                        glyph(tile)
                    }
                }
            };
            line.push(mark);
            line.push(' ');
        }
        println!("  {line}");
    }
    let water = map
        .tiles
        .values()
        .filter(|tile| matches!(tile.terrain.as_str(), "coast" | "ocean" | "lake"))
        .count();
    let rough = map
        .tiles
        .values()
        .filter(|tile| tile.terrain.as_str() == "mountain")
        .count();
    println!(
        "  {}x{} · {water} water · {rough} mountain · {} tiles",
        map.width,
        map.height,
        map.tiles.len()
    );
}

fn main() {
    let wanted: Vec<String> = std::env::args().skip(1).collect();
    if wanted.is_empty() {
        for scenario in historical_scenarios::generic_scenarios() {
            if historical_terrain::by_id(scenario.id).is_some() {
                draw(scenario.id);
            }
        }
    } else {
        for id in wanted {
            draw(&id);
        }
    }
}
