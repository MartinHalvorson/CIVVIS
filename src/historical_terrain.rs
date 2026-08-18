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
mod tests {
    use super::*;
    use crate::setup::{MapPoles, MapTopology};

    /// ★★★★★ EVERY CATALOGUED BATTLE IS DRAWN, OR SAYS WHY NOT.
    ///
    /// `historical_terrain_plans.rs` is 2,223 lines and had no test of any kind,
    /// and the join it depends on — one `Plan` per catalogue row — was enforced
    /// by nothing. A battle added to the catalogue without a chart does not fail
    /// anything: it quietly renders on whatever the generic script produces, and
    /// a field nobody drew is exactly the defect this module exists to prevent.
    ///
    /// Trafalgar is the one deliberate exception and is named here rather than
    /// skipped by a rule, so adding a second bespoke module is a decision
    /// somebody writes down.
    const BESPOKE_FIELDS: &[&str] = &["trafalgar"];

    #[test]
    fn every_catalogued_battle_has_a_field_drawn_for_it() {
        let drawn: std::collections::BTreeSet<&str> = PLANS.iter().map(|plan| plan.id).collect();
        let undrawn: Vec<&str> = crate::historical_scenarios::all()
            .iter()
            .map(|scenario| scenario.id)
            .filter(|id| !drawn.contains(id) && !BESPOKE_FIELDS.contains(id))
            .collect();
        assert!(
            undrawn.is_empty(),
            "these battles are in the catalogue with no chart and would render on              generic ground: {undrawn:?}"
        );
    }

    #[test]
    fn every_plan_draws_a_battle_the_catalogue_actually_lists() {
        let listed: std::collections::BTreeSet<&str> = crate::historical_scenarios::all()
            .iter()
            .map(|scenario| scenario.id)
            .collect();
        let orphans: Vec<&str> = PLANS
            .iter()
            .map(|plan| plan.id)
            .filter(|id| !listed.contains(id))
            .collect();
        assert!(
            orphans.is_empty(),
            "charts for battles nobody can play: {orphans:?}"
        );
    }

    #[test]
    fn a_bespoke_field_is_not_also_a_plan() {
        // Two sources for one battle's ground is worse than none: the painter
        // would run and the module would overwrite it, or the reverse, and which
        // won would depend on call order.
        for id in BESPOKE_FIELDS {
            assert!(
                !PLANS.iter().any(|plan| plan.id == *id),
                "{id} has both a bespoke module and a chart"
            );
            assert!(
                crate::historical_scenarios::by_id(id).is_some(),
                "{id} is exempted from needing a chart but is not in the catalogue"
            );
        }
    }

    #[test]
    fn no_plan_names_terrain_or_a_feature_the_rules_do_not_ship() {
        // A misspelled paint is a stroke that does nothing, on a field whose
        // whole purpose is to be the ground the battle was fought on.
        let rules = crate::rules::Rules::embedded();
        for plan in PLANS {
            assert!(
                rules.terrains.contains_key(plan.base),
                "{}: base terrain {:?} is not in the ruleset",
                plan.id,
                plan.base
            );
        }
    }

    fn chart(id: &str) -> WorldMap {
        let scenario = crate::historical_scenarios::by_id(id).expect("catalogue row");
        let rules = crate::rules::Rules::embedded();
        let mut rng = crate::rng::Rng::new(7);
        let (map, _) = crate::mapgen::generate_with_script(
            &rules,
            scenario.width,
            scenario.height,
            2,
            0,
            0,
            1,
            crate::historical_scenarios::script_from_id(id).expect("map script"),
            MapTopology::Flat,
            MapPoles::Poles,
            &mut rng,
        );
        map
    }

    fn tiles_where(map: &WorldMap, test: impl Fn(&crate::world::Tile) -> bool) -> usize {
        map.tiles.values().filter(|tile| test(tile)).count()
    }

