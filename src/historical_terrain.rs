//! The ground each historical battle was actually fought on.
//!
//! [`crate::historical_scenarios`] is the catalogue: who fought, when, with
//! what, and one line promising what the field looks like. This module is the
//! field itself. Every battle here declares its real geography — the pass, the
//! ridge, the marsh, the river bend, the two woods — as a short list of strokes
//! laid over a base, and the same list decides where the two sides form up.
//!
//! # Why this exists
//!
//! The catalogue's map promises were written from the histories ("marsh, beach,
//! and open running ground"; "a narrow road below the ridge"; "the Round Tops
//! anchor the flank") but nothing painted them: every battle got the same
//! coordinate-hash noise — grassland with a scatter of hills, a few mountains
//! sprinkled by `hash % 19`, and for five of them a straight river down the
//! middle column. Thermopylae, whose entire military point is that the ground
//! is narrow, was open ground with random mountains on it. A briefing that
//! describes one battlefield while the board shows another teaches the reader
//! the wrong thing about the battle, and it costs the scenario the only
//! quality it has over a rolled arena: that the terrain is *evidence*.
//!
//! # What a plan claims
//!
//! A [`Plan`] is a sketch map, not a survey. The coordinates are normalized —
//! `(0.0, 0.0)` is the north-west corner of the chart and `(1.0, 1.0)` the
//! south-east — so a plan reads as proportions of the field ("the ridge runs
//! two-thirds of the width, a third of the way down") and stays true when a
//! chart is resized. Each stroke is a shape and the paint it lays down; later
//! strokes paint over earlier ones, so a plan is read top to bottom like map
//! layers. What a plan does *not* claim is metric accuracy: a hex is a
//! coarse unit, and where a real field's feature is smaller than one, the
//! plan keeps the feature's *tactical* consequence — a frontage, a flank
//! anchor, a blocked approach — rather than its footprint.
//!
//! Orientation is stated per battle in its own comment, because it is the
//! first thing a reader needs and the easiest thing to get backwards.

use std::collections::BTreeSet;

use crate::hex;
use crate::world::WorldMap;
use crate::Pos;

/// A point on the chart in normalized coordinates: `x` runs 0.0 (west edge) to
/// 1.0 (east edge), `y` runs 0.0 (north edge) to 1.0 (south edge).
#[derive(Clone, Copy, Debug)]
pub struct P {
    pub x: f32,
    pub y: f32,
}

/// Shorthand so a plan reads as a sketch rather than as struct literals.
pub const fn p(x: f32, y: f32) -> P {
    P { x, y }
}

/// The region a stroke covers.
#[derive(Clone, Copy, Debug)]
pub enum Shape {
    /// Everything within `reach` of the segment `from`–`to`. The workhorse:
    /// ridges, roads, rivers, shorelines, walls, hedgerows and treelines are
    /// all thick lines.
    Band { from: P, to: P, reach: f32 },
    /// Everything within `radius` of a point: a knoll, a wood, a village, an
    /// island, a redoubt.
    Blob { at: P, radius: f32 },
    /// An axis-aligned region, corner to corner.
    Area { from: P, to: P },
    /// Everything on the far side of the line `from`–`to`, taking "far" as the
    /// side on your LEFT as you walk from `from` toward `to` across the drawn
    /// chart — so a shoreline drawn west-to-east floods the north, and drawn
    /// east-to-west floods the south. (Left is read on the picture, where `y`
    /// increases downward, rather than in the sign convention of a graph whose
    /// `y` increases upward; the two are mirror images and this is the one a
    /// person drawing a map is holding in their head.) Used for the sea beyond
    /// a shore and the high ground beyond a crest, where the feature runs off
    /// the edge of the chart rather than ending on it.
    Beyond { from: P, to: P },
    /// The whole chart. A base coat, or a wash that later strokes cut into.
    All,
}

