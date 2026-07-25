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
        let candidates: Vec<Pos> = wm.neighbors(current).into_iter()
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

/// The subdivision frequency of the globe a rectangle asks for.
///
/// A globe's rectangle is `5n` by `2n + 2`, so a caller that already has one
/// gets exactly the globe it names. Anything else — a caller that asked for
/// Planet with a flat map's dimensions — gets the globe closest in tiles to
/// the rectangle it wanted, which is how the shipped sizes stay recognizable.
pub fn globe_frequency(width: i32, height: i32) -> i32 {
    if width > 0 && width % 5 == 0 && height == 2 * (width / 5) + 2 {
        return width / 5;
    }
    let wanted = (width.max(1) as i64) * (height.max(1) as i64);
    (1..=64)
        .min_by_key(|frequency| {
            (crate::sphere::Sphere::tiles_for(*frequency) as i64 - wanted).abs()
        })
        .unwrap_or(1)
}

/// Where a tile samples a fractal height field.
///
/// The fields are cylindrical: they wrap east to west and stop north and
/// south, which is the shape of a flat map, so a flat map samples one by its
/// own column and row. A globe samples by longitude and latitude instead. Its
/// storage rectangle is cut into ten rhombi, and reading the field through
/// those would put a straight-line seam between two of them through every
/// desert and every mountain range; read through longitude and latitude, a
/// region stays one region wherever it lies on the globe.
fn noise_cell(wm: &WorldMap, pos: Pos) -> (i32, i32) {
    let Some(sphere) = wm.sphere() else {
        return hex::axial_to_offset(pos.0, pos.1);
    };
    use std::f64::consts::{FRAC_PI_2, PI};
    let east = (sphere.longitude(pos) + PI) / (2.0 * PI);
    let south = (FRAC_PI_2 - sphere.latitude(pos)) / PI;
    (
        (east * wm.width as f64) as i32 % wm.width.max(1),
        ((south * wm.height as f64) as i32).clamp(0, wm.height - 1),
    )
}

/// Even-odd point-in-polygon in degrees. No ring below crosses the
/// antimeridian, so the test needs no wrapping to be exact.
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

/// Earth as a deliberately low-frequency silhouette, one ring of
/// `(longitude, latitude)` degrees per landmass.
///
/// The coastlines are coarse on purpose. A globe at the sizes this engine
/// plays holds a few thousand tiles, so one tile spans several hundred
/// kilometres and any detail finer than a peninsula would vanish in the
/// sampling. What has to survive that resolution is the shape a player
/// recognises and the shape that decides play: the Mediterranean, the Sahara's
/// width, the gap at Panama, the fact that the Americas are reached by sea.
fn earth_is_land(longitude: f64, latitude: f64) -> bool {
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
    const CONTINENTS: &[&[(f64, f64)]] = &[
        NORTH_AMERICA,
        SOUTH_AMERICA,
        EURASIA,
        AFRICA,
        ARABIA_INDIA,
        SOUTHEAST_ASIA,
        AUSTRALIA,
        GREENLAND,
    ];
    CONTINENTS
        .iter()
        .chain(ISLANDS.iter())
        .any(|polygon| point_in_polygon(longitude, latitude, polygon))
}

/// Where each civilization actually began, in `(longitude, latitude)` degrees.
///
/// `CIV_NAMES` is ordered Rome, Egypt, Greece, China, Sumeria, Aztec, Nubia,
/// Scythia. Preserve that order here so a True Start map is true in play and
/// not merely Earth-shaped in the setup preview.
const EARTH_HOMELANDS: [(f64, f64); 8] = [
    (12.5, 41.9),
    (31.2, 30.0),
    (23.7, 38.0),
    (116.4, 39.9),
    (44.4, 32.5),
    (-99.1, 19.4),
    (32.5, 19.6),
    (64.0, 48.0),
];

/// The unit vector a longitude and latitude in degrees point at.
fn earth_direction(longitude: f64, latitude: f64) -> [f64; 3] {
    let (longitude, latitude) = (longitude.to_radians(), latitude.to_radians());
    [
        latitude.cos() * longitude.cos(),
        latitude.cos() * longitude.sin(),
        latitude.sin(),
    ]
}

/// Earth's land, sampled onto the globe's tiles.
///
/// Nothing here is generated: each tile asks the sphere where it is and the
/// silhouette answers, so every game of this script is played on the same
/// coastlines. The seed still moves the rivers, the resources and the terrain
/// inside them, which is where a true-start map should differ between games.
///
/// The twelve pentagons are left wherever Earth puts them. Planet holds its
/// twelve under water so that every land tile has six neighbours, and H3 turns
/// its icosahedron until all twelve fall in open ocean — but neither option is
/// open to Earth. The ten off-pole corners sit on two rings at ±26.57°, five to
/// a ring and 72° apart, and at 26.57°N the ocean comes in gaps of only 65° and
/// 127° of longitude; a 127° gap holds two of those five points and a 65° gap
/// holds one, so no spin of the globe can seat all five at sea. Since a true
/// Earth may not be rotated to suit its lattice anyway, the two that land on
/// Earth — one in the Sahara near 0°E, one in the Indus near 72°E — stay land
/// and simply have five neighbours. Adjacency, rings and distance all read the
/// tile graph, so those two tiles are irregular, not special-cased.
fn earth_land(wm: &WorldMap) -> BTreeSet<Pos> {
    let Some(sphere) = wm.sphere() else {
        return BTreeSet::new();
    };
    sphere
        .positions()
        .filter(|pos| {
            earth_is_land(
                sphere.longitude(*pos).to_degrees(),
                sphere.latitude(*pos).to_degrees(),
            )
        })
        .collect()
}

