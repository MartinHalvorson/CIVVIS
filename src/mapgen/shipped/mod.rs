//! The stock map scripts' own land, relief and climate passes.
//!
//! `Continents.lua`, `Pangaea.lua`, `Small_Continents.lua`,
//! `Island_Plates.lua`, `InlandSea.lua` and `Lakes.lua` (Gathering Storm),
//! with the three utilities they share: `MapUtilities.lua`,
//! `MountainsCliffs.lua` and `TerrainGenerator.lua`. Each function here is the
//! Lua function of the same name, read top to bottom, on the engine's own
//! fractal ([`fractal::CvFractal`]).
//!
//! Two things are kept that a tidy rewrite would lose, because they shape the
//! worlds the game ships:
//!
//! - **The scripts' table indexing.** A plot table is filled by plot index
//!   (from zero) in one function and read "plus one because Lua arrays start
//!   at 1" in the next. [`PlotTable`] is that table, absent entries included,
//!   so each pass reads exactly the cells its original reads.
//! - **What the scripts actually run.** Pangaea's shift toward a pole compares
//!   an area object with an area id and never fires; every script's plate
//!   count looks a number up in a table keyed by name and always falls back
//!   to four. Both are ported as they behave, not as they were meant.
//!
//! The map options are the setup screen's defaults throughout: Standard world
//! age, Standard sea level and Standard temperature.
//!
//! What cannot be matched is the dice. The engine's random stream is not
//! exposed, so a seed here is not the same world there: the rules are the
//! same, and the worlds are drawn from the same distribution.

pub mod fractal;

use crate::rng::Rng;
use fractal::{CvFractal, FRAC_POLAR, FRAC_WRAP_X};

/// `g_PLOT_TYPE_*` (`MapEnums.lua`). `PLOT_NONE` also stands for an entry a
/// table does not hold, which Lua reads as `nil`.
pub const PLOT_NONE: i8 = -1;
pub const PLOT_MOUNTAIN: i8 = 0;
pub const PLOT_HILLS: i8 = 1;
pub const PLOT_LAND: i8 = 2;
pub const PLOT_OCEAN: i8 = 3;

/// The climate a plot is painted, before hills and mountains are laid on it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Climate {
    Ocean,
    Coast,
    Grass,
    Plains,
    Desert,
    Tundra,
    Snow,
}

/// Which stock script draws the world.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StockScript {
    Continents,
    Pangaea,
    SmallContinents,
    IslandPlates,
    InlandSea,
    Lakes,
}

/// `DirectionTypes`, in the engine's order.
const NORTHEAST: i32 = 0;
const EAST: i32 = 1;
const SOUTHEAST: i32 = 2;
const SOUTHWEST: i32 = 3;
const WEST: i32 = 4;
const NORTHWEST: i32 = 5;
const NUM_DIRECTIONS: i32 = 6;

/// The plot grid a script draws on: `Map.GetGridSize()` and whether the map
/// wraps east to west.
#[derive(Clone, Copy, Debug)]
pub struct Grid {
    pub w: i32,
    pub h: i32,
    pub wrap_x: bool,
}

impl Grid {
    fn plots(self) -> i32 {
        self.w * self.h
    }

    /// `Map.GetAdjacentPlot`. Odd rows sit half a hex east, and `y` counts up
    /// from the southern edge.
    fn adjacent(self, x: i32, y: i32, direction: i32) -> Option<(i32, i32)> {
        let odd = y & 1;
        let (nx, ny) = match direction {
            NORTHEAST => (x + odd, y + 1),
            EAST => (x + 1, y),
            SOUTHEAST => (x + odd, y - 1),
            SOUTHWEST => (x + odd - 1, y - 1),
            WEST => (x - 1, y),
            _ => (x + odd - 1, y + 1),
        };
        if ny < 0 || ny >= self.h {
            return None;
        }
        if self.wrap_x {
            Some((nx.rem_euclid(self.w), ny))
        } else if nx < 0 || nx >= self.w {
            None
        } else {
            Some((nx, ny))
        }
    }

    /// `TerrainBuilder.GetFractalFlags()`: what a field has to do to fit the
    /// map it is laid over.
    fn fractal_flags(self) -> u32 {
        if self.wrap_x {
            FRAC_WRAP_X
        } else {
            0
        }
    }
}

/// A Lua table of plot types, keyed by whatever integer a script used.
#[derive(Clone)]
struct PlotTable {
    cells: Vec<i8>,
}

impl PlotTable {
    fn new() -> Self {
        PlotTable { cells: Vec::new() }
    }

    fn get(&self, index: i32) -> i8 {
        if index < 0 {
            return PLOT_NONE;
        }
        self.cells.get(index as usize).copied().unwrap_or(PLOT_NONE)
    }

    fn set(&mut self, index: i32, value: i8) {
        if index < 0 {
            return;
        }
        let index = index as usize;
        if index >= self.cells.len() {
            self.cells.resize(index + 1, PLOT_NONE);
        }
        self.cells[index] = value;
    }
}

/// `TerrainBuilder.GetRandomNumber(n, ...)`: zero up to `n - 1`. The engine
/// takes an integer, so a script's fractional `n` arrives truncated, and a
/// die with no sides lands on zero.
fn roll(rng: &mut Rng, sides: i32) -> i32 {
    if sides <= 0 {
        0
    } else {
        rng.below(sides as usize) as i32
    }
}

/// A finished stock world: one plot type and one climate per plot, indexed
/// `y * w + x` with `y` counted from the southern edge.
pub struct StockWorld {
    pub grid: Grid,
    pub plot_types: Vec<i8>,
    pub climate: Vec<Climate>,
}

impl StockWorld {
    pub fn plot(&self, x: i32, y: i32) -> i8 {
        self.plot_types[(y * self.grid.w + x) as usize]
    }

    pub fn climate_at(&self, x: i32, y: i32) -> Climate {
        self.climate[(y * self.grid.w + x) as usize]
    }
}

/// `world_age_normal`, which the Standard world-age option selects. Every
/// script names it 3 but Island Plates, which names it 2.
fn world_age_normal(script: StockScript) -> i32 {
    match script {
        StockScript::IslandPlates => 2,
        _ => 3,
    }
}

