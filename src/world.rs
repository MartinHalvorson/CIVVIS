//! Tiles and the world map (mirrors civvis/world.py).
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};

use crate::name::Name;
use crate::sphere::trig;
use crate::{hex, Pos};

/// A district site that has been placed but has not finished construction.
/// Placement locks both the chosen district and its production cost.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DistrictFoundation {
    pub district: Name,
    pub cost: f64,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Tile {
    pub pos: Pos,
    pub terrain: Name,
    /// An external partial-map reconstruction may let pathfinding probe a tile
    /// whose terrain is still `unknown`. This is an explicit planning prior,
    /// separate from the terrain fact, and is never set by ordinary map
    /// generation.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub assumed_traversable: bool,
    /// The sea's half of the same prior: an `unknown` tile at the edge of
    /// water the seat has charted, which a ship may plan toward. Kept apart
    /// from `assumed_traversable` because that flag is what a LAND unit reads
    /// as passable ground — and `come_ashore` deliberately keeps the land army
    /// out of the water, which it cannot do for fog that has no domain yet.
    /// Set only by the Civilization VI mirror's frontier growth, never by map
    /// generation, so a native world's ships behave exactly as before.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub assumed_navigable: bool,
    pub feature: Option<Name>,
    pub hills: bool,
    pub resource: Option<Name>,
    pub improvement: Option<Name>,
    /// Improvements and ordinary districts stop producing yields while
    /// pillaged. City/Encampment defenses keep their dedicated damage state.
    #[serde(default)]
    pub pillaged: bool,
    pub district: Option<Name>,
    /// Placed districts occupy their tile and count against district limits,
    /// but do not grant completed-district yields or abilities.
    #[serde(default)]
    pub district_foundation: Option<DistrictFoundation>,
    #[serde(default)]
    pub wonder: Option<Name>,
    pub owner_city: Option<u32>,
    #[serde(default)]
    /// River segments on this hex's six edges, in `hex::DIRS` order.
    /// Shared edges are mirrored on both neighboring tiles.
    pub river_edges: [bool; 6],
    /// The host's own answer to "is this plot riverside" (`Plot:IsRiver`), kept
    /// apart from the edges: a segment whose Firaxis holder is an unrevealed
    /// neighbour is on none of this tile's `river_edges`, yet the plot IS
    /// riverside for housing, fresh water and district adjacency. Read by
    /// [`Tile::has_river`]; a crossing still needs the edge. Not serialised
    /// while false, so a generated world's save bytes — and the platform
    /// digests `mapgen` pins over them — are untouched by a flag only a
    /// mirrored board ever sets.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub riverside: bool,
    /// Coastal cliff segments on this hex's six shared edges. Like rivers,
    /// cliff edges are mirrored onto the neighboring tile so saves and
    /// observations remain self-contained.
    #[serde(default)]
    pub cliff_edges: [bool; 6],
    #[serde(default)]
    // Route level, the shipped PlacementValue ladder: 0 none, 1 Ancient,
    // 2 Medieval, 3 Industrial, 4 Modern, 5 Railroad.
    pub road: u8,
    /// Stock Civ VI continent region, zero-based. Water has no continent.
    #[serde(default)]
    pub continent: Option<usize>,
    /// Permanent Faith added by Great Bath flood mitigation.
    #[serde(default)]
    pub disaster_faith: f64,
    /// Permanent Food and Production a disaster left behind. Gathering Storm
    /// pays for the damage its storms do with fertility, so a tile that keeps
    /// being hit ends up better than it started.
    #[serde(default)]
    pub disaster_food: f64,
    #[serde(default)]
    pub disaster_production: f64,
    /// Whether this tile is currently suffering a drought's -1 Food effect.
    #[serde(default)]
    pub drought: bool,
    /// Gathering Storm coastal-lowland elevation band (1–3 meters). Zero
    /// means this tile is not vulnerable to sea-level rise.
    #[serde(default)]
    pub coastal_lowland: u8,
    /// A flooded lowland is unusable until its city completes a Flood Barrier.
    #[serde(default)]
    pub flooded: bool,
    /// Submerged lowlands are permanently converted to Coast.
    #[serde(default)]
    pub submerged: bool,
    /// Turn through which a nuclear accident's fallout makes the tile yieldless.
    #[serde(default)]
    pub fallout_until: u32,
    /// The storm class currently passing over this tile, if any. Storms drift
    /// for three turns, so the marker moves with the system rather than
    /// belonging to the tile.
    #[serde(default)]
    pub storm: Option<String>,
}

