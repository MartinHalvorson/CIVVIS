//! Map generation (mirrors civvis/mapgen.py).
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};
use std::sync::Arc;

use crate::fractal::Fractal;
use crate::leader_roster::{self, TrueStartPoint};
use crate::name::Name;
use crate::rng::Rng;
use crate::rules::{volcanic_soil_valid_terrain, Rules};
use crate::setup::{MapPoles, MapScript, MapTopology};
use crate::sphere::trig;
use crate::world::WorldMap;
use crate::{hex, Pos};

mod canals;
mod lakes;
mod rivers;
use canals::*;
use lakes::*;
use rivers::*;

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
        let candidates: Vec<Pos> = wm
            .neighbors(current)
            .into_iter()
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

/// Earth's real surface, on a half-degree grid.
///
/// `data/earth_surface.txt` is 720 by 360 cells built by
/// `tools/earth_surface.py` out of Natural Earth's 1:50m coastlines and lakes,
/// SRTM15+ elevation and the Koeppen-Geiger climate classification. Every cell
/// carries what is actually there: sea, lake or one of the land terrains, plus
/// whether it stands high enough to be hills or a mountain and what grows on
/// it. Nothing about this world is rolled, which is the whole point of a
/// true-start map — the seed still decides the rivers, the resources and the
/// scatter inside each biome, and never the geography.
///
/// Half a degree is chosen against the finest world the engine builds:
/// Ludicrous averages about 1.6 degrees to a tile, so the source out-resolves
/// even that by three to one, and anything finer would be bytes no sampler
/// could see.
const EARTH_SURFACE: &str = include_str!("../data/earth_surface.txt");
const EARTH_GRID_WIDTH: usize = 720;
const EARTH_GRID_HEIGHT: usize = 360;
const EARTH_CELL_DEGREES: f64 = 0.5;

/// The surface classes a cell can hold, in the order the asset encodes them.
const EARTH_SEA: u8 = 0;
const EARTH_LAKE: u8 = 1;
const EARTH_MOUNTAIN: u8 = 7;
/// The terrain each land class names, indexed by the class itself. Sea and
/// lake are water and never read from here.
const EARTH_TERRAIN: [&str; 8] = [
    "ocean",
    "lake",
    "grassland",
    "plains",
    "desert",
    "tundra",
    "snow",
    "mountain",
];
/// What grows on a cell, indexed by its vegetation field.
const EARTH_VEGETATION: [Option<&str>; 4] = [None, Some("forest"), Some("jungle"), Some("marsh")];

/// The decoded grid, one packed byte per cell, row 0 running east from 180W
/// along 90N..89.5N.
fn earth_grid() -> &'static [u8] {
    static GRID: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    GRID.get_or_init(|| {
        let mut cells = Vec::with_capacity(EARTH_GRID_WIDTH * EARTH_GRID_HEIGHT);
        for line in EARTH_SURFACE.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            for token in line.split_whitespace() {
                let (count, value) = token
                    .split_once(':')
                    .expect("earth_surface.txt run is `count:value`");
                let count = usize::from_str_radix(count, 36).expect("run length is base 36");
                let value = u8::from_str_radix(value, 36).expect("cell value is base 36");
                cells.extend(std::iter::repeat_n(value, count));
            }
        }
        assert_eq!(
            cells.len(),
            EARTH_GRID_WIDTH * EARTH_GRID_HEIGHT,
            "earth_surface.txt must decode to the full grid"
        );
        cells
    })
}

/// One cell of the grid: three fields packed into a byte.
#[derive(Clone, Copy, PartialEq, Eq)]
struct EarthCell(u8);

impl EarthCell {
    fn surface(self) -> u8 {
        self.0 & 0b111
    }
    fn hills(self) -> bool {
        self.0 & 0b1000 != 0
    }
    fn vegetation(self) -> usize {
        ((self.0 >> 4) & 0b11) as usize
    }
    fn is_land(self) -> bool {
        !matches!(self.surface(), EARTH_SEA | EARTH_LAKE)
    }
}

/// The cell a point in degrees falls in. Longitude wraps; latitude clamps,
/// because there is nothing past a pole to read.
fn earth_cell(longitude: f64, latitude: f64) -> EarthCell {
    let column = ((longitude + 180.0) / EARTH_CELL_DEGREES)
        .floor()
        .rem_euclid(EARTH_GRID_WIDTH as f64) as usize;
    let row = (((90.0 - latitude) / EARTH_CELL_DEGREES).floor() as i64)
        .clamp(0, EARTH_GRID_HEIGHT as i64 - 1) as usize;
    EarthCell(earth_grid()[row * EARTH_GRID_WIDTH + column])
}

/// How much of the world one tile covers, as half-widths in degrees at the
/// equator.
///
/// A globe's tiles are equal-area, so the one number that describes them is
/// the angular radius of a cap holding a tile's share of the sphere: from
/// `2*pi*(1 - cos r) = 4*pi / tiles`, `r` is very nearly `2 / sqrt(tiles)`.
/// Away from the equator that cap spans more longitude than it does latitude,
/// which [`earth_tile`] corrects for. A flat map has no such stretch — its
/// columns are meridians spread evenly whatever the row — so its span is read
/// straight off the rectangle.
fn earth_tile_span(wm: &WorldMap) -> (f64, f64) {
    if wm.sphere().is_some() {
        let radius = (4.0 / wm.tiles.len().max(1) as f64).sqrt().to_degrees();
        (radius, radius)
    } else {
        (
            180.0 / wm.width.max(1) as f64,
            90.0 / (wm.height - 1).max(1) as f64,
        )
    }
}

/// What Earth puts under one tile.
#[derive(Clone, Copy)]
struct EarthTile {
    /// Water of some kind, so not part of the world's land.
    water: bool,
    terrain: &'static str,
    /// The climate underneath a mountain tile. Civ VI keeps this distinction
    /// in `TERRAIN_*_MOUNTAIN` even though CIVVIS renders every peak with the
    /// common `mountain` terrain.
    mountain_terrain: Option<&'static str>,
    hills: bool,
    vegetation: Option<&'static str>,
}

/// The share of a tile's land that has to be mountain before the tile is one,
/// and the share that has to be raised at all before it is hills.
///
/// A majority would be wrong for both. Ranges are narrow — the Alps are two or
/// three cells across where a Standard tile is seven — so a tile that is a
/// third mountain is a tile the range runs through, and demanding half of it
/// would flatten every range on the map into foothills.
const EARTH_MOUNTAIN_SHARE: usize = 30;
const EARTH_HILL_SHARE: usize = 40;
/// The share of a tile's land one plant has to cover before it grows there.
const EARTH_VEGETATION_SHARE: usize = 40;

/// Earth, under one tile, decided by every grid cell the tile covers.
///
/// A tile is far wider than a cell at every size the engine plays, so reading
/// the single cell under its centre would be a coin toss on every coastline
/// and would drop whole ranges between samples. Sampling the tile's own
/// footprint and letting the cells vote is what makes the same silhouette come
/// out right on a 1,144-tile Duel world and a 58,000-tile Ludicrous one.
fn earth_tile(wm: &WorldMap, pos: Pos) -> EarthTile {
    let (longitude, latitude) = wm.lon_lat(pos);
    let (span_longitude, span_latitude) = earth_tile_span(wm);
    let stretch = if wm.sphere().is_some() {
        1.0 / trig::cos(latitude.to_radians()).abs().max(0.02)
    } else {
        1.0
    };
    let span_longitude = (span_longitude * stretch).min(180.0);

    let steps = |span: f64| ((2.0 * span / EARTH_CELL_DEGREES).round() as usize).clamp(1, 9);
    let (columns, rows) = (steps(span_longitude), steps(span_latitude));

    let mut surfaces = [0usize; 8];
    let mut vegetation = [0usize; 4];
    let (mut raised, mut total) = (0usize, 0usize);
    for row in 0..rows {
        // Sample centres, so a one-sample axis reads the tile's own middle.
        let offset = |index: usize, count: usize, span: f64| {
            if count == 1 {
                0.0
            } else {
                span * (2.0 * index as f64 / (count - 1) as f64 - 1.0)
            }
        };
        let sample_latitude = (latitude + offset(row, rows, span_latitude)).clamp(-89.999, 89.999);
        for column in 0..columns {
            let cell = earth_cell(
                longitude + offset(column, columns, span_longitude),
                sample_latitude,
            );
            surfaces[cell.surface() as usize] += 1;
            if cell.is_land() {
                vegetation[cell.vegetation()] += 1;
                if cell.hills() {
                    raised += 1;
                }
            }
            total += 1;
        }
    }

    let land = total - surfaces[EARTH_SEA as usize] - surfaces[EARTH_LAKE as usize];
    if land * 2 < total {
        // Enclosed water is still water here; `classify_lakes` sorts every
        // body the coastline encloses into lakes and inland seas by area,
        // and it does that for a read coastline exactly as for a grown one.
        return EarthTile {
            water: true,
            terrain: "ocean",
            mountain_terrain: None,
            hills: false,
            vegetation: None,
        };
    }

    let peaks = surfaces[EARTH_MOUNTAIN as usize];
    let mountain = peaks * 100 >= land * EARTH_MOUNTAIN_SHARE;
    let hills = !mountain && (raised + peaks) * 100 >= land * EARTH_HILL_SHARE;
    // The terrain is whichever land class holds most of the tile. Mountain is
    // decided by its own share above, so it is not a candidate here, and a
    // tile that is nothing but peaks keeps the grassland default it will never
    // show.
    let terrain = (2..EARTH_MOUNTAIN as usize)
        .max_by_key(|class| surfaces[*class])
        .filter(|class| surfaces[*class] > 0)
        .map_or("grassland", |class| EARTH_TERRAIN[class]);
    let plant = (1..EARTH_VEGETATION.len())
        .max_by_key(|kind| vegetation[*kind])
        .filter(|kind| !mountain && vegetation[*kind] * 100 >= land * EARTH_VEGETATION_SHARE)
        .and_then(|kind| EARTH_VEGETATION[kind]);
    EarthTile {
        water: false,
        terrain: if mountain { "mountain" } else { terrain },
        mountain_terrain: mountain.then_some(terrain),
        hills,
        vegetation: plant,
    }
}

/// Islands a tile-wide vote would lose, and that a map of Earth should not be
/// without, each with its area in thousands of square kilometres.
///
/// Every one of them is smaller than a tile at some map size, so the cells
/// carrying them are outvoted by the sea around them. Each is seated on the
/// single tile nearest it, provided the map is fine enough to be worth a tile
/// of — see [`earth_tile_area`]. Most of the list is consequential in play: a
/// civilization begins on Britain, Japan, Java, Luzon, Sri Lanka, Madagascar
/// and New Zealand, and the rest are the stepping stones that decide whether
/// an ocean can be crossed at all.
/// Ways through a range that a tile-wide vote loses, and that a map of Earth
/// should not be without.
///
/// The mirror of [`EARTH_ISLANDS`] one rank up. An island is a piece of land
/// the sea around it outvotes; these are the corridors the range around them
/// outvotes. A Standard globe gives Asia a tile about four degrees across,
/// and the gap between the Pamir and the Taklamakan is nowhere near four
/// degrees wide — so Kashgar, whose own half-degree cell is desert at 1,460 m
/// with 402 m of relief inside it, lands in a tile that is mostly Pamir and
/// comes out as impassable rock. Measured before this list: Samarkand could
/// not reach Kashgar at Standard or Large by any route, and Kashgar could not
/// be stood on at all.
///
/// Every entry is a real corridor that carried real traffic, named for what
/// it is. A pass only ever demotes a tile the vote made a mountain, to the
/// climate underneath it plus hills — so on a map fine enough to resolve the
/// corridor by itself, each of these is a no-op, and none of them can put a
/// hole in a range the vote did not already misplace.
///
/// ⚠ This is a curated list and it is meant to stay short. It is not a place
/// to soften mountains that are genuinely in the way: the Karakoram between
/// Kashgar and Ladakh stays shut, and a test says so.
const EARTH_PASSES: &[(f64, f64, &str)] = &[
    (67.9, 40.1, "the Sanzar gorge, the Gates of Tamerlane"),
    (71.8, 40.4, "the Ferghana valley"),
    (73.5, 39.6, "the Alai valley and Irkeshtam"),
    (76.0, 39.5, "Kashgar and the Terek pass"),
    (79.9, 37.1, "Khotan, on the southern Tarim road"),
];

const EARTH_ISLANDS: &[(f64, f64, f64)] = &[
    (-4.0, 54.0, 209.3),   // Britain
    (-8.0, 53.3, 84.4),    // Ireland
    (-19.0, 64.9, 103.0),  // Iceland
    (-7.0, 62.0, 1.4),     // the Faroes
    (-25.7, 37.8, 2.3),    // the Azores
    (-15.6, 28.1, 7.5),    // the Canaries
    (-23.6, 15.1, 4.0),    // Cape Verde
    (14.3, 37.6, 25.7),    // Sicily
    (9.0, 40.1, 24.1),     // Sardinia
    (25.0, 35.3, 8.3),     // Crete
    (33.3, 35.1, 9.3),     // Cyprus
    (28.2, 36.4, 1.4),     // Rhodes
    (-77.0, 21.5, 105.8),  // Cuba
    (-71.0, 19.0, 76.2),   // Hispaniola
    (-66.5, 18.2, 8.9),    // Puerto Rico
    (-61.0, 13.5, 14.0),   // the Lesser Antilles
    (-90.4, -0.6, 7.9),    // the Galapagos
    (-109.4, -27.1, 0.2),  // Rapa Nui
    (-149.5, -17.6, 1.0),  // Tahiti
    (-171.8, -13.8, 2.8),  // Samoa
    (178.4, -17.8, 18.3),  // Fiji
    (166.5, -21.5, 18.6),  // New Caledonia
    (168.0, -16.5, 12.2),  // Vanuatu
    (159.9, -9.4, 28.4),   // the Solomons
    (150.5, -5.5, 49.7),   // the Bismarcks
    (-157.9, 21.3, 28.3),  // Hawaii
    (145.7, 15.2, 1.0),    // the Marianas
    (134.5, 7.5, 0.5),     // Palau
    (168.7, 7.1, 0.2),     // the Marshalls
    (172.9, 1.4, 0.8),     // Kiribati
    (121.0, 23.7, 36.2),   // Taiwan
    (127.8, 26.3, 2.3),    // Okinawa
    (139.5, 36.5, 228.0),  // Japan
    (142.5, 43.5, 83.4),   // Hokkaido
    (124.0, 11.0, 56.0),   // the Visayas
    (121.0, 15.5, 110.0),  // Luzon
    (110.0, -7.3, 138.8),  // Java
    (115.2, -8.4, 5.8),    // Bali
    (101.5, 0.0, 473.5),   // Sumatra
    (80.7, 7.9, 65.6),     // Sri Lanka
    (73.0, 4.2, 0.3),      // the Maldives
    (55.5, -4.6, 0.5),     // the Seychelles
    (57.5, -20.3, 2.0),    // Mauritius
    (46.5, -19.0, 587.0),  // Madagascar
    (43.3, -11.7, 1.9),    // the Comoros
    (39.3, -6.1, 2.5),     // Zanzibar
    (50.6, 26.0, 0.8),     // Bahrain
    (175.5, -38.5, 113.7), // New Zealand, north
    (170.5, -44.0, 150.4), // New Zealand, south
    (146.8, -42.0, 68.4),  // Tasmania
    (-59.0, -51.7, 12.2),  // the Falklands
    (-73.8, -42.6, 8.4),   // Chiloe
    (-55.5, 48.5, 108.9),  // Newfoundland
    (-63.0, 45.0, 55.3),   // Nova Scotia
    (-132.3, 53.2, 10.2),  // Haida Gwaii
    (-134.5, 57.0, 36.3),  // the Alexander Archipelago
];

/// Sovereign atolls whose physical land is too small for the half-degree
/// source grid to retain, even on the largest playable Earth.  They are only
/// promoted to a tile where that tile represents at most 9,000 square
/// kilometres — the Ludicrous 100-civilization world and any finer successor.
///
/// This is deliberately a physical-map supplement, not a claim about a
/// country's borders or affiliation.  It gives a modern-world map a real
/// place to seat the atoll rather than silently moving it hundreds of miles to
/// another country's coast.  Smaller worlds omit them because turning every
/// atoll into a Duel-sized hex would be less accurate than leaving it below
/// that world's resolution.
const EARTH_MICROSTATE_ISLANDS: &[(f64, f64)] = &[
    (73.2, 3.2),     // Maldives
    (-175.1, -21.1), // Tonga
    (55.4, -4.6),    // Seychelles
    (171.1, 7.1),    // Marshall Islands
    (150.5, 7.4),    // Micronesia
    (134.5, 7.5),    // Palau
    (-168.7, -3.3),  // Kiribati
    (-61.7, 17.0),   // Antigua and Barbuda
    (-62.7, 17.3),   // Saint Kitts and Nevis
    (166.9, -0.5),   // Nauru
    (177.6, -7.1),   // Tuvalu
];

/// The highest stock map density is 8.8 thousand square kilometres per tile.
/// At that scale an atoll earns one strategic tile; at lower densities it
/// would become a visibly fictional landmass.
const MICROSTATE_ISLAND_MAX_TILE_AREA: f64 = 9.0;

/// The inland waters a tile-wide vote would drain, each with its area in
/// thousands of square kilometres.
///
/// These are the bodies that decide something: fresh water for the cities on
/// them, a Harbor a landlocked civilization would otherwise never build, and a
/// barrier armies have to go around. The Caspian is given two points because
/// it is long enough that one would seat only half of it, and Chad and the
/// Aral are given the extent they had before the twentieth century drained
/// them, which is the Earth this map is of.
///
/// The area gate matters more here than it does for an island, and is set
/// twice as tight. Seating Hawaii is the coarsest true thing a map at this
/// resolution can say about that tile — there really is land in it. Draining
/// the tile that holds Lake Erie on a Duel world is not: that tile is
/// overwhelmingly Ontario, and calling it water would be a plain error rather
/// than a rounding of one.
const EARTH_LAKES: &[(f64, f64, f64)] = &[
    (51.0, 41.5, 371.0),  // the Caspian, southern basin
    (50.5, 45.5, 371.0),  // the Caspian, northern basin
    (59.5, 45.0, 68.0),   // the Aral, at its 1960 extent
    (108.0, 53.5, 31.7),  // Baikal
    (74.5, 46.3, 16.4),   // Balkhash
    (31.5, 61.0, 17.7),   // Ladoga
    (-87.5, 47.7, 82.1),  // Superior
    (-87.0, 44.0, 58.0),  // Michigan
    (-82.2, 44.8, 59.6),  // Huron
    (-79.5, 43.0, 25.7),  // Erie and Ontario
    (-97.5, 52.5, 24.5),  // Winnipeg
    (-110.0, 59.3, 7.9),  // Athabasca
    (-114.0, 61.5, 27.2), // Great Slave
    (-121.0, 66.0, 31.0), // Great Bear
    (33.0, -1.0, 68.8),   // Victoria
    (29.6, -6.0, 32.9),   // Tanganyika
    (34.5, -12.0, 29.6),  // Malawi
    (14.3, 13.2, 25.0),   // Chad, at the extent it held into the 1960s
    (-69.3, -15.8, 8.4),  // Titicaca
    (-71.5, 9.8, 13.2),   // Maracaibo
];

