//! Coastal cliffs, by `AddCliffs` (`MountainsCliffs.lua`).
//!
//! The stock rule has no dice in it. A Hills plot that touches salt water and
//! no sea ice is a cliff along every edge it shares with water, a lake's edge
//! included; flat ground on the coast never is. The one exception is a
//! landmass that would be left with no way ashore: a single-plot island, or
//! one with no flat coast at all, keeps its hills open.
//!
//! The script also means to spare a hill beside a river, but its
//! `IsAdjacentToRiver` reads two globals nobody sets and answers "no" for
//! every plot, so rivers spare nothing and are not consulted here either.

use std::collections::BTreeSet;

use super::connected_components;
use crate::world::{Tile, WorldMap};
use crate::Pos;

fn is_salt_water(tile: &Tile) -> bool {
    matches!(tile.terrain.as_str(), "coast" | "ocean")
}

fn is_water(tile: &Tile) -> bool {
    matches!(tile.terrain.as_str(), "coast" | "ocean" | "lake")
}

/// Lay the cliffs of a finished coastline.
///
/// `table_hills` is the script's own plot table where there is one: the
/// stock pass reads `plotTypes`, which still calls a plot Hills after
/// `AddTerrainFromContinents` has raised a Mountain on it and does not know
/// the hills that pass added. A world with no such table is read off its
/// tiles.
pub(super) fn add_cliffs(
    wm: &mut WorldMap,
    land: &BTreeSet<Pos>,
    table_hills: Option<&BTreeSet<Pos>>,
) {
    let mut edges: Vec<(Pos, Pos)> = Vec::new();
    for area in connected_components(wm, land) {
        if area.len() <= 1 {
            continue;
        }
        // `not area:HasNoFlatCoast()`.
        let has_flat_coast = area.iter().any(|pos| {
            let tile = &wm.tiles[pos];
            !tile.hills
                && tile.terrain != "mountain"
                && wm
                    .neighbors(*pos)
                    .into_iter()
                    .any(|neighbor| wm.tiles.get(&neighbor).is_some_and(is_salt_water))
        });
        if !has_flat_coast {
            continue;
        }
        for pos in &area {
            let is_hills = match table_hills {
                Some(hills) => hills.contains(pos),
                None => wm.tiles[pos].hills,
            };
            if !is_hills {
                continue;
            }
            let neighbors = wm.neighbors(*pos);
            let beside_salt_water = neighbors
                .into_iter()
                .any(|neighbor| wm.tiles.get(&neighbor).is_some_and(is_salt_water));
            let beside_ice = neighbors.into_iter().any(|neighbor| {
                wm.tiles
                    .get(&neighbor)
                    .is_some_and(|tile| tile.feature.as_deref() == Some("ice"))
            });
            if !beside_salt_water || beside_ice {
                continue;
            }
            for neighbor in neighbors {
                if wm.tiles.get(&neighbor).is_some_and(is_water) {
                    edges.push((*pos, neighbor));
                }
            }
        }
    }
    for (land, water) in edges {
        wm.set_cliff_edge(land, water, true);
    }
}