/// Whether the stock script's map wraps east to west. Inland Sea alone asks
/// for a map with four edges (`GetMapInitData` returns `WrapX = false`).
pub fn wraps_east_west(script: StockScript) -> bool {
    script != StockScript::InlandSea
}

/// Run a stock script as far as `ApplyBaseTerrain`: land and sea, hills and
/// mountains, and the climate painted over them.
///
/// `thermal` replaces latitude with a field of the caller's own, for a world
/// whose heat does not follow its poles; the scripts themselves always pass
/// the row.
pub fn generate(
    script: StockScript,
    grid: Grid,
    scattered_heat: bool,
    rng: &mut Rng,
) -> StockWorld {
    let world_age = world_age_normal(script);
    let mut plot_table = match script {
        StockScript::Continents => continents_plot_types(grid, world_age, rng),
        StockScript::Pangaea => pangaea_plot_types(grid, world_age, rng),
        StockScript::SmallContinents => small_continents_plot_types(grid, world_age, rng),
        StockScript::IslandPlates => island_plates_plot_types(grid, world_age, rng),
        StockScript::InlandSea => inland_sea_plot_types(grid, world_age, rng),
        StockScript::Lakes => lakes_plot_types(grid, rng),
    };
    let no_coastal_mountains = script == StockScript::Pangaea;
    let climate = generate_terrain_types(
        grid,
        &mut plot_table,
        no_coastal_mountains,
        scattered_heat,
        rng,
    );
    // Every later pass reads the table by plot index. An entry no pass wrote
    // is not Ocean and not Mountain to any of them, which makes it flat land.
    let plot_types = (0..grid.plots())
        .map(|index| match plot_table.get(index) {
            PLOT_NONE => PLOT_LAND,
            plot => plot,
        })
        .collect();
    StockWorld {
        grid,
        plot_types,
        climate,
    }
}

/// The size of the biggest land area on a grid of land flags, which is what
/// `AreaBuilder.Recalculate()` and `Areas.FindBiggestArea(false)` report.
fn biggest_land_area(grid: Grid, land: &[bool]) -> i32 {
    let mut seen = vec![false; land.len()];
    let mut stack = Vec::new();
    let mut biggest = 0;
    for start in 0..land.len() {
        if !land[start] || seen[start] {
            continue;
        }
        seen[start] = true;
        stack.push(start as i32);
        let mut size = 0;
        while let Some(index) = stack.pop() {
            size += 1;
            let (x, y) = (index % grid.w, index / grid.w);
            for direction in 0..NUM_DIRECTIONS {
                if let Some((nx, ny)) = grid.adjacent(x, y, direction) {
                    let neighbor = (ny * grid.w + nx) as usize;
                    if land[neighbor] && !seen[neighbor] {
                        seen[neighbor] = true;
                        stack.push(neighbor as i32);
                    }
                }
            }
        }
        biggest = biggest.max(size);
    }
    biggest
}

/// `InitFractal` (the same in every script that has one): the continental
/// field, pinned to zero along its edges, optionally rifted down the seam,
/// with a little plate ridge blended in "to roughen the coastlines and build
/// inland seas".
fn init_fractal(grid: Grid, continent_grain: i32, rift_grain: i32, rng: &mut Rng) -> CvFractal {
    let mut field = if rift_grain > 0 && rift_grain < 4 {
        let rifts = CvFractal::create(rng, grid.w, grid.h, rift_grain, 0, 6, 5);
        CvFractal::create_rifts(
            rng,
            grid.w,
            grid.h,
            continent_grain,
            FRAC_POLAR,
            &rifts,
            6,
            5,
        )
    } else {
        CvFractal::create(rng, grid.w, grid.h, continent_grain, FRAC_POLAR, 6, 5)
    };
    // `MapSizeTypes[sizekey] or 4`: the table is keyed by size name and the
    // key is the size's number, so the lookup misses on every size.
    let num_plates = 4;
    field.build_ridges(rng, num_plates, 0, 1, 2);
    field
}

/// `GeneratePlotTypes` in `Continents.lua`.
fn continents_plot_types(grid: Grid, world_age: i32, rng: &mut Rng) -> PlotTable {
    let (w, h) = (grid.w, grid.h);
    // `sea_level_normal`.
    let water_percent = 62;
    let mut plot_types = PlotTable::new();

    // "Generate continental fractal layer and examine the largest landmass.
    // Reject the result until the largest landmass occupies 64% or less of
    // the total land."
    for attempt in 0.. {
        let grain_dice = if roll(rng, 7) < 4 { 2 } else { 1 };
        let rift_dice = match roll(rng, 3) {
            0 => -1,
            dice => dice,
        };
        let field = init_fractal(grid, grain_dice, rift_dice, rng);
        let water_threshold = field.height_from_percent(water_percent);
        let buffer = (f64::from(h) / 13.0).floor() as i32;
        let buffer2 = (f64::from(h) / 13.0 / 2.0).floor() as i32;

        // The table is the one table for every attempt: each fills the whole
        // map afresh, and whatever an earlier shift wrote past the map's end
        // is still there for the passes that read one cell along.
        let mut land = vec![false; (w * h) as usize];
        let mut total_land = 0;
        for x in 0..w {
            for y in 0..h {
                let i = y * w + x;
                let val = field.height(x, y);
                let mut is_land = false;
                if y <= buffer || y >= h - buffer - 1 {
                    // The polar rows are always sea.
                } else if val >= water_threshold {
                    if y <= buffer + buffer2 {
                        let random_roll = y - buffer + 1;
                        let random = roll(rng, random_roll);
                        is_land = random == 0 && random_roll > 0;
                    } else if y >= h - buffer - buffer2 - 1 {
                        let random_roll = h - y - buffer;
                        let random = roll(rng, random_roll);
                        is_land = random == 0 && random_roll > 0;
                    } else {
                        is_land = true;
                    }
                }
                if is_land {
                    plot_types.set(i, PLOT_LAND);
                    land[i as usize] = true;
                    total_land += 1;
                } else {
                    plot_types.set(i, PLOT_OCEAN);
                }
            }
        }

        shift_plot_types(grid, &mut plot_types);
        generate_center_rift(grid, &mut plot_types, rng);

        // The areas are recalculated from the terrain the loop above stamped
        // on the map, which neither the shift nor the rift has touched.
        let biggest = biggest_land_area(grid, &land);
        // A board too small to roll two continents keeps what it has rather
        // than rolling for ever.
        if f64::from(biggest) <= f64::from(total_land) * 0.64 || attempt >= 200 {
            break;
        }
    }

    let tectonics = Tectonics {
        world_age: f64::from(world_age),
        extra_mountains: 5.0,
        adjust_plates: 2.0,
        blend_ridge: 10,
        blend_fract: 5,
    };
    apply_tectonics(grid, &tectonics, &mut plot_types, rng);
    add_lonely_mountains(grid, &mut plot_types, f64::from(8 + world_age * 3), rng);
    plot_types
}

