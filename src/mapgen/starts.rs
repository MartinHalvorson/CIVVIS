//! Start placement: dividing the land among the seats and choosing where
//! each one begins.
//!
//! How good a start is and how it scores against a civilization's biases
//! (`start_quality`, `start_bias_score`, `assign_starts_by_bias`), how a
//! whole layout scores (`spawn_layout_score`); the separation rules
//! (`START_DISTANCE_*`, `MIN_START_SEPARATION` and the buffers); dividing
//! the land into regions and apportioning them to seats
//! (`divide_into_regions`, `apportion`, `regions_for_seats`,
//! `archipelago_minor_regions`); placing starts in them
//! (`regional_starts`, `fill_remaining_starts`); and balancing territory
//! and start quality afterwards. `place_starts_and_finish` runs all of it,
//! as the last stage of map generation.
//!
//! Moved verbatim out of `mapgen.rs` (2026-11-05). `mapgen.rs` re-exports
//! everything here, so every path is unchanged; items the rest of `mapgen`
//! uses are `pub(super)`.

use super::*;

/// How even a finished layout came out. Nothing in the generator consults it —
/// region division is what produces the spread — but it is the vocabulary the
/// tests hold that spread to, so it lives beside the placer it grades.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct SpawnLayoutScore {
    /// No two civilizations should begin unavoidably crowded.
    pub(super) minimum_separation: i32,
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
pub(super) fn start_quality(rules: &Rules, wm: &WorldMap, pos: Pos) -> i32 {
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
pub(super) fn spawn_layout_score(
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
pub(super) fn layout_balance_percentages(
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

/// Shipped `START_DISTANCE_MINOR_NATURAL_WONDER`. `__NaturalWonderBuffer`
/// rejects a site with a Natural Wonder inside this range, the distance itself
/// included, so the nearest legal site stands one tile farther out.
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

pub(super) fn tile_fertility(rules: &Rules, tile: &crate::world::Tile) -> i32 {
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
pub(super) enum MapDistanceRow<'a> {
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
    pub(super) fn new(wm: &'a WorldMap, from: Pos) -> Self {
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

    pub(super) fn distance(&self, to: Pos) -> i32 {
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
pub(super) fn divide_into_regions(
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
pub(super) const MIN_LANDMASS_FOR_A_START: usize = 12;

/// Hand `seats` out over `weights` by largest remainder — the apportionment
/// rule that gives each landmass its fair share of the world's seats and lets
/// rounding fall where the shortfall is largest, rather than where the list
/// happens to start — with each landmass capped at what it has room for, and
/// the overflow re-apportioned among those with slack.
pub(super) fn apportion_capped(weights: &[i64], caps: &[usize], seats: usize) -> Vec<usize> {
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

pub(super) fn apportion(weights: &[i64], seats: usize) -> Vec<usize> {
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
pub(super) fn regions_for_seats(
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
pub(super) fn archipelago_minor_regions(
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

pub(super) struct StartBuffers<'a> {
    pub(super) foreign: &'a [Pos],
    pub(super) foreign_buffer: i32,
    pub(super) own_buffer: i32,
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
pub(super) fn regional_starts(
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
pub(super) fn fill_remaining_starts(
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
pub(super) fn balance_territory(
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
pub(super) fn equalize_start_quality(
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

/// How many seats a map has, and any true start points they must take.
pub(super) struct SeatRequest<'a> {
    pub(super) num_major_spawns: usize,
    pub(super) num_minor_spawns: usize,
    pub(super) leader_starts: &'a [TrueStartPoint],
    pub(super) minor_starts: &'a [Option<TrueStartPoint>],
}

/// The end of map generation, once the terrain is final: choose every start,
/// top up the strategic resources and artifacts around them, and retire any
/// feature a later pass left invalid.
pub(super) fn place_starts_and_finish(
    mut wm: WorldMap,
    land: BTreeSet<Pos>,
    rules: &Rules,
    script: MapScript,
    seats: SeatRequest<'_>,
    rng: &mut Rng,
) -> (WorldMap, Vec<Pos>) {
    let SeatRequest {
        num_major_spawns,
        num_minor_spawns,
        leader_starts,
        minor_starts,
    } = seats;
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
                    .all(|wonder| wm.distance(*position, *wonder) > floor)
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
