//! Map generation (mirrors civvis/mapgen.py).
use std::collections::{BTreeMap, BTreeSet};

use crate::fractal::Fractal;
use crate::rng::Rng;
use crate::rules::Rules;
use crate::setup::MapScript;
use crate::world::WorldMap;
use crate::{hex, Pos};

/// `GlobalParameters.LAKE_PLOT_RANDOM`: one eligible inland plot in forty
/// floods when the stock generator adds lakes.
const LAKE_PLOT_RANDOM: usize = 40;

/// `GlobalParameters.LAKE_MAX_AREA_SIZE`: the whole of Civ VI's distinction
/// between the two kinds of enclosed water. A body of at most nine plots is a
/// **lake** — fresh water for every tile around it, a Fishery, and the site
/// Huey Teocalli has to be built on. A larger one is an **inland sea**: Coast
/// that ships can sail and work, and that no city can drink from.
const LAKE_MAX_AREA_SIZE: usize = 9;

/// The share of the world `Lakes.lua` leaves as land. It stacks three fractal
/// layers at 81, 88 and 95 percent, but `Adjacent` bars every layer after the
/// first from any plot an earlier one made land *or* borders, so only the first
/// cut really decides the world: land nearly everywhere, and the driest fifth
/// of the field left behind as scattered interior basins.
const LAKES_SCRIPT_LAND_PERCENT: u32 = 81;

fn offset_region(
    wm: &WorldMap,
    col_start: i32,
    col_end: i32,
    row_start: i32,
    row_end: i32,
) -> BTreeSet<Pos> {
    (row_start.max(0)..row_end.min(wm.height))
        .flat_map(|row| {
            (col_start.max(0)..col_end.min(wm.width)).map(move |col| hex::offset_to_axial(col, row))
        })
        .filter(|pos| wm.tiles.contains_key(pos))
        .collect()
}

/// Grow a single guaranteed-connected landmass inside an allowed region.
fn grow_blob(
    wm: &WorldMap,
    allowed: &BTreeSet<Pos>,
    seed: Pos,
    target: usize,
    rng: &mut Rng,
) -> BTreeSet<Pos> {
    if !allowed.contains(&seed) || target == 0 {
        return BTreeSet::new();
    }
    let mut land = BTreeSet::from([seed]);
    let mut frontier = vec![seed];
    for _ in 0..(50 * wm.width * wm.height) {
        if land.len() >= target.min(allowed.len()) || frontier.is_empty() {
            break;
        }
        let index = rng.below(frontier.len());
        let current = frontier[index];
        let candidates: Vec<Pos> = hex::neighbors(current)
            .into_iter()
            .map(|neighbor| hex::canon(neighbor, wm.width))
            .filter(|neighbor| allowed.contains(neighbor) && !land.contains(neighbor))
            .collect();
        if candidates.is_empty() {
            frontier.swap_remove(index);
            continue;
        }
        let next = candidates[rng.below(candidates.len())];
        land.insert(next);
        frontier.push(next);
        if rng.chance(0.18) {
            frontier.swap_remove(index);
        }
    }
    land
}