/// The unit vector a longitude and latitude in degrees point at.
fn earth_direction(longitude: f64, latitude: f64) -> [f64; 3] {
    let (longitude, latitude) = (longitude.to_radians(), latitude.to_radians());
    [
        trig::cos(latitude) * trig::cos(longitude),
        trig::cos(latitude) * trig::sin(longitude),
        trig::sin(latitude),
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Earth's land, sampled onto the world's tiles.
///
/// Nothing here is generated: each tile asks the world where it is and the
/// grid answers, so every game of this script is played on the same
/// coastlines. The seed still moves the rivers, the resources and the scatter
/// inside each biome, which is where a true-start map should differ between
/// games.
///
/// The islands in [`EARTH_ISLANDS`] are added after the vote. A tile-wide vote
/// necessarily loses anything smaller than a tile, and an Earth without
/// Britain, Java or Hawaii is not the Earth anyone means, so each is seated on
/// the one tile nearest it.
///
/// The twelve pentagons are left wherever Earth puts them. Planet holds its
/// twelve under water so that every land tile has six neighbours, and H3 turns
/// its icosahedron until all twelve fall in open ocean — but neither option is
/// open to Earth. Antarctica takes the south polar corner outright. The ten
/// off-pole corners sit on two rings at ±26.57°, five to a ring and 72° apart,
/// and no whole-degree spin seats more than nine of them at sea, because at
/// that latitude the ocean simply does not come in five gaps 72° apart. Since a
/// true Earth may not be rotated to suit its lattice anyway, the three that
/// land on it — the Sahara near 0°E, the Indus near 72°E and the pole — stay
/// land and simply have five neighbours. Adjacency, rings and distance all read
/// the tile graph, so those three tiles are irregular, not special-cased.
/// A flat Earth is the same silhouette read through the same longitudes and
/// latitudes, which is exactly what a paper world map is: the globe rolled
/// flat. The two pentagons stay a globe's problem, because a flat map has no
/// pentagons to begin with.
fn earth_tile_cache(wm: &WorldMap) -> BTreeMap<Pos, EarthTile> {
    let mut cache: BTreeMap<Pos, EarthTile> = wm
        .tiles
        .keys()
        .copied()
        .map(|position| (position, earth_tile(wm, position)))
        .collect();
    // The corridors the vote loses, opened after it. A pass is demoted to the
    // climate that was already recorded underneath its peaks, so it is the
    // ground the range stands on rather than an invented terrain, and it keeps
    // its hills: a pass is high, it is simply not a wall.
    for (longitude, latitude, _name) in EARTH_PASSES {
        let Some(position) = nearest_tile(wm, *longitude, *latitude) else {
            continue;
        };
        let Some(tile) = cache.get_mut(&position) else {
            continue;
        };
        if tile.water || tile.terrain != "mountain" {
            continue;
        }
        tile.terrain = tile.mountain_terrain.unwrap_or("plains");
        tile.mountain_terrain = None;
        tile.hills = true;
    }
    cache
}

fn earth_land(wm: &WorldMap) -> BTreeSet<Pos> {
    let cache = earth_tile_cache(wm);
    earth_land_from_cache(wm, &cache)
}

fn earth_land_from_cache(wm: &WorldMap, cache: &BTreeMap<Pos, EarthTile>) -> BTreeSet<Pos> {
    let mut land: BTreeSet<Pos> = wm
        .tiles
        .keys()
        .copied()
        .filter(|pos| !cache[pos].water)
        .collect();
    let tile_area = earth_tile_area(wm);
    let mut islands: BTreeSet<Pos> = BTreeSet::new();
    for (longitude, latitude, area) in EARTH_ISLANDS {
        if area * ISLAND_TILE_SHARE < tile_area {
            continue;
        }
        if let Some(pos) = nearest_tile(wm, *longitude, *latitude) {
            land.insert(pos);
            islands.insert(pos);
        }
    }
    if tile_area <= MICROSTATE_ISLAND_MAX_TILE_AREA {
        for (longitude, latitude) in EARTH_MICROSTATE_ISLANDS {
            if let Some(pos) = nearest_tile(wm, *longitude, *latitude) {
                land.insert(pos);
                islands.insert(pos);
            }
        }
    }
    // And the same guarantee in reverse. A lake narrower than a tile is
    // outvoted by the land around it exactly as an island is outvoted by the
    // sea, and the Caspian is one tile wide on a Standard globe. An island
    // already seated keeps its tile: nothing on this list is worth drowning a
    // landmass for.
    for (longitude, latitude, area) in EARTH_LAKES {
        if area * LAKE_TILE_SHARE < tile_area {
            continue;
        }
        if let Some(pos) = nearest_tile(wm, *longitude, *latitude) {
            if !islands.contains(&pos) {
                land.remove(&pos);
            }
        }
    }
    land
}

/// Earth's surface in thousands of square kilometres, and what one tile of a
/// given world is worth of it.
const EARTH_AREA: f64 = 510_072.0;

fn earth_tile_area(wm: &WorldMap) -> f64 {
    EARTH_AREA / wm.tiles.len().max(1) as f64
}

/// How much of a tile a guaranteed island or lake has to be worth before the
/// map is fine enough to draw it. An island earns its tile at an eighth of
/// one and a lake only at a half, for the reason [`EARTH_LAKES`] gives.
const ISLAND_TILE_SHARE: f64 = 8.0;
const LAKE_TILE_SHARE: f64 = 2.0;

/// Earth under a tile the world has already decided is land.
///
/// [`earth_tile`] can call a tile water and [`earth_land`] still keep it: an
/// island from [`EARTH_ISLANDS`] is smaller than the tile that carries it, so
/// the sea around it wins the vote. The island is still made of something, so
/// widen the search until a land cell turns up and let that speak for it.
fn earth_ground_from_sample(wm: &WorldMap, pos: Pos, sampled: EarthTile) -> EarthTile {
    if !sampled.water {
        return sampled;
    }
    let (longitude, latitude) = wm.lon_lat(pos);
    for ring in 1..=6 {
        let reach = ring as f64 * EARTH_CELL_DEGREES;
        for step in 0..(8 * ring) {
            let angle = std::f64::consts::TAU * step as f64 / (8 * ring) as f64;
            let cell = earth_cell(
                longitude
                    + reach * trig::cos(angle) / trig::cos(latitude.to_radians()).abs().max(0.02),
                (latitude + reach * trig::sin(angle)).clamp(-89.999, 89.999),
            );
            if cell.is_land() {
                return EarthTile {
                    water: false,
                    terrain: EARTH_TERRAIN[cell.surface() as usize],
                    mountain_terrain: (cell.surface() == EARTH_MOUNTAIN).then_some("grassland"),
                    hills: cell.hills(),
                    vegetation: EARTH_VEGETATION[cell.vegetation()],
                };
            }
        }
    }
    EarthTile {
        water: false,
        terrain: "grassland",
        mountain_terrain: None,
        hills: false,
        vegetation: None,
    }
}

/// Earth's relief, climate and vegetation, painted onto the world's land.
///
/// This is what replaces `MountainsCliffs.lua` and `TerrainGenerator.lua` on a
/// true-start map. The Alps, the Andes, the Himalaya and the Rockies are where
/// they are because the elevation grid says so; the Sahara, the Amazon and the
/// Siberian taiga are where they are because the climate grid says so. Both
/// arrive together, per tile, because both describe the same tile.
///
/// The one setting still worth honouring is the poles. `Poles` is Earth's own
/// answer and takes the real climate. A world asked for **no** cold ends, or
/// for cold ends somewhere else, is being asked for a climate Earth does not
/// have, so those two hand the terrain back to the latitude bands and keep
/// only the relief — which is the part of Earth the setting was never about.
fn paint_earth(
    wm: &mut WorldMap,
    land: &BTreeSet<Pos>,
    poles: MapPoles,
    earth: Option<&BTreeMap<Pos, EarthTile>>,
    rng: &mut Rng,
) -> BTreeMap<Pos, Name> {
    let painted: Vec<(Pos, EarthTile)> = land
        .iter()
        .map(|pos| {
            let sampled = earth
                .and_then(|cache| cache.get(pos).copied())
                .unwrap_or_else(|| earth_tile(wm, *pos));
            (*pos, earth_ground_from_sample(wm, *pos, sampled))
        })
        .collect();
    for (pos, earth) in &painted {
        let tile = wm.tiles.get_mut(pos).unwrap();
        tile.terrain = earth.terrain.into();
        tile.hills = earth.hills;
    }

    let mut mountain_terrain: BTreeMap<Pos, Name> = painted
        .iter()
        .filter_map(|(pos, earth)| {
            earth
                .mountain_terrain
                .map(|terrain| (*pos, Name::new(terrain)))
        })
        .collect();
    if !poles.has_poles() {
        let land_list: Vec<Pos> = land.iter().copied().collect();
        mountain_terrain = assign_biomes(wm, &land_list, poles, rng);
    }

    // Earth's real coastal ranges stay: Norway, Chile and Honshu are
    // mountainous down to the water and should read that way. What cannot
    // stay is a landmass made of nothing but rock, because no unit can stand
    // on it and no city can be founded there — the tile would be land that is
    // not land. Any such island is brought down to its own foothills.
    let mut seen: BTreeSet<Pos> = BTreeSet::new();
    let mut levelled: Vec<Pos> = Vec::new();
    for start in land {
        if !seen.insert(*start) {
            continue;
        }
        let mut body = vec![*start];
        let mut frontier = VecDeque::from([*start]);
        while let Some(pos) = frontier.pop_front() {
            for neighbor in wm.neighbors(pos) {
                if land.contains(&neighbor) && seen.insert(neighbor) {
                    body.push(neighbor);
                    frontier.push_back(neighbor);
                }
            }
        }
        if body.iter().all(|pos| wm.tiles[pos].terrain == "mountain") {
            levelled.extend(body);
        }
    }
    for pos in levelled {
        let tile = wm.tiles.get_mut(&pos).unwrap();
        tile.terrain = "plains".into();
        tile.hills = true;
        mountain_terrain.remove(&pos);
    }

    // Vegetation last, and only where the terrain it landed on can carry it:
    // the grid's rainforest belongs to the tropics it was read from, and a
    // world without poles has moved those tropics somewhere else.
    for (pos, earth) in painted {
        let Some(plant) = earth.vegetation else {
            continue;
        };
        let tile = wm.tiles.get_mut(&pos).unwrap();
        let suits = match plant {
            "jungle" => matches!(tile.terrain.as_str(), "grassland" | "plains"),
            "marsh" => matches!(tile.terrain.as_str(), "grassland" | "plains" | "tundra"),
            _ => !matches!(tile.terrain.as_str(), "mountain" | "desert" | "snow"),
        };
        if suits && tile.feature.is_none() {
            if plant == "jungle" {
                // Rainforest leaves the ground beneath it Plains, as it does
                // everywhere else the generator lays it down.
                tile.terrain = "plains".into();
            }
            tile.feature = Some(plant.into());
        }
    }
    mountain_terrain
}

/// The tile whose centre points nearest a place on Earth.
fn nearest_tile(wm: &WorldMap, longitude: f64, latitude: f64) -> Option<Pos> {
    let target = earth_direction(longitude, latitude);
    wm.tiles.keys().copied().max_by(|a, b| {
        dot(wm.direction(*a), target)
            .partial_cmp(&dot(wm.direction(*b), target))
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

/// The physical Earth tile nearest a WGS84 point, restricted to sampled land.
///
/// A country-level coordinate is not necessarily a capital coordinate: an
/// archipelagic or geographically split country can reasonably be represented
/// by a centroid in the sea between its land areas.  Scenario builders must
/// therefore resolve the coordinate through the physical map rather than put
/// a city on the raw nearest tile.
#[cfg(test)]
fn nearest_earth_land(
    wm: &WorldMap,
    land: &BTreeSet<Pos>,
    longitude: f64,
    latitude: f64,
) -> Option<Pos> {
    let target = earth_direction(longitude, latitude);
    land.iter().copied().max_by(|a, b| {
        dot(wm.direction(*a), target)
            .partial_cmp(&dot(wm.direction(*b), target))
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

/// Seat each civilization on the viable tile closest to its homeland.
///
/// Closeness is measured on the globe, not in the storage rectangle: the tile
/// whose centre points nearest the homeland's direction wins.  The caller
/// hands in the selected roster's points, so a named civilization always
/// receives *its* address rather than whatever happened to occupy its seat
/// number in an old stock list.
///
/// Spacing is a floor and being home is what is maximised, which is the
/// opposite of how a rolled map is laid out and the whole difference between a
/// true start and a balanced one. The floor is [`MIN_START_SEPARATION`],
/// because that is the radius `Game::can_found_city` refuses to build inside:
/// a capital seated closer than that to its neighbour is a Settler that cannot
/// found where it stands. Above the floor nothing is bought by standing
/// further off, so every seat takes the tile nearest its own homeland and
/// Europe comes out crowded — because Europe *is* crowded. Rome, Greece and
/// Macedon stand a founding radius apart rather than being fanned across the
/// Mediterranean to satisfy a spacing rule none of them ever obeyed.
///
/// The floor gives way rather than leaving a seat unfilled, one ring at a
/// time, down to a plain refusal to share a hex.
fn historic_major_spawns(
    wm: &WorldMap,
    candidates: &[Pos],
    homelands: &[TrueStartPoint],
    count: usize,
) -> Vec<Pos> {
    assert!(
        !homelands.is_empty(),
        "True Start Earth needs at least one homeland"
    );
    let mut available: Vec<Pos> = candidates.to_vec();
    let mut starts: Vec<Pos> = Vec::new();
    for index in 0..count {
        if available.is_empty() {
            break;
        }
        let home = homelands[index % homelands.len()];
        let target = earth_direction(home.longitude, home.latitude);
        let closest = |pool: &mut dyn Iterator<Item = (usize, &Pos)>| {
            pool.max_by(|(_, a), (_, b)| {
                dot(wm.direction(**a), target)
                    .partial_cmp(&dot(wm.direction(**b), target))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(candidate_index, _)| candidate_index)
        };
        let mut selected = 0;
        for separation in (1..=MIN_START_SEPARATION).rev() {
            let taken = taken_within(wm, &starts, separation - 1);
            if let Some(candidate) = closest(
                &mut available
                    .iter()
                    .enumerate()
                    .filter(|(_, candidate)| !taken.contains(candidate)),
            ) {
                selected = candidate;
                break;
            }
        }
        starts.push(available.swap_remove(selected));
    }

    // An early selected leader can take the hex a later leader wanted and
    // leave it walking. Nothing about seating order says Rome should outrank
    // Venice for Italian ground, so trade any pair of seats that both leaders
    // would rather have the other way.
    // The occupied hexes never change, only who is on which, so every hex of
    // clearance the pass above bought survives untouched.
    let homes: Vec<[f64; 3]> = (0..starts.len())
        .map(|index| {
            let home = homelands[index % homelands.len()];
            earth_direction(home.longitude, home.latitude)
        })
        .collect();
    let seats: Vec<[f64; 3]> = starts.iter().map(|start| wm.direction(*start)).collect();
    let mut order: Vec<usize> = (0..starts.len()).collect();
    let mut traded = true;
    while traded {
        traded = false;
        for first in 0..order.len() {
            for second in (first + 1)..order.len() {
                let cost = |civ: usize, seat: usize| 1.0 - dot(seats[order[seat]], homes[civ]);
                let now = cost(first, first) + cost(second, second);
                let swapped = cost(first, second) + cost(second, first);
                if swapped < now - f64::EPSILON {
                    order.swap(first, second);
                    traded = true;
                }
            }
        }
    }
    order.into_iter().map(|seat| starts[seat]).collect()
}

/// Seat city-states at their own sites on a fixed-geography world.
///
/// The same walk as [`historic_major_spawns`], with two differences that come
/// from city-states being seated second rather than first.
///
/// The civilizations are already down, so `majors` is a floor this pass may
/// never break: a city-state that lands on a capital's hex would be relocated
/// or dropped later, and dropping is what used to leave a game with a third of
/// the city-states it asked for. Their clearance is the wider
/// `MINOR_MAJOR_BUFFER`, relaxed one ring at a time exactly as the major pass
/// relaxes its own, so a crowded region seats a tighter city-state rather than
/// no city-state.
///
/// And a city-state may have no site — a mod overlay's roster row can omit the
/// coordinates. Those seats are left to the caller, which falls back to the
/// regional model for the whole draw rather than mixing two placement rules in
/// one world.
///
/// No seat-trading pass. Trading exists because seating order should not
/// decide which of two civilizations gets Italy, and it is safe there because
/// the set of occupied hexes never changes. Here the draw is already dispersed
/// by construction — `city_states::seat_selection` takes the identity farthest
/// from the ones already chosen — so two city-states rarely want one hex, and
/// the order they arrive in is not a fairness question between rivals.
fn historic_minor_spawns(
    wm: &WorldMap,
    candidates: &[Pos],
    sites: &[Option<TrueStartPoint>],
    majors: &[Pos],
    count: usize,
) -> Vec<Pos> {
    let mut available: Vec<Pos> = candidates.to_vec();
    let mut starts: Vec<Pos> = Vec::new();
    for home in sites.iter().take(count.min(sites.len())) {
        if available.is_empty() {
            break;
        }
        let Some(home) = home else { continue };
        let target = earth_direction(home.longitude, home.latitude);
        let closest = |pool: &mut dyn Iterator<Item = (usize, &Pos)>| {
            pool.max_by(|(_, a), (_, b)| {
                dot(wm.direction(**a), target)
                    .partial_cmp(&dot(wm.direction(**b), target))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(candidate_index, _)| candidate_index)
        };
        let mut selected = None;
        for separation in (1..=MINOR_MAJOR_BUFFER).rev() {
            let placed: Vec<Pos> = majors.iter().chain(starts.iter()).copied().collect();
            let taken = taken_within(wm, &placed, separation - 1);
            if let Some(candidate) = closest(
                &mut available
                    .iter()
                    .enumerate()
                    .filter(|(_, candidate)| !taken.contains(candidate)),
            ) {
                selected = Some(candidate);
                break;
            }
        }
        // Every remaining hex is inside somebody's innermost ring. The caller
        // fills the shortfall the way it fills any other, rather than stacking
        // a city-state on a capital to keep the count.
        let Some(selected) = selected else { break };
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
        // Earth is read rather than rolled. See [`earth_land`] for what the
        // world is asked, and why the two pentagons that fall on land are
        // allowed to stay there.
        return earth_land(wm);
    }
    if script == MapScript::GrandCanals {
        // Answered before the shape is dispatched on, the way Earth is: the
        // six canals are one piece of geometry that a world of either shape
        // reads the same way, and the ground they leave is filled in once for
        // both. See [`canal_world`].
        return canal_world(wm, poles, rng);
    }
    if script == MapScript::GrandCanalsTwo {
        // The same again for the world of blocks: what cuts it is a question
        // about tiles rather than about a rectangle or a sphere, so it is
        // answered once for both shapes. See [`canal_blocks`].
        return canal_blocks(wm, poles, rng).land;
    }
    if script == MapScript::TeninsBall {
        // The seam is also one piece of geometry, like the six lanes of Grand
        // Canals. Its latitude is sampled from the world's own surface, so a
        // flat map wraps the seam at its east edge and a globe closes it at
        // every longitude. See [`tennis_ball_land`].
        return tennis_ball_land(wm, poles);
    }
    if wm.sphere().is_some() {
        if script == MapScript::Fjords {
            // Fjords are a coastline-first world on either shape. Reading the
            // same wrap-aware channel field through longitude and latitude keeps
            // the globe from falling back to rounded inland seas, which are the
            // one thing this script should not look like.
            return fjord_land(wm, poles, rng);
        }
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
        MapScript::Pangaea | MapScript::TacticsPlanet => {
            // A compact oval gives every seat comparable hinterland while
            // retaining a single coast-to-coast supercontinent. The stock
            // scripts cut their coastline out of a fractal rather than a
            // curve, which is what produces bays, peninsulas and the odd
            // offshore island; the oval only decides where the sea level sits.
            let center_col = (width - 1) as f64 / 2.0;
            let center_row = (height - 1) as f64 / 2.0;
            let radius_col = width as f64 * 0.39;
            let radius_row = height as f64 * 0.343;
            let shore = Fractal::new_without_index(rng, width, height, 4);
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
        MapScript::Fjords => fjord_land(wm, poles, rng),
        MapScript::InlandSea => {
            let center_col = (width - 1) as f64 / 2.0;
            let center_row = (height - 1) as f64 / 2.0;
            let radius_col = width as f64 * 0.34;
            let radius_row = height as f64 * 0.30;
            let shore = Fractal::new_without_index(rng, width, height, 4);
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
        MapScript::TeninsBall => tennis_ball_land(wm, poles),
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
                IslandScatterPlan {
                    target,
                    min_size: MIN_LANDMASS_FOR_A_START,
                    max_size: 24,
                    channel: ISLAND_CHANNEL,
                    min_bodies: seats,
                },
                rng,
            )
        }
        // The naval battlefield is always a globe — `is_planet_battlefield`
        // holds its topology at Planet before the shape is dispatched on, and
        // `globe_land` lays its two home islands and its islets. It is grouped
        // here so a direct call that forces the flat shape anyway still gets
        // an ocean with land enough to seat both sides, rather than falling
        // through to a branch that would grow it a continent.
        MapScript::WaterWorld | MapScript::TacticsOcean => {
            // The far end of the same dial as Land Only: specks of land in an
            // ocean. The floor is what keeps it playable — a world that cannot
            // seat every civilization and city-state on land is not a map, so
            // the smallest sizes get a little more land than the share asks
            // for rather than a broken world.
            let mut open = offset_region(wm, 0, width, 1, height - 1);
            let seats = num_major_spawns + num_minor_spawns;
            let target = (area * script.land_percent() as usize / 100)
                .max(seats * WATER_WORLD_TILES_PER_SEAT);
            scatter_islands(
                wm,
                &mut open,
                IslandScatterPlan {
                    target,
                    min_size: WATER_WORLD_TILES_PER_SEAT,
                    max_size: 15,
                    channel: ISLAND_CHANNEL,
                    min_bodies: seats,
                },
                rng,
            )
        }
        MapScript::Battlefield => {
            // A bounded arena rather than a world: every tile is ground, and
            // the four walls are the map's own edges rather than a rim of sea
            // (`Topology::Rectangle`). There is no water on a battlefield at
            // all — no coast to hug, no pond to anchor a flank against, and
            // no seam to walk through — so what decides the fight is the
            // ground and what each side does with it. The whole field is laid
            // out again by [`paint_battlefield_ground`] once the world passes
            // have run; this only says that all of it is land.
            offset_region(wm, 0, width, 0, height)
        }
        // The mirror image of the arena above, and read rather than rolled:
        // the Trafalgar scenario is nearly all water, and the shore it does
        // have is the Andalusian coast in a particular place. Both come off
        // the chart in [`crate::trafalgar`]; this only says which of its
        // cells are dry.
        MapScript::Trafalgar => crate::trafalgar::chart_land(wm),
        MapScript::Kadesh
        | MapScript::Marathon
        | MapScript::Thermopylae
        | MapScript::Gaugamela
        | MapScript::Cannae
        | MapScript::Actium
        | MapScript::Hastings
        | MapScript::Hattin
        | MapScript::Agincourt
        | MapScript::Constantinople1453
        | MapScript::Lepanto
        | MapScript::SpanishArmada
        | MapScript::Waterloo
        | MapScript::Gettysburg
        | MapScript::Stalingrad
        | MapScript::Normandy
        | MapScript::Midway
        | MapScript::Inchon
        | MapScript::DienBienPhu
        | MapScript::SixDayWar
        | MapScript::DesertStorm
        | MapScript::Fallujah
        | MapScript::Mosul => {
            // Which cells of a real place are dry is a fact about that place,
            // so it comes off the battle's own drawn chart in
            // [`crate::historical_terrain`] — the same plan that paints it —
            // rather than from a rule about where water tends to go.
            let scenario = crate::historical_scenarios::by_script(script)
                .expect("every scenario map script has a catalogue row");
            let plan = crate::historical_terrain::by_id(scenario.id)
                .expect("every catalogue battle has a drawn chart");
            crate::historical_terrain::land_tiles(wm, plan)
        }
        // Answered before the shape was dispatched on, because Earth's
        // coastlines are read rather than rolled and are the same coastlines
        // on either shape. The two choices differ only when starts are seated.
        MapScript::Earth | MapScript::TrueStartEarth => earth_land(wm),
        // Likewise: a canal is cut at an angle to an axis of the world, which
        // is a question neither shape answers differently. [`generate_land`]
        // sends it to [`canal_world`] before it reaches here.
        MapScript::GrandCanals => canal_world(wm, poles, rng),
        // And likewise for the blocks of Grand Canals II, which are measured
        // in tiles and so are the same construction on either shape.
        MapScript::GrandCanalsTwo => canal_blocks(wm, poles, rng).land,
    }
}

/// A water-carved coastal world with room for all three requested terrain
/// classes: forty percent sea, forty percent ordinary land, and twenty percent
/// mountain. The high ridges of the field are removed as water rather than
/// grown as isolated lakes, so they make long inlets, shorter cuts, and narrow
/// passages throughout the map instead of one ocean around a continent.
fn fjord_land(wm: &WorldMap, poles: MapPoles, rng: &mut Rng) -> BTreeSet<Pos> {
    let total = wm.tiles.len();
    // Flat worlds keep their two polar rows wet; a poled globe keeps a small
    // cap wet for the same climate reason. Pentagons stay wet on a globe so
    // every surfaced tile retains the six-neighbour contract. Choose the
    // remaining water from the eligible field, so those reservations do not
    // quietly turn the advertised 40/40/20 mix into a drier map.
    let pentagons: BTreeSet<Pos> = wm
        .sphere()
        .map(|sphere| sphere.pentagons().into_iter().collect())
        .unwrap_or_default();
    let cap = if poles.has_poles() { 0.93 } else { f64::MAX };
    let interior: BTreeSet<Pos> = if wm.sphere().is_some() {
        wm.tiles
            .keys()
            .copied()
            .filter(|pos| !pentagons.contains(pos) && wm.polar_fraction(*pos) < cap)
            .collect()
    } else {
        offset_region(wm, 0, wm.width, 1, wm.height - 1)
    };
    let reserved_water = total.saturating_sub(interior.len());
    let water_inside = (total * 40 / 100)
        .saturating_sub(reserved_water)
        .min(interior.len());
    let mut channels = Fractal::new_without_index(rng, wm.width, wm.height, 4);
    // More, finer plates than an ordinary world gives the sea many independent
    // cuts: some run the length of a continent, some stop at a bay, and their
    // joins leave the constricted passages the script is named for. A second
    // fine field only roughens the banks; the ridge field keeps the channels
    // long enough to leave stringy land between them.
    channels.build_ridges_without_index(rng, 24, 6.0, 3.0);
    let teeth = Fractal::new_without_index(rng, wm.width, wm.height, 5);
    let mut isthmuses = Fractal::new_without_index(rng, wm.width, wm.height, 3);
    isthmuses.build_ridges_without_index(rng, 6, 5.0, 5.0);
    // A few worlds get a long land spine deliberately protected from the
    // channel cut. Since the spine is another ridged field, it arrives as a
    // crooked isthmus rather than a straight bridge; the zero-strength roll
    // leaves other worlds with separated peninsulas instead.
    let isthmus_weight = match rng.below(4) {
        0 => 0,
        1 => 1,
        _ => 2,
    };
    let scores: BTreeMap<Pos, i32> = interior
        .iter()
        .copied()
        .map(|pos| {
            let (col, row) = noise_cell(wm, pos);
            (
                pos,
                channels.at(col, row) as i32 * 3 + teeth.at(col, row) as i32
                    - isthmus_weight * isthmuses.at(col, row) as i32,
            )
        })
        .collect();

    // Grow the sea from its existing edge instead of selecting the highest
    // scores independently. A raw percentile can leave the map with many
    // disconnected inland pockets; a ranked frontier keeps most of the water
    // in one navigable system while the ridges still decide where its fingers
    // run. Choosing among the six best frontier tiles leaves the banks ragged.
    let mut water: BTreeSet<Pos> = wm
        .tiles
        .keys()
        .copied()
        .filter(|pos| !interior.contains(pos))
        .collect();
    let mut queued = BTreeSet::new();
    let mut frontier = BinaryHeap::new();
    for pos in water
        .iter()
        .flat_map(|pos| wm.neighbors(*pos))
        .filter(|pos| interior.contains(pos))
    {
        if queued.insert(pos) {
            frontier.push((scores[&pos], std::cmp::Reverse(pos)));
        }
    }
    let mut carved = 0;
    while carved < water_inside {
        if frontier.is_empty() {
            // This is only reachable on a degenerate tiny map or a globe with
            // an entirely reserved cap. Starting a new seed is safer than
            // missing the script's advertised water share.
            let Some(seed) = scores
                .keys()
                .filter(|pos| !water.contains(pos))
                .max_by_key(|pos| (scores[pos], std::cmp::Reverse(**pos)))
                .copied()
            else {
                break;
            };
            water.insert(seed);
            carved += 1;
            for pos in wm
                .neighbors(seed)
                .into_iter()
                .filter(|pos| interior.contains(pos) && !water.contains(pos))
            {
                if queued.insert(pos) {
                    frontier.push((scores[&pos], std::cmp::Reverse(pos)));
                }
            }
            continue;
        }
        let mut choices = Vec::with_capacity(6);
        for _ in 0..6 {
            if let Some(choice) = frontier.pop() {
                choices.push(choice);
            } else {
                break;
            }
        }
        let choice_index = rng.below(choices.len());
        let (_, std::cmp::Reverse(chosen)) = choices.swap_remove(choice_index);
        queued.remove(&chosen);
        frontier.extend(choices);
        if !water.insert(chosen) {
            continue;
        }
        carved += 1;
        for pos in wm
            .neighbors(chosen)
            .into_iter()
            .filter(|pos| interior.contains(pos) && !water.contains(pos))
        {
            if queued.insert(pos) {
                frontier.push((scores[&pos], std::cmp::Reverse(pos)));
            }
        }
    }
    interior.difference(&water).copied().collect()
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

#[derive(Clone, Copy)]
struct IslandScatterPlan {
    target: usize,
    min_size: usize,
    max_size: usize,
    channel: i32,
    min_bodies: usize,
}

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
    plan: IslandScatterPlan,
    rng: &mut Rng,
) -> BTreeSet<Pos> {
    let span = (plan.max_size + 1).saturating_sub(plan.min_size).max(1);
    let mut seeds: Vec<Pos> = open.iter().copied().collect();
    for index in (1..seeds.len()).rev() {
        let other = rng.below(index + 1);
        seeds.swap(index, other);
    }
    let mut land = BTreeSet::new();
    let mut bodies = 0;
    while (land.len() < plan.target || bodies < plan.min_bodies) && !open.is_empty() {
        let Some(seed) = seeds.pop() else { break };
        if !open.contains(&seed) {
            continue;
        }
        // Once the land target is met and only the island count is still
        // owed, the remaining islands are kept to the smallest size that can
        // still hold a capital, so meeting the count costs the world as
        // little of its ocean as possible.
        let wanted = if land.len() >= plan.target {
            plan.min_size
        } else {
            (plan.min_size + rng.below(span)).min((plan.target - land.len()).max(plan.min_size))
        };
        let island = grow_blob(wm, open, seed, wanted.max(1), rng);
        // A gap that cannot hold the promised minimum stays water. Committing
        // it as a one-plot "island" made the body quota look satisfied while
        // leaving a civilization with nowhere to found its capital.
        if island.len() < plan.min_size {
            for pos in island {
                open.remove(&pos);
            }
            continue;
        }
        bodies += 1;
        close_off(wm, open, &island, plan.channel);
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
        MapScript::Islands => scatter_islands(
            wm,
            &mut field,
            IslandScatterPlan {
                target,
                min_size: MIN_LANDMASS_FOR_A_START,
                max_size: 24,
                channel: ISLAND_CHANNEL,
                min_bodies: seats,
            },
            rng,
        ),
        MapScript::WaterWorld => scatter_islands(
            wm,
            &mut field,
            IslandScatterPlan {
                target,
                min_size: WATER_WORLD_TILES_PER_SEAT,
                max_size: 15,
                channel: ISLAND_CHANNEL,
                min_bodies: seats,
            },
            rng,
        ),
        // The naval battlefield, laid in two passes because its land does two
        // different jobs.
        //
        // First the home islands: one per seat, each grown to a size that can
        // actually carry a city, because a fleet with no port is not the
        // battle this mode is about and a capital squeezed onto a rock is the
        // failure Water World's per-seat floor exists to prevent. They are cut
        // with the full channel between them, so the two sides start as far
        // apart as an archipelago ever seats anyone.
        //
        // Then the islets, from whatever share of the water is left: small,
        // and deliberately packed at a one-tile channel rather than three.
        // These are the only features on an otherwise empty sea — the thing a
        // flank is worked around, the gap that can be held by a few ships
        // against many — and a three-tile channel between every pair would
        // make them scenery a fleet sails past instead of ground it fights
        // over.
        MapScript::TacticsOcean => {
            let ports = seats.max(2);
            let home = (ports * WATER_WORLD_TILES_PER_SEAT * 2).min(target);
            let mut land = scatter_islands(
                wm,
                &mut field,
                IslandScatterPlan {
                    target: home,
                    min_size: WATER_WORLD_TILES_PER_SEAT,
                    max_size: WATER_WORLD_TILES_PER_SEAT * 2,
                    channel: ISLAND_CHANNEL,
                    min_bodies: ports,
                },
                rng,
            );
            land.extend(scatter_islands(
                wm,
                &mut field,
                IslandScatterPlan {
                    target: target.saturating_sub(home),
                    min_size: 2,
                    max_size: 7,
                    channel: 1,
                    min_bodies: 0,
                },
                rng,
            ));
            land
        }
        _ => {
            let continents = match script {
                MapScript::Pangaea | MapScript::TacticsPlanet => 1,
                MapScript::Continents => 2,
                _ => num_major_spawns.div_ceil(2).clamp(3, 7),
            };
            let mut land = scatter_bodies(wm, &mut field, continents, target, rng);
            // Something in the open ocean to find, without turning the sea
            // lanes into an archipelago of their own.
            let islands = (target / 12).min(continents * 3);
            land.extend(scatter_islands(
                wm,
                &mut field,
                IslandScatterPlan {
                    target: islands * 6,
                    min_size: 3,
                    max_size: 11,
                    channel: 1,
                    min_bodies: 0,
                },
                rng,
            ));
            land
        }
    }
}

/// The Standard map has 84 × 54 tiles, and its seam is six tiles wide. Scaling
/// by the square root of tile count keeps the seam's proportion steady as map
/// sizes grow, while rounding leaves the stock sizes at clean whole-tile
/// widths: three on Duel, four on Tiny, five on Small, and six on Standard.
const TENNIS_BALL_STANDARD_TILES: f64 = 84.0 * 54.0;
const TENNIS_BALL_STANDARD_GAP_TILES: f64 = 5.5;
const TENNIS_BALL_MIN_GAP_TILES: f64 = 3.0;
const TENNIS_BALL_MAX_GAP_TILES: f64 = 10.0;

/// How far the seam bows north and south. Two turns around the longitude make
/// the single closed band read like the crossing curves stitched into a tennis
/// ball, rather than like an ordinary equatorial belt.
const TENNIS_BALL_SEAM_AMPLITUDE: f64 = 0.60;

/// The nearest point on a smooth seam is always within one half-wave of the
/// tile's own longitude. The sample count is chosen from the tile's angular
/// scale, so the approximation stays finer than a tile on every shipped map
/// while generation remains linear in map size.
const TENNIS_BALL_SEAM_SEARCH_RADIUS: f64 = std::f64::consts::FRAC_PI_2;
const TENNIS_BALL_SEAM_MIN_SAMPLES: usize = 32;
const TENNIS_BALL_SEAM_MAX_SAMPLES: usize = 256;

fn tennis_ball_gap_tiles(wm: &WorldMap) -> usize {
    (TENNIS_BALL_STANDARD_GAP_TILES * (wm.tiles.len() as f64 / TENNIS_BALL_STANDARD_TILES).sqrt())
        .round()
        .clamp(TENNIS_BALL_MIN_GAP_TILES, TENNIS_BALL_MAX_GAP_TILES) as usize
}

fn tennis_ball_seam_point(longitude: f64) -> [f64; 3] {
    let latitude = TENNIS_BALL_SEAM_AMPLITUDE * trig::sin(2.0 * longitude);
    [
        trig::cos(latitude) * trig::cos(longitude),
        trig::cos(latitude) * trig::sin(longitude),
        trig::sin(latitude),
    ]
}

/// The cosine of the angular distance from a world point to the tennis-ball
/// seam. Working in cosine space avoids an expensive inverse cosine for every
/// tile and is exact enough for the whole-tile band the map asks for.
fn tennis_ball_seam_proximity(point: [f64; 3], samples: usize) -> f64 {
    let longitude = trig::atan2(point[1], point[0]);
    (0..=samples)
        .map(|sample| {
            let fraction = sample as f64 / samples as f64;
            let longitude = longitude + (2.0 * fraction - 1.0) * TENNIS_BALL_SEAM_SEARCH_RADIUS;
            dot(point, tennis_ball_seam_point(longitude))
        })
        .fold(-1.0, f64::max)
}

/// The water stitching of a Tenins Ball world: one closed, wavy band around
/// every longitude.
fn tennis_ball_seam_water(wm: &WorldMap) -> BTreeSet<Pos> {
    let arc = tile_arc(wm);
    let half_width = tennis_ball_gap_tiles(wm) as f64 * arc / 2.0;
    let seam_cutoff = trig::cos(half_width);
    let samples = (std::f64::consts::PI / arc).ceil().clamp(
        TENNIS_BALL_SEAM_MIN_SAMPLES as f64,
        TENNIS_BALL_SEAM_MAX_SAMPLES as f64,
    ) as usize;
    wm.tiles
        .keys()
        .copied()
        .filter(|pos| tennis_ball_seam_proximity(wm.direction(*pos), samples) >= seam_cutoff)
        .collect()
}

/// A globe also keeps its twelve pentagons and, with poles, its polar caps in
/// water, as every generated globe does; those are not part of the playable
/// seam.
fn tennis_ball_water(wm: &WorldMap, poles: MapPoles) -> BTreeSet<Pos> {
    let globe = wm.sphere().is_some();
    let mut water = tennis_ball_seam_water(wm);
    let cap = if globe && poles.has_poles() {
        0.93
    } else {
        f64::MAX
    };
    let pentagons: BTreeSet<Pos> = wm
        .sphere()
        .map(|sphere| sphere.pentagons().into_iter().collect())
        .unwrap_or_default();
    water.extend(pentagons);
    water.extend(
        wm.tiles
            .keys()
            .copied()
            .filter(|pos| wm.polar_fraction(*pos) >= cap),
    );
    water
}

fn tennis_ball_land(wm: &WorldMap, poles: MapPoles) -> BTreeSet<Pos> {
    let water = tennis_ball_water(wm, poles);
    wm.tiles
        .keys()
        .copied()
        .filter(|pos| !water.contains(pos))
        .collect()
}

/// How far in from the seam's coast a four-seat game opens, in the world's own
/// tiles: near an end of the lobe, but not on the beach.
const TENNIS_BALL_START_INLAND_TILES: f64 = 6.5;

/// The stock four-seat layout of a Tenins Ball world: one civilization at each
/// end of each of the two lobes.
///
/// The seam swings north and south twice on its way around the world, so each
/// lobe is fattest where the seam has swung deepest into the other lobe's
/// hemisphere. Those four swings are the ends somebody sees when they look at
/// the map, and they are a quarter of the world apart from each other. A start
/// stands [`TENNIS_BALL_START_INLAND_TILES`] in from the seam's own coast at
/// its end, snapped to the nearest tile the candidate pool would seat a
/// capital on.
///
/// `None` means the pool had nothing to offer near one of the ends, or two
/// picks drifted inside the shipped clearance; the caller falls back to the
/// regional model, so the promise is a layout, never a lost or crowded seat.
fn tennis_ball_major_starts(wm: &WorldMap, pool: &BTreeSet<Pos>) -> Option<Vec<Pos>> {
    let coast_stand_off =
        (tennis_ball_gap_tiles(wm) as f64 / 2.0 + TENNIS_BALL_START_INLAND_TILES) * tile_arc(wm);
    // Which side of the seam each end is on, and the longitude where that
    // lobe is fattest: where `sin(2λ)` swings the seam deepest the other way.
    let ends = [
        (1.0_f64, 0.75 * std::f64::consts::PI),
        (1.0, 1.75 * std::f64::consts::PI),
        (-1.0, 0.25 * std::f64::consts::PI),
        (-1.0, 1.25 * std::f64::consts::PI),
    ];
    let mut starts: Vec<Pos> = Vec::with_capacity(ends.len());
    for (side, longitude) in ends {
        // At its own end the seam sits a full amplitude into this lobe's
        // opposite hemisphere; standing off its coast can leave the target on
        // either side of the equator, depending on how big a tile is here.
        let latitude = side * (coast_stand_off - TENNIS_BALL_SEAM_AMPLITUDE);
        let target = [
            trig::cos(latitude) * trig::cos(longitude),
            trig::cos(latitude) * trig::sin(longitude),
            trig::sin(latitude),
        ];
        let pick = pool
            .iter()
            .copied()
            .filter(|position| {
                let direction = wm.direction(*position);
                let seam = TENNIS_BALL_SEAM_AMPLITUDE
                    * trig::sin(2.0 * trig::atan2(direction[1], direction[0]));
                side * (trig::asin(direction[2]) - seam) > 0.0
            })
            .max_by(|first, second| {
                dot(wm.direction(*first), target)
                    .total_cmp(&dot(wm.direction(*second), target))
                    // The pool iterates in `Pos` order, so an exact tie goes
                    // to the lower position and the layout stays a function
                    // of the map alone.
                    .then_with(|| second.cmp(first))
            })?;
        starts.push(pick);
    }
    let clear = starts.iter().enumerate().all(|(index, start)| {
        starts[index + 1..]
            .iter()
            .all(|other| wm.distance(*start, *other) > MAJOR_START_BUFFER)
    });
    clear.then_some(starts)
}

/// How many rows at each end of an arena are that side's deployment band.
///
/// The band is where a side's army is set out, and it is left as open ground:
/// a side that opened the game already inside a wood on a hill would be
/// holding a fortress it never had to take, which is the opposite of what an
/// arena is for. All of the cover therefore sits in the ground between the
/// two bands, which is exactly the ground both sides have to come out and
/// fight over. Two rows is enough to set out an opening army without stacking
/// it, on the narrowest field offered.
pub const BATTLEFIELD_DEPLOYMENT_ROWS: i32 = 2;

/// How far in from its corner a square arena seats a side, on a twenty-hex
/// field. Scaled to the arena by [`battlefield_corner_inset`].
///
/// Off the corner itself, so that the city has ground on every side of it and
/// the army has somewhere to form up that is not a wedge two hexes wide.
const BATTLEFIELD_CORNER_INSET_PER_TWENTY: i32 = 3;

/// How far in from the corner this arena seats a side.
///
/// Three hexes in on the twenty-hex Field, scaled down for a smaller one —
/// and the scaling is not fussiness, it is the hex grid. A step along the
/// north-west/south-east offset diagonal is *two* hexes of real distance, so
/// an inset costs four hexes of approach for every one it looks like:
///
/// | arena | corner to corner | inset 1 | inset 2 | inset 3 |
/// | --- | --- | --- | --- | --- |
/// | 10x10 | 14 | 10 | 8 | **4** |
/// | 20x20 | 29 | 25 | 23 | **19** |
///
/// Three hexes in leaves the Field's two sides 19 apart — exactly the
/// approach the old mid-wall layout gave them — and would leave the Square's
/// two sides *four* apart, which is not an approach at all: the two armies
/// would deploy into each other. One hex in gives the Square 10, and that is
/// the same layout at the size it is.
fn battlefield_corner_inset(wm: &WorldMap) -> i32 {
    (wm.width.min(wm.height) * BATTLEFIELD_CORNER_INSET_PER_TWENTY / 20).max(1)
}

/// Where the two sides of a battlefield begin.
///
/// A square arena seats them in opposite corners, a little way in — the
/// longest line across a square is its diagonal, and a corner start is a
/// corner to hold rather than a wall to be backed against. A long arena seats
/// them at opposite ends of its long axis instead, because a rectangle's
/// diagonal is barely longer than its length and the fight it wants is up and
/// down the field.
///
/// `None` means the pool had nothing to offer or the two picks crowded
/// together; the caller falls back to the regional model, so the promise is a
/// layout, never a lost seat.
fn battlefield_major_starts(wm: &WorldMap, pool: &BTreeSet<Pos>) -> Option<Vec<Pos>> {
    let inset = battlefield_corner_inset(wm);
    let targets = if wm.width == wm.height {
        // North-west and south-east. Of the two diagonals this is the longer
        // one on an odd-r grid — 14 against 13 on the Square, 29 against 28
        // on the Field — because it runs against the rows' half-hex offset
        // rather than along with it.
        [
            (inset, inset),
            (wm.width - 1 - inset, wm.height - 1 - inset),
        ]
    } else if wm.width > wm.height {
        [(0, wm.height / 2), (wm.width - 1, wm.height / 2)]
    } else {
        [(wm.width / 2, 0), (wm.width / 2, wm.height - 1)]
    };
    let offset_distance = |position: Pos, (col, row): (i32, i32)| {
        let (c, r) = hex::axial_to_offset(position.0, position.1);
        (c - col).abs() + (r - row).abs()
    };
    let mut starts: Vec<Pos> = Vec::with_capacity(targets.len());
    for target in targets {
        let pick = pool
            .iter()
            .copied()
            .filter(|position| !starts.contains(position))
            .min_by(|first, second| {
                offset_distance(*first, target)
                    .cmp(&offset_distance(*second, target))
                    // The pool iterates in `Pos` order, so an exact tie goes
                    // to the lower position and the layout stays a function
                    // of the map alone.
                    .then_with(|| first.cmp(second))
            })?;
        starts.push(pick);
    }
    // Both snaps drifting toward the middle would be a different battle than
    // the one advertised; ask for at least half the arena between them.
    let apart = offset_distance(starts[0], hex::axial_to_offset(starts[1].0, starts[1].1))
        > wm.width.max(wm.height) / 2;
    apart.then_some(starts)
}

/// Where the two sides of the Tactics planet begin.
///
/// A globe has no meaningful north-west corner or long axis, so the map's
/// promise is expressed by its surface distance instead: choose the pair of
/// usable plots that are farthest apart on the sphere. On a world that is
/// four-fifths ground that pair is as near antipodal as the candidate plots
/// allow — one side opens wherever it stands and the other opens on the far
/// face of the planet, half a world of marching away.
///
/// The pool this is handed is one connected landmass, narrowed by the caller,
/// which is what keeps the far face reachable on foot and keeps a stray
/// offshore rock from stealing a capital.
fn tactics_planet_major_starts(wm: &WorldMap, pool: &BTreeSet<Pos>) -> Option<Vec<Pos>> {
    let mut best: Option<(i32, Pos, Pos)> = None;
    for (index, first) in pool.iter().enumerate() {
        for second in pool.iter().skip(index + 1) {
            let distance = wm.distance(*first, *second);
            let replace = best.is_none_or(|(best_distance, best_first, best_second)| {
                distance > best_distance
                    || (distance == best_distance && (*first, *second) < (best_first, best_second))
            });
            if replace {
                best = Some((distance, *first, *second));
            }
        }
    }
    let (_, first, second) = best?;
    Some(vec![first, second])
}

/// Lay out a Tactics arena's ground.
///
/// A world's terrain is rolled from plates, latitude bands and moisture, and
/// on a hundred-tile field all three degenerate: there is no room for a
/// fractal to vary, so the arena comes out either uniform or split across a
/// climate band the battle is not being fought in. #1367 already had to paint
/// the snow and tundra back out of it. An arena is designed ground rather
/// than a rolled world, so this lays it out from what a tactical map needs
/// instead, over whatever the world passes left behind.
///
/// What it needs is the defensive palette the mode is about, and nothing that
/// refuses a unit: open ground to manoeuvre over (no modifier), hills and
/// woods to hold (+3 to a defender, and woods block sight through them),
/// jungle that costs a march to enter, and marsh that is worse to be caught
/// in than the open (-2). Nothing here is water and nothing is impassable, so
/// every tile is ground both sides can walk and the result is a test of
/// tactics rather than of who drew the shorter path around a lake.
fn paint_battlefield_ground(wm: &mut WorldMap, rng: &mut Rng) {
    let field: Vec<Pos> = wm.tiles.keys().copied().collect();
    for pos in &field {
        let tile = wm.tiles.get_mut(pos).unwrap();
        // Plains and grassland are the two open terrains that carry no
        // defensive modifier of their own, so which one a tile is decides
        // nothing about the fight — it is what keeps the field from reading
        // as one flat colour.
        tile.terrain = if rng.chance(0.4) {
            "plains".into()
        } else {
            "grassland".into()
        };
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.river_edges = [false; 6];
        tile.cliff_edges = [false; 6];
        tile.coastal_lowland = 0;
        tile.continent = Some(0);
    }

    // The contested ground: everything the two deployment zones do not hold.
    // The zones follow the starts — the two corners of a square arena, the
    // two end walls of a long one — because their whole job is to keep the
    // cover off the ground a side is set down on.
    let (rows, cols) = (wm.height, wm.width);
    let mut open: BTreeSet<Pos> = if cols == rows {
        // A corner's zone is the triangle of ground cut off by a diagonal
        // that far in, which has to be deep enough to seat the army that
        // starts in it. Half again the seat's own depth measures out at 15
        // hexes a corner on the Square for its 8 units and 36 on the Field
        // for its 24, leaving 70% and 82% of the arena as contested ground.
        let reach = (battlefield_corner_inset(wm) + BATTLEFIELD_DEPLOYMENT_ROWS) * 3 / 2;
        offset_region(wm, 0, cols, 0, rows)
            .into_iter()
            .filter(|pos| {
                let (col, row) = hex::axial_to_offset(pos.0, pos.1);
                (col + row).min(cols - 1 - col + rows - 1 - row) > reach
            })
            .collect()
    } else if cols > rows {
        offset_region(
            wm,
            BATTLEFIELD_DEPLOYMENT_ROWS,
            cols - BATTLEFIELD_DEPLOYMENT_ROWS,
            0,
            rows,
        )
    } else {
        offset_region(
            wm,
            0,
            cols,
            BATTLEFIELD_DEPLOYMENT_ROWS,
            rows - BATTLEFIELD_DEPLOYMENT_ROWS,
        )
    };
    if open.is_empty() {
        return;
    }
    let contested = open.len();
    let share = |percent: usize| contested * percent / 100;

    // Cover is laid in patches rather than tile by tile, because a wood a
    // unit can shelter in is a wood several tiles across and scattered single
    // trees are noise. Roughly half the contested ground ends up under cover
    // of some kind, which leaves the other half to manoeuvre over.
    let hills = carve_patches(wm, &mut open, share(22), 3..7, rng);
    let forest = carve_patches(wm, &mut open, share(18), 3..7, rng);
    let jungle = carve_patches(wm, &mut open, share(8), 2..5, rng);
    let marsh = carve_patches(wm, &mut open, share(6), 2..4, rng);

    for pos in &hills {
        wm.tiles.get_mut(pos).unwrap().hills = true;
    }
    for pos in &forest {
        let tile = wm.tiles.get_mut(pos).unwrap();
        tile.feature = Some("forest".into());
        // Wooded hills: the strongest ground on the field, +3 for the trees
        // on top of +3 for the slope. Rare enough to be a position worth
        // taking rather than the shape of the whole map.
        tile.hills = rng.chance(0.25);
    }
    for pos in &jungle {
        wm.tiles.get_mut(pos).unwrap().feature = Some("jungle".into());
    }
    for pos in &marsh {
        let tile = wm.tiles.get_mut(pos).unwrap();
        // Marsh is low wet ground by definition, and the shipped feature sits
        // on grassland; a marsh on a hilltop would be neither.
        tile.terrain = "grassland".into();
        tile.hills = false;
        tile.feature = Some("marsh".into());
    }
}

/// Carve roughly `target` tiles out of `open` as a handful of blobs, each of
/// a size drawn from `patch`, and hand back what was taken. Claimed tiles
/// leave `open`, so successive calls lay disjoint cover.
fn carve_patches(
    wm: &WorldMap,
    open: &mut BTreeSet<Pos>,
    target: usize,
    patch: std::ops::Range<usize>,
    rng: &mut Rng,
) -> BTreeSet<Pos> {
    let mut taken = BTreeSet::new();
    let span = patch.len().max(1);
    while taken.len() < target && !open.is_empty() {
        let seeds: Vec<Pos> = open.iter().copied().collect();
        let seed = seeds[rng.below(seeds.len())];
        let want = (patch.start + rng.below(span)).clamp(1, target - taken.len());
        let blob = grow_blob(wm, open, seed, want, rng);
        // `grow_blob` returns at least its own seed while the seed is in the
        // allowed set, so this cannot spin; the guard is for the day some
        // caller passes a target of zero.
        if blob.is_empty() {
            break;
        }
        for pos in &blob {
            open.remove(pos);
        }
        taken.extend(blob);
    }
    taken
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

/// How much dry ground one block of a Grand Canals II world is meant to be
/// left holding, once the canals around its rim have taken their share.
///
/// A block is somewhere a civilization lives rather than a tile it steps over:
/// this is a few cities' worth of ground, so a seat that opens alone on one
/// has room to grow into it and a seat that shares one has a neighbour it can
/// see. It is a count of tiles rather than a share of the world, which is what
/// makes a block on a Duel map the same place as a block on a Ludicrous one —
/// the larger world has more of them, not bigger ones.
const CANAL_BLOCK_LAND_TILES: f64 = 80.0;

/// The rim a block pays towards the canals around it, per tile of their width,
/// as a multiple of the square root of the block's own area.
///
/// A hexagon of area `A` has a perimeter of `3.72·√A`, and every canal is
/// shared with the block on its far side, so half of that is what any one
/// block gives up. Blocks cut by a spread of seeds are not regular hexagons,
/// so this is the figure the size of a block is *aimed* with; what it actually
/// comes out at is measured in the tests.
const CANAL_BLOCK_RIM: f64 = 1.86;

/// The fewest blocks a Grand Canals II world is cut into, however small it is.
/// Below about this the canals stop being a network a fleet moves around and
/// become a couple of straits.
const CANAL_MIN_BLOCKS: usize = 5;

/// How many rounds the blocks are settled for after their seeds are spread.
/// See [`canal_blocks`]: spreading gets them apart, settling gets them even.
const CANAL_BLOCK_SETTLING_ROUNDS: usize = 6;

/// How much of the ground a block is over or under its share it gives up or
/// takes in one settling round. A whole share at once overshoots and the
/// blocks trade places round after round; half of it converges.
const CANAL_BLOCK_SETTLING_RATE: f64 = 0.5;

/// The furthest a block's boundary may be pushed out or pulled in by the
/// settling, as a fraction of how far across a block is. Past about this a
/// block that has been squeezed by all of its neighbours at once disappears
/// between them.
const CANAL_BLOCK_SETTLING_LIMIT: f64 = 0.35;

/// How far apart two rows of a flat hex map stand, when two neighbouring tiles
/// in the same row stand one apart. Every one of a tile's six neighbours is
/// then exactly one away, which is what lets a distance in this frame be read
/// as a count of tiles.
const HEX_ROW_SPACING: f64 = 0.866_025_403_784_438_6;

/// The three layers of a canal, in tiles: a shelf of shallow water off either
/// bank, and a channel of deep ocean between them.
///
/// Those two are what a canal is *for*. The shelf is water a galley can
/// work from the first turn, and it runs the whole way round a block, so every
/// block has a coast and a use for a harbour before anyone has a ship that can
/// leave. The channel is Ocean, which `Game::class_can_traverse` refuses to
/// anything without Cartography, so it is also a wall: a block is its own
/// world until the open sea is understood, and the age the map opens into is
/// one of neighbours nobody has met.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CanalProfile {
    shelf: usize,
    channel: usize,
}

impl CanalProfile {
    /// How wide the canal runs, bank to bank.
    fn width(self) -> f64 {
        (2 * self.shelf + self.channel) as f64
    }
}

/// The profiles a canal may be dug to, one drawn for each pair of blocks that
/// share a canal. Every layer is between one and three tiles; the four
/// together average four and a quarter tiles bank to bank, which is the width
/// [`canal_block_count`] sizes a block against.
const CANAL_PROFILES: [CanalProfile; 4] = [
    CanalProfile {
        shelf: 1,
        channel: 1,
    },
    CanalProfile {
        shelf: 1,
        channel: 2,
    },
    CanalProfile {
        shelf: 1,
        channel: 3,
    },
    CanalProfile {
        shelf: 2,
        channel: 1,
    },
];

/// Where the tiles of a world stand, in a frame whose unit is one tile.
///
/// Everything a Grand Canals II world is built from — how big a block is, how
/// wide a canal — is counted in tiles, so the geometry has to be asked in a
/// frame that answers in tiles. A globe measures the angle between two tiles
/// and divides it by the angle one tile subtends, which works because a
/// geodesic's cells are equal-area. A flat map is a hex grid on a cylinder,
/// where the six neighbours of a tile are all exactly one away and the world
/// comes back to itself going east.
///
/// This is the one place the two shapes are told apart, and it is not the
/// [`grand_canals`] reading of the same problem: those six lanes are cut at an
/// angle to an axis of the world, which is a question about degrees, and
/// degrees are the same on both shapes. A block is a question about tiles, and
/// a degree of longitude on a flat map is a tile near the equator and a
/// fraction of one near the pole. Asking this one in degrees would leave the
/// polar blocks many times the size of the equatorial ones on a flat map.
struct TileFrame {
    tiles: Vec<Pos>,
    points: Vec<[f64; 3]>,
    /// The angle one tile subtends at the centre of a globe. A flat map's
    /// points are already counted in tiles, so it has none.
    arc: Option<f64>,
    /// How far east a flat world runs before it comes back, in tiles.
    wrap: f64,
}

impl TileFrame {
    fn of(wm: &WorldMap) -> Self {
        let tiles: Vec<Pos> = wm.tiles.keys().copied().collect();
        let globe = wm.sphere().is_some();
        let points = tiles
            .iter()
            .map(|pos| {
                if globe {
                    return wm.direction(*pos);
                }
                let (col, row) = hex::axial_to_offset(pos.0, pos.1);
                [
                    col as f64 + 0.5 * (row & 1) as f64,
                    row as f64 * HEX_ROW_SPACING,
                    0.0,
                ]
            })
            .collect();
        Self {
            tiles,
            points,
            arc: globe.then(|| tile_arc(wm)),
            wrap: wm.width.max(1) as f64,
        }
    }

    /// How many tiles apart two points of the frame stand.
    fn span(&self, from: [f64; 3], to: [f64; 3]) -> f64 {
        match self.arc {
            Some(arc) => trig::acos(dot(from, to).clamp(-1.0, 1.0)) / arc,
            None => {
                let east = (from[0] - to[0]).abs();
                let east = east.min(self.wrap - east);
                let north = from[1] - to[1];
                (east * east + north * north).sqrt()
            }
        }
    }

    /// The two blocks a point stands nearest, and how far each one is once its
    /// reach is allowed for.
    ///
    /// The nearer names the block the point belongs to; half the difference of
    /// the two is how far the point stands off the line between them, because
    /// a step across that line takes one away from one span and adds it to the
    /// other. `reach` is what the settling has given each block over a plain
    /// span — a block short of ground reaches further for it — and it moves
    /// that line without bending it, so the difference still measures tiles.
    fn nearest_two(
        &self,
        seeds: &[[f64; 3]],
        reach: &[f64],
        point: [f64; 3],
    ) -> ((usize, f64), (usize, f64)) {
        let (mut own, mut rival) = ((usize::MAX, f64::MAX), (usize::MAX, f64::MAX));
        for (block, seed) in seeds.iter().enumerate() {
            let span = self.span(point, *seed) - reach[block];
            if span < own.1 {
                rival = own;
                own = (block, span);
            } else if span < rival.1 {
                rival = (block, span);
            }
        }
        (own, rival)
    }

    /// What one point contributes towards the middle of the block it is in.
    /// A flat map is a cylinder, so its east-west coordinate is an angle and
    /// has to be summed as one — otherwise a block sitting across the seam
    /// would average out on the far side of the world.
    fn toward_middle(&self, point: [f64; 3]) -> [f64; 3] {
        match self.arc {
            Some(_) => point,
            None => {
                let turn = std::f64::consts::TAU * point[0] / self.wrap;
                [trig::cos(turn), trig::sin(turn), point[1]]
            }
        }
    }

    /// The middle of the points those contributions came from.
    fn middle(&self, sum: [f64; 3], count: usize) -> Option<[f64; 3]> {
        if count == 0 {
            return None;
        }
        match self.arc {
            Some(_) => {
                let length = dot(sum, sum).sqrt();
                (length > 1e-9).then(|| [sum[0] / length, sum[1] / length, sum[2] / length])
            }
            None => {
                let turn = trig::atan2(sum[1], sum[0]).rem_euclid(std::f64::consts::TAU);
                Some([
                    turn / std::f64::consts::TAU * self.wrap,
                    sum[2] / count as f64,
                    0.0,
                ])
            }
        }
    }
}

/// A Grand Canals II world, as the three things it is made of.
#[derive(Default)]
struct CanalBlocks {
    /// The dry ground: every block of it.
    land: BTreeSet<Pos>,
    /// The shelf off either bank of every canal, which is shallow water.
    shelf: BTreeSet<Pos>,
    /// The channel down the middle of every canal, which is deep ocean.
    channel: BTreeSet<Pos>,
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
    generate_with_script_and_leader_starts(
        rules,
        width,
        height,
        num_major_spawns,
        num_minor_spawns,
        num_natural_wonders,
        num_continents,
        script,
        topology,
        poles,
        &[],
        &[],
        rng,
    )
}

/// Generate a world with the supplied identities' exact True Start addresses.
///
/// Ordinary scripts deliberately ignore `leader_starts`: their balanced layout
/// is independent of roster identity.  True Start Earth consumes one point per
/// major in seating order; callers which predate the roster contract pass an
/// empty slice and preserve the legacy data order.
///
/// `minor_starts` is the same contract one rank down: the real site of each
/// city-state the caller drew, in the order it means to seat them, so a
/// true-start world puts Jerusalem in the Levant and Cahokia on the
/// Mississippi instead of wherever the regional cut had room. An entry is
/// `None` when the roster carries no coordinates for that identity, and an
/// empty slice keeps the regional model for every seat -- which is what every
/// other script uses regardless.
#[allow(clippy::too_many_arguments)]
pub fn generate_with_script_and_leader_starts(
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
    leader_starts: &[TrueStartPoint],
    minor_starts: &[Option<TrueStartPoint>],
    rng: &mut Rng,
) -> (WorldMap, Vec<Pos>) {
    // The world's shape is asked for separately from what fills it. Fixed
    // geography only means its coastline is sampled rather than rolled: the
    // same longitudes and latitudes can be laid onto a flat atlas or a globe.
    // A globe is stored in a rectangle of its own shape, so its size's globe
    // is built rather than the cylinder a flat world lays out.
    //
    // The bounded battlefield is the one scripted exception to shape
    // independence among the Tactics maps: it needs corners and ends for its
    // two sides to start in. The Tactics planet keeps its globe, where its
    // opposite-side promise is measured over the sphere.
    let topology = if script.is_planet_battlefield() {
        MapTopology::Planet
    } else if script.is_battlefield() {
        MapTopology::Flat
    } else {
        topology
    };
    // Nothing on a battlefield exists to be developed or discovered, so the
    // arena seats no city-states and plants no natural wonders whatever the
    // size profile that reached this call would have asked for.
    let num_minor_spawns = if script.is_battlefield() {
        0
    } else {
        num_minor_spawns
    };
    let num_natural_wonders = if script.is_battlefield() {
        0
    } else {
        num_natural_wonders
    };
    let mut wm = if topology.is_globe() {
        WorldMap::globe(globe_frequency(width, height))
    } else if script.is_battlefield() {
        // Four walls rather than an east-west seam. A world wants its two
        // longitudes to be the same longitude; an arena must not, or a flank
        // that walks off the east side arrives behind the enemy in the west
        // and an archer on one wall is two steps from the far wall.
        WorldMap::arena(width, height)
    } else {
        WorldMap::new(width, height)
    };

    // --- landmass, from the world type asked for and the shape it landed on.
    // A Grand Canals II world is cut once and kept, rather than cut again
    // when the coast is painted: which layer of a canal a tile is in is
    // rolled along with the blocks, so asking a second time would not get the
    // same answer back. Every other type, [`grand_canals`] included, is the
    // same geometry every time it is asked.
    let blocks = (script == MapScript::GrandCanalsTwo).then(|| canal_blocks(&wm, poles, rng));
    let earth = script.is_fixed_geography().then(|| earth_tile_cache(&wm));
    let mut land = match &blocks {
        Some(plan) => plan.land.clone(),
        None if script.is_fixed_geography() => earth_land_from_cache(&wm, earth.as_ref().unwrap()),
        None => generate_land(&wm, script, poles, num_major_spawns, num_minor_spawns, rng),
    };

    // The dedicated continent scripts use their named regions as moving
    // plates. Plan those regions before relief so their seams can grow into
    // narrow rift belts, while preserving the legacy generation path for
    // other map scripts.
    let continental_rifts = matches!(script, MapScript::Continents | MapScript::SmallContinents);
    if continental_rifts {
        // Keep that planning on a clone: the legacy main-stream draw stays at
        // its original point below, so unrelated seeded geography does not
        // reroll.
        let mut continent_rng = rng.clone();
        assign_continents(&mut wm, &land, num_continents, &mut continent_rng);
    }

    let land_list: Vec<Pos> = land.iter().cloned().collect();

    // --- relief, then climate. The stock generator settles elevation first
    // (MountainsCliffs.lua) and only then paints biomes over it, because the
    // mountain fractal has to be free of the latitude bands to run across them.
    // A fixed-geography world skips both: its ranges and its climates are as
    // real as its coastlines and are read from the same grid, so there is no
    // fractal to cut and no latitude band to paint.
    let mut mountain_terrain = if script.is_fixed_geography() {
        paint_earth(&mut wm, &land, poles, earth.as_ref(), rng)
    } else {
        apply_tectonics(&mut wm, &land, script, continental_rifts, rng);
        assign_biomes(&mut wm, &land_list, poles, rng)
    };

    // A battlefield is temperate throughout. The latitude bands paint a
    // ten-row arena from pole to pole — half of it snow and tundra — but the
    // arena is not a world with a climate, it is fighting ground, so the
    // extremes are brought back to the open terrain the battle is fought on.
    if script == MapScript::Battlefield {
        for pos in &land_list {
            if let Some(tile) = wm.tiles.get_mut(pos) {
                match tile.terrain.as_str() {
                    "snow" | "tundra" => tile.terrain = "plains".into(),
                    "desert" => tile.terrain = "grassland".into(),
                    _ => {}
                }
            }
        }
        for terrain in mountain_terrain.values_mut() {
            match terrain.as_str() {
                "snow" | "tundra" => *terrain = "plains".into(),
                "desert" => *terrain = "grassland".into(),
                _ => {}
            }
        }
    }

    // --- coast. A shelf is one tile of shallow water plus the stock's three
    // expansion passes, each giving a quarter of the Ocean tiles that already
    // touch shallow water their own turn to become Coast. Shelves therefore
    // vary from one tile in the open sea to five or more in a broad bay,
    // instead of a uniform outline traced around every landmass.
    let coastal: Vec<Pos> = wm
        .tiles
        .iter()
        .filter(|(pos, t)| {
            t.terrain == "ocean" && wm.neighbors(**pos).iter().any(|n| land.contains(n))
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
                    && wm.neighbors(**pos).into_iter().any(|neighbor| {
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

    // A Grand Canals II canal is dug in three layers and the shelf pass knows
    // about only one of them, so both are laid back over what it painted: the
    // shelf off either bank is shallow whether or not the pass reached that
    // far in, and the channel between them is deep whether or not the pass
    // spilled shallow water across it. That is the whole shape of the world —
    // a galley may sail right round its own block, and nothing may cross to
    // the next one until the open sea is understood.
    if let Some(plan) = &blocks {
        for pos in &plan.shelf {
            if let Some(tile) = wm.tiles.get_mut(pos) {
                tile.terrain = "coast".into();
            }
        }
        for pos in &plan.channel {
            if let Some(tile) = wm.tiles.get_mut(pos) {
                tile.terrain = "ocean".into();
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
        for neighbor in wm.neighbors(pos).into_iter() {
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
    // Each one runs *downstream*, from a headwater in the highlands to the sea
    // it drains into, so it has the one mouth a river has. Walking the corner
    // graph (rather than the tile-center graph) keeps every consecutive segment
    // joined at a hex corner and never sends a channel through a tile.
    generate_rivers(&mut wm, &land_list, rng);

    // --- lakes and inland seas, in the stock order: the rivers already have
    // their outlets, so nothing that floods now can dam one. `add_lakes` only
    // creates water; `classify_lakes` then sorts every enclosed body on the
    // map — the ones it just made and the ones the coastline enclosed by
    // itself — into lakes and inland seas by area.
    //
    // A fixed-geography world floods nothing. Its lakes are in the grid its
    // coastline came from, so the one-in-forty pond roll would be putting
    // water in the middle of the Sahara on a map whose whole promise is that
    // its water is real. `classify_lakes` still runs: the Caspian, the Great
    // Lakes, Victoria and Baikal arrive as water the coastline encloses, and
    // sorting those by area is exactly what it is for.
    if !script.is_fixed_geography() && script != MapScript::Fjords {
        add_lakes(
            &mut wm,
            &mut land,
            large_lake_budget(script, num_continents),
            rng,
        );
    }
    classify_lakes(&mut wm);
    // Every later pass reads river adjacency from the finished river network.
    // In particular, floodplains are placed only after any edge that became a
    // water boundary has been removed, so they cannot outlive their river.
    remove_water_boundary_rivers(&mut wm);
    let land_list: Vec<Pos> = land.iter().cloned().collect();

    // --- tribal villages (goody huts), roughly 1 per 40 land tiles. The
    // battlefield has none: nothing on an arena exists to be discovered.
    if !script.is_battlefield() {
        for pos in &land_list {
            let t = &wm.tiles[pos];
            if t.terrain == "mountain" || t.has_river() {
                continue;
            }
            if rng.f64() < 0.025 {
                wm.tiles.get_mut(pos).unwrap().improvement = Some("goody_hut".into());
            }
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
            wm.neighbors(*position).into_iter().any(|neighbor| {
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
    // The battlefield keeps its mountains ordinary: an arena a dozen tiles
    // across has no room for eruptions rewriting the ground mid-battle.
    let volcano_target = if script.is_battlefield() {
        0
    } else {
        (land_list.len() / 180).max(1)
    };
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
        let mut foothills: Vec<Pos> = wm
            .neighbors(*volcano)
            .into_iter()
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
                && wm.neighbors(*position).into_iter().any(|neighbor| {
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
    let fissure_target = if script.is_battlefield() {
        0
    } else {
        (land_list.len() / 140).max(1)
    };
    let mut fissures = Vec::new();
    if continental_rifts {
        let rift_fissure_target = (fissure_target * CONTINENTAL_RIFT_FISSURE_NUMERATOR)
            .div_ceil(CONTINENTAL_RIFT_FISSURE_DENOMINATOR);
        // The full candidate list has already been shuffled. Stable sorting
        // keeps each class random while giving the continental rift its share.
        fissure_candidates.sort_by_key(|position| !continent_rift_band(&wm, *position));
        let rift_candidate_count =
            fissure_candidates.partition_point(|position| continent_rift_band(&wm, *position));
        for (index, position) in fissure_candidates.into_iter().enumerate() {
            if index < rift_candidate_count && fissures.len() >= rift_fissure_target {
                continue;
            }
            if index >= rift_candidate_count && fissures.len() >= fissure_target {
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
    } else {
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
    }

    // Polar sea ice occupies both Ocean and Coast. Latitude controls density,
    // leaving navigable gaps instead of drawing an artificial solid wall. A
    // world with no poles has no cold end for it to form on, so it forms none:
    // the ends of a poleless world are as open as its middle, which is most of
    // what the setting is for.
    let polar_water: Vec<Pos> = if poles.has_poles() && !script.is_battlefield() {
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

    // --- natural wonders: the shipped roster, drawn with the shipped odds.
    //
    // `NaturalWonderGenerator` does not work from a shortlist. It walks every
    // Natural Wonder in the database, keeps the ones with at least one legal
    // hex on this map, gives each survivor a single 0-99 roll, sorts on that
    // roll and plants the highest `NumNaturalWonders` of them. Two properties
    // follow, and both are the point of doing it this way: the draw is uniform
    // over the whole eligible roster, so a wonder that fits one hex is exactly
    // as likely as one that fits a thousand; and no wonder is guaranteed, so a
    // standard map showing five of thirty-four is a different five each game.
    // This pass used to draw from a fixed eight, which made those eight appear
    // in five games out of eight and the other twenty-six never.
    //
    // Placement then follows the footprint: `Features.Tiles` hexes laid out in
    // the wonder's shipped silhouette, so discovery, adjacency and yields
    // operate on every constituent hex rather than on one representative.
    // The stock generator also spreads wonders out, scoring a candidate plot
    // by its distance from wonders already drawn, so no two share a border and
    // one region never collects the map's whole allowance. Two wonders that
    // want the same biome
    // — Yosemite and Mount Everest both want high ground — otherwise settle
    // onto the same range and read as one oversized feature. The separation is
    // a preference, not a quota: it relaxes one ring at a time down to
    // `MIN_WONDER_SEPARATION` before a wonder places unconstrained, so a
    // cramped map still receives its full count.
    let survey = survey_wonder_sites(&wm);
    let roster: Vec<&str> = rules
        .features
        .iter()
        .filter(|(_, spec)| spec.natural_wonder)
        .map(|(name, _)| name.as_str())
        .collect();
    let wonder_masks: Vec<(u32, u32)> = roster
        .iter()
        .map(|wonder| {
            let placement = &rules.features[*wonder].placement;
            (
                wonder_terrain_mask(&placement.adjacent_terrain),
                wonder_terrain_mask(&placement.not_adjacent_terrain),
            )
        })
        .collect();
    // Keep the one-tile ground scan as the anchor pool. The exact silhouette
    // is checked when an anchor is attempted below; separating the cheap
    // roster-wide scan from that geometry preserves the generator's seeded
    // draw while ensuring no invalid footprint can actually be placed.
    let anchors: Vec<Vec<Pos>> = roster
        .iter()
        .enumerate()
        .map(|(index, wonder)| {
            let placement = &rules.features[*wonder].placement;
            wm.tiles
                .iter()
                .map(|(position, _)| *position)
                .filter(|position| {
                    wonder_anchor(
                        &wm,
                        placement,
                        *position,
                        &survey,
                        &mountain_terrain,
                        wonder_masks[index].0,
                        wonder_masks[index].1,
                    )
                })
                .collect()
        })
        .collect();
    // One roll each, highest first, ties broken by roster order so a seed
    // reproduces exactly. Wonders with no eligible ground anchor never enter
    // the draw, which is what makes a poleless desert world offer desert
    // wonders.
    let mut draw: Vec<(usize, usize)> = (0..roster.len())
        .filter(|index| !anchors[*index].is_empty())
        .map(|index| (rng.below(100), index))
        .collect();
    draw.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    // A map that asks for more wonders than the roster can seat still gets its
    // count: the ones with no legal anchor come last and are shaped in.
    let mut order: Vec<usize> = draw.into_iter().map(|(_, index)| index).collect();
    order.extend((0..roster.len()).filter(|index| anchors[*index].is_empty()));

    let mut placed_wonder_tiles: Vec<Pos> = Vec::new();
    for index in order.into_iter().take(num_natural_wonders) {
        let wonder = roster[index];
        let placement = &rules.features[wonder].placement;
        let footprint = placement.tiles.max(1);
        let water_tiles = placement.water_tiles.min(footprint.saturating_sub(1));
        // Which element the wonder belongs to. A shaped site — see below — has
        // to be of the same one, because rewriting an ocean into a desert to
        // seat Uluru would punch a hole in the map.
        let wants_water = placement
            .terrain
            .iter()
            .all(|terrain| matches!(terrain.as_str(), "coast" | "ocean"));
        // A true-start world arrives with Earth's own forests and rainforest
        // already down, so requiring bare ground would push half the roster
        // off its own address — the Giant's Causeway's headland is wooded and
        // so is Vesuvius. There, a wonder may grow through what grew on it;
        // what it may not grow through is another wonder.
        let unclaimed = |tile: &crate::world::Tile| {
            if script.is_fixed_geography() {
                tile.feature
                    .as_deref()
                    .and_then(|feature| rules.features.get(feature))
                    .is_none_or(|feature| !feature.natural_wonder)
            } else {
                tile.feature.is_none()
            }
        };
        let is_open = |tile: &crate::world::Tile| {
            let water = matches!(tile.terrain.as_str(), "coast" | "ocean");
            water == wants_water
                && unclaimed(tile)
                && tile.resource.is_none()
                && !(placement.no_river && tile.has_river())
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
        // `target` is the footprint being attempted, which is the wonder's own
        // everywhere but a true-start map — see the fixed-geography branch
        // below, where a wonder that cannot fit where it belongs keeps the
        // address and gives up the size.
        let cluster_from = |anchor: Pos, strict: bool, separation: i32, target: usize| {
            let water_here = water_tiles.min(target.saturating_sub(1));
            wonder_footprints(&wm, placement, anchor, target)
                .into_iter()
                .find(|cluster| {
                    let ground_is_valid = cluster.iter().enumerate().all(|(slot, position)| {
                        if !far_enough(*position, separation) {
                            return false;
                        }
                        let tile = &wm.tiles[position];
                        // The Giant's Causeway is the one wonder whose
                        // footprint spans the shoreline: its last hex steps
                        // from the headland into shallow water.
                        if slot >= target - water_here {
                            tile.terrain == "coast" && unclaimed(tile) && tile.resource.is_none()
                        } else if strict {
                            wonder_anchor(
                                &wm,
                                placement,
                                *position,
                                &survey,
                                &mountain_terrain,
                                wonder_masks[index].0,
                                wonder_masks[index].1,
                            )
                        } else {
                            is_open(tile)
                        }
                    });
                    ground_is_valid
                        && (placement.shape != crate::rules::WonderShape::CoastalTriangle
                            || target != placement.tiles
                            || coastal_triangle_is_valid(&wm, cluster))
                })
        };
        // Very unusual seeds can lack the biome even of a wonder that had an
        // anchor before its neighbours were drawn. Keep the correct footprint
        // and map-size count by shaping an otherwise empty exact footprint
        // into the wonder's own terrain.
        let shaped_sites: Vec<Pos> = wm
            .tiles
            .iter()
            .filter(|(_, tile)| is_open(tile))
            .map(|(position, _)| *position)
            .collect();
        // Sites are tried in order of how far each one departs from the ideal:
        // the wonder's own ground at the widest spacing, then narrower rings,
        // then the shaped fallback down the same ladder. Rewriting a region
        // into the wonder's terrain is the larger departure of the two, so the
        // whole strict ladder is exhausted first. Dropping the separation
        // altogether is worse than either and comes last, once no pool can
        // seat this wonder `MIN_WONDER_SEPARATION` hexes from its neighbours.
        let pools = [(&anchors[index], true), (&shaped_sites, false)];
        let mut attempts: Vec<(&Vec<Pos>, bool, i32)> = Vec::new();
        for (sites, strict) in pools {
            for separation in (MIN_WONDER_SEPARATION..=PREFERRED_WONDER_SEPARATION).rev() {
                attempts.push((sites, strict, separation));
            }
        }
        for (sites, strict) in pools {
            attempts.push((sites, strict, 1));
        }
        let mut footprint_tiles = None;
        // On a true-start map a natural wonder is not placed, it is found.
        // Every one of the roster is a real place with a real address, so the
        // search starts at the hex nearest that address and stays within
        // `EARTH_WONDER_REACH` of it. The ground-and-spacing ladder below is
        // not consulted: Uluru's own ground is already desert and the Great
        // Barrier Reef's is already coast, and where the grid and the
        // placement rule disagree the grid is the one that is real. `is_open`
        // still holds, so the wonder lands in its own element and never on
        // top of another wonder.
        //
        // Size gives way before address does. Bohol is one hex on most of
        // these worlds and the Chocolate Hills cover four; Milford Sound is a
        // notch in a two-hex island. A smaller wonder in the right fjord is a
        // truer map than a whole one in Tasmania, so the footprint is tried
        // full first and then shrunk, and only if even one hex will not fit
        // does the wonder fall through to the ordinary search.
        if script.is_fixed_geography() {
            if let Some((longitude, latitude)) = earth_wonder_site(wonder) {
                let toward = earth_direction(longitude, latitude);
                let home = nearest_tile(&wm, longitude, latitude);
                let mut by_distance: Vec<Pos> = shaped_sites
                    .iter()
                    .copied()
                    .filter(|position| {
                        home.is_none_or(|home| {
                            wm.distance(home, *position) <= 4 * EARTH_WONDER_REACH
                        })
                    })
                    .collect();
                by_distance.sort_by(|a, b| {
                    dot(wm.direction(*b), toward)
                        .partial_cmp(&dot(wm.direction(*a), toward))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                'found: for reach in [1, 2, 4].map(|step| step * EARTH_WONDER_REACH) {
                    for size in (1..=footprint).rev() {
                        for anchor in by_distance.iter().copied() {
                            if home.is_some_and(|home| wm.distance(home, anchor) > reach) {
                                continue;
                            }
                            footprint_tiles = cluster_from(anchor, false, 1, size);
                            if footprint_tiles.is_some() {
                                break 'found;
                            }
                        }
                    }
                }
            }
        }
        for (sites, strict, separation) in attempts {
            if footprint_tiles.is_some() {
                break;
            }
            let mut cands: Vec<Pos> = sites
                .iter()
                .copied()
                .filter(|position| far_enough(*position, separation))
                .collect();
            while !cands.is_empty() && footprint_tiles.is_none() {
                let anchor = cands.swap_remove(rng.below(cands.len()));
                footprint_tiles = cluster_from(anchor, strict, separation, footprint);
            }
            if footprint_tiles.is_some() {
                break;
            }
        }
        if let Some(cluster) = footprint_tiles {
            // The stock generator calls `ResetTerrain` on every hex it plants
            // a wonder on, which normalises the ground under it to what the
            // wonder is drawn standing on. Do the same, so a shaped Everest is
            // a mountain and a shaped Uluru is desert rather than whatever the
            // fallback happened to land on.
            let ground = placement.terrain.first().cloned();
            for position in cluster {
                let valid_ground = wonder_ground(
                    placement,
                    &wm.tiles[&position],
                    mountain_terrain.get(&position),
                );
                let tile = wm.tiles.get_mut(&position).unwrap();
                let water = matches!(tile.terrain.as_str(), "coast" | "ocean");
                match ground.as_deref() {
                    Some(terrain) if !water && !valid_ground => {
                        tile.terrain = terrain.into();
                        tile.hills = placement.hills.unwrap_or(false) && terrain != "mountain";
                        if terrain == "mountain" {
                            if let Some(base) = placement.mountain_terrain.first() {
                                mountain_terrain.insert(position, *base);
                            }
                        } else {
                            mountain_terrain.remove(&position);
                        }
                    }
                    _ => {}
                }
                if tile.terrain == "mountain" {
                    tile.hills = false;
                }
                tile.feature = Some(wonder.into());
                tile.resource = None;
                tile.improvement = None;
                placed_wonder_tiles.push(position);
            }
        }
    }

    // --- resources. The battlefield places none — no lottery, no strategic
    // floors, no artifacts: its ground is terrain to fight over, not to work.
    let all_pos: Vec<Pos> = if script.is_battlefield() {
        Vec::new()
    } else {
        wm.tiles.keys().cloned().collect()
    };
    for pos in all_pos {
        let (terrain, feature) = {
            let t = &wm.tiles[&pos];
            (t.terrain, t.feature)
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
            let valid: Vec<Name> = rules
                .resources
                .iter()
                .filter(|(_, s)| {
                    // The shipped placement is a union: a listed feature on
                    // the tile, or a listed terrain on a featureless tile —
                    // and hills-only spawns (Sheep) respect the tile's form.
                    let by_feature = feature
                        .as_ref()
                        .map(|feature| s.feature.contains(feature))
                        .unwrap_or(false);
                    let by_terrain = feature.is_none() && s.terrain.contains(&terrain);
                    (by_feature || by_terrain) && s.hills.is_none_or(|want| want == hills)
                })
                .map(|(name, _)| *name)
                .collect();
            let water = rules.is_water(&wm.tiles[&pos]);
            if let Some(pick) = weighted_resource_pick(rules, &valid, water, rng) {
                wm.tiles.get_mut(&pos).unwrap().resource = Some(pick);
            }
        }
    }

    if !script.is_battlefield() {
        place_strategic_quotas(
            rules,
            &mut wm,
            &land,
            num_major_spawns,
            &BTreeSet::new(),
            rng,
        );
    }

    if continental_rifts {
        // `assign_continents` historically used one phase draw here. Its early
        // tectonic plan uses a cloned stream, so retain the main-stream draw
        // at the original point and preserve every later random roll.
        if !land.is_empty() && num_continents > 0 {
            let _ = rng.below(land.len());
        }
    } else {
        assign_continents(&mut wm, &land, num_continents, rng);
    }

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
            wm.neighbors(**position).into_iter().any(|neighbor| {
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

    // --- the arena's own ground, laid over whatever the world passes made of
    // it. This runs before the starts are chosen so that both sides are
    // seated on the field they will actually fight over.
    if script == MapScript::Battlefield {
        paint_battlefield_ground(&mut wm, rng);
    } else if script == MapScript::Trafalgar {
        crate::trafalgar::paint_chart(&mut wm);
    } else if script.is_scenario() {
        let scenario = crate::historical_scenarios::by_script(script)
            .expect("every scenario map script has a catalogue row");
        let plan = crate::historical_terrain::by_id(scenario.id)
            .expect("every catalogue battle has a drawn chart");
        crate::historical_terrain::paint(&mut wm, plan);
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
        // Nobody has ever founded a city on an ice cap. A true-start world
        // carries a real Antarctica and a real Greenland, which between them
        // are a tenth of its land; left in the ground the regions are cut from
        // they take a tenth of the city-states with them and seat them where
        // no city has stood — and a region with no other ground to offer seats
        // one there whatever the candidate pool says. A rolled world's snow is
        // a thin polar fringe rather than a continent, and is left alone.
        .filter(|pos| !script.is_fixed_geography() || wm.tiles[pos].terrain != "snow")
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
                        && tile.feature.as_deref() != Some("oasis")
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
    // A four-seat Tenins Ball is the stock game, and its layout is part of
    // that map type's promise: one civilization at each end of each lobe, a
    // handful of tiles in from the seam. The regional model would seat them
    // at each half-lobe's center of fertility instead — the right general
    // answer and the wrong shape for this world.
    let lobe_ends = (script == MapScript::TeninsBall && num_major_spawns == 4)
        .then(|| tennis_ball_major_starts(&wm, &major_pool))
        .flatten();
    // A two-seat Tactics planet's layout is its type's promise too: the sides
    // open at the farthest usable plots on one landmass, on opposite faces of
    // the world but joined by ground.
    //
    // Both ends come out of a *single* connected component, never out of the
    // world-wide candidate pool, and that is the whole guarantee: two plots in
    // one component of `passable` have a walkable line between them, so the
    // side that opens on the far face can march to the other rather than
    // needing a boat it has not got. `components` is ordered largest first, so
    // the first one that can seat both sides is the main continent. Only if no
    // landmass at all holds two candidate plots does this widen — and it
    // widens to the largest component itself rather than to the pool, because
    // a start on bare ground is a smaller compromise than a start the enemy
    // cannot reach.
    //
    // At sea the guarantee inverts, and so does the component it is taken
    // from. The naval globe seats the two sides on *different* islands by
    // design — that is what makes it a naval battle rather than a land one
    // with water around it — so requiring one landmass would defeat the map
    // type. What has to connect instead is the water between them: two fleets
    // launched into seas that do not join could never meet, which is the same
    // unwinnable battle the land rule exists to prevent, reached from the
    // other element. So the pool is every candidate plot whose own shore
    // touches the world ocean, meaning the largest connected body of passable
    // water, and the two ends are the farthest apart of those.
    let planet_pool = (script.is_planet_battlefield() && num_major_spawns == 2)
        .then(|| {
            if script.is_naval_battlefield() {
                let sea: BTreeSet<Pos> = wm
                    .tiles
                    .iter()
                    .filter(|(_, tile)| rules.is_water(tile) && rules.is_passable(tile))
                    .map(|(pos, _)| *pos)
                    .collect();
                let ocean = connected_components(&wm, &sea).into_iter().next()?;
                let pool: BTreeSet<Pos> = major_pool
                    .iter()
                    .copied()
                    .filter(|pos| wm.neighbors(*pos).into_iter().any(|n| ocean.contains(&n)))
                    .collect();
                return (pool.len() >= 2).then_some(pool);
            }
            components
                .iter()
                .find_map(|component| {
                    let pool: BTreeSet<Pos> = major_pool.intersection(component).copied().collect();
                    (pool.len() >= 2).then_some(pool)
                })
                .or_else(|| {
                    components
                        .iter()
                        .find(|component| component.len() >= 2)
                        .cloned()
                })
        })
        .flatten();
    let planet_ends = planet_pool
        .as_ref()
        .and_then(|pool| tactics_planet_major_starts(&wm, pool));
    // A two-seat bounded battlefield instead uses opposite corners or ends of
    // its rectangle. More seats than two fall back to the regional model.
    let arena_ends = (script == MapScript::Battlefield && num_major_spawns == 2)
        .then(|| battlefield_major_starts(&wm, &major_pool))
        .flatten();
    // A scenario's seats are neither chosen nor dealt: each side opens where
    // it actually was, and which of the two is seat 0 decides who moves first.
    // So this is answered before the shuffle below and skips it — swapping
    // Nelson and Villeneuve would be a different battle with the same name.
    let scenario_seats = if script.is_scenario() && num_major_spawns == 2 {
        if script == MapScript::Trafalgar {
            crate::trafalgar::major_starts(&wm)
        } else {
            let scenario = crate::historical_scenarios::by_script(script)
                .expect("every scenario map script has a catalogue row");
            let plan = crate::historical_terrain::by_id(scenario.id)
                .expect("every catalogue battle has a drawn chart");
            let afloat = crate::historical_terrain::sides_afloat(rules, scenario);
            crate::historical_terrain::major_starts(&wm, plan, afloat)
        }
    } else {
        None
    };
    let mut spawns = if let Some(starts) = scenario_seats {
        starts
    } else if let Some(mut starts) = lobe_ends.or(planet_ends).or(arena_ends) {
        // Seat order should not correlate with the order the ends are listed.
        for index in (1..starts.len()).rev() {
            let other = rng.below(index + 1);
            starts.swap(index, other);
        }
        starts
    } else if script.is_true_start() {
        // Earth does not divide into regions: the whole point of the script is
        // that Rome opens in Italy and the Aztecs open in Mexico, however
        // lopsided that leaves the continents.
        //
        // Nor does a homeland have to be bare grassland to be a homeland. The
        // pool every other script draws from wants open ground and only widens
        // when it runs short, which on this map would move Rome out of its own
        // wooded hills and Cusco off its own mountainside to find some. Asking
        // for more seats than the world has tiles takes the wider pool
        // outright: every passable tile that is not a wonder or a village.
        let candidates = candidates_for(&passable, usize::MAX);
        let homelands: Vec<TrueStartPoint> = (0..num_major_spawns)
            .map(|index| {
                leader_starts
                    .get(index)
                    .copied()
                    .unwrap_or_else(|| leader_roster::legacy_true_start(index))
            })
            .collect();
        historic_major_spawns(&wm, &candidates, &homelands, num_major_spawns)
    } else {
        let mut seated = regional_starts(
            rules,
            &wm,
            &major_regions,
            &major_pool,
            &fertility,
            StartBuffers {
                foreign: &[],
                foreign_buffer: MAJOR_START_BUFFER,
                own_buffer: MAJOR_START_BUFFER,
            },
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
        archipelago_minor_regions(&wm, &passable, &fertility, &spawns, num_minor_spawns)
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
    // A city-state on True Start Earth belongs where it stood, for the same
    // reason Rome does. The regional cut above divides each civilization's own
    // cell so everyone has neighbours to court, which is the right answer on a
    // rolled world and the wrong one here: it put Jerusalem in Siberia and
    // Cahokia in the Sahara as readily as anywhere else, on the one map whose
    // whole promise is that places are where they are.
    //
    // The majors are already down, so their hexes are the ones a city-state
    // must not take; `historic_minor_spawns` seats around them the way
    // `historic_major_spawns` seats around each other. An identity the roster
    // has no coordinates for keeps the regional seat it would have had.
    let historic_minors = script
        .is_true_start()
        .then(|| {
            (!minor_starts.is_empty() && minor_starts.iter().any(Option::is_some)).then(|| {
                let candidates = candidates_for(&passable, usize::MAX);
                historic_minor_spawns(&wm, &candidates, minor_starts, &majors, num_minor_spawns)
            })
        })
        .flatten();
    if let Some(minors) = historic_minors {
        spawns.extend(minors);
    } else {
        let minors = regional_starts(
            rules,
            &wm,
            &minor_regions,
            &minor_pool,
            &fertility,
            StartBuffers {
                foreign: &majors,
                foreign_buffer: MINOR_MAJOR_BUFFER,
                own_buffer: MINOR_MINOR_BUFFER,
            },
        );
        spawns.extend(minors.into_iter().map(|(_, start)| start));
    }
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
    if !script.is_battlefield() {
        place_strategic_quotas(rules, &mut wm, &land, num_major_spawns, &occupied, rng);
        place_artifact_quotas(rules, &mut wm, num_major_spawns, &occupied, rng);
    }

    // CIVVIS grows ordinary features before it draws Natural Wonders, the
    // reverse of Firaxis' effective order for this rule. A later Woods,
    // Floodplains, or Wonder can therefore invalidate an Oasis that was legal
    // when rolled. Retire only those Oases after generation is final; this
    // enforces `NoAdjacentFeatures` without changing the seeded draw stream,
    // displacing a Natural Wonder, or redrawing a long-established map.
    let invalid_oases: Vec<Pos> = wm
        .tiles
        .iter()
        .filter(|(position, tile)| {
            tile.feature.as_deref() == Some("oasis") && !placed_oasis_is_valid(&wm, **position)
        })
        .map(|(position, _)| *position)
        .collect();
    for position in invalid_oases {
        wm.tiles.get_mut(&position).unwrap().feature = None;
    }
    // The old-deposit pass above predates separate Lake terrain and can stage
    // Volcanic Soil there while preserving the RNG sequence on which wonder,
    // resource, and start placement depend. Retire any deposit whose final
    // terrain is not one of Gathering Storm's valid flat/hills land types only
    // after every seeded generation decision is complete.
    for tile in wm.tiles.values_mut() {
        if tile.feature.as_deref() == Some("volcanic_soil") && !volcanic_soil_valid_terrain(tile) {
            tile.feature = None;
        }
    }
    (wm, spawns)
}

/// Draw one eligible resource using the game's separate land and sea weights.
///
/// A zero weight is not an equally likely entry: it means the ordinary map
/// lottery does not place that resource. This keeps artifacts out of the early
/// pass so their per-civilization quota owns them, and preserves the extreme
/// Fish (23) / Whale (1) difference in the shipped sea table.
fn weighted_resource_pick(
    rules: &Rules,
    candidates: &[Name],
    water: bool,
    rng: &mut Rng,
) -> Option<Name> {
    let total = candidates
        .iter()
        .map(|name| {
            let spec = &rules.resources[name];
            if water {
                spec.sea_frequency as usize
            } else {
                spec.frequency as usize
            }
        })
        .sum::<usize>();
    if total == 0 {
        return None;
    }
    let mut draw = rng.below(total);
    for name in candidates {
        let spec = &rules.resources[name];
        let weight = if water {
            spec.sea_frequency as usize
        } else {
            spec.frequency as usize
        };
        if draw < weight {
            return Some(*name);
        }
        draw -= weight;
    }
    unreachable!("a positive resource-weight total must select a candidate")
}

/// Hexes the generator tries to keep between any two natural wonders, and the
/// floor it will not go below while a spacing-respecting site still exists.
/// `NaturalWonderGenerator` spreads the stock roster over the whole map rather
/// than letting two of them share a mountain range or a reef; the floor of 3
/// is the part that matters most, because it is what stops a pair from reading
/// as one oversized feature.
///
/// The preferred figure is the shipped `Features.MinDistanceNW`, which is 8 on
/// every Natural Wonder in the database. It is a preference here and not a
/// quota: the scaled map sizes ask for more wonders than an 8-hex lattice fits,
/// and a map that asks for a wonder is entitled to receive one.
const PREFERRED_WONDER_SEPARATION: i32 = 8;
const MIN_WONDER_SEPARATION: i32 = 3;

/// Where every natural wonder in the roster actually is, in
/// `(longitude, latitude)` degrees, for the one map script that can put them
/// there. A true-start Earth still draws only its map size's allowance from
/// the same roll every other script uses, so *which* wonders a game gets is
/// still rolled — but each one it gets is where it belongs, not on the first
/// patch of the right ground the search happened to find.
///
/// Three of these are places only in the sense that the story puts them
/// somewhere: the Bermuda Triangle is the vertex off Bermuda itself, the
/// Fountain of Youth is where Ponce de Leon was said to have looked for it,
/// and Paititi is the stretch of Amazon headwater the legend places it in.
const EARTH_WONDERS: [(&str, f64, f64); 34] = [
    ("great_barrier_reef", 146.8, -18.3),
    ("crater_lake", -122.11, 42.94),
    ("pantanal", -56.8, -17.5),
    ("uluru", 131.04, -25.34),
    ("yosemite", -119.54, 37.75),
    ("dead_sea", 35.5, 31.5),
    ("mount_everest", 86.93, 27.99),
    ("pamukkale", 29.12, 37.92),
    ("torres_del_paine", -73.0, -50.98),
    ("eye_of_the_sahara", -11.4, 21.12),
    ("zhangye_danxia", 100.13, 38.92),
    ("ha_long_bay", 107.05, 20.9),
    ("cliffs_of_dover", 1.35, 51.13),
    ("giants_causeway", -6.51, 55.24),
    ("galapagos_islands", -90.4, -0.6),
    ("matterhorn", 7.66, 45.98),
    ("kilimanjaro", 37.35, -3.07),
    ("piopiotahi", 167.92, -44.62),
    ("ik_kil", -88.57, 20.68),
    ("gobustan", 49.4, 40.1),
    ("ubsunur_hollow", 92.8, 50.3),
    ("mato_tipila", -104.72, 44.59),
    ("delicate_arch", -109.5, 38.74),
    ("chocolate_hills", 124.14, 9.92),
    ("vesuvius", 14.43, 40.82),
    ("lake_retba", -17.23, 14.84),
    ("bermuda_triangle", -65.0, 27.0),
    ("eyjafjallajokull", -19.62, 63.63),
    ("fountain_of_youth", -81.31, 29.9),
    ("lysefjord", 6.2, 59.0),
    ("paititi", -71.0, -12.5),
    ("mount_roraima", -60.76, 5.14),
    ("tsingy_de_bemaraha", 44.75, -18.7),
    ("sahara_el_beyda", 27.8, 27.2),
];

/// How far from a wonder's true address the search walks before it will
/// consider a smaller wonder, and — at four times this — before it gives up on
/// the address entirely.
///
/// A wonder whose own hex is taken should move to the next hill along, and
/// past that it is better off smaller than elsewhere. The outer ring exists
/// for the one case where even a single hex will not do: the Giant's Causeway
/// may not stand on a river, and the two islands it belongs to are small
/// enough that a seed can put a river across every coastal hex of them.
const EARTH_WONDER_REACH: i32 = 3;

/// Where a natural wonder is on Earth, if it is one of the ones that is.
fn earth_wonder_site(wonder: &str) -> Option<(f64, f64)> {
    EARTH_WONDERS
        .iter()
        .find(|(name, _, _)| *name == wonder)
        .map(|(_, longitude, latitude)| (*longitude, *latitude))
}

/// Base terrains a Natural Wonder placement rule can name, in bit order.
/// `mountain` covers every coloured `TERRAIN_*_MOUNTAIN` variant.
const WONDER_TERRAIN_BITS: [&str; 9] = [
    "grassland",
    "plains",
    "desert",
    "tundra",
    "snow",
    "coast",
    "ocean",
    "mountain",
    "lake",
];

fn wonder_terrain_bit(terrain: &str) -> u32 {
    match WONDER_TERRAIN_BITS.iter().position(|name| *name == terrain) {
        Some(index) => 1 << index,
        None => 0,
    }
}

fn wonder_terrain_mask(terrains: &[Name]) -> u32 {
    terrains
        .iter()
        .fold(0, |mask, terrain| mask | wonder_terrain_bit(terrain))
}

/// What a placement rule needs to know about the ring around a hex, read once
/// per hex rather than once per hex per wonder. Thirty-four rosters' worth of
/// neighbour walks is the difference between a scan that costs nothing and one
/// that shows up in a map-generation profile.
#[derive(Clone, Copy)]
struct WonderSite {
    /// Bit per neighbouring base terrain, indexed by `WONDER_TERRAIN_BITS`.
    terrains: u32,
    /// Whether any neighbour carries a feature at all (`NoAdjacentFeatures`).
    any_feature: bool,
    /// Hexes of open water between this hex and the nearest land, for the
    /// wonders that ship `MinDistanceLand` / `MaxDistanceLand`. Land is 0.
    land_distance: i32,
}

/// Read the ring around every hex once, so the roster-wide eligibility scan
/// below is a handful of integer tests per wonder per hex.
fn survey_wonder_sites(wm: &WorldMap) -> Vec<WonderSite> {
    let water = |terrain: &str| matches!(terrain, "coast" | "ocean");
    let positions: Vec<Pos> = wm.tiles.keys().copied().collect();
    let mut survey = Vec::with_capacity(positions.len());
    let mut frontier: VecDeque<usize> = VecDeque::new();
    for (index, (position, tile)) in wm.tiles.iter().enumerate() {
        let mut terrains = 0;
        let mut any_feature = false;
        for neighbor in wm.neighbors(*position) {
            let Some(other) = wm.tiles.get(&neighbor) else {
                continue;
            };
            terrains |= wonder_terrain_bit(&other.terrain);
            any_feature |= other.feature.is_some();
        }
        let land = !water(&tile.terrain);
        if land {
            frontier.push_back(index);
        }
        survey.push(WonderSite {
            terrains,
            any_feature,
            land_distance: if land { 0 } else { i32::MAX },
        });
    }
    // One multi-source breadth-first walk out from the coastline gives every
    // water hex its distance to land, which is what the offshore wonders are
    // placed by: the Great Barrier Reef hugs the shore, the Galapagos do not.
    while let Some(index) = frontier.pop_front() {
        let distance = survey[index].land_distance;
        for neighbor in wm.neighbors(positions[index]) {
            let Some(neighbor_index) = wm.tiles.index_of(neighbor) else {
                continue;
            };
            if survey[neighbor_index].land_distance > distance + 1 {
                survey[neighbor_index].land_distance = distance + 1;
                frontier.push_back(neighbor_index);
            }
        }
    }
    survey
}

/// Whether this hex is ground the wonder can stand on, ignoring its
/// surroundings. Every hex of a multi-hex wonder has to pass this.
fn wonder_ground(
    placement: &crate::rules::FeaturePlacement,
    tile: &crate::world::Tile,
    mountain_terrain: Option<&Name>,
) -> bool {
    if tile.feature.is_some() || tile.resource.is_some() {
        return false;
    }
    if !placement
        .terrain
        .iter()
        .any(|terrain| terrain == &tile.terrain)
    {
        return false;
    }
    if placement.hills.is_some_and(|hills| hills != tile.hills) {
        return false;
    }
    if tile.terrain == "mountain"
        && !placement.mountain_terrain.is_empty()
        && mountain_terrain.is_none_or(|terrain| !placement.mountain_terrain.contains(terrain))
    {
        return false;
    }
    !(placement.no_river && tile.has_river())
}

/// Whether the wonder can be anchored here: the ground test plus everything
/// its rule says about the ring around it.
fn wonder_anchor(
    wm: &WorldMap,
    placement: &crate::rules::FeaturePlacement,
    position: Pos,
    survey: &[WonderSite],
    mountain_terrain: &BTreeMap<Pos, Name>,
    adjacent_terrain_mask: u32,
    not_adjacent_terrain_mask: u32,
) -> bool {
    let Some(index) = wm.tiles.index_of(position) else {
        return false;
    };
    let (Some(tile), Some(site)) = (wm.tiles.get(&position), survey.get(index)) else {
        return false;
    };
    if !wonder_ground(placement, tile, mountain_terrain.get(&position)) {
        return false;
    }
    if !placement.adjacent_terrain.is_empty() && site.terrains & adjacent_terrain_mask == 0 {
        return false;
    }
    if site.terrains & not_adjacent_terrain_mask != 0 {
        return false;
    }
    if placement.no_adjacent_features && site.any_feature {
        return false;
    }
    if let Some([near, far]) = placement.land_distance {
        if site.land_distance < near || site.land_distance > far {
            return false;
        }
    }
    if !placement.adjacent_feature.is_empty() || !placement.avoid_feature.is_empty() {
        let mut wanted = placement.adjacent_feature.is_empty();
        for neighbor in wm.neighbors(position) {
            let Some(feature) = wm.tiles.get(&neighbor).and_then(|t| t.feature.as_deref()) else {
                continue;
            };
            if placement.avoid_feature.iter().any(|name| name == feature) {
                return false;
            }
            wanted |= placement
                .adjacent_feature
                .iter()
                .any(|name| name == feature);
        }
        if !wanted {
            return false;
        }
    }
    true
}

/// Continue through `next` in the same direction as the edge from `from`.
/// Wonder sites never use the twelve pentagons, but returning `None` for an
/// odd-degree tile keeps the geometry honest if a custom water map exposes
/// one.
fn straight_wonder_step(wm: &WorldMap, from: Pos, next: Pos) -> Option<Pos> {
    let back = wm.direction_to(next, from)?;
    let degree = wm.neighbors(next).len();
    (degree > 0 && degree.is_multiple_of(2))
        .then(|| wm.step(next, back + degree / 2))
        .flatten()
}

/// Every rotation of a wonder's shipped footprint with `anchor` as its first
/// tile. The shape, not a greedy growth walk, decides all remaining tiles.
fn wonder_footprints(
    wm: &WorldMap,
    placement: &crate::rules::FeaturePlacement,
    anchor: Pos,
    target: usize,
) -> Vec<Vec<Pos>> {
    use crate::rules::WonderShape;

    let shape = if target == placement.tiles.max(1) {
        placement.shape
    } else {
        // True-start maps may shrink a landmark before moving it away from its
        // real address. A smaller remnant keeps the compact stock silhouette.
        match target {
            0 | 1 => WonderShape::Single,
            2 => WonderShape::Adjacent,
            3 => WonderShape::Triangle,
            _ => WonderShape::Diamond,
        }
    };
    let degree = wm.neighbors(anchor).len();
    let mut footprints = Vec::new();
    let mut push = |candidate: Vec<Pos>| {
        if candidate.len() == target
            && candidate
                .iter()
                .all(|position| wm.tiles.contains_key(position))
            && candidate.iter().copied().collect::<BTreeSet<_>>().len() == target
            && !footprints.contains(&candidate)
        {
            footprints.push(candidate);
        }
    };

    if shape == WonderShape::Single {
        push(vec![anchor]);
        return footprints;
    }
    for heading in 0..degree {
        let Some(first) = wm.step(anchor, heading) else {
            continue;
        };
        match shape {
            WonderShape::Single => unreachable!(),
            WonderShape::Adjacent => push(vec![anchor, first]),
            WonderShape::Triangle | WonderShape::CoastalTriangle => {
                let Some(second) = wm.step(anchor, heading + 1) else {
                    continue;
                };
                if wm.direction_to(first, second).is_some() {
                    push(vec![anchor, first, second]);
                }
            }
            WonderShape::Diamond => {
                let Some(second) = wm.step(anchor, heading + 1) else {
                    continue;
                };
                if wm.direction_to(first, second).is_none() {
                    continue;
                }
                for corner in wm.neighbors(first) {
                    if corner != anchor && wm.direction_to(second, corner).is_some() {
                        push(vec![anchor, first, second, corner]);
                    }
                }
            }
            WonderShape::Straight => {
                if let Some(second) = straight_wonder_step(wm, anchor, first) {
                    push(vec![anchor, first, second]);
                }
            }
            WonderShape::Roraima => {
                let Some(second) = straight_wonder_step(wm, anchor, first) else {
                    continue;
                };
                let Some(side) = wm.step(anchor, (heading + degree - 1) % degree.max(1)) else {
                    continue;
                };
                if wm.direction_to(first, side).is_some() {
                    push(vec![anchor, first, second, side]);
                }
            }
        }
    }
    footprints
}

/// Firaxis's `PLACEMENT_PIOPIOTAHI` layout: a land triangle backed by land
/// with a three-hex water frontage. Lysefjord reuses the same placement.
fn coastal_triangle_is_valid(wm: &WorldMap, footprint: &[Pos]) -> bool {
    if footprint.len() != 3 {
        return false;
    }
    let [anchor, first, second] = [footprint[0], footprint[1], footprint[2]];
    if wm.neighbors(anchor).into_iter().any(|position| {
        wm.get(position)
            .is_none_or(|tile| matches!(tile.terrain.as_str(), "coast" | "ocean" | "lake"))
    }) {
        return false;
    }
    let (Some(first_front), Some(second_front)) = (
        straight_wonder_step(wm, anchor, first),
        straight_wonder_step(wm, anchor, second),
    ) else {
        return false;
    };
    let Some(middle_front) = wm
        .neighbors(first)
        .into_iter()
        .find(|position| *position != anchor && wm.direction_to(second, *position).is_some())
    else {
        return false;
    };
    [first_front, middle_front, second_front]
        .into_iter()
        .all(|position| wm.get(position).is_some_and(|tile| tile.terrain == "coast"))
}

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

/// The knobs that make a named map's mountains read as that map rather than
/// as a different coastline painted under the stock relief pass.
#[derive(Clone, Copy)]
struct TectonicProfile {
    plates: usize,
    mountain_percentile: u32,
    foothills_percentile: u32,
    pass_percentile: u32,
    coastal_demote_numerator: usize,
    coastal_demote_denominator: usize,
    /// A share of every tile on the map, rather than a share of land. Fjords
    /// names its three categories this way: 40% ordinary land, 20% mountains,
    /// and 40% water.
    exact_mountain_percent: Option<usize>,
}

/// A continental rift lowers its mountain cutoff by eight percentiles. With
/// the stock normal-age cutoff of ninety-four that is eighty-six; Continents'
/// deliberately denser base profile gets the matching eighty-four so the
/// seam remains visibly more tectonic without becoming an impassable wall.
const CONTINENTAL_RIFT_MOUNTAIN_PERCENTILE_DELTA: u32 = 8;

/// Most fissures should read as part of a continental rift, while leaving a
/// minority for other mountain systems such as isolated volcanic ranges.
const CONTINENTAL_RIFT_FISSURE_NUMERATOR: usize = 2;
const CONTINENTAL_RIFT_FISSURE_DENOMINATOR: usize = 3;

/// A land tile touches another named continent.
fn continent_boundary(wm: &WorldMap, position: Pos) -> bool {
    let Some(continent) = wm.tiles.get(&position).and_then(|tile| tile.continent) else {
        return false;
    };
    wm.neighbors(position).into_iter().any(|neighbor| {
        wm.tiles
            .get(&neighbor)
            .and_then(|tile| tile.continent)
            .is_some_and(|other| other != continent)
    })
}

/// The one-hex-wide continental seam. Narrowing the rift to actual border
/// plots keeps maps with many named regions from losing too much capital land.
fn continent_rift_band(wm: &WorldMap, position: Pos) -> bool {
    continent_boundary(wm, position)
}

/// Elevation, the way `MountainsCliffs.lua` builds it: two fractal fields,
/// the mountain one with tectonic plate boundaries woven through it, cut at
/// percentiles. Mountains therefore arrive as ranges following a collision
/// line, ringed by their own foothills, rather than as short random walks;
/// hills additionally come in clumps wherever the hills field sits inside one
/// of its two bands.
fn tectonic_profile(script: MapScript) -> TectonicProfile {
    let standard = TectonicProfile {
        // `MountainsCliffs.lua` weaves nine tectonic plates through the field
        // whatever the map size; the ridges they collide along are the ranges.
        plates: 9,
        mountain_percentile: (97 - WORLD_AGE) as u32,
        foothills_percentile: (91 - 2 * WORLD_AGE) as u32,
        pass_percentile: (91 - 2 * WORLD_AGE) as u32,
        coastal_demote_numerator: 9,
        coastal_demote_denominator: 10,
        exact_mountain_percent: None,
    };
    match script {
        // Normal Continents used the top six percent of its ridged field.
        // Taking the top eight instead is one third more raw mountain range;
        // the same coast erosion and passes still keep the resulting shores
        // and routes playable.
        MapScript::Continents => TectonicProfile {
            mountain_percentile: 92,
            ..standard
        },
        // Fjords needs a denser network of shorter and longer ranges, many
        // more passable breaks, and mountains that meet its advertised share.
        // Coastal peaks stay: water against a range is the point of a fjord.
        MapScript::Fjords => TectonicProfile {
            // Keep the raw mountain cut close to its final share. The old
            // profile raised the bottom 40% of the ridged field and then
            // discarded half of it to reach 20%; that made ranges read as
            // broad blocks. A higher cut leaves the collision lines narrow
            // and lets the exact-share pass add only their strongest fringes.
            plates: 6,
            mountain_percentile: 72,
            foothills_percentile: 84,
            pass_percentile: 88,
            coastal_demote_numerator: 0,
            coastal_demote_denominator: 1,
            exact_mountain_percent: Some(20),
        },
        // A battlefield has no mountains: on a field a dozen tiles across, an
        // impassable ridge is not terrain to fight over, it is a wall that
        // decides the fight before either side moves. Every tile of an arena
        // is ground both sides can walk, and the relief that survives is
        // hills — which cost a march and pay a defender, rather than refusing
        // both. The share is exact rather than a percentile cut because a
        // percentile on a hundred tiles lands anywhere between none and a
        // wall.
        MapScript::Battlefield => TectonicProfile {
            plates: 4,
            coastal_demote_numerator: 0,
            coastal_demote_denominator: 1,
            exact_mountain_percent: Some(0),
            ..standard
        },
        _ => standard,
    }
}

/// Bring a profile that promises an exact mountain share to that number while
/// retaining the ridged field's own order. Any added peak is therefore still
/// part of a natural range, and the high hill values reserved as passes stay
/// open through it.
fn balance_mountain_share(
    wm: &mut WorldMap,
    land: &BTreeSet<Pos>,
    mountains: &Fractal,
    hills: &Fractal,
    pass_threshold: u8,
    wanted_percent: usize,
) {
    let wanted = wm.tiles.len() * wanted_percent / 100;
    let mut current: Vec<Pos> = land
        .iter()
        .copied()
        .filter(|pos| wm.tiles[pos].terrain == "mountain")
        .collect();
    if current.len() > wanted {
        let excess = current.len() - wanted;
        current.sort_unstable_by_key(|pos| {
            let (col, row) = noise_cell(wm, *pos);
            mountains.at(col, row)
        });
        for pos in current.into_iter().take(excess) {
            let tile = wm.tiles.get_mut(&pos).unwrap();
            // The climate pass immediately after this one repaints the ground;
            // this only says that a lowered peak remains a foothill.
            tile.terrain = "grassland".into();
            tile.hills = true;
        }
        return;
    }
    if current.len() == wanted {
        return;
    }

    let needed = wanted - current.len();
    let mut candidates: Vec<Pos> = land
        .iter()
        .copied()
        .filter(|pos| wm.tiles[pos].terrain != "mountain")
        .filter(|pos| {
            let (col, row) = noise_cell(wm, *pos);
            // This was deliberately made a pass in the ridge. Filling it back
            // in merely to hit a percentage would defeat the navigable cuts.
            hills.at(col, row) < pass_threshold
        })
        .collect();
    candidates.sort_unstable_by_key(|pos| {
        let (col, row) = noise_cell(wm, *pos);
        std::cmp::Reverse(mountains.at(col, row))
    });
    for pos in candidates.into_iter().take(needed) {
        let tile = wm.tiles.get_mut(&pos).unwrap();
        tile.terrain = "mountain".into();
        tile.hills = false;
    }
}

fn apply_tectonics(
    wm: &mut WorldMap,
    land: &BTreeSet<Pos>,
    script: MapScript,
    continental_rifts: bool,
    rng: &mut Rng,
) {
    let profile = tectonic_profile(script);
    let (width, height) = (wm.width, wm.height);
    let mountain_grain = if script == MapScript::Fjords { 4 } else { 3 };
    let mut mountains = Fractal::new_without_index(rng, width, height, mountain_grain);
    let (ridge_blend, fractal_blend) = if script == MapScript::Fjords {
        // Let the collision seams dominate the relief field. The ordinary
        // half-fractal blend is deliberately broad; Fjords wants the sharp,
        // tapering edge of a range to meet the water.
        (9.0, 1.0)
    } else {
        (5.0, 5.0)
    };
    mountains.build_ridges_without_index(rng, profile.plates, ridge_blend, fractal_blend);
    let hills = Fractal::new_without_index(rng, width, height, mountain_grain);

    let cells: Vec<(i32, i32)> = land.iter().map(|pos| noise_cell(wm, *pos)).collect();
    let mountain_bands = mountains.percentiles_within(
        &cells,
        &[profile.mountain_percentile, profile.foothills_percentile],
    );
    let mountain_threshold = mountain_bands[0];
    let rift_mountain_threshold = if continental_rifts {
        let rift_cells: Vec<(i32, i32)> = land
            .iter()
            .filter(|position| continent_rift_band(wm, **position))
            .map(|position| noise_cell(wm, *position))
            .collect();
        mountains
            .percentile_within(
                rift_cells.iter().copied(),
                profile
                    .mountain_percentile
                    .saturating_sub(CONTINENTAL_RIFT_MOUNTAIN_PERCENTILE_DELTA),
            )
            .min(mountain_threshold)
    } else {
        mountain_threshold
    };
    let foothills_threshold = mountain_bands[1];
    let hill_bands = hills.percentiles_within(
        &cells,
        &[
            profile.pass_percentile,
            (28 - WORLD_AGE) as u32,
            (28 + WORLD_AGE) as u32,
            (72 - WORLD_AGE) as u32,
            (72 + WORLD_AGE) as u32,
        ],
    );
    let pass_threshold = hill_bands[0];
    let low_band = (hill_bands[1], hill_bands[2]);
    let high_band = (hill_bands[3], hill_bands[4]);

    for pos in land {
        let (col, row) = noise_cell(wm, *pos);
        let mountain_value = mountains.at(col, row);
        let hill_value = hills.at(col, row);
        let mountain_cutoff = if continental_rifts && continent_rift_band(wm, *pos) {
            rift_mountain_threshold
        } else {
            mountain_threshold
        };
        let tile = wm.tiles.get_mut(pos).unwrap();
        if mountain_value >= mountain_cutoff {
            if hill_value >= pass_threshold {
                // A pass through the ridgeline, so a range is crossable.
                tile.hills = true;
            } else {
                tile.terrain = "mountain".into();
            }
        } else if mountain_value >= foothills_threshold
            || (hill_value >= low_band.0 && hill_value <= low_band.1)
            || (hill_value >= high_band.0 && hill_value <= high_band.1)
        {
            tile.hills = true;
        }
    }

    // The stock generator demotes nine in ten mountains that reach the water,
    // which is what keeps ordinary coastlines workable and leaves the ranges
    // inland. Fjords deliberately sets this to zero: its sea meets mountains.
    if profile.coastal_demote_numerator > 0 {
        let coastal_peaks: Vec<Pos> = land
            .iter()
            .copied()
            .filter(|pos| wm.tiles[pos].terrain == "mountain")
            .filter(|pos| {
                wm.neighbors(*pos)
                    .into_iter()
                    .any(|neighbor| !land.contains(&neighbor))
            })
            .collect();
        for pos in coastal_peaks {
            if rng.below(profile.coastal_demote_denominator) < profile.coastal_demote_numerator {
                let tile = wm.tiles.get_mut(&pos).unwrap();
                // The climate pass, which runs next, repaints every tile that
                // is no longer a mountain, so only the elevation matters here.
                tile.terrain = "grassland".into();
                tile.hills = true;
            }
        }
    }

    if let Some(wanted_percent) = profile.exact_mountain_percent {
        balance_mountain_share(wm, land, &mountains, &hills, pass_threshold, wanted_percent);
    }
}

/// The latitude to paint at when nothing else names one.
///
/// `TerrainGenerator.lua`'s bands always need *some* latitude, and this is the
/// one that reads as warm: below [`TUNDRA_LATITUDE`], so it is never cold
/// enough for tundra or snow, and inside the desert belt, so the dry fractal
/// is still free to lay deserts down wherever it is dry. Only the randomized
/// arm below can reach it, and only if its own thermal fractal were missing.
const TEMPERATE_LATITUDE: f64 = 0.34;

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
/// ending in tundra and then snow. Randomized hands each tile a latitude drawn
/// from a fourth fractal instead, so the full range from snow to jungle
/// survives but stops running north to south.
///
/// That fourth fractal is built **only** for `Randomized`. Drawing it
/// unconditionally would advance `rng` before the desert and plains fractals
/// and re-roll every existing world from the same seed.
fn assign_biomes(
    wm: &mut WorldMap,
    land: &[Pos],
    poles: MapPoles,
    rng: &mut Rng,
) -> BTreeMap<Pos, Name> {
    let (width, height) = (wm.width, wm.height);
    let deserts = Fractal::new(rng, width, height, 3);
    let plains = Fractal::new(rng, width, height, 3);
    let variation = Fractal::new_without_index(rng, width, height, 3);
    let desert_bottom = deserts.percentile(100 - DESERT_PERCENT);
    let plains_bottom = plains.percentile(100 - PLAINS_PERCENT);
    let thermal = matches!(poles, MapPoles::Randomized)
        .then(|| Fractal::new_without_index(rng, width, height, THERMAL_FRACTAL_GRAIN));

    let mut mountain_terrain = BTreeMap::new();
    for pos in land {
        let (col, row) = noise_cell(wm, *pos);
        let base = match poles {
            MapPoles::Poles => wm.polar_fraction(*pos),
            // `thermal` is Some for exactly this arm.
            MapPoles::Randomized => thermal
                .as_ref()
                .map_or(TEMPERATE_LATITUDE, |f| f.at(col, row) as f64 / 255.0),
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
        if wm.tiles[pos].terrain == "mountain" {
            mountain_terrain.insert(*pos, Name::new(terrain));
        } else {
            wm.tiles.get_mut(pos).unwrap().terrain = terrain.into();
        }
    }
    mountain_terrain
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
    wm.neighbors(pos)
        .into_iter()
        .filter(|neighbor| {
            wm.get(*neighbor)
                .is_some_and(|tile| tile.feature.as_deref() == Some(feature))
        })
        .count()
}

/// The ordinary-feature rule Firaxis ships for an Oasis. `TERRAIN_DESERT`
/// names the flat Desert terrain, while `NoRiver`, `NoCoast`, and
/// `NoAdjacentFeatures` constrain the tile and its ring. The adjacent land
/// itself does not have to be Desert.
fn oasis_ground_and_ring_are_valid(wm: &WorldMap, pos: Pos) -> bool {
    wm.get(pos).is_some_and(|tile| {
        tile.terrain == "desert"
            && !tile.hills
            && !tile.has_river()
            && wm.neighbors(pos).into_iter().all(|neighbor| {
                wm.get(neighbor).is_none_or(|adjacent| {
                    adjacent.terrain != "coast" && adjacent.feature.is_none()
                })
            })
    })
}

#[cfg(test)]
fn oasis_site_is_valid(wm: &WorldMap, pos: Pos) -> bool {
    wm.get(pos).is_some_and(|tile| tile.feature.is_none())
        && oasis_ground_and_ring_are_valid(wm, pos)
}

fn placed_oasis_is_valid(wm: &WorldMap, pos: Pos) -> bool {
    wm.get(pos)
        .is_some_and(|tile| tile.feature.as_deref() == Some("oasis"))
        && oasis_ground_and_ring_are_valid(wm, pos)
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
    // Start each counter at what the world already carries, so the shipped
    // share caps measure the whole map rather than only this pass. Nothing a
    // rolled script places before now is vegetation, so those worlds start at
    // zero exactly as they always did; a true-start world arrives with its own
    // real forests and rainforest already down, and this is what stops a
    // second, fractal Amazon being grown on top of the real one.
    let count = |feature: &str| {
        land.iter()
            .filter(|pos| wm.tiles[*pos].feature.as_deref() == Some(feature))
            .count()
    };
    let (mut jungles, mut forests, mut marshes, mut oases) =
        (count("jungle"), count("forest"), count("marsh"), 0);

    for row in 0..height {
        for col in 0..width {
            let pos = hex::offset_to_axial(col, row);
            if !land.contains(&pos) {
                continue;
            }
            let (terrain, hills, river, has_feature) = {
                let tile = &wm.tiles[&pos];
                (
                    tile.terrain,
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
            // Every desert tile bordering a river floods, as in the stock
            // generator.
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
        || wm.neighbors(pos).into_iter().any(|neighbor| {
            wm.get(neighbor)
                .is_some_and(|tile| tile.feature.as_deref() == Some("oasis"))
        });
    let coastal = wm
        .neighbors(pos)
        .into_iter()
        .any(|neighbor| wm.get(neighbor).is_some_and(|tile| rules.is_water(tile)));

    let mut nearby_yields = Vec::new();
    let mut workable_land = 0;
    for tile_pos in wm.disk(pos, 3) {
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

fn cached_start_quality(
    cache: &mut BTreeMap<Pos, i32>,
    rules: &Rules,
    wm: &WorldMap,
    pos: Pos,
) -> i32 {
    *cache
        .entry(pos)
        .or_insert_with(|| start_quality(rules, wm, pos))
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
    // Bias rows are part of Civilization VI civilization content.  A neutral
    // historical or contemporary identity keeps the fair generic site on
    // rolled maps; True Start Earth has already used its own explicit point.
    if !leader_roster::uses_civ6_mechanics(civ) {
        return 0;
    }
    let Some(bias) = rules
        .civs
        .get(civ)
        .and_then(|spec| spec.start_bias.as_ref())
    else {
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
            bias.terrain.contains(&tile.terrain) && (!bias.terrain_hills || tile.hills)
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
    Flat {
        from: Pos,
        width: i32,
    },
    Planet {
        sphere: &'a crate::sphere::Sphere,
        row: Arc<[u16]>,
    },
}

impl<'a> MapDistanceRow<'a> {
    fn new(wm: &'a WorldMap, from: Pos) -> Self {
        if let Some(sphere) = wm.sphere() {
            Self::Planet {
                sphere,
                row: sphere.distance_row(from).into(),
            }
        } else {
            Self::Flat {
                from,
                width: wm.width,
            }
        }
    }

    fn new_cached(wm: &'a WorldMap, from: Pos, cache: &mut BTreeMap<Pos, Arc<[u16]>>) -> Self {
        if let Some(sphere) = wm.sphere() {
            let row = cache
                .entry(from)
                .or_insert_with(|| sphere.distance_row(from).into())
                .clone();
            Self::Planet { sphere, row }
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
        for (cost, distance) in costs.iter_mut().zip(map_distances_to(wm, other, region)) {
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
    let mut distance_cache = BTreeMap::new();
    while centers.len() < count {
        let newest = *centers.last().unwrap();
        let distances = MapDistanceRow::new_cached(wm, newest, &mut distance_cache);
        for (index, position) in land.iter().enumerate() {
            nearest[index] = nearest[index].min(distances.distance(*position));
        }
        let Some(next) = land
            .iter()
            .enumerate()
            .filter(|(index, _)| nearest[*index] != 0)
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
            .map(|center| MapDistanceRow::new_cached(wm, center, &mut distance_cache))
            .collect();
        let mut reach: Vec<(i32, Pos, usize)> = Vec::with_capacity(land.len() * centers.len());
        for position in land {
            for (index, distances) in distance_rows.iter().enumerate() {
                reach.push((distances.distance(*position), *position, index));
            }
        }
        reach.sort_unstable();
        let mut owner = vec![usize::MAX; wm.tiles.len()];
        let mut load = vec![0_i64; centers.len()];
        for (_, position, index) in &reach {
            let tile_index = wm.tiles.index_of(*position).unwrap();
            if owner[tile_index] != usize::MAX || load[*index] >= capacity {
                continue;
            }
            owner[tile_index] = *index;
            load[*index] += fertility.get(position).copied().unwrap_or(1) as i64;
        }
        // Anything whose every region filled up joins its nearest one anyway;
        // `reach` is sorted by distance, so the first entry for a tile is it.
        for (_, position, index) in &reach {
            let tile_index = wm.tiles.index_of(*position).unwrap();
            if owner[tile_index] == usize::MAX {
                owner[tile_index] = *index;
            }
        }
        regions = vec![Vec::new(); centers.len()];
        for position in land {
            let tile_index = wm.tiles.index_of(*position).unwrap();
            let index = owner[tile_index];
            if index != usize::MAX {
                regions[index].push(*position);
            }
        }
        for region in &mut regions {
            region.sort_unstable();
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
                        major_distances[owner].distance(*position) <= 2 * START_DISTANCE_MINOR_MAJOR
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

struct StartBuffers<'a> {
    foreign: &'a [Pos],
    foreign_buffer: i32,
    own_buffer: i32,
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
    buffers: StartBuffers<'_>,
) -> Vec<(usize, Pos)> {
    let mut seated: Vec<(usize, Pos)> = Vec::with_capacity(regions.len());
    let mut placed: Vec<Pos> = Vec::with_capacity(regions.len());
    let relax_limit = buffers.foreign_buffer.max(buffers.own_buffer);
    let mut foreign_blocked: BTreeMap<i32, BTreeSet<Pos>> = BTreeMap::new();
    for relaxed in 0..=relax_limit {
        let radius = (buffers.foreign_buffer - relaxed).max(MIN_START_SEPARATION - 1);
        foreign_blocked.entry(radius).or_insert_with(|| {
            buffers
                .foreign
                .iter()
                .flat_map(|start| wm.disk(*start, radius))
                .collect()
        });
    }
    let mut quality_cache = BTreeMap::new();
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
        let mut worth = |position: &Pos| {
            cached_start_quality(&mut quality_cache, rules, wm, *position)
                - REGION_CENTRALITY_PULL * center_distances.distance(*position)
        };
        let mut chosen = None;
        'search: for relaxed in 0..=relax_limit {
            let foreign_want = (buffers.foreign_buffer - relaxed).max(MIN_START_SEPARATION - 1);
            let own_want = (buffers.own_buffer - relaxed).max(MIN_START_SEPARATION - 1);
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
    let mut room = vec![i32::MAX; wm.tiles.len()];
    let mut occupied = vec![false; wm.tiles.len()];
    let mut frontier = VecDeque::new();
    for start in spawns.iter().copied() {
        if let Some(index) = wm.tiles.index_of(start) {
            occupied[index] = true;
            if room[index] != 0 {
                room[index] = 0;
                frontier.push_back(start);
            }
        }
    }
    while let Some(position) = frontier.pop_front() {
        let position_index = wm.tiles.index_of(position).unwrap();
        let next = room[position_index].saturating_add(1);
        for neighbor in wm.neighbors(position) {
            let neighbor_index = wm.tiles.index_of(neighbor).unwrap();
            if next < room[neighbor_index] {
                room[neighbor_index] = next;
                frontier.push_back(neighbor);
            }
        }
    }
    let mut quality_cache = BTreeMap::new();
    for _ in 0..count {
        let mut pick = |pool: &BTreeSet<Pos>, floor: i32| {
            pool.iter()
                .filter(|position| {
                    let index = wm.tiles.index_of(**position).unwrap();
                    !occupied[index] && room[index] >= floor
                })
                .max_by_key(|position| {
                    let index = wm.tiles.index_of(**position).unwrap();
                    (
                        room[index],
                        cached_start_quality(&mut quality_cache, rules, wm, **position),
                        **position,
                    )
                })
                .copied()
        };
        let next = pick(candidates, MIN_START_SEPARATION)
            .or_else(|| pick(land, MIN_START_SEPARATION))
            .or_else(|| pick(candidates, 0))
            .or_else(|| pick(land, 0));
        let Some(next) = next else {
            break;
        };
        spawns.push(next);
        let next_index = wm.tiles.index_of(next).unwrap();
        occupied[next_index] = true;
        if room[next_index] != 0 {
            room[next_index] = 0;
            frontier.push_back(next);
        }
        while let Some(position) = frontier.pop_front() {
            let position_index = wm.tiles.index_of(position).unwrap();
            let next_distance = room[position_index].saturating_add(1);
            for neighbor in wm.neighbors(position) {
                let neighbor_index = wm.tiles.index_of(neighbor).unwrap();
                if next_distance < room[neighbor_index] {
                    room[neighbor_index] = next_distance;
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
    let mut distance_cache = BTreeMap::new();
    let mut survey = |starts: &[Pos]| -> Vec<(i32, usize, i32, usize)> {
        let distance_rows: Vec<MapDistanceRow<'_>> = starts
            .iter()
            .copied()
            .map(|start| MapDistanceRow::new_cached(wm, start, &mut distance_cache))
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
    let mut quality_cache = BTreeMap::new();
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
        let keep = cached_start_quality(&mut quality_cache, rules, wm, current)
            * (100 - TERRITORY_QUALITY_TOLERANCE)
            / 100;
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
                    && cached_start_quality(&mut quality_cache, rules, wm, *position) >= keep
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
    let mut quality_cache = BTreeMap::new();
    let mut qualities: Vec<i32> = seated
        .iter()
        .map(|(_, start)| cached_start_quality(&mut quality_cache, rules, wm, *start))
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
            .map(|position| {
                (
                    cached_start_quality(&mut quality_cache, rules, wm, position),
                    position,
                )
            })
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

#[cfg(test)]
fn all_shared_edges(wm: &WorldMap) -> BTreeSet<RiverEdge> {
    let mut edges = BTreeSet::new();
    for pos in wm.tiles.keys().copied() {
        for neighbor in wm
            .neighbors(pos)
            .into_iter()
            .filter(|p| wm.tiles.contains_key(p))
        {
            edges.insert(canonical_river_edge(pos, neighbor));
        }
    }
    edges
}

/// A hex corner, named by the three mutually adjacent tiles that meet there.
///
/// A river is a walk from corner to corner. Each step crosses the edge shared
/// by the two tiles the walk keeps and lays that edge as a river segment, so
/// consecutive segments are joined at a corner by construction and a channel
/// never runs through the middle of a tile.
type RiverCorner = (Pos, Pos, Pos);

fn canonical_corner(a: Pos, b: Pos, c: Pos) -> RiverCorner {
    let mut corner = [a, b, c];
    corner.sort_unstable();
    (corner[0], corner[1], corner[2])
}

/// The tiles adjacent to both `a` and `b` — one per corner of their shared
/// edge, and the name that corner goes by.
fn shared_neighbors(wm: &WorldMap, a: Pos, b: Pos) -> Vec<Pos> {
    let b_neighbors: BTreeSet<Pos> = wm.neighbors(b).into_iter().collect();
    let mut shared: BTreeSet<Pos> = wm
        .neighbors(a)
        .into_iter()
        .filter(|p| *p != b && b_neighbors.contains(p))
        .collect();
    shared.remove(&a);
    shared.into_iter().collect()
}

fn corners_of_edge(wm: &WorldMap, edge: RiverEdge) -> Vec<RiverCorner> {
    shared_neighbors(wm, edge.0, edge.1)
        .into_iter()
        .map(|third| canonical_corner(edge.0, edge.1, third))
        .collect()
}

/// Every way a river standing at `corner` can continue: the edge the step
/// lays, the tile that step reveals, and the corner it arrives at. Three
/// tiles meet at a corner, so there are three edges to cross and the walk
/// arrives at one of three corners — one of which is where it came from.
fn corner_steps(wm: &WorldMap, corner: RiverCorner) -> Vec<(RiverEdge, Pos, RiverCorner)> {
    let (a, b, c) = corner;
    let mut steps = Vec::new();
    for (x, y, behind) in [(a, b, c), (b, c, a), (a, c, b)] {
        for revealed in shared_neighbors(wm, x, y) {
            if revealed == behind {
                continue;
            }
            steps.push((
                canonical_river_edge(x, y),
                revealed,
                canonical_corner(x, y, revealed),
            ));
        }
    }
    steps
}

/// The single tile two consecutive river segments have in common — the one the
/// channel bends around. A river holds a straight course by alternating which
/// of its two banks the next bend pivots on; pivoting on the same tile twice
/// running curls it back on itself.
fn shared_tile(first: RiverEdge, second: RiverEdge) -> Option<Pos> {
    let pair = [second.0, second.1];
    let mut common = [first.0, first.1].into_iter().filter(|p| pair.contains(p));
    let only = common.next();
    common.next().is_none().then_some(only).flatten()
}

/// Civ VI's `GetPlotElevation`: the four-rung ladder its river flow reads off
/// the terrain. Water sits at the bottom, which is what makes a flow that
/// always steps to the lowest ground run downhill to the sea.
fn plot_elevation(wm: &WorldMap, pos: Pos) -> i32 {
    match wm.tiles.get(&pos) {
        None => 1,
        Some(tile) => match tile.terrain.as_str() {
            "ocean" | "coast" | "lake" => 1,
            "mountain" => 4,
            _ if tile.hills => 3,
            _ => 2,
        },
    }
}

/// How steeply the land is taken to rise as it leaves the sea. A river only
/// runs somewhere because the ground it starts on is higher than the ground it
/// ends on, and this is the term that says so.
const RIVER_INLAND_SLOPE: i32 = 3;

/// A corner of `pos` with dry land on all three sides, Civ VI's
/// `GetInlandCorner`. A river has to start somewhere it can run between two
/// banks, so a headwater whose every corner touches water is no headwater.
fn inland_corner(wm: &WorldMap, pos: Pos, is_water: &impl Fn(Pos) -> bool) -> Option<RiverCorner> {
    if is_water(pos) {
        return None;
    }
    for neighbor in wm.neighbors(pos) {
        if is_water(neighbor) {
            continue;
        }
        for third in shared_neighbors(wm, pos, neighbor) {
            if third != pos && !is_water(third) {
                return Some(canonical_corner(pos, neighbor, third));
            }
        }
    }
    None
}

/// Where a traced channel came to rest.
#[derive(PartialEq, Eq, Clone, Copy)]
enum RiverEnding {
    /// At the coast. This is the river's mouth, and it is the only one it has.
    Sea,
    /// On a river already laid. From the confluence down, the tributary drains
    /// through the trunk and out of the trunk's mouth.
    Confluence,
}

/// Civ VI's `RIVER_PLOTS_PER_EDGE`: a landmass is worth about one river edge
/// per twelve of its land tiles.
const RIVER_PLOTS_PER_EDGE: usize = 12;

/// `RIVER_SOURCE_RANGE_DEFAULT`: how far a headwater must sit from fresh water
/// that is already there — which, as each river is laid before the next is
/// sought, means from every river already on the map. This is the rule that
/// spaces rivers out instead of letting them mat together.
const RIVER_SOURCE_RANGE: i32 = 4;

/// `RIVER_SEA_WATER_RANGE_DEFAULT`: how far a headwater must sit from the sea,
/// so that a river has a country to cross before it gets there.
const RIVER_SEA_WATER_RANGE: i32 = 3;

/// How far a finished river pulls the country either side of it downhill, and
/// by how much at its own banks.
///
/// A river carves the ground it runs through, and the land it has carved
/// drains into it — which is the whole reason tributaries exist rather than
/// every stream cutting its own private line to the coast. Without this a
/// generated map is all trunks and no branches: each river is laid against an
/// untouched landscape that gives it no reason to prefer the valley already
/// there. The drop tapers to nothing at the edge of the basin, so what a later
/// flow feels is a slope toward the river rather than a cliff at its bank.
const RIVER_VALLEY_DEPTH: i32 = 15;
const RIVER_VALLEY_REACH: i32 = 3;

/// How near an existing river a tributary's headwater is sought, and how much
/// ground it must have to itself before it gets there. A source closer than
/// the clearance would join within a segment or two, which is a fork in a
/// river rather than a river of its own.
const RIVER_TRIBUTARY_REACH: i32 = 6;
const RIVER_TRIBUTARY_CLEARANCE: i32 = 3;

/// The fewest segments worth drawing. A headwater that rises two tiles from
/// the coast reaches it almost at once, and what that leaves on the map is a
/// nick in the shoreline rather than a river — most visibly on the island
/// scripts, where scarcely any ground sits far enough inland to do better.
/// Those sources are passed over instead.
const RIVER_MIN_COURSE: usize = 4;

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
    let strategics: Vec<Name> = rules
        .resources
        .iter()
        .filter(|(_, spec)| spec.class == "strategic")
        .map(|(name, _)| *name)
        .collect();
    let land_list: Vec<Pos> = land.iter().cloned().collect();
    for resource in strategics {
        let spec = &rules.resources[&resource];
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
            wm.tiles.get_mut(&pos).unwrap().resource = Some(Name::new(&resource));
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
    let artifacts: Vec<Name> = rules
        .resources
        .iter()
        .filter(|(_, spec)| spec.class == "artifact")
        .map(|(name, _)| *name)
        .collect();
    let all: Vec<Pos> = wm.tiles.keys().copied().collect();
    for resource in artifacts {
        let spec = &rules.resources[&resource];
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
            wm.tiles.get_mut(&pos).unwrap().resource = Some(Name::new(&resource));
            wanted -= 1;
        }
    }
}

/// Divide land into the stock number of named geographic regions. A landmass
/// can contain several continents (Europe and Asia are the familiar example),
/// but a continent boundary should not jump an ocean merely because a centre
/// on another landmass happened to be closer in straight-line distance.
///
/// The substantial landmasses therefore receive their proportional share of
/// the continent budget first and are divided independently. Small offshore
/// islands do not consume that budget: each joins the nearest finished region
/// as a whole, so a three-tile island cannot be striped between two continents.
fn assign_continents(wm: &mut WorldMap, land: &BTreeSet<Pos>, requested: usize, rng: &mut Rng) {
    if land.is_empty() || requested == 0 {
        return;
    }
    let count = requested.min(land.len());
    let components = connected_components(wm, land);
    let fair_share = land.len().div_ceil(count);
    // Half of one continent's fair share is enough to be geography in its own
    // right. Anything smaller is an offshore island, unless more landmasses
    // are needed to hold all of the requested one-tile-minimum regions.
    let minimum_primary = fair_share.div_ceil(2).max(1);
    let mut primary: Vec<usize> = components
        .iter()
        .enumerate()
        .filter(|(_, component)| component.len() >= minimum_primary)
        .map(|(index, _)| index)
        .take(count)
        .collect();
    if primary.is_empty() {
        primary.push(0);
    }
    let mut primary_land: usize = primary.iter().map(|index| components[*index].len()).sum();
    for (index, component) in components.iter().enumerate() {
        if primary_land >= count || primary.len() >= count {
            break;
        }
        if !primary.contains(&index) {
            primary.push(index);
            primary_land += component.len();
        }
    }

    // Every primary landmass gets one name. Largest-remainder apportionment
    // divides the remaining names by area, capped so no region can be empty.
    let weights: Vec<i64> = primary
        .iter()
        .map(|index| components[*index].len() as i64)
        .collect();
    let caps: Vec<usize> = primary
        .iter()
        .map(|index| components[*index].len().saturating_sub(1))
        .collect();
    let extra = apportion_capped(&weights, &caps, count - primary.len());
    let allocation: Vec<usize> = extra.into_iter().map(|additional| additional + 1).collect();

    // Consume exactly the one draw the former whole-world Voronoi pass used,
    // so changing continent borders does not silently redraw later spawns.
    let phase = rng.below(land.len());
    let mut rows: Vec<(usize, MapDistanceRow<'_>)> = Vec::with_capacity(count);
    let mut assigned: Vec<(Pos, Option<usize>)> = Vec::with_capacity(land.len());
    let mut next_id = 0;
    for (slot, component_index) in primary.iter().copied().enumerate() {
        let component: Vec<Pos> = components[component_index].iter().copied().collect();
        let region_count = allocation[slot];
        let mut centers = Vec::with_capacity(region_count);
        let mut local_rows = Vec::with_capacity(region_count);
        let mut nearest = vec![i32::MAX; component.len()];
        let mut next = component[phase.wrapping_add(component_index) % component.len()];
        loop {
            let row = MapDistanceRow::new(wm, next);
            for (index, position) in component.iter().enumerate() {
                nearest[index] = nearest[index].min(row.distance(*position));
            }
            centers.push(next);
            local_rows.push((next_id + centers.len() - 1, row));
            if centers.len() >= region_count {
                break;
            }
            let (index, _) = component
                .iter()
                .enumerate()
                .filter(|(_, position)| !centers.contains(position))
                .max_by_key(|(index, position)| (nearest[*index], **position))
                .expect("a landmass has more tiles than assigned continents");
            next = component[index];
        }
        for position in component {
            let continent = local_rows
                .iter()
                .min_by_key(|(id, row)| (row.distance(position), *id))
                .map(|(id, _)| *id);
            assigned.push((position, continent));
        }
        next_id += region_count;
        rows.extend(local_rows);
    }

    // Satellite islands inherit one nearby region wholesale. Comparing the
    // closest shore tile, rather than the island centroid, also does the right
    // thing for a long island following a continental coast.
    for (index, component) in components.iter().enumerate() {
        if primary.contains(&index) {
            continue;
        }
        let continent = rows
            .iter()
            .min_by_key(|(id, row)| {
                (
                    component
                        .iter()
                        .map(|position| row.distance(*position))
                        .min()
                        .unwrap_or(i32::MAX),
                    *id,
                )
            })
            .map(|(id, _)| *id);
        assigned.extend(component.iter().map(|position| (*position, continent)));
    }
    drop(rows);
    for (pos, continent) in assigned {
        wm.tiles.get_mut(&pos).unwrap().continent = continent;
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
mod river_tests;

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
            ("Mali", 1, 0, 5),    // DESERT Tier 1, ten resources Tier 5
            ("Maya", 1, 0, 2),    // GRASS/PLAINS Tier 1, thirteen luxuries Tier 2
            ("Vietnam", 0, 1, 0), // FOREST/JUNGLE/MARSH Tier 1
            ("Gaul", 0, 0, 2),    // seven resources Tier 2
            ("Kongo", 0, 2, 0),   // FOREST/JUNGLE Tier 2
            ("Russia", 2, 0, 0),  // TUNDRA and TUNDRA_HILLS Tier 2
            ("Nubia", 2, 0, 5),   // already exact before this change
            ("Brazil", 0, 2, 0),
            ("Egypt", 0, 2, 0),
            ("Korea", 3, 0, 0), // four hills terrains, Tier 3
            ("Greece", 3, 0, 0),
            ("Scythia", 5, 0, 2), // GRASS/PLAINS Tier 5, HORSES Tier 2
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
                    rules.civs["Scythia"]
                        .start_bias
                        .as_ref()
                        .unwrap()
                        .terrain_tier,
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
