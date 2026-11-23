use super::shipped::{self, Grid, StockScript, PLOT_HILLS, PLOT_OCEAN};
use super::*;
use crate::setup::{MapPoles, MapScript, MapTopology, CIV6_MAP_SIZES};

const FLAT: MapTopology = MapTopology::Flat;
const GLOBE: MapTopology = MapTopology::Planet;
const POLED: MapPoles = MapPoles::Poles;

/// The world types the game ships, each with the script that draws it.
const STOCK: [(MapScript, StockScript); 6] = [
    (MapScript::Continents, StockScript::Continents),
    (MapScript::Pangaea, StockScript::Pangaea),
    (MapScript::SmallContinents, StockScript::SmallContinents),
    (MapScript::Islands, StockScript::IslandPlates),
    (MapScript::InlandSea, StockScript::InlandSea),
    (MapScript::Lakes, StockScript::Lakes),
];

fn is_sea(tile: &crate::world::Tile) -> bool {
    matches!(tile.terrain.as_str(), "coast" | "ocean")
}

fn is_water(tile: &crate::world::Tile) -> bool {
    matches!(tile.terrain.as_str(), "coast" | "ocean" | "lake")
}

/// A Small world, six seats and nine city-states, the shape the games are
/// played on.
fn small_world(script: MapScript, topology: MapTopology, seed: u64) -> WorldMap {
    let rules = Rules::embedded();
    let mut rng = Rng::new(seed);
    generate_with_script(
        &rules, 74, 46, 6, 9, 4, 3, script, topology, POLED, &mut rng,
    )
    .0
}

/// Every world type the game ships is on the list, and nothing of CIVVIS's
/// own is: a type either has a script to follow or it does not.
#[test]
fn the_six_shipped_world_types_and_no_others_have_a_stock_script() {
    for (script, stock) in STOCK {
        assert_eq!(stock_script(script), Some(stock), "{script:?}");
    }
    for spec in crate::setup::CIV6_MAP_SCRIPTS {
        let listed = STOCK.iter().any(|(script, _)| *script == spec.script);
        assert_eq!(
            stock_script(spec.script).is_some(),
            listed,
            "{:?}",
            spec.script
        );
    }
}

/// The stock scripts draw every board the game could itself make, and none
/// it could not: Duel is the smallest map there is, and the boards under it
/// are fixtures that keep the compact generators they were built on.
#[test]
fn a_stock_script_draws_from_duel_up_and_not_below() {
    for size in CIV6_MAP_SIZES {
        for topology in [FLAT, GLOBE] {
            let (width, height) = size.dimensions(topology);
            for (script, stock) in STOCK {
                assert_eq!(
                    draws_by_stock_script(script, width, height),
                    Some(stock),
                    "{script:?} on {} {topology:?}",
                    size.name
                );
            }
        }
    }
    for (width, height) in [(8, 8), (20, 14), (24, 16), (32, 24), (40, 26), (43, 26)] {
        for (script, _) in STOCK {
            assert_eq!(
                draws_by_stock_script(script, width, height),
                None,
                "{script:?} at {width}x{height}"
            );
        }
    }
    assert_eq!(
        draws_by_stock_script(MapScript::TeninsBall, 74, 46),
        None,
        "Tennis Ball is CIVVIS's own and has no script to follow"
    );
}