fn point_in_polygon(longitude: f64, latitude: f64, polygon: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let mut previous = polygon.len() - 1;
    for current in 0..polygon.len() {
        let (xi, yi) = polygon[current];
        let (xj, yj) = polygon[previous];
        if ((yi > latitude) != (yj > latitude))
            && longitude < (xj - xi) * (latitude - yi) / (yj - yi) + xi
        {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

/// A deliberately low-frequency Earth silhouette. The generator samples these
/// coastlines through the same equal-area latitude bands used by both browser
/// projections: each row spans an equal amount of spherical surface, while its
/// rendered circumference naturally shrinks toward either pole.
fn generate_earth_land(wm: &WorldMap) -> BTreeSet<Pos> {
    const NORTH_AMERICA: &[(f64, f64)] = &[
        (-168.0, 71.0),
        (-142.0, 72.0),
        (-126.0, 59.0),
        (-123.0, 49.0),
        (-105.0, 48.0),
        (-82.0, 25.0),
        (-97.0, 16.0),
        (-112.0, 28.0),
        (-126.0, 43.0),
        (-151.0, 58.0),
        (-168.0, 60.0),
    ];
    const SOUTH_AMERICA: &[(f64, f64)] = &[
        (-81.0, 12.0),
        (-61.0, 11.0),
        (-49.0, 2.0),
        (-35.0, -7.0),
        (-52.0, -35.0),
        (-68.0, -55.0),
        (-76.0, -38.0),
        (-81.0, -5.0),
    ];
    const EURASIA: &[(f64, f64)] = &[
        (-11.0, 36.0),
        (-10.0, 59.0),
        (5.0, 71.0),
        (44.0, 72.0),
        (82.0, 75.0),
        (126.0, 70.0),
        (169.0, 64.0),
        (179.0, 52.0),
        (145.0, 43.0),
        (128.0, 31.0),
        (121.0, 19.0),
        (105.0, 7.0),
        (93.0, 21.0),
        (78.0, 8.0),
        (66.0, 25.0),
        (49.0, 29.0),
        (35.0, 36.0),
        (20.0, 35.0),
        (8.0, 43.0),
    ];
    const AFRICA: &[(f64, f64)] = &[
        (-17.0, 36.0),
        (12.0, 37.0),
        (34.0, 31.0),
        (51.0, 12.0),
        (42.0, -12.0),
        (31.0, -35.0),
        (17.0, -35.0),
        (8.0, -18.0),
        (-10.0, 5.0),
    ];
    const ARABIA_INDIA: &[(f64, f64)] = &[
        (34.0, 31.0),
        (67.0, 29.0),
        (91.0, 24.0),
        (83.0, 7.0),
        (73.0, 8.0),
        (61.0, 25.0),
        (52.0, 13.0),
        (42.0, 14.0),
    ];
    const SOUTHEAST_ASIA: &[(f64, f64)] = &[
        (91.0, 25.0),
        (121.0, 21.0),
        (132.0, 4.0),
        (118.0, -9.0),
        (103.0, -7.0),
        (97.0, 9.0),
    ];
    const AUSTRALIA: &[(f64, f64)] = &[
        (112.0, -11.0),
        (154.0, -10.0),
        (153.0, -39.0),
        (132.0, -44.0),
        (113.0, -34.0),
    ];
    const GREENLAND: &[(f64, f64)] = &[(-73.0, 59.0), (-18.0, 60.0), (-14.0, 82.0), (-54.0, 84.0)];
    const ISLANDS: &[&[(f64, f64)]] = &[
        &[(-10.0, 50.0), (2.0, 51.0), (1.0, 59.0), (-8.0, 58.0)],
        &[(129.0, 31.0), (145.0, 33.0), (146.0, 46.0), (137.0, 43.0)],
        &[(43.0, -12.0), (51.0, -13.0), (50.0, -26.0), (44.0, -25.0)],
        &[
            (166.0, -34.0),
            (179.0, -37.0),
            (178.0, -48.0),
            (168.0, -47.0),
        ],
    ];
    let continents = [
        NORTH_AMERICA,
        SOUTH_AMERICA,
        EURASIA,
        AFRICA,
        ARABIA_INDIA,
        SOUTHEAST_ASIA,
        AUSTRALIA,
        GREENLAND,
    ];
    let mut land = BTreeSet::new();
    for row in 0..wm.height {
        let surface_y = 1.0 - 2.0 * (row as f64 + 0.5) / wm.height as f64;
        let latitude = surface_y.clamp(-1.0, 1.0).asin().to_degrees();
        for col in 0..wm.width {
            let longitude = 360.0 * (col as f64 + 0.5) / wm.width as f64 - 180.0;
            if continents
                .iter()
                .chain(ISLANDS.iter())
                .any(|polygon| point_in_polygon(longitude, latitude, polygon))
            {
                land.insert(hex::offset_to_axial(col, row));
            }
        }
    }
    land
}

fn earth_position(width: i32, height: i32, longitude: f64, latitude: f64) -> Pos {
    let col = (((longitude + 180.0) / 360.0 * width as f64).floor() as i32).clamp(0, width - 1);
    let surface_y = latitude.to_radians().sin();
    let row = (((1.0 - surface_y) * 0.5 * height as f64).floor() as i32).clamp(0, height - 1);
    hex::offset_to_axial(col, row)
}

fn normalized_latitude(row: i32, height: i32, equal_area: bool) -> f64 {
    if equal_area {
        let surface_y = 1.0 - 2.0 * (row as f64 + 0.5) / height.max(1) as f64;
        surface_y.clamp(-1.0, 1.0).asin().abs() / std::f64::consts::FRAC_PI_2
    } else {
        (2.0 * row as f64 / (height - 1).max(1) as f64 - 1.0).abs()
    }
}

/// CIV_NAMES is ordered Rome, Egypt, Greece, China, Sumeria, Aztec, Nubia,
/// Scythia. Preserve that order here so a True Start script is true in play,
/// not only shaped like Earth in the setup preview.
fn true_start_major_spawns(wm: &WorldMap, candidates: &[Pos], count: usize) -> Vec<Pos> {
    const HOMELANDS: [(f64, f64); 8] = [
        (12.5, 41.9),
        (31.2, 30.0),
        (23.7, 38.0),
        (116.4, 39.9),
        (44.4, 32.5),
        (-99.1, 19.4),
        (32.5, 19.6),
        (64.0, 48.0),
    ];
    let mut available = candidates.to_vec();
    let mut starts = Vec::new();
    for index in 0..count {
        if available.is_empty() {
            break;
        }
        let (longitude, latitude) = HOMELANDS[index % HOMELANDS.len()];
        let target = earth_position(wm.width, wm.height, longitude, latitude);
        let minimum_separation = (0..=4)
            .rev()
            .find(|separation| {
                available.iter().any(|candidate| {
                    starts
                        .iter()
                        .all(|start| hex::wdistance(*candidate, *start, wm.width) >= *separation)
                })
            })
            .unwrap_or(0);
        let selected = available
            .iter()
            .enumerate()
            .filter(|(_, candidate)| {
                starts.iter().all(|start| {
                    hex::wdistance(**candidate, *start, wm.width) >= minimum_separation
                })
            })
            .min_by_key(|(_, candidate)| hex::wdistance(**candidate, target, wm.width))
            .map(|(candidate_index, _)| candidate_index)
            .unwrap_or(0);
        starts.push(available.swap_remove(selected));
    }
    starts
}

fn generate_land(
    wm: &WorldMap,
    script: MapScript,
    num_major_spawns: usize,
    rng: &mut Rng,
) -> BTreeSet<Pos> {
    let width = wm.width;
    let height = wm.height;
    let area = (width * height) as usize;
    match script {
        MapScript::Pangaea => {
            // A compact oval gives every seat comparable hinterland while
            // retaining a single coast-to-coast supercontinent. The stock
            // scripts cut their coastline out of a fractal rather than a
            // curve, which is what produces bays, peninsulas and the odd
            // offshore island; the oval only decides where the sea level sits.
            let center_col = (width - 1) as f64 / 2.0;
            let center_row = (height - 1) as f64 / 2.0;
            let radius_col = width as f64 * 0.39;
            let radius_row = height as f64 * 0.343;
            let shore = Fractal::new(rng, width, height, 4);
            let mut land = BTreeSet::new();
            for row in 1..height - 1 {
                for col in 0..width {
                    let x = (col as f64 - center_col) / radius_col;
                    let y = (row as f64 - center_row) / radius_row;
                    let ragged = 1.0 + 0.30 * (shore.at(col, row) as f64 / 255.0 - 0.5) * 2.0;
                    if (x * x + y * y).sqrt() <= ragged {
                        land.insert(hex::offset_to_axial(col, row));
                    }
                }
            }
            land
        }
        MapScript::Continents => {
            let gap = (width / 18).max(2);
            let midpoint = width / 2;
            let regions = [
                (gap, midpoint - gap, 2, height - 2),
                (midpoint + gap, width - gap, 2, height - 2),
            ];
            let mut land = BTreeSet::new();
            let per_continent = (area as f64 * 0.21) as usize;
            for (left, right, top, bottom) in regions {
                let allowed = offset_region(wm, left, right, top, bottom);
                let seed = hex::offset_to_axial((left + right) / 2, (top + bottom) / 2);
                land.extend(grow_blob(wm, &allowed, seed, per_continent, rng));
            }
            land
        }
        MapScript::SmallContinents => {
            let count = num_major_spawns.div_ceil(2).clamp(4, 8);
            let columns = if count <= 4 { 2 } else { 3 };
            let rows = count.div_ceil(columns);
            let per_island = ((area as f64 * 0.36) as usize / count).max(12);
            let mut land = BTreeSet::new();
            for index in 0..count {
                let column = index % columns;
                let row = index / columns;
                let left = (column * width as usize / columns) as i32 + 2;
                let right = ((column + 1) * width as usize / columns) as i32 - 2;
                let top = (row * height as usize / rows) as i32 + 2;
                let bottom = ((row + 1) * height as usize / rows) as i32 - 2;
                let allowed = offset_region(wm, left, right, top, bottom);
                let seed = hex::offset_to_axial((left + right) / 2, (top + bottom) / 2);
                land.extend(grow_blob(wm, &allowed, seed, per_island, rng));
            }
            land
        }
        MapScript::InlandSea => {
            let center_col = (width - 1) as f64 / 2.0;
            let center_row = (height - 1) as f64 / 2.0;
            let radius_col = width as f64 * 0.34;
            let radius_row = height as f64 * 0.30;
            let shore = Fractal::new(rng, width, height, 4);
            let mut land = BTreeSet::new();
            for row in 0..height {
                for col in 0..width {
                    let edge = col < 2 || col >= width - 2 || row < 2 || row >= height - 2;
                    let x = (col as f64 - center_col) / radius_col;
                    let y = (row as f64 - center_row) / radius_row;
                    // The same fractal shore, applied to the sea's edge, gives
                    // the basin gulfs and headlands instead of a drawn ellipse.
                    let ragged = 1.0 + 0.26 * (shore.at(col, row) as f64 / 255.0 - 0.5) * 2.0;
                    if edge || (x * x + y * y).sqrt() >= ragged {
                        land.insert(hex::offset_to_axial(col, row));
                    }
                }
            }
            land
        }
        MapScript::Lakes => {
            // The basins this leaves behind are the map's water. They are cut
            // from the field rather than grown, so they arrive in the range of
            // sizes the fractal happens to hold: most of them small enough to
            // be lakes, a few broad enough to be inland seas. Only the poles
            // are kept open, because a world with no sea at all has nowhere for
            // a river to run to and no shelf for polar ice.
            let basin = Fractal::new(rng, width, height, 3);
            let waterline = basin.percentile(LAKES_SCRIPT_LAND_PERCENT);
            let mut land = BTreeSet::new();
            for row in 1..height - 1 {
                for col in 0..width {
                    if basin.at(col, row) < waterline {
                        land.insert(hex::offset_to_axial(col, row));
                    }
                }
            }
            land
        }
        MapScript::TrueStartEarth | MapScript::TrueStartTrueEarth => generate_earth_land(wm),
    }
}

/// How many bodies a script may spread past a single plot.
///
/// `Lakes.lua` asks for four per continent region; every other stock script
/// asks for none and receives the one-plot ponds the same roll produces. The
/// two single-supercontinent scripts are given a budget of their own here,
/// which is a deliberate departure: their interiors are deep enough to hold an
/// inland sea, and a supercontinent whose only water is its own shoreline plays
/// as a flat expanse. The island scripts keep the stock zero — an island has no
/// interior to put a lake in, and the enclosure rule would refuse one anyway.
///
/// True Start Earth is left out of lake generation entirely: its coastline is
/// the real one, and a randomly placed lake in the middle of it would be a
/// worse answer than the real lakes CIVVIS does not yet model.
fn large_lake_budget(script: MapScript, num_continents: usize) -> usize {
    match script {
        MapScript::Lakes => num_continents * 4,
        MapScript::Pangaea | MapScript::InlandSea => num_continents,
        MapScript::Continents => num_continents / 2,
        MapScript::SmallContinents
        | MapScript::TrueStartEarth
        | MapScript::TrueStartTrueEarth => 0,
    }
}

/// Whether a plot may flood, using the stock script's filters.
///
/// A lake plot has to be inland land: not water, not *coastal* land, and clear
/// of rivers and of tiles carrying one. The enclosure is a precondition of
/// placement rather than something checked afterwards, and it is also what
/// keeps two lakes apart — a plot beside one that has already flooded is
/// coastal land by the time the scan reaches it.
fn lake_eligible(wm: &WorldMap, land: &BTreeSet<Pos>, pos: Pos) -> bool {
    if !land.contains(&pos) || wm.tiles.get(&pos).is_none_or(|tile| tile.has_river()) {
        return false;
    }
    hex::neighbors(pos)
        .into_iter()
        .map(|neighbor| hex::canon(neighbor, wm.width))
        .all(|neighbor| match wm.tiles.get(&neighbor) {
            // Off the map is the world's edge, not a shore.
            None => true,
            Some(tile) => land.contains(&neighbor) && !tile.has_river(),
        })
}

/// Flood one plot. The stock script leaves it as Coast and lets the engine
/// decide afterwards whether the body it belongs to is small enough to be a
/// lake, which is what [`classify_lakes`] does here.
///
/// Dropping the plot from `land` is what makes the water real to the rest of
/// generation: every later pass — features, volcanoes, natural wonders,
/// resources, continents, spawns — works from that set, so none of them will
/// plant anything in the new lake or strand a civilization in it.
fn flood_lake_plot(wm: &mut WorldMap, land: &mut BTreeSet<Pos>, pos: Pos) {
    land.remove(&pos);
    let tile = wm.tiles.get_mut(&pos).unwrap();
    tile.terrain = "coast".into();
    // Relief was settled before the rivers ran; water has none.
    tile.hills = false;
}

/// `AddMoreLake`: give the six neighbours of a fresh lake a chance to join it.
///
/// The denominator grows with the body, so the sixth plot is far less likely
/// than the first and a lake stops of its own accord. The stock script counts
/// the attempt against the large-lake budget only when three or more take, but
/// floods whatever it picked either way.
fn spread_lake(wm: &mut WorldMap, land: &mut BTreeSet<Pos>, pos: Pos, rng: &mut Rng) -> bool {
    let mut picked: Vec<Pos> = Vec::new();
    for neighbor in hex::neighbors(pos)
        .into_iter()
        .map(|neighbor| hex::canon(neighbor, wm.width))
    {
        // Eligibility is read before anything floods, so the six are judged
        // against the shore as it stood when the lake was drawn.
        if lake_eligible(wm, land, neighbor) && rng.below(4 + picked.len()) < 3 {
            picked.push(neighbor);
        }
    }
    let grew = picked.len() > 2;
    for neighbor in picked {
        flood_lake_plot(wm, land, neighbor);
    }
    grew
}

/// Civ VI's `AddLakes` (`RiversLakes.lua`), in its stock position in the
/// pipeline: after the rivers, because "lakes would interfere with rivers,
/// causing them to stop and not reach the ocean, if placed any sooner".
fn add_lakes(
    wm: &mut WorldMap,
    land: &mut BTreeSet<Pos>,
    mut large_lakes: usize,
    rng: &mut Rng,
) {
    let (width, height) = (wm.width, wm.height);
    let scan: Vec<Pos> = (0..height)
        .flat_map(|row| (0..width).map(move |col| hex::offset_to_axial(col, row)))
        .collect();
    for pos in scan {
        if !lake_eligible(wm, land, pos) || rng.below(LAKE_PLOT_RANDOM) != 0 {
            continue;
        }
        if large_lakes > 0 && spread_lake(wm, land, pos, rng) {
            large_lakes -= 1;
        }
        flood_lake_plot(wm, land, pos);
    }
}

/// Sort the world's enclosed water into lakes and inland seas by area, the way
/// `AreaBuilder` does once the terrain is settled.
///
/// This runs for every script, not only the ones that add lakes, because a
/// fractal coastline encloses basins of its own: a pocket the sea cannot reach
/// has always been a lake, and until now CIVVIS painted it as open ocean.
fn classify_lakes(wm: &mut WorldMap) {
    let width = wm.width;
    let water: BTreeSet<Pos> = wm
        .tiles
        .iter()
        .filter(|(_, tile)| matches!(tile.terrain.as_str(), "coast" | "ocean"))
        .map(|(pos, _)| *pos)
        .collect();
    for body in connected_components(&water, width) {
        if body.len() > LAKE_MAX_AREA_SIZE {
            continue;
        }
        for pos in body {
            wm.tiles.get_mut(&pos).unwrap().terrain = "lake".into();
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn generate(
    rules: &Rules,
    width: i32,
    height: i32,
    num_major_spawns: usize,
    num_minor_spawns: usize,
    num_natural_wonders: usize,
    num_continents: usize,
    rng: &mut Rng,
) -> (WorldMap, Vec<Pos>) {
    generate_with_script(
        rules,
        width,
        height,
        num_major_spawns,
        num_minor_spawns,
        num_natural_wonders,
        num_continents,
        MapScript::Pangaea,
        rng,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn generate_with_script(
    rules: &Rules,
    width: i32,
    height: i32,
    num_major_spawns: usize,
    num_minor_spawns: usize,
    num_natural_wonders: usize,
    num_continents: usize,
    script: MapScript,
    rng: &mut Rng,
) -> (WorldMap, Vec<Pos>) {
    let mut wm = WorldMap::new(width, height);

    // --- landmass topology selected by the stock-style map script
    let mut land = generate_land(&wm, script, num_major_spawns, rng);

    let land_list: Vec<Pos> = land.iter().cloned().collect();
    let equal_area = matches!(
        script,
        MapScript::TrueStartEarth | MapScript::TrueStartTrueEarth
    );
    let latitude = |pos: Pos| -> f64 {
        let (_, row) = hex::axial_to_offset(pos.0, pos.1);
        normalized_latitude(row, height, equal_area)
    };

    // --- relief, then climate. The stock generator settles elevation first
    // (MountainsCliffs.lua) and only then paints biomes over it, because the
    // mountain fractal has to be free of the latitude bands to run across them.
    apply_tectonics(&mut wm, &land, rng);
    assign_biomes(&mut wm, &land_list, equal_area, rng);

    // --- coast. A shelf is one tile of shallow water plus the stock's three
    // expansion passes, each giving a quarter of the Ocean tiles that already
    // touch shallow water their own turn to become Coast. Shelves therefore
    // vary from one tile in the open sea to five or more in a broad bay,
    // instead of a uniform outline traced around every landmass.
    let coastal: Vec<Pos> = wm
        .tiles
        .iter()
        .filter(|(pos, t)| {
            t.terrain == "ocean"
                && hex::neighbors(**pos)
                    .iter()
                    .any(|n| land.contains(&hex::canon(*n, width)))
        })
        .map(|(pos, _)| *pos)
        .collect();
    for pos in coastal {
        wm.tiles.get_mut(&pos).unwrap().terrain = "coast".into();
    }
    for _ in 0..3 {
        let expansion: Vec<Pos> = wm
            .tiles
            .iter()
            .filter(|(pos, tile)| {
                tile.terrain == "ocean"
                    && hex::neighbors(**pos)
                        .into_iter()
                        .map(|neighbor| hex::canon(neighbor, width))
                        .any(|neighbor| {
                            wm.tiles
                                .get(&neighbor)
                                .is_some_and(|tile| tile.terrain == "coast")
                        })
            })
            .map(|(pos, _)| *pos)
            .collect();
        for pos in expansion {
            if rng.below(4) == 0 {
                wm.tiles.get_mut(&pos).unwrap().terrain = "coast".into();
            }
        }
    }

    // Coastal cliffs are shared edge features rather than tile terrain.
    // Generate from the land side and mirror the edge onto the water tile;
    // this makes embark/disembark legality exact at bays and narrow points.
    let mut cliff_edges = Vec::new();
    for &pos in &land_list {
        if wm.tiles[&pos].terrain == "mountain" {
            continue;
        }
        for neighbor in hex::neighbors(pos)
            .into_iter()
            .map(|neighbor| hex::canon(neighbor, width))
        {
            if wm
                .tiles
                .get(&neighbor)
                .is_some_and(|tile| matches!(tile.terrain.as_str(), "coast" | "ocean"))
                && rng.chance(0.35)
            {
                cliff_edges.push((pos, neighbor));
            }
        }
    }
    for (land, water) in cliff_edges {
        wm.set_cliff_edge(land, water, true);
    }

    // --- rivers: connected chains along shared hex edges, as in Civ VI.
    // Build each river upstream from a guaranteed coastal outlet. Walking the
    // edge graph (rather than the tile-center graph) keeps every consecutive
    // segment joined at a hex corner and never sends a channel through a tile.
    generate_rivers(&mut wm, &land_list, rng);

    // --- lakes and inland seas, in the stock order: the rivers already have
    // their outlets, so nothing that floods now can dam one. `add_lakes` only
    // creates water; `classify_lakes` then sorts every enclosed body on the
    // map — the ones it just made and the ones the coastline enclosed by
    // itself — into lakes and inland seas by area.
    if !matches!(
        script,
        MapScript::TrueStartEarth | MapScript::TrueStartTrueEarth
    ) {
        add_lakes(
            &mut wm,
            &mut land,
            large_lake_budget(script, num_continents),
            rng,
        );
    }
    classify_lakes(&mut wm);
    let land_list: Vec<Pos> = land.iter().cloned().collect();

    // --- tribal villages (goody huts), roughly 1 per 40 land tiles
    for pos in &land_list {
        let t = &wm.tiles[pos];
        if t.terrain == "mountain" || t.has_river() {
            continue;
        }
        if rng.f64() < 0.025 {
            wm.tiles.get_mut(pos).unwrap().improvement = Some("goody_hut".into());
        }
    }

    // --- tectonic and polar features
    // Volcanoes replace a small, well-spaced subset of mountain tiles. Every
    // candidate needs exposed land at its foot so the volcano reads as part of
    // the landscape and can seed the volcanic soil produced by old eruptions.
    let mut volcano_candidates: Vec<Pos> = land_list
        .iter()
        .copied()
        .filter(|position| wm.tiles[position].terrain == "mountain")
        .filter(|position| {
            hex::neighbors(*position)
                .into_iter()
                .map(|neighbor| hex::canon(neighbor, width))
                .any(|neighbor| {
                    wm.tiles.get(&neighbor).is_some_and(|tile| {
                        !matches!(tile.terrain.as_str(), "mountain" | "coast" | "ocean")
                    })
                })
        })
        .collect();
    for index in (1..volcano_candidates.len()).rev() {
        let other = rng.below(index + 1);
        volcano_candidates.swap(index, other);
    }
    let volcano_target = (land_list.len() / 180).max(1);
    let mut volcanoes = Vec::new();
    for position in volcano_candidates {
        if volcanoes.len() >= volcano_target {
            break;
        }
        if volcanoes
            .iter()
            .all(|other| hex::wdistance(position, *other, width) >= 4)
        {
            wm.tiles.get_mut(&position).unwrap().feature = Some("volcano".into());
            volcanoes.push(position);
        }
    }

    // Ancient eruption deposits make volcanoes legible even while dormant.
    // Guarantee one deposit where geography allows, then scatter a few more
    // without consuming the RNG differently for later per-tile feature rolls.
    for volcano in &volcanoes {
        let mut foothills: Vec<Pos> = hex::neighbors(*volcano)
            .into_iter()
            .map(|neighbor| hex::canon(neighbor, width))
            .filter(|neighbor| {
                wm.tiles.get(neighbor).is_some_and(|tile| {
                    !matches!(tile.terrain.as_str(), "mountain" | "coast" | "ocean")
                        && tile.feature.is_none()
                })
            })
            .collect();
        for index in (1..foothills.len()).rev() {
            let other = rng.below(index + 1);
            foothills.swap(index, other);
        }
        for (index, position) in foothills.into_iter().enumerate() {
            if index == 0 || rng.chance(0.28) {
                wm.tiles.get_mut(&position).unwrap().feature = Some("volcanic_soil".into());
            }
        }
    }

    // Fissures follow tectonic relief rather than appearing on arbitrary flat
    // tiles. Spacing them preserves their value as recognizable landmarks.
    let mut fissure_candidates: Vec<Pos> = land_list
        .iter()
        .copied()
        .filter(|position| {
            let tile = &wm.tiles[position];
            tile.terrain != "mountain"
                && tile.feature.is_none()
                && hex::neighbors(*position)
                    .into_iter()
                    .map(|neighbor| hex::canon(neighbor, width))
                    .any(|neighbor| {
                        wm.tiles.get(&neighbor).is_some_and(|neighbor_tile| {
                            neighbor_tile.terrain == "mountain"
                                || neighbor_tile.feature.as_deref() == Some("volcano")
                        })
                    })
        })
        .collect();
    for index in (1..fissure_candidates.len()).rev() {
        let other = rng.below(index + 1);
        fissure_candidates.swap(index, other);
    }
    let fissure_target = (land_list.len() / 140).max(1);
    let mut fissures = Vec::new();
    for position in fissure_candidates {
        if fissures.len() >= fissure_target {
            break;
        }
        if fissures
            .iter()
            .all(|other| hex::wdistance(position, *other, width) >= 3)
        {
            wm.tiles.get_mut(&position).unwrap().feature = Some("geothermal_fissure".into());
            fissures.push(position);
        }
    }

    // Polar sea ice occupies both Ocean and Coast. Latitude controls density,
    // leaving navigable gaps instead of drawing an artificial solid wall.
    let polar_water: Vec<Pos> = wm
        .tiles
        .iter()
        .filter(|(position, tile)| {
            matches!(tile.terrain.as_str(), "coast" | "ocean") && latitude(**position) > 0.82
        })
        .map(|(position, _)| *position)
        .collect();
    for position in polar_water {
        let chance = ((latitude(position) - 0.82) / 0.18 * 0.72).clamp(0.0, 0.72);
        if rng.chance(chance) {
            wm.tiles.get_mut(&position).unwrap().feature = Some("ice".into());
        }
    }

    // --- vegetative, wetland and river-basin features, and the reefs that
    // supply the Campus's major Gathering Storm adjacency source.
    add_features(&mut wm, &land, equal_area, rng);

    // --- natural wonders: use the stock per-map-size count and the actual
    // footprint of each modeled wonder. Multi-tile wonders are grown as a
    // connected cluster so discovery, adjacency and yields operate on every
    // constituent tile rather than on a single representative hex.
    //
    // The stock generator also spreads them out: `NaturalWonderGenerator`
    // rejects a candidate plot that sits too near a wonder it has already
    // drawn, so no two of them ever share a border and a single region never
    // collects the map's whole allowance. Two wonders that prefer the same
    // biome — Yosemite and Mount Everest both want mountains — otherwise
    // settle onto the same range and read as one oversized feature. The
    // separation is a preference, not a quota: it is relaxed one ring at a
    // time down to `MIN_WONDER_SEPARATION` before a wonder is allowed to
    // place unconstrained, so a cramped map still receives its full count.
    let mut placed_wonder_tiles: Vec<Pos> = Vec::new();
    let mut wonder_names = [
        "great_barrier_reef",
        "crater_lake",
        "pantanal",
        "uluru",
        "yosemite",
        "dead_sea",
        "mount_everest",
        "pamukkale",
    ];
    for index in (1..wonder_names.len()).rev() {
        let other = rng.below(index + 1);
        wonder_names.swap(index, other);
    }
    for wonder in wonder_names.iter().take(num_natural_wonders) {
        let footprint = match *wonder {
            "great_barrier_reef" | "yosemite" | "dead_sea" | "pamukkale" => 2,
            "mount_everest" => 3,
            "pantanal" => 4,
            _ => 1,
        };
        let preferred = |t: &crate::world::Tile| {
            if t.feature.is_some() || t.resource.is_some() {
                return false;
            }
            match *wonder {
                "great_barrier_reef" => t.terrain == "coast",
                "crater_lake" => {
                    matches!(t.terrain.as_str(), "grassland" | "plains" | "tundra")
                        && !t.hills
                        && !t.has_river()
                }
                "pantanal" => matches!(t.terrain.as_str(), "grassland" | "plains") && !t.hills,
                "uluru" => t.terrain == "desert" && !t.hills,
                "yosemite" | "mount_everest" => t.terrain == "mountain",
                "dead_sea" => {
                    matches!(t.terrain.as_str(), "desert" | "plains") && !t.hills && !t.has_river()
                }
                "pamukkale" => {
                    matches!(t.terrain.as_str(), "desert" | "grassland" | "plains") && !t.hills
                }
                _ => false,
            }
        };
        // A tile is far enough from the wonders already drawn when every one
        // of their tiles is at least `separation` hexes away. `separation`
        // of 1 is no constraint at all, which is what the final unconstrained
        // attempt uses.
        let far_enough = |position: Pos, separation: i32| {
            placed_wonder_tiles
                .iter()
                .all(|placed| hex::wdistance(position, *placed, width) >= separation)
        };
        let cluster_from = |anchor: Pos, preferred_only: bool, separation: i32| {
            let mut cluster = vec![anchor];
            while cluster.len() < footprint {
                let mut frontier: Vec<Pos> = cluster
                    .iter()
                    .flat_map(|position| hex::neighbors(*position))
                    .map(|position| hex::canon(position, width))
                    .filter(|position| wm.tiles.contains_key(position))
                    .filter(|position| !cluster.contains(position))
                    .filter(|position| far_enough(*position, separation))
                    .filter(|position| {
                        let tile = &wm.tiles[position];
                        if preferred_only {
                            preferred(tile)
                        } else if *wonder == "great_barrier_reef" {
                            tile.terrain == "coast"
                                && tile.feature.is_none()
                                && tile.resource.is_none()
                        } else {
                            !matches!(tile.terrain.as_str(), "ocean" | "coast")
                                && tile.feature.is_none()
                                && tile.resource.is_none()
                        }
                    })
                    .collect();
                frontier.sort();
                frontier.dedup();
                if frontier.is_empty() {
                    return None;
                }
                cluster.push(frontier[0]);
            }
            Some(cluster)
        };
        let preferred_sites: Vec<Pos> = wm
            .tiles
            .iter()
            .filter(|(_, t)| preferred(t))
            .map(|(p, _)| *p)
            .collect();
        // Very unusual seeds can lack a large enough preferred biome. Keep
        // the correct footprint and map-size count by shaping an otherwise
        // empty connected region into the wonder's terrain family.
        let shaped_sites: Vec<Pos> = wm
            .tiles
            .iter()
            .filter(|(_, t)| {
                ((*wonder == "great_barrier_reef" && t.terrain == "coast")
                    || (*wonder != "great_barrier_reef"
                        && !matches!(t.terrain.as_str(), "ocean" | "coast")))
                    && t.feature.is_none()
                    && t.resource.is_none()
            })
            .map(|(p, _)| *p)
            .collect();
        // Sites are tried in order of how far each one departs from the ideal:
        // the wonder's own biome at the widest spacing, then narrower rings,
        // then the shaped fallback down the same ladder. Rewriting a region
        // into the wonder's terrain is the larger departure of the two, so the
        // whole preferred ladder is exhausted first. Dropping the separation
        // altogether is worse than either and comes last, once no pool can
        // seat this wonder `MIN_WONDER_SEPARATION` hexes from its neighbours.
        let pools = [(&preferred_sites, true), (&shaped_sites, false)];
        let mut attempts: Vec<(&Vec<Pos>, bool, i32)> = Vec::new();
        for (sites, preferred_only) in pools {
            for separation in (MIN_WONDER_SEPARATION..=PREFERRED_WONDER_SEPARATION).rev() {
                attempts.push((sites, preferred_only, separation));
            }
        }
        for (sites, preferred_only) in pools {
            attempts.push((sites, preferred_only, 1));
        }
        let mut footprint_tiles = None;
        for (sites, preferred_only, separation) in attempts {
            let mut cands: Vec<Pos> = sites
                .iter()
                .copied()
                .filter(|position| far_enough(*position, separation))
                .collect();
            while !cands.is_empty() && footprint_tiles.is_none() {
                let index = rng.below(cands.len());
                let anchor = cands.swap_remove(index);
                footprint_tiles = cluster_from(anchor, preferred_only, separation);
            }
            if footprint_tiles.is_some() {
                break;
            }
        }
        if let Some(cluster) = footprint_tiles {
            for position in cluster {
                let tile = wm.tiles.get_mut(&position).unwrap();
                if matches!(*wonder, "yosemite" | "mount_everest") {
                    tile.terrain = "mountain".into();
                    tile.hills = false;
                }
                tile.feature = Some((*wonder).into());
                tile.resource = None;
                tile.improvement = None;
                placed_wonder_tiles.push(position);
            }
        }
    }

    // --- resources
    let all_pos: Vec<Pos> = wm.tiles.keys().cloned().collect();
    for pos in all_pos {
        let (terrain, feature) = {
            let t = &wm.tiles[&pos];
            (t.terrain.clone(), t.feature.clone())
        };
        let natural_wonder = feature
            .as_ref()
            .and_then(|f| rules.features.get(f))
            .map(|f| f.natural_wonder)
            .unwrap_or(false);
        if !rules.is_passable(&wm.tiles[&pos])
            || natural_wonder
            || feature.as_deref() == Some("oasis")
            || feature.as_deref() == Some("marsh")
            || feature.as_deref() == Some("volcanic_soil")
        {
            continue;
        }
        if rng.chance(0.13) {
            let hills = wm.tiles[&pos].hills;
            let valid: Vec<String> = rules
                .resources
                .iter()
                .filter(|(_, s)| {
                    // The shipped placement is a union: a listed feature on
                    // the tile, or a listed terrain on a featureless tile —
                    // and hills-only spawns (Sheep) respect the tile's form.
                    let by_feature = feature
                        .as_ref()
                        .map(|f| s.feature.contains(f))
                        .unwrap_or(false);
                    let by_terrain = feature.is_none() && s.terrain.contains(&terrain);
                    (by_feature || by_terrain) && s.hills.is_none_or(|want| want == hills)
                })
                .map(|(name, _)| name.clone())
                .collect();
            if !valid.is_empty() {
                let pick = valid[rng.below(valid.len())].clone();
                wm.tiles.get_mut(&pos).unwrap().resource = Some(pick);
            }
        }
    }

    place_strategic_quotas(rules, &mut wm, &land, num_major_spawns, rng);

    assign_continents(&mut wm, &land, width, num_continents, rng);

    // Gathering Storm marks only a subset of flat coastal land as vulnerable
    // 1 m, 2 m, or 3 m Coastal Lowland. The stock generator derives these
    // bands from its elevation field; this deterministic coordinate hash is
    // the equivalent for CIVVIS's biome generator and does not perturb the
    // seeded gameplay RNG stream.
    let coastal_candidates: Vec<Pos> = wm
        .tiles
        .iter()
        .filter(|(_, tile)| {
            !tile.hills
                && rules.is_passable(tile)
                && !rules.is_water(tile)
                && tile
                    .feature
                    .as_ref()
                    .and_then(|feature| rules.features.get(feature))
                    .is_none_or(|feature| !feature.natural_wonder)
        })
        .filter(|(position, _)| {
            hex::neighbors(**position)
                .into_iter()
                .map(|neighbor| hex::canon(neighbor, width))
                .any(|neighbor| {
                    // Sea level is what rises, so a lake shore is not lowland
                    // however low it lies.
                    wm.tiles
                        .get(&neighbor)
                        .is_some_and(|tile| matches!(tile.terrain.as_str(), "coast" | "ocean"))
                })
        })
        .map(|(position, _)| *position)
        .collect();
    for position in coastal_candidates {
        let hash = (position.0 as i64)
            .wrapping_mul(73_856_093)
            .wrapping_add((position.1 as i64).wrapping_mul(19_349_663))
            .unsigned_abs();
        if !hash.is_multiple_of(5) {
            wm.tiles.get_mut(&position).unwrap().coastal_lowland = (hash % 3 + 1) as u8;
        }
    }

    // --- spawns. Pangaea and Inland Sea share one primary landmass; the
    // ocean-separated scripts deliberately seed majors across their viable
    // components so their geography affects play from turn one.
    let passable: BTreeSet<Pos> = land
        .iter()
        .filter(|pos| rules.is_passable(&wm.tiles[pos]))
        .cloned()
        .collect();
    let total_spawns = num_major_spawns + num_minor_spawns;
    let candidates_for = |component: &BTreeSet<Pos>, needed: usize| {
        let mut candidates: Vec<Pos> = component
            .iter()
            .filter(|position| {
                let tile = &wm.tiles[position];
                matches!(tile.terrain.as_str(), "grassland" | "plains")
                    && tile.feature.is_none()
                    && tile.improvement.is_none()
            })
            .cloned()
            .collect();
        if candidates.len() < needed {
            candidates = component
                .iter()
                .filter(|position| {
                    let tile = &wm.tiles[position];
                    tile.improvement.is_none()
                        && !tile
                            .feature
                            .as_ref()
                            .and_then(|feature| rules.features.get(feature))
                            .is_some_and(|feature| feature.natural_wonder)
                })
                .cloned()
                .collect();
        }
        candidates.sort();
        candidates
    };
    let components = connected_components(&passable, width);
    let primary = components.first().cloned().unwrap_or_default();
    let mut all_candidates = candidates_for(&passable, total_spawns);
    let mut spawns = if matches!(
        script,
        MapScript::TrueStartEarth | MapScript::TrueStartTrueEarth
    ) {
        true_start_major_spawns(&wm, &all_candidates, num_major_spawns)
    } else if matches!(script, MapScript::Continents | MapScript::SmallContinents) {
        let viable: Vec<(BTreeSet<Pos>, Vec<Pos>)> = components
            .into_iter()
            .map(|component| {
                let candidates = candidates_for(&component, 1);
                (component, candidates)
            })
            .filter(|(_, candidates)| !candidates.is_empty())
            .collect();
        let mut allocations = vec![0usize; viable.len()];
        for _ in 0..num_major_spawns {
            let Some(index) = (0..viable.len())
                .filter(|index| allocations[*index] < viable[*index].1.len())
                .min_by_key(|index| {
                    // Fill every landmass once, then distribute proportionally
                    // to its available capital sites.
                    (
                        allocations[*index] > 0,
                        allocations[*index] * 1_000_000 / viable[*index].1.len().max(1),
                        *index,
                    )
                })
            else {
                break;
            };
            allocations[index] += 1;
        }
        let mut starts = Vec::new();
        for ((component, candidates), count) in viable.iter().zip(allocations) {
            starts.extend(balanced_major_spawns(
                rules, &wm, component, candidates, count, rng,
            ));
        }
        for index in (1..starts.len()).rev() {
            let other = rng.below(index + 1);
            starts.swap(index, other);
        }
        starts
    } else {
        let primary_candidates = candidates_for(&primary, total_spawns);
        all_candidates = primary_candidates.clone();
        balanced_major_spawns(
            rules,
            &wm,
            &primary,
            &primary_candidates,
            num_major_spawns,
            rng,
        )
    };
    // Defensive completion for unusually mountain-heavy seeds, followed by
    // city-state placement in the largest remaining gaps on eligible land.
    if spawns.len() < num_major_spawns {
        let missing = num_major_spawns - spawns.len();
        complete_major_spawns(rules, &wm, &all_candidates, &mut spawns, missing);
    }
    add_minor_spawns(
        rules,
        &wm,
        &all_candidates,
        &mut spawns,
        num_minor_spawns,
        num_major_spawns,
    );
    for s in &spawns {
        let t = wm.tiles.get_mut(s).unwrap();
        t.feature = None;
        t.resource = None;
    }
    (wm, spawns)
}

/// Hexes the generator tries to keep between any two natural wonders, and the
/// floor it will not go below while a spacing-respecting site still exists.
/// `NaturalWonderGenerator` spreads the stock roster over the whole map rather
/// than letting two of them share a mountain range or a reef; the floor of 3
/// is the part that matters most, because it is what stops a pair from reading
/// as one oversized feature.
const PREFERRED_WONDER_SEPARATION: i32 = 6;
const MIN_WONDER_SEPARATION: i32 = 3;

/// World Age, which the stock scripts pass to every elevation percentile.
/// Continents.lua's "normal" is 3; a younger world raises more mountains.
const WORLD_AGE: i32 = 3;

/// Terrain band shares from `TerrainGenerator.lua` at Temperate: the driest
/// quarter of the desert field becomes Desert where the latitude allows it,
/// and the wetter half of the plains field becomes Plains.
const DESERT_PERCENT: u32 = 25;
const PLAINS_PERCENT: u32 = 50;
const SNOW_LATITUDE: f64 = 0.8;
const TUNDRA_LATITUDE: f64 = 0.65;
const GRASS_LATITUDE: f64 = 0.1;
const DESERT_BOTTOM_LATITUDE: f64 = 0.2;
const DESERT_TOP_LATITUDE: f64 = 0.5;

/// Elevation, the way `MountainsCliffs.lua` builds it: two fractal fields,
/// the mountain one with tectonic plate boundaries woven through it, cut at
/// percentiles. Mountains therefore arrive as ranges following a collision
/// line, ringed by their own foothills, rather than as short random walks;
/// hills additionally come in clumps wherever the hills field sits inside one
/// of its two bands.
fn apply_tectonics(wm: &mut WorldMap, land: &BTreeSet<Pos>, rng: &mut Rng) {
    let (width, height) = (wm.width, wm.height);
    // `MountainsCliffs.lua` weaves nine tectonic plates through the field
    // whatever the map size; the ridges they collide along are the ranges.
    const PLATES: usize = 9;
    let mut mountains = Fractal::new(rng, width, height, 3);
    mountains.build_ridges(rng, PLATES, 5.0, 5.0);
    let hills = Fractal::new(rng, width, height, 3);

    let cells: Vec<(i32, i32)> = land
        .iter()
        .map(|pos| hex::axial_to_offset(pos.0, pos.1))
        .collect();
    let mountain_threshold =
        mountains.percentile_within(cells.iter().copied(), (97 - WORLD_AGE) as u32);
    let foothills_threshold =
        mountains.percentile_within(cells.iter().copied(), (91 - 2 * WORLD_AGE) as u32);
    let pass_threshold =
        hills.percentile_within(cells.iter().copied(), (91 - 2 * WORLD_AGE) as u32);
    let low_band = (
        hills.percentile_within(cells.iter().copied(), (28 - WORLD_AGE) as u32),
        hills.percentile_within(cells.iter().copied(), (28 + WORLD_AGE) as u32),
    );
    let high_band = (
        hills.percentile_within(cells.iter().copied(), (72 - WORLD_AGE) as u32),
        hills.percentile_within(cells.iter().copied(), (72 + WORLD_AGE) as u32),
    );

    for pos in land {
        let (col, row) = hex::axial_to_offset(pos.0, pos.1);
        let mountain_value = mountains.at(col, row);
        let hill_value = hills.at(col, row);
        let tile = wm.tiles.get_mut(pos).unwrap();
        if mountain_value >= mountain_threshold {
            if hill_value >= pass_threshold {
                // A pass through the ridgeline, so a range is crossable.
                tile.hills = true;
            } else {
                tile.terrain = "mountain".into();
            }
        } else if mountain_value >= foothills_threshold {
            tile.hills = true;
        } else if (hill_value >= low_band.0 && hill_value <= low_band.1)
            || (hill_value >= high_band.0 && hill_value <= high_band.1)
        {
            tile.hills = true;
        }
    }

    // The stock generator demotes nine in ten mountains that reach the water,
    // which is what keeps coastlines workable and leaves the ranges inland.
    let coastal_peaks: Vec<Pos> = land
        .iter()
        .copied()
        .filter(|pos| wm.tiles[pos].terrain == "mountain")
        .filter(|pos| {
            hex::neighbors(*pos)
                .into_iter()
                .map(|neighbor| hex::canon(neighbor, width))
                .any(|neighbor| !land.contains(&neighbor))
        })
        .collect();
    for pos in coastal_peaks {
        if rng.below(10) < 9 {
            let tile = wm.tiles.get_mut(&pos).unwrap();
            // The climate pass, which runs next, repaints every tile that is
            // no longer a mountain, so only the elevation matters here.
            tile.terrain = "grassland".into();
            tile.hills = true;
        }
    }
}

/// Climate, the way `TerrainGenerator.lua` paints it: latitude bands whose
/// borders are roughened by a variation fractal, with Desert and Plains cut
/// out of two further fractals so that both arrive as regions. Desert is
/// additionally confined to the subtropics, which is why Civ VI worlds have
/// desert belts either side of a green equator rather than desert everywhere.
fn assign_biomes(wm: &mut WorldMap, land: &[Pos], equal_area: bool, rng: &mut Rng) {
    let (width, height) = (wm.width, wm.height);
    let deserts = Fractal::new(rng, width, height, 3);
    let plains = Fractal::new(rng, width, height, 3);
    let variation = Fractal::new(rng, width, height, 3);
    let desert_bottom = deserts.percentile(100 - DESERT_PERCENT);
    let plains_bottom = plains.percentile(100 - PLAINS_PERCENT);

    for pos in land {
        let (col, row) = hex::axial_to_offset(pos.0, pos.1);
        if wm.tiles[pos].terrain == "mountain" {
            continue;
        }
        let base = normalized_latitude(row, height, equal_area);
        let latitude =
            (base + (128.0 - variation.at(col, row) as f64) / (255.0 * 5.0)).clamp(0.0, 1.0);
        let terrain = if latitude >= SNOW_LATITUDE {
            "snow"
        } else if latitude >= TUNDRA_LATITUDE {
            "tundra"
        } else if latitude < GRASS_LATITUDE {
            "grassland"
        } else if deserts.at(col, row) >= desert_bottom
            && (DESERT_BOTTOM_LATITUDE..DESERT_TOP_LATITUDE).contains(&latitude)
        {
            "desert"
        } else if plains.at(col, row) >= plains_bottom {
            "plains"
        } else {
            "grassland"
        };
        wm.tiles.get_mut(pos).unwrap().terrain = terrain.into();
    }
}

/// Feature shares from the Gathering Storm `FeatureGenerator.lua` at Normal
/// rainfall: Rainforest fills 40% of the tropical band it is allowed in,
/// Woods 18% of land, Marsh 3%, Oasis 1%, and Reef 9% of eligible water.
const JUNGLE_PERCENT: usize = 40;
const FOREST_PERCENT: usize = 18;
const MARSH_PERCENT: usize = 3;
const OASIS_PERCENT: usize = 1;
const REEF_PERCENT: usize = 9;

/// The shipped clustering weight. A tile with two or three neighbours already
/// carrying the feature is the most likely to take it, and one ringed by five
/// is the least, so vegetation grows as forests and rainforests instead of
/// speckling every eligible tile independently.
fn cluster_score(adjacent: usize) -> i32 {
    match adjacent {
        0 => 300,
        1 => 350,
        2 | 3 => 450,
        4 => 250,
        _ => 100,
    }
}

fn adjacent_feature_count(wm: &WorldMap, pos: Pos, feature: &str) -> usize {
    hex::neighbors(pos)
        .into_iter()
        .map(|neighbor| hex::canon(neighbor, wm.width))
        .filter(|neighbor| {
            wm.get(*neighbor)
                .is_some_and(|tile| tile.feature.as_deref() == Some(feature))
        })
        .count()
}

/// Running-share cap: a feature stops being placed once it holds its quota of
/// the tiles considered so far, exactly as the stock generator's counters work.
fn within_share(count: usize, considered: usize, percent: usize) -> bool {
    considered == 0 || (count * 100).div_ceil(considered) <= percent
}

fn add_features(wm: &mut WorldMap, land: &BTreeSet<Pos>, equal_area: bool, rng: &mut Rng) {
    let (width, height) = (wm.width, wm.height);
    let equator = (height + 1) / 2;

    let mut considered_land = 0;
    let mut jungle_candidates = 0;
    let (mut jungles, mut forests, mut marshes, mut oases) = (0, 0, 0, 0);

    for row in 0..height {
        for col in 0..width {
            let pos = hex::offset_to_axial(col, row);
            if !land.contains(&pos) {
                continue;
            }
            let (terrain, hills, river, has_feature) = {
                let tile = &wm.tiles[&pos];
                (
                    tile.terrain.clone(),
                    tile.hills,
                    tile.has_river(),
                    tile.feature.is_some(),
                )
            };
            if terrain == "mountain" {
                continue;
            }
            considered_land += 1;
            if has_feature {
                continue;
            }

            // Every desert tile on a river floods, as in the stock generator.
            // 🟡 The Grassland and Plains variants stand in for river size,
            // which this generator does not model.
            if river {
                let floodplain = match terrain.as_str() {
                    "desert" => Some("floodplains"),
                    "grassland" if rng.chance(0.18) => Some("grassland_floodplains"),
                    "plains" if rng.chance(0.18) => Some("plains_floodplains"),
                    _ => None,
                };
                if let Some(feature) = floodplain {
                    wm.tiles.get_mut(&pos).unwrap().feature = Some(feature.into());
                    continue;
                }
            }

            if terrain == "desert" && !hills && !river {
                if within_share(oases, considered_land, OASIS_PERCENT) && rng.below(4) == 1 {
                    wm.tiles.get_mut(&pos).unwrap().feature = Some("oasis".into());
                    oases += 1;
                }
                continue;
            }

            // Marsh, then Rainforest, then Woods — the shipped precedence.
            if terrain == "grassland"
                && !hills
                && within_share(marshes, considered_land, MARSH_PERCENT)
                && (rng.below(300) as i32)
                    <= cluster_score(adjacent_feature_count(wm, pos, "marsh"))
            {
                wm.tiles.get_mut(&pos).unwrap().feature = Some("marsh".into());
                marshes += 1;
                continue;
            }

            // Rainforest keeps to twenty degrees either side of the equator,
            // whether rows are linear latitude or equal spherical area.
            let tropical = if equal_area {
                normalized_latitude(row, height, true) <= 20.0 / 90.0
            } else {
                (row - equator).abs() <= (20 * height / 180).max(2)
            };
            if tropical && matches!(terrain.as_str(), "grassland" | "plains") {
                jungle_candidates += 1;
                if within_share(jungles, jungle_candidates, JUNGLE_PERCENT)
                    && (rng.below(450) as i32)
                        <= cluster_score(adjacent_feature_count(wm, pos, "jungle"))
                {
                    let tile = wm.tiles.get_mut(&pos).unwrap();
                    // Rainforest leaves the ground beneath it Plains.
                    tile.terrain = "plains".into();
                    tile.feature = Some("jungle".into());
                    jungles += 1;
                    continue;
                }
            }

            if matches!(terrain.as_str(), "grassland" | "plains" | "tundra")
                && within_share(forests, considered_land, FOREST_PERCENT)
                && (rng.below(300) as i32)
                    <= cluster_score(adjacent_feature_count(wm, pos, "forest"))
            {
                wm.tiles.get_mut(&pos).unwrap().feature = Some("forest".into());
                forests += 1;
            }
        }
    }

    // Reefs favour warm water and thin out where they are already dense, so
    // they form scattered banks rather than a border around every continent.
    let mut reefable = 0;
    let mut reefs = 0;
    for row in 0..height {
        for col in 0..width {
            let pos = hex::offset_to_axial(col, row);
            let latitude = normalized_latitude(row, height, equal_area);
            let eligible = wm
                .get(pos)
                .is_some_and(|tile| tile.terrain == "coast" && tile.feature.is_none());
            if !eligible || latitude >= 0.78 * 0.9 {
                continue;
            }
            reefable += 1;
            if !within_share(reefs, reefable, REEF_PERCENT) {
                continue;
            }
            let crowding = match adjacent_feature_count(wm, pos, "reef") {
                0 => 100,
                1 => 125,
                2 => 150,
                3 | 4 => 175,
                _ => 10_000,
            };
            let score = 3 * (row - equator).abs() + crowding;
            if (rng.below(200) as i32) >= score {
                wm.tiles.get_mut(&pos).unwrap().feature = Some("reef".into());
                reefs += 1;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SpawnLayoutScore {
    /// No two civilizations should begin unavoidably crowded.
    minimum_separation: i32,
    /// Cover the complete landmass rather than leaving a large empty end.
    negative_coverage_radius: i32,
    /// Similar nearest-neighbor distances avoid isolated and crowded starts.
    negative_neighbor_range: i32,
    /// Voronoi area is a useful proxy for the land available to each start.
    minimum_territory: i32,
    negative_territory_range: i32,
    /// Only after spatial fairness, prefer layouts without a weak outlier.
    minimum_quality: i32,
    negative_quality_range: i32,
    total_quality: i32,
}

/// A compact estimate of the capital site rather than just its center tile.
/// Early food/production, fresh water and room to work land all matter, while
/// only the best nearby tiles count so a large empty desert is not rewarded.
fn start_quality(rules: &Rules, wm: &WorldMap, pos: Pos) -> i32 {
    let center = &wm.tiles[&pos];
    let fresh_water = center.has_river()
        || hex::neighbors(pos)
            .into_iter()
            .map(|neighbor| hex::canon(neighbor, wm.width))
            .any(|neighbor| {
                wm.get(neighbor)
                    .is_some_and(|tile| tile.feature.as_deref() == Some("oasis"))
            });
    let coastal = hex::neighbors(pos)
        .into_iter()
        .map(|neighbor| hex::canon(neighbor, wm.width))
        .any(|neighbor| wm.get(neighbor).is_some_and(|tile| rules.is_water(tile)));

    let mut nearby_yields = Vec::new();
    let mut workable_land = 0;
    let mut seen = BTreeSet::new();
    for raw in hex::disk(pos, 3) {
        let tile_pos = hex::canon(raw, wm.width);
        if !seen.insert(tile_pos) {
            continue;
        }
        let Some(tile) = wm.get(tile_pos) else {
            continue;
        };
        if !rules.is_water(tile) && rules.is_passable(tile) {
            workable_land += 1;
        }
        if tile_pos == pos || hex::wdistance(pos, tile_pos, wm.width) > 2 {
            continue;
        }
        let yields = rules.tile_yields(tile);
        nearby_yields.push(
            (yields.food * 4.0
                + yields.production * 5.0
                + yields.gold
                + yields.science * 2.0
                + yields.culture * 2.0
                + yields.faith) as i32,
        );
    }
    nearby_yields.sort_unstable_by(|a, b| b.cmp(a));
    let best_nearby: i32 = nearby_yields.into_iter().take(8).sum();
    best_nearby
        + workable_land * 2
        + if fresh_water {
            32
        } else if coastal {
            12
        } else {
            0
        }
}

/// How well a start site satisfies a civilization's shipped `StartBias*` rows.
/// Each satisfied bias scores `6 - Tier`, so a Tier 2 pull outweighs a Tier 5
/// one, and a site that matches nothing scores zero.
///
/// Terrain, feature and resource biases are looked for across the tiles a city
/// actually works — radius 3 — because the game places a civilization *near*
/// what it wants rather than exactly on it. A river bias asks the start tile
/// itself, which is what "starts on a river" means.
pub fn start_bias_score(rules: &Rules, wm: &WorldMap, pos: Pos, civ: &str) -> i32 {
    let Some(bias) = rules.civs.get(civ).and_then(|spec| spec.start_bias.as_ref()) else {
        return 0;
    };
    let mut score = 0;
    let nearby: Vec<&crate::world::Tile> = hex::disk(pos, 3)
        .into_iter()
        .map(|raw| hex::canon(raw, wm.width))
        .filter_map(|tile| wm.get(tile))
        .collect();

    if !bias.terrain.is_empty() {
        let matched = nearby.iter().any(|tile| {
            bias.terrain.iter().any(|wanted| tile.terrain == *wanted)
                && (!bias.terrain_hills || tile.hills)
        });
        if matched {
            score += crate::rules::StartBias::weight(bias.terrain_tier);
        }
    }
    if !bias.feature.is_empty() {
        let matched = nearby.iter().any(|tile| {
            tile.feature
                .as_deref()
                .is_some_and(|feature| bias.feature.iter().any(|wanted| wanted == feature))
        });
        if matched {
            score += crate::rules::StartBias::weight(bias.feature_tier);
        }
    }
    if !bias.resource.is_empty() {
        let matched = nearby.iter().any(|tile| {
            tile.resource
                .as_deref()
                .is_some_and(|resource| bias.resource.iter().any(|wanted| wanted == resource))
        });
        if matched {
            score += crate::rules::StartBias::weight(bias.resource_tier);
        }
    }
    if bias.river_tier > 0 && wm.get(pos).is_some_and(|tile| tile.has_river()) {
        score += crate::rules::StartBias::weight(bias.river_tier);
    }
    score
}

/// Hand each seat the start its civilization is biased toward. Civilization VI
/// decides *which* start a civilization gets from its `StartBias*` rows; CIVVIS
/// generated the layout well and then handed seat `i` `spawns[i]`, so an Egypt
/// on a river was luck.
///
/// Greedy over the strongest (seat, site) pull first, which is deterministic
/// and cheap; an optimal assignment is not worth the cost at eight to twelve
/// seats, and the strong Tier 2 pulls are what actually matter.
pub fn assign_starts_by_bias(rules: &Rules, wm: &WorldMap, sites: &mut [Pos], civs: &[String]) {
    let seats = sites.len().min(civs.len());
    if seats < 2 {
        return;
    }
    let mut pairs: Vec<(i32, usize, usize)> = Vec::new();
    for (seat, civ) in civs.iter().enumerate().take(seats) {
        for (site, pos) in sites.iter().enumerate().take(seats) {
            let score = start_bias_score(rules, wm, *pos, civ);
            if score > 0 {
                pairs.push((score, seat, site));
            }
        }
    }
    // Strongest pull first; ties resolve by seat then site so a given map and
    // roster always produce the same assignment.
    pairs.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    let mut seat_site: Vec<Option<usize>> = vec![None; seats];
    let mut taken = vec![false; seats];
    for (_, seat, site) in pairs {
        if seat_site[seat].is_none() && !taken[site] {
            seat_site[seat] = Some(site);
            taken[site] = true;
        }
    }
    // Seats with no satisfied bias keep whatever is left, in order.
    let mut spare = (0..seats).filter(|site| !taken[*site]);
    for slot in seat_site.iter_mut() {
        if slot.is_none() {
            *slot = spare.next();
        }
    }
    let original: Vec<Pos> = sites[..seats].to_vec();
    for (seat, slot) in seat_site.iter().enumerate() {
        if let Some(site) = slot {
            sites[seat] = original[*site];
        }
    }
}

fn spawn_layout_score(
    wm: &WorldMap,
    landmass: &BTreeSet<Pos>,
    layout: &[Pos],
    qualities: &BTreeMap<Pos, i32>,
) -> SpawnLayoutScore {
    if layout.is_empty() {
        return SpawnLayoutScore {
            minimum_separation: 0,
            negative_coverage_radius: 0,
            negative_neighbor_range: 0,
            minimum_territory: 0,
            negative_territory_range: 0,
            minimum_quality: 0,
            negative_quality_range: 0,
            total_quality: 0,
        };
    }

    let mut ordered = layout.to_vec();
    ordered.sort();
    let nearest: Vec<i32> = if ordered.len() == 1 {
        vec![0]
    } else {
        ordered
            .iter()
            .map(|start| {
                ordered
                    .iter()
                    .filter(|other| *other != start)
                    .map(|other| hex::wdistance(*start, *other, wm.width))
                    .min()
                    .unwrap()
            })
            .collect()
    };
    let minimum_separation = nearest.iter().copied().min().unwrap_or(0);
    let neighbor_range = nearest.iter().copied().max().unwrap_or(0) - minimum_separation;

    let mut territory = vec![0_i32; ordered.len()];
    let mut coverage_radius = 0;
    for tile in landmass {
        let (distance, owner) = ordered
            .iter()
            .enumerate()
            .map(|(index, start)| (hex::wdistance(*tile, *start, wm.width), index))
            .min()
            .unwrap();
        coverage_radius = coverage_radius.max(distance);
        territory[owner] += 1;
    }
    let territory_range =
        territory.iter().copied().max().unwrap_or(0) - territory.iter().copied().min().unwrap_or(0);
    let minimum_territory = territory.iter().copied().min().unwrap_or(0);

    let qualities: Vec<i32> = ordered.iter().map(|start| qualities[start]).collect();
    let minimum_quality = qualities.iter().copied().min().unwrap_or(0);
    let maximum_quality = qualities.iter().copied().max().unwrap_or(0);

    SpawnLayoutScore {
        minimum_separation,
        negative_coverage_radius: -coverage_radius,
        negative_neighbor_range: -neighbor_range,
        minimum_territory,
        negative_territory_range: -territory_range,
        minimum_quality,
        negative_quality_range: -(maximum_quality - minimum_quality),
        total_quality: qualities.iter().sum(),
    }
}

fn layout_balance_percentages(
    score: SpawnLayoutScore,
    civilization_count: usize,
    landmass_tiles: usize,
) -> (i32, i32, i32) {
    let territory =
        score.minimum_territory * civilization_count as i32 * 100 / landmass_tiles.max(1) as i32;
    let neighbor = if civilization_count <= 1 {
        100
    } else {
        let maximum = score.minimum_separation - score.negative_neighbor_range;
        score.minimum_separation * 100 / maximum.max(1)
    };
    let maximum_quality = score.minimum_quality - score.negative_quality_range;
    let quality = score.minimum_quality * 100 / maximum_quality.max(1);
    (territory, neighbor, quality)
}

/// Shipped `START_DISTANCE_MAJOR_CIVILIZATION`: major civilizations are aimed
/// this far apart, not as far apart as the landmass allows.
pub(crate) const START_DISTANCE_MAJOR: i32 = 12;
/// Shipped `START_DISTANCE_RANGE_MAJOR`: how far either side of the target a
/// start may sit before the placement counts as a compromise.
pub(crate) const START_RANGE_MAJOR: i32 = 2;

/// Shipped `START_DISTANCE_MINOR_MAJOR_CIVILIZATION`: a city-state is aimed
/// this far from the nearest major civilization.
pub(crate) const START_DISTANCE_MINOR_MAJOR: i32 = 6;
/// Shipped `START_DISTANCE_MINOR_CIVILIZATION_START`: and this far from
/// another city-state.
pub(crate) const START_DISTANCE_MINOR_MINOR: i32 = 5;
/// Shipped `START_DISTANCE_RANGE_MINOR`.
pub(crate) const START_RANGE_MINOR: i32 = 3;
/// Shipped `START_DISTANCE_MINOR_NATURAL_WONDER`: a city-state keeps this much
/// clear of a Natural Wonder. (`START_DISTANCE_MAJOR_NATURAL_WONDER` is 2, and
/// major placement already satisfies it — measured 0 violations in 96 starts —
/// because Natural Wonder tiles are excluded from the candidate set outright.)
pub(crate) const START_DISTANCE_MINOR_NATURAL_WONDER: i32 = 3;
/// Shipped `START_DISTANCE_MAJOR_NATURAL_WONDER`.
pub(crate) const START_DISTANCE_MAJOR_NATURAL_WONDER: i32 = 2;

/// How badly one distance misses a shipped target band. Zero inside the band,
/// growing outside it, and counting crowding double — two starts on top of
/// each other is worse than two a little too far apart.
pub(crate) fn distance_miss(distance: i32, target: i32, range: i32) -> i32 {
    let low = target - range;
    let high = target + range;
    if distance < low {
        2 * (low - distance)
    } else if distance > high {
        distance - high
    } else {
        0
    }
}

/// The major-civilization band, 10..=14.
pub(crate) fn start_distance_miss(distance: i32) -> i32 {
    distance_miss(distance, START_DISTANCE_MAJOR, START_RANGE_MAJOR)
}

/// Place each start at the shipped distance from its nearest neighbour rather
/// than at the greatest distance available. The old farthest-point rule put
/// every civilization on the tournament map 17-23 tiles from its neighbour
/// where Civilization VI aims for 10-14, which moves settling races, border
/// friction, and the Loyalty and religious pressure that depend on proximity.
fn targeted_layout(
    wm: &WorldMap,
    candidates: &[Pos],
    qualities: &BTreeMap<Pos, i32>,
    first: Pos,
    count: usize,
) -> Vec<Pos> {
    let mut layout = vec![first];
    while layout.len() < count {
        let Some(next) = candidates
            .iter()
            .filter(|candidate| !layout.contains(candidate))
            .min_by_key(|candidate| {
                let nearest = layout
                    .iter()
                    .map(|start| hex::wdistance(**candidate, *start, wm.width))
                    .min()
                    .unwrap_or(0);
                (
                    start_distance_miss(nearest),
                    -qualities[*candidate],
                    **candidate,
                )
            })
            .copied()
        else {
            break;
        };
        layout.push(next);
    }
    layout
}

/// Try farthest-point layouts from seeds spread throughout the candidate set,
/// then retain the layout with the best spacing, coverage, territory balance
/// and site quality. This removes the large positional bias caused by making
/// a single random tile the permanent anchor for every other civilization.
fn balanced_major_spawns(
    rules: &Rules,
    wm: &WorldMap,
    landmass: &BTreeSet<Pos>,
    candidates: &[Pos],
    count: usize,
    rng: &mut Rng,
) -> Vec<Pos> {
    if count == 0 || candidates.is_empty() {
        return Vec::new();
    }
    let count = count.min(candidates.len());
    let qualities: BTreeMap<Pos, i32> = candidates
        .iter()
        .map(|candidate| (*candidate, start_quality(rules, wm, *candidate)))
        .collect();
    let mut quality_values: Vec<i32> = qualities.values().copied().collect();
    quality_values.sort_unstable();
    let quality_floor = quality_values[quality_values.len() / 4];
    let preferred_candidates: Vec<Pos> = candidates
        .iter()
        .filter(|candidate| qualities[*candidate] >= quality_floor)
        .copied()
        .collect();

    let mut layouts = Vec::with_capacity(82);
    for (pool, trial_limit) in [
        (candidates, 64_usize),
        (preferred_candidates.as_slice(), 16_usize),
    ] {
        if pool.len() < count {
            continue;
        }
        let trial_count = pool.len().min(trial_limit);
        let mut seeds = Vec::with_capacity(trial_count + 1);
        for index in 0..trial_count {
            let candidate_index = index * pool.len() / trial_count;
            if seeds.last() != pool.get(candidate_index) {
                seeds.push(pool[candidate_index]);
            }
        }
        if let Some(best_site) = pool
            .iter()
            .max_by_key(|candidate| (qualities[*candidate], **candidate))
            .copied()
        {
            if !seeds.contains(&best_site) {
                seeds.push(best_site);
            }
        }
        for seed in seeds {
            let layout = targeted_layout(wm, pool, &qualities, seed, count);
            let score = spawn_layout_score(wm, landmass, &layout, &qualities);
            layouts.push((score, layout));
        }
    }
    let best_separation = layouts
        .iter()
        .map(|(score, _)| score.minimum_separation)
        .max()
        .unwrap();
    // One hex off the theoretical maximum can buy more even neighbors,
    // territory and capital quality — but only while every seat still starts
    // comfortably apart. Below that, distance is the fairness that matters.
    let separation_floor = if best_separation > 10 {
        best_separation - 1
    } else {
        best_separation
    };
    layouts.retain(|(score, _)| score.minimum_separation >= separation_floor);
    let best_coverage = layouts
        .iter()
        .map(|(score, _)| score.negative_coverage_radius)
        .max()
        .unwrap();
    layouts.retain(|(score, _)| score.negative_coverage_radius >= best_coverage - 1);
    let mut layout = layouts
        .into_iter()
        .max_by_key(|(score, _)| {
            let (territory_balance, neighbor_balance, quality_balance) =
                layout_balance_percentages(*score, count, landmass.len());
            let worst_balance = territory_balance.min(neighbor_balance).min(quality_balance);
            (
                worst_balance,
                territory_balance + neighbor_balance + quality_balance,
                score.minimum_territory,
                score.negative_neighbor_range,
                score.minimum_quality,
                score.negative_territory_range,
                score.negative_quality_range,
                score.total_quality,
                score.minimum_separation,
                score.negative_coverage_radius,
            )
        })
        .unwrap()
        .1;

    // Farthest-point sampling fixes the coarse grid but cannot see that one
    // seat ends up with a thin territory wedge. Hill-climb each start over its
    // immediate neighbourhood, keeping any single swap that lifts the balance
    // ranking, so no seat is left an outlier the sampler simply never offered.
    let rank = |layout: &[Pos]| {
        let score = spawn_layout_score(wm, landmass, layout, &qualities);
        let (territory_balance, neighbor_balance, quality_balance) =
            layout_balance_percentages(score, count, landmass.len());
        (
            territory_balance.min(neighbor_balance).min(quality_balance),
            territory_balance + neighbor_balance + quality_balance,
            score.minimum_separation,
            score.minimum_territory,
            score.minimum_quality,
            score.total_quality,
        )
    };
    let mut best_rank = rank(&layout);
    for _ in 0..4 {
        let mut improved = false;
        for index in 0..layout.len() {
            let current = layout[index];
            let Some((candidate_rank, candidate)) = candidates
                .iter()
                .filter(|candidate| {
                    hex::wdistance(**candidate, current, wm.width) <= 3
                        && !layout.contains(candidate)
                })
                .map(|candidate| {
                    let mut trial = layout.clone();
                    trial[index] = *candidate;
                    (rank(&trial), *candidate)
                })
                // A balance win must not spend the separation the layout
                // stage just guaranteed.
                .filter(|((_, _, separation, _, _, _), _)| *separation >= separation_floor)
                .max()
            else {
                continue;
            };
            if candidate_rank > best_rank {
                best_rank = candidate_rank;
                layout[index] = candidate;
                improved = true;
            }
        }
        if !improved {
            break;
        }
    }

    // Seat order should not correlate with an anchor, edge, or the order in
    // which farthest-point sampling filled the landmass.
    for index in (1..layout.len()).rev() {
        let other = rng.below(index + 1);
        layout.swap(index, other);
    }
    layout
}

/// City-states fill the remaining largest gaps after major civilizations are
/// fixed, so they cannot pull a major start away from an otherwise fair grid.
/// Place city-states at the shipped distances rather than in the largest
/// remaining gaps: `START_DISTANCE_MINOR_MAJOR_CIVILIZATION` 6 from the nearest
/// major and `START_DISTANCE_MINOR_CIVILIZATION_START` 5 from another
/// city-state, both within `START_DISTANCE_RANGE_MINOR` 3. Filling the gaps
/// instead put them roughly twice as far out as Civilization VI does, which
/// changes envoy competition and how early a suzerain is worth contesting.
///
/// `major_count` is how many of `spawns` are major civilizations; the rest are
/// city-states already placed by this pass.
fn add_minor_spawns(
    rules: &Rules,
    wm: &WorldMap,
    candidates: &[Pos],
    spawns: &mut Vec<Pos>,
    count: usize,
    major_count: usize,
) {
    let qualities: BTreeMap<Pos, i32> = candidates
        .iter()
        .map(|candidate| (*candidate, start_quality(rules, wm, *candidate)))
        .collect();
    let wonders: Vec<Pos> = wm
        .tiles
        .iter()
        .filter(|(_, tile)| {
            tile.feature
                .as_ref()
                .and_then(|feature| rules.features.get(feature))
                .is_some_and(|feature| feature.natural_wonder)
        })
        .map(|(position, _)| *position)
        .collect();
    // Keep the shipped standoff where the map allows it, and fall back rather
    // than fail on a wonder-dense seed.
    let clear_of_wonders: Vec<Pos> = candidates
        .iter()
        .copied()
        .filter(|candidate| {
            wonders
                .iter()
                .all(|wonder| {
                    hex::wdistance(*candidate, *wonder, wm.width)
                        >= START_DISTANCE_MINOR_NATURAL_WONDER
                })
        })
        .collect();
    let target = spawns.len() + count;
    while spawns.len() < target {
        let pool: &[Pos] = if clear_of_wonders.len() >= count {
            &clear_of_wonders
        } else {
            candidates
        };
        let Some(next) = pool
            .iter()
            .filter(|candidate| !spawns.contains(candidate))
            .min_by_key(|candidate| {
                let nearest = |group: &[Pos]| {
                    group
                        .iter()
                        .map(|start| hex::wdistance(**candidate, *start, wm.width))
                        .min()
                };
                // Aim at the target, not merely inside the band. The minor
                // bands are wide (3..=9 and 2..=8), so scoring band membership
                // alone leaves most candidates tied at zero and hands the
                // choice to the quality tiebreak, which clusters city-states
                // against their neighbours at the near edge.
                let deviation = |distance: i32, target: i32| {
                    distance_miss(distance, target, START_RANGE_MINOR) + (distance - target).abs()
                };
                let major_miss = nearest(&spawns[..major_count.min(spawns.len())])
                    .map(|d| deviation(d, START_DISTANCE_MINOR_MAJOR))
                    .unwrap_or(0);
                let minor_miss = nearest(&spawns[major_count.min(spawns.len())..])
                    .map(|d| deviation(d, START_DISTANCE_MINOR_MINOR))
                    .unwrap_or(0);
                (
                    major_miss + minor_miss,
                    -qualities[*candidate],
                    **candidate,
                )
            })
            .copied()
        else {
            break;
        };
        spawns.push(next);
    }
}

/// Finish a major layout the sampler could not complete on a mountain-heavy
/// seed, using the major band rather than the city-state one.
fn complete_major_spawns(
    rules: &Rules,
    wm: &WorldMap,
    candidates: &[Pos],
    spawns: &mut Vec<Pos>,
    count: usize,
) {
    let qualities: BTreeMap<Pos, i32> = candidates
        .iter()
        .map(|candidate| (*candidate, start_quality(rules, wm, *candidate)))
        .collect();
    let target = spawns.len() + count;
    while spawns.len() < target {
        let Some(next) = candidates
            .iter()
            .filter(|candidate| !spawns.contains(candidate))
            .min_by_key(|candidate| {
                let nearest = spawns
                    .iter()
                    .map(|start| hex::wdistance(**candidate, *start, wm.width))
                    .min();
                (
                    nearest.map(start_distance_miss).unwrap_or(0),
                    -qualities[*candidate],
                    **candidate,
                )
            })
            .copied()
        else {
            break;
        };
        spawns.push(next);
    }
}

type RiverEdge = (Pos, Pos);

fn canonical_river_edge(a: Pos, b: Pos) -> RiverEdge {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

fn all_shared_edges(wm: &WorldMap) -> BTreeSet<RiverEdge> {
    let mut edges = BTreeSet::new();
    for pos in wm.tiles.keys().copied() {
        for neighbor in hex::neighbors(pos)
            .into_iter()
            .map(|p| hex::canon(p, wm.width))
            .filter(|p| wm.tiles.contains_key(p))
        {
            edges.insert(canonical_river_edge(pos, neighbor));
        }
    }
    edges
}

/// The other shared edges touching either endpoint of a hex edge. For two
/// adjacent hexes A/B, each endpoint also touches one common neighbor C; the
/// four possible continuations are A/C and B/C at those two vertices.
fn connected_river_edges(wm: &WorldMap, edge: RiverEdge) -> Vec<RiverEdge> {
    let (a, b) = edge;
    let b_neighbors: BTreeSet<Pos> = hex::neighbors(b)
        .into_iter()
        .map(|p| hex::canon(p, wm.width))
        .collect();
    let mut connected = BTreeSet::new();
    for common in hex::neighbors(a)
        .into_iter()
        .map(|p| hex::canon(p, wm.width))
        .filter(|p| *p != b && wm.tiles.contains_key(p) && b_neighbors.contains(p))
    {
        connected.insert(canonical_river_edge(a, common));
        connected.insert(canonical_river_edge(b, common));
    }
    connected.remove(&edge);
    connected.into_iter().collect()
}

fn river_edge_depth(
    edge: RiverEdge,
    is_water: &impl Fn(Pos) -> bool,
    distance_to_water: &impl Fn(Pos) -> i32,
) -> i32 {
    [edge.0, edge.1]
        .into_iter()
        .filter(|p| !is_water(*p))
        .map(distance_to_water)
        .max()
        .unwrap_or(0)
}

fn generate_rivers(wm: &mut WorldMap, land: &[Pos], rng: &mut Rng) {
    let water_tiles: Vec<Pos> = wm
        .tiles
        .iter()
        .filter(|(_, tile)| matches!(tile.terrain.as_str(), "ocean" | "coast"))
        .map(|(pos, _)| *pos)
        .collect();
    if water_tiles.is_empty() || land.is_empty() {
        return;
    }

    let width = wm.width;
    let is_water = |pos: Pos| {
        wm.tiles
            .get(&pos)
            .is_some_and(|tile| matches!(tile.terrain.as_str(), "ocean" | "coast"))
    };
    let distance_to_water = |pos: Pos| {
        water_tiles
            .iter()
            .map(|water| hex::wdistance(pos, *water, width))
            .min()
            .unwrap_or(0)
    };
    let mut outlets: Vec<RiverEdge> = all_shared_edges(wm)
        .into_iter()
        .filter(|(a, b)| is_water(*a) != is_water(*b))
        .filter(|edge| {
            connected_river_edges(wm, *edge)
                .into_iter()
                .any(|next| !is_water(next.0) && !is_water(next.1))
        })
        .collect();
    let river_count = 2.max(land.len() / 45).min(outlets.len());
    let mut rivers = BTreeSet::new();

    for _ in 0..river_count {
        let outlet = outlets.swap_remove(rng.below(outlets.len()));
        if rivers.contains(&outlet) {
            continue;
        }
        let mut current = outlet;
        let mut local = BTreeSet::new();
        let target_length = rng.randint(7, 16) as usize;
        for _ in 0..target_length {
            local.insert(current);
            rivers.insert(current);
            let current_depth = river_edge_depth(current, &is_water, &distance_to_water);
            let candidates: Vec<RiverEdge> = connected_river_edges(wm, current)
                .into_iter()
                .filter(|edge| !local.contains(edge))
                .filter(|(a, b)| !(is_water(*a) && is_water(*b)))
                .filter(|edge| {
                    river_edge_depth(*edge, &is_water, &distance_to_water) >= current_depth
                })
                .collect();
            if candidates.is_empty() {
                break;
            }
            let best_depth = candidates
                .iter()
                .map(|edge| river_edge_depth(*edge, &is_water, &distance_to_water))
                .max()
                .unwrap();
            let deepest: Vec<RiverEdge> = candidates
                .into_iter()
                .filter(|edge| river_edge_depth(*edge, &is_water, &distance_to_water) == best_depth)
                .collect();
            current = deepest[rng.below(deepest.len())];
            if rivers.contains(&current) {
                break;
            }
        }
    }

    for (a, b) in rivers {
        wm.set_river_edge(a, b, true);
    }
}

/// Top every strategic resource up to a supply the map can actually sustain.
///
/// Civ VI does not leave strategic supply to the same per-tile lottery it uses
/// for luxuries: its resource script places each strategic resource against a
/// quota derived from the land area and the number of civilizations, because
/// an empire that never finds Iron cannot train or upgrade into a single unit
/// on the Swordsman line. Rolling them against the whole 52-entry catalog put
/// **one** Iron and **one** Horses deposit on a 957-tile six-player map, which
/// left the Swordsman, Knight, Man-at-Arms and Musketman branches unbuildable
/// for everyone and armies of Warriors and Archers still in the field in the
/// Industrial era.
///
/// The eligibility test is the shipped one used by the lottery above, so this
/// changes how many deposits appear, never where they are allowed to appear.
fn place_strategic_quotas(
    rules: &Rules,
    wm: &mut WorldMap,
    land: &BTreeSet<Pos>,
    num_major_spawns: usize,
    rng: &mut Rng,
) {
    // Enough for every civilization to hold a source with some left to fight
    // over, and enough on a large map that the deposits are not all in one
    // empire's borders.
    let quota = (num_major_spawns + 1).max(land.len() / 90);
    let strategics: Vec<String> = rules
        .resources
        .iter()
        .filter(|(_, spec)| spec.class == "strategic")
        .map(|(name, _)| name.clone())
        .collect();
    let land_list: Vec<Pos> = land.iter().cloned().collect();
    for resource in strategics {
        let spec = &rules.resources[resource.as_str()];
        let placed = wm
            .tiles
            .values()
            .filter(|tile| tile.resource.as_deref() == Some(resource.as_str()))
            .count();
        let mut wanted = quota.saturating_sub(placed);
        if wanted == 0 {
            continue;
        }
        let mut candidates: Vec<Pos> = land_list
            .iter()
            .copied()
            .filter(|pos| {
                let tile = &wm.tiles[pos];
                if tile.resource.is_some() || !rules.is_passable(tile) {
                    return false;
                }
                let natural_wonder = tile
                    .feature
                    .as_deref()
                    .and_then(|feature| rules.features.get(feature))
                    .is_some_and(|feature| feature.natural_wonder);
                if natural_wonder {
                    return false;
                }
                let by_feature = tile
                    .feature
                    .as_ref()
                    .is_some_and(|feature| spec.feature.contains(feature));
                let by_terrain = tile.feature.is_none() && spec.terrain.contains(&tile.terrain);
                (by_feature || by_terrain) && spec.hills.is_none_or(|want| want == tile.hills)
            })
            .collect();
        while wanted > 0 && !candidates.is_empty() {
            let pick = rng.below(candidates.len());
            let pos = candidates.swap_remove(pick);
            wm.tiles.get_mut(&pos).unwrap().resource = Some(resource.clone());
            wanted -= 1;
        }
    }
}

/// Divide land into the stock number of named geographic regions. Civ VI's
/// continent count is not a promise of disconnected landmasses; a large
/// landmass can span several continents, so farthest-point Voronoi regions
/// are a closer model than equating one flood-fill component to one continent.
fn assign_continents(
    wm: &mut WorldMap,
    land: &BTreeSet<Pos>,
    width: i32,
    requested: usize,
    rng: &mut Rng,
) {
    if land.is_empty() || requested == 0 {
        return;
    }
    let count = requested.min(land.len());
    let land_vec: Vec<Pos> = land.iter().cloned().collect();
    let mut centers = vec![land_vec[rng.below(land_vec.len())]];
    while centers.len() < count {
        let next = *land_vec
            .iter()
            .filter(|p| !centers.contains(p))
            .max_by_key(|p| {
                let nearest = centers
                    .iter()
                    .map(|c| hex::wdistance(**p, *c, width))
                    .min()
                    .unwrap_or(0);
                (nearest, **p)
            })
            .unwrap();
        centers.push(next);
    }
    for pos in land {
        let continent = centers
            .iter()
            .enumerate()
            .min_by_key(|(id, center)| (hex::wdistance(*pos, **center, width), *id))
            .map(|(id, _)| id);
        wm.tiles.get_mut(pos).unwrap().continent = continent;
    }
}

fn connected_components(cells: &BTreeSet<Pos>, width: i32) -> Vec<BTreeSet<Pos>> {
    let mut seen: BTreeSet<Pos> = BTreeSet::new();
    let mut components = Vec::new();
    for start in cells {
        if seen.contains(start) {
            continue;
        }
        let mut comp: BTreeSet<Pos> = BTreeSet::new();
        comp.insert(*start);
        let mut stack = vec![*start];
        while let Some(cur) = stack.pop() {
            for n0 in hex::neighbors(cur) {
                let n = hex::canon(n0, width);
                if cells.contains(&n) && !comp.contains(&n) {
                    comp.insert(n);
                    stack.push(n);
                }
            }
        }
        seen.extend(comp.iter().cloned());
        components.push(comp);
    }
    components.sort_by_key(|component| std::cmp::Reverse(component.len()));
    components
}

#[cfg(test)]
fn largest_component(cells: &BTreeSet<Pos>, width: i32) -> BTreeSet<Pos> {
    connected_components(cells, width)
        .into_iter()
        .next()
        .unwrap_or_default()
}

#[cfg(test)]
mod river_tests {
    use super::*;
    use crate::setup::{MapScript, CIV6_MAP_SIZES};

    fn land_components(world: &WorldMap, rules: &Rules) -> Vec<BTreeSet<Pos>> {
        let land = world
            .tiles
            .iter()
            .filter(|(_, tile)| !rules.is_water(tile))
            .map(|(position, _)| *position)
            .collect();
        connected_components(&land, world.width)
    }

    /// Lakes were modeled everywhere except on the map. `terrains.json` has
    /// carried a Lake row from the beginning, and with it the Fishery, the
    /// Water Park, the Offshore Wind Farm, fresh-water Housing, and Huey
    /// Teocalli — whose stock placement rule is a Lake tile and nothing else.
    /// The generator never made one, so every bit of that was unreachable.
    ///
    /// What makes a lake a lake is that the sea cannot reach it; what separates
    /// it from an inland sea is area. Both halves are checked here, because a
    /// "lake" open to the ocean would hand out fresh water for a bay, and one
    /// over the ceiling would hand it out for a sea.
    #[test]
    fn every_land_script_grows_enclosed_lakes_within_the_stock_area_ceiling() {
        let rules = Rules::embedded();
        for (index, script) in [
            MapScript::Pangaea,
            MapScript::Continents,
            MapScript::SmallContinents,
            MapScript::InlandSea,
            MapScript::Lakes,
        ]
        .into_iter()
        .enumerate()
        {
            let mut lake_tiles = 0usize;
            let mut watered_shores = 0usize;
            for seed in 0..3u64 {
                let mut rng = Rng::new(64_000 + index as u64 * 16 + seed);
                let (world, spawns) =
                    generate_with_script(&rules, 74, 46, 6, 9, 4, 3, script, &mut rng);
                let lakes: BTreeSet<Pos> = world
                    .tiles
                    .iter()
                    .filter(|(_, tile)| tile.terrain == "lake")
                    .map(|(position, _)| *position)
                    .collect();
                lake_tiles += lakes.len();
                let where_ = format!("{script:?} seed {seed}");
                for position in &lakes {
                    let tile = &world.tiles[position];
                    assert!(!tile.hills, "{where_}: lake on hills at {position:?}");
                    assert!(
                        tile.improvement.is_none(),
                        "{where_}: a tribal village is under the lake at {position:?}"
                    );
                    assert!(
                        !spawns.contains(position),
                        "{where_}: a civilization starts in the water at {position:?}"
                    );
                    for neighbor in hex::neighbors(*position)
                        .into_iter()
                        .map(|neighbor| hex::canon(neighbor, world.width))
                    {
                        let Some(neighbor_tile) = world.tiles.get(&neighbor) else {
                            continue;
                        };
                        assert!(
                            !matches!(neighbor_tile.terrain.as_str(), "coast" | "ocean"),
                            "{where_}: the lake at {position:?} opens onto the sea"
                        );
                        if !rules.is_water(neighbor_tile) {
                            watered_shores += 1;
                        }
                    }
                }
                for body in connected_components(&lakes, world.width) {
                    assert!(
                        body.len() <= LAKE_MAX_AREA_SIZE,
                        "{where_}: a body of {} tiles is an inland sea, not a lake",
                        body.len()
                    );
                }
            }
            assert!(
                lake_tiles > 0,
                "{script:?} generated no lake in three worlds, leaving the Fishery, \
                 Huey Teocalli and fresh-water Housing unreachable on it"
            );
            assert!(
                watered_shores > 0,
                "{script:?} generated lakes that water no land"
            );
        }
    }

    /// The Lakes script inverts the usual world: `Lakes.lua` fills it with land
    /// and leaves the water inside. A player should find both kinds of inland
    /// water there — ponds that water a city, and basins too broad to drink
    /// from that still have to be sailed around.
    #[test]
    fn the_lakes_script_is_a_land_world_holding_both_lakes_and_inland_seas() {
        let rules = Rules::embedded();
        for seed in 0..3u64 {
            let mut rng = Rng::new(31_400 + seed);
            let (world, _) =
                generate_with_script(&rules, 74, 46, 6, 9, 4, 3, MapScript::Lakes, &mut rng);
            let land = world
                .tiles
                .values()
                .filter(|tile| !rules.is_water(tile))
                .count();
            let land_share = land * 100 / world.tiles.len();
            assert!(
                land_share >= 65,
                "seed {seed}: Lakes should be a world of land, got {land_share}%"
            );
            let lakes = world
                .tiles
                .values()
                .filter(|tile| tile.terrain == "lake")
                .count();
            assert!(lakes >= 40, "seed {seed}: only {lakes} lake tiles");

            // An inland sea is enclosed water the area rule leaves as Coast.
            // The polar water is excluded by construction: a body that reaches
            // the top or bottom row is the open sea, whatever its size.
            let sea: BTreeSet<Pos> = world
                .tiles
                .iter()
                .filter(|(_, tile)| matches!(tile.terrain.as_str(), "coast" | "ocean"))
                .map(|(position, _)| *position)
                .collect();
            let bodies = connected_components(&sea, world.width);
            let inland_seas = bodies
                .iter()
                .filter(|body| {
                    body.len() > LAKE_MAX_AREA_SIZE
                        && body.iter().all(|position| {
                            let (_, row) = hex::axial_to_offset(position.0, position.1);
                            row > 0 && row < world.height - 1
                        })
                })
                .count();
            assert!(
                inland_seas >= 1,
                "seed {seed}: Lakes should hold at least one basin too large to drink from, \
                 water bodies were {:?}",
                bodies.iter().map(|body| body.len()).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn stock_map_scripts_create_distinct_playable_topologies() {
        let rules = Rules::embedded();
        for (index, script) in [
            MapScript::Pangaea,
            MapScript::Continents,
            MapScript::SmallContinents,
            MapScript::InlandSea,
            MapScript::Lakes,
        ]
        .into_iter()
        .enumerate()
        {
            let mut rng = Rng::new(72_000 + index as u64);
            let (world, spawns) =
                generate_with_script(&rules, 60, 38, 6, 6, 0, 3, script, &mut rng);
            assert_eq!(spawns.len(), 12, "{script:?} spawn count");
            for (spawn_index, start) in spawns.iter().enumerate() {
                assert!(
                    spawns[spawn_index + 1..].iter().all(|other| hex::wdistance(
                        *start,
                        *other,
                        world.width
                    ) >= 4),
                    "{script:?} starts must leave room for distinct cities"
                );
            }
            // A fractal coastline sheds islands, exactly as the stock scripts
            // do, so a topology is judged by how much land its main bodies
            // hold rather than by an exact component count.
            let components = land_components(&world, &rules);
            let total: usize = components.iter().map(|component| component.len()).sum();
            let share = |count: usize| components[..count.min(components.len())]
                .iter()
                .map(|component| component.len())
                .sum::<usize>()
                * 100
                / total.max(1);
            match script {
                MapScript::Pangaea | MapScript::InlandSea | MapScript::Lakes => assert!(
                    share(1) >= 80,
                    "{script:?} should be one continent with at most a few islets, \n                     largest holds {}%",
                    share(1)
                ),
                MapScript::Continents => {
                    assert!(
                        share(2) >= 80 && components[1].len() * 3 >= components[0].len(),
                        "Continents needs two comparable landmasses, got {:?}",
                        components.iter().map(|c| c.len()).collect::<Vec<_>>()
                    )
                }
                MapScript::SmallContinents => assert!(
                    components.iter().filter(|component| component.len() >= 20).count() >= 4,
                    "Small Continents needs several separated landmasses, got {:?}",
                    components.iter().map(|c| c.len()).collect::<Vec<_>>()
                ),
                MapScript::TrueStartEarth | MapScript::TrueStartTrueEarth => unreachable!(),
            }

            let occupied_components = components
                .iter()
                .filter(|component| spawns[..6].iter().any(|spawn| component.contains(spawn)))
                .count();
            let expected = match script {
                MapScript::Continents => 2,
                MapScript::SmallContinents => 4,
                _ => 1,
            };
            assert!(
                occupied_components >= expected,
                "{script:?} should distribute majors across its landmasses"
            );

            if script == MapScript::InlandSea {
                let center = hex::offset_to_axial(world.width / 2, world.height / 2);
                assert!(rules.is_water(&world.tiles[&center]));
                for col in 0..world.width {
                    for row in [0, world.height - 1] {
                        assert!(!rules.is_water(&world.tiles[&hex::offset_to_axial(col, row)]));
                    }
                }
            }
        }
    }

    #[test]
    fn true_earth_scripts_share_geography_and_use_historic_major_starts() {
        let rules = Rules::embedded();
        let mut flat_rng = Rng::new(93_011);
        let mut globe_rng = Rng::new(93_011);
        let (flat, flat_spawns) = generate_with_script(
            &rules,
            60,
            38,
            8,
            6,
            0,
            3,
            MapScript::TrueStartEarth,
            &mut flat_rng,
        );
        let (globe, globe_spawns) = generate_with_script(
            &rules,
            60,
            38,
            8,
            6,
            0,
            3,
            MapScript::TrueStartTrueEarth,
            &mut globe_rng,
        );
        assert!(flat.tiles == globe.tiles);
        assert_eq!(flat_spawns, globe_spawns);
        assert_eq!(flat_spawns.len(), 14);

        let homelands = [
            (12.5, 41.9),
            (31.2, 30.0),
            (23.7, 38.0),
            (116.4, 39.9),
            (44.4, 32.5),
            (-99.1, 19.4),
            (32.5, 19.6),
            (64.0, 48.0),
        ];
        for (spawn, (longitude, latitude)) in flat_spawns.iter().zip(homelands) {
            let target = earth_position(flat.width, flat.height, longitude, latitude);
            assert!(
                hex::wdistance(*spawn, target, flat.width) <= 7,
                "historic start {longitude},{latitude} moved too far: {spawn:?}"
            );
            assert!(!rules.is_water(&flat.tiles[spawn]));
        }
        // Equal-area rows converge onto ocean at both poles; the recognized
        // continents occupy both hemispheres and leave both great oceans open.
        for (longitude, latitude, land) in [
            (-110.0, 40.0, true),
            (-60.0, -20.0, true),
            (20.0, 5.0, true),
            (100.0, 50.0, true),
            (135.0, -25.0, true),
            (-150.0, 0.0, false),
            (-25.0, 0.0, false),
            (0.0, 88.0, false),
        ] {
            let tile = &flat.tiles[&earth_position(flat.width, flat.height, longitude, latitude)];
            assert_eq!(!rules.is_water(tile), land, "at {longitude},{latitude}");
        }
    }

    /// Rolling strategic resources against the whole 52-entry catalog, one
    /// 13% chance per tile, put a single Iron and a single Horses deposit on a
    /// six-player Pangaea. With no Iron nobody can train or upgrade into a
    /// Swordsman, Legion, Man-at-Arms or Knight, so every civilization fought
    /// the whole game on the branches that cost no material - Warriors,
    /// Archers, Crossbowmen - and the Gold upgrade pass had nothing to buy.
    #[test]
    fn every_strategic_resource_reaches_a_playable_supply() {
        let rules = Rules::embedded();
        let strategics: Vec<&str> = rules
            .resources
            .iter()
            .filter(|(_, spec)| spec.class == "strategic")
            .map(|(name, _)| name.as_str())
            .collect();
        assert!(strategics.contains(&"iron") && strategics.contains(&"horses"));
        for (index, script) in [
            MapScript::Pangaea,
            MapScript::Continents,
            MapScript::SmallContinents,
            MapScript::InlandSea,
            MapScript::Lakes,
            MapScript::TrueStartEarth,
            MapScript::TrueStartTrueEarth,
        ]
        .into_iter()
        .enumerate()
        {
            let mut rng = Rng::new(81_000 + index as u64);
            let (world, _) = generate_with_script(&rules, 60, 38, 6, 6, 0, 3, script, &mut rng);
            for resource in &strategics {
                let count = world
                    .tiles
                    .values()
                    .filter(|tile| tile.resource.as_deref() == Some(*resource))
                    .count();
                // Six majors and six city-states: every civilization needs to
                // be able to reach a source without conquering for it.
                assert!(
                    count >= 7,
                    "{script:?} placed only {count} {resource} for six civilizations"
                );
            }
        }
    }

    #[test]
    fn generated_rivers_are_mirrored_connected_edge_chains_with_outlets() {
        let mut wm = WorldMap::new(24, 16);
        let mut land = Vec::new();
        for row in 3..13 {
            for col in 5..19 {
                let pos = hex::offset_to_axial(col, row);
                wm.tiles.get_mut(&pos).unwrap().terrain = "plains".to_string();
                land.push(pos);
            }
        }
        let mut rng = Rng::new(73);
        generate_rivers(&mut wm, &land, &mut rng);
        let river_edges: BTreeSet<RiverEdge> = all_shared_edges(&wm)
            .into_iter()
            .filter(|(a, b)| wm.has_river_edge(*a, *b))
            .collect();
        assert!(!river_edges.is_empty());
        assert!(
            river_edges.iter().any(|(a, b)| {
                wm.tiles[a].terrain == "plains" && wm.tiles[b].terrain == "plains"
            }),
            "a generated river should extend inland from its coastal outlet"
        );

        // Every serialized tile mask agrees with the neighbor's opposite edge.
        for (pos, tile) in &wm.tiles {
            for (direction, present) in tile.river_edges.iter().copied().enumerate() {
                let neighbor = hex::canon(hex::neighbors(*pos)[direction], wm.width);
                if let Some(other) = wm.get(neighbor) {
                    assert_eq!(
                        present,
                        other.river_edges[(direction + 3) % 6],
                        "river edge mismatch between {pos:?} and {neighbor:?}",
                    );
                } else {
                    assert!(!present, "river cannot leave the north/south map boundary");
                }
            }
        }

        // Each edge-connected river component reaches a land/water boundary.
        let is_water = |p: Pos| wm.tiles[&p].terrain == "ocean";
        let mut unseen = river_edges.clone();
        while let Some(start) = unseen.iter().next().copied() {
            let mut stack = vec![start];
            let mut has_outlet = false;
            unseen.remove(&start);
            while let Some(edge) = stack.pop() {
                has_outlet |= is_water(edge.0) != is_water(edge.1);
                for next in connected_river_edges(&wm, edge) {
                    if river_edges.contains(&next) && unseen.remove(&next) {
                        stack.push(next);
                    }
                }
            }
            assert!(
                has_outlet,
                "every generated river component needs a coastal outlet"
            );
        }
    }

    #[test]
    fn balanced_layout_is_independent_of_a_random_first_anchor() {
        let rules = Rules::embedded();
        let mut wm = WorldMap::new(32, 18);
        let mut landmass = BTreeSet::new();
        for row in 2..16 {
            for col in 3..29 {
                let pos = hex::offset_to_axial(col, row);
                wm.tiles.get_mut(&pos).unwrap().terrain = "plains".to_string();
                landmass.insert(pos);
            }
        }
        let candidates: Vec<Pos> = landmass.iter().copied().collect();
        let mut first_rng = Rng::new(1);
        let mut second_rng = Rng::new(999);
        let first = balanced_major_spawns(&rules, &wm, &landmass, &candidates, 6, &mut first_rng);
        let second = balanced_major_spawns(&rules, &wm, &landmass, &candidates, 6, &mut second_rng);

        assert_eq!(
            first.iter().copied().collect::<BTreeSet<_>>(),
            second.iter().copied().collect(),
            "RNG may randomize seats, but must not anchor the spatial layout"
        );
        let qualities = candidates
            .iter()
            .map(|candidate| (*candidate, start_quality(&rules, &wm, *candidate)))
            .collect();
        let score = spawn_layout_score(&wm, &landmass, &first, &qualities);
        assert!(score.minimum_separation >= 8, "{score:?}");
        assert!(score.negative_neighbor_range >= -2, "{score:?}");
    }

    #[test]
    fn stock_map_profiles_produce_spread_and_complete_spawn_sets() {
        let rules = Rules::embedded();
        for (index, size) in CIV6_MAP_SIZES.iter().enumerate() {
            let mut rng = Rng::new(10_001 + index as u64);
            let (wm, spawns) = generate(
                &rules,
                size.width,
                size.height,
                size.default_players,
                size.default_city_states,
                size.natural_wonders,
                size.continents,
                &mut rng,
            );
            assert_eq!(
                spawns.len(),
                size.default_players + size.default_city_states,
                "{} did not receive every requested spawn",
                size.name
            );

            let passable: BTreeSet<Pos> = wm
                .tiles
                .iter()
                .filter(|(_, tile)| !rules.is_water(tile) && rules.is_passable(tile))
                .map(|(pos, _)| *pos)
                .collect();
            let landmass = largest_component(&passable, wm.width);
            let majors = &spawns[..size.default_players];
            assert!(majors.iter().all(|start| landmass.contains(start)));
            assert_eq!(
                spawns.iter().copied().collect::<BTreeSet<_>>().len(),
                spawns.len(),
                "{} assigned two civilizations the same start",
                size.name
            );
            for (spawn_index, start) in spawns.iter().enumerate() {
                assert!(
                    spawns[spawn_index + 1..]
                        .iter()
                        .all(|other| hex::wdistance(*start, *other, wm.width) >= 4),
                    "{} produced starts too close to found distinct cities",
                    size.name
                );
            }
            let qualities = majors
                .iter()
                .map(|start| (*start, start_quality(&rules, &wm, *start)))
                .collect();
            let score = spawn_layout_score(&wm, &landmass, majors, &qualities);
            let balance = layout_balance_percentages(score, size.default_players, landmass.len());
            // Every start sits inside the shipped band, not merely apart.
            assert!(
                score.minimum_separation >= START_DISTANCE_MAJOR - START_RANGE_MAJOR,
                "{} crowds a start inside the shipped band: {score:?}",
                size.name
            );
            for start in majors {
                let nearest = majors
                    .iter()
                    .filter(|other| *other != start)
                    .map(|other| hex::wdistance(*start, *other, wm.width))
                    .min()
                    .unwrap_or(START_DISTANCE_MAJOR);
                assert!(
                    start_distance_miss(nearest) <= START_RANGE_MAJOR,
                    "{} places a start {nearest} from its neighbour, far outside 10..=14",
                    size.name
                );
            }
            assert!(
                balance.0 >= 50 && balance.1 >= 50 && balance.2 >= 50,
                "{} has an unfair start outlier: territory/neighbor/quality balance = {balance:?}, {score:?}",
                size.name,
            );
        }
    }

    #[test]
    fn starts_keep_the_shipped_standoff_from_natural_wonders() {
        // START_DISTANCE_MINOR_NATURAL_WONDER 3 and
        // START_DISTANCE_MAJOR_NATURAL_WONDER 2. Majors already satisfied
        // theirs because Natural Wonder tiles are not candidates at all;
        // city-states did not, landing inside 3 about one time in eighteen.
        let rules = Rules::embedded();
        for seed in 0..6u64 {
            let mut rng = Rng::new(61_000 + seed);
            let (wm, spawns) = generate(&rules, 84, 54, 8, 12, 4, 2, &mut rng);
            let wonders: Vec<Pos> = wm
                .tiles
                .iter()
                .filter(|(_, tile)| {
                    tile.feature
                        .as_ref()
                        .and_then(|feature| rules.features.get(feature))
                        .is_some_and(|feature| feature.natural_wonder)
                })
                .map(|(position, _)| *position)
                .collect();
            if wonders.is_empty() {
                continue;
            }
            for (index, start) in spawns.iter().enumerate() {
                let nearest = wonders
                    .iter()
                    .map(|wonder| hex::wdistance(*start, *wonder, wm.width))
                    .min()
                    .unwrap();
                let floor = if index < 8 {
                    START_DISTANCE_MAJOR_NATURAL_WONDER
                } else {
                    START_DISTANCE_MINOR_NATURAL_WONDER
                };
                assert!(
                    nearest >= floor,
                    "seed {seed}: start {index} sits {nearest} from a Natural Wonder, inside {floor}"
                );
            }
        }
    }

    #[test]
    fn city_states_sit_at_the_shipped_distance_from_civilizations_and_each_other() {
        // START_DISTANCE_MINOR_MAJOR_CIVILIZATION 6 and
        // START_DISTANCE_MINOR_CIVILIZATION_START 5, both within
        // START_DISTANCE_RANGE_MINOR 3. Filling the largest remaining gaps
        // instead put city-states roughly twice as far out as the game does.
        let rules = Rules::embedded();
        for seed in 0..6u64 {
            let mut rng = Rng::new(52_000 + seed);
            let (wm, spawns) = generate(&rules, 84, 54, 8, 12, 4, 2, &mut rng);
            assert_eq!(spawns.len(), 20, "seed {seed}");
            let (majors, minors) = spawns.split_at(8);
            for (index, minor) in minors.iter().enumerate() {
                let to_major = majors
                    .iter()
                    .map(|major| hex::wdistance(*minor, *major, wm.width))
                    .min()
                    .unwrap();
                assert!(
                    distance_miss(to_major, START_DISTANCE_MINOR_MAJOR, START_RANGE_MINOR) == 0,
                    "seed {seed}: city-state {to_major} from the nearest major, outside 3..=9"
                );
                if let Some(to_minor) = minors
                    .iter()
                    .enumerate()
                    .filter(|(other, _)| *other != index)
                    .map(|(_, other)| hex::wdistance(*minor, *other, wm.width))
                    .min()
                {
                    assert!(
                        distance_miss(to_minor, START_DISTANCE_MINOR_MINOR, START_RANGE_MINOR) == 0,
                        "seed {seed}: city-states {to_minor} apart, outside 2..=8"
                    );
                }
            }
        }
    }

    #[test]
    fn varied_seeds_keep_major_start_outliers_within_a_roughly_equal_band() {
        let rules = Rules::embedded();
        for seed in 0..8 {
            let mut rng = Rng::new(30_000 + seed);
            let (wm, spawns) = generate(&rules, 48, 30, 4, 6, 3, 2, &mut rng);
            assert_eq!(spawns.len(), 10, "seed {seed}");
            let passable: BTreeSet<Pos> = wm
                .tiles
                .iter()
                .filter(|(_, tile)| !rules.is_water(tile) && rules.is_passable(tile))
                .map(|(pos, _)| *pos)
                .collect();
            let landmass = largest_component(&passable, wm.width);
            let majors = &spawns[..4];
            let qualities = majors
                .iter()
                .map(|start| (*start, start_quality(&rules, &wm, *start)))
                .collect();
            let score = spawn_layout_score(&wm, &landmass, majors, &qualities);
            let balance = layout_balance_percentages(score, majors.len(), landmass.len());
            assert!(
                score.minimum_separation >= 10
                    && balance.0 >= 50
                    && balance.1 >= 50
                    && balance.2 >= 50,
                "seed {seed} has an unfair start outlier: territory/neighbor/quality balance = {balance:?}, {score:?}",
            );
        }
    }

    #[test]
    fn complete_civ6_feature_roster_is_modeled_and_generated_in_valid_biomes() {
        let rules = Rules::embedded();
        let modeled = [
            "forest",
            "jungle",
            "marsh",
            "floodplains",
            "grassland_floodplains",
            "plains_floodplains",
            "oasis",
            "reef",
            "geothermal_fissure",
            "ice",
            "volcano",
            "volcanic_soil",
            "impact_zone",
            "burning_forest",
            "burnt_forest",
            "burning_jungle",
            "burnt_jungle",
        ];
        for feature in modeled {
            assert!(
                rules.features.contains_key(feature),
                "rules are missing Civ VI feature {feature}"
            );
        }

        let mut generated = BTreeSet::new();
        for seed in [7_001, 7_002, 7_003] {
            let mut rng = Rng::new(seed);
            let (world, _) = generate(&rules, 60, 38, 4, 0, 4, 3, &mut rng);
            for (position, tile) in &world.tiles {
                let Some(feature) = tile.feature.as_deref() else {
                    continue;
                };
                generated.insert(feature.to_string());
                match feature {
                    "ice" => assert!(
                        matches!(tile.terrain.as_str(), "coast" | "ocean"),
                        "sea ice generated on {} at {position:?}",
                        tile.terrain
                    ),
                    "reef" => assert_eq!(tile.terrain, "coast", "reef at {position:?}"),
                    "volcano" => {
                        assert_eq!(tile.terrain, "mountain", "volcano at {position:?}")
                    }
                    "volcanic_soil" => assert!(
                        hex::neighbors(*position)
                            .into_iter()
                            .map(|neighbor| hex::canon(neighbor, world.width))
                            .any(|neighbor| world.tiles.get(&neighbor).is_some_and(
                                |neighbor_tile| {
                                    neighbor_tile.feature.as_deref() == Some("volcano")
                                }
                            )),
                        "volcanic soil at {position:?} has no volcano"
                    ),
                    "geothermal_fissure" => assert!(
                        hex::neighbors(*position)
                            .into_iter()
                            .map(|neighbor| hex::canon(neighbor, world.width))
                            .any(|neighbor| world.tiles.get(&neighbor).is_some_and(
                                |neighbor_tile| { neighbor_tile.terrain == "mountain" }
                            )),
                        "geothermal fissure at {position:?} is not tectonic"
                    ),
                    _ => {}
                }
            }
        }
        for feature in [
            "forest",
            "jungle",
            "marsh",
            "floodplains",
            "grassland_floodplains",
            "plains_floodplains",
            "oasis",
            "reef",
            "geothermal_fissure",
            "ice",
            "volcano",
            "volcanic_soil",
        ] {
            assert!(
                generated.contains(feature),
                "ordinary generated worlds never produced {feature}: {generated:?}"
            );
        }
    }

    #[test]
    fn natural_wonders_use_their_connected_multi_tile_footprints() {
        let rules = Rules::embedded();
        let mut rng = Rng::new(88_104);
        let (world, _) = generate(&rules, 50, 32, 2, 0, 8, 3, &mut rng);
        let expected = [
            ("great_barrier_reef", 2usize),
            ("crater_lake", 1),
            ("pantanal", 4),
            ("uluru", 1),
            ("yosemite", 2),
            ("dead_sea", 2),
            ("mount_everest", 3),
            ("pamukkale", 2),
        ];
        for (wonder, footprint) in expected {
            let tiles: BTreeSet<Pos> = world
                .tiles
                .iter()
                .filter(|(_, tile)| tile.feature.as_deref() == Some(wonder))
                .map(|(position, _)| *position)
                .collect();
            assert_eq!(tiles.len(), footprint, "{wonder} footprint");
            let mut reached = BTreeSet::new();
            let mut frontier = vec![*tiles.iter().next().unwrap()];
            while let Some(position) = frontier.pop() {
                if !reached.insert(position) {
                    continue;
                }
                frontier.extend(
                    hex::neighbors(position)
                        .into_iter()
                        .map(|neighbor| hex::canon(neighbor, world.width))
                        .filter(|neighbor| tiles.contains(neighbor)),
                );
            }
            assert_eq!(reached, tiles, "{wonder} must be contiguous");
        }
    }

    /// Civ VI's natural wonder allowance is a per-map-size row, not a range,
    /// and `NaturalWonderGenerator` keeps the ones it draws apart. Both halves
    /// matter to a player: a map short of its allowance is missing content it
    /// paid a whole biome for, and two wonders sharing a mountain range read
    /// on screen as one oversized feature rather than two discoveries.
    #[test]
    fn every_map_size_draws_its_full_wonder_allowance_well_spaced() {
        let rules = Rules::embedded();
        for size in CIV6_MAP_SIZES.iter() {
            for script in [
                MapScript::Pangaea,
                MapScript::Continents,
                MapScript::SmallContinents,
                MapScript::InlandSea,
                MapScript::Lakes,
            ] {
                for seed in 0..3u64 {
                    let mut rng = Rng::new(seed);
                    let (world, _) = generate_with_script(
                        &rules,
                        size.width,
                        size.height,
                        size.default_players,
                        size.default_city_states,
                        size.natural_wonders,
                        size.continents,
                        script,
                        &mut rng,
                    );
                    let mut footprints: BTreeMap<String, Vec<Pos>> = BTreeMap::new();
                    for (position, tile) in world.tiles.iter() {
                        if let Some(feature) = &tile.feature {
                            if rules.features[feature.as_str()].natural_wonder {
                                footprints
                                    .entry(feature.clone())
                                    .or_default()
                                    .push(*position);
                            }
                        }
                    }
                    let where_ = format!("{} {script:?} seed {seed}", size.id);
                    assert_eq!(
                        footprints.len(),
                        size.natural_wonders,
                        "{where_} placed {:?}",
                        footprints.keys().collect::<Vec<_>>()
                    );
                    let names: Vec<&String> = footprints.keys().collect();
                    for (index, first) in names.iter().enumerate() {
                        for second in &names[index + 1..] {
                            let gap = footprints[*first]
                                .iter()
                                .flat_map(|left| {
                                    footprints[*second]
                                        .iter()
                                        .map(move |right| hex::wdistance(*left, *right, size.width))
                                })
                                .min()
                                .unwrap();
                            assert!(
                                gap >= MIN_WONDER_SEPARATION,
                                "{where_}: {first} and {second} are only {gap} hexes apart"
                            );
                        }
                    }
                }
            }
        }
    }
}