/// `GeneratePlotTypes` in `Pangaea.lua`.
///
/// The script goes on to shift its landmass toward a pole "to reduce how much
/// jungles tend to dominate". It looks for the Pangaea by comparing each
/// plot's area object with the biggest area's id, which never matches, so
/// the shift never runs and is not ported; only its one die is thrown.
fn pangaea_plot_types(grid: Grid, world_age: i32, rng: &mut Rng) -> PlotTable {
    let (w, h) = (grid.w, grid.h);
    let water_percent = 58;
    let mut plot_types = PlotTable::new();

    // "Reject the result until the largest landmass occupies 84% or more of
    // the total land."
    for attempt in 0.. {
        let grain_dice = if roll(rng, 7) < 4 { 1 } else { 2 };
        let rift_dice = match roll(rng, 3) {
            0 => -1,
            dice => dice,
        };
        let field = init_fractal(grid, grain_dice, rift_dice, rng);
        let water_threshold = field.height_from_percent(water_percent);

        let mut land = vec![false; (w * h) as usize];
        let mut total_land = 0;
        for x in 0..w {
            for y in 0..h {
                let i = y * w + x;
                if field.height(x, y) <= water_threshold {
                    plot_types.set(i, PLOT_OCEAN);
                } else {
                    plot_types.set(i, PLOT_LAND);
                    land[i as usize] = true;
                    total_land += 1;
                }
            }
        }
        let biggest = biggest_land_area(grid, &land);
        if f64::from(biggest) >= f64::from(total_land) * 0.84 || attempt >= 200 {
            break;
        }
    }

    let tectonics = Tectonics {
        world_age: f64::from(world_age),
        extra_mountains: 4.0,
        adjust_plates: 2.0,
        blend_ridge: 10,
        blend_fract: 1,
    };
    apply_tectonics(grid, &tectonics, &mut plot_types, rng);
    let _shift_dice = roll(rng, 2);
    plot_types
}

/// `GeneratePlotTypes` in `Small_Continents.lua`.
fn small_continents_plot_types(grid: Grid, world_age: i32, rng: &mut Rng) -> PlotTable {
    let (w, h) = (grid.w, grid.h);
    let water_percent = 69;
    let water_percent_modifier = 0;
    let mut plot_types = PlotTable::new();

    // "Now test the biggest landmass to see if it is small enough."
    for attempt in 0.. {
        let field = init_fractal(grid, 3, -1, rng);
        let water_threshold = field.height_from_percent(water_percent);
        // The table is reused from one attempt to the next, so an entry past
        // the map that an earlier shift left behind is still there.
        let mut land = vec![false; (w * h) as usize];
        let mut total_land = 0;
        for x in 0..w {
            for y in 0..h {
                let i = y * w + x;
                if field.height(x, y) >= water_threshold {
                    plot_types.set(i, PLOT_LAND);
                    land[i as usize] = true;
                    total_land += 1;
                } else {
                    plot_types.set(i, PLOT_OCEAN);
                }
            }
        }
        shift_plot_types(grid, &mut plot_types);
        let biggest = biggest_land_area(grid, &land);
        if f64::from(biggest) <= f64::from(total_land) * 0.34 || attempt >= 200 {
            break;
        }
    }

    // "Generate Medium Islands", then "Generate Tiny Islands". `islands` is
    // the plot table itself, so each layer keeps one plot clear of whatever
    // land stood when it began.
    fractal_layer_without_hills(
        grid,
        &mut plot_types,
        FractalLayer {
            water_percent: 85 + water_percent_modifier,
            grain: 4,
            frac_x_exp: 7,
            frac_y_exp: 6,
            land_above_the_cut: true,
            clear_of_islands: true,
        },
        rng,
    );
    fractal_layer_without_hills(
        grid,
        &mut plot_types,
        FractalLayer {
            water_percent: 95 + water_percent_modifier,
            grain: 5,
            frac_x_exp: 7,
            frac_y_exp: 6,
            land_above_the_cut: true,
            clear_of_islands: true,
        },
        rng,
    );

    let tectonics = Tectonics {
        world_age: f64::from(world_age),
        extra_mountains: 10.0,
        adjust_plates: 3.0,
        blend_ridge: 10,
        blend_fract: 1,
    };
    apply_tectonics(grid, &tectonics, &mut plot_types, rng);
    add_lonely_mountains(grid, &mut plot_types, f64::from(8 + world_age * 3), rng);
    plot_types
}

/// `GeneratePlotTypes` in `Island_Plates.lua`: a layer of islands, a layer of
/// islets, and then a layer of water cut back out of both.
fn island_plates_plot_types(grid: Grid, world_age: i32, rng: &mut Rng) -> PlotTable {
    let (w, h) = (grid.w, grid.h);
    let mut plot_types = PlotTable::new();
    for x in 0..w {
        for y in 0..h {
            plot_types.set(y * w + x + 1, PLOT_OCEAN);
        }
    }
    let water_percent_modifier = 0;
    fractal_layer_without_hills(
        grid,
        &mut plot_types,
        FractalLayer {
            water_percent: 65 + water_percent_modifier,
            grain: 3,
            frac_x_exp: 7,
            frac_y_exp: 6,
            land_above_the_cut: true,
            clear_of_islands: false,
        },
        rng,
    );
    fractal_layer_without_hills(
        grid,
        &mut plot_types,
        FractalLayer {
            water_percent: 95 + water_percent_modifier,
            grain: 4,
            frac_x_exp: 7,
            frac_y_exp: 6,
            land_above_the_cut: true,
            clear_of_islands: false,
        },
        rng,
    );
    water_layer(grid, &mut plot_types, 90 + water_percent_modifier, 4, rng);

    let tectonics = Tectonics {
        world_age: f64::from(world_age) + 0.25,
        extra_mountains: 0.0,
        adjust_plates: 2.0,
        blend_ridge: 5,
        blend_fract: 5,
    };
    apply_tectonics(grid, &tectonics, &mut plot_types, rng);
    add_lonely_mountains(grid, &mut plot_types, f64::from(2 + world_age * 2), rng);
    plot_types
}