/// What a stroke lays down. A stroke may carry several, applied in order.
#[derive(Clone, Copy, Debug)]
pub enum Paint {
    /// One of the ruleset's nine terrains.
    Terrain(&'static str),
    /// Raise or clear hills. Hills are the engine's only elevation, so a plan
    /// says "hills" for a rise a unit can fight on and "mountain" (a terrain)
    /// for ground it cannot cross at all.
    Hills(bool),
    /// Place or clear a feature: `forest`, `jungle`, `marsh`, `floodplains`,
    /// `oasis`, `reef`.
    Feature(Option<&'static str>),
    /// A river along this stroke, as edges on the hexes it runs through.
    /// Rivers in this engine are edges rather than tiles, so a river band
    /// marks the edges facing across the stroke.
    River,
    /// Cliff edges facing the same way a river's would. A shoreline a landing
    /// cannot simply walk up.
    Cliff,
    /// An improvement, for the few fields whose works are the battle: the
    /// Phocian wall, the Theodosian walls, a redoubt line.
    Improvement(Option<&'static str>),
}

/// One layer of a battlefield.
#[derive(Clone, Copy, Debug)]
pub struct Stroke {
    pub shape: Shape,
    pub paint: &'static [Paint],
}

/// Shorthand for a stroke.
pub const fn stroke(shape: Shape, paint: &'static [Paint]) -> Stroke {
    Stroke { shape, paint }
}

/// Where one side formed up: a front line, given as the segment its order of
/// battle is laid along. Units fill from the segment's start toward its end,
/// which is why a plan puts the historically decisive wing first.
#[derive(Clone, Copy, Debug)]
pub struct Front {
    pub from: P,
    pub to: P,
}

/// Shorthand for a front.
pub const fn front(from: P, to: P) -> Front {
    Front { from, to }
}

/// A whole battlefield: what the ground is made of, and where the two armies
/// stood on it at the moment the scenario opens.
#[derive(Clone, Copy, Debug)]
pub struct Plan {
    /// The catalogue id this plan draws.
    pub id: &'static str,
    /// The base terrain the strokes are laid over.
    pub base: &'static str,
    /// Whether the base coat starts as hills. Rare — a plan usually raises
    /// specific ground — but a few fields are rolling country throughout.
    pub base_hills: bool,
    pub strokes: &'static [Stroke],
    /// Index 0 is the first force in the catalogue row, index 1 the second.
    pub fronts: [Front; 2],
}

// ---------------------------------------------------------------- geometry

/// Where a normalized point falls on a chart, in offset (column, row) space.
fn chart_point(wm: &WorldMap, point: P) -> (f32, f32) {
    (
        point.x * (wm.width - 1).max(1) as f32,
        point.y * (wm.height - 1).max(1) as f32,
    )
}

/// Distance from a point to a segment, in chart columns/rows.
fn distance_to_segment(at: (f32, f32), from: (f32, f32), to: (f32, f32)) -> f32 {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let span = dx * dx + dy * dy;
    // A zero-length segment is a point, and the projection below would divide
    // by nothing.
    let t = if span <= f32::EPSILON {
        0.0
    } else {
        (((at.0 - from.0) * dx + (at.1 - from.1) * dy) / span).clamp(0.0, 1.0)
    };
    let (px, py) = (from.0 + t * dx, from.1 + t * dy);
    ((at.0 - px).powi(2) + (at.1 - py).powi(2)).sqrt()
}

/// Which side of the line `from`–`to` a point lies on. Positive is the
/// left-hand side looking from `from` toward `to`.
fn side_of_line(at: (f32, f32), from: (f32, f32), to: (f32, f32)) -> f32 {
    (to.0 - from.0) * (at.1 - from.1) - (to.1 - from.1) * (at.0 - from.0)
}

/// Whether a chart tile falls inside a shape.
fn covers(wm: &WorldMap, shape: &Shape, pos: Pos) -> bool {
    let (col, row) = hex::axial_to_offset(pos.0, pos.1);
    let at = (col as f32, row as f32);
    match *shape {
        Shape::All => true,
        Shape::Band { from, to, reach } => {
            let reach = reach * (wm.width - 1).max(1) as f32;
            distance_to_segment(at, chart_point(wm, from), chart_point(wm, to)) <= reach
        }
        Shape::Blob { at: center, radius } => {
            let radius = radius * (wm.width - 1).max(1) as f32;
            let (cx, cy) = chart_point(wm, center);
            ((at.0 - cx).powi(2) + (at.1 - cy).powi(2)).sqrt() <= radius
        }
        Shape::Area { from, to } => {
            let (x0, y0) = chart_point(wm, from);
            let (x1, y1) = chart_point(wm, to);
            at.0 >= x0.min(x1) && at.0 <= x0.max(x1) && at.1 >= y0.min(y1) && at.1 <= y0.max(y1)
        }
        // Negative is the left-hand side on the drawn chart: the picture's `y`
        // grows downward, so the sign of the cross product is the mirror of
        // the graph-paper convention. See [`Shape::Beyond`].
        Shape::Beyond { from, to } => {
            side_of_line(at, chart_point(wm, from), chart_point(wm, to)) < 0.0
        }
    }
}

/// The hex edge pointing across a stroke at this tile — the edge a river or a
/// cliff along that stroke would occupy. Rivers and cliffs are edges rather
/// than tiles, so a linear feature has to choose which of the six to mark;
/// it takes the one whose neighbour lies furthest across the line.
fn crossing_edge(wm: &WorldMap, shape: &Shape, pos: Pos) -> Option<usize> {
    let (from, to) = match *shape {
        Shape::Band { from, to, .. } => (from, to),
        Shape::Beyond { from, to } => (from, to),
        _ => return None,
    };
    let (fx, fy) = chart_point(wm, from);
    let (tx, ty) = chart_point(wm, to);
    let (col, row) = hex::axial_to_offset(pos.0, pos.1);
    let here = side_of_line((col as f32, row as f32), (fx, fy), (tx, ty));
    // The neighbour that is furthest to the other side of the line, so the
    // marked edge is the one somebody crossing the feature would step over.
    (0..6)
        .filter(|dir| {
            let neighbour = hex::neighbors(pos)[*dir];
            let (ncol, nrow) = hex::axial_to_offset(neighbour.0, neighbour.1);
            let there = side_of_line((ncol as f32, nrow as f32), (fx, fy), (tx, ty));
            here * there < 0.0 || (here.abs() < f32::EPSILON && there.abs() > f32::EPSILON)
        })
        .min_by_key(|dir| *dir)
}

// ------------------------------------------------------------------ painting

/// Whether a terrain name is one of the engine's water terrains.
fn is_water_name(name: &str) -> bool {
    matches!(name, "coast" | "ocean" | "lake")
}

/// The dry tiles of a planned battlefield.
///
/// `mapgen` needs this before the world passes run, so it is computed from the
/// plan alone rather than from a painted map: a tile is land unless some
/// stroke paints water over it and no later stroke paints it back.
pub fn land_tiles(wm: &WorldMap, plan: &Plan) -> BTreeSet<Pos> {
    wm.tiles
        .keys()
        .copied()
        .filter(|pos| !is_water_name(&planned_terrain(wm, plan, *pos)))
        .collect()
}

/// The terrain a plan ends up putting on one tile, without painting anything.
fn planned_terrain(wm: &WorldMap, plan: &Plan, pos: Pos) -> String {
    let mut terrain = plan.base.to_string();
    for stroke in plan.strokes {
        if !covers(wm, &stroke.shape, pos) {
            continue;
        }
        for paint in stroke.paint {
            if let Paint::Terrain(name) = paint {
                terrain = (*name).to_string();
            }
        }
    }
    terrain
}

/// Lay a plan over the generic world passes.
pub fn paint(wm: &mut WorldMap, plan: &Plan) {
    let positions: Vec<Pos> = wm.tiles.keys().copied().collect();
    // Start every tile from the plan's base coat, so nothing rolled by the
    // world passes survives into a chart that claims to be a real place.
    for pos in &positions {
        let tile = wm.tiles.get_mut(pos).unwrap();
        tile.terrain = plan.base.into();
        tile.hills = plan.base_hills;
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
    for stroke in plan.strokes {
        for pos in &positions {
            if !covers(wm, &stroke.shape, *pos) {
                continue;
            }
            let edge = crossing_edge(wm, &stroke.shape, *pos);
            let tile = wm.tiles.get_mut(pos).unwrap();
            for paint in stroke.paint {
                match paint {
                    Paint::Terrain(name) => tile.terrain = (*name).into(),
                    Paint::Hills(raised) => tile.hills = *raised,
                    Paint::Feature(feature) => {
                        tile.feature = feature.map(|name| name.into());
                    }
                    Paint::Improvement(improvement) => {
                        tile.improvement = improvement.map(|name| name.into());
                    }
                    Paint::River => {
                        if let Some(dir) = edge {
                            tile.river_edges[dir] = true;
                        }
                    }
                    Paint::Cliff => {
                        if let Some(dir) = edge {
                            tile.cliff_edges[dir] = true;
                        }
                    }
                }
            }
        }
    }
    // Water cannot carry a land feature or an improvement, and mountains
    // carry neither: a plan that paints a wood and then floods it should get
    // water, not a floating forest.
    for pos in &positions {
        let tile = wm.tiles.get_mut(pos).unwrap();
        let water = is_water_name(&tile.terrain);
        if water || tile.terrain.as_str() == "mountain" {
            tile.hills = false;
            tile.improvement = None;
            if !water || tile.feature.as_deref() != Some("reef") {
                tile.feature = None;
            }
        }
    }
    // River and cliff edges are shared: an edge marked from one side has to be
    // marked from the other, or the two tiles disagree about the same seam.
    mirror_edges(wm);
}

/// Mirror every river and cliff edge onto the neighbour that shares it.
fn mirror_edges(wm: &mut WorldMap) {
    let marks: Vec<(Pos, usize, bool, bool)> = wm
        .tiles
        .values()
        .flat_map(|tile| {
            (0..6).map(move |dir| (tile.pos, dir, tile.river_edges[dir], tile.cliff_edges[dir]))
        })
        .filter(|(_, _, river, cliff)| *river || *cliff)
        .collect();
    for (pos, dir, river, cliff) in marks {
        let neighbour = hex::neighbors(pos)[dir];
        let opposite = (dir + 3) % 6;
        if let Some(tile) = wm.tiles.get_mut(&neighbour) {
            // A river or cliff cannot run along the edge of water on both
            // sides; the seam belongs to the shore, not to the open sea.
            if river && !is_water_name(&tile.terrain) {
                tile.river_edges[opposite] = true;
            }
            if cliff {
                tile.cliff_edges[opposite] = true;
            }
        }
        if river || cliff {
            if let Some(tile) = wm.tiles.get_mut(&pos) {
                if river && is_water_name(&tile.terrain) {
                    tile.river_edges[dir] = false;
                }
            }
        }
    }
}

// ---------------------------------------------------------------- deployment

/// The tiles one side forms up on, in the order its order of battle fills
/// them: along the historical front, from the wing the plan names first.
///
/// Only ground a unit of that force could stand on is offered. A naval force
/// gets water and a land force gets land, decided by the caller, because a
/// landing scenario has both on the same front.
pub fn front_tiles(wm: &WorldMap, plan: &Plan, side: usize, water: bool) -> Vec<Pos> {
    let front = plan.fronts[side.min(1)];
    let (fx, fy) = chart_point(wm, front.from);
    let (tx, ty) = chart_point(wm, front.to);
    let mut tiles: Vec<(u64, Pos)> = wm
        .tiles
        .values()
        .filter(|tile| is_water_name(&tile.terrain) == water && tile.terrain.as_str() != "mountain")
        .map(|tile| {
            let (col, row) = hex::axial_to_offset(tile.pos.0, tile.pos.1);
            let at = (col as f32, row as f32);
            // Sort along the front first and away from it second, so a force
            // fills its line before it stacks up behind it. Both are scaled to
            // integers so the order is exactly reproducible.
            let along = {
                let (dx, dy) = (tx - fx, ty - fy);
                let span = dx * dx + dy * dy;
                if span <= f32::EPSILON {
                    0.0
                } else {
                    (((at.0 - fx) * dx + (at.1 - fy) * dy) / span).clamp(0.0, 1.0)
                }
            };
            let off = distance_to_segment(at, (fx, fy), (tx, ty));
            (
                (off * 64.0) as u64 * 4096 + (along * 512.0) as u64,
                tile.pos,
            )
        })
        .collect();
    tiles.sort_by_key(|(rank, pos)| (*rank, *pos));
    tiles.into_iter().map(|(_, pos)| pos).collect()
}

/// The anchor a scenario's chart reports as each side's start: the head of its
/// own front line.
///
/// `afloat` says, per side, whether that force stands on water — which is a
/// fact about the force rather than about the battle. Actium is a fleet action
/// fought in the mouth of a gulf, so its lobby lens reads `land_water` for the
/// headlands while both orders of battle are squadrons; anchoring off the lens
/// put two fleets on a beach.
pub fn major_starts(wm: &WorldMap, plan: &Plan, afloat: [bool; 2]) -> Option<Vec<Pos>> {
    let first = *front_tiles(wm, plan, 0, afloat[0]).first()?;
    let second = front_tiles(wm, plan, 1, afloat[1])
        .into_iter()
        .find(|pos| *pos != first)?;
    Some(vec![first, second])
}

/// Whether each side of a battle forms up afloat, read from the elements its
/// own order of battle is written in: a force whose leading piece is a ship
/// anchors on water.
pub fn sides_afloat(
    rules: &crate::rules::Rules,
    scenario: &crate::historical_scenarios::HistoricalScenario,
) -> [bool; 2] {
    let afloat = |side: usize| {
        scenario.forces[side]
            .units
            .first()
            .and_then(|kind| rules.units.get(kind))
            .is_some_and(|spec| spec.domain.as_deref() == Some("sea"))
    };
    [afloat(0), afloat(1)]
}

/// The plan for a catalogue id, if this battle's ground has been drawn.
pub fn by_id(id: &str) -> Option<&'static Plan> {
    PLANS.iter().find(|plan| plan.id == id)
}

include!("historical_terrain_plans.rs");

#[cfg(test)]
mod tests;