/// Seat each civilization on the viable tile closest to its homeland.
///
/// Closeness is measured on the globe, not in the storage rectangle: the tile
/// whose centre points nearest the homeland's direction wins. Sites are handed
/// out in `CIV_NAMES` order, and a start keeps clear of the ones already
/// placed by the widest margin that still leaves every remaining seat a tile —
/// so Rome and Greece stay distinct neighbours rather than collapsing onto the
/// same Aegean plain.
fn historic_major_spawns(wm: &WorldMap, candidates: &[Pos], count: usize) -> Vec<Pos> {
    let Some(sphere) = wm.sphere() else {
        return Vec::new();
    };
    let mut available: Vec<Pos> = candidates.to_vec();
    let mut starts: Vec<Pos> = Vec::new();
    for index in 0..count {
        if available.is_empty() {
            break;
        }
        let (longitude, latitude) = EARTH_HOMELANDS[index % EARTH_HOMELANDS.len()];
        let target = earth_direction(longitude, latitude);
        let seats_left = count - index;
        let separation = (0..=4)
            .rev()
            .find(|separation| {
                let taken = taken_within(sphere, &starts, *separation);
                available
                    .iter()
                    .filter(|candidate| !taken.contains(candidate))
                    .count()
                    >= seats_left
            })
            .unwrap_or(0);
        let taken = taken_within(sphere, &starts, separation);
        let selected = available
            .iter()
            .enumerate()
            .filter(|(_, candidate)| !taken.contains(candidate))
            .max_by(|(_, a), (_, b)| {
                let toward = |pos: &Pos| {
                    sphere.center(*pos).map_or(-1.0, |center| {
                        center[0] * target[0] + center[1] * target[1] + center[2] * target[2]
                    })
                };
                toward(a)
                    .partial_cmp(&toward(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(candidate_index, _)| candidate_index)
            .unwrap_or(0);
        starts.push(available.swap_remove(selected));
    }
    starts
}

/// Every tile within `radius` steps of a start already placed.
fn taken_within(sphere: &crate::sphere::Sphere, starts: &[Pos], radius: i32) -> BTreeSet<Pos> {
    starts
        .iter()
        .flat_map(|start| sphere.disk(*start, radius))
        .collect()
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
        MapScript::Planet => {
            // A globe has no edge to hold the ocean against, so its land is
            // seeded rather than cut out: continents are dropped around the
            // sphere at arm's length from one another, kept off the two caps so
            // the poles stay open water for the ice to form on, and grown until
            // the world is about a third land. Every continent is separated by
            // at least one tile of water, so "sail west and you arrive from the
            // east" is a fact about this map in every direction, not just one.
            //
            // The twelve pentagons are held under water as well. Uber's H3
            // grid, built the same way, turns its icosahedron so that all
            // twelve corners fall in the ocean and the pentagons never surface
            // in the data; a generated world can just be told to keep them
            // wet. Every land tile then has six neighbours, so district
            // adjacency, city work radii and the rest behave exactly as they
            // do on a flat map.
            let pole = 0.93;
            let pentagons: BTreeSet<Pos> = wm
                .sphere()
                .map(|sphere| sphere.pentagons().into_iter().collect())
                .unwrap_or_default();
            let open_water: BTreeSet<Pos> = wm
                .tiles
                .keys()
                .copied()
                .filter(|pos| wm.polar_fraction(*pos) < pole && !pentagons.contains(pos))
                .collect();
            let continents = num_major_spawns.div_ceil(2).clamp(3, 7);
            let per_continent = (wm.tiles.len() as f64 * 0.31) as usize / continents;
            // A grown blob of `n` tiles is roughly a disc of radius √(n/3); ask
            // for seeds a little under two radii apart and the continents stand
            // clear of one another without the search having to back off far.
            let mut separation = (1.6 * (per_continent as f64 / 3.0).sqrt()) as i32;
            let pool: Vec<Pos> = open_water.iter().copied().collect();
            let mut seeds: Vec<Pos> = Vec::new();
            while seeds.len() < continents {
                let mut placed = false;
                for _ in 0..(4 * pool.len()).min(2_000) {
                    let candidate = pool[rng.below(pool.len())];
                    if seeds
                        .iter()
                        .all(|seed| wm.distance(candidate, *seed) >= separation)
                    {
                        seeds.push(candidate);
                        placed = true;
                        break;
                    }
                }
                if !placed {
                    if separation <= 2 {
                        break;
                    }
                    separation -= 2;
                }
            }

            let mut land = BTreeSet::new();
            let mut open = open_water;
            for seed in seeds {
                if !open.contains(&seed) {
                    continue;
                }
                let blob = grow_blob(wm, &open, seed, per_continent, rng);
                for pos in &blob {
                    for neighbor in wm.neighbors(*pos) {
                        open.remove(&neighbor);
                    }
                    open.remove(pos);
                }
                land.extend(blob);
            }
            // Islands: enough to give the open ocean something in it without
            // turning the sea lanes into an archipelago.
            for _ in 0..continents * 3 {
                if open.is_empty() {
                    break;
                }
                let remaining: Vec<Pos> = open.iter().copied().collect();
                let seed = remaining[rng.below(remaining.len())];
                let island = grow_blob(wm, &open, seed, 3 + rng.below(9), rng);
                for pos in &island {
                    for neighbor in wm.neighbors(*pos) {
                        open.remove(&neighbor);
                    }
                    open.remove(pos);
                }
                land.extend(island);
            }
            land
        }
        MapScript::TrueStartEarth => {
            // The one script that is read rather than rolled. See
            // [`earth_land`] for what the globe is asked, and why the two
            // pentagons that fall on land are allowed to stay there.
            earth_land(wm)
        }
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
/// The two globes carry them like the continent scripts do: their landmasses
/// have interiors, and a lake is judged by the same enclosure rule there as
/// anywhere else. Earth's interiors are the ones that earned the rule.
fn large_lake_budget(script: MapScript, num_continents: usize) -> usize {
    match script {
        MapScript::Lakes => num_continents * 4,
        MapScript::Pangaea | MapScript::InlandSea => num_continents,
        MapScript::Continents => num_continents / 2,
        MapScript::Planet | MapScript::TrueStartEarth => num_continents / 2,
        MapScript::SmallContinents => 0,
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
    wm.around(pos)
        .into_iter()
        .all(|neighbor| match wm.tiles.get(&neighbor) {
            // Civ VI's polar rows are always ocean, so a plot on the world's
            // edge is coastal land there and never floods. A CIVVIS script may
            // put land on the rim — Inland Sea rings its basin that way — and
            // treating the edge as a shore keeps that rim whole rather than
            // punching a lake through it.
            None => false,
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
    for neighbor in wm.neighbors(pos) {
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
    let water: BTreeSet<Pos> = wm
        .tiles
        .iter()
        .filter(|(_, tile)| matches!(tile.terrain.as_str(), "coast" | "ocean"))
        .map(|(pos, _)| *pos)
        .collect();
    let enclosed = connected_components(wm, &water);
    for body in enclosed {
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
    // Planet is stored in a rectangle of its own shape, so the size's globe
    // is built rather than the cylinder the other scripts lay out.
    let mut wm = if script.is_globe() {
        WorldMap::globe(globe_frequency(width, height))
    } else {
        WorldMap::new(width, height)
    };

    // --- landmass topology selected by the stock-style map script
    let mut land = generate_land(&wm, script, num_major_spawns, rng);

    let land_list: Vec<Pos> = land.iter().cloned().collect();

    // --- relief, then climate. The stock generator settles elevation first
    // (MountainsCliffs.lua) and only then paints biomes over it, because the
    // mountain fractal has to be free of the latitude bands to run across them.
    apply_tectonics(&mut wm, &land, rng);
    assign_biomes(&mut wm, &land_list, rng);

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
                && wm.neighbors(**pos).iter().any(|n| land.contains(n))
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
                    && wm.neighbors(**pos).into_iter()
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
        for neighbor in wm.neighbors(pos).into_iter()
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
    {
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
            wm.neighbors(*position).into_iter()
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
            .all(|other| wm.distance(position, *other) >= 4)
        {
            wm.tiles.get_mut(&position).unwrap().feature = Some("volcano".into());
            volcanoes.push(position);
        }
    }

    // Ancient eruption deposits make volcanoes legible even while dormant.
    // Guarantee one deposit where geography allows, then scatter a few more
    // without consuming the RNG differently for later per-tile feature rolls.
    for volcano in &volcanoes {
        let mut foothills: Vec<Pos> = wm.neighbors(*volcano).into_iter()
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
                && wm.neighbors(*position).into_iter()
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
            .all(|other| wm.distance(position, *other) >= 3)
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
            matches!(tile.terrain.as_str(), "coast" | "ocean")
                && wm.polar_fraction(**position) > 0.82
        })
        .map(|(position, _)| *position)
        .collect();
    for position in polar_water {
        let chance = ((wm.polar_fraction(position) - 0.82) / 0.18 * 0.72).clamp(0.0, 0.72);
        if rng.chance(chance) {
            wm.tiles.get_mut(&position).unwrap().feature = Some("ice".into());
        }
    }

    // --- vegetative, wetland and river-basin features, and the reefs that
    // supply the Campus's major Gathering Storm adjacency source.
    add_features(&mut wm, &land, rng);

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
                .all(|placed| wm.distance(position, *placed) >= separation)
        };
        let cluster_from = |anchor: Pos, preferred_only: bool, separation: i32| {
            let mut cluster = vec![anchor];
            while cluster.len() < footprint {
                let mut frontier: Vec<Pos> = cluster
                    .iter()
                    .flat_map(|position| wm.neighbors(*position))
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

    assign_continents(&mut wm, &land, num_continents, rng);

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
            wm.neighbors(**position).into_iter()
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

    // --- spawns. Civilization VI does not search for good plots and hope they
    // come out spread: it divides the map into one region per seat of roughly
    // equal fertility and gives each region a start. So does this. Which
    // landmass a seat lands on falls out of the division rather than being
    // allocated by script, so an ocean-separated world still seats every
    // continent it can afford to seat.
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
    let components = connected_components(&wm, &passable);
    let fertility: BTreeMap<Pos, i32> = passable
        .iter()
        .map(|position| (*position, tile_fertility(rules, &wm.tiles[position])))
        .collect();

    let wonders: Vec<Pos> = wm
        .tiles
        .iter()
        .filter(|(_, tile)| {
            tile.feature
                .as_deref()
                .and_then(|feature| rules.features.get(feature))
                .is_some_and(|feature| feature.natural_wonder)
        })
        .map(|(position, _)| *position)
        .collect();
    // Founding clears the centre tile's resource, so a start standing on a
    // strategic deposit deletes it. With the quota at one source per
    // civilization plus one, a few starts on iron is the difference between a
    // world that can field Swordsmen and one that cannot — so the deposits are
    // taken out of the pool rather than out of the map.
    let every_candidate = candidates_for(&passable, total_spawns);
    let all_candidates = {
        let spared: Vec<Pos> = every_candidate
            .iter()
            .copied()
            .filter(|position| {
                wm.tiles[position]
                    .resource
                    .as_deref()
                    .and_then(|resource| rules.resources.get(resource))
                    .is_none_or(|spec| spec.class != "strategic")
            })
            .collect();
        if spared.len() >= total_spawns {
            spared
        } else {
            every_candidate.clone()
        }
    };
    // Standing beside a Natural Wonder costs a capital the tiles it cannot work
    // from its first turn, so the shipped standoffs are applied to the pool a
    // region picks from. That was the wrong place for it while a search over
    // the pool decided the *spacing* too; now the regions fix the spacing and
    // the pool only decides where inside a region a start stands.
    let pool_clear_of_wonders = |floor: i32, needed: usize| -> BTreeSet<Pos> {
        let clear: BTreeSet<Pos> = all_candidates
            .iter()
            .copied()
            .filter(|position| {
                wonders
                    .iter()
                    .all(|wonder| wm.distance(*position, *wonder) >= floor)
            })
            .collect();
        if clear.len() >= needed {
            clear
        } else {
            all_candidates.iter().copied().collect()
        }
    };

    let major_pool = pool_clear_of_wonders(START_DISTANCE_MAJOR_NATURAL_WONDER, num_major_spawns);
    let major_regions = regions_for_seats(&wm, &components, &fertility, num_major_spawns);
    let mut spawns = if script == MapScript::TrueStartEarth {
        // Earth does not divide into regions: the whole point of the script is
        // that Rome opens in Italy and the Aztecs open in Mexico, however
        // lopsided that leaves the continents.
        // ...and it does not spare strategic deposits either: a homeland is a
        // handful of tiles wide, and skipping the one with iron on it moves
        // Rome out of Italy.
        historic_major_spawns(&wm, &every_candidate, num_major_spawns)
    } else {
        let mut seated = regional_starts(
            rules,
            &wm,
            &major_regions,
            &major_pool,
            &fertility,
            &[],
            MAJOR_START_BUFFER,
            MAJOR_START_BUFFER,
        );
        equalize_start_quality(
            rules,
            &wm,
            &major_regions,
            &major_pool,
            &fertility,
            &mut seated,
            &[],
        );
        let reachable: Vec<Pos> = major_regions.iter().flatten().copied().collect();
        balance_territory(
            rules,
            &wm,
            &reachable,
            &major_regions,
            &major_pool,
            &mut seated,
        );
        let mut starts: Vec<Pos> = seated.into_iter().map(|(_, start)| start).collect();
        // Seat order should not correlate with the order the regions were cut.
        for index in (1..starts.len()).rev() {
            let other = rng.below(index + 1);
            starts.swap(index, other);
        }
        starts
    };
    if spawns.len() < num_major_spawns {
        let missing = num_major_spawns - spawns.len();
        fill_remaining_starts(rules, &wm, &major_pool, &mut spawns, missing);
    }

    // City-states get a second, finer set of regions once the majors are down,
    // the way `StartPositioner.DivideMapIntoMinorRegions` cuts one. Here they
    // are cut *inside* each civilization's region and apportioned across them,
    // so every civilization has city-states of its own to court. Dividing the
    // whole world again instead is what let the greedy fill this replaced chain
    // four city-states around one civilization while another had none within
    // ten hexes — measured on every stock profile.
    let minor_pool = pool_clear_of_wonders(START_DISTANCE_MINOR_NATURAL_WONDER, num_minor_spawns);
    let minor_regions = if major_regions.is_empty() || spawns.is_empty() {
        regions_for_seats(&wm, &components, &fertility, num_minor_spawns)
    } else {
        // Each civilization's own ground, meaning the land nearer to it than to
        // anyone else — not the region it was given, which its start may sit
        // off-centre in. Cutting the city-state regions out of the cell is what
        // keeps a city-state on the side of the frontier it was meant for.
        let mut cells: Vec<Vec<Pos>> = vec![Vec::new(); spawns.len()];
        for tile in major_regions.iter().flatten() {
            if let Some((_, owner)) = spawns
                .iter()
                .enumerate()
                .map(|(index, start)| (wm.distance(*tile, *start), index))
                .min()
            {
                cells[owner].push(*tile);
            }
        }
        for cell in cells.iter_mut() {
            cell.sort_unstable();
        }
        let weights: Vec<i64> = cells
            .iter()
            .map(|cell| {
                cell.iter()
                    .map(|position| fertility.get(position).copied().unwrap_or(1) as i64)
                    .sum()
            })
            .collect();
        let allocation = apportion(&weights, num_minor_spawns);
        let mut cut = Vec::with_capacity(num_minor_spawns);
        for (cell, count) in cells.iter().zip(allocation) {
            if count == 0 {
                continue;
            }
            cut.extend(divide_into_regions(&wm, cell, &fertility, count));
        }
        cut
    };
    let majors: Vec<Pos> = spawns.clone();
    let minors = regional_starts(
        rules,
        &wm,
        &minor_regions,
        &minor_pool,
        &fertility,
        &majors,
        MINOR_MAJOR_BUFFER,
        MINOR_MINOR_BUFFER,
    );
    spawns.extend(minors.into_iter().map(|(_, start)| start));
    if spawns.len() < total_spawns {
        let missing = total_spawns - spawns.len();
        fill_remaining_starts(rules, &wm, &minor_pool, &mut spawns, missing);
    }
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

    let cells: Vec<(i32, i32)> = land.iter().map(|pos| noise_cell(wm, *pos)).collect();
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
        let (col, row) = noise_cell(wm, *pos);
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
            wm.neighbors(*pos).into_iter()
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
fn assign_biomes(wm: &mut WorldMap, land: &[Pos], rng: &mut Rng) {
    let (width, height) = (wm.width, wm.height);
    let deserts = Fractal::new(rng, width, height, 3);
    let plains = Fractal::new(rng, width, height, 3);
    let variation = Fractal::new(rng, width, height, 3);
    let desert_bottom = deserts.percentile(100 - DESERT_PERCENT);
    let plains_bottom = plains.percentile(100 - PLAINS_PERCENT);

    for pos in land {
        let (col, row) = noise_cell(wm, *pos);
        if wm.tiles[pos].terrain == "mountain" {
            continue;
        }
        let base = wm.polar_fraction(*pos);
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
    wm.neighbors(pos).into_iter()
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

fn add_features(wm: &mut WorldMap, land: &BTreeSet<Pos>, rng: &mut Rng) {
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

            // Rainforest keeps to twenty degrees either side of the equator.
            // A globe measures that on the sphere; a flat map counts rows.
            let tropical = if wm.sphere().is_some() {
                wm.polar_fraction(pos) <= 20.0 / 90.0
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
            let latitude = wm.polar_fraction(pos);
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
            // Warm water first: on a flat map that is the distance in rows
            // from the equator, and on a globe the same distance measured
            // around the sphere instead.
            let from_equator = if wm.sphere().is_some() {
                (latitude * (height as f64 - 1.0) / 2.0) as i32
            } else {
                (row - equator).abs()
            };
            let score = 3 * from_equator + crowding;
            if (rng.below(200) as i32) >= score {
                wm.tiles.get_mut(&pos).unwrap().feature = Some("reef".into());
                reefs += 1;
            }
        }
    }
}

/// How even a finished layout came out. Nothing in the generator consults it —
/// region division is what produces the spread — but it is the vocabulary the
/// tests below hold that spread to, so it lives beside the placer it grades.
#[cfg(test)]
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
        || wm.neighbors(pos).into_iter()
            .any(|neighbor| {
                wm.get(neighbor)
                    .is_some_and(|tile| tile.feature.as_deref() == Some("oasis"))
            });
    let coastal = wm.neighbors(pos).into_iter()
        .any(|neighbor| wm.get(neighbor).is_some_and(|tile| rules.is_water(tile)));

    let mut nearby_yields = Vec::new();
    let mut workable_land = 0;
    let mut seen = BTreeSet::new();
    for tile_pos in wm.disk(pos, 3) {
        if !seen.insert(tile_pos) {
            continue;
        }
        let Some(tile) = wm.get(tile_pos) else {
            continue;
        };
        if !rules.is_water(tile) && rules.is_passable(tile) {
            workable_land += 1;
        }
        if tile_pos == pos || wm.distance(pos, tile_pos) > 2 {
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
    let nearby: Vec<&crate::world::Tile> = wm
        .disk(pos, 3)
        .into_iter()
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

#[cfg(test)]
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
                    .map(|other| wm.distance(*start, *other))
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
            .map(|(index, start)| (wm.distance(*tile, *start), index))
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

#[cfg(test)]
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
// `START_DISTANCE_RANGE_MINOR` is declared in `GlobalParameters.xml` and used
// by no shipped map script, so there is no minor band to model — the two minor
// distances below are plain buffers.
/// Shipped `START_DISTANCE_MINOR_NATURAL_WONDER`: a city-state keeps this much
/// clear of a Natural Wonder. (`START_DISTANCE_MAJOR_NATURAL_WONDER` is 2, and
/// major placement already satisfies it — measured 0 violations in 96 starts —
/// because Natural Wonder tiles are excluded from the candidate set outright.)
pub(crate) const START_DISTANCE_MINOR_NATURAL_WONDER: i32 = 3;
/// Shipped `START_DISTANCE_MAJOR_NATURAL_WONDER`.
pub(crate) const START_DISTANCE_MAJOR_NATURAL_WONDER: i32 = 2;

/// `Game::can_found_city` refuses a site within four hexes of an existing city,
/// so two starts closer than this are not two starts: the second seat's Settler
/// cannot found where it stands, and a city-state placed there is teleported by
/// `Game::city_state_site` to whatever tile on the map has the most room —
/// undoing the even spread this module just worked for. It is a hard floor
/// rather than a target, and the only rule allowed to overrule the buffers
/// below.
pub(crate) const MIN_START_SEPARATION: i32 = 4;

/// The shipped numbers above are **floors, not targets.** Civilization VI reads
/// them in `AssignStartingPlots:__MajorCivBuffer`, which rejects a major site
/// when any major already placed is within
/// `START_DISTANCE_MAJOR_CIVILIZATION - START_DISTANCE_RANGE_MAJOR` — so 12 and
/// 2 describe eleven hexes of clearance, not a band centred on twelve. CIVVIS
/// read them as a target and pulled every start back toward it, which left a
/// map with room to spare unused and, worse, let the pull *overrule* clearance:
/// measured over 72 generated worlds, starts landed as close as 2 hexes apart
/// on Continents, Small Continents and Planet, inside the radius in which no
/// city can be founded at all.
pub(crate) const MAJOR_START_BUFFER: i32 = START_DISTANCE_MAJOR - START_RANGE_MAJOR;
/// `__MinorMajorCivBuffer`: a city-state may not sit within
/// `START_DISTANCE_MINOR_MAJOR_CIVILIZATION` of a major civilization.
pub(crate) const MINOR_MAJOR_BUFFER: i32 = START_DISTANCE_MINOR_MAJOR;
/// `__MinorMinorCivBuffer`: nor within `START_DISTANCE_MINOR_CIVILIZATION_START`
/// of another city-state.
pub(crate) const MINOR_MINOR_BUFFER: i32 = START_DISTANCE_MINOR_MINOR;

/// How much one tile is worth to whoever ends up working it. Civilization VI
/// scores every plot's fertility before it considers a single start and then
/// divides the map into one region per civilization of roughly equal total
/// fertility (`StartPositioner.DivideMapIntoMajorRegions`, called from
/// `AssignStartingPlots` before any plot is chosen). The regions are what makes
/// a layout even; the buffers only keep two regions' best sites from touching.
///
/// Every passable land tile is worth at least one, so bare ground still counts
/// as room. Without that floor a region is whatever small patch of very good
/// land adds up to a share, and its civilization starts hemmed in.
fn tile_fertility(rules: &Rules, tile: &crate::world::Tile) -> i32 {
    if rules.is_water(tile) || !rules.is_passable(tile) {
        return 0;
    }
    let yields = rules.tile_yields(tile);
    1 + (yields.food * 2.0
        + yields.production * 2.0
        + yields.gold
        + yields.science
        + yields.culture
        + yields.faith) as i32
}

/// Lloyd passes the region division is allowed. The loop leaves early once the
/// centres stop moving, which on the stock sizes happens well inside six.
const REGION_PASSES: usize = 6;
/// A region's centre is measured against an even sample of it rather than all
/// of it, because weighing every pair is quadratic and a huge world's region
/// runs to hundreds of tiles.
const REGION_SAMPLE: usize = 48;
/// How much a hex away from its region's centre costs a candidate site, in the
/// units `start_quality` returns. A start should be good *and* in the middle of
/// the land it is being given; without the pull the best tile in a region is
/// routinely on its border, facing a neighbour across a shared frontier while
/// its own half of the region goes unclaimed and the territory it actually
/// holds collapses. Sites inside a region differ by roughly 50 points, so 20 a
/// hex buys a genuinely better capital two or three hexes out and no further.
const REGION_CENTRALITY_PULL: i32 = 20;

/// The middle of a region: the tile whose fertility-weighted distance to the
/// rest of the region is smallest. A hex globe has no arithmetic mean, so this
/// is a medoid over an evenly-strided sample — a stride and not a draw, so it
/// never touches the map's random stream.
fn region_center(wm: &WorldMap, region: &[Pos], fertility: &BTreeMap<Pos, i32>) -> Option<Pos> {
    if region.is_empty() {
        return None;
    }
    let stride = region.len().div_ceil(REGION_SAMPLE).max(1);
    let sample: Vec<Pos> = region.iter().copied().step_by(stride).collect();
    region.iter().copied().min_by_key(|position| {
        let cost: i64 = sample
            .iter()
            .map(|other| {
                wm.distance(*position, *other) as i64 * fertility.get(other).copied().unwrap_or(1) as i64
            })
            .sum();
        (cost, *position)
    })
}

/// Divide the viable land into `count` regions of roughly equal fertility, one
/// per seat. This is CIVVIS's `DivideMapIntoMajorRegions`: seeds are spread by
/// farthest-point sampling — which lands them on separate continents without
/// being told the continents exist — and then a capacity-constrained Lloyd
/// relaxation trades tiles between neighbouring regions until each holds about
/// the same fertility.
///
/// The capacity is what makes the result *even* rather than merely spread. A
/// plain nearest-centre Voronoi over an irregular coastline hands one seat a
/// third of a continent and another a peninsula; refusing a region more than
/// its share forces the overflow into its neighbours and moves the centres
/// apart on the next pass.
fn divide_into_regions(
    wm: &WorldMap,
    land: &[Pos],
    fertility: &BTreeMap<Pos, i32>,
    count: usize,
) -> Vec<Vec<Pos>> {
    if count == 0 || land.is_empty() {
        return Vec::new();
    }
    let count = count.min(land.len());
    let mut centers: Vec<Pos> = Vec::with_capacity(count);
    if let Some(first) = land
        .iter()
        .max_by_key(|position| (fertility.get(*position).copied().unwrap_or(1), **position))
    {
        centers.push(*first);
    }
    while centers.len() < count {
        let Some(next) = land
            .iter()
            .filter(|position| !centers.contains(position))
            .max_by_key(|position| {
                (
                    centers
                        .iter()
                        .map(|center| wm.distance(**position, *center))
                        .min()
                        .unwrap_or(0),
                    **position,
                )
            })
            .copied()
        else {
            break;
        };
        centers.push(next);
    }

    let total: i64 = land
        .iter()
        .map(|position| fertility.get(position).copied().unwrap_or(1) as i64)
        .sum();
    // A little slack keeps the last region from being handed a ring of
    // leftovers on the far side of the map purely to top its quota up.
    let capacity = (total * 102 / (100 * centers.len().max(1) as i64)).max(1);

    let mut regions: Vec<Vec<Pos>> = Vec::new();
    for _ in 0..REGION_PASSES {
        let mut reach: Vec<(i32, Pos, usize)> = Vec::with_capacity(land.len() * centers.len());
        for position in land {
            for (index, center) in centers.iter().enumerate() {
                reach.push((wm.distance(*position, *center), *position, index));
            }
        }
        reach.sort_unstable();
        let mut owner: BTreeMap<Pos, usize> = BTreeMap::new();
        let mut load = vec![0_i64; centers.len()];
        for (_, position, index) in &reach {
            if owner.contains_key(position) || load[*index] >= capacity {
                continue;
            }
            owner.insert(*position, *index);
            load[*index] += fertility.get(position).copied().unwrap_or(1) as i64;
        }
        // Anything whose every region filled up joins its nearest one anyway;
        // `reach` is sorted by distance, so the first entry for a tile is it.
        for (_, position, index) in &reach {
            owner.entry(*position).or_insert(*index);
        }
        regions = vec![Vec::new(); centers.len()];
        for (position, index) in owner {
            regions[index].push(position);
        }

        let moved: Vec<Pos> = regions
            .iter()
            .zip(&centers)
            .map(|(region, center)| region_center(wm, region, fertility).unwrap_or(*center))
            .collect();
        if moved == centers {
            break;
        }
        centers = moved;
    }
    regions
}

/// Hand `seats` out over `weights` by largest remainder — the apportionment
/// rule that gives each landmass its fair share of the world's seats and lets
/// rounding fall where the shortfall is largest, rather than where the list
/// happens to start.
fn apportion(weights: &[i64], seats: usize) -> Vec<usize> {
    let total: i64 = weights.iter().sum();
    if total <= 0 || weights.is_empty() {
        return vec![0; weights.len()];
    }
    let mut given: Vec<usize> = weights
        .iter()
        .map(|weight| (seats as i64 * weight / total) as usize)
        .collect();
    let mut order: Vec<usize> = (0..weights.len()).collect();
    order.sort_by_key(|index| {
        let remainder = seats as i64 * weights[*index] - given[*index] as i64 * total;
        (std::cmp::Reverse(remainder), *index)
    });
    let mut left = seats.saturating_sub(given.iter().sum::<usize>());
    while left > 0 {
        let before = left;
        for index in &order {
            if left == 0 {
                break;
            }
            given[*index] += 1;
            left -= 1;
        }
        if left == before {
            break;
        }
    }
    given
}

/// Cut one region per seat out of the world, landmass by landmass. Seats are
/// apportioned to landmasses by fertility first, so a region never spans an
/// ocean — a region that did would put its centre in open water and its start
/// on whichever shore happened to score best, which is how a civilization ends
/// up wedged against a neighbour with a fifth of anyone else's land.
///
/// A landmass worth less than half a seat's share is not somewhere to start.
/// It keeps its terrain and its resources and stays on the map as unclaimed
/// ground worth sailing to; it just does not get handed a civilization whose
/// whole game would be a different game from everyone else's.
fn regions_for_seats(
    wm: &WorldMap,
    components: &[BTreeSet<Pos>],
    fertility: &BTreeMap<Pos, i32>,
    seats: usize,
) -> Vec<Vec<Pos>> {
    if seats == 0 || components.is_empty() {
        return Vec::new();
    }
    let weights: Vec<i64> = components
        .iter()
        .map(|component| {
            component
                .iter()
                .map(|position| fertility.get(position).copied().unwrap_or(1) as i64)
                .sum()
        })
        .collect();
    let total: i64 = weights.iter().sum();
    let floor = total / (2 * seats as i64).max(1);
    let mut eligible: Vec<usize> = (0..components.len())
        .filter(|index| weights[*index] >= floor)
        .collect();
    if eligible.is_empty() {
        eligible = vec![0];
    }
    let shares: Vec<i64> = eligible.iter().map(|index| weights[*index]).collect();
    let allocation = apportion(&shares, seats);
    let mut regions = Vec::with_capacity(seats);
    for (slot, index) in eligible.iter().enumerate() {
        if allocation[slot] == 0 {
            continue;
        }
        let land: Vec<Pos> = components[*index].iter().copied().collect();
        regions.extend(divide_into_regions(wm, &land, fertility, allocation[slot]));
    }
    regions
}

/// Whether a site keeps `buffer` hexes of clearance from every start in `from`.
/// Civilization VI's buffers reject a plot *at* the distance, so clearance is
/// strictly greater than the parameter.
fn clear_of(wm: &WorldMap, position: Pos, from: &[Pos], buffer: i32) -> bool {
    from.iter().all(|start| wm.distance(position, *start) > buffer)
}

/// Give each region its start: the site that is both good and central, subject
/// to the shipped clearance buffers against everything already on the map.
///
/// A region that cannot honour its buffer relaxes it a hex at a time rather
/// than failing, down to `MIN_START_SEPARATION` — the radius inside which a
/// city cannot be founded at all, which is never given up. A region with no
/// site even that clear is left to the caller's fallback rather than handed a
/// start that the game will silently move somewhere else.
fn regional_starts(
    rules: &Rules,
    wm: &WorldMap,
    regions: &[Vec<Pos>],
    candidates: &BTreeSet<Pos>,
    fertility: &BTreeMap<Pos, i32>,
    foreign: &[Pos],
    foreign_buffer: i32,
    own_buffer: i32,
) -> Vec<(usize, Pos)> {
    let mut seated: Vec<(usize, Pos)> = Vec::with_capacity(regions.len());
    let mut placed: Vec<Pos> = Vec::with_capacity(regions.len());
    for (index, region) in regions.iter().enumerate() {
        let Some(center) = region_center(wm, region, fertility) else {
            continue;
        };
        let pool: Vec<Pos> = region
            .iter()
            .copied()
            .filter(|position| candidates.contains(position))
            .collect();
        let pool = if pool.is_empty() { region.clone() } else { pool };
        let worth = |position: &Pos| {
            start_quality(rules, wm, *position) - REGION_CENTRALITY_PULL * wm.distance(*position, center)
        };
        let mut chosen = None;
        for relaxed in 0..=foreign_buffer.max(own_buffer) {
            let foreign_want = (foreign_buffer - relaxed).max(MIN_START_SEPARATION - 1);
            let own_want = (own_buffer - relaxed).max(MIN_START_SEPARATION - 1);
            chosen = pool
                .iter()
                .filter(|position| {
                    clear_of(wm, **position, foreign, foreign_want)
                        && clear_of(wm, **position, &placed, own_want)
                })
                .max_by_key(|position| (worth(position), **position))
                .copied();
            if chosen.is_some() {
                break;
            }
        }
        if let Some(position) = chosen {
            seated.push((index, position));
            placed.push(position);
        }
    }
    seated
}

/// Fill seats no region could seat, on a map whose land is too broken or too
/// crowded for one start apiece. Takes the site with the most room left,
/// which is the same rule `Game::city_state_site` would apply afterwards —
/// applying it here keeps the choice inside the generator, where the map is
/// still visible.
fn fill_remaining_starts(
    rules: &Rules,
    wm: &WorldMap,
    candidates: &BTreeSet<Pos>,
    spawns: &mut Vec<Pos>,
    count: usize,
) {
    let pool: Vec<Pos> = candidates
        .iter()
        .copied()
        .filter(|position| !spawns.contains(position))
        .collect();
    for _ in 0..count {
        let Some(next) = pool
            .iter()
            .filter(|position| !spawns.contains(position))
            .max_by_key(|position| {
                let room = spawns
                    .iter()
                    .map(|start| wm.distance(**position, *start))
                    .min()
                    .unwrap_or(i32::MAX);
                (room, start_quality(rules, wm, **position), **position)
            })
            .copied()
        else {
            break;
        };
        spawns.push(next);
    }
}

/// How much of a capital's quality the territory pass may trade away to even
/// the land out. A tenth moves a start a hex or two onto slightly worse ground;
/// more than that and the pass is solving the wrong problem.
const TERRITORY_QUALITY_TOLERANCE: i32 = 10;

/// Even out the land each civilization can actually reach. Region division
/// balances the ground a seat is *given*; what a seat *holds* is decided by
/// where its neighbours ended up standing, and a start that terrain pushed to
/// one end of its region keeps only a fraction of it. This walks the poorest
/// seat back toward the middle until no single move lifts the worst share.
///
/// A move is refused if it would spend clearance the layout already has, and
/// refused if it would cost that capital more than
/// `TERRITORY_QUALITY_TOLERANCE` per cent of its site — there is no fairness
/// in giving a seat a fair share of bad ground.
fn balance_territory(
    rules: &Rules,
    wm: &WorldMap,
    land: &[Pos],
    regions: &[Vec<Pos>],
    candidates: &BTreeSet<Pos>,
    seated: &mut [(usize, Pos)],
) {
    if seated.len() < 2 || land.is_empty() {
        return;
    }
    let mut starts: Vec<Pos> = seated.iter().map(|(_, start)| *start).collect();
    let mut floor = i32::MAX;
    for (index, start) in starts.iter().enumerate() {
        for other in &starts[index + 1..] {
            floor = floor.min(wm.distance(*start, *other));
        }
    }
    let shares = |starts: &[Pos]| -> Vec<usize> {
        let mut held = vec![0_usize; starts.len()];
        for tile in land {
            let owner = starts
                .iter()
                .enumerate()
                .map(|(index, start)| (wm.distance(*tile, *start), index))
                .min()
                .map(|(_, index)| index)
                .unwrap_or(0);
            held[owner] += 1;
        }
        held
    };
    // The worst-off seat first, then the gap between best and worst: raising
    // the floor is the point, narrowing the spread is the tiebreak.
    let rank = |starts: &[Pos]| -> (usize, i64) {
        let held = shares(starts);
        let fewest = held.iter().copied().min().unwrap_or(0);
        let most = held.iter().copied().max().unwrap_or(0);
        (fewest, -((most - fewest) as i64))
    };
    let mut best = rank(&starts);
    for _ in 0..REGION_PASSES {
        let held = shares(&starts);
        let Some(poorest) = (0..starts.len()).min_by_key(|index| (held[*index], *index)) else {
            return;
        };
        let Some(region) = regions.get(seated[poorest].0) else {
            return;
        };
        let current = starts[poorest];
        let keep = start_quality(rules, wm, current) * (100 - TERRITORY_QUALITY_TOLERANCE) / 100;
        let others: Vec<Pos> = starts
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != poorest)
            .map(|(_, start)| *start)
            .collect();
        let mut trials: Vec<Pos> = region
            .iter()
            .copied()
            .filter(|position| {
                *position != current
                    && candidates.contains(position)
                    && wm.distance(*position, current) <= 4
                    && clear_of(wm, *position, &others, floor - 1)
                    && start_quality(rules, wm, *position) >= keep
            })
            .collect();
        trials.sort_by_key(|position| (wm.distance(*position, current), *position));
        trials.truncate(24);
        let mut improved = false;
        for trial in trials {
            let mut moved = starts.clone();
            moved[poorest] = trial;
            let score = rank(&moved);
            if score > best {
                best = score;
                starts = moved;
                improved = true;
            }
        }
        if !improved {
            break;
        }
    }
    for (slot, (_, position)) in seated.iter_mut().enumerate() {
        *position = starts[slot];
    }
}

/// Lift the least fortunate start once every seat has one, and keep lifting
/// until nothing can be lifted. Region division equalizes the *land* each
/// civilization is given; it cannot equalize what is growing on the tile each
/// one actually stands on, and a capital's first twenty turns are decided by
/// that tile and its ring.
///
/// A start may only move inside its own region, so the even division survives
/// the pass, and only to a site that keeps the clearance the layout already
/// has — measured, not assumed, so a crowded map cannot be made more crowded
/// in the name of a better capital.
fn equalize_start_quality(
    rules: &Rules,
    wm: &WorldMap,
    regions: &[Vec<Pos>],
    candidates: &BTreeSet<Pos>,
    fertility: &BTreeMap<Pos, i32>,
    seated: &mut [(usize, Pos)],
    foreign: &[Pos],
) {
    if seated.len() < 2 {
        return;
    }
    let mut qualities: Vec<i32> = seated
        .iter()
        .map(|(_, start)| start_quality(rules, wm, *start))
        .collect();
    // The clearance already achieved, which no swap is allowed to spend.
    let mut floor = i32::MAX;
    for (index, (_, start)) in seated.iter().enumerate() {
        for (_, other) in &seated[index + 1..] {
            floor = floor.min(wm.distance(*start, *other));
        }
        for other in foreign {
            floor = floor.min(wm.distance(*start, *other));
        }
    }
    let floor = floor.min(i32::MAX - 1);
    for _ in 0..seated.len() {
        let Some(weakest) = (0..seated.len()).min_by_key(|index| (qualities[*index], *index)) else {
            return;
        };
        let (region_index, _) = seated[weakest];
        let Some(region) = regions.get(region_index) else {
            return;
        };
        let Some(center) = region_center(wm, region, fertility) else {
            return;
        };
        let others: Vec<Pos> = seated
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != weakest)
            .map(|(_, (_, start))| *start)
            .collect();
        let current = qualities[weakest];
        // The same centrality pull the first pick used, so lifting the weakest
        // capital cannot quietly push it into a corner of its own region and
        // hand the territory the division just balanced to its neighbours.
        let Some(better) = region
            .iter()
            .copied()
            .filter(|position| candidates.contains(position))
            .filter(|position| {
                clear_of(wm, *position, &others, floor - 1)
                    && clear_of(wm, *position, foreign, floor - 1)
            })
            .map(|position| (start_quality(rules, wm, position), position))
            .filter(|(quality, _)| *quality > current)
            .max_by_key(|(quality, position)| {
                (
                    quality - REGION_CENTRALITY_PULL * wm.distance(*position, center),
                    *position,
                )
            })
        else {
            return;
        };
        seated[weakest].1 = better.1;
        qualities[weakest] = better.0;
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
        for neighbor in wm.neighbors(pos).into_iter()
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
    let b_neighbors: BTreeSet<Pos> = wm.neighbors(b).into_iter()
        .collect();
    let mut connected = BTreeSet::new();
    for common in wm.neighbors(a).into_iter()
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

    let is_water = |pos: Pos| {
        wm.tiles
            .get(&pos)
            .is_some_and(|tile| matches!(tile.terrain.as_str(), "ocean" | "coast"))
    };
    let distance_to_water = |pos: Pos| {
        water_tiles
            .iter()
            .map(|water| wm.distance(pos, *water))
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
fn assign_continents(wm: &mut WorldMap, land: &BTreeSet<Pos>, requested: usize, rng: &mut Rng) {
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
                    .map(|c| wm.distance(**p, *c))
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
            .min_by_key(|(id, center)| (wm.distance(*pos, **center), *id))
            .map(|(id, _)| id);
        wm.tiles.get_mut(pos).unwrap().continent = continent;
    }
}

fn connected_components(wm: &WorldMap, cells: &BTreeSet<Pos>) -> Vec<BTreeSet<Pos>> {
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
            for n in wm.neighbors(cur) {
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
fn largest_component(wm: &WorldMap, cells: &BTreeSet<Pos>) -> BTreeSet<Pos> {
    connected_components(wm, cells)
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
        connected_components(world, &land)
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
                for body in connected_components(&world, &lakes) {
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
            let bodies = connected_components(&world, &sea);
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
                    spawns[spawn_index + 1..].iter().all(|other| world.distance(*start, *other) >= 4),
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
                MapScript::Planet | MapScript::TrueStartEarth => {
                    unreachable!("the globe scripts are not cylinders")
                }
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

    /// The size table names a globe for every map size, and the generator has
    /// to reach the same one from either rectangle — the size's own, or the
    /// globe's — or the lobby and the world it builds would disagree.
    #[test]
    fn every_size_names_the_same_globe_from_either_rectangle() {
        for size in CIV6_MAP_SIZES {
            assert_eq!(
                globe_frequency(size.width, size.height),
                size.globe_frequency,
                "{} from its own rectangle",
                size.id
            );
            assert_eq!(
                globe_frequency(size.globe_width(), size.globe_height()),
                size.globe_frequency,
                "{} from its globe's rectangle",
                size.id
            );
        }
    }

    /// Planet is the one script whose world has no edge: every tile has a
    /// neighbour in every direction, so a fleet can leave any coast on any
    /// heading and come back to it.
    #[test]
    fn planet_closes_into_a_sphere_of_hexagons_and_twelve_pentagons() {
        let rules = Rules::embedded();
        let size = CIV6_MAP_SIZES
            .iter()
            .find(|size| size.id == "small")
            .unwrap();
        let mut rng = Rng::new(51_517);
        let (world, spawns) = generate_with_script(
            &rules,
            size.width,
            size.height,
            6,
            9,
            size.natural_wonders,
            size.continents,
            MapScript::Planet,
            &mut rng,
        );

        // The size's rectangle is re-expressed as that size's globe.
        let frequency = size.globe_frequency;
        assert_eq!((world.width, world.height), (5 * frequency, 2 * frequency + 2));
        assert_eq!(world.tiles.len(), (10 * frequency * frequency + 2) as usize);

        // No edge of the world, anywhere: every tile is surrounded, and the
        // twelve tiles that are short a neighbour are the pentagons.
        let mut pentagons = 0;
        for (pos, _) in world.tiles.iter() {
            let neighbors = world.neighbors(*pos);
            match neighbors.len() {
                5 => pentagons += 1,
                6 => {}
                other => panic!("{pos:?} has {other} neighbours"),
            }
            for neighbor in neighbors {
                assert!(world.tiles.contains_key(&neighbor));
                assert!(world.neighbors(neighbor).contains(pos));
            }
        }
        assert_eq!(pentagons, 12);

        // Following the H3 grid's trick of keeping its pentagons in the ocean,
        // no pentagon carries land, so every workable tile is a full hexagon.
        for pos in world.sphere().unwrap().pentagons() {
            assert!(rules.is_water(&world.tiles[&pos]), "{pos:?} is a land pentagon");
        }
        let land: BTreeSet<Pos> = world
            .tiles
            .iter()
            .filter(|(_, tile)| !rules.is_water(tile))
            .map(|(pos, _)| *pos)
            .collect();
        for pos in &land {
            assert_eq!(world.neighbors(*pos).len(), 6, "land tile {pos:?}");
        }

        // A world worth playing: about a third land, in several landmasses,
        // with open water over both poles.
        let share = land.len() * 100 / world.tiles.len();
        assert!((20..45).contains(&share), "{share}% land");
        let components = land_components(&world, &rules);
        assert!(
            components.iter().filter(|body| body.len() >= 20).count() >= 3,
            "Planet needs several landmasses, got {:?}",
            components.iter().map(|body| body.len()).collect::<Vec<_>>()
        );
        for row in [0, world.height - 1] {
            let pole = hex::offset_to_axial(0, row);
            assert!(world.polar_fraction(pole) > 0.9, "row {row} holds a pole");
            assert_eq!(world.neighbors(pole).len(), 5, "a pole is a pentagon");
            assert!(rules.is_water(&world.tiles[&pole]), "the caps stay open water");
        }

        // Sailing keeps going: step away from a start and around the globe,
        // always taking the neighbour furthest from where the walk began until
        // it turns back, and the walk returns to its own starting tile.
        let start = *land.iter().next().unwrap();
        let mut at = start;
        let mut previous = start;
        for _ in 0..(30 * frequency) {
            let next = world
                .neighbors(at)
                .into_iter()
                .filter(|pos| *pos != previous)
                .max_by_key(|pos| (world.distance(start, *pos), *pos))
                .unwrap();
            previous = at;
            at = next;
            if at == start {
                break;
            }
        }
        assert!(world.distance(start, at) <= 3 * frequency, "the walk never left the globe");

        assert_eq!(spawns.len(), 15);
        for start in &spawns {
            assert!(!rules.is_water(&world.tiles[start]));
        }
    }

    /// True Start Earth is the same globe as Planet, tiled the same way, but
    /// its coastlines are read off Earth instead of grown. What this pins is
    /// that the world a player recognises survives the sampling.
    #[test]
    fn true_start_earth_is_earth_on_the_hexagon_globe() {
        let rules = Rules::embedded();
        let size = CIV6_MAP_SIZES
            .iter()
            .find(|size| size.id == "standard")
            .unwrap();
        let mut rng = Rng::new(4_071);
        let (world, spawns) = generate_with_script(
            &rules,
            size.width,
            size.height,
            8,
            12,
            size.natural_wonders,
            size.continents,
            MapScript::TrueStartEarth,
            &mut rng,
        );
        let frequency = size.globe_frequency;
        assert_eq!((world.width, world.height), (5 * frequency, 2 * frequency + 2));
        assert_eq!(world.tiles.len(), (10 * frequency * frequency + 2) as usize);

        // Still a closed globe of hexagons and exactly twelve pentagons.
        let mut pentagons = 0;
        for (pos, _) in world.tiles.iter() {
            match world.neighbors(*pos).len() {
                5 => pentagons += 1,
                6 => {}
                other => panic!("{pos:?} has {other} neighbours"),
            }
        }
        assert_eq!(pentagons, 12);

        // Earth is where it should be. Each probe is a place whose nearest
        // tile must be land, or open sea whose nearest tile must not be.
        let sphere = world.sphere().unwrap();
        let nearest = |longitude: f64, latitude: f64| {
            let target = earth_direction(longitude, latitude);
            sphere
                .positions()
                .max_by(|a, b| {
                    let toward = |pos: &Pos| {
                        let center = sphere.center(*pos).unwrap();
                        center[0] * target[0] + center[1] * target[1] + center[2] * target[2]
                    };
                    toward(a).partial_cmp(&toward(b)).unwrap()
                })
                .unwrap()
        };
        // Continental interiors only: a tile on this globe spans some three
        // degrees, so Italy and the Nile delta are thinner than the sampling
        // and a probe on either could honestly land offshore. What must
        // survive the resolution is the body of each continent.
        for (name, longitude, latitude) in [
            ("central Europe", 20.0, 50.0),
            ("central Asia", 80.0, 50.0),
            ("Siberia", 100.0, 60.0),
            ("the Congo", 20.0, 0.0),
            ("the Sahara", 5.0, 22.0),
            ("the Deccan", 77.0, 20.0),
            ("Beijing", 116.4, 39.9),
            ("central Brazil", -55.0, -10.0),
            ("the Australian interior", 133.0, -25.0),
        ] {
            let pos = nearest(longitude, latitude);
            assert!(!rules.is_water(&world.tiles[&pos]), "{name} came out at sea");
        }
        for (name, longitude, latitude) in [
            ("the mid-Pacific", -150.0, 0.0),
            ("the mid-Atlantic", -30.0, 10.0),
            ("the south Pacific", -120.0, -30.0),
            ("the Indian Ocean", 75.0, -25.0),
            ("the Southern Ocean", 100.0, -60.0),
            ("the north pole", 0.0, 90.0),
        ] {
            let pos = nearest(longitude, latitude);
            assert!(rules.is_water(&world.tiles[&pos]), "{name} came out as land");
        }

        // Earth is about a third land, in several separate bodies — the Old
        // World, the Americas and Australia at the very least.
        let land: BTreeSet<Pos> = world
            .tiles
            .iter()
            .filter(|(_, tile)| !rules.is_water(tile))
            .map(|(pos, _)| *pos)
            .collect();
        let share = land.len() * 100 / world.tiles.len();
        assert!((20..40).contains(&share), "{share}% land");
        let components = land_components(&world, &rules);
        assert!(
            components.iter().filter(|body| body.len() >= 20).count() >= 3,
            "Earth needs several landmasses, got {:?}",
            components.iter().map(|body| body.len()).collect::<Vec<_>>()
        );

        // Every civilization opens in its own homeland. The seats are handed
        // out in CIV_NAMES order, so the eight majors lead the spawn list.
        for (index, (longitude, latitude)) in EARTH_HOMELANDS.iter().enumerate() {
            let home = nearest(*longitude, *latitude);
            let start = spawns[index];
            assert!(!rules.is_water(&world.tiles[&start]));
            assert!(
                sphere.distance(home, start) <= 4,
                "civilization {index} opened {} tiles from its homeland",
                sphere.distance(home, start)
            );
        }
    }

    /// Earth may not be spun to suit its lattice, so unlike Planet it cannot
    /// keep all twelve pentagons at sea. Exactly two fall on land, and this
    /// pins both the count and the reason no rotation about the pole fixes it.
    #[test]
    fn earth_keeps_the_two_pentagons_that_land_on_it() {
        let ring = (0.5f64).atan().to_degrees();
        let corners: Vec<(f64, f64)> = (0..5)
            .map(|k| (72.0 * k as f64, ring))
            .chain((0..5).map(|k| (72.0 * k as f64 + 36.0, -ring)))
            .collect();
        let wrap = |longitude: f64| {
            let mut longitude = longitude;
            while longitude > 180.0 {
                longitude -= 360.0;
            }
            while longitude <= -180.0 {
                longitude += 360.0;
            }
            longitude
        };

        // Both poles are at sea: the Arctic is ocean and this Earth carries no
        // Antarctica, so only the ten off-pole corners are ever in question.
        assert!(!earth_is_land(0.0, 90.0) && !earth_is_land(0.0, -90.0));
        let on_land: Vec<(f64, f64)> = corners
            .iter()
            .copied()
            .filter(|(longitude, latitude)| earth_is_land(wrap(*longitude), *latitude))
            .collect();
        assert_eq!(on_land.len(), 2, "expected two land pentagons, got {on_land:?}");
        assert_eq!(on_land[0].0, 0.0, "the Saharan corner");
        assert_eq!(on_land[1].0, 72.0, "the Indus corner");

        // And no spin of the globe does better, at any whole degree.
        for spin in 0..360 {
            let at_sea = corners
                .iter()
                .filter(|(longitude, latitude)| {
                    !earth_is_land(wrap(*longitude + spin as f64), *latitude)
                })
                .count();
            assert!(at_sea < 10, "a spin of {spin}° would seat every pentagon at sea");
        }
    }

    /// The seed moves what grows on Earth, never Earth itself. The two runs
    /// are not identical — lakes are still cut into the interiors and rivers
    /// still run where the roll puts them — but the outline they are cut into
    /// is the same one, so the disagreement stays inland and stays small.
    #[test]
    fn true_start_earth_draws_the_same_coastlines_every_game() {
        let rules = Rules::embedded();
        let land_of = |seed: u64| {
            let mut rng = Rng::new(seed);
            let (world, _) = generate_with_script(
                &rules, 60, 38, 4, 6, 3, 2, MapScript::TrueStartEarth, &mut rng,
            );
            let land: BTreeSet<Pos> = world
                .tiles
                .iter()
                .filter(|(_, tile)| !rules.is_water(tile))
                .map(|(pos, _)| *pos)
                .collect();
            (world, land)
        };
        let (world, first) = land_of(11);
        let (_, second) = land_of(9_999_991);
        assert!(!first.is_empty());

        // The sampled silhouette itself is a pure function of the globe.
        assert_eq!(earth_land(&world), earth_land(&world));
        assert!(first.is_subset(&earth_land(&world)));
        assert!(second.is_subset(&earth_land(&world)));

        let moved = first.symmetric_difference(&second).count();
        assert!(
            moved * 100 < world.tiles.len(),
            "{moved} of {} tiles changed between seeds — more than inland water",
            world.tiles.len()
        );
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
            MapScript::Planet,
            MapScript::TrueStartEarth,
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

    /// Uniform ground is the case with a right answer: six regions cut out of
    /// one flat rectangle of plains must come out the same size, and their six
    /// starts must sit the same distance apart. Anything the placer does that
    /// is arbitrary — an anchor tile, a seed order, a greedy chain — shows up
    /// here as a spread that has no business existing, because the map itself
    /// offers no reason to prefer one tile over another.
    #[test]
    fn one_flat_landmass_is_divided_into_equal_regions_with_evenly_spaced_starts() {
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
        let land: Vec<Pos> = landmass.iter().copied().collect();
        let fertility: BTreeMap<Pos, i32> = land
            .iter()
            .map(|position| (*position, tile_fertility(&rules, &wm.tiles[position])))
            .collect();
        let regions = divide_into_regions(&wm, &land, &fertility, 6);
        assert_eq!(regions.len(), 6);
        assert_eq!(
            regions.iter().map(|region| region.len()).sum::<usize>(),
            land.len(),
            "every tile belongs to exactly one region"
        );
        let sizes: Vec<usize> = regions.iter().map(|region| region.len()).collect();
        let smallest = sizes.iter().copied().min().unwrap();
        let largest = sizes.iter().copied().max().unwrap();
        assert!(
            smallest * 100 / largest >= 75,
            "regions of flat ground came out uneven: {sizes:?}"
        );

        let candidates: BTreeSet<Pos> = landmass.clone();
        let seated = regional_starts(
            &rules,
            &wm,
            &regions,
            &candidates,
            &fertility,
            &[],
            MAJOR_START_BUFFER,
            MAJOR_START_BUFFER,
        );
        assert_eq!(seated.len(), 6, "every region seated a civilization");
        let starts: Vec<Pos> = seated.iter().map(|(_, start)| *start).collect();
        let nearest: Vec<i32> = starts
            .iter()
            .map(|start| {
                starts
                    .iter()
                    .filter(|other| *other != start)
                    .map(|other| wm.distance(*start, *other))
                    .min()
                    .unwrap()
            })
            .collect();
        let closest = nearest.iter().copied().min().unwrap();
        let farthest = nearest.iter().copied().max().unwrap();
        assert!(
            closest >= 8 && farthest - closest <= 2,
            "starts on flat ground are not evenly spaced: {nearest:?}"
        );

        // And none of it comes from the random stream: the same map divides the
        // same way every time, whatever seed the game was rolled with.
        let repeat = divide_into_regions(&wm, &land, &fertility, 6);
        assert_eq!(regions, repeat, "region division must not depend on chance");
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
            let landmass = largest_component(&wm, &passable);
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
                        .all(|other| wm.distance(*start, *other) >= 4),
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
            // `__MajorCivBuffer` clearance, which is a floor and not a band:
            // a map with room to spare is allowed to use it.
            assert!(
                score.minimum_separation > MAJOR_START_BUFFER,
                "{} crowds a start inside the shipped buffer: {score:?}",
                size.name
            );
            // What replaces the old upper bound. Regular means the nearest
            // neighbour is about the same distance for everyone, so nobody
            // fights two neighbours for land while somebody else fights none.
            let nearest: Vec<i32> = majors
                .iter()
                .map(|start| {
                    majors
                        .iter()
                        .filter(|other| *other != start)
                        .map(|other| wm.distance(*start, *other))
                        .min()
                        .unwrap_or(START_DISTANCE_MAJOR)
                })
                .collect();
            let closest = nearest.iter().copied().min().unwrap();
            let farthest = nearest.iter().copied().max().unwrap();
            assert!(
                closest * 100 / farthest.max(1) >= 60,
                "{} spaces its starts irregularly: nearest-neighbour {nearest:?}",
                size.name
            );
            assert!(
                balance.0 >= 60 && balance.1 >= 60 && balance.2 >= 60,
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
                    .map(|wonder| wm.distance(*start, *wonder))
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

    /// `__MinorMajorCivBuffer` rejects a city-state site within
    /// `START_DISTANCE_MINOR_MAJOR_CIVILIZATION` 6 of a major and
    /// `__MinorMinorCivBuffer` within `START_DISTANCE_MINOR_CIVILIZATION_START`
    /// 5 of another city-state. Both are clearances; neither has an upper
    /// bound, and `START_DISTANCE_RANGE_MINOR` is used by no shipped script.
    #[test]
    fn city_states_keep_the_shipped_clearance_from_civilizations_and_each_other() {
        let rules = Rules::embedded();
        for seed in 0..6u64 {
            let mut rng = Rng::new(52_000 + seed);
            let (wm, spawns) = generate(&rules, 84, 54, 8, 12, 4, 2, &mut rng);
            assert_eq!(spawns.len(), 20, "seed {seed}");
            let (majors, minors) = spawns.split_at(8);
            for (index, minor) in minors.iter().enumerate() {
                let to_major = majors
                    .iter()
                    .map(|major| wm.distance(*minor, *major))
                    .min()
                    .unwrap();
                assert!(
                    to_major > MINOR_MAJOR_BUFFER,
                    "seed {seed}: city-state {to_major} from the nearest major, inside {MINOR_MAJOR_BUFFER}"
                );
                if let Some(to_minor) = minors
                    .iter()
                    .enumerate()
                    .filter(|(other, _)| *other != index)
                    .map(|(_, other)| wm.distance(*minor, *other))
                    .min()
                {
                    assert!(
                        to_minor > MINOR_MINOR_BUFFER,
                        "seed {seed}: city-states {to_minor} apart, inside {MINOR_MINOR_BUFFER}"
                    );
                }
            }
        }
    }

    /// A city-state is only worth contesting if it is somewhere near you.
    /// Filling the largest-scoring gaps one at a time chained city-states
    /// around whichever civilization happened to have the best ground, and
    /// measured over 72 worlds *every* stock profile left at least one
    /// civilization with none within ten hexes while another had up to
    /// eighteen. Dividing the map a second time, one region per city-state,
    /// is what fixes it.
    #[test]
    fn every_civilization_has_city_states_within_reach() {
        let rules = Rules::embedded();
        for (index, size) in CIV6_MAP_SIZES.iter().enumerate() {
            for seed in 0..3u64 {
                let mut rng = Rng::new(77_000 + seed * 13 + index as u64);
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
                let (majors, minors) = spawns.split_at(size.default_players);
                if minors.len() < majors.len() {
                    continue;
                }
                // How far the least lucky civilization has to look. Twelve hexes
                // is inside the range an early envoy mission can cover.
                for major in majors {
                    let nearest = minors
                        .iter()
                        .map(|minor| wm.distance(*major, *minor))
                        .min()
                        .unwrap();
                    assert!(
                        nearest <= 12,
                        "{} seed {seed}: a civilization's nearest city-state is {nearest} hexes away",
                        size.id
                    );
                }
                // And how the envoy race is actually shared out: a city-state
                // belongs to whichever civilization is nearest it.
                let mut owned = vec![0_usize; majors.len()];
                for minor in minors {
                    let owner = majors
                        .iter()
                        .enumerate()
                        .map(|(index, major)| (wm.distance(*minor, *major), index))
                        .min()
                        .map(|(_, index)| index)
                        .unwrap();
                    owned[owner] += 1;
                }
                let fewest = owned.iter().copied().min().unwrap();
                let most = owned.iter().copied().max().unwrap();
                assert!(
                    most - fewest <= 1,
                    "{} seed {seed}: city-states are shared out unevenly: {owned:?}",
                    size.id
                );
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
            let landmass = largest_component(&wm, &passable);
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
                        world.neighbors(*position).into_iter()
                            .any(|neighbor| world.tiles.get(&neighbor).is_some_and(
                                |neighbor_tile| {
                                    neighbor_tile.feature.as_deref() == Some("volcano")
                                }
                            )),
                        "volcanic soil at {position:?} has no volcano"
                    ),
                    "geothermal_fissure" => assert!(
                        world.neighbors(*position).into_iter()
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
                    world.neighbors(position).into_iter()
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
                                        .map(|right| world.distance(*left, *right))
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

    /// Scratch measurement of how evenly a layout is spread. Prints rather than
    /// asserts; run with `--ignored --nocapture`.
    #[test]
    #[ignore]
    fn measure_start_spread() {
        let rules = Rules::embedded();
        for script in [
            MapScript::Pangaea,
            MapScript::Continents,
            MapScript::SmallContinents,
            MapScript::Planet,
        ] {
            for size_index in [1usize, 3, 5] {
                let size = &CIV6_MAP_SIZES[size_index];
                let mut rows: Vec<String> = Vec::new();
                for seed in 0..6u64 {
                    let mut rng = Rng::new(90_000 + seed * 31 + size_index as u64);
                    let (wm, spawns) = generate_with_script(
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
                    let passable: BTreeSet<Pos> = wm
                        .tiles
                        .iter()
                        .filter(|(_, tile)| !rules.is_water(tile) && rules.is_passable(tile))
                        .map(|(pos, _)| *pos)
                        .collect();
                    let majors = &spawns[..size.default_players.min(spawns.len())];
                    let minors = &spawns[size.default_players.min(spawns.len())..];

                    let nn = |group: &[Pos]| -> Vec<i32> {
                        group
                            .iter()
                            .map(|start| {
                                group
                                    .iter()
                                    .filter(|other| *other != start)
                                    .map(|other| wm.distance(*start, *other))
                                    .min()
                                    .unwrap_or(0)
                            })
                            .collect()
                    };
                    let spread = |values: &[i32]| -> (i32, i32, f64) {
                        let min = values.iter().copied().min().unwrap_or(0);
                        let max = values.iter().copied().max().unwrap_or(0);
                        let mean =
                            values.iter().copied().sum::<i32>() as f64 / values.len().max(1) as f64;
                        (min, max, mean)
                    };
                    let major_nn = nn(majors);
                    let minor_nn = nn(minors);
                    let (major_min, major_max, major_mean) = spread(&major_nn);
                    let (minor_min, minor_max, minor_mean) = spread(&minor_nn);

                    let mut territory = vec![0i32; majors.len()];
                    let mut coverage = 0;
                    for tile in &passable {
                        let (distance, owner) = majors
                            .iter()
                            .enumerate()
                            .map(|(index, start)| (wm.distance(*tile, *start), index))
                            .min()
                            .unwrap();
                        coverage = coverage.max(distance);
                        territory[owner] += 1;
                    }
                    let (territory_min, territory_max, _) = spread(&territory);

                    // How many city-states each civilization has within reach,
                    // and how far the nearest one is: an envoy race is not fair
                    // if one capital has four neighbours and another has none.
                    let neighbours: Vec<i32> = majors
                        .iter()
                        .map(|major| {
                            minors
                                .iter()
                                .filter(|minor| wm.distance(*major, **minor) <= 10)
                                .count() as i32
                        })
                        .collect();
                    let (neighbours_min, neighbours_max, _) = spread(&neighbours);
                    // City-states counted by which civilization is nearest,
                    // which is what an envoy race actually turns on.
                    let mut owned = vec![0i32; majors.len()];
                    let mut nearest_minor = vec![i32::MAX; majors.len()];
                    for minor in minors {
                        if let Some((_, owner)) = majors
                            .iter()
                            .enumerate()
                            .map(|(index, major)| (wm.distance(*minor, *major), index))
                            .min()
                        {
                            owned[owner] += 1;
                        }
                        for (index, major) in majors.iter().enumerate() {
                            nearest_minor[index] =
                                nearest_minor[index].min(wm.distance(*minor, *major));
                        }
                    }
                    let (owned_min, owned_max, _) = spread(&owned);
                    let lonely = nearest_minor.iter().copied().max().unwrap_or(0);

                    let qualities: Vec<i32> = majors
                        .iter()
                        .map(|start| start_quality(&rules, &wm, *start))
                        .collect();
                    let (quality_min, quality_max, _) = spread(&qualities);

                    let mut closest_pair = i32::MAX;
                    for (index, start) in spawns.iter().enumerate() {
                        for other in &spawns[index + 1..] {
                            closest_pair = closest_pair.min(wm.distance(*start, *other));
                        }
                    }

                    // How many landmasses hold a start, out of how many could.
                    let components = connected_components(&wm, &passable);
                    let usable = components
                        .iter()
                        .filter(|component| component.len() >= 12)
                        .count();
                    let occupied = components
                        .iter()
                        .filter(|component| {
                            component.len() >= 12
                                && spawns.iter().any(|spawn| component.contains(spawn))
                        })
                        .count();

                    let crowded = {
                        let mut count = 0;
                        for (index, start) in spawns.iter().enumerate() {
                            for other in &spawns[index + 1..] {
                                if wm.distance(*start, *other) < MIN_START_SEPARATION {
                                    count += 1;
                                }
                            }
                        }
                        count
                    };
                    rows.push(format!(
                        "  seed {seed}: major nn {major_min}-{major_max} (mean {major_mean:.1}) \
                         terr {territory_min}-{territory_max} qual {quality_min}-{quality_max} \
                         | minor nn {minor_min}-{minor_max} (mean {minor_mean:.1}) \
                         nbrs {neighbours_min}-{neighbours_max} \
                         own {owned_min}-{owned_max} lonely {lonely} \
                         | closest {closest_pair} crowded {crowded} cover {coverage} \
                         land {occupied}/{usable}"
                    ));
                }
                println!(
                    "{script:?} {} ({}x{}, {} civs, {} city-states)",
                    size.id,
                    size.width,
                    size.height,
                    size.default_players,
                    size.default_city_states
                );
                for row in rows {
                    println!("{row}");
                }
            }
        }
    }
}