/// `GeneratePlotTypes` in `InlandSea.lua`: a world of land with a fractal sea
/// cut from its middle and an oval cut from the middle of that, so the sea is
/// always one sea however the fractal fell.
fn inland_sea_plot_types(grid: Grid, world_age: i32, rng: &mut Rng) -> PlotTable {
    let (w, h) = (grid.w, grid.h);
    let (fw, fh) = (f64::from(w), f64::from(h));
    // `table.fill(g_PLOT_TYPE_LAND, g_iW * g_iH)`, which fills from one.
    let mut plot_types = PlotTable::new();
    for index in 1..=w * h {
        plot_types.set(index, PLOT_LAND);
    }

    let west_x = (fw * 0.18).floor() as i32 - 1;
    let east_x = (fw * 0.82).ceil() as i32 - 1;
    let width = east_x - west_x;
    let south_y = (fh * 0.28).floor() as i32 - 1;
    let north_y = (fh * 0.72).ceil() as i32 - 1;
    let height = north_y - south_y;

    let grain = 1 + roll(rng, 2);
    let field = CvFractal::create(rng, width, height, grain, FRAC_POLAR, -1, -1);
    let sea_threshold = field.height_from_percent(60);
    for region_y in 0..height {
        for region_x in 0..width {
            if field.height(region_x, region_y) >= sea_threshold {
                let x = region_x + west_x;
                let y = region_y + south_y;
                plot_types.set(y * w + x + 1, PLOT_OCEAN);
            }
        }
    }

    // "Second, oval layer to ensure one main body of water."
    let center_x = (fw / 2.0) - 1.0;
    let center_y = (fh / 2.0) - 1.0;
    let x_axis = center_x / 2.0;
    let y_axis = center_y * 0.35;
    let x_axis_squared = x_axis * x_axis;
    let y_axis_squared = y_axis * y_axis;
    for x in 0..w {
        for y in 0..h {
            let delta_x = f64::from(x) - center_x;
            let delta_y = f64::from(y) - center_y;
            let oval_value =
                delta_x * delta_x / x_axis_squared + delta_y * delta_y / y_axis_squared;
            if oval_value <= 1.0 {
                plot_types.set(y * w + x + 1, PLOT_OCEAN);
            }
        }
    }

    let tectonics = Tectonics {
        world_age: f64::from(world_age),
        extra_mountains: f64::from(7 - world_age),
        adjust_plates: 2.0,
        blend_ridge: 10,
        blend_fract: 1,
    };
    apply_tectonics(grid, &tectonics, &mut plot_types, rng);
    add_lonely_mountains(grid, &mut plot_types, f64::from(13 + world_age), rng);
    plot_types
}

/// `GeneratePlotTypes` in `Lakes.lua`. Its layers cut the other way from
/// every other script's: a plot is land where the field lies *under* the
/// percentile, so the first layer alone leaves four fifths of the world dry.
/// The script also declares its own `world_age` of 1 over the one it was
/// handed, and that is the one its hills and mountains get.
fn lakes_plot_types(grid: Grid, rng: &mut Rng) -> PlotTable {
    let (w, h) = (grid.w, grid.h);
    let world_age = 1;
    let water_percent_modifier = 0;
    let mut plot_types = PlotTable::new();
    for x in 0..w {
        for y in 0..h {
            plot_types.set(y * w + x + 1, PLOT_OCEAN);
        }
    }
    for (water_percent, grain, frac_x_exp, frac_y_exp) in
        [(81, 3, 6, 5), (88, 4, 7, 6), (95, 5, 7, 6)]
    {
        fractal_layer_without_hills(
            grid,
            &mut plot_types,
            FractalLayer {
                water_percent: water_percent + water_percent_modifier,
                grain,
                frac_x_exp,
                frac_y_exp,
                land_above_the_cut: false,
                clear_of_islands: true,
            },
            rng,
        );
    }

    let tectonics = Tectonics {
        world_age: f64::from(world_age),
        extra_mountains: 5.0,
        adjust_plates: 2.0,
        blend_ridge: 10,
        blend_fract: 5,
    };
    apply_tectonics(grid, &tectonics, &mut plot_types, rng);
    add_lonely_mountains(grid, &mut plot_types, f64::from(8 + world_age * 3), rng);
    plot_types
}

/// One call of `GenerateFractalLayerWithoutHills`, over the whole map.
struct FractalLayer {
    water_percent: i32,
    grain: i32,
    frac_x_exp: i32,
    frac_y_exp: i32,
    /// `val <= iWaterThreshold` leaves a plot wet everywhere but in
    /// `Lakes.lua`, which wrote `>=`.
    land_above_the_cut: bool,
    /// Whether the script's copy of the function asks `Adjacent(i)`, which
    /// keeps a layer one plot clear of the land already standing. Island
    /// Plates' copy does not.
    clear_of_islands: bool,
}

/// `Adjacent(index)`: whether the plot, or any plot round it, is flat land in
/// the `islands` table. The scripts point `islands` at the plot table itself,
/// so it is always the world as it stands.
fn adjacent_to_islands(grid: Grid, islands: &PlotTable, index: i32) -> bool {
    let index = index - 1;
    if index < 0 {
        return false;
    }
    if islands.get(index) == PLOT_LAND {
        return true;
    }
    let (x, y) = (index % grid.w, index / grid.w);
    if y >= grid.h {
        return false;
    }
    (0..NUM_DIRECTIONS).any(|direction| {
        grid.adjacent(x, y, direction)
            .is_some_and(|(nx, ny)| islands.get(ny * grid.w + nx) == PLOT_LAND)
    })
}