/// On a flat map the tile is the plot: the seas the game plays on are the
/// seas the script drew, from the same seed. Later passes only ever take
/// land away (a lake) or rename enclosed water (a lake again), so every plot
/// the script left as Ocean is water and every tile of open sea was Ocean to
/// the script.
#[test]
fn a_flat_stock_world_has_the_seas_its_script_drew() {
    let rules = Rules::embedded();
    for (script, stock) in STOCK {
        for size in &CIV6_MAP_SIZES[..4] {
            let seed = 4_100 + size.width as u64;
            let (world, _) = generate_with_script(
                &rules,
                size.width,
                size.height,
                size.default_players,
                size.default_city_states,
                size.natural_wonders,
                size.continents,
                script,
                FLAT,
                POLED,
                &mut Rng::new(seed),
            );
            let grid = Grid {
                w: size.width,
                h: size.height,
                wrap_x: shipped::wraps_east_west(stock),
            };
            let drawn = shipped::generate(stock, grid, false, &mut Rng::new(seed));
            for row in 0..size.height {
                for col in 0..size.width {
                    let tile = &world.tiles[&hex::offset_to_axial(col, row)];
                    if drawn.plot(col, row) == PLOT_OCEAN {
                        assert!(
                            is_water(tile),
                            "{script:?} {}: the script's sea at ({col}, {row}) is {}",
                            size.name,
                            tile.terrain
                        );
                    } else {
                        assert!(
                            !is_sea(tile),
                            "{script:?} {}: the script's land at ({col}, {row}) is {}",
                            size.name,
                            tile.terrain
                        );
                    }
                }
            }
        }
    }
}

/// Continents, as the games meet it: two continents with open sea between
/// them, on the size and seat count self-play runs.
#[test]
fn a_small_continents_world_has_two_continents() {
    let mut split = 0;
    let worlds = 12;
    for seed in 0..worlds {
        let world = small_world(MapScript::Continents, FLAT, 9_000 + seed);
        let land: BTreeSet<Pos> = world
            .tiles
            .iter()
            .filter(|(_, tile)| !is_water(tile))
            .map(|(pos, _)| *pos)
            .collect();
        let mut sizes: Vec<usize> = connected_components(&world, &land)
            .iter()
            .map(BTreeSet::len)
            .collect();
        sizes.sort_unstable_by(|a, b| b.cmp(a));
        if sizes.len() >= 2 && sizes[1] * 100 >= land.len() * 15 {
            split += 1;
        }
    }
    assert!(
        split * 10 >= worlds * 8,
        "only {split} of {worlds} Small Continents worlds had a second continent"
    );
}

/// `InlandSea.lua` returns `WrapX = false` from `GetMapInitData`: the world
/// has an east edge and a west edge, and land along both. Every other stock
/// world wraps.
#[test]
fn a_flat_inland_sea_does_not_wrap_and_every_other_stock_world_does() {
    for (script, _) in STOCK {
        let world = small_world(script, FLAT, 77);
        assert_eq!(
            world.wraps_east_west(),
            script != MapScript::InlandSea,
            "{script:?}"
        );
    }
    let world = small_world(MapScript::InlandSea, FLAT, 77);
    for row in 0..46 {
        for col in [1, 73] {
            let tile = &world.tiles[&hex::offset_to_axial(col, row)];
            assert!(!is_sea(tile), "open sea on the rim at ({col}, {row})");
        }
    }
    let far_west = hex::offset_to_axial(0, 20);
    let far_east = hex::offset_to_axial(73, 20);
    assert!(
        world.distance(far_west, far_east) >= 70,
        "the two rims are {} apart",
        world.distance(far_west, far_east)
    );
}

/// A globe wraps the script's rectangle round a sphere. The twelve pentagons
/// stay sea, so every land tile keeps six neighbours, and no deep ocean
/// touches land: the shallows are found on the globe's own adjacency.
#[test]
fn a_stock_globe_keeps_its_pentagons_wet_and_its_shores_shallow() {
    for (script, _) in STOCK {
        let world = small_world(script, GLOBE, 31);
        let sphere = world.sphere().expect("a globe");
        for pentagon in sphere.pentagons() {
            assert!(
                is_water(&world.tiles[&pentagon]),
                "{script:?}: a pentagon is {}",
                world.tiles[&pentagon].terrain
            );
        }
        for (pos, tile) in world.tiles.iter() {
            if tile.terrain != "ocean" {
                continue;
            }
            for neighbor in world.neighbors(*pos) {
                assert!(
                    is_water(&world.tiles[&neighbor]),
                    "{script:?}: deep ocean at {pos:?} beside {}",
                    world.tiles[&neighbor].terrain
                );
            }
        }
    }
}

