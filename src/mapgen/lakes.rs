//! Lakes: flooding inland plots, and telling a lake from an inland sea.
//!
//! The stock generator's lake budget for a large map, which plots are
//! eligible, flooding one and spreading it (`flood_lake_plot`,
//! `spread_lake`), `add_lakes` with `LAKE_PLOT_RANDOM`, and
//! `classify_lakes`, which marks a body of at most `LAKE_MAX_AREA_SIZE`
//! plots a lake and a larger one an inland sea.
//!
//! Moved verbatim out of `mapgen.rs` (2026-11-05). `mapgen.rs` re-exports
//! everything here, so every path is unchanged; items the rest of `mapgen`
//! uses are `pub(super)`.

use super::*;

/// `GlobalParameters.LAKE_PLOT_RANDOM`: one eligible inland plot in forty
/// floods when the stock generator adds lakes.
const LAKE_PLOT_RANDOM: usize = 40;

/// `GlobalParameters.LAKE_MAX_AREA_SIZE`: the whole of Civ VI's distinction
/// between the two kinds of enclosed water. A body of at most nine plots is a
/// **lake** — fresh water for every tile around it, a Fishery, and the site
/// Huey Teocalli has to be built on. A larger one is an **inland sea**: Coast
/// that ships can sail and work, and that no city can drink from.
pub(super) const LAKE_MAX_AREA_SIZE: usize = 9;

/// How many bodies a world type may spread past a single plot.
///
/// Each stock script passes `AddLakes` a multiple of the `Maps.Continents`
/// column: four times it on Lakes, one and a half on Pangaea, once on
/// Continents and Small Continents, and half on Inland Sea and Island Plates,
/// rounded up. Every roll past that budget is a one-plot pond.
///
/// Earth asks for none, but for a third reason again: it already has its own.
/// The Caspian, the Great Lakes, Victoria, Baikal, Balkhash and the Aral are
/// in the grid the coastlines come from, so they arrive as enclosed water and
/// `classify_lakes` sorts them by area exactly as it sorts a grown one. Adding
/// a rolled lake on top would be inventing a body of water on a map whose
/// whole promise is that its water is real.
///
/// Land Only asks for none for the opposite reason: the water it already has
/// is inland by construction, so the pass would be spreading lakes through a
/// world that is nothing but lakes.
pub(super) fn large_lake_budget(script: MapScript, num_continents: usize) -> usize {
    match script {
        MapScript::Lakes => num_continents * 4,
        MapScript::Pangaea => (num_continents * 3).div_ceil(2),
        MapScript::Continents | MapScript::SmallContinents => num_continents,
        MapScript::InlandSea | MapScript::Islands => num_continents.div_ceil(2),
        // The blocks a canal world is cut into are broad enough to hold one,
        // and a lake is the water a canal world does not already have: fresh,
        // enclosed, and nothing a ship arrives by.
        MapScript::TacticsPlanet | MapScript::GrandCanals => num_continents,
        // A block of a Grand Canals II world is a few dozen tiles of ground
        // with a canal already around it, so its middle is never far from a
        // bank and a spread lake would be most of what a city had to work.
        // The one-plot ponds the same roll produces still fall.
        MapScript::GrandCanalsTwo => 0,
        MapScript::Earth
        | MapScript::TrueStartEarth
        | MapScript::LandOnly
        | MapScript::TeninsBall
        | MapScript::Fjords
        | MapScript::WaterWorld => 0,
        // The battlefield carves its own ponds when its land is laid; a
        // spread lake on a ten-tile-wide arena would be most of the arena.
        MapScript::Battlefield => 0,
        // And the naval one is already almost all water. Its land arrives as
        // islets a handful of tiles across, which is smaller than a spread
        // lake — there is nothing on the map for one to be inside.
        MapScript::TacticsOcean => 0,
        // And a scenario's water is its chart's, down to the tile.
        MapScript::Trafalgar => 0,
        MapScript::Kadesh
        | MapScript::Marathon
        | MapScript::Thermopylae
        | MapScript::Gaugamela
        | MapScript::Cannae
        | MapScript::Actium
        | MapScript::Hastings
        | MapScript::Hattin
        | MapScript::Agincourt
        | MapScript::Constantinople1453
        | MapScript::Lepanto
        | MapScript::SpanishArmada
        | MapScript::Waterloo
        | MapScript::Gettysburg
        | MapScript::Stalingrad
        | MapScript::Normandy
        | MapScript::Midway
        | MapScript::Inchon
        | MapScript::DienBienPhu
        | MapScript::SixDayWar
        | MapScript::DesertStorm
        | MapScript::Fallujah
        | MapScript::Mosul => 0,
    }
}