    /// Every battle in the catalogue has ground drawn for it. The generic
    /// noise painter this replaced is gone, so a battle without a plan would
    /// open on an unpainted world rather than on a quietly wrong one.
    #[test]
    fn every_catalogue_battle_has_a_plan() {
        for scenario in crate::historical_scenarios::generic_scenarios() {
            assert!(
                by_id(scenario.id).is_some(),
                "{} has no terrain plan",
                scenario.id
            );
        }
        // And no plan describes a battle that is not in the catalogue.
        for plan in PLANS {
            assert!(
                crate::historical_scenarios::by_id(plan.id).is_some(),
                "{} is a plan for no catalogue battle",
                plan.id
            );
        }
    }

    /// Every name a plan paints has to be in the ruleset, or it paints a tile
    /// the engine cannot read.
    #[test]
    fn every_plan_paints_only_ruleset_names() {
        let rules = crate::rules::Rules::embedded();
        for plan in PLANS {
            assert!(
                rules.terrains.contains_key(plan.base),
                "{} bases on unknown terrain {}",
                plan.id,
                plan.base
            );
            for stroke in plan.strokes {
                for paint in stroke.paint {
                    match paint {
                        Paint::Terrain(name) => assert!(
                            rules.terrains.contains_key(name),
                            "{} paints unknown terrain {name}",
                            plan.id
                        ),
                        Paint::Feature(Some(name)) => assert!(
                            rules.features.contains_key(name),
                            "{} paints unknown feature {name}",
                            plan.id
                        ),
                        Paint::Improvement(Some(name)) => assert!(
                            rules.improvements.contains_key(name),
                            "{} paints unknown improvement {name}",
                            plan.id
                        ),
                        _ => {}
                    }
                }
            }
        }
    }

    /// Both sides must have somewhere to stand, and the two must not be the
    /// same tile. This is the reachability floor the old generic chart had.
    #[test]
    fn every_plan_seats_two_separated_sides() {
        for scenario in crate::historical_scenarios::generic_scenarios() {
            let map = chart(scenario.id);
            let plan = by_id(scenario.id).unwrap();
            let rules = crate::rules::Rules::embedded();
            let afloat = sides_afloat(&rules, scenario);
            let starts = major_starts(&map, plan, afloat)
                .unwrap_or_else(|| panic!("{} seats nobody", scenario.id));
            assert_eq!(starts.len(), 2, "{}", scenario.id);
            assert_ne!(starts[0], starts[1], "{}", scenario.id);
            // Far enough apart to be two armies rather than one melee.
            let (ax, ay) = hex::axial_to_offset(starts[0].0, starts[0].1);
            let (bx, by) = hex::axial_to_offset(starts[1].0, starts[1].1);
            let apart = (ax - bx).abs().max((ay - by).abs());
            assert!(apart >= 4, "{} seats both sides {apart} apart", scenario.id);
        }
    }

    /// A plan has to leave a battle passable: an army that cannot reach the
    /// other one is a chart, not a scenario. Checked as a flood fill from one
    /// side's anchor to the other's over ground that force could cross.
    #[test]
    fn every_plan_leaves_a_route_between_the_armies() {
        for scenario in crate::historical_scenarios::generic_scenarios() {
            let map = chart(scenario.id);
            let plan = by_id(scenario.id).unwrap();
            let rules = crate::rules::Rules::embedded();
            let afloat = sides_afloat(&rules, scenario);
            let naval = afloat[0] && afloat[1];
            let starts = major_starts(&map, plan, afloat).unwrap();
            let passable = |pos: Pos| {
                map.get(pos).is_some_and(|tile| {
                    let water = is_water_name(&tile.terrain);
                    tile.terrain.as_str() != "mountain" && (water == naval || !naval)
                })
            };
            let mut seen = BTreeSet::new();
            let mut queue = vec![starts[0]];
            seen.insert(starts[0]);
            while let Some(pos) = queue.pop() {
                for dir in 0..6 {
                    let next = hex::neighbors(pos)[dir];
                    if map.get(next).is_some() && passable(next) && seen.insert(next) {
                        queue.push(next);
                    }
                }
            }
            assert!(
                seen.contains(&starts[1]),
                "{} walls the two armies apart",
                scenario.id
            );
        }
    }