/// The same continents arrive on a globe: Continents is still two of them
/// and Pangaea still one, read through longitude and latitude.
#[test]
fn a_stock_globe_keeps_the_shape_of_its_world_type() {
    let masses = |script: MapScript, seed: u64| {
        let world = small_world(script, GLOBE, seed);
        let land: BTreeSet<Pos> = world
            .tiles
            .iter()
            .filter(|(_, tile)| !is_water(tile))
            .map(|(pos, _)| *pos)
            .collect();
        let mut sizes: Vec<usize> = connected_components(&world, &land)
            .iter()
            .map(BTreeSet::len)
            .collect();
        sizes.sort_unstable_by(|a, b| b.cmp(a));
        (land.len(), sizes)
    };
    let mut split = 0;
    for seed in 0..8 {
        let (land, sizes) = masses(MapScript::Continents, 600 + seed);
        if sizes.len() >= 2 && sizes[1] * 100 >= land * 15 {
            split += 1;
        }
        let (land, sizes) = masses(MapScript::Pangaea, 600 + seed);
        assert!(
            sizes[0] * 100 >= land * 75,
            "seed {seed}: a Pangaea globe's biggest landmass holds {} of {land}",
            sizes[0]
        );
    }
    assert!(split >= 6, "only {split} of 8 Continents globes were split");
}

/// `AddCliffs`: a cliff stands on a water edge of a Hills plot that touches
/// the sea, and nowhere else. Flat coast is never a cliff, which is what
/// keeps every landmass with a way ashore; and a landmass with no flat coast
/// of its own, or of a single plot, is left with none, so its hills stay
/// open.
#[test]
fn cliffs_stand_on_coastal_hills_and_never_on_flat_coast() {
    let rules = Rules::embedded();
    for (script, stock) in STOCK {
        let seed = 512;
        let (world, _) = generate_with_script(
            &rules,
            74,
            46,
            6,
            9,
            4,
            3,
            script,
            FLAT,
            POLED,
            &mut Rng::new(seed),
        );
        let grid = Grid {
            w: 74,
            h: 46,
            wrap_x: shipped::wraps_east_west(stock),
        };
        let drawn = shipped::generate(stock, grid, false, &mut Rng::new(seed));
        // Which land stands on a landmass the rule cliffs at all.
        let land: BTreeSet<Pos> = world
            .tiles
            .iter()
            .filter(|(_, tile)| !is_water(tile))
            .map(|(pos, _)| *pos)
            .collect();
        let mut cliffed_landmass: BTreeSet<Pos> = BTreeSet::new();
        for area in connected_components(&world, &land) {
            let flat_coast = area.iter().any(|pos| {
                let tile = &world.tiles[pos];
                !tile.hills
                    && tile.terrain != "mountain"
                    && world
                        .neighbors(*pos)
                        .into_iter()
                        .any(|neighbor| is_sea(&world.tiles[&neighbor]))
            });
            if area.len() > 1 && flat_coast {
                cliffed_landmass.extend(area);
            }
        }
        let mut cliffs = 0;
        let mut open_hill_shores = 0;
        for (pos, tile) in world.tiles.iter() {
            if is_water(tile) {
                continue;
            }
            let (col, row) = hex::axial_to_offset(pos.0, pos.1);
            let table_hills = drawn.plot(col, row) == PLOT_HILLS;
            let neighbors = world.neighbors(*pos);
            let beside_sea = neighbors
                .into_iter()
                .any(|neighbor| is_sea(&world.tiles[&neighbor]));
            let beside_ice = neighbors
                .into_iter()
                .any(|neighbor| world.tiles[&neighbor].feature.as_deref() == Some("ice"));
            for neighbor in neighbors {
                if !is_water(&world.tiles[&neighbor]) {
                    continue;
                }
                if world.has_cliff_edge(*pos, neighbor) {
                    cliffs += 1;
                    assert!(
                        table_hills && beside_sea && !beside_ice,
                        "{script:?}: a cliff at {pos:?} on ground the rule does not cliff"
                    );
                } else if table_hills && beside_sea && !beside_ice && cliffed_landmass.contains(pos)
                {
                    open_hill_shores += 1;
                }
            }
        }
        assert!(cliffs > 0, "{script:?}: no cliffs at all");
        // A wonder raised after the cliffs were laid can take a landmass's
        // one flat shore or give it one, which moves a handful of edges at
        // most; anything more is the rule not being followed.
        assert!(
            open_hill_shores * 50 <= cliffs,
            "{script:?}: {open_hill_shores} hill shores left open beside {cliffs} cliffs"
        );
    }
}