/// `GenerateFractalLayerWithoutHills`: one more field cut at a percentile and
/// laid over the world, after the world has been shifted to put its emptiest
/// columns on the seam.
fn fractal_layer_without_hills(
    grid: Grid,
    plot_types: &mut PlotTable,
    layer: FractalLayer,
    rng: &mut Rng,
) {
    let (w, h) = (grid.w, grid.h);
    let field = CvFractal::create(
        rng,
        w,
        h,
        layer.grain,
        grid.fractal_flags(),
        layer.frac_x_exp,
        layer.frac_y_exp,
    );
    let water_threshold = field.height_from_percent(layer.water_percent);
    let mut layer_land = vec![false; (w * h) as usize];
    for x in 0..w {
        for y in 0..h {
            let i = y * w + x + 1;
            let val = field.height(x, y);
            let wet = if layer.land_above_the_cut {
                val <= water_threshold
            } else {
                val >= water_threshold
            };
            if !(wet || layer.clear_of_islands && adjacent_to_islands(grid, plot_types, i)) {
                layer_land[(i - 1) as usize] = true;
            }
        }
    }

    shift_plot_types(grid, plot_types);

    for x in 0..w {
        for y in 0..h {
            let index = y * w + x + 1;
            if layer_land[(index - 1) as usize] {
                plot_types.set(index, PLOT_LAND);
            }
        }
    }
}

/// `GenerateWaterLayer` (`Island_Plates.lua`): the same cut, but what stands
/// above it is turned to sea.
fn water_layer(
    grid: Grid,
    plot_types: &mut PlotTable,
    water_percent: i32,
    grain: i32,
    rng: &mut Rng,
) {
    let (w, h) = (grid.w, grid.h);
    let field = CvFractal::create(rng, w, h, grain, grid.fractal_flags(), 7, 6);
    let water_threshold = field.height_from_percent(water_percent);
    shift_plot_types(grid, plot_types);
    for x in 0..w {
        for y in 0..h {
            if field.height(x, y) > water_threshold {
                plot_types.set(y * w + x + 1, PLOT_OCEAN);
            }
        }
    }
}

/// `ShiftPlotTypes` (`MapUtilities.lua`): turn the world so its emptiest
/// columns lie on the east-west seam and its emptiest rows on the north-south
/// one.
fn shift_plot_types(grid: Grid, plot_types: &mut PlotTable) {
    let x_shift = determine_x_shift(grid, plot_types);
    let y_shift = determine_y_shift(grid, plot_types);
    shift_plot_types_by(grid, plot_types, x_shift, y_shift);
}

/// `ShiftPlotTypesBy`. Both loops run one past the map, as written.
fn shift_plot_types_by(grid: Grid, plot_types: &mut PlotTable, x_shift: i32, y_shift: i32) {
    let (w, h) = (grid.w, grid.h);
    if !(x_shift > 0 || y_shift > 0) {
        return;
    }
    let buffer = plot_types.clone();
    for dest_y in 0..=h {
        for dest_x in 0..=w {
            let dest = w * dest_y + dest_x;
            let source_x = (dest_x + x_shift) % w;
            let source_y = (dest_y + y_shift) % h;
            plot_types.set(dest, buffer.get(w * source_y + source_x));
        }
    }
}

/// `DetermineXShift`: "align the most water-heavy vertical portion of the map
/// with the vertical map edge".
fn determine_x_shift(grid: Grid, plot_types: &PlotTable) -> i32 {
    let (w, h) = (grid.w, grid.h);
    let land_totals: Vec<i32> = (0..w)
        .map(|x| {
            (0..h)
                .filter(|y| plot_types.get(y * w + x + 1) != PLOT_OCEAN)
                .count() as i32
        })
        .collect();
    let group_radius = w / 10;
    let mut best_value = h * (2 * group_radius + 1);
    let mut best_group = 1;
    for column_index in 1..=w {
        let mut group_total = 0;
        for column in column_index - group_radius..=column_index + group_radius {
            let mut index = column.rem_euclid(w);
            if index == 0 {
                index = w;
            }
            group_total += land_totals[(index - 1) as usize];
        }
        if group_total < best_value {
            best_value = group_total;
            best_group = column_index;
        }
    }
    best_group - 1
}

/// `DetermineYShift`, the same for rows.
fn determine_y_shift(grid: Grid, plot_types: &PlotTable) -> i32 {
    let (w, h) = (grid.w, grid.h);
    let land_totals: Vec<i32> = (0..h)
        .map(|y| {
            (0..w)
                .filter(|x| plot_types.get(y * w + x + 1) != PLOT_OCEAN)
                .count() as i32
        })
        .collect();
    let group_radius = h / 15;
    let mut best_value = w * (2 * group_radius + 1);
    let mut best_group = 1;
    for row_index in 1..=h {
        let mut group_total = 0;
        for row in row_index - group_radius..=row_index + group_radius {
            let mut index = row.rem_euclid(h);
            if index == 0 {
                index = h;
            }
            group_total += land_totals[(index - 1) as usize];
        }
        if group_total < best_value {
            best_value = group_total;
            best_group = row_index;
        }
    }
    best_group - 1
}