    /// An army that stood in a line opens in a line. Every piece of a force
    /// has to come down on that side's own front rather than in a clump around
    /// one tile — and, at Thermopylae in particular, the Greeks have to be in
    /// the gate and the Persians outside it, because a rearguard that opens
    /// west of the wall is not holding anything.
    #[test]
    fn each_force_forms_up_along_its_own_front() {
        for scenario in crate::historical_scenarios::generic_scenarios() {
            let mut options = crate::game::GameOptions::new(
                2,
                scenario.width,
                scenario.height,
                2026,
                scenario.turns,
                0,
            );
            options.map_script = crate::historical_scenarios::script_from_id(scenario.id).unwrap();
            options.start_era = scenario.era_index;
            options.barbarians = false;
            let game = crate::game::Game::new_with(options);
            let plan = by_id(scenario.id).unwrap();
            let rules = crate::rules::Rules::embedded();
            for side in 0..2 {
                // Aircraft are based rather than drawn up: a squadron belongs
                // to whatever can carry it, not to a place in the line, so it
                // is not held to the front the ground and the ships form on.
                let placed: Vec<Pos> = game
                    .player_unit_ids(side)
                    .iter()
                    .filter_map(|uid| game.units.get(uid))
                    .filter(|unit| {
                        rules
                            .units
                            .get(unit.kind.as_str())
                            .is_none_or(|spec| spec.domain.as_deref() != Some("air"))
                    })
                    .map(|unit| unit.pos)
                    .collect();
                assert!(
                    !placed.is_empty(),
                    "{} side {side} deployed nothing",
                    scenario.id
                );
                // Every piece within reach of the segment its side formed on.
                // Four hexes is loose on purpose: a line spills backward when
                // its own front is short, and some of these fronts are two
                // tiles wide by design.
                let front = plan.fronts[side];
                let (fx, fy) = chart_point(&game.map, front.from);
                let (tx, ty) = chart_point(&game.map, front.to);
                for pos in &placed {
                    let (col, row) = hex::axial_to_offset(pos.0, pos.1);
                    let off = distance_to_segment((col as f32, row as f32), (fx, fy), (tx, ty));
                    assert!(
                        off <= 4.5,
                        "{} side {side} put a unit {off:.1} from its front at {pos:?}",
                        scenario.id
                    );
                }
            }
        }
    }

    /// The Greeks hold the gate and the Persians are outside it: the one
    /// arrangement that makes Thermopylae the battle it was, checked on the
    /// board rather than in the briefing.
    #[test]
    fn leonidas_opens_inside_the_gate_and_xerxes_outside_it() {
        let scenario = crate::historical_scenarios::by_id("thermopylae").unwrap();
        let mut options = crate::game::GameOptions::new(
            2,
            scenario.width,
            scenario.height,
            2026,
            scenario.turns,
            0,
        );
        options.map_script = crate::historical_scenarios::script_from_id("thermopylae").unwrap();
        options.start_era = scenario.era_index;
        options.barbarians = false;
        let game = crate::game::Game::new_with(options);
        let column = |side: usize| {
            game.player_unit_ids(side)
                .iter()
                .filter_map(|uid| game.units.get(uid))
                .map(|unit| hex::axial_to_offset(unit.pos.0, unit.pos.1).0)
                .collect::<Vec<_>>()
        };
        let greeks = column(0);
        let persians = column(1);
        let gate = game.map.width / 2;
        assert!(
            greeks.iter().all(|col| *col >= gate),
            "the Hellenic rearguard must stand at or east of the Middle Gate, got {greeks:?}"
        );
        assert!(
            persians.iter().all(|col| *col < gate),
            "the Persian host must still be west of the gate, got {persians:?}"
        );
    }

