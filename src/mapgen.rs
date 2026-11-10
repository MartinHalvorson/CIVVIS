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
mod earth;
mod lakes;
mod natural_wonders;
mod rivers;
mod starts;
use canals::*;
use earth::*;
use lakes::*;
use natural_wonders::*;
use rivers::*;
pub use starts::*;

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

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// How much of a tile a guaranteed island or lake has to be worth before the
/// map is fine enough to draw it. An island earns its tile at an eighth of
/// one and a lake only at a half, for the reason [`EARTH_LAKES`] gives.
const ISLAND_TILE_SHARE: f64 = 8.0;
const LAKE_TILE_SHARE: f64 = 2.0;

/// The tile whose centre points nearest a place on Earth.
fn nearest_tile(wm: &WorldMap, longitude: f64, latitude: f64) -> Option<Pos> {
    let target = earth_direction(longitude, latitude);
    wm.tiles.keys().copied().max_by(|a, b| {
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

    // Every stock script stamps its continents before the relief pass whose
    // volcanoes and mountains follow the seams between them. Keep the planning
    // on a clone: the legacy main-stream draw stays at its original point
    // below, so unrelated seeded geography does not reroll.
    let stamps_continents = stamps_continents_before_relief(script, width, height);
    if stamps_continents {
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
        apply_tectonics(&mut wm, &land, script, rng);
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

    // --- `AddTerrainFromContinents`, which the stock scripts run after the
    // biomes and before the rivers, so a river can start in the mountains it
    // raises. An arena has no room for eruptions rewriting its ground.
    if !script.is_battlefield() {
        let desired_volcanoes = if script.is_fixed_geography() {
            land.len() as f64 / FIXED_GEOGRAPHY_LAND_PER_VOLCANO
        } else {
            land.len() as f64 / f64::from((8 - stock_world_age(script)) * 50)
        };
        let seams = stamped_seams(&wm, stamps_continents);
        add_terrain_from_continents(
            &mut wm,
            &land,
            &mut mountain_terrain,
            &seams,
            desired_volcanoes,
            rng,
        );
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

    // Tribal Villages are placed last now (`add_goodies`), but this pass's
    // draws stay: dropping them would redraw the terrain, wonders, and
    // resources of every seeded world for a rule that touches none of those.
    if !script.is_battlefield() {
        for pos in &land_list {
            let tile = &wm.tiles[pos];
            if tile.terrain != "mountain" && !tile.has_river() {
                rng.f64();
            }
        }
    }

    // --- polar features
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
    // `iNumLandPlots`, which `AddFeatures` counts as it scans: passable land
    // no feature has claimed. It sets the fissure budget further down.
    let open_land = land
        .iter()
        .filter(|pos| {
            let tile = &wm.tiles[*pos];
            tile.terrain != "mountain" && tile.feature.is_none()
        })
        .count();
    add_features(&mut wm, &land, rng);

    // --- natural wonders. See `mapgen/natural_wonders.rs`.
    wm = place_natural_wonders(
        wm,
        mountain_terrain,
        rules,
        num_natural_wonders,
        script,
        rng,
    );

    // --- Geothermal Fissures, in the stock position: after the wonders, so a
    // wonder never has to give way to one.
    if !script.is_battlefield() {
        let desired_fissures = if script.is_fixed_geography() {
            land.len() as f64 / FIXED_GEOGRAPHY_LAND_PER_FISSURE
        } else {
            open_land as f64 / LAND_PER_FISSURE
        };
        let seams = stamped_seams(&wm, stamps_continents);
        add_geothermal_fissures(&mut wm, &land, &seams, desired_fissures, rng);
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

    if stamps_continents {
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

    // --- the starts, and the passes that have to follow them. See
    // `mapgen/starts.rs`.
    let seats = SeatRequest {
        num_major_spawns,
        num_minor_spawns,
        leader_starts,
        minor_starts,
    };
    place_starts_and_finish(wm, land, rules, script, seats, rng)
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

/// The stock scripts this generator mirrors, which all call
/// `TerrainBuilder.StampContinents` and then `AddTerrainFromContinents`. CIVVIS's
/// own world types have no shipped seam rules, so they get volcanoes on lone
/// mountains only. Neither does a board under the smallest shipped map, Duel
/// at 44x26: a seam there can swallow the land the seats need.
fn stamps_continents_before_relief(script: MapScript, width: i32, height: i32) -> bool {
    width * height >= 44 * 26
        && matches!(
            script,
            MapScript::Continents
                | MapScript::SmallContinents
                | MapScript::Pangaea
                | MapScript::Islands
                | MapScript::Lakes
                | MapScript::InlandSea
        )
}

/// The seams of a world whose continents are stamped, and none otherwise. A
/// fixed-geography world carries real continents, but its ranges are real too.
fn stamped_seams(wm: &WorldMap, stamps_continents: bool) -> BTreeMap<Pos, i32> {
    if stamps_continents {
        continent_seams(wm)
    } else {
        BTreeMap::new()
    }
}

/// The `world_age_normal` each stock script declares. Only Island Plates
/// differs, and it is what sets that script's volcano budget.
fn stock_world_age(script: MapScript) -> i32 {
    match script {
        MapScript::Islands => 2,
        _ => WORLD_AGE,
    }
}

/// `iNumLandPlots / 200` in `FeatureGenerator:AddFeaturesFromContinents`.
const LAND_PER_FISSURE: f64 = 200.0;

/// The shipped Earth maps place these by hand, between 127 and 195 land plots
/// per volcano and 152 and 182 per fissure on `EarthStandard_XP2` and
/// `WorldMapHuge_XP2`. CIVVIS rolls the sites but keeps a density in that band.
const FIXED_GEOGRAPHY_LAND_PER_VOLCANO: f64 = 180.0;
const FIXED_GEOGRAPHY_LAND_PER_FISSURE: f64 = 140.0;

/// `Map.FindSecondContinent` reports whether two continents fall within a
/// range of a plot. This returns the smallest range, up to the three the
/// scripts ask about, at which it would say yes.
///
/// 🟡 The function is compiled. Two continents in range is the reading its name
/// gives, and it is the only one under which a water plot can answer at all.
fn second_continent_distance(wm: &WorldMap, pos: Pos) -> Option<i32> {
    let mut nearest: BTreeMap<usize, i32> = BTreeMap::new();
    for other in wm.disk(pos, 3) {
        if let Some(continent) = wm.tiles.get(&other).and_then(|tile| tile.continent) {
            let distance = wm.distance(pos, other);
            nearest
                .entry(continent)
                .and_modify(|best| *best = (*best).min(distance))
                .or_insert(distance);
        }
    }
    let mut distances: Vec<i32> = nearest.into_values().collect();
    distances.sort_unstable();
    distances.get(1).copied()
}

/// Plots a second continent reaches, with the range at which it does.
fn continent_seams(wm: &WorldMap) -> BTreeMap<Pos, i32> {
    wm.tiles
        .keys()
        .filter_map(|pos| second_continent_distance(wm, *pos).map(|range| (*pos, range)))
        .collect()
}

/// A die of a Lua-computed size. `GetRandomNumber` takes an integer, so the
/// fraction truncates, and a die smaller than one always lands on zero.
fn fractional_die(rng: &mut Rng, sides: f64) -> usize {
    rng.below((sides as usize).max(1))
}

/// `iMountainPercentByDistance` and `iHillPercentByDistance`, indexed by the
/// distance to the second continent less one.
const SEAM_MOUNTAIN_PERCENT: [usize; 3] = [42, 24, 6];
const SEAM_HILL_PERCENT: [usize; 3] = [50, 40, 30];

/// `AddTerrainFromContinents` (`TerrainGenerator.lua`).
///
/// Two thirds of the volcano budget is rolled along continent seams, where the
/// same scan raises Mountains and Hills. Whatever is left falls on lone
/// mountains, shared between continents by how many each has. The `/ 2` the
/// script applies to mountains beside a volcano cannot fire: a plot within
/// three of a placed volcano never reaches that roll.
fn add_terrain_from_continents(
    wm: &mut WorldMap,
    land: &BTreeSet<Pos>,
    mountain_terrain: &mut BTreeMap<Pos, Name>,
    seams: &BTreeMap<Pos, i32>,
    desired_volcanoes: f64,
    rng: &mut Rng,
) {
    let desired_near_seams = desired_volcanoes * 2.0 / 3.0;
    // Counted over every plot, the water included.
    let plots_per_volcano = seams.len() as f64 / desired_near_seams.max(f64::MIN_POSITIVE);
    let mut placed: Vec<Pos> = Vec::new();
    let mut lonely: BTreeMap<Option<usize>, Vec<Pos>> = BTreeMap::new();

    let (width, height) = (wm.width, wm.height);
    for col in 0..width {
        for row in 0..height {
            let pos = hex::offset_to_axial(col, row);
            if !land.contains(&pos) {
                continue;
            }
            let adjacent_mountains = wm
                .neighbors(pos)
                .into_iter()
                .filter(|neighbor| {
                    wm.tiles
                        .get(neighbor)
                        .is_some_and(|tile| tile.terrain == "mountain")
                })
                .count();
            let mut seam_range = None;
            let mut volcano_here = false;
            if adjacent_mountains != 6 && placed.iter().all(|v| wm.distance(pos, *v) > 3) {
                seam_range = seams.get(&pos).copied();
                match seam_range {
                    Some(1) => {
                        volcano_here = fractional_die(rng, plots_per_volcano * 0.7) == 0;
                    }
                    Some(2) => {
                        volcano_here = fractional_die(rng, plots_per_volcano) == 0;
                    }
                    Some(_) => {
                        volcano_here = fractional_die(rng, plots_per_volcano * 1.5) == 0;
                    }
                    None => {
                        if wm.tiles[&pos].terrain == "mountain"
                            && matches!(adjacent_mountains, 0 | 2 | 3)
                        {
                            lonely
                                .entry(wm.tiles[&pos].continent)
                                .or_default()
                                .push(pos);
                        }
                    }
                }
            }
            if volcano_here {
                raise_mountain(wm, mountain_terrain, pos);
                wm.tiles.get_mut(&pos).unwrap().feature = Some("volcano".into());
                placed.push(pos);
            } else if let Some(range) = seam_range {
                let index = (range - 1) as usize;
                if rng.below(100) < SEAM_MOUNTAIN_PERCENT[index] {
                    raise_mountain(wm, mountain_terrain, pos);
                } else if rng.below(100) < SEAM_HILL_PERCENT[index] {
                    // `ConvertToHills` also takes a standing Mountain down.
                    let tile = wm.tiles.get_mut(&pos).unwrap();
                    if tile.terrain == "mountain" {
                        tile.terrain = mountain_terrain
                            .remove(&pos)
                            .unwrap_or_else(|| Name::new("grassland"));
                    }
                    tile.hills = true;
                }
            }
        }
    }

    let lonely_volcanoes = desired_volcanoes - placed.len() as f64;
    let total_mountains: usize = lonely.values().map(Vec::len).sum();
    if total_mountains == 0 || lonely_volcanoes <= 0.0 {
        return;
    }
    for mut mountains in lonely.into_values() {
        let share = lonely_volcanoes * mountains.len() as f64 / total_mountains as f64;
        for index in (1..mountains.len()).rev() {
            let other = rng.below(index + 1);
            mountains.swap(index, other);
        }
        let mut placed_here = 0usize;
        for pos in mountains {
            if placed.iter().all(|v| wm.distance(pos, *v) > 4) {
                wm.tiles.get_mut(&pos).unwrap().feature = Some("volcano".into());
                placed.push(pos);
                placed_here += 1;
                if placed_here as f64 >= share {
                    break;
                }
            }
        }
    }
}

/// `ConvertToMountain`, keeping the biome so the tile can come back down.
fn raise_mountain(wm: &mut WorldMap, mountain_terrain: &mut BTreeMap<Pos, Name>, pos: Pos) {
    let tile = wm.tiles.get_mut(&pos).unwrap();
    if tile.terrain != "mountain" {
        mountain_terrain.insert(pos, tile.terrain);
        tile.terrain = "mountain".into();
    }
    tile.hills = false;
}

/// The fissure half of `FeatureGenerator:AddFeaturesFromContinents`: shuffled
/// plots near a continent seam first, then anywhere. Nothing spaces them.
fn add_geothermal_fissures(
    wm: &mut WorldMap,
    land: &BTreeSet<Pos>,
    seams: &BTreeMap<Pos, i32>,
    desired_fissures: f64,
    rng: &mut Rng,
) {
    if desired_fissures <= 0.0 {
        return;
    }
    let mut placed = 0usize;
    for near_seam in [true, false] {
        if placed as f64 >= desired_fissures {
            break;
        }
        let mut candidates: Vec<Pos> = land
            .iter()
            .filter(|pos| {
                let tile = &wm.tiles[*pos];
                tile.terrain != "mountain"
                    && tile.feature.is_none()
                    && seams.contains_key(*pos) == near_seam
            })
            .copied()
            .collect();
        for index in (1..candidates.len()).rev() {
            let other = rng.below(index + 1);
            candidates.swap(index, other);
        }
        for pos in candidates {
            wm.tiles.get_mut(&pos).unwrap().feature = Some("geothermal_fissure".into());
            placed += 1;
            if placed as f64 >= desired_fissures {
                break;
            }
        }
    }
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
        let tile = wm.tiles.get_mut(pos).unwrap();
        if mountain_value >= mountain_threshold {
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
                && (rng.below(450) as i32)
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
                && (rng.below(450) as i32)
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

// `START_DISTANCE_RANGE_MINOR` is declared in `GlobalParameters.xml` and used
// by no shipped map script, so there is no minor band to model — the two minor
// distances below are plain buffers.

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