/// `GenerateCenterRift` (`Continents.lua`): "Causes a rift to break apart and
/// separate any landmasses overlaying the map center. Rift runs south to
/// north ala the Atlantic Ocean."
///
/// The line climbs from the southern edge in segments that lean one way,
/// doubling back at a boundary column; then everything west of it is slid
/// three plots west and two rows along the lean, everything east the other
/// way, and the gap left between is sea.
fn generate_center_rift(grid: Grid, plot_types: &mut PlotTable, rng: &mut Rng) {
    let (w, h) = (grid.w, grid.h);
    // 0 = starts west, leans east. 1 = starts east, leans west.
    let rift_lean = roll(rng, 2);

    // The row tables are keyed the way the script keys them: by `y + 1` where
    // it starts and ends the line, and by the row just climbed to in between.
    let mut west_of_rift = vec![0i32; (h + 2) as usize];
    let mut east_of_rift = vec![0i32; (h + 2) as usize];

    let primary_max_length = (h / 8).max(1);
    let secondary_max_length = (h / 11).max(1);
    let tertiary_max_length = (h / 14).max(1);

    let mut start_distance = h / 8;
    if rift_lean == 0 {
        start_distance = -start_distance;
    }
    let start_x = w / 2 + start_distance;
    let rift_x_boundary = w / 2 - start_distance;

    // The primary direction leans with the rift, the secondary against it,
    // and the tertiary runs flat along the lean.
    let (primary, secondary, tertiary) = if rift_lean == 0 {
        (NORTHEAST, NORTHWEST, EAST)
    } else {
        (NORTHWEST, NORTHEAST, WEST)
    };
    let past_boundary = |x: i32| {
        if rift_lean == 0 {
            x >= rift_x_boundary
        } else {
            x <= rift_x_boundary
        }
    };

    let mut current_direction = primary;
    let mut current_x = start_x;
    let mut current_y = 0;
    west_of_rift[1] = current_x - 1;
    east_of_rift[1] = current_x + 1;
    plot_types.set(current_x + 1, PLOT_OCEAN);

    while current_y < h - 1 {
        let next_direction;
        if current_direction == tertiary {
            let segment_length = roll(rng, tertiary_max_length + 1);
            next_direction = if past_boundary(current_x) {
                secondary
            } else if roll(rng, 3) == 1 {
                secondary
            } else {
                primary
            };
            for _ in 0..segment_length {
                let row = current_y as usize;
                if rift_lean == 0 {
                    current_x += 1;
                    east_of_rift[row] = current_x + 1;
                } else {
                    current_x -= 1;
                    west_of_rift[row] = current_x - 1;
                }
                plot_types.set(current_y * w + current_x + 1, PLOT_OCEAN);
            }
        } else {
            let segment_length;
            if current_direction == secondary {
                segment_length = roll(rng, secondary_max_length + 1);
                next_direction = if past_boundary(current_x) {
                    secondary
                } else if roll(rng, 4) == 2 {
                    tertiary
                } else {
                    primary
                };
            } else {
                segment_length = roll(rng, primary_max_length + 1);
                next_direction = if past_boundary(current_x) {
                    secondary
                } else if roll(rng, 2) == 1 && f64::from(current_y) > f64::from(h) * 0.28 {
                    tertiary
                } else {
                    secondary
                };
            }
            let mut plots_to_do = segment_length;
            while plots_to_do > 0 && current_y < h - 1 {
                if let Some((next_x, _)) = grid.adjacent(current_x, current_y, current_direction) {
                    current_x = next_x;
                }
                current_y += 1;
                let row = current_y as usize;
                west_of_rift[row] = current_x - 1;
                east_of_rift[row] = current_x + 1;
                plot_types.set(current_y * w + current_x + 1, PLOT_OCEAN);
                plots_to_do -= 1;
            }
        }
        current_direction = next_direction;
    }
    // "Process the final plot in the rift."
    west_of_rift[h as usize] = current_x - 1;
    east_of_rift[h as usize] = current_x + 1;
    plot_types.set((h - 1) * w + current_x + 1, PLOT_OCEAN);

    // "Now force the rift to widen, causing land on either side of the rift
    // to drift apart."
    let horizontal_drift = 3;
    let vertical_drift = 2;
    let west = |y: i32| west_of_rift[(y + 1) as usize];
    let east = |y: i32| east_of_rift[(y + 1) as usize];
    let copy = |plot_types: &mut PlotTable, source: i32, dest: i32| {
        let value = plot_types.get(source);
        plot_types.set(dest, value);
    };

    if rift_lean == 0 {
        // Western side from the top down.
        for y in (0..=h - 1 - vertical_drift).rev() {
            for x in horizontal_drift..=west(y) {
                let source = y * w + x + 1;
                let dest = (y + vertical_drift) * w + (x - horizontal_drift) + 1;
                copy(plot_types, source, dest);
            }
        }
        // Eastern side from the bottom up.
        for y in vertical_drift..h {
            for x in east(y)..=w - horizontal_drift - 1 {
                let source = y * w + x + 1;
                let dest = (y - vertical_drift) * w + (x + horizontal_drift) + 1;
                copy(plot_types, source, dest);
            }
        }
        // Bottom left.
        for y in 0..vertical_drift {
            for x in 0..=west(y) {
                plot_types.set(y * w + x + 1, PLOT_OCEAN);
            }
        }
        // Top right.
        for y in h - vertical_drift..h {
            for x in east(y)..w {
                plot_types.set(y * w + x + 1, PLOT_OCEAN);
            }
        }
        // The rift itself.
        for y in vertical_drift..=h - 1 - vertical_drift {
            let west_x = west(y - vertical_drift) - horizontal_drift + 1;
            let east_x = east(y + vertical_drift) + horizontal_drift - 1;
            for x in west_x..=east_x {
                plot_types.set(y * w + x + 1, PLOT_OCEAN);
            }
        }
    } else {
        // Western side from the bottom up.
        for y in vertical_drift..h {
            for x in horizontal_drift..=west(y) {
                let source = y * w + x + 1;
                let dest = (y - vertical_drift) * w + (x - horizontal_drift) + 1;
                copy(plot_types, source, dest);
            }
        }
        // Eastern side from the top down.
        for y in (0..=h - 1 - vertical_drift).rev() {
            for x in east(y)..=w - horizontal_drift - 1 {
                let source = y * w + x + 1;
                let dest = (y + vertical_drift) * w + (x + horizontal_drift) + 1;
                copy(plot_types, source, dest);
            }
        }
        // Top left.
        for y in h - vertical_drift..h {
            for x in 0..=west(y) {
                plot_types.set(y * w + x + 1, PLOT_OCEAN);
            }
        }
        // Bottom right.
        for y in 0..vertical_drift {
            for x in east(y)..w {
                plot_types.set(y * w + x + 1, PLOT_OCEAN);
            }
        }
        // The rift itself.
        for y in vertical_drift..=h - 1 - vertical_drift {
            let west_x = west(y + vertical_drift) - horizontal_drift + 1;
            let east_x = east(y - vertical_drift) + horizontal_drift - 1;
            for x in west_x..=east_x {
                plot_types.set(y * w + x + 1, PLOT_OCEAN);
            }
        }
    }
}

