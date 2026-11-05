//! Rivers: tracing them down the land, and the edges they run along.
//!
//! River edges (`canonical_river_edge`, `connected_river_edges`), the value
//! each plot offers a river, tracing one (`trace_river`), whether an edge
//! reaches an outlet, `generate_rivers`, and removing rivers that run along
//! a water boundary.
//!
//! Moved verbatim out of `mapgen.rs` (2026-11-05). `mapgen.rs` re-exports
//! everything here, so every path is unchanged; items the rest of `mapgen`
//! uses are `pub(super)`.

use super::*;

pub(super) fn canonical_river_edge(a: Pos, b: Pos) -> RiverEdge {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

/// The other shared edges touching either endpoint of a hex edge. For two
/// adjacent hexes A/B, each endpoint also touches one common neighbor C; the
/// four possible continuations are A/C and B/C at those two vertices.
///
/// Generation itself walks corners rather than edges (see [`corner_steps`]);
/// this is how the assertions read a finished river network back.
#[cfg(test)]
pub(super) fn connected_river_edges(wm: &WorldMap, edge: RiverEdge) -> Vec<RiverEdge> {
    let (a, b) = edge;
    let b_neighbors: BTreeSet<Pos> = wm.neighbors(b).into_iter().collect();
    let mut connected = BTreeSet::new();
    for common in wm
        .neighbors(a)
        .into_iter()
        .filter(|p| *p != b && wm.tiles.contains_key(p) && b_neighbors.contains(p))
    {
        connected.insert(canonical_river_edge(a, common));
        connected.insert(canonical_river_edge(b, common));
    }
    connected.remove(&edge);
    connected.into_iter().collect()
}

/// Civ VI's `GetRiverValueAtPlot`: a tile's own elevation weighted twenty to
/// one against its six surroundings, deserts drawing the flow toward them and
/// the map edge pushing it away. A river runs to the lowest value it can see.
///
/// Two departures from the Lua, both forced by what our terrain does and does
/// not record:
///
/// * **The continental slope is added back.** Civ VI reads its rivers off a
///   fractal height field and keeps only four rungs of it in the terrain; we
///   generate the four rungs directly, so between two flat plains tiles there
///   is no telling which way is downhill. A greedy flow on ground with no
///   gradient is a random walk that only stops when it blunders into the sea —
///   which is exactly what it did, wandering 300 tiles across a continent that
///   is 40 wide. Depth inland stands in for the height the terrain forgot.
/// * **The jitter is rolled once per tile** rather than once per lookup, so the
///   field a river descends is a fixed landscape rather than noise that
///   re-rolls underneath it. Terrain does not move while a river crosses it.
fn river_values(wm: &WorldMap, rng: &mut Rng, inland: &impl Fn(Pos) -> i32) -> Vec<i32> {
    let mut values = vec![0; wm.tiles.len()];
    for (index, pos) in wm.tiles.keys().copied().enumerate() {
        let mut sum = plot_elevation(wm, pos) * 20 + inland(pos) * RIVER_INLAND_SLOPE;
        for direction in wm.around(pos) {
            match wm.tiles.get(&direction) {
                None => sum += 40,
                Some(tile) => {
                    sum += plot_elevation(wm, direction);
                    if tile.terrain == "desert" {
                        sum += 4;
                    }
                }
            }
        }
        sum += rng.randint(0, 9);
        values[index] = sum;
    }
    values
}

/// Walk one river down from its headwater, Civ VI's `DoRiver`.
///
/// Each step crosses to the corner whose newly revealed tile lies lowest, at a
/// twelfth's discount for holding a straight course. The walk ends at water —
/// the sea it drains into, or the river it joins — and a walk that ends
/// anywhere else is thrown away rather than drawn, because a channel that
/// peters out in open country is not a river. That single rule is what keeps
/// the map free of the dangling stubs an upstream trace leaves behind.
fn trace_river(
    wm: &WorldMap,
    start: RiverCorner,
    values: &[i32],
    joined: &BTreeSet<RiverCorner>,
    is_water: &impl Fn(Pos) -> bool,
    limit: usize,
) -> Option<(Vec<RiverEdge>, RiverEnding)> {
    let mut course: Vec<RiverEdge> = Vec::new();
    let mut visited: BTreeSet<RiverCorner> = BTreeSet::new();
    let mut corner = start;
    let mut previous: Option<RiverEdge> = None;
    let mut last_pivot: Option<Pos> = None;
    visited.insert(corner);
    while course.len() < limit {
        let mut best: Option<(i32, RiverEdge, Pos, RiverCorner, Option<Pos>)> = None;
        for (edge, revealed, next) in corner_steps(wm, corner) {
            // A segment is the boundary between two land tiles: a channel that
            // ran along a shoreline would be a river with the sea for a bank.
            if is_water(edge.0) || is_water(edge.1) || visited.contains(&next) {
                continue;
            }
            let pivot = previous.and_then(|before| shared_tile(before, edge));
            let mut value = wm
                .tiles
                .index_of(revealed)
                .and_then(|index| values.get(index))
                .copied()
                .unwrap_or(i32::MAX);
            if let (Some(pivot), Some(last)) = (pivot, last_pivot) {
                if pivot != last {
                    value = value * 11 / 12;
                }
            }
            if best.as_ref().is_none_or(|(best, ..)| value < *best) {
                best = Some((value, edge, revealed, next, pivot));
            }
        }
        let (_, edge, revealed, next, pivot) = best?;
        course.push(edge);
        // The far corner of this last segment is where the channel meets the
        // water: a river mouth, or a confluence with the river it joins.
        if is_water(revealed) {
            return Some((course, RiverEnding::Sea));
        }
        if joined.contains(&next) {
            return Some((course, RiverEnding::Confluence));
        }
        visited.insert(next);
        previous = Some(edge);
        last_pivot = pivot;
        corner = next;
    }
    None
}

#[cfg(test)]
pub(super) fn river_edge_has_outlet(
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

pub(super) fn generate_rivers(wm: &mut WorldMap, land: &[Pos], rng: &mut Rng) {
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
    let mut water_distance = vec![i32::MAX; wm.tiles.len()];
    let mut water_frontier: VecDeque<Pos> = water_tiles.iter().copied().collect();
    for position in &water_tiles {
        water_distance[wm.tiles.index_of(*position).unwrap()] = 0;
    }
    while let Some(position) = water_frontier.pop_front() {
        let position_index = wm.tiles.index_of(position).unwrap();
        let next_distance = water_distance[position_index] + 1;
        for neighbor in wm.neighbors(position) {
            let neighbor_index = wm.tiles.index_of(neighbor).unwrap();
            if water_distance[neighbor_index] == i32::MAX {
                water_distance[neighbor_index] = next_distance;
                water_frontier.push_back(neighbor);
            }
        }
    }
    let distance_to_sea = |pos: Pos| {
        wm.tiles
            .index_of(pos)
            .map(|index| water_distance[index])
            .unwrap_or(0)
    };

    // The landmass a headwater stands on sets that river's budget: Civ VI
    // rations river edges per continent, not per world, so an island does not
    // inherit a mainland's allowance.
    let land_set: BTreeSet<Pos> = land.iter().copied().collect();
    let landmasses = connected_components(wm, &land_set);
    let mut landmass_of = vec![usize::MAX; wm.tiles.len()];
    for (index, landmass) in landmasses.iter().enumerate() {
        for pos in landmass {
            landmass_of[wm.tiles.index_of(*pos).unwrap()] = index;
        }
    }
    let budget: Vec<usize> = landmasses
        .iter()
        .map(|landmass| landmass.len() / RIVER_PLOTS_PER_EDGE + 1)
        .collect();
    // The trunks do not get to spend the whole allowance: a fifth of it is
    // held back so the tributary pass has something left to run on. Without
    // the reserve the trunks eat the budget and every river on the map is an
    // only child.
    let trunk_budget: Vec<usize> = budget.iter().map(|edges| edges * 4 / 5).collect();
    let mut spent = vec![0usize; landmasses.len()];

    let mut values = river_values(wm, rng, &distance_to_sea);
    // A channel longer than a lap of the world is a runaway, not a river.
    let limit = (wm.width as usize + wm.height as usize) * 2;

    let mut rivers: BTreeSet<RiverEdge> = BTreeSet::new();
    let mut confluences: BTreeSet<RiverCorner> = BTreeSet::new();
    let mut fresh_water = vec![false; wm.tiles.len()];

    // Civ VI's four passes of `AddRivers`, in order. The first two seed the
    // highlands and a thin scatter of the deep interior; the last two go back
    // over any landmass still short of its allowance, with the spacing rules
    // relaxed by half so a river-poor continent can be topped up. A fifth pass
    // of our own then hangs tributaries off what those four laid.
    for pass in 0..5 {
        let tributary = pass == 4;
        let (source_range, sea_range) = if pass < 2 || tributary {
            (RIVER_SOURCE_RANGE, RIVER_SEA_WATER_RANGE)
        } else {
            (RIVER_SOURCE_RANGE / 2, RIVER_SEA_WATER_RANGE / 2)
        };
        // Civ VI walks the map in index order, which spends a landmass's whole
        // allowance on whichever corner of it the scan reaches first. Ours is
        // a real cap on every pass rather than a top-up on the last two, so the
        // order has to be drawn rather than read off the grid — otherwise every
        // river on the map is a northern one.
        let mut sources: Vec<Pos> = land.to_vec();
        for index in (1..sources.len()).rev() {
            sources.swap(index, rng.below(index + 1));
        }
        for source in sources {
            let Some(tile) = wm.tiles.get(&source) else {
                continue;
            };
            let highland = tile.terrain == "mountain" || tile.hills;
            let landmass = landmass_of[wm.tiles.index_of(source).unwrap()];
            let allowance = if tributary {
                budget[landmass]
            } else {
                trunk_budget[landmass]
            };
            // The allowance binds on every pass, not just the last two. Civ VI
            // lets its highland pass run unchecked and relies on the source
            // spacing to hold the count down; ours has denser highlands than
            // that assumption survives, and `RIVER_PLOTS_PER_EDGE` is the
            // density knob the game already declares — so it is used as one.
            let qualifies = spent[landmass] < allowance
                && match pass {
                    0 | 2 => highland,
                    1 => distance_to_sea(source) > 1 && rng.below(8) == 0,
                    _ => true,
                };
            if !qualifies {
                continue;
            }
            // Every headwater rises away from the sea it is going to reach.
            // Where it stands relative to the rivers already laid is what
            // decides which kind of river it becomes: a trunk rises clear of
            // all of them, a tributary deliberately within reach of one but
            // never on its bank, so that it has a country of its own to cross
            // before it arrives.
            if distance_to_sea(source) <= sea_range {
                continue;
            }
            let reach = if tributary {
                RIVER_TRIBUTARY_REACH
            } else {
                source_range
            };
            let within_reach = wm
                .disk(source, reach)
                .into_iter()
                .any(|near| fresh_water[wm.tiles.index_of(near).unwrap()]);
            let on_the_bank = tributary
                && wm
                    .disk(source, RIVER_TRIBUTARY_CLEARANCE)
                    .into_iter()
                    .any(|near| fresh_water[wm.tiles.index_of(near).unwrap()]);
            if within_reach != tributary || on_the_bank {
                continue;
            }
            let Some(start) = inland_corner(wm, source, &is_water) else {
                continue;
            };
            // A river may be joined, but it may not be branched off of: a new
            // channel starting on an existing one is the offshoot, not the
            // tributary.
            if confluences.contains(&start) {
                continue;
            }
            let Some((course, ending)) =
                trace_river(wm, start, &values, &confluences, &is_water, limit)
            else {
                continue;
            };
            // The tributary pass exists to produce confluences. A channel it
            // seeded that found its own way to the coast is just another trunk
            // crowding the one it started beside, so it is not laid at all.
            if course.len() < RIVER_MIN_COURSE || (tributary && ending != RiverEnding::Confluence) {
                continue;
            }
            for edge in &course {
                rivers.insert(*edge);
                fresh_water[wm.tiles.index_of(edge.0).unwrap()] = true;
                fresh_water[wm.tiles.index_of(edge.1).unwrap()] = true;
                confluences.extend(corners_of_edge(wm, *edge));
            }
            spent[landmass] += course.len();
            // Sink the valley this river just cut, so the next one starting
            // within reach of it runs down into it as a tributary instead of
            // laying a second trunk alongside.
            let mut basin = vec![0; wm.tiles.len()];
            let mut basin_tiles = Vec::new();
            for bank in course.iter().flat_map(|edge| [edge.0, edge.1]) {
                for near in wm.disk(bank, RIVER_VALLEY_REACH) {
                    let fall = RIVER_VALLEY_DEPTH
                        * (RIVER_VALLEY_REACH + 1 - wm.distance(bank, near))
                        / (RIVER_VALLEY_REACH + 1);
                    let index = wm.tiles.index_of(near).unwrap();
                    if fall > basin[index] {
                        if basin[index] == 0 {
                            basin_tiles.push(index);
                        }
                        basin[index] = fall;
                    }
                }
            }
            for index in basin_tiles {
                values[index] -= basin[index];
                basin[index] = 0;
            }
        }
    }

    for (a, b) in rivers {
        wm.set_river_edge(a, b, true);
    }
}

/// Keep every river mouth off the shoreline edge and on its inland reach.
///
/// Tracing downstream already ends a river at the corner where it meets the
/// water rather than along it, so this now removes nothing on a freshly
/// generated map. It stays as the backstop for the passes that run *after*
/// rivers: `add_lakes` refuses a plot that carries or touches a river, but a
/// river with the sea for a bank is wrong however it came about, and this is
/// the one place that has to be true no matter what floods later.
pub(super) fn remove_water_boundary_rivers(wm: &mut WorldMap) {
    let water = |position: Pos| {
        matches!(
            wm.tiles[&position].terrain.as_str(),
            "ocean" | "coast" | "lake"
        )
    };
    let shoreline: Vec<RiverEdge> = wm
        .tiles
        .keys()
        .copied()
        .flat_map(|position| {
            wm.neighbors(position)
                .into_iter()
                .filter(move |neighbor| position < *neighbor)
                .map(move |neighbor| (position, neighbor))
        })
        .filter(|edge| wm.has_river_edge(edge.0, edge.1))
        .filter(|edge| water(edge.0) || water(edge.1))
        .collect();
    for (a, b) in shoreline {
        wm.set_river_edge(a, b, false);
    }
}
