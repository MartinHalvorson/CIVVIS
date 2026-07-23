//! Tiles and the world map (mirrors civvis/world.py).
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};

use crate::{hex, Pos};

/// A district site that has been placed but has not finished construction.
/// Placement locks both the chosen district and its production cost.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DistrictFoundation {
    pub district: String,
    pub cost: f64,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Tile {
    pub pos: Pos,
    pub terrain: String,
    pub feature: Option<String>,
    pub hills: bool,
    pub resource: Option<String>,
    pub improvement: Option<String>,
    /// Improvements and ordinary districts stop producing yields while
    /// pillaged. City/Encampment defenses keep their dedicated damage state.
    #[serde(default)]
    pub pillaged: bool,
    pub district: Option<String>,
    /// Placed districts occupy their tile and count against district limits,
    /// but do not grant completed-district yields or abilities.
    #[serde(default)]
    pub district_foundation: Option<DistrictFoundation>,
    #[serde(default)]
    pub wonder: Option<String>,
    pub owner_city: Option<u32>,
    #[serde(default)]
    /// River segments on this hex's six edges, in `hex::DIRS` order.
    /// Shared edges are mirrored on both neighboring tiles.
    pub river_edges: [bool; 6],
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
    /// Permanent fertility left by floods, eruptions, storms, and fires.
    /// These stay separate from the underlying terrain/feature so repeated
    /// events can accumulate exactly as they do in Gathering Storm.
    #[serde(default)]
    pub disaster_food: f64,
    #[serde(default)]
    pub disaster_production: f64,
    #[serde(default)]
    pub disaster_science: f64,
    #[serde(default)]
    pub disaster_culture: f64,
    /// Ordinary volcano lifecycle: 0 dormant, 1 active, 2 erupting.
    #[serde(default)]
    pub volcano_state: u8,
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
pub struct TileMemory(BTreeMap<Pos, RememberedTile>);

impl Deref for TileMemory {
    type Target = BTreeMap<Pos, RememberedTile>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TileMemory {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<RememberedTile>> for TileMemory {
    fn from(tiles: Vec<RememberedTile>) -> Self {
        Self(
            tiles
                .into_iter()
                .map(|remembered| (remembered.tile.pos, remembered))
                .collect(),
        )
    }
}

impl From<TileMemory> for Vec<RememberedTile> {
    fn from(memory: TileMemory) -> Self {
        memory.0.into_values().collect()
    }
}

impl Tile {
    pub fn new(pos: Pos) -> Tile {
        Tile {
            pos,
            terrain: "ocean".to_string(),
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
            cliff_edges: [false; 6],
            road: 0,
            continent: None,
            disaster_faith: 0.0,
            disaster_food: 0.0,
            disaster_production: 0.0,
            disaster_science: 0.0,
            disaster_culture: 0.0,
            volcano_state: 0,
            drought: false,
            coastal_lowland: 0,
            flooded: false,
            submerged: false,
            fallout_until: 0,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(from = "WorldMapSer", into = "WorldMapSer")]
pub struct WorldMap {
    pub width: i32,
    pub height: i32,
    /// Whether the east and west edges are adjacent. Legacy maps default to
    /// the original cylindrical topology when this field is absent.
    pub wrap_x: bool,
    pub tiles: BTreeMap<Pos, Tile>,
}

#[derive(Clone, Serialize, Deserialize)]
struct WorldMapSer {
    width: i32,
    height: i32,
    #[serde(default = "default_wrap_x")]
    wrap_x: bool,
    tiles: Vec<Tile>,
}

const fn default_wrap_x() -> bool {
    true
}

impl From<WorldMapSer> for WorldMap {
    fn from(s: WorldMapSer) -> WorldMap {
        let tiles = s.tiles.into_iter().map(|t| (t.pos, t)).collect();
        WorldMap {
            width: s.width,
            height: s.height,
            wrap_x: s.wrap_x,
            tiles,
        }
    }
}

impl From<WorldMap> for WorldMapSer {
    fn from(m: WorldMap) -> WorldMapSer {
        WorldMapSer {
            width: m.width,
            height: m.height,
            wrap_x: m.wrap_x,
            tiles: m.tiles.into_values().collect(),
        }
    }
}

impl WorldMap {
    pub fn new(width: i32, height: i32) -> WorldMap {
        Self::new_with_wrap(width, height, true)
    }

    pub fn new_with_wrap(width: i32, height: i32, wrap_x: bool) -> WorldMap {
        let mut tiles = BTreeMap::new();
        for row in 0..height {
            for col in 0..width {
                let pos = hex::offset_to_axial(col, row);
                tiles.insert(pos, Tile::new(pos));
            }
        }
        WorldMap {
            width,
            height,
            wrap_x,
            tiles,
        }
    }

    pub fn get(&self, pos: Pos) -> Option<&Tile> {
        self.tiles.get(&pos)
    }

    /// Normalize a coordinate only on cylindrical maps. On bounded maps an
    /// off-map coordinate stays off-map and will fail ordinary tile lookup.
    pub fn canon(&self, pos: Pos) -> Pos {
        if self.wrap_x {
            hex::canon(pos, self.width)
        } else {
            pos
        }
    }

    pub fn distance(&self, a: Pos, b: Pos) -> i32 {
        if self.wrap_x {
            hex::wdistance(a, b, self.width)
        } else {
            hex::distance(a, b)
        }
    }

    pub fn neighbors(&self, pos: Pos) -> Vec<Pos> {
        hex::neighbors(pos)
            .into_iter()
            .map(|neighbor| self.canon(neighbor))
            .filter(|neighbor| self.tiles.contains_key(neighbor))
            .collect()
    }

    pub fn disk(&self, center: Pos, radius: i32) -> Vec<Pos> {
        let mut positions: Vec<Pos> = hex::disk(center, radius)
            .into_iter()
            .map(|position| self.canon(position))
            .filter(|position| self.tiles.contains_key(position))
            .collect();
        positions.sort();
        positions.dedup();
        positions
    }

    /// Direction index from one adjacent tile to another under this map's
    /// bounded or cylindrical topology.
    pub fn direction_to(&self, from: Pos, to: Pos) -> Option<usize> {
        hex::neighbors(from)
            .into_iter()
            .map(|position| self.canon(position))
            .position(|p| p == to)
    }

    /// Add or remove the river segment shared by two adjacent tiles.
    /// Returns false when either tile is absent or the positions are not
    /// adjacent. Keeping both edge masks in sync makes saves and observations
    /// self-contained tile by tile.
    pub fn set_river_edge(&mut self, a: Pos, b: Pos, present: bool) -> bool {
        let Some(direction) = self.direction_to(a, b) else {
            return false;
        };
        if !self.tiles.contains_key(&a) || !self.tiles.contains_key(&b) {
            return false;
        }
        self.tiles.get_mut(&a).unwrap().river_edges[direction] = present;
        self.tiles.get_mut(&b).unwrap().river_edges[(direction + 3) % 6] = present;
        true
    }

    /// Whether the shared boundary between two adjacent tiles carries a river.
    pub fn has_river_edge(&self, a: Pos, b: Pos) -> bool {
        self.direction_to(a, b)
            .and_then(|direction| self.tiles.get(&a).map(|t| t.river_edges[direction]))
            .unwrap_or(false)
    }

    /// Add or remove a coastal cliff on the shared edge between two tiles.
    pub fn set_cliff_edge(&mut self, a: Pos, b: Pos, present: bool) -> bool {
        let Some(direction) = self.direction_to(a, b) else {
            return false;
        };
        if !self.tiles.contains_key(&a) || !self.tiles.contains_key(&b) {
            return false;
        }
        self.tiles.get_mut(&a).unwrap().cliff_edges[direction] = present;
        self.tiles.get_mut(&b).unwrap().cliff_edges[(direction + 3) % 6] = present;
        true
    }

    pub fn has_cliff_edge(&self, a: Pos, b: Pos) -> bool {
        self.direction_to(a, b)
            .and_then(|direction| self.tiles.get(&a).map(|t| t.cliff_edges[direction]))
            .unwrap_or(false)
    }

    pub fn clear_rivers(&mut self) {
        for tile in self.tiles.values_mut() {
            tile.river_edges = [false; 6];
        }
    }
}

impl Tile {
    pub fn has_river(&self) -> bool {
        self.river_edges.iter().any(|edge| *edge)
    }
}