/// The arguments a script hands `ApplyTectonics`.
struct Tectonics {
    world_age: f64,
    extra_mountains: f64,
    /// `args.adjust_plates or 2.0`: no stock script's own `adjust_plates`
    /// local reaches the call, so this is 2.0 unless the script passes one.
    adjust_plates: f64,
    blend_ridge: i32,
    blend_fract: i32,
}

/// `AdjacentToWater(x, y, plotTypes)`, with the two indexings it mixes: the
/// plot itself is read one past its index and its neighbours at theirs.
fn adjacent_to_water(grid: Grid, plot_types: &PlotTable, x: i32, y: i32) -> bool {
    if plot_types.get(y * grid.w + x + 1) == PLOT_OCEAN {
        return false;
    }
    (0..NUM_DIRECTIONS).any(|direction| {
        grid.adjacent(x, y, direction)
            .is_some_and(|(nx, ny)| plot_types.get(ny * grid.w + nx) == PLOT_OCEAN)
    })
}

/// `ApplyTectonics` (`MountainsCliffs.lua`): two fields with plate ridges
/// woven through them, cut at percentiles. A plot high in the mountain field
/// is a Mountain unless the hills field makes it a pass; a plot just under
/// that is a foothill; and anywhere else the hills field's two bands are
/// Hills. Then nine in ten of the mountains that stand on a coast come down.
fn apply_tectonics(grid: Grid, args: &Tectonics, plot_types: &mut PlotTable, rng: &mut Rng) {
    let (w, h) = (grid.w, grid.h);
    let adjustment = args.world_age;
    let mut adjust_plates = args.adjust_plates;
    if adjustment < 3.0 {
        adjust_plates *= 0.75;
    } else if adjustment > 3.0 {
        adjust_plates *= 1.5;
    }

    let hills_bottom1 = 28.0 - adjustment;
    let hills_top1 = 28.0 + adjustment;
    let hills_bottom2 = 72.0 - adjustment;
    let hills_top2 = 72.0 + adjustment;
    let hills_near_mountains = 91.0 - (adjustment * 2.0) - args.extra_mountains;
    let mountains = 97.0 - adjustment - args.extra_mountains;

    let num_plates = (9.0 * adjust_plates) as i32;
    let flags = grid.fractal_flags();
    let grain = 3;

    let mut hills_frac = CvFractal::create(rng, w, h, grain, flags, -1, -1);
    let mut mountains_frac = CvFractal::create(rng, w, h, grain, flags, -1, -1);
    hills_frac.build_ridges(rng, num_plates, flags, args.blend_ridge, args.blend_fract);
    mountains_frac.build_ridges(rng, num_plates, flags, args.blend_ridge, args.blend_fract);

    let i_hills_bottom1 = hills_frac.height_from_percent(hills_bottom1 as i32);
    let i_hills_top1 = hills_frac.height_from_percent(hills_top1 as i32);
    let i_hills_bottom2 = hills_frac.height_from_percent(hills_bottom2 as i32);
    let i_hills_top2 = hills_frac.height_from_percent(hills_top2 as i32);
    let i_hills_near_mountains = mountains_frac.height_from_percent(hills_near_mountains as i32);
    let i_mountain_threshold = mountains_frac.height_from_percent(mountains as i32);
    let i_pass_threshold = hills_frac.height_from_percent(hills_near_mountains as i32);

    for x in 0..w {
        for y in 0..h {
            let mountain_val = mountains_frac.height(x, y);
            let hill_val = hills_frac.height(x, y);
            let i = y * w + x + 1;
            if plot_types.get(i) == PLOT_OCEAN {
                continue;
            }
            let plot = if mountain_val >= i_mountain_threshold {
                if hill_val >= i_pass_threshold {
                    // "Mountain Pass though the ridgeline"
                    PLOT_HILLS
                } else {
                    PLOT_MOUNTAIN
                }
            } else if mountain_val >= i_hills_near_mountains {
                // "Foot hills"
                PLOT_HILLS
            } else if (hill_val >= i_hills_bottom1 && hill_val <= i_hills_top1)
                || (hill_val >= i_hills_bottom2 && hill_val <= i_hills_top2)
            {
                PLOT_HILLS
            } else {
                PLOT_LAND
            };
            plot_types.set(i, plot);
        }
    }

    // "Remove Random Coastal Mountains"
    for x in 0..w {
        for y in 0..h {
            let i = y * w + x + 1;
            if plot_types.get(i) == PLOT_MOUNTAIN && adjacent_to_water(grid, plot_types, x, y) {
                if roll(rng, 10) < 9 {
                    plot_types.set(i, PLOT_HILLS);
                }
            }
        }
    }
}

/// `GetShuffledCopyOfTable`: draw one of what is left, over and over.
fn shuffled(mut items: Vec<i32>, rng: &mut Rng) -> Vec<i32> {
    let mut out = Vec::with_capacity(items.len());
    while !items.is_empty() {
        let index = roll(rng, items.len() as i32) as usize;
        out.push(items.remove(index));
    }
    out
}

/// `CanAddLonelyMountains`: land with nothing but land round it, and the full
/// six of that.
fn can_add_lonely_mountain(grid: Grid, plot_types: &PlotTable, index: i32) -> bool {
    let plot = plot_types.get(index);
    if plot == PLOT_OCEAN || plot == PLOT_MOUNTAIN {
        return false;
    }
    let (x, y) = (index % grid.w, index / grid.w);
    (0..NUM_DIRECTIONS).all(|direction| match grid.adjacent(x, y, direction) {
        Some((nx, ny)) => {
            let neighbor = plot_types.get(ny * grid.w + nx);
            neighbor != PLOT_OCEAN && neighbor != PLOT_MOUNTAIN
        }
        None => false,
    })
}

