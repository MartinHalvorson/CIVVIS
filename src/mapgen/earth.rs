//! The Earth map: the real-world map script, from the surface data to the
//! painted board.
//!
//! The surface grid decoded from `data/earth_surface.txt` (`earth_grid`;
//! the include itself, `EARTH_SURFACE`, stays in `mapgen.rs` beside the
//! path it is relative to), a grid cell and the span of cells under each
//! tile, the ground a tile samples (`earth_tile`,
//! `earth_ground_from_sample`) with its mountain, hill and vegetation
//! shares, the hand-placed passes, islands, microstate islands and lakes,
//! compass directions on the sphere, the tile cache and the land mask built
//! from it (`earth_land`, `earth_land_from_cache`), the area of a tile,
//! painting the board (`paint_earth`), and the nearest land to a point.
//!
//! Moved verbatim out of `mapgen.rs` (2026-11-05). `mapgen.rs` re-exports
//! everything here, so every path is unchanged; items the rest of `mapgen`
//! uses are `pub(super)`.

use super::*;

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
pub(super) struct EarthCell(u8);

impl EarthCell {
    pub(super) fn surface(self) -> u8 {
        self.0 & 0b111
    }
    pub(super) fn hills(self) -> bool {
        self.0 & 0b1000 != 0
    }
    pub(super) fn vegetation(self) -> usize {
        ((self.0 >> 4) & 0b11) as usize
    }
    pub(super) fn is_land(self) -> bool {
        !matches!(self.surface(), EARTH_SEA | EARTH_LAKE)
    }
}

/// The cell a point in degrees falls in. Longitude wraps; latitude clamps,
/// because there is nothing past a pole to read.
pub(super) fn earth_cell(longitude: f64, latitude: f64) -> EarthCell {
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
pub(super) struct EarthTile {
    /// Water of some kind, so not part of the world's land.
    pub(super) water: bool,
    pub(super) terrain: &'static str,
    /// The climate underneath a mountain tile. Civ VI keeps this distinction
    /// in `TERRAIN_*_MOUNTAIN` even though CIVVIS renders every peak with the
    /// common `mountain` terrain.
    pub(super) mountain_terrain: Option<&'static str>,
    pub(super) hills: bool,
    pub(super) vegetation: Option<&'static str>,
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
pub(super) const EARTH_PASSES: &[(f64, f64, &str)] = &[
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
pub(super) const EARTH_LAKES: &[(f64, f64, f64)] = &[
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
pub(super) fn earth_direction(longitude: f64, latitude: f64) -> [f64; 3] {
    let (longitude, latitude) = (longitude.to_radians(), latitude.to_radians());
    [
        trig::cos(latitude) * trig::cos(longitude),
        trig::cos(latitude) * trig::sin(longitude),
        trig::sin(latitude),
    ]
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
pub(super) fn earth_tile_cache(wm: &WorldMap) -> BTreeMap<Pos, EarthTile> {
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

pub(super) fn earth_land(wm: &WorldMap) -> BTreeSet<Pos> {
    let cache = earth_tile_cache(wm);
    earth_land_from_cache(wm, &cache)
}

pub(super) fn earth_land_from_cache(
    wm: &WorldMap,
    cache: &BTreeMap<Pos, EarthTile>,
) -> BTreeSet<Pos> {
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
pub(super) fn paint_earth(
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

/// The physical Earth tile nearest a WGS84 point, restricted to sampled land.
///
/// A country-level coordinate is not necessarily a capital coordinate: an
/// archipelagic or geographically split country can reasonably be represented
/// by a centroid in the sea between its land areas.  Scenario builders must
/// therefore resolve the coordinate through the physical map rather than put
/// a city on the raw nearest tile.
#[cfg(test)]
pub(super) fn nearest_earth_land(
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