/// Last tile state actually observed by one player. `owner` is snapshotted
/// separately because a tile stores its owning city ID, while ownership of
/// that city can change outside the observer's current vision.
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct RememberedTile {
    pub tile: Tile,
    pub owner: Option<usize>,
    #[serde(default)]
    pub seen_turn: u32,
}

/// JSON cannot directly encode tuple-keyed maps. Keep fast position lookup at
/// runtime while serializing player map memory as a stable list of snapshots.
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "Vec<RememberedTile>", into = "Vec<RememberedTile>")]
pub struct TileMemory {
    tiles: std::sync::Arc<BTreeMap<Pos, RememberedTile>>,
    /// When each remembered tile was last actually looked at.
    ///
    /// This moves every turn while the tiles themselves almost never do, so
    /// it is kept out of the shared map. Restamping it in place would copy
    /// every remembered tile — a few hundred hexes, each with its own
    /// strings — which is most of what refreshing a seat's map used to cost.
    stamps: BTreeMap<Pos, u32>,
}

impl TileMemory {
    /// The turn a tile was last seen on, or zero for one never seen.
    pub fn seen_turn(&self, position: &Pos) -> u32 {
        self.stamps.get(position).copied().unwrap_or_default()
    }

    /// Note that a remembered tile was looked at again. Cheap: it does not
    /// touch the shared map.
    pub fn mark_seen(&mut self, position: Pos, turn: u32) {
        if let Some(last) = self.stamps.get_mut(&position) {
            if *last != turn {
                *last = turn;
            }
        }
    }

    /// Record what a tile looks like now.
    pub fn remember(&mut self, position: Pos, tile: RememberedTile, turn: u32) {
        std::sync::Arc::make_mut(&mut self.tiles).insert(position, tile);
        self.stamps.insert(position, turn);
    }

    pub fn forget_all(&mut self) {
        std::sync::Arc::make_mut(&mut self.tiles).clear();
        self.stamps.clear();
    }
}

impl Deref for TileMemory {
    type Target = BTreeMap<Pos, RememberedTile>;

    fn deref(&self) -> &Self::Target {
        &self.tiles
    }
}

/// Taking a mutable borrow is what copies the memory, so a player's
/// last-known map is shared until somebody writes to it.
///
/// A game is cloned to look ahead — that is what this engine exists for — and
/// a player's remembered map is the largest thing in it: a tile for every hex
/// they have ever seen, each with its own strings. Copying fifteen of those
/// per branch was about half the cost of cloning a game.
impl DerefMut for TileMemory {
    fn deref_mut(&mut self) -> &mut Self::Target {
        std::sync::Arc::make_mut(&mut self.tiles)
    }
}

impl From<Vec<RememberedTile>> for TileMemory {
    fn from(tiles: Vec<RememberedTile>) -> Self {
        let stamps = tiles
            .iter()
            .map(|remembered| (remembered.tile.pos, remembered.seen_turn))
            .collect();
        TileMemory {
            tiles: std::sync::Arc::new(
                tiles
                    .into_iter()
                    .map(|remembered| (remembered.tile.pos, remembered))
                    .collect(),
            ),
            stamps,
        }
    }
}

/// A save carries the stamp on each tile, which is where it used to live, so
/// the two are put back together on the way out.
impl From<TileMemory> for Vec<RememberedTile> {
    fn from(memory: TileMemory) -> Self {
        let stamps = memory.stamps;
        let restamp = |mut remembered: RememberedTile| {
            remembered.seen_turn = stamps
                .get(&remembered.tile.pos)
                .copied()
                .unwrap_or(remembered.seen_turn);
            remembered
        };
        match std::sync::Arc::try_unwrap(memory.tiles) {
            Ok(tiles) => tiles.into_values().map(restamp).collect(),
            Err(shared) => shared.values().cloned().map(restamp).collect(),
        }
    }
}

impl Tile {
    pub fn new(pos: Pos) -> Tile {
        Tile {
            pos,
            terrain: crate::name!("ocean"),
            assumed_traversable: false,
            assumed_navigable: false,
            feature: None,
            hills: false,
            resource: None,
            improvement: None,
            pillaged: false,
            district: None,
            district_foundation: None,
            wonder: None,
            owner_city: None,
            river_edges: [false; 6],
            riverside: false,
            cliff_edges: [false; 6],
            road: 0,
            continent: None,
            disaster_faith: 0.0,
            disaster_food: 0.0,
            disaster_production: 0.0,
            drought: false,
            coastal_lowland: 0,
            flooded: false,
            submerged: false,
            fallout_until: 0,
            storm: None,
        }
    }
}

