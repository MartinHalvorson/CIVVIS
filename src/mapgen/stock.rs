//! Laying a stock script's world onto the map.
//!
//! [`super::shipped`] draws a world the way Civilization VI's own script
//! does, on the plot grid that script works on. This module is the step
//! `ApplyBaseTerrain` is to the game: it hands every plot's land, relief and
//! climate to the tile that stands there.
//!
//! On a flat map the tile is the plot. The grids are the same grid, row for
//! row and column for column (both set odd rows half a hex east), so the
//! world the script drew is the world the game plays, shallow water and all.
//!
//! A globe has no such grid, so the script draws on a rectangle of the
//! globe's own resolution and each tile reads the plot under its longitude
//! and latitude: the same continents, wrapped round a sphere instead of a
//! cylinder. Two things are then settled on the globe itself rather than
//! read off the rectangle. The twelve pentagons stay sea, as they do on every
//! generated globe, so every land tile keeps six neighbours. And shallow
//! water is worked out on the globe's own adjacency, by the script's own
//! rule, because a coast has to be beside the land a ship actually reaches.

use std::collections::{BTreeMap, BTreeSet};

use super::shipped::{self, Climate, Grid, StockScript, PLOT_HILLS, PLOT_MOUNTAIN, PLOT_OCEAN};
use crate::name::Name;
use crate::rng::Rng;
use crate::setup::{MapPoles, MapScript};
use crate::world::WorldMap;
use crate::{hex, Pos};

/// The stock script behind a world type, where the type is one the game
/// ships. CIVVIS's own types have no script to follow and answer `None`.
pub(super) fn stock_script(script: MapScript) -> Option<StockScript> {
    match script {
        MapScript::Continents => Some(StockScript::Continents),
        MapScript::Pangaea => Some(StockScript::Pangaea),
        MapScript::SmallContinents => Some(StockScript::SmallContinents),
        MapScript::Islands => Some(StockScript::IslandPlates),
        MapScript::InlandSea => Some(StockScript::InlandSea),
        MapScript::Lakes => Some(StockScript::Lakes),
        _ => None,
    }
}

/// Whether a flat world of this type has four edges instead of wrapping east
/// to west. `InlandSea.lua` asks for exactly that map (`WrapX = false`): its
/// sea is ringed by land, and a ring that wrapped would be a band.
pub(super) fn flat_world_is_bounded(script: MapScript) -> bool {
    stock_script(script).is_some_and(|stock| !shipped::wraps_east_west(stock))
}

/// What a stock world leaves for the passes after it.
pub(super) struct StockGround {
    /// Every tile that is not water.
    pub land: BTreeSet<Pos>,
    /// The climate under each Mountain, for a pass that lowers one.
    pub mountain_terrain: BTreeMap<Pos, Name>,
    /// The plots the script's own table calls Hills, which is what its cliff
    /// pass reads however the ground is changed in between.
    pub hills: BTreeSet<Pos>,
    /// Whether shallow water is already painted. It is on a flat map, where
    /// the script's own coast is the coast.
    pub coast_painted: bool,
}

/// The rectangle a globe's script draws on: as many columns as the globe has
/// tiles round its equator and as many rows as it has from pole to pole, so
/// a plot at the equator is one tile.
fn globe_grid(wm: &WorldMap, script: StockScript) -> Grid {
    let tiles = wm.tiles.len().max(1) as f64;
    // A tile's width across its flats, in radians, on a unit sphere.
    let width = (8.0 * std::f64::consts::PI / (3.0f64.sqrt() * tiles)).sqrt();
    let columns = (2.0 * std::f64::consts::PI / width).round() as i32;
    let rows = (std::f64::consts::PI / (width * 3.0f64.sqrt() / 2.0)).round() as i32;
    Grid {
        w: columns.max(8),
        h: rows.max(6),
        wrap_x: shipped::wraps_east_west(script),
    }
}

fn climate_name(climate: Climate) -> Name {
    Name::new(match climate {
        Climate::Ocean => "ocean",
        Climate::Coast => "coast",
        Climate::Grass => "grassland",
        Climate::Plains => "plains",
        Climate::Desert => "desert",
        Climate::Tundra => "tundra",
        Climate::Snow => "snow",
    })
}

/// Draw the stock world and paint it onto `wm`.
pub(super) fn paint_stock_world(
    wm: &mut WorldMap,
    script: StockScript,
    poles: MapPoles,
    rng: &mut Rng,
) -> StockGround {
    let scattered_heat = matches!(poles, MapPoles::Randomized);
    let globe = wm.sphere().is_some();
    let grid = if globe {
        globe_grid(wm, script)
    } else {
        Grid {
            w: wm.width,
            h: wm.height,
            wrap_x: wm.wraps_east_west(),
        }
    };
    let world = shipped::generate(script, grid, scattered_heat, rng);

    let pentagons: BTreeSet<Pos> = wm
        .sphere()
        .map(|sphere| sphere.pentagons().into_iter().collect())
        .unwrap_or_default();
    let positions: Vec<Pos> = wm.tiles.keys().copied().collect();
    let mut ground = StockGround {
        land: BTreeSet::new(),
        mountain_terrain: BTreeMap::new(),
        hills: BTreeSet::new(),
        coast_painted: !globe,
    };
    for pos in positions {
        let (x, y) = match wm.sphere() {
            Some(sphere) => {
                use std::f64::consts::{FRAC_PI_2, PI};
                let east = (sphere.longitude(pos) + PI) / (2.0 * PI);
                let north = (sphere.latitude(pos) + FRAC_PI_2) / PI;
                (
                    ((east * f64::from(grid.w)) as i32).rem_euclid(grid.w),
                    ((north * f64::from(grid.h)) as i32).clamp(0, grid.h - 1),
                )
            }
            None => hex::axial_to_offset(pos.0, pos.1),
        };
        let plot = world.plot(x, y);
        let climate = world.climate_at(x, y);
        let tile = wm.tiles.get_mut(&pos).unwrap();
        if plot == PLOT_OCEAN || pentagons.contains(&pos) {
            // A globe's shallows are found on the globe, further on.
            tile.terrain = if globe {
                Name::new("ocean")
            } else {
                climate_name(climate)
            };
            continue;
        }
        ground.land.insert(pos);
        // A plot the rectangle calls land but paints as water can only be a
        // pentagon's neighbour read through a seam; give it the ground the
        // script gives a plot with no climate of its own.
        let terrain = match climate {
            Climate::Ocean | Climate::Coast => Name::new("grassland"),
            climate => climate_name(climate),
        };
        if plot == PLOT_MOUNTAIN {
            ground.mountain_terrain.insert(pos, terrain);
            tile.terrain = Name::new("mountain");
            tile.hills = false;
        } else {
            tile.terrain = terrain;
            tile.hills = plot == PLOT_HILLS;
            if plot == PLOT_HILLS {
                ground.hills.insert(pos);
            }
        }
    }
    ground
}
