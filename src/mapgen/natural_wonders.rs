//! Natural Wonders: drawing the roster and planting each one in its shipped
//! footprint.
//!
//! `place_natural_wonders` is the stage of map generation: it surveys the map
//! (`survey_wonder_sites`), gives every eligible wonder a roll, and plants the
//! highest rolls, preferring sites away from wonders already placed
//! (`PREFERRED_WONDER_SEPARATION`, relaxing to `MIN_WONDER_SEPARATION`). The
//! helpers read which ground a wonder takes (`wonder_ground`, `wonder_anchor`,
//! `wonder_footprints`) and, on a true-start world, where it stands on Earth
//! (`EARTH_WONDERS`, `earth_wonder_site`).
//!
//! Moved verbatim out of `mapgen.rs` (2026-11-06). `mapgen.rs` imports
//! everything here, so every path is unchanged; items the rest of `mapgen`
//! uses are `pub(super)`.

use super::*;

/// Plant the map's Natural Wonders, and hand the map back.
pub(super) fn place_natural_wonders(
    mut wm: WorldMap,
    mut mountain_terrain: BTreeMap<Pos, Name>,
    rules: &Rules,
    num_natural_wonders: usize,
    script: MapScript,
    rng: &mut Rng,
) -> WorldMap {
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
    wm
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

pub(super) const MIN_WONDER_SEPARATION: i32 = 3;

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
pub(super) const EARTH_WONDERS: [(&str, f64, f64); 34] = [
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
pub(super) const EARTH_WONDER_REACH: i32 = 3;

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
pub(super) struct WonderSite {
    /// Bit per neighbouring base terrain, indexed by `WONDER_TERRAIN_BITS`.
    pub(super) terrains: u32,
    /// Whether any neighbour carries a feature at all (`NoAdjacentFeatures`).
    any_feature: bool,
    /// Hexes of open water between this hex and the nearest land, for the
    /// wonders that ship `MinDistanceLand` / `MaxDistanceLand`. Land is 0.
    pub(super) land_distance: i32,
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
pub(super) fn wonder_ground(
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
pub(super) fn wonder_footprints(
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
pub(super) fn coastal_triangle_is_valid(wm: &WorldMap, footprint: &[Pos]) -> bool {
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