/// `AddLonelyMountains` (`MountainsCliffs.lua`): bring the world up to one
/// mountain in every `mountain_ratio` land plots, with single peaks standing
/// clear of the sea and of each other. It always raises one more than it
/// counted, so it raises one even on a world already over the ratio.
fn add_lonely_mountains(
    grid: Grid,
    plot_types: &mut PlotTable,
    mountain_ratio: f64,
    rng: &mut Rng,
) {
    let (w, h) = (grid.w, grid.h);
    let mut total_land = 0;
    let mut total_mountains = 0;
    let mut indices = Vec::new();
    for x in 0..w {
        for y in 0..h {
            let index = y * w + x;
            let plot = plot_types.get(index);
            if plot != PLOT_OCEAN {
                total_land += 1;
                if plot == PLOT_MOUNTAIN {
                    total_mountains += 1;
                } else {
                    indices.push(index);
                }
            }
        }
    }
    let new_mountains =
        ((f64::from(total_land) / mountain_ratio).floor() as i32 - total_mountains).max(0);
    let mut mountains_set = 0;
    for index in shuffled(indices, rng) {
        if can_add_lonely_mountain(grid, plot_types, index) {
            mountains_set += 1;
            plot_types.set(index, PLOT_MOUNTAIN);
        }
        if mountains_set > new_mountains {
            break;
        }
    }
}

/// `IsAdjacentToLand(plotTypes, iX, iY)`.
fn is_adjacent_to_land(grid: Grid, plot_types: &PlotTable, x: i32, y: i32) -> bool {
    (0..NUM_DIRECTIONS).any(|direction| {
        grid.adjacent(x, y, direction)
            .is_some_and(|(nx, ny)| plot_types.get(ny * grid.w + nx) != PLOT_OCEAN)
    })
}

/// `IsAdjacentToShallowWater(terrainTypes, iX, iY)`.
fn is_adjacent_to_shallow_water(grid: Grid, climate: &[Climate], x: i32, y: i32) -> bool {
    (0..NUM_DIRECTIONS).any(|direction| {
        grid.adjacent(x, y, direction)
            .is_some_and(|(nx, ny)| climate[(ny * grid.w + nx) as usize] == Climate::Coast)
    })
}

/// `GenerateTerrainTypes` (`TerrainGenerator.lua`) at Standard temperature.
///
/// Water beside land is Coast and the rest Ocean. Land is banded by latitude,
/// the bands roughened by a variation field: Snow from 0.8, Tundra from 0.65,
/// Grass under 0.1, and between those Desert where the desert field's top
/// quarter falls between 0.2 and 0.5, Plains on the plains field's top half,
/// and Grass on what is left. Then three passes give each Ocean plot that
/// touches Coast one chance in four of becoming Coast itself.
fn generate_terrain_types(
    grid: Grid,
    plot_types: &mut PlotTable,
    no_coastal_mountains: bool,
    scattered_heat: bool,
    rng: &mut Rng,
) -> Vec<Climate> {
    let (w, h) = (grid.w, grid.h);
    let flags = grid.fractal_flags();
    let mut climate = vec![Climate::Ocean; (w * h) as usize];

    let desert_percent = 25;
    let plains_percent = 50;
    let snow_latitude = 0.8;
    let tundra_latitude = 0.65;
    let grass_latitude = 0.1;
    let desert_bottom_latitude = 0.2;
    let desert_top_latitude = 0.5;

    let grain = 3;
    let deserts = CvFractal::create(rng, w, h, grain, flags, -1, -1);
    let desert_top = deserts.height_from_percent(100);
    let desert_bottom = deserts.height_from_percent(100 - desert_percent);
    let plains = CvFractal::create(rng, w, h, grain, flags, -1, -1);
    let plains_top = plains.height_from_percent(100);
    let plains_bottom = plains.height_from_percent(100 - plains_percent);
    let variation = CvFractal::create(rng, w, h, grain, flags, -1, -1);
    // Not in the script: a world whose heat ignores its poles reads its
    // latitude off a field of its own, drawn only when that world is asked
    // for, so every other world's dice fall exactly as the script's do.
    let thermal = scattered_heat.then(|| CvFractal::create(rng, w, h, grain, flags, -1, -1));

    for x in 0..w {
        for y in 0..h {
            let index = y * w + x;
            if plot_types.get(index) == PLOT_OCEAN {
                climate[index as usize] = if is_adjacent_to_land(grid, plot_types, x, y) {
                    Climate::Coast
                } else {
                    Climate::Ocean
                };
            }
        }
    }

    if no_coastal_mountains {
        // `RemoveCoastalMountains`.
        for x in 0..w {
            for y in 0..h {
                let index = y * w + x;
                if plot_types.get(index) == PLOT_MOUNTAIN
                    && is_adjacent_to_shallow_water(grid, &climate, x, y)
                {
                    plot_types.set(index, PLOT_HILLS);
                }
            }
        }
    }

    for x in 0..w {
        for y in 0..h {
            let index = y * w + x;
            if plot_types.get(index) == PLOT_OCEAN {
                continue;
            }
            // `GetLatitudeAtPlot`.
            let base = match &thermal {
                Some(thermal) => f64::from(thermal.height(x, y)) / 255.0,
                None => (f64::from(h) / 2.0 - f64::from(y)).abs() / (f64::from(h) / 2.0),
            };
            let lat =
                (base + f64::from(128 - variation.height(x, y)) / (255.0 * 5.0)).clamp(0.0, 1.0);
            climate[index as usize] = if lat >= snow_latitude {
                Climate::Snow
            } else if lat >= tundra_latitude {
                Climate::Tundra
            } else if lat < grass_latitude {
                Climate::Grass
            } else {
                let desert_val = deserts.height(x, y);
                let plains_val = plains.height(x, y);
                if desert_val >= desert_bottom
                    && desert_val <= desert_top
                    && lat >= desert_bottom_latitude
                    && lat < desert_top_latitude
                {
                    Climate::Desert
                } else if plains_val >= plains_bottom && plains_val <= plains_top {
                    Climate::Plains
                } else {
                    Climate::Grass
                }
            };
        }
    }

    // "Expanding coasts"
    for _ in 0..3 {
        let mut shallow = Vec::new();
        for x in 0..w {
            for y in 0..h {
                let index = (y * w + x) as usize;
                if climate[index] == Climate::Ocean
                    && is_adjacent_to_shallow_water(grid, &climate, x, y)
                    && roll(rng, 4) == 0
                {
                    shallow.push(index);
                }
            }
        }
        for index in shallow {
            climate[index] = Climate::Coast;
        }
    }
    climate
}

#[cfg(test)]
mod tests;