/// Dense storage for the world's hexes.
///
/// Every position on the cylinder maps to exactly one offset column/row, so
/// the map is a rectangle with no holes and a tile lookup can be a pair of
/// array reads instead of a balanced-tree descent. Tile access sits under
/// essentially every rule in the engine, which made the old `BTreeMap<Pos,
/// Tile>` one of the hottest structures in a simulated turn.
///
/// `tiles` is kept sorted by `Pos` so iteration matches the ordering the map
/// has always had — saves, observations, and per-seed determinism all depend
/// on it — while `slot` indexes that vector by offset coordinates.
#[derive(Clone, Default)]
pub struct TileGrid {
    width: i32,
    height: i32,
    /// Bumped by every route that can write to a tile. Anything that caches a
    /// conclusion drawn from the map — what a unit can see, say — records the
    /// epoch it was drawn under and recomputes when the map has moved on.
    epoch: u64,
    /// Shared until written to. A game cloned to look ahead usually never
    /// touches the map at all — units move, tiles do not — so the hexes are
    /// copied only when something actually changes one.
    tiles: std::sync::Arc<Vec<Tile>>,
    /// `row * width + col` -> index into `tiles`, or `u32::MAX` when a save
    /// omitted that hex.
    slot: Vec<u32>,
}

const EMPTY_SLOT: u32 = u32::MAX;

impl TileGrid {
    pub fn new(width: i32, height: i32) -> TileGrid {
        let mut grid = TileGrid {
            width,
            height,
            epoch: 0,
            tiles: std::sync::Arc::new(Vec::new()),
            slot: Vec::new(),
        };
        let mut tiles = Vec::with_capacity((width.max(0) * height.max(0)) as usize);
        for row in 0..height {
            for col in 0..width {
                tiles.push(Tile::new(hex::offset_to_axial(col, row)));
            }
        }
        grid.rebuild(tiles);
        grid
    }

    fn from_tiles(width: i32, height: i32, tiles: Vec<Tile>) -> TileGrid {
        let mut grid = TileGrid {
            width,
            height,
            epoch: 0,
            tiles: std::sync::Arc::new(Vec::new()),
            slot: Vec::new(),
        };
        grid.rebuild(tiles);
        grid
    }

    fn rebuild(&mut self, mut tiles: Vec<Tile>) {
        self.epoch += 1;
        tiles.sort_unstable_by_key(|tile| tile.pos);
        tiles.dedup_by_key(|tile| tile.pos);
        let cells = (self.width.max(0) as usize) * (self.height.max(0) as usize);
        self.slot = vec![EMPTY_SLOT; cells];
        for (index, tile) in tiles.iter().enumerate() {
            if let Some(cell) = self.cell(tile.pos) {
                self.slot[cell] = index as u32;
            }
        }
        self.tiles = std::sync::Arc::new(tiles);
    }

    #[inline]
    fn cell(&self, pos: Pos) -> Option<usize> {
        let (col, row) = hex::axial_to_offset(pos.0, pos.1);
        if col < 0 || col >= self.width || row < 0 || row >= self.height {
            return None;
        }
        Some((row * self.width + col) as usize)
    }

    /// Where a position sits in the tile vector. Callers that keep their own
    /// per-tile table — a visibility sweep's height cache, say — index it by
    /// this, so the table is dense and in the same order as the map itself.
    #[inline]
    pub fn index_of(&self, pos: Pos) -> Option<usize> {
        let slot = *self.slot.get(self.cell(pos)?)?;
        if slot == EMPTY_SLOT {
            None
        } else {
            Some(slot as usize)
        }
    }

    #[inline]
    pub fn get(&self, pos: &Pos) -> Option<&Tile> {
        self.index_of(*pos).map(|index| &self.tiles[index])
    }

    #[inline]
    pub fn get_mut(&mut self, pos: &Pos) -> Option<&mut Tile> {
        self.epoch += 1;
        let index = self.index_of(*pos)?;
        Some(&mut std::sync::Arc::make_mut(&mut self.tiles)[index])
    }