    /// Thermopylae is the battle whose whole military content is the width of
    /// the ground, and it is the one the old noise painter flattered worst.
    /// The claims: the Malian Gulf is north, Kallidromos is south, and the
    /// road between them pinches to a gate in the middle that is far narrower
    /// than the western approach the Persians march in along.
    #[test]
    fn thermopylae_is_a_pass_and_not_a_field() {
        let map = chart("thermopylae");
        let width = map.width;
        let height = map.height;
        let open_in_column = |col: i32| {
            (0..height)
                .filter(|row| {
                    let (q, r) = hex::offset_to_axial(col, *row);
                    map.get((q, r)).is_some_and(|tile| {
                        !is_water_name(&tile.terrain) && tile.terrain.as_str() != "mountain"
                    })
                })
                .count()
        };
        let gate = (width / 2 - 1..=width / 2 + 1)
            .map(open_in_column)
            .min()
            .unwrap();
        let approach = open_in_column(2);
        assert!(gate >= 1, "the pass is blocked outright");
        assert!(
            gate <= 3,
            "the middle gate is {gate} tiles wide and is supposed to be a gate"
        );
        assert!(
            approach >= gate * 2,
            "the western approach ({approach}) should be far wider than the gate ({gate})"
        );
        // Sea to the north, mountain to the south: the two walls of the pass.
        let north = tiles_where(&map, |tile| {
            hex::axial_to_offset(tile.pos.0, tile.pos.1).1 == 0 && is_water_name(&tile.terrain)
        });
        let south = tiles_where(&map, |tile| {
            hex::axial_to_offset(tile.pos.0, tile.pos.1).1 == height - 1
                && tile.terrain.as_str() == "mountain"
        });
        assert!(
            north >= (width / 2) as usize,
            "the Malian Gulf should hold the north edge"
        );
        assert!(
            south >= (width / 2) as usize,
            "Kallidromos should hold the south edge"
        );
    }

    /// Marathon's plain is bounded by the sea it was fought beside and the
    /// Great Marsh that closed its northern end — the two features that made
    /// the Athenian charge a race across open ground rather than a maneuver.
    #[test]
    fn marathon_has_its_bay_and_its_great_marsh() {
        let map = chart("marathon");
        assert!(
            tiles_where(&map, |tile| is_water_name(&tile.terrain)) >= 12,
            "the bay of Marathon is missing"
        );
        assert!(
            tiles_where(&map, |tile| tile.feature.as_deref() == Some("marsh")) >= 6,
            "the Great Marsh is missing"
        );
    }

    /// Gaugamela's ground is the claim: Darius had it levelled for his scythed
    /// chariots, so an open plain is the historical fact and any wood or hill
    /// in the middle of it is an error.
    #[test]
    fn gaugamela_is_the_ground_darius_cleared() {
        let map = chart("gaugamela");
        let cleared = map.tiles.values().filter(|tile| {
            let (col, row) = hex::axial_to_offset(tile.pos.0, tile.pos.1);
            (col >= 4 && col < map.width - 4) && (row >= 3 && row < map.height - 3)
        });
        for tile in cleared {
            assert!(
                !tile.hills && tile.feature.is_none() && tile.terrain.as_str() != "mountain",
                "the cleared chariot ground carries {:?}/{:?} at {:?}",
                tile.terrain,
                tile.feature,
                tile.pos
            );
        }
    }

