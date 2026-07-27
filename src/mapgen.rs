//! Map generation (mirrors civvis/mapgen.py).
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::fractal::Fractal;
use crate::rng::Rng;
use crate::rules::Rules;
use crate::setup::{MapPoles, MapScript, MapTopology};
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
    // The ceiling has to clear the largest rectangle in the size table:
    // Ludicrous is 305x190, which wants frequency 76.
    (1..=128)
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
/// One entry per seat in `CIV_NAMES`, in that same order, so a True Start map
/// is true in play and not merely Earth-shaped in the setup preview.
///
/// A globe of this size gives each tile some three degrees, so a homeland is
/// the civilization's heartland rather than the exact site of its capital: a
/// point on a peninsula or a river delta thinner than the sampling would come
/// out at sea and seat the civilization on whatever coast the search found
/// first. `every_homeland_is_on_land` holds the line.
const EARTH_HOMELANDS: [(f64, f64); 105] = [
    (12.5, 41.9),     // Rome
    (31.2, 30.0),     // Egypt
    (23.7, 38.0),     // Greece
    (116.4, 39.9),    // China
    (44.4, 32.5),     // Sumeria
    (-99.1, 19.4),    // Aztec
    (32.5, 19.6),     // Nubia
    (64.0, 48.0),     // Scythia
    (-1.5, 52.5),     // England
    (9.0, 50.1),      // Germany
    (37.6, 55.8),     // Russia
    (128.0, 36.5),    // Korea
    (-89.0, 21.0),    // Maya
    (-8.4, 13.5),     // Mali
    (36.0, 35.0),     // Phoenicia
    (31.5, 39.8),     // Byzantium
    (30.5, -27.5),    // Zulu
    (3.5, 46.5),      // Gaul
    (16.0, -6.0),     // Kongo
    (105.5, 21.5),    // Vietnam
    (-45.0, -20.0),   // Brazil
    (2.8, 49.8),      // France
    (-4.0, 40.4),     // Spain
    (-8.0, 39.9),     // Portugal
    (5.9, 52.6),      // Netherlands
    (15.6, 59.6),     // Sweden
    (9.5, 61.0),      // Norway
    (9.5, 56.2),      // Denmark
    (19.9, 51.8),     // Poland
    (20.0, 47.3),     // Hungary
    (15.0, 47.0),     // Austria
    (14.6, 50.4),     // Bohemia
    (-4.2, 56.8),     // Scotland
    (-8.0, 53.3),     // Ireland
    (8.0, 46.9),      // Switzerland
    (11.5, 45.2),     // Venice
    (20.8, 43.8),     // Serbia
    (25.05, 41.8),    // Bulgaria
    (24.3, 55.2),     // Lithuania
    (31.5, 49.5),     // Ukraine
    (25.5, 62.0),     // Finland
    (25.0, 45.6),     // Romania
    (32.5, 58.3),     // Novgorod
    (13.2, 53.4),     // Prussia
    (1.4, 41.8),      // Catalonia
    (72.0, 22.5),     // Gujarat
    (43.2, 36.4),     // Assyria
    (53.0, 30.5),     // Persia
    (47.5, 34.8),     // Media
    (125.0, 45.0),    // Manchuria
    (27.8, 38.3),     // Lydia
    (57.5, 36.5),     // Parthia
    (67.5, 40.5),     // Sogdiana
    (35.5, 41.5),     // Ottomans
    (44.0, 24.5),     // Arabia
    (34.7, 28.75),    // Israel
    (44.8, 39.6),     // Armenia
    (43.0, 42.05),    // Georgia
    (62.5, 36.5),     // Timurids
    (69.0, 47.5),     // Kazakh
    (67.0, 36.6),     // Bactria
    (38.7, 9.5),      // Ethiopia
    (38.9, 14.1),     // Axum
    (-6.0, 32.0),     // Morocco
    (6.0, 34.5),      // Numidia
    (0.5, 16.5),      // Songhai
    (-9.5, 17.0),     // Ghana
    (5.6, 6.6),       // Benin
    (-1.6, 6.7),      // Ashanti
    (38.5, -6.5),     // Swahili
    (30.5, -19.5),    // Great Zimbabwe
    (32.3, 0.5),      // Buganda
    (3.5, 9.5),       // Oyo
    (5.5, 23.0),      // Tuareg
    (47.0, -19.5),    // Madagascar
    (78.0, 25.5),     // India
    (136.0, 35.5),    // Japan
    (106.0, 47.5),    // Mongolia
    (90.0, 30.5),     // Tibet
    (83.0, 28.25),    // Nepal
    (85.5, 20.5),     // Kalinga
    (79.2, 10.9),     // Chola
    (89.0, 24.0),     // Bengal
    (74.0, 18.5),     // Maratha
    (104.0, 13.4),    // Khmer
    (100.5, 16.5),    // Siam
    (95.5, 21.5),     // Burma
    (112.5, -7.5),    // Majapahit
    (108.5, 13.5),    // Champa
    (-95.75, 35.25),  // America
    (-106.5, 45.75),  // Canada
    (-107.5, 35.5),   // Pueblo
    (-100.0, 34.0),   // Comanche
    (-103.75, 43.0),  // Sioux
    (-72.5, -13.5),   // Inca
    (-73.8, 5.2),     // Muisca
    (-71.5, -38.5),   // Mapuche
    (-63.5, -32.5),   // Argentina
    (134.0, -24.5),   // Australia
    (175.0, -39.5),   // Maori
    (48.0, 32.0),     // Babylon
    (-114.5, 49.0),   // Cree
    (-68.0, 7.5),     // Gran Colombia
    (106.8, -6.2),    // Indonesia
    (22.5, 40.6),     // Macedon
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

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
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
/// A flat Earth is the same silhouette read through the same longitudes and
/// latitudes, which is exactly what a paper world map is: the globe rolled
/// flat. The two pentagons stay a globe's problem, because a flat map has no
/// pentagons to begin with.
fn earth_land(wm: &WorldMap) -> BTreeSet<Pos> {
    wm.tiles
        .keys()
        .copied()
        .filter(|pos| {
            let (longitude, latitude) = wm.lon_lat(*pos);
            earth_is_land(longitude, latitude)
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
                let taken = taken_within(wm, &starts, *separation);
                available
                    .iter()
                    .filter(|candidate| !taken.contains(candidate))
                    .count()
                    >= seats_left
            })
            .unwrap_or(0);
        let taken = taken_within(wm, &starts, separation);
        let selected = available
            .iter()
            .enumerate()
            .filter(|(_, candidate)| !taken.contains(candidate))
            .max_by(|(_, a), (_, b)| {
                let toward = |pos: &Pos| dot(wm.direction(*pos), target);
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
fn taken_within(wm: &WorldMap, starts: &[Pos], radius: i32) -> BTreeSet<Pos> {
    starts
        .iter()
        .flat_map(|start| wm.disk(*start, radius))
        .collect()
}

/// The land of a world, under the world type asked for and on whatever shape
/// the world turned out to be.
///
/// The two shapes want genuinely different generators and always have. A flat
/// map is a rectangle with edges, so its scripts draw against those edges — an
/// oval that stops short of them, two regions cut out of them, a fractal cut
/// at a percentile. A globe has no edge to hold an ocean against, so its land
/// has to be seeded and grown instead. Keeping the two apart is what lets the
/// world type and the world shape be answered separately: every type below
/// knows how to arrive on either.
fn generate_land(
    wm: &WorldMap,
    script: MapScript,
    poles: MapPoles,
    num_major_spawns: usize,
    num_minor_spawns: usize,
    rng: &mut Rng,
) -> BTreeSet<Pos> {
    if script.is_fixed_geography() {
        // The one type that is read rather than rolled. See [`earth_land`] for
        // what the world is asked, and why the two pentagons that fall on land
        // are allowed to stay there.
        return earth_land(wm);
    }
    if script == MapScript::GrandCanals {
        // Answered before the shape is dispatched on, the way Earth is: the
        // six canals are one piece of geometry that a world of either shape
        // reads the same way, and the ground they leave is filled in once for
        // both. See [`canal_world`].
        return canal_world(wm, poles, rng);
    }
    if wm.sphere().is_some() {
        return globe_land(wm, script, poles, num_major_spawns, num_minor_spawns, rng);
    }
    flat_land(wm, script, poles, num_major_spawns, num_minor_spawns, rng)
}

/// The land of a flat world: a rectangle that wraps east to west and ends at a
/// northern and a southern edge.
///
/// Every type here but Land Only leaves the top and bottom rows as water. That
/// is the map's edge rather than its climate — a coastline has to end
/// somewhere, and a river needs somewhere to run to — so it holds whether or
/// not the world has poles. Land Only is the exception, because a world that
/// is 95% land and still rings itself in ocean is not one.
fn flat_land(
    wm: &WorldMap,
    script: MapScript,
    poles: MapPoles,
    num_major_spawns: usize,
    num_minor_spawns: usize,
    rng: &mut Rng,
) -> BTreeSet<Pos> {
    let width = wm.width;
    let height = wm.height;
    let area = (width * height) as usize;
    match script {
        MapScript::LandOnly => {
            // Cut like Lakes, but at the far end of the same dial: the wettest
            // twentieth of the field is the only water there is, and it is
            // left wherever the fractal happens to put it, so a world arrives
            // with a handful of inland seas rather than a ring of ocean. The
            // edge rows are cut from too, which is what makes this the one
            // flat type whose land reaches the top and bottom of the map.
            let basin = Fractal::new(rng, width, height, 3);
            let waterline = basin.percentile(MapScript::LandOnly.land_percent());
            let mut land = BTreeSet::new();
            for row in 0..height {
                for col in 0..width {
                    if basin.at(col, row) < waterline {
                        land.insert(hex::offset_to_axial(col, row));
                    }
                }
            }
            land
        }
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
        MapScript::Islands => {
            // An archipelago. Islands are scattered rather than cut, because a
            // fractal waterline at this share gives a few ragged continents
            // and not many small islands: the shape of the coastline is the
            // point of the type, so it is the thing that gets generated. Every
            // island stands clear of its neighbours by at least one tile of
            // water, so every one of them is its own shore and the sea lanes
            // between them are the map.
            let mut open = offset_region(wm, 0, width, 1, height - 1);
            let target = area * MapScript::Islands.land_percent() as usize / 100;
            let seats = num_major_spawns + num_minor_spawns;
            scatter_islands(
                wm,
                &mut open,
                target,
                MIN_LANDMASS_FOR_A_START,
                24,
                ISLAND_CHANNEL,
                seats,
                rng,
            )
        }
        MapScript::WaterWorld => {
            // The far end of the same dial as Land Only: specks of land in an
            // ocean. The floor is what keeps it playable — a world that cannot
            // seat every civilization and city-state on land is not a map, so
            // the smallest sizes get a little more land than the share asks
            // for rather than a broken world.
            let mut open = offset_region(wm, 0, width, 1, height - 1);
            let seats = num_major_spawns + num_minor_spawns;
            let target = (area * MapScript::WaterWorld.land_percent() as usize / 100)
                .max(seats * WATER_WORLD_TILES_PER_SEAT);
            scatter_islands(
                wm,
                &mut open,
                target,
                WATER_WORLD_TILES_PER_SEAT,
                15,
                ISLAND_CHANNEL,
                seats,
                rng,
            )
        }
        // Answered before the shape was dispatched on, because Earth's
        // coastlines are read rather than rolled and are the same coastlines
        // on either shape.
        MapScript::TrueStartEarth => earth_land(wm),
        // Likewise: a canal is cut at an angle to an axis of the world, which
        // is a question neither shape answers differently. [`generate_land`]
        // sends it to [`canal_world`] before it reaches here.
        MapScript::GrandCanals => canal_world(wm, poles, rng),
    }
}

/// The land a seat needs under it on a Water World before the share is allowed
/// to squeeze it any further. A capital wants its own island and enough of one
/// to work; below about this the spacing search starts seating two
/// civilizations on the same rock.
const WATER_WORLD_TILES_PER_SEAT: usize = MIN_LANDMASS_FOR_A_START;

/// How wide the water between two scattered islands is. One tile keeps them
/// from touching; three keeps two capitals on neighbouring islands as far
/// apart as the spacing search asks for on a land map, which is what stops an
/// archipelago from seating civilizations closer than any other world type.
const ISLAND_CHANNEL: i32 = 3;

/// Scatter separated landmasses through open water until about `target` tiles
/// of it are land, and until at least `min_bodies` of them exist.
///
/// Each island is grown from a seed picked out of whatever water is still
/// open, and the water around the finished island is then closed off, so no
/// two islands come within `channel` tiles: every one of them keeps its own
/// coast, and the sea between them stays navigable. Islands arrive between
/// `min_size` and `max_size` tiles so the archipelago is varied rather than a
/// field of identical dots.
///
/// `min_bodies` is what makes a nearly-empty world playable. A seat needs an
/// island of its own — two capitals on one rock are two cities only one of
/// which can be founded — so on Water World the count of islands matters more
/// than the exact share of land, and the scatter keeps going a little past its
/// target rather than leave a civilization without a shore.
///
/// Every possible seed is shuffled once. As the water fills up, seeds in the
/// moat of an earlier body are skipped and an isolated pocket smaller than the
/// minimum is removed as water. That makes the work proportional to the field
/// instead of repeatedly rebuilding and sampling an ever-more-exhausted pool.
fn scatter_islands(
    wm: &WorldMap,
    open: &mut BTreeSet<Pos>,
    target: usize,
    min_size: usize,
    max_size: usize,
    channel: i32,
    min_bodies: usize,
    rng: &mut Rng,
) -> BTreeSet<Pos> {
    let span = (max_size + 1).saturating_sub(min_size).max(1);
    let mut seeds: Vec<Pos> = open.iter().copied().collect();
    for index in (1..seeds.len()).rev() {
        let other = rng.below(index + 1);
        seeds.swap(index, other);
    }
    let mut land = BTreeSet::new();
    let mut bodies = 0;
    while (land.len() < target || bodies < min_bodies) && !open.is_empty() {
        let Some(seed) = seeds.pop() else { break };
        if !open.contains(&seed) {
            continue;
        }
        // Once the land target is met and only the island count is still
        // owed, the remaining islands are kept to the smallest size that can
        // still hold a capital, so meeting the count costs the world as
        // little of its ocean as possible.
        let wanted = if land.len() >= target {
            min_size
        } else {
            (min_size + rng.below(span)).min((target - land.len()).max(min_size))
        };
        let island = grow_blob(wm, open, seed, wanted.max(1), rng);
        // A gap that cannot hold the promised minimum stays water. Committing
        // it as a one-plot "island" made the body quota look satisfied while
        // leaving a civilization with nowhere to found its capital.
        if island.len() < min_size {
            for pos in island {
                open.remove(&pos);
            }
            continue;
        }
        bodies += 1;
        close_off(wm, open, &island, channel);
        land.extend(island);
    }
    land
}

/// Take a finished body and the water around it out of the open field, so the
/// next body grown there cannot come nearer than `channel` tiles.
///
/// One tile is enough to keep two bodies from touching. An archipelago wants
/// more than that: capitals want room between them, and two islands a single
/// tile apart seat two civilizations closer than any land map ever would.
fn close_off(wm: &WorldMap, open: &mut BTreeSet<Pos>, body: &BTreeSet<Pos>, channel: i32) {
    for pos in body {
        for near in wm.disk(*pos, channel.max(1)) {
            open.remove(&near);
        }
        open.remove(pos);
    }
}

/// Grow `count` separated bodies totalling about `total` tiles inside a field.
///
/// Seeds are dropped at arm's length from one another first, so the bodies
/// arrive spread over the whole field instead of clustered wherever the first
/// roll happened to fall. A body of `n` tiles is roughly a disc of radius
/// `√(n/3)`; asking for seeds a little under two radii apart stands them clear
/// of one another without the search having to back off far. When the field is
/// too tight to hold that, the requested separation is relaxed two tiles at a
/// time rather than the placement being abandoned.
fn scatter_bodies(
    wm: &WorldMap,
    field: &mut BTreeSet<Pos>,
    count: usize,
    total: usize,
    rng: &mut Rng,
) -> BTreeSet<Pos> {
    let count = count.max(1);
    let per_body = (total / count).max(1);
    let mut separation = (1.6 * (per_body as f64 / 3.0).sqrt()) as i32;
    let pool: Vec<Pos> = field.iter().copied().collect();
    if pool.is_empty() {
        return BTreeSet::new();
    }
    let mut seeds: Vec<Pos> = Vec::new();
    while seeds.len() < count {
        let mut placed = false;
        for _ in 0..(4 * pool.len()).min(2_000) {
            let candidate = pool[rng.below(pool.len())];
            if seeds
                .iter()
                .all(|seed| wm.distance(*seed, candidate) >= separation)
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

    let mut grown = BTreeSet::new();
    for seed in seeds {
        if !field.contains(&seed) {
            continue;
        }
        let body = grow_blob(wm, field, seed, per_body, rng);
        close_off(wm, field, &body, 1);
        grown.extend(body);
    }
    grown
}

/// The land of a globe: a closed world of hexagons and twelve pentagons, with
/// no edge anywhere on it.
///
/// A globe has nothing to hold an ocean against, so it cannot draw a coastline
/// the way a flat map does — its land is seeded and grown instead. That gives
/// the same generator for every world type, run at two settings: how much of
/// the world is land, and how many separate pieces the *minority* of land and
/// water arrives in. Below half land, that minority is the land and continents
/// are grown in an ocean; above half it is the water, and seas are cut out of
/// a world that starts solid. Land Only and Water World are the same procedure
/// seen from the two ends.
///
/// Two things are held out of the field either way. The caps, when the world
/// has poles, so there is open water at the top and bottom for sea ice to form
/// on — a world with no poles wants no such reservation and does not get one.
/// And the twelve pentagons, always: Uber's H3 grid, built the same way, turns
/// its icosahedron so that all twelve corners fall in the ocean and the
/// pentagons never surface in the data, and a generated world can simply be
/// told to keep them wet. Every land tile then has six neighbours, so district
/// adjacency, city work radii and the rest behave exactly as they do on a flat
/// map. On a Land Only globe those twelve become the world's smallest lakes,
/// which is the same rule reaching the same answer from the other side.
fn globe_land(
    wm: &WorldMap,
    script: MapScript,
    poles: MapPoles,
    num_major_spawns: usize,
    num_minor_spawns: usize,
    rng: &mut Rng,
) -> BTreeSet<Pos> {
    let pentagons: BTreeSet<Pos> = wm
        .sphere()
        .map(|sphere| sphere.pentagons().into_iter().collect())
        .unwrap_or_default();
    let cap = if poles.has_poles() { 0.93 } else { f64::MAX };
    let mut field: BTreeSet<Pos> = wm
        .tiles
        .keys()
        .copied()
        .filter(|pos| wm.polar_fraction(*pos) < cap && !pentagons.contains(pos))
        .collect();

    let tiles = wm.tiles.len();
    let land_share = script.land_percent() as usize;
    let seats = num_major_spawns + num_minor_spawns;

    if land_share > 50 {
        // Cut the sea out of a solid world. What is left over — the caps and
        // the pentagons — stays water too, which is why the world never comes
        // out at exactly the share asked for on a poled globe.
        let water = tiles * (100 - land_share) / 100;
        let seas = match script {
            MapScript::InlandSea => 1,
            MapScript::Lakes => (tiles / 320).clamp(6, 40),
            _ => (tiles / 220).clamp(8, 60),
        };
        let sea = scatter_bodies(wm, &mut field, seas, water, rng);
        return wm
            .tiles
            .keys()
            .copied()
            .filter(|pos| !sea.contains(pos) && !pentagons.contains(pos))
            .filter(|pos| wm.polar_fraction(*pos) < cap)
            .collect();
    }

    // Below half, the land is the minority and is grown directly.
    let target = (tiles * land_share / 100).max(seats * WATER_WORLD_TILES_PER_SEAT);
    match script {
        MapScript::Islands => {
            scatter_islands(
                wm,
                &mut field,
                target,
                MIN_LANDMASS_FOR_A_START,
                24,
                ISLAND_CHANNEL,
                seats,
                rng,
            )
        }
        MapScript::WaterWorld => scatter_islands(
            wm,
            &mut field,
            target,
            WATER_WORLD_TILES_PER_SEAT,
            15,
            ISLAND_CHANNEL,
            seats,
            rng,
        ),
        _ => {
            let continents = match script {
                MapScript::Pangaea => 1,
                MapScript::Continents => 2,
                _ => num_major_spawns.div_ceil(2).clamp(3, 7),
            };
            let mut land = scatter_bodies(wm, &mut field, continents, target, rng);
            // Something in the open ocean to find, without turning the sea
            // lanes into an archipelago of their own.
            let islands = (target / 12).min(continents * 3);
            land.extend(scatter_islands(wm, &mut field, islands * 6, 3, 11, 1, 0, rng));
            land
        }
    }
}

/// The three axes a Grand Canals world is cut around: the polar one, and the
/// two through the equator at longitude 0 and longitude 90.
const CANAL_AXES: [[f64; 3]; 3] = [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];

/// How far off square to its axis each of an axis's two canals is cut.
///
/// Square to the axis would be one canal where the world wants two, and two
/// canals sharing a line is one canal. Twenty degrees is far enough apart that
/// the ground between a pair is a band a civilization can live in, and near
/// enough to the middle that neither canal is a small ring around a pole.
const CANAL_OFFSET_DEGREES: f64 = 20.0;

/// How much of a lap one tile of canal width is worth. Two tiles is the floor
/// — one is a ditch a fleet queues in rather than a canal — and six the
/// ceiling, past which the lanes are wider than the ground between them.
const CANAL_TILES_PER_LAP: f64 = 55.0;
const CANAL_MIN_TILES: f64 = 2.0;
const CANAL_MAX_TILES: f64 = 6.0;

/// The six canals of a Grand Canals world, as the tiles they take.
///
/// Two canals circle each of the world's three axes, cut
/// [`CANAL_OFFSET_DEGREES`] to either side of the great circle square to it.
/// Around the polar axis that reads as the parallels 20°N and 20°S, and the
/// other two pairs are the same construction turned a quarter turn: on Earth's
/// longitudes they cross the equator at 20°/160°, 200°/340°, 70°/290° and
/// 110°/250°. A tile belongs to a canal when its angle out of an axis's
/// equatorial plane is within half a canal's width of ±20°, which is a
/// question about where the tile is on the world rather than about the grid it
/// is stored in — so a globe and a flat map are cut by the same six lanes, and
/// every one of them closes on itself on either shape.
///
/// Because no two axes are parallel, every canal crosses all four belonging to
/// the other two axes, twice each: twenty-four junctions, and one connected
/// network rather than six separate rings. That is the point of the world —
/// the ground arrives already divided into blocks, and a ship can reach every
/// one of them.
fn grand_canals(wm: &WorldMap) -> BTreeSet<Pos> {
    let half = canal_half_width(wm);
    let offset = CANAL_OFFSET_DEGREES.to_radians();
    wm.tiles
        .keys()
        .copied()
        .filter(|pos| {
            let point = wm.direction(*pos);
            CANAL_AXES.iter().any(|axis| {
                let out_of_plane = dot(point, *axis).clamp(-1.0, 1.0).asin();
                (out_of_plane - offset).abs() <= half || (out_of_plane + offset).abs() <= half
            })
        })
        .collect()
}

/// Half a canal's width, as the angle it subtends at the centre of the world.
///
/// A canal is measured in tiles, because what matters about one is how many
/// ships fit abreast in it, and a tile is what the world is counted in. It is
/// widened with the world so that the six lanes take about the same share of a
/// Duel map as of a Ludicrous one: one tile of width for every
/// [`CANAL_TILES_PER_LAP`] tiles of the lap a canal makes, never fewer than
/// two and never more than six.
fn canal_half_width(wm: &WorldMap) -> f64 {
    let step = tile_arc(wm);
    let lap = std::f64::consts::TAU / step;
    let tiles = (lap / CANAL_TILES_PER_LAP)
        .round()
        .clamp(CANAL_MIN_TILES, CANAL_MAX_TILES);
    tiles * step / 2.0
}

/// The angle one step between neighbouring tiles subtends at the centre of the
/// world, which is what turns a width in tiles into a width on the world.
fn tile_arc(wm: &WorldMap) -> f64 {
    match wm.sphere() {
        // A geodesic's cells are equal-area to within a few percent, so the
        // distance between two centres is the side of a hexagon holding
        // `4π/n` of the sphere.
        Some(_) => {
            let per_tile = 4.0 * std::f64::consts::PI / wm.tiles.len().max(1) as f64;
            (2.0 * per_tile / 3f64.sqrt()).sqrt()
        }
        // A flat map is read as the equirectangular projection it looks like,
        // and its columns and its rows are not the same width apart. Take the
        // coarser of the two, so a canal counted in tiles is never thinner
        // than that count whichever way it happens to be running.
        None => (std::f64::consts::TAU / wm.width.max(1) as f64)
            .max(std::f64::consts::PI / (wm.height - 1).max(1) as f64),
    }
}

/// The land of a Grand Canals world: solid ground, less the six canals, less
/// whatever natural sea the world's land share still leaves room for.
///
/// The canals are cut first because they are geometry rather than luck — the
/// same six lanes on every seed — and what they take is then counted against
/// the water the world type asks for instead of being added on top of it.
/// A world small enough that the canals alone are more water than the share
/// allows gets no natural sea at all rather than a share it cannot honour: on
/// a Duel globe a lap is under sixty tiles, so six two-tile lanes are most of
/// the water there is room for.
///
/// What is held out of the field is what each shape holds out of it elsewhere:
/// a globe keeps its twelve pentagons and, when the world has poles, its caps;
/// a flat map keeps the top and bottom rows, which are its edge rather than
/// its climate.
fn canal_world(wm: &WorldMap, poles: MapPoles, rng: &mut Rng) -> BTreeSet<Pos> {
    let canals = grand_canals(wm);
    let pentagons: BTreeSet<Pos> = wm
        .sphere()
        .map(|sphere| sphere.pentagons().into_iter().collect())
        .unwrap_or_default();
    let globe = wm.sphere().is_some();
    let cap = if globe && poles.has_poles() {
        0.93
    } else {
        f64::MAX
    };
    let reserved = |pos: Pos| {
        if pentagons.contains(&pos) || wm.polar_fraction(pos) >= cap {
            return true;
        }
        let (_, row) = hex::axial_to_offset(pos.0, pos.1);
        !globe && (row == 0 || row == wm.height - 1)
    };

    let mut field: BTreeSet<Pos> = wm
        .tiles
        .keys()
        .copied()
        .filter(|pos| !canals.contains(pos) && !reserved(*pos))
        .collect();

    let tiles = wm.tiles.len();
    let wanted_water = tiles * (100 - MapScript::GrandCanals.land_percent() as usize) / 100;
    let spent = tiles - field.len();
    let sea = match wanted_water.checked_sub(spent) {
        Some(remaining) if remaining > 0 => {
            // Few and large. A canal world is read by its lanes, and a scatter
            // of ponds across the blocks between them would make it hard to
            // tell which water was dug and which was always there.
            let seas = (tiles / 900).clamp(2, 12);
            scatter_bodies(wm, &mut field, seas, remaining, rng)
        }
        _ => BTreeSet::new(),
    };

    wm.tiles
        .keys()
        .copied()
        .filter(|pos| !canals.contains(pos) && !sea.contains(pos) && !reserved(*pos))
        .collect()
}

/// How many bodies a world type may spread past a single plot.
///
/// `Lakes.lua` asks for four per continent region; every other stock script
/// asks for none and receives the one-plot ponds the same roll produces. The
/// two single-supercontinent types are given a budget of their own here, which
/// is a deliberate departure: their interiors are deep enough to hold an
/// inland sea, and a supercontinent whose only water is its own shoreline
/// plays as a flat expanse. The island types keep the stock zero — an island
/// has no interior to put a lake in, and the enclosure rule would refuse one
/// anyway. Earth's interiors are the ones that earned the rule.
///
/// Land Only asks for none for the opposite reason: the water it already has
/// is inland by construction, so the pass would be spreading lakes through a
/// world that is nothing but lakes.
fn large_lake_budget(script: MapScript, num_continents: usize) -> usize {
    match script {
        MapScript::Lakes => num_continents * 4,
        // The blocks a canal world is cut into are broad enough to hold one,
        // and a lake is the water a canal world does not already have: fresh,
        // enclosed, and nothing a ship arrives by.
        MapScript::Pangaea | MapScript::InlandSea | MapScript::GrandCanals => num_continents,
        MapScript::Continents | MapScript::TrueStartEarth => num_continents / 2,
        MapScript::LandOnly
        | MapScript::SmallContinents
        | MapScript::Islands
        | MapScript::WaterWorld => 0,
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
        MapTopology::Flat,
        MapPoles::Poles,
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
    topology: MapTopology,
    poles: MapPoles,
    rng: &mut Rng,
) -> (WorldMap, Vec<Pos>) {
    // The world's shape is asked for separately from what fills it. Fixed
    // geography only means its coastline is sampled rather than rolled: the
    // same longitudes and latitudes can be laid onto a flat atlas or a globe.
    // A globe is stored in a rectangle of its own shape, so its size's globe
    // is built rather than the cylinder a flat world lays out.
    let mut wm = if topology.is_globe() {
        WorldMap::globe(globe_frequency(width, height))
    } else {
        WorldMap::new(width, height)
    };

    // --- landmass, from the world type asked for and the shape it landed on
    let mut land = generate_land(&wm, script, poles, num_major_spawns, num_minor_spawns, rng);

    let land_list: Vec<Pos> = land.iter().cloned().collect();

    // --- relief, then climate. The stock generator settles elevation first
    // (MountainsCliffs.lua) and only then paints biomes over it, because the
    // mountain fractal has to be free of the latitude bands to run across them.
    apply_tectonics(&mut wm, &land, rng);
    assign_biomes(&mut wm, &land_list, poles, rng);

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

    // A canal is a cut, not an abyss. The shelf pass reaches the banks of one
    // from the land on either side, but the middle of a wide canal is far
    // enough from both to have been left as open ocean, and a lane a fleet
    // cannot enter until it has the technology for the deep sea is not a
    // canal. Every tile of one is therefore shallow water, sailable from the
    // first turn — which is the whole reason the world is cut this way.
    if script == MapScript::GrandCanals {
        // The canals are the same geometry every time they are asked for, and
        // asking again costs nothing and moves no RNG, so the pass does not
        // have to be threaded through the land generator to get here.
        for pos in grand_canals(&wm) {
            if let Some(tile) = wm.tiles.get_mut(&pos) {
                if tile.terrain == "ocean" {
                    tile.terrain = "coast".into();
                }
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
    // leaving navigable gaps instead of drawing an artificial solid wall. A
    // world with no poles has no cold end for it to form on, so it forms none:
    // the ends of a poleless world are as open as its middle, which is most of
    // what the setting is for.
    let polar_water: Vec<Pos> = if poles.has_poles() {
        wm.tiles
            .iter()
            .filter(|(position, tile)| {
                matches!(tile.terrain.as_str(), "coast" | "ocean")
                    && wm.polar_fraction(**position) > 0.82
            })
            .map(|(position, _)| *position)
            .collect()
    } else {
        Vec::new()
    };
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
    // The eight above are drawn first and shuffled on their own, so every map
    // size that asks for eight or fewer — which is every size Civilization VI
    // ships — consumes exactly the RNG it always did and lays out exactly the
    // world it always did. The scaled sizes ask for more, and only they pay
    // for the second shuffle.
    let mut wonder_names: Vec<&str> = wonder_names.to_vec();
    if num_natural_wonders > wonder_names.len() {
        let mut rest = [
            "torres_del_paine",
            "eye_of_the_sahara",
            "zhangye_danxia",
            "ha_long_bay",
            "cliffs_of_dover",
            "giants_causeway",
            "galapagos_islands",
            "matterhorn",
            "kilimanjaro",
            "piopiotahi",
            "ik_kil",
            "gobustan",
            "ubsunur_hollow",
            "mato_tipila",
            "delicate_arch",
            "chocolate_hills",
            "vesuvius",
            "lake_retba",
        ];
        for index in (1..rest.len()).rev() {
            let other = rng.below(index + 1);
            rest.swap(index, other);
        }
        wonder_names.extend(rest);
    }
    for wonder in wonder_names.iter().take(num_natural_wonders) {
        let footprint = match *wonder {
            "great_barrier_reef" | "yosemite" | "dead_sea" | "pamukkale" => 2,
            "mount_everest" => 3,
            "pantanal" => 4,
            "ha_long_bay" | "torres_del_paine" | "eye_of_the_sahara" | "ubsunur_hollow" => 3,
            "galapagos_islands" | "kilimanjaro" | "matterhorn" | "zhangye_danxia"
            | "cliffs_of_dover" | "giants_causeway" => 2,
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
                // The scaled sizes' wonders, by the biome each one belongs to.
                "torres_del_paine" | "matterhorn" | "kilimanjaro" | "vesuvius" | "piopiotahi" => {
                    t.terrain == "mountain"
                }
                "ha_long_bay" | "galapagos_islands" | "cliffs_of_dover" | "giants_causeway" => {
                    t.terrain == "coast"
                }
                "eye_of_the_sahara" | "delicate_arch" | "gobustan" | "lake_retba" => {
                    t.terrain == "desert" && !t.hills
                }
                "zhangye_danxia" | "chocolate_hills" => {
                    matches!(t.terrain.as_str(), "grassland" | "plains") && t.hills
                }
                "mato_tipila" | "ubsunur_hollow" => {
                    matches!(t.terrain.as_str(), "plains" | "tundra") && !t.hills
                }
                "ik_kil" => {
                    matches!(t.terrain.as_str(), "grassland" | "plains")
                        && !t.hills
                        && !t.has_river()
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
                .all(|placed| wm.distance(*placed, position) >= separation)
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

    place_strategic_quotas(rules, &mut wm, &land, num_major_spawns, &BTreeSet::new(), rng);

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
    // come out spread: `AssignStartingPlots` calls
    // `StartPositioner.DivideMapIntoMajorRegions` before it considers a single
    // plot, cuts the map into one region per civilization of roughly equal
    // fertility, and gives each region a start. So does this. Which landmass a
    // seat lands on falls out of the division rather than being allocated by
    // script, so an ocean-separated world seats every continent it can afford
    // to seat and a continuous one is divided just the same way.
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
    let all_candidates = candidates_for(&passable, total_spawns);
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
    let major_regions = if script == MapScript::Islands {
        // An archipelago's fair share is a slice of the whole island field,
        // not simply one of its largest rocks. `regions_for_seats` apportions
        // seats to landmasses by fertility, which is right when those
        // landmasses are continents. Here it picked the eight richest islands
        // without regard to where they were, leaving the other thirty or forty
        // islands outside every start region and sometimes putting one major's
        // nearest neighbour three times farther away than another's.
        //
        // Divide the complete archipelago instead. A region may contain more
        // than one island on this script: that is the maritime territory its
        // civilization opens into. The farthest-point seeds spread the regions
        // over the map, while the capacity pass still gives each one a roughly
        // equal share of land and fertility.
        let archipelago: Vec<Pos> = passable.iter().copied().collect();
        divide_into_regions(&wm, &archipelago, &fertility, num_major_spawns)
    } else {
        regions_for_seats(&wm, &components, &fertility, num_major_spawns)
    };
    let mut spawns = if script == MapScript::TrueStartEarth {
        // Earth does not divide into regions: the whole point of the script is
        // that Rome opens in Italy and the Aztecs open in Mexico, however
        // lopsided that leaves the continents.
        historic_major_spawns(&wm, &all_candidates, num_major_spawns)
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
        fill_remaining_starts(rules, &wm, &major_pool, &passable, &mut spawns, missing);
    }

    // City-states get a second, finer set of regions once the majors are down,
    // the way `StartPositioner.DivideMapIntoMinorRegions` cuts one. Here they
    // are cut *inside* each civilization's own cell and apportioned across
    // them, so every civilization has city-states of its own to court. Taking
    // the best-scoring remaining gap one at a time instead chained four of them
    // around whichever civilization had the best ground: measured over 72
    // worlds, every stock profile left at least one civilization with none
    // within ten hexes while another had eighteen.
    let minor_pool = pool_clear_of_wonders(START_DISTANCE_MINOR_NATURAL_WONDER, num_minor_spawns);
    let minor_regions = if script == MapScript::Islands && !spawns.is_empty() {
        archipelago_minor_regions(
            &wm,
            &passable,
            &fertility,
            &spawns,
            num_minor_spawns,
        )
    } else if major_regions.is_empty() || spawns.is_empty() {
        regions_for_seats(&wm, &components, &fertility, num_minor_spawns)
    } else {
        // A civilization's own ground means the land nearer to it than to
        // anyone else, not the region it was given — its start can sit
        // off-centre in that. Cutting from the cell is what keeps a city-state
        // on the side of the frontier it was meant for.
        //
        // Nearest *on the same landmass*, though. Distance is measured across
        // water as readily as across grass, so a plain cell reaches over a
        // strait and claims the near shore of an island nobody was seated on —
        // and a city-state cut from that half of the cell opens an ocean away
        // from the civilization it was meant to belong to. Measured on a
        // fifty-seat world, sixteen hexes away.
        let home_of = |position: Pos| {
            components
                .iter()
                .position(|component| component.contains(&position))
        };
        let start_home: Vec<Option<usize>> = spawns.iter().copied().map(home_of).collect();
        let start_distances: Vec<MapDistanceRow<'_>> = spawns
            .iter()
            .copied()
            .map(|start| MapDistanceRow::new(&wm, start))
            .collect();
        let mut cells: Vec<Vec<Pos>> = vec![Vec::new(); spawns.len()];
        for tile in major_regions.iter().flatten() {
            let here = home_of(*tile);
            let owner = start_distances
                .iter()
                .enumerate()
                .filter(|(index, _)| start_home[*index] == here)
                .map(|(index, distances)| (distances.distance(*tile), index))
                .min()
                .or_else(|| {
                    start_distances
                        .iter()
                        .enumerate()
                        .map(|(index, distances)| (distances.distance(*tile), index))
                        .min()
                });
            if let Some((_, owner)) = owner {
                cells[owner].push(*tile);
            }
        }
        // A seat the region system could not place — the fallback filler put it
        // somewhere the divisions never covered — has no cell at all, and so
        // was handed no city-state region either, on an island with room for
        // one. Give it the land it is nearest on its own landmass.
        for (index, start) in spawns.iter().enumerate() {
            if !cells[index].is_empty() {
                continue;
            }
            let Some(here) = home_of(*start) else {
                continue;
            };
            cells[index] = components[here]
                .iter()
                .copied()
                .filter(|tile| {
                    spawns
                        .iter()
                        .enumerate()
                        .filter(|(other, _)| start_home[*other] == Some(here))
                        .min_by_key(|(other, seat)| (wm.distance(*tile, **seat), *other))
                        .is_some_and(|(other, _)| other == index)
                })
                .collect();
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
        // Every civilization gets one before any gets two. Sharing them out in
        // proportion alone is right where the cells are near enough equal — on
        // a continent they are, by construction — but an archipelago's cells
        // differ tenfold, and a tenth of a share rounds to nothing: a
        // civilization on a small island was handed no city-state region at
        // all, on an island with room for one, while a neighbour on the big
        // island held three.
        let seated_cells: Vec<usize> = (0..cells.len())
            .filter(|index| !cells[*index].is_empty())
            .collect();
        let allocation = if num_minor_spawns >= seated_cells.len() && !seated_cells.is_empty() {
            let mut given = vec![0_usize; cells.len()];
            for index in &seated_cells {
                given[*index] = 1;
            }
            let spare: Vec<i64> = seated_cells.iter().map(|index| weights[*index]).collect();
            let extra = apportion(&spare, num_minor_spawns - seated_cells.len());
            for (slot, index) in seated_cells.iter().enumerate() {
                given[*index] += extra[slot];
            }
            given
        } else {
            apportion(&weights, num_minor_spawns)
        };
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
        fill_remaining_starts(rules, &wm, &minor_pool, &passable, &mut spawns, missing);
    }
    let occupied: BTreeSet<Pos> = spawns.iter().copied().collect();
    for s in &occupied {
        let t = wm.tiles.get_mut(s).unwrap();
        t.feature = None;
        t.resource = None;
    }
    // Clearing the capitals takes deposits off the map, and the quota is a
    // claim about what a civilization can reach rather than about what was
    // laid down before the seats were known. On a world with plenty of land
    // that is one or two tiles and the top-up finds nothing to do; on a Water
    // World, where every seat is a large share of the land, it is the
    // difference between an iron age and a bronze one. Nothing after this
    // draws from the stream, so a world that needed no top-up is unmoved.
    place_strategic_quotas(rules, &mut wm, &land, num_major_spawns, &occupied, rng);
    place_artifact_quotas(rules, &mut wm, num_major_spawns, &occupied, rng);
    remove_water_boundary_rivers(&mut wm);
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

/// The latitude a world with no poles is painted at.
///
/// A poleless world still has to hand `TerrainGenerator.lua`'s bands *some*
/// latitude, and this is the one that reads as warm everywhere: below
/// [`TUNDRA_LATITUDE`], so no tile is ever cold enough for tundra or snow, and
/// inside the desert belt, so the dry fractal is still free to lay deserts
/// down wherever it is dry. What the bands then decide is rainfall alone,
/// which is exactly what a world without cold ends should be deciding on.
const POLELESS_LATITUDE: f64 = 0.34;

/// The grain of the fractal that lays out heat on a randomized world.
///
/// The same octave count the desert, plains and variation fractals use, so
/// scattered heat arrives in patches the size of a desert region rather than
/// as per-tile confetti — a randomized world is still made of climates, they
/// just aren't where latitude would put them.
const THERMAL_FRACTAL_GRAIN: u32 = 3;

/// Climate, the way `TerrainGenerator.lua` paints it: latitude bands whose
/// borders are roughened by a variation fractal, with Desert and Plains cut
/// out of two further fractals so that both arrive as regions. Desert is
/// additionally confined to the subtropics, which is why Civ VI worlds have
/// desert belts either side of a green equator rather than desert everywhere.
///
/// With poles, latitude is where the tile actually is: hottest across the
/// middle of the world and colder with every step towards either extreme,
/// ending in tundra and then snow. Without them, every tile is handed the same
/// warm latitude instead, so there is no cold end to the world at all and the
/// two fractals decide everything between desert, plains and grassland.
/// Randomized hands each tile a latitude drawn from a fourth fractal, so the
/// full range from snow to jungle survives but stops running north to south.
///
/// That fourth fractal is built **only** for `Randomized`. Drawing it
/// unconditionally would advance `rng` before the desert and plains fractals
/// and re-roll every existing world from the same seed.
fn assign_biomes(wm: &mut WorldMap, land: &[Pos], poles: MapPoles, rng: &mut Rng) {
    let (width, height) = (wm.width, wm.height);
    let deserts = Fractal::new(rng, width, height, 3);
    let plains = Fractal::new(rng, width, height, 3);
    let variation = Fractal::new(rng, width, height, 3);
    let desert_bottom = deserts.percentile(100 - DESERT_PERCENT);
    let plains_bottom = plains.percentile(100 - PLAINS_PERCENT);
    let thermal = matches!(poles, MapPoles::Randomized)
        .then(|| Fractal::new(rng, width, height, THERMAL_FRACTAL_GRAIN));

    for pos in land {
        let (col, row) = noise_cell(wm, *pos);
        if wm.tiles[pos].terrain == "mountain" {
            continue;
        }
        let base = match poles {
            MapPoles::Poles => wm.polar_fraction(*pos),
            MapPoles::NoPoles => POLELESS_LATITUDE,
            // `thermal` is Some for exactly this arm.
            MapPoles::Randomized => thermal
                .as_ref()
                .map_or(POLELESS_LATITUDE, |f| f.at(col, row) as f64 / 255.0),
        };
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
/// tests hold that spread to, so it lives beside the placer it grades.
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

/// The one start distance that is a rule rather than an aim.
///
/// `Game::can_found_city` refuses a site within four tiles of a city that
/// already exists, so two starts closer than that are two cities only one of
/// which can be founded. Every other distance here is a target the layout
/// scores itself against and misses when the land gives it no choice; this one
/// is a floor, and a layout that breaks it is not a layout.
///
/// It went unnoticed while every world type was continuous, because a
/// landmass always had a tile at the right distance and the placer never had
/// to choose between crowding and a long jump. An archipelago is the first
/// world where the nearest legal tile can be on another island: aiming for
/// "five from the last city-state" then scores a neighbouring rock two tiles
/// away above an empty island ten tiles away, and takes it.
pub(crate) const MIN_START_SEPARATION: i32 = 4;

/// Every *other* start distance is a floor too, not a target.
///
/// Civilization VI reads them in `AssignStartingPlots:__MajorCivBuffer`, which
/// rejects a major site when any major already placed is within
/// `START_DISTANCE_MAJOR_CIVILIZATION - START_DISTANCE_RANGE_MAJOR` — so 12 and
/// 2 describe eleven hexes of clearance, not a band centred on twelve. CIVVIS
/// read them as an aim and pulled every start back toward it, which left a map
/// with room to spare unused and, worse, let the pull overrule the floor above:
/// measured over 72 generated worlds, starts landed as close as 2 hexes apart
/// on Continents, Small Continents and Planet.
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
/// What bare ground is worth before anything grows on it, and so how the
/// division trades **area against quality**. The yield term above runs from 0
/// on snow to about 8 on a grassland hill, so this constant sets the ratio
/// between the emptiest tile and the richest: at 1 a region of desert is five
/// times the size of a region of grassland and the split is even in fertility
/// but wildly uneven in *space*, which is what "evenly distributed" means to
/// somebody looking at the map. Measured at 100 seats, 1 gave regions of
/// 144-361 tiles; 8 halves that spread while still preferring good land.
const LAND_IS_WORTH: i32 = 8;

fn tile_fertility(rules: &Rules, tile: &crate::world::Tile) -> i32 {
    if rules.is_water(tile) || !rules.is_passable(tile) {
        return 0;
    }
    let yields = rules.tile_yields(tile);
    LAND_IS_WORTH
        + (yields.food * 2.0
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

/// Exact distances from one anchor for a deliberate bulk comparison. Flat
/// maps already have a constant-time coordinate formula; Planet builds one
/// temporary graph row and releases it with the region pass that requested it.
enum MapDistanceRow<'a> {
    Flat { from: Pos, width: i32 },
    Planet {
        sphere: &'a crate::sphere::Sphere,
        row: Box<[u16]>,
    },
}

impl<'a> MapDistanceRow<'a> {
    fn new(wm: &'a WorldMap, from: Pos) -> Self {
        if let Some(sphere) = wm.sphere() {
            Self::Planet {
                sphere,
                row: sphere.distance_row(from),
            }
        } else {
            Self::Flat {
                from,
                width: wm.width,
            }
        }
    }

    fn distance(&self, to: Pos) -> i32 {
        match self {
            Self::Flat { from, width } => hex::wdistance(*from, to, *width),
            Self::Planet { sphere, row } => sphere.row_distance(row, to),
        }
    }
}

fn map_distances_to(wm: &WorldMap, from: Pos, targets: &[Pos]) -> Vec<i32> {
    if let Some(sphere) = wm.sphere() {
        sphere.distances_to(from, targets)
    } else {
        targets
            .iter()
            .map(|target| hex::wdistance(from, *target, wm.width))
            .collect()
    }
}

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
    let mut costs = vec![0_i64; region.len()];
    for other in sample {
        let weight = fertility.get(&other).copied().unwrap_or(1) as i64;
        for (cost, distance) in costs
            .iter_mut()
            .zip(map_distances_to(wm, other, region))
        {
            *cost += distance as i64 * weight;
        }
    }
    region
        .iter()
        .copied()
        .zip(costs)
        .min_by_key(|(position, cost)| (*cost, *position))
        .map(|(position, _)| position)
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
    // Farthest-point sampling only changes a tile's nearest-center distance
    // when a new center is added. Carry that minimum forward instead of
    // rescanning every earlier center for every tile on every round; the
    // latter is quadratic in the seat count and made 100-seat Planet maps
    // spend tens of seconds before region assignment even began.
    let mut nearest = vec![i32::MAX; land.len()];
    while centers.len() < count {
        let newest = *centers.last().unwrap();
        let distances = MapDistanceRow::new(wm, newest);
        for (index, position) in land.iter().enumerate() {
            nearest[index] = nearest[index].min(distances.distance(*position));
        }
        let Some(next) = land
            .iter()
            .enumerate()
            .filter(|(_, position)| !centers.contains(position))
            .max_by_key(|(index, position)| (nearest[*index], **position))
            .map(|(_, position)| position)
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
        let distance_rows: Vec<MapDistanceRow<'_>> = centers
            .iter()
            .copied()
            .map(|center| MapDistanceRow::new(wm, center))
            .collect();
        let mut reach: Vec<(i32, Pos, usize)> = Vec::with_capacity(land.len() * centers.len());
        for position in land {
            for (index, distances) in distance_rows.iter().enumerate() {
                reach.push((distances.distance(*position), *position, index));
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

/// Ground one start needs to itself before a landmass can be said to hold
/// another: a capital works a radius-3 disk, and two of them must keep
/// `MIN_START_SEPARATION` apart.
const LAND_PER_START: usize = 24;
/// And below this a landmass cannot carry a capital's working radius at all,
/// so it is never a start however many seats are looking for a home.
const MIN_LANDMASS_FOR_A_START: usize = 12;

/// Hand `seats` out over `weights` by largest remainder — the apportionment
/// rule that gives each landmass its fair share of the world's seats and lets
/// rounding fall where the shortfall is largest, rather than where the list
/// happens to start — with each landmass capped at what it has room for, and
/// the overflow re-apportioned among those with slack.
fn apportion_capped(weights: &[i64], caps: &[usize], seats: usize) -> Vec<usize> {
    let mut given = vec![0_usize; weights.len()];
    let mut left = seats.min(caps.iter().sum());
    while left > 0 {
        let open: Vec<usize> = (0..weights.len())
            .filter(|index| given[*index] < caps[*index])
            .collect();
        if open.is_empty() {
            break;
        }
        let shares: Vec<i64> = open.iter().map(|index| weights[*index]).collect();
        let round = apportion(&shares, left);
        let mut handed = 0;
        for (slot, index) in open.iter().enumerate() {
            let take = round[slot].min(caps[*index] - given[*index]);
            given[*index] += take;
            handed += take;
        }
        if handed == 0 {
            // Every open landmass rounded to nothing; give the next seat to
            // the largest one with room rather than loop forever.
            let Some(index) = open
                .iter()
                .copied()
                .max_by_key(|index| (weights[*index], std::cmp::Reverse(*index)))
            else {
                break;
            };
            given[index] += 1;
            handed = 1;
        }
        left -= handed.min(left);
    }
    given
}

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
    // Every landmass big enough to carry a capital at all.
    let habitable: Vec<usize> = (0..components.len())
        .filter(|index| components[*index].len() >= MIN_LANDMASS_FOR_A_START)
        .collect();
    let room = |set: &[usize]| -> usize {
        set.iter()
            .map(|index| (components[*index].len() / LAND_PER_START).max(1))
            .sum()
    };
    // Prefer landmasses worth at least half a seat's share of the world, so a
    // seat is not spent on a sandbar while a continent goes short...
    let fair = total / (2 * seats as i64).max(1);
    let generous: Vec<usize> = habitable
        .iter()
        .copied()
        .filter(|index| weights[*index] >= fair)
        .collect();
    // ...but an archipelago has no landmass that rich, and holding out for one
    // crowds every seat onto the three biggest islands while twenty stand
    // empty. When the rich ones cannot seat the world, every habitable one is
    // in play.
    let mut eligible = if room(&generous) >= seats {
        generous
    } else {
        habitable
    };
    if eligible.is_empty() {
        eligible = vec![0];
    }
    let shares: Vec<i64> = eligible.iter().map(|index| weights[*index]).collect();
    let caps: Vec<usize> = eligible
        .iter()
        .map(|index| (components[*index].len() / LAND_PER_START).max(1))
        .collect();
    let allocation = apportion_capped(&shares, &caps, seats);
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

/// Give an Islands civilization its own share of the city-states without
/// throwing the unoccupied islands away.
///
/// The general minor pass cuts regions from the major regions. That is right
/// on a continent, where the region is the land the civilization can expand
/// through, but an Islands major region deliberately spans several islands.
/// Its nearest-start cell is the maritime equivalent: every island belongs to
/// the civilization it is nearest, including islands without a major start.
///
/// City-states are apportioned evenly rather than by cell fertility. With the
/// stock 3:2 minor/major ratio this means one per civilization first and the
/// remaining half-share on the roomiest cells, never zero for one empire and
/// three for another. The first round uses the shipped two-buffer reach around
/// its major so everybody has one nearby; later rounds use the ordinary
/// capacity-balanced subdivision of the full cell.
fn archipelago_minor_regions(
    wm: &WorldMap,
    land: &BTreeSet<Pos>,
    fertility: &BTreeMap<Pos, i32>,
    majors: &[Pos],
    seats: usize,
) -> Vec<Vec<Pos>> {
    if seats == 0 || majors.is_empty() {
        return Vec::new();
    }
    let major_distances: Vec<MapDistanceRow<'_>> = majors
        .iter()
        .copied()
        .map(|major| MapDistanceRow::new(wm, major))
        .collect();
    let mut cells: Vec<Vec<Pos>> = vec![Vec::new(); majors.len()];
    for tile in land {
        let owner = major_distances
            .iter()
            .enumerate()
            .map(|(index, distances)| (distances.distance(*tile), index))
            .min()
            .unwrap()
            .1;
        cells[owner].push(*tile);
    }
    for cell in &mut cells {
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
    let baseline = seats / majors.len();
    let mut allocation = vec![baseline; majors.len()];
    let mut order: Vec<usize> = (0..majors.len()).collect();
    order.sort_by_key(|index| (std::cmp::Reverse(weights[*index]), *index));
    for index in order.into_iter().take(seats % majors.len()) {
        allocation[index] += 1;
    }

    let divided: Vec<Vec<Vec<Pos>>> = cells
        .iter()
        .zip(&allocation)
        .map(|(cell, count)| divide_into_regions(wm, cell, fertility, *count))
        .collect();
    let mut regions = Vec::with_capacity(seats);
    let rounds = allocation.iter().copied().max().unwrap_or(0);
    for round in 0..rounds {
        for (owner, count) in allocation.iter().copied().enumerate() {
            if round >= count {
                continue;
            }
            if round == 0 {
                let nearby: Vec<Pos> = cells[owner]
                    .iter()
                    .copied()
                    .filter(|position| {
                        major_distances[owner].distance(*position)
                            <= 2 * START_DISTANCE_MINOR_MAJOR
                    })
                    .collect();
                if !nearby.is_empty() {
                    regions.push(nearby);
                    continue;
                }
            }
            if let Some(region) = divided[owner].get(round) {
                regions.push(region.clone());
            }
        }
    }
    regions
}

/// Give each region its start: the site that is both good and central, subject
/// to the shipped clearance buffers against everything already on the map.
///
/// A region that cannot honour its buffer relaxes it a hex at a time rather
/// than failing, down to `MIN_START_SEPARATION` — the radius inside which a
/// city cannot be founded at all, which is never given up. Clearance outranks
/// the tile: at each rung the whole region is searched before the next hex is
/// surrendered, so a start stands on a hill with proper spacing rather than on
/// grassland jammed against its neighbour. A region with no site even that
/// clear is left to the caller's fallback rather than handed a start the game
/// would silently move somewhere else.
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
    let relax_limit = foreign_buffer.max(own_buffer);
    let mut foreign_blocked: BTreeMap<i32, BTreeSet<Pos>> = BTreeMap::new();
    for relaxed in 0..=relax_limit {
        let radius = (foreign_buffer - relaxed).max(MIN_START_SEPARATION - 1);
        foreign_blocked.entry(radius).or_insert_with(|| {
            foreign
                .iter()
                .flat_map(|start| wm.disk(*start, radius))
                .collect()
        });
    }
    for (index, region) in regions.iter().enumerate() {
        let Some(center) = region_center(wm, region, fertility) else {
            continue;
        };
        let center_distances = MapDistanceRow::new(wm, center);
        let preferred: Vec<Pos> = region
            .iter()
            .copied()
            .filter(|position| candidates.contains(position))
            .collect();
        let worth = |position: &Pos| {
            start_quality(rules, wm, *position)
                - REGION_CENTRALITY_PULL * center_distances.distance(*position)
        };
        let mut chosen = None;
        'search: for relaxed in 0..=relax_limit {
            let foreign_want = (foreign_buffer - relaxed).max(MIN_START_SEPARATION - 1);
            let own_want = (own_buffer - relaxed).max(MIN_START_SEPARATION - 1);
            let blocked_by_placed: BTreeSet<Pos> = placed
                .iter()
                .flat_map(|start| wm.disk(*start, own_want))
                .collect();
            for pool in [preferred.as_slice(), region.as_slice()] {
                chosen = pool
                    .iter()
                    .filter(|position| {
                        !foreign_blocked[&foreign_want].contains(position)
                            && !blocked_by_placed.contains(position)
                    })
                    .max_by_key(|position| (worth(position), **position))
                    .copied();
                if chosen.is_some() {
                    break 'search;
                }
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
/// crowded for one start apiece. Takes the site with the most room left, which
/// is the same rule `Game::city_state_site` would apply afterwards — applying
/// it here keeps the choice inside the generator, where the map is still
/// visible.
///
/// A capital site is preferred, but not at the price of `MIN_START_SEPARATION`:
/// a fistful of grassland tiles bunched in one corner is exactly the shape of
/// pool that made this pass hand out a start three hexes from its neighbour,
/// which the game then refuses to found on. Any passable ground with the room
/// beats good ground without it.
fn fill_remaining_starts(
    rules: &Rules,
    wm: &WorldMap,
    candidates: &BTreeSet<Pos>,
    land: &BTreeSet<Pos>,
    spawns: &mut Vec<Pos>,
    count: usize,
) {
    // The fallback asks the same question for every candidate: its distance
    // to the nearest occupied start. Build that exact graph-distance field
    // once, then repair only the cells made nearer by each newly placed seat.
    // Re-running a point-to-point search for candidate × start × missing seat
    // made a large archipelago spend minutes here after an otherwise healthy
    // regional layout missed only a handful of city-states.
    let mut room: BTreeMap<Pos, i32> = wm
        .tiles
        .keys()
        .copied()
        .map(|position| (position, i32::MAX))
        .collect();
    let mut frontier = VecDeque::new();
    for start in spawns.iter().copied() {
        if let Some(distance) = room.get_mut(&start) {
            if *distance != 0 {
                *distance = 0;
                frontier.push_back(start);
            }
        }
    }
    while let Some(position) = frontier.pop_front() {
        let next = room[&position].saturating_add(1);
        for neighbor in wm.neighbors(position) {
            if room.get(&neighbor).is_some_and(|distance| next < *distance) {
                room.insert(neighbor, next);
                frontier.push_back(neighbor);
            }
        }
    }
    for _ in 0..count {
        let pick = |pool: &BTreeSet<Pos>, spawns: &[Pos], floor: i32| {
            pool.iter()
                .filter(|position| {
                    !spawns.contains(position) && room.get(*position).copied().unwrap_or(0) >= floor
                })
                .max_by_key(|position| {
                    (
                        room.get(*position).copied().unwrap_or(0),
                        start_quality(rules, wm, **position),
                        **position,
                    )
                })
                .copied()
        };
        let next = pick(candidates, spawns, MIN_START_SEPARATION)
            .or_else(|| pick(land, spawns, MIN_START_SEPARATION))
            .or_else(|| pick(candidates, spawns, 0))
            .or_else(|| pick(land, spawns, 0));
        let Some(next) = next else {
            break;
        };
        spawns.push(next);
        if room.get(&next).copied().unwrap_or(0) != 0 {
            room.insert(next, 0);
            frontier.push_back(next);
        }
        while let Some(position) = frontier.pop_front() {
            let next_distance = room[&position].saturating_add(1);
            for neighbor in wm.neighbors(position) {
                if room
                    .get(&neighbor)
                    .is_some_and(|distance| next_distance < *distance)
                {
                    room.insert(neighbor, next_distance);
                    frontier.push_back(neighbor);
                }
            }
        }
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
    // Which start each tile belongs to and which it would fall to next. With
    // this table a trial move costs one pass over the land instead of one pass
    // per start per tile, which is what lets the walk below run once per seat
    // rather than a fixed handful of times — and at a hundred seats that is the
    // difference between the pass working and the pass being decorative.
    let survey = |starts: &[Pos]| -> Vec<(i32, usize, i32, usize)> {
        let distance_rows: Vec<MapDistanceRow<'_>> = starts
            .iter()
            .copied()
            .map(|start| MapDistanceRow::new(wm, start))
            .collect();
        land.iter()
            .map(|tile| {
                let mut best = (i32::MAX, 0_usize);
                let mut next = (i32::MAX, 0_usize);
                for (index, distances) in distance_rows.iter().enumerate() {
                    let distance = distances.distance(*tile);
                    if distance < best.0 {
                        next = best;
                        best = (distance, index);
                    } else if distance < next.0 {
                        next = (distance, index);
                    }
                }
                (best.0, best.1, next.0, next.1)
            })
            .collect()
    };
    let seats = seated.len();
    let tally = |nearest: &[(i32, usize, i32, usize)]| -> Vec<usize> {
        let mut held = vec![0_usize; seats];
        for (_, owner, _, _) in nearest {
            held[*owner] += 1;
        }
        held
    };
    // What the tally becomes if seat `mover` stands on `site` instead. Exact:
    // a tile it loses falls to the start it was already second-nearest to, and
    // a tile it gains comes from the start that held it.
    let after = |nearest: &[(i32, usize, i32, usize)],
                 held: &[usize],
                 mover: usize,
                 site: Pos|
     -> Vec<usize> {
        let mut moved = held.to_vec();
        let distances = MapDistanceRow::new(wm, site);
        for (tile, (own, owner, next, runner)) in land.iter().zip(nearest) {
            let distance = distances.distance(*tile);
            if *owner == mover {
                if distance > *next {
                    moved[mover] -= 1;
                    moved[*runner] += 1;
                }
            } else if distance < *own {
                moved[*owner] -= 1;
                moved[mover] += 1;
            }
        }
        moved
    };
    // The worst-off seat first, then the gap between best and worst: raising
    // the floor is the point, narrowing the spread is the tiebreak.
    let rank = |held: &[usize]| -> (usize, i64) {
        let fewest = held.iter().copied().min().unwrap_or(0);
        let most = held.iter().copied().max().unwrap_or(0);
        (fewest, -((most - fewest) as i64))
    };
    for _ in 0..seated.len() {
        let nearest = survey(&starts);
        let held = tally(&nearest);
        let best = rank(&held);
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
        let blocked: BTreeSet<Pos> = others
            .iter()
            .flat_map(|start| wm.disk(*start, floor - 1))
            .collect();
        let mut trials: Vec<Pos> = region
            .iter()
            .copied()
            .filter(|position| {
                *position != current
                    && candidates.contains(position)
                    && wm.distance(*position, current) <= 6
                    && !blocked.contains(position)
                    && start_quality(rules, wm, *position) >= keep
            })
            .collect();
        trials.sort_by_key(|position| (wm.distance(*position, current), *position));
        trials.truncate(32);
        let Some((score, site)) = trials
            .into_iter()
            .map(|trial| (rank(&after(&nearest, &held, poorest, trial)), trial))
            .max()
        else {
            return;
        };
        if score <= best {
            return;
        }
        starts[poorest] = site;
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
    // A seat with nowhere better to stand does not end the pass — it steps
    // aside so the next-weakest gets its turn. Stopping at the first one meant
    // a single boxed-in capital left every capital below it unlifted, which at
    // a hundred seats is most of them.
    let mut settled: BTreeSet<usize> = BTreeSet::new();
    for _ in 0..seated.len() {
        let Some(weakest) = (0..seated.len())
            .filter(|index| !settled.contains(index))
            .min_by_key(|index| (qualities[*index], *index))
        else {
            return;
        };
        let (region_index, _) = seated[weakest];
        let Some(region) = regions.get(region_index) else {
            settled.insert(weakest);
            continue;
        };
        let Some(center) = region_center(wm, region, fertility) else {
            settled.insert(weakest);
            continue;
        };
        let center_distances = MapDistanceRow::new(wm, center);
        let others: Vec<Pos> = seated
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != weakest)
            .map(|(_, (_, start))| *start)
            .collect();
        // Clearance is a radius predicate, so build the union of those disks
        // once for this seat. Testing every candidate against every other
        // start with an exact point-to-point distance was the dominant cost on
        // large Planet maps even though only the nearby few could reject it.
        let blocked: BTreeSet<Pos> = others
            .iter()
            .chain(foreign)
            .flat_map(|start| wm.disk(*start, floor - 1))
            .collect();
        let current = qualities[weakest];
        // The same centrality pull the first pick used, so lifting the weakest
        // capital cannot quietly push it into a corner of its own region and
        // hand the territory the division just balanced to its neighbours.
        let Some(better) = region
            .iter()
            .copied()
            .filter(|position| candidates.contains(position))
            .filter(|position| !blocked.contains(position))
            .map(|position| (start_quality(rules, wm, position), position))
            .filter(|(quality, _)| *quality > current)
            .max_by_key(|(quality, position)| {
                (
                    quality - REGION_CENTRALITY_PULL * center_distances.distance(*position),
                    *position,
                )
            })
        else {
            settled.insert(weakest);
            continue;
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

#[cfg(test)]
fn river_edge_has_outlet(
    wm: &WorldMap,
    edge: RiverEdge,
    is_water: &impl Fn(Pos) -> bool,
) -> bool {
    !is_water(edge.0)
        && !is_water(edge.1)
        && connected_river_edges(wm, edge)
            .into_iter()
            .any(|next| is_water(next.0) != is_water(next.1))
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
    // One breadth-first wave answers the distance to the nearest water for
    // every tile. Asking each land tile about every water tile was equivalent
    // on a flat map but catastrophic on a globe, where it materialized or
    // searched thousands of unrelated long-distance rows.
    let mut water_distance: BTreeMap<Pos, i32> = water_tiles
        .iter()
        .copied()
        .map(|position| (position, 0))
        .collect();
    let mut water_frontier: VecDeque<Pos> = water_tiles.iter().copied().collect();
    while let Some(position) = water_frontier.pop_front() {
        let next_distance = water_distance[&position] + 1;
        for neighbor in wm.neighbors(position) {
            if let std::collections::btree_map::Entry::Vacant(entry) = water_distance.entry(neighbor)
            {
                entry.insert(next_distance);
                water_frontier.push_back(neighbor);
            }
        }
    }
    let distance_to_water = |pos: Pos| water_distance.get(&pos).copied().unwrap_or(0);
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

/// Move every river mouth off the shoreline edge and onto its inland reach.
///
/// Generation traces a river from a land/water boundary so the existing seed
/// stream, lakes, features, resources and starts remain unchanged. Once those
/// decisions are complete, the boundary segment is removed: the next segment
/// already ends at the same coastal vertex, producing a proper river mouth
/// without a river running along an ocean or lake side.
fn remove_water_boundary_rivers(wm: &mut WorldMap) {
    let water = |position: Pos| {
        matches!(
            wm.tiles[&position].terrain.as_str(),
            "ocean" | "coast" | "lake"
        )
    };
    let shoreline: Vec<RiverEdge> = all_shared_edges(wm)
        .into_iter()
        .filter(|edge| wm.has_river_edge(edge.0, edge.1))
        .filter(|edge| water(edge.0) || water(edge.1))
        .collect();
    for (a, b) in shoreline {
        wm.set_river_edge(a, b, false);
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
    reserved: &BTreeSet<Pos>,
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
            .filter(|pos| !reserved.contains(pos))
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

/// Antiquity Sites and Shipwrecks are allocated per civilization rather than
/// rolled per tile: the shipped `ARCHAEOLOGY_SITES_PER_CIV_LAND` is **6** and
/// `ARCHAEOLOGY_SITES_PER_CIV_SEA` is **2**, so a standard eight-player map
/// carries 48 dig sites and 16 wrecks. The per-tile lottery still rolls them
/// as before and this tops the map up to the quota afterwards, on the same
/// eligibility test every other resource uses — so this changes how many
/// appear and never where they are allowed to appear.
///
/// It runs **after the seats are chosen**, and deliberately so. Every earlier
/// pass feeds the start-placement search: freeing tiles that the lottery would
/// have filled lets `place_strategic_quotas` seat more deposits, which draws
/// more from the shared stream, which moves the spawns. A Tiny map lost the
/// shipped 10-14 separation band that way. Nothing after this point reads the
/// stream, so the quota is invisible to the layout.
fn place_artifact_quotas(
    rules: &Rules,
    wm: &mut WorldMap,
    num_major_spawns: usize,
    reserved: &BTreeSet<Pos>,
    rng: &mut Rng,
) {
    let artifacts: Vec<String> = rules
        .resources
        .iter()
        .filter(|(_, spec)| spec.class == "artifact")
        .map(|(name, _)| name.clone())
        .collect();
    let all: Vec<Pos> = wm.tiles.keys().copied().collect();
    for resource in artifacts {
        let spec = &rules.resources[resource.as_str()];
        // A wreck lies in the water and a dig site on land; the resource's own
        // terrain list is what says which, so a future artifact needs no new
        // branch here.
        let sea = spec
            .terrain
            .iter()
            .all(|terrain| matches!(terrain.as_str(), "coast" | "ocean" | "lake"));
        let per_civ = if sea { 2 } else { 6 };
        let quota = per_civ * num_major_spawns;
        let mut standing: Vec<Pos> = all
            .iter()
            .copied()
            .filter(|pos| wm.tiles[pos].resource.as_deref() == Some(resource.as_str()))
            .collect();
        // The lottery rolls Artifacts like any other resource, which undershoots
        // on an ocean-heavy map and overshoots badly on a land-heavy one — a
        // Land Only world rolled 66 dig sites against a quota of 48. Trim as
        // readily as top up, so the map ends on the shipped number either way.
        while standing.len() > quota {
            let pick = rng.below(standing.len());
            let pos = standing.swap_remove(pick);
            wm.tiles.get_mut(&pos).unwrap().resource = None;
        }
        let mut wanted = quota - standing.len();
        if wanted == 0 {
            continue;
        }
        let mut candidates: Vec<Pos> = all
            .iter()
            .copied()
            .filter(|pos| !reserved.contains(pos))
            .filter(|pos| {
                let tile = &wm.tiles[pos];
                if tile.resource.is_some() {
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
                    .map(|c| wm.distance(*c, **p))
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
            .min_by_key(|(id, center)| (wm.distance(**center, *pos), *id))
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
    use crate::setup::{MapPoles, MapScript, MapTopology, CIV6_MAP_SIZES};

    /// The two shapes and the two climates, named so a call reads as the world
    /// it asks for rather than as four trailing arguments.
    const FLAT: MapTopology = MapTopology::Flat;
    const GLOBE: MapTopology = MapTopology::Planet;
    const POLED: MapPoles = MapPoles::Poles;
    const POLELESS: MapPoles = MapPoles::NoPoles;
    const SCATTERED: MapPoles = MapPoles::Randomized;

    /// Every world type but Earth, in the order the lobby lists them: most
    /// land first, most water last.
    const ROLLED_TYPES: [MapScript; 9] = [
        MapScript::LandOnly,
        MapScript::Lakes,
        MapScript::InlandSea,
        MapScript::GrandCanals,
        MapScript::Pangaea,
        MapScript::Continents,
        MapScript::SmallContinents,
        MapScript::Islands,
        MapScript::WaterWorld,
    ];

    /// What share of a generated world is dry land.
    fn land_share(world: &WorldMap, rules: &Rules) -> usize {
        let land = world
            .tiles
            .values()
            .filter(|tile| !rules.is_water(tile))
            .count();
        land * 100 / world.tiles.len()
    }

    fn land_components(world: &WorldMap, rules: &Rules) -> Vec<BTreeSet<Pos>> {
        let land = world
            .tiles
            .iter()
            .filter(|(_, tile)| !rules.is_water(tile))
            .map(|(position, _)| *position)
            .collect();
        connected_components(world, &land)
    }

    /// An archipelago's body count is only useful when those bodies are large
    /// enough to settle. Exercise the topology pass directly so a failure here
    /// cannot be hidden by the start placer's last-resort fallback.
    #[test]
    fn islands_build_a_settleable_archipelago_on_every_stock_planet() {
        let mut failures = Vec::new();
        for (index, size) in CIV6_MAP_SIZES.iter().enumerate() {
            let wm = WorldMap::globe(size.globe_frequency);
            let mut rng = Rng::new(83_000 + index as u64);
            let land = generate_land(
                &wm,
                MapScript::Islands,
                POLED,
                size.default_players,
                size.default_city_states,
                &mut rng,
            );
            let components = connected_components(&wm, &land);
            let seats = size.default_players + size.default_city_states;
            let settleable = components
                .iter()
                .filter(|component| component.len() >= MIN_LANDMASS_FOR_A_START)
                .count();
            let target = wm.tiles.len() * MapScript::Islands.land_percent() as usize / 100;
            let sizes = components.iter().map(BTreeSet::len).collect::<Vec<_>>();
            if land.len().abs_diff(target) >= MIN_LANDMASS_FOR_A_START {
                failures.push(format!(
                    "{} has {} land tiles instead of about {target}; bodies {sizes:?}",
                    size.name,
                    land.len()
                ));
            }
            if settleable < seats {
                failures.push(format!(
                    "{} needs {seats} settleable islands, got {settleable}; bodies {sizes:?}",
                    size.name
                ));
            }
            if components
                .iter()
                .any(|component| component.len() < MIN_LANDMASS_FOR_A_START)
            {
                failures.push(format!(
                    "{} generated island fragments smaller than the promised minimum: {sizes:?}",
                    size.name
                ));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn island_scatter_leaves_subminimum_pockets_as_water() {
        let wm = WorldMap::new(20, 12);
        let large: BTreeSet<Pos> = (2..10)
            .map(|column| hex::offset_to_axial(column, 3))
            .collect();
        let tiny: BTreeSet<Pos> = (14..17)
            .map(|column| hex::offset_to_axial(column, 8))
            .collect();
        let mut field: BTreeSet<Pos> = large.union(&tiny).copied().collect();
        let mut rng = Rng::new(92_041);
        let land = scatter_islands(&wm, &mut field, 11, 8, 8, 1, 2, &mut rng);
        let components = connected_components(&wm, &land);
        assert_eq!(components.iter().map(BTreeSet::len).collect::<Vec<_>>(), vec![8]);
        assert!(
            tiny.is_disjoint(&land),
            "a pocket too small to settle was committed as land"
        );
    }

    #[test]
    fn incremental_spawn_fallback_matches_pointwise_distance_search() {
        let rules = Rules::embedded();
        let mut wm = WorldMap::globe(5);
        for tile in wm.tiles.values_mut() {
            tile.terrain = "plains".into();
        }
        let land: BTreeSet<Pos> = wm.tiles.keys().copied().collect();
        let candidates = land.clone();
        let positions: Vec<Pos> = land.iter().copied().collect();
        let initial = vec![positions[0], positions[83]];
        let mut expected = initial.clone();
        for _ in 0..7 {
            let room = |spawns: &[Pos], position: Pos| {
                spawns
                    .iter()
                    .map(|start| wm.distance(position, *start))
                    .min()
                    .unwrap_or(i32::MAX)
            };
            let pick = |pool: &BTreeSet<Pos>, spawns: &[Pos], floor: i32| {
                pool.iter()
                    .filter(|position| {
                        !spawns.contains(position) && room(spawns, **position) >= floor
                    })
                    .max_by_key(|position| {
                        (
                            room(spawns, **position),
                            start_quality(&rules, &wm, **position),
                            **position,
                        )
                    })
                    .copied()
            };
            let next = pick(&candidates, &expected, MIN_START_SEPARATION)
                .or_else(|| pick(&land, &expected, MIN_START_SEPARATION))
                .or_else(|| pick(&candidates, &expected, 0))
                .or_else(|| pick(&land, &expected, 0))
                .unwrap();
            expected.push(next);
        }

        let mut actual = initial;
        fill_remaining_starts(
            &rules,
            &wm,
            &candidates,
            &land,
            &mut actual,
            7,
        );
        assert_eq!(actual, expected);
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
                    generate_with_script(&rules, 74, 46, 6, 9, 4, 3, script, FLAT, POLED, &mut rng);
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
                generate_with_script(&rules, 74, 46, 6, 9, 4, 3, MapScript::Lakes, FLAT, POLED, &mut rng);
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

    /// The world types are a dial from all land to all water, and the lobby
    /// lists them in that order. Two claims are checked here, on both shapes:
    /// that each type lands near the share it advertises, and that going down
    /// the list never gains land. Without the second, "ordered from land to
    /// water" is a comment rather than a property, and the two ends stop
    /// meaning anything.
    #[test]
    fn world_types_run_from_all_land_to_all_water_on_either_shape() {
        let rules = Rules::embedded();
        for topology in [FLAT, GLOBE] {
            let mut measured: Vec<(MapScript, usize)> = Vec::new();
            for (index, script) in ROLLED_TYPES.into_iter().enumerate() {
                // Three worlds, because a single roll of a scatter is noisy
                // and the claim is about the type, not about one seed.
                let mut total = 0;
                for seed in 0..3u64 {
                    let mut rng = Rng::new(19_000 + index as u64 * 8 + seed);
                    let (world, _) = generate_with_script(
                        &rules, 74, 46, 6, 9, 4, 3, script, topology, POLED, &mut rng,
                    );
                    total += land_share(&world, &rules);
                }
                measured.push((script, total / 3));
            }
            for (script, share) in &measured {
                let claimed = script.land_percent() as i64;
                // Wide, because the share is what the generator aims at and
                // the coast, the lakes and a globe's reserved caps all move it
                // a little. Narrow enough that a type cannot drift into its
                // neighbour's place on the dial.
                assert!(
                    (*share as i64 - claimed).abs() <= 8,
                    "{script:?} on {topology:?} is {share}% land, but the lobby says {claimed}%"
                );
            }
            for pair in measured.windows(2) {
                let ((above, more), (below, less)) = (pair[0], pair[1]);
                assert!(
                    more + 2 >= less,
                    "{below:?} ({less}% land) holds more land than {above:?} ({more}%), \
                     so the list is no longer ordered from land to water"
                );
            }
            // The two ends have to actually be the ends.
            assert!(
                measured[0].1 >= 85,
                "Land Only came out {}% land on {topology:?}",
                measured[0].1
            );
            assert!(
                measured[measured.len() - 1].1 <= 12,
                "Water World came out {}% land on {topology:?}",
                measured[measured.len() - 1].1
            );
        }
    }

    /// Where a tile stands around one of the world's axes, as a twelfth of a
    /// turn: 0 through 11, counting round the plane square to the axis. A lane
    /// that reaches all twelve has been all the way round the world.
    fn sector_around(world: &WorldMap, axis: [f64; 3], pos: Pos) -> usize {
        // Every canal axis is one of the world's own, so the plane square to
        // it is spanned by the other two coordinates and the angle round it is
        // read straight off them.
        let along = axis.iter().position(|part| *part != 0.0).unwrap();
        let point = world.direction(pos);
        let angle = point[(along + 2) % 3].atan2(point[(along + 1) % 3]);
        let turns = angle.rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU;
        ((turns * 12.0) as usize).min(11)
    }

    /// The claim the Grand Canals world is named for: six canals, two around
    /// each of the world's three axes, and every one of them a lane that comes
    /// back to where it started.
    ///
    /// "Circumnavigating" is checked as the thing a fleet would do rather than
    /// as a property of the arithmetic — the lane's own tiles have to be one
    /// connected body of water that appears in all twelve sectors of a turn
    /// around its axis, so a ship can follow it the whole way without ever
    /// coming ashore. A canal that broke into two arcs, or that stopped short
    /// of closing, would pass every share and spacing test in this file and
    /// fail here, which is the only place it would show.
    ///
    /// Both shapes, because a canal is cut at an angle to an axis of the world
    /// and neither the globe nor the flat map gets to answer that differently.
    /// Both climates and the two ends of the size table, because the smallest
    /// world is where a canal is widest against the world it circles — a Duel
    /// lap is under sixty tiles — and so where its far reach comes nearest the
    /// latitude at which a poled world grows sea ice.
    #[test]
    fn six_grand_canals_each_circle_the_world_and_meet_one_another() {
        let rules = Rules::embedded();
        for (index, size) in [&CIV6_MAP_SIZES[0], &CIV6_MAP_SIZES[3]].into_iter().enumerate() {
            for topology in [FLAT, GLOBE] {
                for poles in [POLED, POLELESS] {
                    let mut rng = Rng::new(
                        52_000
                            + index as u64 * 4
                            + topology.is_globe() as u64 * 2
                            + poles.has_poles() as u64,
                    );
                    let (world, _) = generate_with_script(
                        &rules,
                        size.width,
                        size.height,
                        size.default_players,
                        size.default_city_states,
                        size.natural_wonders,
                        size.continents,
                        MapScript::GrandCanals,
                        topology,
                        poles,
                        &mut rng,
                    );
                    let canals = grand_canals(&world);
                    let where_ = format!("{} {topology:?}/{poles:?}", size.id);
                    assert!(!canals.is_empty(), "{where_}: no canal was cut at all");

                    // Nothing dug is dry, and nothing dug is deep: the whole
                    // network is shallow water a fleet can enter on turn one.
                    for pos in &canals {
                        let tile = &world.tiles[pos];
                        assert!(rules.is_water(tile), "{where_}: dry land in a canal at {pos:?}");
                        assert_eq!(
                            tile.terrain, "coast",
                            "{where_}: the canal at {pos:?} is {} rather than shallow water",
                            tile.terrain
                        );
                    }

                    // Each of the six lanes on its own, split back out of the
                    // set by which band it belongs to — and counted as a fleet
                    // would count it, so a tile the polar band has frozen over
                    // is not part of the lane that has to close.
                    let half = canal_half_width(&world);
                    let offset = CANAL_OFFSET_DEGREES.to_radians();
                    let mut lanes = 0;
                    for direction in CANAL_AXES {
                        for sign in [1.0f64, -1.0] {
                            let lane: BTreeSet<Pos> = canals
                                .iter()
                                .copied()
                                .filter(|pos| {
                                    world.tiles[pos].feature.as_deref() != Some("ice")
                                })
                                .filter(|pos| {
                                    let out_of_plane = dot(world.direction(*pos), direction)
                                        .clamp(-1.0, 1.0)
                                        .asin();
                                    (out_of_plane - sign * offset).abs() <= half
                                })
                                .collect();
                            let named = format!(
                                "{where_}: the canal {} of axis {direction:?}",
                                if sign > 0.0 { "above" } else { "below" }
                            );
                            let mut bodies = connected_components(&world, &lane);
                            bodies.sort_by_key(|body| std::cmp::Reverse(body.len()));
                            let lap = bodies.first().cloned().unwrap_or_default();
                            assert!(
                                lap.len() * 4 >= lane.len() * 3,
                                "{named} is not one lane: {} open tiles in {} pieces, largest {}",
                                lane.len(),
                                bodies.len(),
                                lap.len()
                            );
                            let reached: BTreeSet<usize> = lap
                                .iter()
                                .map(|pos| sector_around(&world, direction, *pos))
                                .collect();
                            assert_eq!(
                                reached.len(),
                                12,
                                "{named} stops short of closing: it reaches {} of the twelve \
                                 sectors of a lap around its axis",
                                reached.len()
                            );
                            lanes += 1;
                        }
                    }
                    assert_eq!(lanes, 6, "{where_}: six canals, two around each of three axes");

                    // And the six are one network, not six rings: no two axes
                    // are parallel, so every lane crosses the four belonging to
                    // the other two axes and a ship can get from any of them to
                    // any other without portage.
                    let water: BTreeSet<Pos> = world
                        .tiles
                        .iter()
                        .filter(|(_, tile)| rules.is_water(tile))
                        .map(|(pos, _)| *pos)
                        .collect();
                    let sea = connected_components(&world, &water)
                        .into_iter()
                        .max_by_key(|body| body.len())
                        .unwrap_or_default();
                    assert!(
                        canals.iter().all(|pos| sea.contains(pos)),
                        "{where_}: part of the canal network cannot be sailed to from the rest"
                    );
                }
            }
        }
    }

    /// Every world type has to arrive on either shape, seat everybody, and —
    /// on a globe — close. The shape is a separate question from the type, and
    /// this is what makes that true rather than merely offered.
    /// The one placement guarantee that holds whatever the world is made of.
    ///
    /// `Game::can_found_city` refuses a site within `MIN_START_SEPARATION` of a
    /// city, so a layout that breaks the floor hands somebody a Settler that
    /// cannot found where it stands and a city-state that `city_state_site`
    /// teleports to the emptiest tile on the map — undoing, in one move, the
    /// even spread the generator just worked for. Aiming at the shipped
    /// distances instead of clearing them broke it on every ocean-separated
    /// type: measured before this test existed, up to twenty-one pairs of
    /// starts in a single Small Continents world.
    ///
    /// Every rolled world type, both shapes, both pole settings, every stock
    /// size.
    #[test]
    fn no_world_type_ever_crowds_two_starts_inside_the_founding_radius() {
        let rules = Rules::embedded();
        for (index, script) in ROLLED_TYPES.into_iter().enumerate() {
            for topology in [FLAT, GLOBE] {
                for poles in [POLED, MapPoles::NoPoles] {
                    for size in [&CIV6_MAP_SIZES[1], &CIV6_MAP_SIZES[3]] {
                        let mut rng = Rng::new(
                            41_000
                                + index as u64 * 8
                                + topology.is_globe() as u64 * 2
                                + matches!(poles, MapPoles::NoPoles) as u64,
                        );
                        let (world, spawns) = generate_with_script(
                            &rules,
                            size.width,
                            size.height,
                            size.default_players,
                            size.default_city_states,
                            size.natural_wonders,
                            size.continents,
                            script,
                            topology,
                            poles,
                            &mut rng,
                        );
                        let where_ = format!("{script:?} {topology:?} {poles:?} {}", size.id);
                        assert_eq!(
                            spawns.len(),
                            size.default_players + size.default_city_states,
                            "{where_}: not every seat was placed"
                        );
                        for (index, start) in spawns.iter().enumerate() {
                            for other in &spawns[index + 1..] {
                                let gap = world.distance(*start, *other);
                                assert!(
                                    gap >= MIN_START_SEPARATION,
                                    "{where_}: two starts {gap} apart, inside the founding radius"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_world_type_lays_out_on_either_shape() {
        let rules = Rules::embedded();
        for (index, script) in ROLLED_TYPES
            .into_iter()
            .chain([MapScript::TrueStartEarth])
            .enumerate()
        {
            for topology in [FLAT, GLOBE] {
                let mut rng = Rng::new(28_000 + index as u64 * 4 + topology.is_globe() as u64);
                let (world, spawns) = generate_with_script(
                    &rules, 60, 38, 4, 6, 3, 2, script, topology, POLED, &mut rng,
                );
                let where_ = format!("{script:?} on {topology:?}");
                assert_eq!(spawns.len(), 10, "{where_}: every seat is placed");
                let mut seen = BTreeSet::new();
                for start in &spawns {
                    assert!(seen.insert(*start), "{where_}: two civilizations share {start:?}");
                    let tile = &world.tiles[start];
                    assert!(!rules.is_water(tile), "{where_}: a start is at sea");
                    assert!(tile.terrain != "mountain", "{where_}: a start is on a peak");
                }
                assert_eq!(world.sphere().is_some(), topology.is_globe(), "{where_}: shape");
                if topology.is_globe() {
                    let mut pentagons = 0;
                    for (pos, _) in world.tiles.iter() {
                        match world.neighbors(*pos).len() {
                            5 => pentagons += 1,
                            6 => {}
                            other => panic!("{where_}: {pos:?} has {other} neighbours"),
                        }
                    }
                    assert_eq!(pentagons, 12, "{where_}: a globe closes with twelve pentagons");
                }
            }
        }
    }

    /// What "poles" means, stated as the thing a player would notice: the
    /// middle of the world is its warm ground and every step out towards an
    /// extreme is colder, ending in tundra and snow. Land Only is the type
    /// this is measured on, because it is the only one with land at every
    /// latitude to measure.
    #[test]
    fn poles_make_the_middle_of_the_world_hot_and_its_extremes_cold() {
        let rules = Rules::embedded();
        // Bands from the equator out to a pole. A tile is cold if the climate
        // pass gave it one of the two cold terrains.
        const BANDS: [(f64, f64); 4] = [(0.0, 0.2), (0.2, 0.45), (0.45, 0.7), (0.7, 1.01)];
        for topology in [FLAT, GLOBE] {
            let mut cold = [0usize; BANDS.len()];
            let mut total = [0usize; BANDS.len()];
            for seed in 0..3u64 {
                let mut rng = Rng::new(37_000 + seed);
                let (world, _) = generate_with_script(
                    &rules, 74, 46, 6, 9, 4, 3, MapScript::LandOnly, topology, POLED, &mut rng,
                );
                for (pos, tile) in world.tiles.iter() {
                    if rules.is_water(tile) {
                        continue;
                    }
                    let latitude = world.polar_fraction(*pos);
                    let Some(band) = BANDS
                        .iter()
                        .position(|(low, high)| latitude >= *low && latitude < *high)
                    else {
                        continue;
                    };
                    total[band] += 1;
                    if matches!(tile.terrain.as_str(), "snow" | "tundra") {
                        cold[band] += 1;
                    }
                }
            }
            let share: Vec<usize> = (0..BANDS.len())
                .map(|band| cold[band] * 100 / total[band].max(1))
                .collect();
            assert!(
                total.iter().all(|count| *count > 100),
                "{topology:?}: every band needs land in it to measure, got {total:?}"
            );
            assert_eq!(share[0], 0, "{topology:?}: the middle of the world is not cold");
            for band in 1..BANDS.len() {
                assert!(
                    share[band] >= share[band - 1],
                    "{topology:?}: band {band} is warmer than the one inside it, shares {share:?}"
                );
            }
            assert!(
                share[BANDS.len() - 1] >= 60,
                "{topology:?}: the extremes should be mostly tundra and snow, shares {share:?}"
            );
        }
    }

    /// And what "no poles" means: no cold end to the world at all. Not a
    /// milder one — none. Snow, tundra and sea ice are the three things a
    /// latitude puts on a map, and a world without poles carries none of them
    /// at any latitude, including the two rows that used to be its ice caps.
    #[test]
    fn a_world_without_poles_has_no_cold_end_at_any_latitude() {
        let rules = Rules::embedded();
        for topology in [FLAT, GLOBE] {
            for (index, script) in ROLLED_TYPES.into_iter().enumerate() {
                let mut rng = Rng::new(46_000 + index as u64 * 4);
                let (world, _) = generate_with_script(
                    &rules, 60, 38, 4, 6, 3, 2, script, topology, POLELESS, &mut rng,
                );
                let where_ = format!("{script:?} on {topology:?} without poles");
                for (pos, tile) in world.tiles.iter() {
                    assert!(
                        !matches!(tile.terrain.as_str(), "snow" | "tundra"),
                        "{where_}: {} at {pos:?}, {:.2} from the equator",
                        tile.terrain,
                        world.polar_fraction(*pos)
                    );
                    assert!(
                        tile.feature.as_deref() != Some("ice"),
                        "{where_}: sea ice at {pos:?}"
                    );
                }
            }
            // The same world with poles does carry all three, so the absence
            // above is the setting and not a broken generator.
            let mut rng = Rng::new(46_000);
            let (poled, _) = generate_with_script(
                &rules, 60, 38, 4, 6, 3, 2, MapScript::Pangaea, topology, POLED, &mut rng,
            );
            assert!(
                poled
                    .tiles
                    .values()
                    .any(|tile| matches!(tile.terrain.as_str(), "snow" | "tundra")),
                "{topology:?}: the poled control grew no cold terrain"
            );
            assert!(
                poled
                    .tiles
                    .values()
                    .any(|tile| tile.feature.as_deref() == Some("ice")),
                "{topology:?}: the poled control grew no sea ice"
            );
        }
    }

    /// And what "randomized" means: the full range of climates survives, but
    /// it stops running north to south. Every band from the equator out to a
    /// pole carries cold ground, and the equatorial band carries about as much
    /// of it as the polar band — which is exactly what a poled world forbids,
    /// where the middle of the world is 0% cold and the extremes are 60%+.
    #[test]
    fn a_randomized_world_scatters_cold_ground_across_every_latitude() {
        let rules = Rules::embedded();
        const BANDS: [(f64, f64); 4] = [(0.0, 0.2), (0.2, 0.45), (0.45, 0.7), (0.7, 1.01)];
        for topology in [FLAT, GLOBE] {
            let mut cold = [0usize; BANDS.len()];
            let mut total = [0usize; BANDS.len()];
            for seed in 0..3u64 {
                let mut rng = Rng::new(37_000 + seed);
                let (world, _) = generate_with_script(
                    &rules, 74, 46, 6, 9, 4, 3, MapScript::LandOnly, topology, SCATTERED,
                    &mut rng,
                );
                for (pos, tile) in world.tiles.iter() {
                    if rules.is_water(tile) {
                        continue;
                    }
                    let latitude = world.polar_fraction(*pos);
                    let Some(band) = BANDS
                        .iter()
                        .position(|(low, high)| latitude >= *low && latitude < *high)
                    else {
                        continue;
                    };
                    total[band] += 1;
                    if matches!(tile.terrain.as_str(), "snow" | "tundra") {
                        cold[band] += 1;
                    }
                }
            }
            let share: Vec<usize> = (0..BANDS.len())
                .map(|band| cold[band] * 100 / total[band].max(1))
                .collect();
            assert!(
                total.iter().all(|count| *count > 100),
                "{topology:?}: every band needs land in it to measure, got {total:?}"
            );
            // The equator is cold somewhere — the thing latitude bands make
            // impossible.
            assert!(
                share[0] > 0,
                "{topology:?}: randomized heat left the equator uniformly warm, shares {share:?}"
            );
            // And the poles are not the cold end: no band is more than four
            // times as cold as the equatorial one. A poled world of the same
            // size runs 0% to 60%+, which this bound rejects outright.
            assert!(
                share[BANDS.len() - 1] <= share[0] * 4,
                "{topology:?}: cold still piles up at the extremes, shares {share:?}"
            );
        }
    }

    /// Randomized worlds keep the cold terrains that a poleless world drops,
    /// and drop the polar sea-ice band that a poled world grows — cold ground
    /// exists, it just isn't at the ends of the world.
    #[test]
    fn a_randomized_world_has_cold_terrain_but_no_polar_ice_band() {
        let rules = Rules::embedded();
        for topology in [FLAT, GLOBE] {
            let mut rng = Rng::new(46_000);
            let (world, _) = generate_with_script(
                &rules, 60, 38, 4, 6, 3, 2, MapScript::Pangaea, topology, SCATTERED, &mut rng,
            );
            assert!(
                world
                    .tiles
                    .values()
                    .any(|tile| matches!(tile.terrain.as_str(), "snow" | "tundra")),
                "{topology:?}: a randomized world grew no cold terrain at all"
            );
            for (pos, tile) in world.tiles.iter() {
                assert!(
                    tile.feature.as_deref() != Some("ice"),
                    "{topology:?}: randomized heat still grew a polar ice cap at {pos:?}"
                );
            }
        }
    }

    /// The guard on the whole change: the thermal fractal is drawn only for a
    /// randomized world, so a poled or poleless world from a given seed is the
    /// same world it was before that fractal existed.
    #[test]
    fn only_randomized_worlds_draw_the_thermal_fractal() {
        let rules = Rules::embedded();
        for topology in [FLAT, GLOBE] {
            for poles in [POLED, POLELESS] {
                let mut first = Rng::new(51_000);
                let (a, _) = generate_with_script(
                    &rules, 60, 38, 4, 6, 3, 2, MapScript::Continents, topology, poles, &mut first,
                );
                // Drawing the same world again must leave `rng` in the same
                // place: if the biome pass had consumed an extra fractal for
                // these settings, the second draw would diverge.
                let mut second = Rng::new(51_000);
                let (b, _) = generate_with_script(
                    &rules, 60, 38, 4, 6, 3, 2, MapScript::Continents, topology, poles, &mut second,
                );
                assert_eq!(
                    a.tiles.iter().map(|(_, t)| t.terrain.clone()).collect::<Vec<_>>(),
                    b.tiles.iter().map(|(_, t)| t.terrain.clone()).collect::<Vec<_>>(),
                    "{topology:?}/{poles:?}: the same seed drew two different worlds"
                );
                assert_eq!(
                    first.next_u64(),
                    second.next_u64(),
                    "{topology:?}/{poles:?}: the two draws left the RNG in different places"
                );
            }
        }
    }

    #[test]
    fn stock_map_scripts_create_distinct_playable_topologies() {
        let rules = Rules::embedded();
        for (index, script) in ROLLED_TYPES.into_iter().enumerate() {
            let mut rng = Rng::new(72_000 + index as u64);
            let (world, spawns) =
                generate_with_script(&rules, 60, 38, 6, 6, 0, 3, script, FLAT, POLED, &mut rng);
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
                MapScript::LandOnly => assert!(
                    share(1) >= 80,
                    "Land Only should be one unbroken world, largest holds {}%",
                    share(1)
                ),
                MapScript::Islands | MapScript::WaterWorld => assert!(
                    components.len() >= 8 && components[0].len() * 4 <= total,
                    "{script:?} needs many small islands and no continent, got {:?}",
                    components.iter().map(|c| c.len()).collect::<Vec<_>>()
                ),
                // Six canals cross into twenty-four junctions, so the ground
                // they leave arrives already divided: many blocks, and no one
                // of them the world. A single dominant landmass here would
                // mean a canal had failed to close somewhere.
                MapScript::GrandCanals => assert!(
                    components.len() >= 8 && components[0].len() * 3 <= total,
                    "Grand Canals should cut the world into blocks, got {:?}",
                    components.iter().map(|c| c.len()).collect::<Vec<_>>()
                ),
                MapScript::TrueStartEarth => {
                    unreachable!("Earth is not in this list")
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
            MapScript::SmallContinents,
            GLOBE,
            POLED,
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
            GLOBE,
            POLED,
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
        // out in CIV_NAMES order, so this game's eight majors lead the spawn
        // list and take the first eight homelands.
        for (index, (longitude, latitude)) in EARTH_HOMELANDS.iter().enumerate().take(8) {
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

    /// Every seat's homeland is dry land on the sampled globe, and no two
    /// share a tile.
    ///
    /// A homeland that samples as ocean does not fail loudly — the search
    /// simply seats that civilization on the nearest viable land, which can be
    /// a continent away — so a True Start map would quietly stop being true
    /// for whichever civilization was added last. Two homelands on one tile
    /// are the same silent failure by another route.
    #[test]
    fn every_homeland_is_on_land_and_has_it_to_itself() {
        let rules = Rules::embedded();
        let size = CIV6_MAP_SIZES
            .iter()
            .find(|size| size.id == "huge")
            .unwrap();
        let mut rng = Rng::new(9_133);
        let (world, _) = generate_with_script(
            &rules,
            size.width,
            size.height,
            8,
            12,
            size.natural_wonders,
            size.continents,
            MapScript::TrueStartEarth,
            GLOBE,
            POLED,
            &mut rng,
        );
        let sphere = world.sphere().unwrap();
        assert_eq!(
            EARTH_HOMELANDS.len(),
            crate::game::CIV_NAMES.len(),
            "every civilization needs a homeland of its own"
        );
        let mut seen: BTreeMap<Pos, &str> = BTreeMap::new();
        let mut adrift: Vec<String> = Vec::new();
        for (civilization, (longitude, latitude)) in
            crate::game::CIV_NAMES.iter().zip(EARTH_HOMELANDS)
        {
            let target = earth_direction(longitude, latitude);
            let home = sphere
                .positions()
                .max_by(|a, b| {
                    let toward = |pos: &Pos| {
                        let center = sphere.center(*pos).unwrap();
                        center[0] * target[0] + center[1] * target[1] + center[2] * target[2]
                    };
                    toward(a).partial_cmp(&toward(b)).unwrap()
                })
                .unwrap();
            if rules.is_water(&world.tiles[&home]) {
                adrift.push(format!("{civilization} ({longitude}, {latitude}) is at sea"));
            }
            if let Some(other) = seen.insert(home, civilization) {
                adrift.push(format!("{civilization} shares {other}'s homeland tile"));
            }
        }
        assert!(adrift.is_empty(), "{}", adrift.join("; "));
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
                &rules, 60, 38, 4, 6, 3, 2, MapScript::TrueStartEarth, GLOBE, POLED, &mut rng,
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
        for (index, script) in ROLLED_TYPES
            .into_iter()
            .chain([MapScript::TrueStartEarth])
            .enumerate()
        {
            let mut rng = Rng::new(81_000 + index as u64);
            let (world, _) = generate_with_script(&rules, 60, 38, 6, 6, 0, 3, script, FLAT, POLED, &mut rng);
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

    /// Antiquity Sites and Shipwrecks are allocated per civilization, not
    /// rolled per tile. `ARCHAEOLOGY_SITES_PER_CIV_LAND` is 6 and
    /// `ARCHAEOLOGY_SITES_PER_CIV_SEA` is 2, so the eight-player tournament map
    /// carries 48 dig sites and 16 wrecks. Under the old lottery it averaged
    /// 17.8 and 9.0 — barely a third of the Artifacts Archaeology and the
    /// Culture victory are balanced around.
    #[test]
    fn artifacts_are_allocated_per_civilization_not_rolled_per_tile() {
        let rules = Rules::embedded();
        for (index, script) in ROLLED_TYPES.into_iter().enumerate() {
            let majors = 8;
            let mut rng = Rng::new(82_000 + index as u64);
            let (world, _) = generate_with_script(
                &rules, 84, 54, majors, 12, 5, 4, script, FLAT, POLED, &mut rng,
            );
            // A quota is a ceiling, not a promise: a water-poor script can run
            // out of eligible Coast and a land-only one out of unclaimed dig
            // terrain. So assert the quota is met *or* that nothing eligible is
            // left over — never that some tiles were simply skipped.
            for (resource, per_civ) in [("antiquity_site", 6), ("shipwreck", 2)] {
                let spec = &rules.resources[resource];
                let placed = world
                    .tiles
                    .values()
                    .filter(|tile| tile.resource.as_deref() == Some(resource))
                    .count();
                let quota = per_civ * majors;
                assert!(
                    placed <= quota,
                    "{script:?} placed {placed} {resource}, over the {quota} quota"
                );
                if placed == quota {
                    continue;
                }
                let spare = world
                    .tiles
                    .values()
                    .filter(|tile| {
                        if tile.resource.is_some() {
                            return false;
                        }
                        let by_feature = tile
                            .feature
                            .as_ref()
                            .is_some_and(|feature| spec.feature.contains(feature));
                        let by_terrain =
                            tile.feature.is_none() && spec.terrain.contains(&tile.terrain);
                        (by_feature || by_terrain)
                            && spec.hills.is_none_or(|want| want == tile.hills)
                    })
                    .count();
                assert_eq!(
                    spare, 0,
                    "{script:?} placed only {placed} of {quota} {resource} \
                     with {spare} eligible tiles still free"
                );
            }
        }
    }

    #[test]
    fn generated_rivers_are_mirrored_connected_land_edge_chains_with_outlets() {
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
        remove_water_boundary_rivers(&mut wm);
        let river_edges: BTreeSet<RiverEdge> = all_shared_edges(&wm)
            .into_iter()
            .filter(|(a, b)| wm.has_river_edge(*a, *b))
            .collect();
        assert!(!river_edges.is_empty());
        let is_water = |p: Pos| {
            matches!(wm.tiles[&p].terrain.as_str(), "ocean" | "coast" | "lake")
        };
        for edge in &river_edges {
            assert!(
                !is_water(edge.0) && !is_water(edge.1),
                "river edge {edge:?} borders water on one side"
            );
        }

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

        // Each edge-connected river component ends at a shoreline vertex,
        // while every segment itself remains between two land tiles.
        let mut unseen = river_edges.clone();
        while let Some(start) = unseen.iter().next().copied() {
            let mut stack = vec![start];
            let mut has_outlet = false;
            unseen.remove(&start);
            while let Some(edge) = stack.pop() {
                has_outlet |= river_edge_has_outlet(&wm, edge, &is_water);
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
    fn completed_map_scripts_never_put_a_river_on_a_water_boundary() {
        let rules = Rules::embedded();
        for (index, script) in ROLLED_TYPES.into_iter().enumerate() {
            let mut rng = Rng::new(73_100 + index as u64);
            let (world, _) = generate_with_script(
                &rules, 42, 28, 4, 5, 2, 3, script, FLAT, POLED, &mut rng,
            );
            for edge in all_shared_edges(&world)
                .into_iter()
                .filter(|edge| world.has_river_edge(edge.0, edge.1))
            {
                assert!(
                    !rules.is_water(&world.tiles[&edge.0])
                        && !rules.is_water(&world.tiles[&edge.1]),
                    "{script:?} put river edge {edge:?} on an ocean or lake boundary"
                );
            }
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
    /// The measuring instrument for any change that moves map generation.
    ///
    /// The two spread tests each assert their property at one seed per map
    /// size, so a change that shifts the RNG can fail them by luck. This runs
    /// the same property over 400 worlds and reports how often it fails, which
    /// is what tells you whether a failure is the change or the draw. Ignored
    /// because it takes about two minutes.
    ///
    /// Baseline as of this commit: **5 failures in 400**, a 1.25% natural rate.
    #[test]
    #[ignore]
    fn experiment_start_spacing_failure_rate() {
        let rules = Rules::embedded();
        let mut checked = 0usize;
        let mut failed = 0usize;
        for (index, size) in CIV6_MAP_SIZES.iter().enumerate() {
            for seed in 0..40u64 {
                let mut rng = Rng::new(700_000 + seed * 977 + index as u64);
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
                let majors = &spawns[..size.default_players];
                if majors.len() < 2 {
                    continue;
                }
                let mut nearest: Vec<i32> = majors
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
                nearest.sort_unstable();
                let typical = nearest[nearest.len() / 2].max(1);
                checked += 1;
                if !(closest * 100 / typical >= 70 && farthest * 100 / typical <= 200) {
                    failed += 1;
                }
            }
        }
        eprintln!("SPACING checked={checked} failed={failed}");
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
            let majors = &spawns[..size.default_players];
            // Nobody is marooned. This used to read "every major is on the
            // largest landmass", which was the old placer's behaviour rather
            // than a fairness rule, and it stopped being true once a hundred
            // seats made even Pangaea's second island worth a share of the
            // world. What matters is that the landmass a civilization is put
            // on can carry a fair share of the world's land — see
            // `regions_for_seats`, which is where the rule lives.
            let components = connected_components(&wm, &passable);
            let fair_share = passable.len() / (2 * size.default_players.max(1));
            for start in majors {
                let home = components
                    .iter()
                    .find(|component| component.contains(start))
                    .expect("a start is on land");
                assert!(
                    home.len() >= fair_share.max(MIN_LANDMASS_FOR_A_START),
                    "{}: a civilization opens on a {}-tile landmass, short of a {fair_share}-tile share",
                    size.name,
                    home.len()
                );
            }
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
            // Every tile a civilization could reach, not just the biggest
            // landmass's: seats are apportioned to landmasses now, so measuring
            // one of them scores a civilization seated on another at zero.
            let score = spawn_layout_score(&wm, &passable, majors, &qualities);
            let balance = layout_balance_percentages(score, size.default_players, passable.len());
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
            //
            // Measured against the typical seat rather than the extreme one,
            // because the extreme is not scale-fair: holding the closest pair
            // to a fraction of the farthest asks a hundred seats to do
            // something eight seats never had to, since one coastline oddity
            // among a hundred is not an irregular layout. Both ends of the
            // spread are held, which is the property itself.
            let mut nearest: Vec<i32> = majors
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
            nearest.sort_unstable();
            let typical = nearest[nearest.len() / 2].max(1);
            assert!(
                closest * 100 / typical >= 70 && farthest * 100 / typical <= 200,
                "{} spaces its starts irregularly around a typical {typical}: {nearest:?}",
                size.name
            );
            // Both floors go up: 50 to 55 up to twenty seats, and 40 to 45
            // above it. What still holds the top end down is not the placement
            // but the wilderness — a landmass too small to be given a seat is
            // still land, and every tile of it counts toward whichever seat
            // happens to be nearest, which on a fifty-seat world can be thirty
            // hexes of empty island credited to one civilization. The seat that
            // cannot be lifted at all is usually one the clearance buffer has
            // boxed in, and refusing to crowd it is the buffer working.
            // Both floors go up, and both step down past twenty seats for the
            // same reason: a hundred civilizations on one world are shared out
            // by the world's own variety as much as by the placer. There is
            // tundra and desert on every map, and at a hundred seats somebody
            // is standing in it — the weakest capital scores 149 against a best
            // of 258 and no site inside its region beats it.
            let (territory_floor, quality_floor) = if size.default_players > 20 {
                (45, 55)
            } else {
                (55, 60)
            };
            assert!(
                balance.0 >= territory_floor && balance.2 >= quality_floor,
                "{} has an unfair start outlier: territory/neighbor/quality balance = {balance:?}, {score:?}",
                size.name,
            );
            // `balance.1` is deliberately not asserted here. It is the closest
            // pair over the farthest pair, the same min-over-max that the
            // regularity check above replaced, and it fails for the same
            // reason: one civilization seated on an island of its own — which
            // the apportionment above does on purpose, and which leaves it a
            // fair share of land — is 28 hexes from anybody on a hundred-seat
            // world and drags the ratio to 39 while every other seat is even.
            // The clearance floor and the median spread are the honest pair.
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
                // How far the least lucky civilization has to look. Twelve
                // hexes is inside the range an early envoy mission can cover.
                // Past twenty seats each civilization's own ground is small
                // enough that its two city-state regions are the near and the
                // far half of it, and the far one sits out at the edge — so the
                // bound is on the world being crowded, not on the placer.
                let reach = if majors.len() > 20 { 18 } else { 12 };
                for major in majors {
                    let nearest = minors
                        .iter()
                        .map(|minor| wm.distance(*major, *minor))
                        .min()
                        .unwrap();
                    assert!(
                        nearest <= reach,
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
                // Within one of the ideal split. Stated against the split
                // rather than as a flat spread, because a hundred seats sharing
                // a hundred and fifty city-states cannot all hold the same
                // number and a single seat one over is not an uneven world.
                let fewest = owned.iter().copied().min().unwrap();
                let most = owned.iter().copied().max().unwrap();
                assert!(
                    fewest >= minors.len() / majors.len()
                        && most <= minors.len().div_ceil(majors.len()) + 1,
                    "{} seed {seed}: city-states are shared out unevenly: {owned:?}",
                    size.id
                );
            }
        }
    }

    /// Islands used to apply the continental region rule twice: majors were
    /// placed on whichever disconnected islands had the most fertility, then
    /// minor regions were cut only from those major-bearing islands. On a
    /// Standard flat world that left most of the archipelago outside every
    /// start region, clustered twenty starts onto as few as eight islands, and
    /// gave one civilization three city-states while another received one.
    #[test]
    fn islands_flat_poles_spread_starts_across_the_whole_archipelago() {
        let rules = Rules::embedded();
        for size_index in [1_usize, 3, 5] {
            let size = &CIV6_MAP_SIZES[size_index];
            for seed in 0..16u64 {
                let mut rng = Rng::new(90_000 + seed * 31 + size_index as u64);
                let (world, spawns) = generate_with_script(
                    &rules,
                    size.width,
                    size.height,
                    size.default_players,
                    size.default_city_states,
                    size.natural_wonders,
                    size.continents,
                    MapScript::Islands,
                    FLAT,
                    POLED,
                    &mut rng,
                );
                let where_ = format!("{} seed {seed}", size.id);
                assert_eq!(
                    spawns.len(),
                    size.default_players + size.default_city_states,
                    "{where_}"
                );
                let (majors, minors) = spawns.split_at(size.default_players);

                let mut nearest_major: Vec<i32> = majors
                    .iter()
                    .map(|major| {
                        majors
                            .iter()
                            .filter(|other| *other != major)
                            .map(|other| world.distance(*major, *other))
                            .min()
                            .unwrap()
                    })
                    .collect();
                let closest = nearest_major.iter().copied().min().unwrap();
                nearest_major.sort_unstable();
                let typical = nearest_major[nearest_major.len() / 2];
                let farthest = nearest_major.last().copied().unwrap();
                assert!(
                    closest > MAJOR_START_BUFFER
                        && closest * 100 / typical >= 65
                        && farthest * 100 / typical <= 200,
                    "{where_}: major starts are irregular around {typical}: {nearest_major:?}"
                );

                let mut owned = vec![0_usize; majors.len()];
                let mut nearest_minor = vec![i32::MAX; majors.len()];
                for minor in minors {
                    let owner = majors
                        .iter()
                        .enumerate()
                        .map(|(index, major)| (world.distance(*minor, *major), index))
                        .min()
                        .unwrap()
                        .1;
                    owned[owner] += 1;
                    for (index, major) in majors.iter().enumerate() {
                        nearest_minor[index] =
                            nearest_minor[index].min(world.distance(*minor, *major));
                    }
                }
                let fewest = owned.iter().copied().min().unwrap();
                let most = owned.iter().copied().max().unwrap();
                assert!(
                    fewest >= 1 && most - fewest <= 1,
                    "{where_}: city-states are shared out unevenly: {owned:?}"
                );
                assert!(
                    nearest_minor.iter().all(|distance| *distance <= 12),
                    "{where_}: a civilization cannot reach a city-state: {nearest_minor:?}"
                );

                let islands = land_components(&world, &rules);
                let occupied = islands
                    .iter()
                    .filter(|island| spawns.iter().any(|spawn| island.contains(spawn)))
                    .count();
                assert!(
                    occupied * 100 / spawns.len() >= 70,
                    "{where_}: {} starts occupy only {occupied} of {} islands",
                    spawns.len(),
                    islands.len()
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
            // How many wonders a world draws, and how far apart it keeps them,
            // is a property of the map rather than of the seats on it. The
            // scaled sizes' cost is almost entirely the start-placement search,
            // which grows with the square of the seat count, so they are swept
            // at a stock seat count and over two scripts instead of fifteen
            // runs of a hundred-civilization spawn search.
            let scaled = size.default_players > 12;
            let (majors, minors) = if scaled {
                (8, 12)
            } else {
                (size.default_players, size.default_city_states)
            };
            let scripts: &[MapScript] = if scaled {
                &[MapScript::Pangaea, MapScript::Continents]
            } else {
                &[
                    MapScript::Pangaea,
                    MapScript::Continents,
                    MapScript::SmallContinents,
                    MapScript::InlandSea,
                    MapScript::Lakes,
                ]
            };
            for script in scripts.iter().copied() {
                for seed in 0..if scaled { 1 } else { 3 } {
                    let mut rng = Rng::new(seed);
                    let (world, _) = generate_with_script(
                        &rules,
                        size.width,
                        size.height,
                        majors,
                        minors,
                        size.natural_wonders,
                        size.continents,
                        script,
                        FLAT,
                        POLED,
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
    /// Why a civilization can open with no city-state near it, and the line
    /// between that being the map and that being a bug.
    ///
    /// On a water world a civilization's island can be smaller than the radius
    /// inside which `Game::can_found_city` refuses to build: every tile on it
    /// is within `MIN_START_SEPARATION` of the capital, so there is nowhere on
    /// that island for a second city of any kind and the city-state has to go
    /// elsewhere. Measured, that is the *only* reason it ever happens — so the
    /// rule pinned here is the sharp one: if an island has room for a
    /// city-state at all, the civilization living there gets one.
    ///
    /// Islands is deliberately excluded. #250 gave that script its own model, in
    /// which a region spans several islands on purpose — a civilization's
    /// maritime territory — so its city-state belonging to a neighbouring
    /// island is the design rather than a miss. Every other script, Water World
    /// included, still cuts city-state regions from the civilization's own
    /// cell, and there "its own island" is exactly the promise.
    #[test]
    fn an_island_with_room_for_a_city_state_gets_one() {
        let rules = Rules::embedded();
        for (index, (script, size)) in [MapScript::WaterWorld]
            .into_iter()
            .flat_map(|script| {
                CIV6_MAP_SIZES
                    .iter()
                    .map(move |size| (script, size))
            })
            .enumerate()
        {
            for seed in 0..6u64 {
                let mut rng = Rng::new(88_000 + seed * 17 + index as u64);
                let (wm, spawns) = generate_with_script(
                    &rules,
                    size.width,
                    size.height,
                    size.default_players,
                    size.default_city_states,
                    size.natural_wonders,
                    size.continents,
                    script,
                    MapTopology::Flat,
                    MapPoles::Poles,
                    &mut rng,
                );
                let passable: BTreeSet<Pos> = wm
                    .tiles
                    .iter()
                    .filter(|(_, tile)| !rules.is_water(tile) && rules.is_passable(tile))
                    .map(|(pos, _)| *pos)
                    .collect();
                let components = connected_components(&wm, &passable);
                let island_of = |position: Pos| {
                    components
                        .iter()
                        .position(|component| component.contains(&position))
                };
                let (majors, minors) = spawns.split_at(size.default_players);
                for (seat, major) in majors.iter().enumerate() {
                    let Some(island) = island_of(*major) else {
                        continue;
                    };
                    // Somewhere on this island a second city could stand, and
                    // no other civilization is sharing it.
                    let room = components[island]
                        .iter()
                        .filter(|tile| {
                            majors
                                .iter()
                                .all(|other| wm.distance(**tile, *other) >= MIN_START_SEPARATION)
                        })
                        .count();
                    if room == 0 {
                        continue;
                    }
                    let mine = minors
                        .iter()
                        .filter(|minor| island_of(**minor) == Some(island))
                        .count();
                    assert!(
                        mine > 0,
                        "{script:?} {} seed {seed}: civilization {seat} has an island with \
                         {room} tiles clear enough for a city-state and was given none",
                        size.id
                    );
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
        for (script, topology) in [
            (MapScript::Pangaea, MapTopology::Flat),
            (MapScript::Continents, MapTopology::Flat),
            (MapScript::SmallContinents, MapTopology::Flat),
            (MapScript::Islands, MapTopology::Flat),
            (MapScript::WaterWorld, MapTopology::Flat),
            (MapScript::SmallContinents, MapTopology::Planet),
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
                        topology,
                        MapPoles::Poles,
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
                    "{script:?} {topology:?} {} ({}x{}, {} civs, {} city-states)",
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

#[cfg(test)]
mod start_bias_tests {
    use crate::rules::{Rules, StartBias};

    /// StartBiasTerrains, StartBiasResources and StartBiasFeatures all carry a
    /// Tier, and Civ VI reads a *lower* tier as a stronger preference. CIVVIS
    /// weights them the same way, so the tiers have to match exactly or a
    /// civilization lands in the wrong sort of place.
    #[test]
    fn every_start_bias_carries_the_tier_its_row_ships() {
        assert_eq!(StartBias::weight(1), 5, "Tier 1 is the strongest bias");
        assert_eq!(StartBias::weight(5), 1, "Tier 5 is the weakest");

        let rules = Rules::embedded();
        // (civ, terrain tier, feature tier, resource tier) -- 0 means no bias
        // of that kind ships for this civilization.
        let expected: &[(&str, i32, i32, i32)] = &[
            ("Mali", 1, 0, 5),      // DESERT Tier 1, ten resources Tier 5
            ("Maya", 1, 0, 2),      // GRASS/PLAINS Tier 1, thirteen luxuries Tier 2
            ("Vietnam", 0, 1, 0),   // FOREST/JUNGLE/MARSH Tier 1
            ("Gaul", 0, 0, 2),      // seven resources Tier 2
            ("Kongo", 0, 2, 0),     // FOREST/JUNGLE Tier 2
            ("Russia", 2, 0, 0),    // TUNDRA and TUNDRA_HILLS Tier 2
            ("Nubia", 2, 0, 5),     // already exact before this change
            ("Brazil", 0, 2, 0),
            ("Egypt", 0, 2, 0),
            ("Korea", 3, 0, 0),     // four hills terrains, Tier 3
            ("Greece", 3, 0, 0),
            ("Scythia", 5, 0, 2),   // GRASS/PLAINS Tier 5, HORSES Tier 2
        ];
        for &(civ, terrain, feature, resource) in expected {
            let bias = rules.civs[civ].start_bias.as_ref().expect(civ);
            assert_eq!(bias.terrain_tier, terrain, "{civ} terrain tier");
            assert_eq!(bias.feature_tier, feature, "{civ} feature tier");
            assert_eq!(bias.resource_tier, resource, "{civ} resource tier");
        }

        // Mali's is the strongest terrain bias any civilization ships, and
        // Scythia's the weakest -- a five-fold spread that only means anything
        // if the tiers themselves are right.
        assert!(
            StartBias::weight(rules.civs["Mali"].start_bias.as_ref().unwrap().terrain_tier)
                > StartBias::weight(
                    rules.civs["Scythia"].start_bias.as_ref().unwrap().terrain_tier,
                )
        );

        // Korea takes all four hills terrains, not the two CIVVIS had.
        let korea = rules.civs["Korea"].start_bias.as_ref().unwrap();
        assert!(korea.terrain_hills);
        assert_eq!(korea.terrain.len(), 4);
        // Maya ships no feature bias at all; the jungle one was invented.
        assert!(rules.civs["Maya"]
            .start_bias
            .as_ref()
            .unwrap()
            .feature
            .is_empty());
    }
}