    /// How many times the map has been opened for writing. Two reads of the
    /// same epoch saw the same map.
    #[inline]
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    #[inline]
    pub fn contains_key(&self, pos: &Pos) -> bool {
        self.index_of(*pos).is_some()
    }

    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &Pos> + ExactSizeIterator + '_ {
        self.tiles.iter().map(|tile| &tile.pos)
    }

    pub fn values(&self) -> std::slice::Iter<'_, Tile> {
        self.tiles.iter()
    }

    pub fn values_mut(&mut self) -> std::slice::IterMut<'_, Tile> {
        self.epoch += 1;
        std::sync::Arc::make_mut(&mut self.tiles).iter_mut()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Pos, &Tile)> + '_ {
        self.tiles.iter().map(|tile| (&tile.pos, tile))
    }

    pub fn into_values(self) -> std::vec::IntoIter<Tile> {
        match std::sync::Arc::try_unwrap(self.tiles) {
            Ok(tiles) => tiles.into_iter(),
            Err(shared) => shared.as_slice().to_vec().into_iter(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cliff_edges_require_one_land_side_and_one_water_side() {
        let mut world = WorldMap::new(5, 5);
        let land = hex::offset_to_axial(2, 2);
        let neighbors = world.neighbors(land);
        let water = neighbors[0];
        let other_land = neighbors[1];
        world.tiles.get_mut(&land).unwrap().terrain = "plains".into();
        world.tiles.get_mut(&other_land).unwrap().terrain = "grassland".into();

        assert!(
            !world.set_cliff_edge(land, other_land, true),
            "a cliff cannot be placed between two land tiles"
        );
        assert!(!world.has_cliff_edge(land, other_land));
        assert!(
            !world.set_cliff_edge(water, neighbors[2], true),
            "a cliff cannot be placed between two water tiles"
        );

        for terrain in ["coast", "ocean", "lake"] {
            world.tiles.get_mut(&water).unwrap().terrain = terrain.into();
            assert!(
                world.set_cliff_edge(land, water, true),
                "{terrain} is water"
            );
            assert!(world.has_cliff_edge(land, water));
            assert!(world.set_cliff_edge(land, water, false));
        }

        world.tiles.get_mut(&water).unwrap().terrain = "coast".into();
        assert!(world.set_cliff_edge(land, water, true));
        world.tiles.get_mut(&water).unwrap().terrain = "plains".into();
        assert!(
            !world.has_cliff_edge(land, water),
            "a stale serialized edge cannot act as a land-to-land cliff"
        );
        assert!(world.set_cliff_edge(land, water, false));
    }
}

impl<'a> IntoIterator for &'a TileGrid {
    type Item = (&'a Pos, &'a Tile);
    type IntoIter = std::iter::Map<std::slice::Iter<'a, Tile>, fn(&'a Tile) -> (&'a Pos, &'a Tile)>;

    fn into_iter(self) -> Self::IntoIter {
        self.tiles.iter().map(|tile| (&tile.pos, tile))
    }
}

/// Mutable iteration hands back an owned `Pos`: the position lives inside the
/// tile, so it cannot be lent out immutably while the tile itself is lent out
/// mutably.
impl<'a> IntoIterator for &'a mut TileGrid {
    type Item = (Pos, &'a mut Tile);
    type IntoIter =
        std::iter::Map<std::slice::IterMut<'a, Tile>, fn(&'a mut Tile) -> (Pos, &'a mut Tile)>;

    fn into_iter(self) -> Self::IntoIter {
        self.epoch += 1;
        std::sync::Arc::make_mut(&mut self.tiles)
            .iter_mut()
            .map(|tile| (tile.pos, tile))
    }
}

impl std::ops::Index<&Pos> for TileGrid {
    type Output = Tile;

    #[inline]
    fn index(&self, pos: &Pos) -> &Tile {
        self.get(pos).expect("tile position outside the world map")
    }
}

impl PartialEq for TileGrid {
    fn eq(&self, other: &Self) -> bool {
        self.width == other.width && self.height == other.height && self.tiles == other.tiles
    }
}

/// What shape the world is.
///
/// Every stock map script is a cylinder: a rectangle that wraps east to west
/// and ends at a northern and a southern edge. Planet is a closed globe — the
/// hexagons and twelve pentagons of a subdivided icosahedron — whose tiles are
/// stored in the same rectangle but whose adjacency, distance and latitude all
/// come from the sphere instead of from the offset coordinates. Rectangle is
/// neither: a bounded arena with a wall on all four sides.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Topology {
    #[default]
    Cylinder,
    /// A geodesic globe, identified by its subdivision frequency.
    Globe(i32),
    /// A bounded rectangle: four walls, no wrap on either axis.
    ///
    /// A cylinder's east and west edges are the same edge, which is what a
    /// world wants and an arena cannot have — on a Tactics battlefield a
    /// flank that walks off the east side must not reappear behind the enemy
    /// in the west, and an archer on one wall must not be in range of a
    /// spearman on the other. Every question a cylinder answers by wrapping —
    /// who is adjacent, how far apart, which way is that — this shape answers
    /// by stopping at the wall.
    Rectangle,
}

impl Topology {
    /// Whether the map's east and west edges are the same edge.
    ///
    /// A globe wraps in every direction, but through its own geometry rather
    /// than through the offset grid's longitudes, so it answers `false` here
    /// and every wrap-aware helper checks for the sphere first.
    #[inline]
    pub const fn wraps_east_west(self) -> bool {
        matches!(self, Self::Cylinder)
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(from = "WorldMapSer", into = "WorldMapSer")]
pub struct WorldMap {
    pub width: i32,
    pub height: i32,
    pub tiles: TileGrid,
    pub topology: Topology,
    /// The globe this map is laid out on, when it is one. Geometry is a pure
    /// function of the frequency, so it is shared rather than saved.
    globe: Option<std::sync::Arc<crate::sphere::Sphere>>,
}

#[derive(Clone, Serialize, Deserialize)]
struct WorldMapSer {
    width: i32,
    height: i32,
    tiles: Vec<Tile>,
    /// Absent in saves written before Planet existed, which were all cylinders.
    #[serde(default, skip_serializing_if = "is_cylinder")]
    topology: Topology,
}

fn is_cylinder(topology: &Topology) -> bool {
    *topology == Topology::Cylinder
}

impl From<WorldMapSer> for WorldMap {
    fn from(s: WorldMapSer) -> WorldMap {
        WorldMap {
            width: s.width,
            height: s.height,
            tiles: TileGrid::from_tiles(s.width, s.height, s.tiles),
            topology: s.topology,
            globe: match s.topology {
                Topology::Cylinder | Topology::Rectangle => None,
                Topology::Globe(frequency) => Some(crate::sphere::sphere(frequency)),
            },
        }
    }
}

impl From<WorldMap> for WorldMapSer {
    fn from(m: WorldMap) -> WorldMapSer {
        WorldMapSer {
            width: m.width,
            height: m.height,
            topology: m.topology,
            tiles: m.tiles.into_values().collect(),
        }
    }
}

impl WorldMap {
    pub fn new(width: i32, height: i32) -> WorldMap {
        WorldMap {
            width,
            height,
            tiles: TileGrid::new(width, height),
            topology: Topology::Cylinder,
            globe: None,
        }
    }

    /// A bounded arena: the same rectangle of tiles as [`Self::new`], walled
    /// on all four sides instead of wrapping east to west.
    pub fn arena(width: i32, height: i32) -> WorldMap {
        WorldMap {
            topology: Topology::Rectangle,
            ..WorldMap::new(width, height)
        }
    }

    /// An all-ocean globe of the given subdivision frequency: `10n² + 2` tiles
    /// laid out in the rectangle [`crate::sphere`] describes.
    pub fn globe(frequency: i32) -> WorldMap {
        let sphere = crate::sphere::sphere(frequency);
        let tiles: Vec<Tile> = sphere.positions().map(Tile::new).collect();
        WorldMap {
            width: sphere.width(),
            height: sphere.height(),
            tiles: TileGrid::from_tiles(sphere.width(), sphere.height(), tiles),
            topology: Topology::Globe(frequency),
            globe: Some(sphere),
        }
    }

    /// The globe this map is laid out on, or `None` on a cylinder.
    #[inline]
    pub fn sphere(&self) -> Option<&crate::sphere::Sphere> {
        self.globe.as_deref()
    }

    /// Whether travelling east far enough comes back round to where it began.
    ///
    /// A cylinder's two longitudes are one longitude and a globe closes on
    /// itself through its own geometry; a bounded arena has a wall there
    /// instead, and is the only shape that answers `false`. The browser needs
    /// this to know whether the chart it draws may be unrolled at all, so it
    /// is read off the built map rather than off the setup — a loaded save
    /// answers too.
    #[inline]
    pub fn wraps_east_west(&self) -> bool {
        self.sphere().is_some() || self.topology.wraps_east_west()
    }

    #[inline]
    pub fn get(&self, pos: Pos) -> Option<&Tile> {
        self.tiles.get(&pos)
    }

    /// Fold a position back onto the map's own longitudes.
    ///
    /// A cylinder's east and west edges are the same edge, so a step past one
    /// arrives at the other. A bounded rectangle has a wall there instead:
    /// the position stays outside the grid, and the tile lookup that every
    /// caller does next is what turns it into "there is nothing that way".
    #[inline]
    fn fold(&self, pos: Pos) -> Pos {
        if self.topology.wraps_east_west() {
            hex::canon(pos, self.width)
        } else {
            pos
        }
    }

    /// The tiles that share an edge with this one, and are on the map.
    #[inline]
    pub fn neighbors(&self, pos: Pos) -> hex::Neighbors {
        if let Some(sphere) = self.sphere() {
            return sphere.neighbors(pos);
        }
        let mut out = hex::Neighbors::new();
        for neighbor in hex::neighbors(pos) {
            let neighbor = self.fold(neighbor);
            if self.tiles.contains_key(&neighbor) {
                out.push(neighbor);
            }
        }
        out
    }

    /// Every direction out of a tile, whether or not the world continues that
    /// way. A cylinder has an edge at the top and the bottom, and rules that
    /// ask "is this hex surrounded?" must see those as directions that lead
    /// nowhere rather than as directions that do not exist. A bounded arena
    /// has four such edges rather than two. A globe has none, so this is
    /// simply its neighbours.
    pub fn around(&self, pos: Pos) -> hex::Neighbors {
        if self.sphere().is_some() {
            return self.neighbors(pos);
        }
        hex::neighbors(pos)
            .into_iter()
            .map(|neighbor| self.fold(neighbor))
            .collect()
    }

    /// Steps between two tiles along the world's own shape.
    #[inline]
    pub fn distance(&self, a: Pos, b: Pos) -> i32 {
        match self.sphere() {
            Some(sphere) => sphere.distance(a, b),
            // Measured through the seam on a cylinder, and never through the
            // wall on an arena: an archer on one edge of a battlefield is the
            // width of the field away from the far edge, not two steps.
            None if self.topology.wraps_east_west() => hex::wdistance(a, b, self.width),
            None => hex::distance(a, b),
        }
    }

    /// Every tile within `radius` steps of `center`, sorted.
    pub fn disk(&self, center: Pos, radius: i32) -> Vec<Pos> {
        if let Some(sphere) = self.sphere() {
            return sphere.disk(center, radius);
        }
        let mut out: Vec<Pos> = hex::disk(center, radius)
            .into_iter()
            .map(|pos| self.fold(pos))
            .filter_map(|pos| self.tiles.index_of(pos).map(|_| pos))
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Every in-map tile exactly `radius` steps from `center`, sorted.
    ///
    /// A search that walks outward ring by ring used to ask for the whole disk
    /// at each radius and throw away everything inside it, which made the walk
    /// cost the square of the distance it covered — and sorted the disk every
    /// time. The exploration search does exactly that walk, and this was the
    /// largest single cost in the engine.
    ///
    /// A caller that filters on [`crate::game::Game::wdist`] gets the same answer either
    /// way: a disk is the union of its rings, and a tile from an inner ring
    /// cannot be at the outer ring's distance.
    pub fn ring(&self, center: Pos, radius: i32) -> Vec<Pos> {
        if let Some(sphere) = self.sphere() {
            return sphere.ring(center, radius);
        }
        let mut out: Vec<Pos> = hex::ring(center, radius)
            .into_iter()
            .map(|pos| self.fold(pos))
            .filter_map(|pos| self.tiles.index_of(pos).map(|_| pos))
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// How far the tile is from the equator, from 0 at the equator to 1 at a
    /// pole. Climate bands are painted from this, so it has to follow the
    /// world's real shape rather than its storage rectangle.
    pub fn polar_fraction(&self, pos: Pos) -> f64 {
        match self.sphere() {
            Some(sphere) => sphere.latitude(pos).abs() / std::f64::consts::FRAC_PI_2,
            None => {
                let (_, row) = hex::axial_to_offset(pos.0, pos.1);
                (2.0 * row as f64 / (self.height - 1).max(1) as f64 - 1.0).abs()
            }
        }
    }

    /// Where the tile stands on the world, in degrees of longitude
    /// (-180..180, east positive) and latitude (-90..90, north positive).
    ///
    /// A globe reads both off the sphere. A cylinder has no globe to read, so
    /// it is treated as the equirectangular projection it looks like: its
    /// columns are meridians spread evenly around the world and its rows are
    /// parallels from pole to pole, which is the same reading
    /// [`Self::polar_fraction`] already takes of a row. Anything that wants to
    /// place a real-world feature — Earth's coastlines, a civilization's
    /// homeland — can then ask the world where it is without first asking what
    /// shape it is.
    pub fn lon_lat(&self, pos: Pos) -> (f64, f64) {
        match self.sphere() {
            Some(sphere) => (
                sphere.longitude(pos).to_degrees(),
                sphere.latitude(pos).to_degrees(),
            ),
            None => {
                let (col, row) = hex::axial_to_offset(pos.0, pos.1);
                let longitude = 360.0 * col as f64 / self.width.max(1) as f64 - 180.0;
                let latitude = 90.0 - 180.0 * row as f64 / (self.height - 1).max(1) as f64;
                (longitude, latitude)
            }
        }
    }

    /// The unit vector the tile's centre points along, on the unit sphere the
    /// world's longitudes and latitudes describe. A globe has a real centre to
    /// return; a cylinder gets the point its projection names, which is what
    /// makes "nearest to this homeland" answerable on either shape.
    pub fn direction(&self, pos: Pos) -> [f64; 3] {
        if let Some(center) = self.sphere().and_then(|sphere| sphere.center(pos)) {
            return center;
        }
        let (longitude, latitude) = self.lon_lat(pos);
        let (longitude, latitude) = (longitude.to_radians(), latitude.to_radians());
        [
            trig::cos(latitude) * trig::cos(longitude),
            trig::cos(latitude) * trig::sin(longitude),
            trig::sin(latitude),
        ]
    }

    /// The neighbour `heading` steps around the tile, used by anything that
    /// travels in a fixed direction. A cylinder or an arena counts from due
    /// east; a globe counts around the tile's own outline.
    pub fn step(&self, pos: Pos, heading: usize) -> Option<Pos> {
        if self.sphere().is_some() {
            let neighbors = self.neighbors(pos);
            return neighbors.get(heading % neighbors.len().max(1)).copied();
        }
        let step = hex::DIRS[heading % 6];
        let next = self.fold((pos.0 + step.0, pos.1 + step.1));
        self.tiles.contains_key(&next).then_some(next)
    }

    /// Direction index from one adjacent tile to another, accounting for the
    /// east-west cylindrical seam or, on a globe, for the tile's own outline.
    /// An arena has no seam to account for.
    pub fn direction_to(&self, from: Pos, to: Pos) -> Option<usize> {
        if let Some(sphere) = self.sphere() {
            return sphere.direction_to(from, to);
        }
        hex::neighbors(from)
            .into_iter()
            .map(|p| self.fold(p))
            .position(|p| p == to)
    }

    /// Add or remove the river segment shared by two adjacent tiles.
    /// Returns false when either tile is absent or the positions are not
    /// adjacent. Keeping both edge masks in sync makes saves and observations
    /// self-contained tile by tile.
    pub fn set_river_edge(&mut self, a: Pos, b: Pos, present: bool) -> bool {
        let (Some(there), Some(back)) = (self.direction_to(a, b), self.direction_to(b, a)) else {
            return false;
        };
        if !self.tiles.contains_key(&a) || !self.tiles.contains_key(&b) {
            return false;
        }
        self.tiles.get_mut(&a).unwrap().river_edges[there] = present;
        self.tiles.get_mut(&b).unwrap().river_edges[back] = present;
        true
    }

    /// Whether the shared boundary between two adjacent tiles carries a river.
    pub fn has_river_edge(&self, a: Pos, b: Pos) -> bool {
        self.direction_to(a, b)
            .and_then(|direction| self.tiles.get(&a).map(|t| t.river_edges[direction]))
            .unwrap_or(false)
    }

    /// Add or remove a coastal cliff on the shared edge between two tiles.
    ///
    /// A cliff is a shoreline feature, never a generic elevation boundary:
    /// setting one is therefore valid only when exactly one side is water.
    /// Clearing remains allowed for an old save whose terrain later changed.
    pub fn set_cliff_edge(&mut self, a: Pos, b: Pos, present: bool) -> bool {
        let (Some(there), Some(back)) = (self.direction_to(a, b), self.direction_to(b, a)) else {
            return false;
        };
        let (Some(a_tile), Some(b_tile)) = (self.tiles.get(&a), self.tiles.get(&b)) else {
            return false;
        };
        if present && !Self::is_cliff_shore(a_tile, b_tile) {
            return false;
        }
        self.tiles.get_mut(&a).unwrap().cliff_edges[there] = present;
        self.tiles.get_mut(&b).unwrap().cliff_edges[back] = present;
        true
    }

    /// Whether the two tiles form the shoreline where a coastal cliff may sit.
    /// `WorldMap` deliberately does not own the rules database, so this names
    /// the complete built-in water terrain set directly.
    fn is_cliff_shore(a: &Tile, b: &Tile) -> bool {
        let water = |tile: &Tile| matches!(tile.terrain.as_str(), "coast" | "ocean" | "lake");
        water(a) != water(b)
    }

    pub fn has_cliff_edge(&self, a: Pos, b: Pos) -> bool {
        let (Some(there), Some(back)) = (self.direction_to(a, b), self.direction_to(b, a)) else {
            return false;
        };
        let (Some(a_tile), Some(b_tile)) = (self.tiles.get(&a), self.tiles.get(&b)) else {
            return false;
        };
        Self::is_cliff_shore(a_tile, b_tile)
            && a_tile.cliff_edges[there]
            && b_tile.cliff_edges[back]
    }

    pub fn clear_rivers(&mut self) {
        for tile in self.tiles.values_mut() {
            tile.river_edges = [false; 6];
            tile.riverside = false;
        }
    }
}

impl Tile {
    pub fn has_river(&self) -> bool {
        self.riverside || self.river_edges.iter().any(|edge| *edge)
    }
}

/// A set of tiles held as one bit each.
///
/// Visibility is unioned constantly — every unit's view into its owner's,
/// every ally's into the alliance's — and doing that through a `BTreeSet` of
/// positions meant an allocation and a tree descent per tile. Bits are
/// indexed by [`TileGrid::index_of`], which runs in position order, so
/// reading a `TileBits` back out yields tiles already sorted.
#[derive(Clone, Default, PartialEq)]
pub struct TileBits {
    words: Vec<u64>,
}

impl TileBits {
    pub fn with_capacity(tiles: usize) -> TileBits {
        TileBits {
            words: vec![0; tiles.div_ceil(64)],
        }
    }

    /// Every tile of a map of `tiles` hexes. The bits past the last tile in
    /// the final word are set too and are never asked about: nothing looks up
    /// an index the map has no tile for.
    pub fn all(tiles: usize) -> TileBits {
        TileBits {
            words: vec![u64::MAX; tiles.div_ceil(64)],
        }
    }

    #[inline]
    pub fn insert(&mut self, index: usize) {
        let word = index / 64;
        if word >= self.words.len() {
            self.words.resize(word + 1, 0);
        }
        self.words[word] |= 1 << (index % 64);
    }

    #[inline]
    pub fn contains(&self, index: usize) -> bool {
        self.words
            .get(index / 64)
            .is_some_and(|word| word & (1 << (index % 64)) != 0)
    }

    pub fn union_with(&mut self, other: &TileBits) {
        if self.words.len() < other.words.len() {
            self.words.resize(other.words.len(), 0);
        }
        for (into, from) in self.words.iter_mut().zip(&other.words) {
            *into |= *from;
        }
    }

    pub fn clear(&mut self) {
        self.words.iter_mut().for_each(|word| *word = 0);
    }

    /// Whether every tile in this set is also in `other`.
    pub fn is_subset_of(&self, other: &TileBits) -> bool {
        self.words
            .iter()
            .enumerate()
            .all(|(slot, word)| word & !other.words.get(slot).copied().unwrap_or(0) == 0)
    }

    /// The set members in ascending index order.
    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().enumerate().flat_map(|(slot, word)| {
            let mut bits = *word;
            std::iter::from_fn(move || {
                if bits == 0 {
                    return None;
                }
                let bit = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                Some(slot * 64 + bit)
            })
        })
    }
}

impl TileGrid {
    /// The position at a tile index, as handed out by [`TileGrid::index_of`].
    #[inline]
    pub fn pos_at(&self, index: usize) -> Option<Pos> {
        self.tiles.get(index).map(|tile| tile.pos)
    }

    /// The positions in a bit set, in map order.
    pub fn positions(&self, bits: &TileBits) -> impl Iterator<Item = Pos> + '_ {
        bits.iter()
            .filter_map(|index| self.tiles.get(index).map(|tile| tile.pos))
            .collect::<Vec<_>>()
            .into_iter()
    }
}