/// CIVVIS's own world types take the stock cliff rule too at any size the
/// game could make, without a throw of their dice moving: the land, rivers
/// and resources of a seeded Tennis Ball are where they were.
#[test]
fn a_world_of_civviss_own_takes_the_stock_cliffs() {
    let world = small_world(MapScript::TeninsBall, FLAT, 2_024);
    let mut cliffs = 0;
    for (pos, tile) in world.tiles.iter() {
        if is_water(tile) {
            continue;
        }
        for neighbor in world.neighbors(*pos) {
            if is_water(&world.tiles[&neighbor]) && world.has_cliff_edge(*pos, neighbor) {
                cliffs += 1;
                assert!(tile.hills, "a cliff on flat ground at {pos:?}");
            }
        }
    }
    assert!(cliffs > 0);
}

/// The relief of a finished Continents world against the game's own.
///
/// Sixty-four recorded Continents games on the Standard map show 8.2% of
/// revealed land as Mountains, 26.3% as Hills and one volcano to 198 land
/// plots (`docs/FIDELITY.md`). A generated world of that size has to come
/// out in the same country: the script's own ranges and lonely peaks, plus
/// what `AddTerrainFromContinents` raises along the seams.
#[test]
fn a_standard_continents_world_has_the_live_games_relief() {
    let rules = Rules::embedded();
    let worlds = 10;
    let (mut land, mut mountains, mut hills, mut volcanoes) = (0usize, 0usize, 0usize, 0usize);
    for seed in 0..worlds {
        let (world, _) = generate_with_script(
            &rules,
            84,
            54,
            8,
            12,
            5,
            4,
            MapScript::Continents,
            FLAT,
            POLED,
            &mut Rng::new(70_000 + seed),
        );
        for tile in world.tiles.values() {
            if is_water(tile) {
                continue;
            }
            land += 1;
            if tile.terrain == "mountain" {
                mountains += 1;
            }
            if tile.hills {
                hills += 1;
            }
            if tile.feature.as_deref() == Some("volcano") {
                volcanoes += 1;
            }
        }
    }
    let mountain_share = 100.0 * mountains as f64 / land as f64;
    let hill_share = 100.0 * hills as f64 / land as f64;
    let land_per_volcano = land as f64 / volcanoes.max(1) as f64;
    eprintln!(
        "Continents 84x54 over {worlds} worlds: {:.0} land plots a world, {mountain_share:.1}% \
         Mountains, {hill_share:.1}% Hills, {land_per_volcano:.0} land plots per volcano",
        land as f64 / worlds as f64
    );
    assert!(
        (6.5..=11.5).contains(&mountain_share),
        "{mountain_share:.1}% Mountains"
    );
    assert!(
        (22.0..=31.0).contains(&hill_share),
        "{hill_share:.1}% Hills"
    );
    assert!(
        (150.0..=270.0).contains(&land_per_volcano),
        "{land_per_volcano:.0} land plots per volcano"
    );
}