/// Whether a plot may flood, using the stock script's filters.
///
/// A lake plot has to be inland land: not water, not *coastal* land, not a
/// volcano, and clear of rivers and of tiles carrying one. The enclosure is a precondition of
/// placement rather than something checked afterwards, and it is also what
/// keeps two lakes apart — a plot beside one that has already flooded is
/// coastal land by the time the scan reaches it.
fn lake_eligible(wm: &WorldMap, land: &BTreeSet<Pos>, pos: Pos) -> bool {
    if !land.contains(&pos)
        || wm
            .tiles
            .get(&pos)
            .is_none_or(|tile| tile.has_river() || tile.feature.as_deref() == Some("volcano"))
    {
        return false;
    }
    wm.around(pos)
        .into_iter()
        .all(|neighbor| match wm.tiles.get(&neighbor) {
            // Civ VI's polar rows are always ocean, so a plot on the world's
            // edge is coastal land there and never floods. A CIVVIS script may
            // put land on the rim — Inland Sea rings its basin that way — and
            // treating the edge as a shore keeps that rim whole rather than
            // punching a lake through it.
            None => false,
            Some(tile) => land.contains(&neighbor) && !tile.has_river(),
        })
}

/// Flood one plot. The stock script leaves it as Coast and lets the engine
/// decide afterwards whether the body it belongs to is small enough to be a
/// lake, which is what [`classify_lakes`] does here.
///
/// Dropping the plot from `land` is what makes the water real to the rest of
/// generation: every later pass — features, volcanoes, natural wonders,
/// resources, and spawns — works from that set, so none of them will
/// plant anything in the new lake or strand a civilization in it.
fn flood_lake_plot(wm: &mut WorldMap, land: &mut BTreeSet<Pos>, pos: Pos) {
    land.remove(&pos);
    let tile = wm.tiles.get_mut(&pos).unwrap();
    tile.terrain = "coast".into();
    // Relief was settled before the rivers ran; water has none.
    tile.hills = false;
    // Continents are assigned before relief so their boundaries can influence
    // tectonics. A plot that becomes water must still keep Tile's public
    // promise that water has no continent.
    tile.continent = None;
}

/// `AddMoreLake`: give the six neighbours of a fresh lake a chance to join it.
///
/// The denominator grows with the body, so the sixth plot is far less likely
/// than the first and a lake stops of its own accord. The stock script counts
/// the attempt against the large-lake budget only when three or more take, but
/// floods whatever it picked either way.
fn spread_lake(wm: &mut WorldMap, land: &mut BTreeSet<Pos>, pos: Pos, rng: &mut Rng) -> bool {
    let mut picked: Vec<Pos> = Vec::new();
    for neighbor in wm.neighbors(pos) {
        // Eligibility is read before anything floods, so the six are judged
        // against the shore as it stood when the lake was drawn.
        if lake_eligible(wm, land, neighbor) && rng.below(4 + picked.len()) < 3 {
            picked.push(neighbor);
        }
    }
    let grew = picked.len() > 2;
    for neighbor in picked {
        flood_lake_plot(wm, land, neighbor);
    }
    grew
}

/// Civ VI's `AddLakes` (`RiversLakes.lua`), in its stock position in the
/// pipeline: after the rivers, because "lakes would interfere with rivers,
/// causing them to stop and not reach the ocean, if placed any sooner".
pub(super) fn add_lakes(
    wm: &mut WorldMap,
    land: &mut BTreeSet<Pos>,
    mut large_lakes: usize,
    rng: &mut Rng,
) {
    let (width, height) = (wm.width, wm.height);
    let scan: Vec<Pos> = (0..height)
        .flat_map(|row| (0..width).map(move |col| hex::offset_to_axial(col, row)))
        .collect();
    for pos in scan {
        if !lake_eligible(wm, land, pos) || rng.below(LAKE_PLOT_RANDOM) != 0 {
            continue;
        }
        if large_lakes > 0 && spread_lake(wm, land, pos, rng) {
            large_lakes -= 1;
        }
        flood_lake_plot(wm, land, pos);
    }
}

/// Sort the world's enclosed water into lakes and inland seas by area, the way
/// `AreaBuilder` does once the terrain is settled.
///
/// This runs for every script, not only the ones that add lakes, because a
/// fractal coastline encloses basins of its own: a pocket the sea cannot reach
/// has always been a lake, and until now CIVVIS painted it as open ocean.
pub(super) fn classify_lakes(wm: &mut WorldMap) {
    let water: BTreeSet<Pos> = wm
        .tiles
        .iter()
        .filter(|(_, tile)| matches!(tile.terrain.as_str(), "coast" | "ocean"))
        .map(|(pos, _)| *pos)
        .collect();
    let enclosed = connected_components(wm, &water);
    for body in enclosed {
        if body.len() > LAKE_MAX_AREA_SIZE {
            continue;
        }
        for pos in body {
            wm.tiles.get_mut(&pos).unwrap().terrain = "lake".into();
        }
    }
}