    /// Agincourt was fought in a corridor between two woods, on ground the
    /// rain had turned to mud. Both are the battle: the woods squeezed the
    /// French frontage and the mud drowned their advance. The corridor is
    /// drawn running north–south, so its two woods are east and west, and it
    /// has to be narrower at the English end than at the French one.
    #[test]
    fn agincourt_is_a_corridor_between_two_woods() {
        let map = chart("agincourt");
        let woods = |west: bool| {
            map.tiles.values().filter(move |tile| {
                let (col, _) = hex::axial_to_offset(tile.pos.0, tile.pos.1);
                tile.feature.as_deref() == Some("forest")
                    && if west {
                        col < map.width / 2
                    } else {
                        col >= map.width / 2
                    }
            })
        };
        assert!(woods(true).count() >= 6, "the wood on one side is missing");
        assert!(
            woods(false).count() >= 6,
            "the wood on the other side is missing"
        );
        assert!(
            tiles_where(&map, |tile| tile.feature.as_deref() == Some("marsh")) >= 8,
            "the ploughed mud that drowned the French advance is missing"
        );
        // The squeeze: the open frontage narrows toward the English end.
        let open_in_row = |row: i32| {
            (0..map.width)
                .filter(|col| {
                    let (q, r) = hex::offset_to_axial(*col, row);
                    map.get((q, r))
                        .is_some_and(|tile| tile.feature.as_deref() != Some("forest"))
                })
                .count()
        };
        let french_end = open_in_row(1);
        let english_end = open_in_row(map.height - 2);
        assert!(
            english_end < french_end,
            "the corridor should pinch toward the English ({english_end}) from the \
             French end ({french_end})"
        );
    }

    /// Cannae's river is why the envelopment worked: the Aufidus pinned the
    /// Roman flank and shortened the frontage they could deploy on. It is
    /// drawn as a channel rather than as edge segments, because a line has to
    /// be able to *rest* on it — so what is checked is water across the top of
    /// the field, and open ground below for the wings to swing through.
    #[test]
    fn cannae_is_pinned_against_the_aufidus() {
        let map = chart("cannae");
        let northern = |tile: &crate::world::Tile| {
            hex::axial_to_offset(tile.pos.0, tile.pos.1).1 < map.height / 3
        };
        let channel = map
            .tiles
            .values()
            .filter(|tile| northern(tile) && is_water_name(&tile.terrain))
            .count();
        assert!(
            channel >= 8,
            "the Aufidus should close the northern flank; found {channel} water tiles there"
        );
        let southern_open = map.tiles.values().filter(|tile| {
            let (_, row) = hex::axial_to_offset(tile.pos.0, tile.pos.1);
            row >= map.height / 2
                && !is_water_name(&tile.terrain)
                && tile.terrain.as_str() != "mountain"
                && tile.feature.is_none()
        });
        assert!(
            southern_open.count() >= map.tiles.len() / 4,
            "the open ground the wings closed through is missing"
        );
    }

    /// A naval battle has to be mostly sea, and a landing has to have both a
    /// sea to come from and a shore to land on.
    #[test]
    fn naval_and_landing_charts_hold_the_water_they_claim() {
        for scenario in crate::historical_scenarios::generic_scenarios() {
            let map = chart(scenario.id);
            let water = tiles_where(&map, |tile| is_water_name(&tile.terrain));
            let land = map.tiles.len() - water;
            match scenario.terrain {
                "water" | "water_air" => assert!(
                    water * 4 >= map.tiles.len() * 3,
                    "{} is a sea fight on {water}/{} water",
                    scenario.id,
                    map.tiles.len()
                ),
                "land_water" | "land_water_air" => {
                    assert!(water >= 8, "{} claims water and has {water}", scenario.id);
                    assert!(land >= 8, "{} claims land and has {land}", scenario.id);
                }
                // A land battle is one fought on its feet, not one with no
                // water in sight: Thermopylae's northern wall is the Malian
                // Gulf and the pass is unreadable without it. What must hold
                // is that most of the chart is ground.
                _ => assert!(
                    land * 2 >= map.tiles.len(),
                    "{} is a land battle with only {land} of {} tiles dry",
                    scenario.id,
                    map.tiles.len()
                ),
            }
        }
    }
}
