use super::*;

/// The six stock sizes (`Maps.xml`).
const SIZES: [(i32, i32); 6] = [(44, 26), (60, 38), (74, 46), (84, 54), (96, 60), (106, 66)];

fn world(script: StockScript, w: i32, h: i32, seed: u64) -> StockWorld {
    let grid = Grid {
        w,
        h,
        wrap_x: wraps_east_west(script),
    };
    generate(script, grid, false, &mut Rng::new(seed))
}

fn land_flags(world: &StockWorld) -> Vec<bool> {
    world
        .plot_types
        .iter()
        .map(|plot| *plot != PLOT_OCEAN)
        .collect()
}

/// Every landmass's size, biggest first.
fn landmasses(world: &StockWorld) -> Vec<i32> {
    let grid = world.grid;
    let land = land_flags(world);
    let mut seen = vec![false; land.len()];
    let mut sizes = Vec::new();
    for start in 0..land.len() {
        if !land[start] || seen[start] {
            continue;
        }
        seen[start] = true;
        let mut stack = vec![start as i32];
        let mut size = 0;
        while let Some(index) = stack.pop() {
            size += 1;
            let (x, y) = (index % grid.w, index / grid.w);
            for direction in 0..NUM_DIRECTIONS {
                if let Some((nx, ny)) = grid.adjacent(x, y, direction) {
                    let neighbor = (ny * grid.w + nx) as usize;
                    if land[neighbor] && !seen[neighbor] {
                        seen[neighbor] = true;
                        stack.push(neighbor as i32);
                    }
                }
            }
        }
        sizes.push(size);
    }
    sizes.sort_unstable_by(|a, b| b.cmp(a));
    sizes
}

fn land_share(world: &StockWorld) -> f64 {
    let land = land_flags(world).iter().filter(|land| **land).count();
    land as f64 / world.plot_types.len() as f64
}

/// The grid is the engine's: odd rows half a hex east, six neighbours, each
/// of them a neighbour back.
#[test]
fn adjacency_is_mutual_and_wraps_only_where_the_map_does() {
    let opposite = [SOUTHWEST, WEST, NORTHWEST, NORTHEAST, EAST, SOUTHEAST];
    for wrap_x in [true, false] {
        let grid = Grid {
            w: 10,
            h: 8,
            wrap_x,
        };
        for y in 0..grid.h {
            for x in 0..grid.w {
                for direction in 0..NUM_DIRECTIONS {
                    let Some((nx, ny)) = grid.adjacent(x, y, direction) else {
                        continue;
                    };
                    assert_eq!(
                        grid.adjacent(nx, ny, opposite[direction as usize]),
                        Some((x, y)),
                        "({x}, {y}) direction {direction}"
                    );
                }
            }
        }
        assert_eq!(grid.adjacent(9, 3, EAST).is_some(), wrap_x);
        assert_eq!(grid.adjacent(0, 3, WEST).is_some(), wrap_x);
        assert_eq!(grid.adjacent(4, 7, NORTHEAST), None);
    }
}

/// Continents is the script's promise in its own words: no landmass holds
/// more than 64% of the land when the fractal is accepted, and the centre
/// rift then parts whatever lies over the middle of the map. So a world has
/// at least two continents worth the name, with open sea between them.
#[test]
fn continents_has_two_or_more_continents() {
    for (w, h) in SIZES {
        let mut two_real_continents = 0;
        let worlds = 40;
        for seed in 0..worlds {
            let world = world(StockScript::Continents, w, h, seed);
            let masses = landmasses(&world);
            let land: i32 = masses.iter().sum();
            // The rift slides each side three plots toward the seam whatever
            // the map's width. On the narrow sizes that can close the seam's
            // own gap and join the two continents round the back of the
            // world: about one Duel world in eight, one Small world in a
            // hundred, and none from Standard up.
            assert!(
                w < 84 || f64::from(masses[0]) <= f64::from(land) * 0.80,
                "{w}x{h} seed {seed}: one landmass holds {} of {land} land plots",
                masses[0]
            );
            if masses.len() >= 2 && masses[1] * 100 >= land * 15 {
                two_real_continents += 1;
            }
        }
        let wanted = if w < 60 { 6 } else { 8 };
        assert!(
            two_real_continents * 10 >= worlds * wanted,
            "{w}x{h}: only {two_real_continents} of {worlds} worlds had a second continent \
             with 15% of the land"
        );
    }
}

/// `sea_level_normal` cuts the field at 62% water, and the polar rows, the
/// seam rift and the centre rift all take a little more: a Continents world
/// is about a third land on every size.
#[test]
fn continents_is_about_a_third_land() {
    for (w, h) in SIZES {
        let worlds = 40;
        let mean: f64 = (0..worlds)
            .map(|seed| land_share(&world(StockScript::Continents, w, h, seed)))
            .sum::<f64>()
            / worlds as f64;
        assert!(
            (0.29..=0.41).contains(&mean),
            "{w}x{h}: mean land share {mean:.3}"
        );
    }
}

/// "y <= iBuffer or y >= g_iH - iBuffer - 1" is always Ocean: Continents
/// keeps a band of open water at the top and bottom of the map. The plot
/// table is read one cell along by the passes that follow, so the band is
/// checked a row in from where it is cut.
#[test]
fn continents_keeps_its_polar_rows_at_sea() {
    for seed in 0..20 {
        let world = world(StockScript::Continents, 74, 46, seed);
        for x in 0..74 {
            for y in [0, 1, 44, 45] {
                assert_eq!(
                    world.plot(x, y),
                    PLOT_OCEAN,
                    "seed {seed}: land at ({x}, {y})"
                );
            }
        }
    }
}

/// Pangaea is accepted only when one landmass holds 84% of the land.
#[test]
fn pangaea_is_one_landmass() {
    for (w, h) in SIZES {
        for seed in 0..25 {
            let world = world(StockScript::Pangaea, w, h, seed);
            let masses = landmasses(&world);
            let land: i32 = masses.iter().sum();
            assert!(
                masses[0] * 100 >= land * 80,
                "{w}x{h} seed {seed}: the biggest landmass holds {} of {land}",
                masses[0]
            );
        }
    }
}

/// Small Continents is accepted only when no landmass holds more than 34%
/// of the main layer, and then two layers of islands are laid over it clear
/// of the land already there.
#[test]
fn small_continents_is_many_landmasses_none_dominant() {
    for (w, h) in &SIZES[1..] {
        let worlds = 25;
        let mut many = 0;
        for seed in 0..worlds {
            let world = world(StockScript::SmallContinents, *w, *h, seed);
            let masses = landmasses(&world);
            let land: i32 = masses.iter().sum();
            assert!(
                masses[0] * 100 <= land * 60,
                "{w}x{h} seed {seed}: one landmass holds {} of {land}",
                masses[0]
            );
            if masses.iter().filter(|size| **size >= 8).count() >= 6 {
                many += 1;
            }
        }
        assert!(
            many * 10 >= worlds * 8,
            "{w}x{h}: only {many} of {worlds} worlds had six landmasses of eight plots"
        );
    }
}

/// Island Plates is islands: a third of the world land, in many pieces.
#[test]
fn island_plates_is_many_islands() {
    for (w, h) in &SIZES[1..] {
        let worlds = 25;
        let mut pieces = 0;
        let mut share = 0.0;
        for seed in 0..worlds {
            let world = world(StockScript::IslandPlates, *w, *h, seed);
            pieces += landmasses(&world).iter().filter(|size| **size >= 8).count();
            share += land_share(&world);
        }
        let mean_pieces = pieces as f64 / worlds as f64;
        let mean_share = share / worlds as f64;
        assert!(
            mean_pieces >= 7.0,
            "{w}x{h}: {mean_pieces:.1} islands a world"
        );
        assert!(
            (0.24..=0.40).contains(&mean_share),
            "{w}x{h}: mean land share {mean_share:.3}"
        );
    }
}

/// Inland Sea: land on every edge of a map that does not wrap, and a sea in
/// the middle of it, which the oval guarantees.
#[test]
fn inland_sea_is_one_sea_ringed_by_land() {
    for (w, h) in SIZES {
        for seed in 0..25 {
            let world = world(StockScript::InlandSea, w, h, seed);
            assert!(!world.grid.wrap_x);
            for x in 1..w {
                assert_ne!(
                    world.plot(x, 0),
                    PLOT_OCEAN,
                    "seed {seed}: sea at the south"
                );
                assert_ne!(
                    world.plot(x, h - 1),
                    PLOT_OCEAN,
                    "seed {seed}: sea at the north"
                );
            }
            for y in 0..h {
                assert_ne!(world.plot(1, y), PLOT_OCEAN, "seed {seed}: sea at the west");
                assert_ne!(
                    world.plot(w - 1, y),
                    PLOT_OCEAN,
                    "seed {seed}: sea at the east"
                );
            }
            assert_eq!(
                world.plot(w / 2, h / 2),
                PLOT_OCEAN,
                "seed {seed}: dry middle"
            );
            // The fractal sea can leave an islet or two standing in it.
            let masses = landmasses(&world);
            let land: i32 = masses.iter().sum();
            assert!(
                masses[0] * 100 >= land * 97,
                "seed {seed}: the ring holds {} of {land} land plots",
                masses[0]
            );
        }
    }
}

/// Lakes cuts its layers the other way: four fifths of the world and more
/// is dry, and the water is basins scattered through it.
#[test]
fn lakes_is_mostly_land() {
    for (w, h) in SIZES {
        let worlds = 25;
        let mean: f64 = (0..worlds)
            .map(|seed| land_share(&world(StockScript::Lakes, w, h, seed)))
            .sum::<f64>()
            / worlds as f64;
        assert!(mean >= 0.78, "{w}x{h}: mean land share {mean:.3}");
    }
}

/// Latitude bands at Standard temperature: Snow from 0.8 and Tundra from
/// 0.65, each roughened by at most a tenth either way, Desert only between
/// 0.2 and 0.5, and nothing but Grass under 0.1.
#[test]
fn climate_follows_the_latitude_bands() {
    for script in [
        StockScript::Continents,
        StockScript::Pangaea,
        StockScript::Lakes,
    ] {
        for seed in 0..10 {
            let world = world(script, 84, 54, seed);
            for y in 0..54 {
                let lat = (27.0 - f64::from(y)).abs() / 27.0;
                for x in 0..84 {
                    if world.plot(x, y) == PLOT_OCEAN {
                        assert!(matches!(
                            world.climate_at(x, y),
                            Climate::Ocean | Climate::Coast
                        ));
                        continue;
                    }
                    let climate = world.climate_at(x, y);
                    assert!(
                        !matches!(climate, Climate::Ocean | Climate::Coast),
                        "{script:?} seed {seed}: land painted as water at ({x}, {y})"
                    );
                    // The variation field moves a plot's latitude by at most
                    // 128 / 1275 either way.
                    let slack = 0.101;
                    match climate {
                        Climate::Snow => assert!(lat >= 0.8 - slack),
                        Climate::Tundra => assert!((0.65 - slack..0.8 + slack).contains(&lat)),
                        Climate::Desert => assert!((0.2 - slack..0.5 + slack).contains(&lat)),
                        _ => assert!(lat < 0.65 + slack),
                    }
                    if lat < 0.1 - slack {
                        assert_eq!(climate, Climate::Grass);
                    }
                }
            }
        }
    }
}

/// Water beside land is always Coast, and the expansion passes only ever
/// add Coast beside Coast: no Ocean plot touches land.
#[test]
fn no_deep_ocean_touches_land() {
    for script in [
        StockScript::Continents,
        StockScript::SmallContinents,
        StockScript::IslandPlates,
        StockScript::InlandSea,
    ] {
        let world = world(script, 74, 46, 5);
        let grid = world.grid;
        for y in 0..grid.h {
            for x in 0..grid.w {
                if world.climate_at(x, y) != Climate::Ocean {
                    continue;
                }
                for direction in 0..NUM_DIRECTIONS {
                    if let Some((nx, ny)) = grid.adjacent(x, y, direction) {
                        assert_eq!(
                            world.plot(nx, ny),
                            PLOT_OCEAN,
                            "{script:?}: deep ocean at ({x}, {y}) beside land"
                        );
                    }
                }
            }
        }
    }
}

/// `AddLonelyMountains` holds a world to its script's ratio of mountains to
/// land, and the coastal pass keeps nine in ten of them off the shore.
#[test]
fn mountains_keep_to_the_scripts_ratio_and_off_the_coast() {
    let worlds = 30;
    let (mut land, mut mountains, mut coastal) = (0, 0, 0);
    for seed in 0..worlds {
        let world = world(StockScript::Continents, 74, 46, seed);
        let grid = world.grid;
        for y in 0..grid.h {
            for x in 0..grid.w {
                let plot = world.plot(x, y);
                if plot == PLOT_OCEAN {
                    continue;
                }
                land += 1;
                if plot != PLOT_MOUNTAIN {
                    continue;
                }
                mountains += 1;
                let on_coast = (0..NUM_DIRECTIONS).any(|direction| {
                    grid.adjacent(x, y, direction)
                        .is_some_and(|(nx, ny)| world.plot(nx, ny) == PLOT_OCEAN)
                });
                if on_coast {
                    coastal += 1;
                }
            }
        }
    }
    // One in seventeen at the least (`8 + world_age * 3`).
    assert!(
        mountains * 17 >= land * 9 / 10,
        "{mountains} mountains on {land} land plots"
    );
    assert!(
        mountains * 8 <= land,
        "{mountains} mountains on {land} land"
    );
    assert!(
        coastal * 4 <= mountains,
        "{coastal} of {mountains} mountains stand on the coast"
    );
}

/// A seed is a world: nothing in a script reads a float it could round two
/// ways, a hash order or the clock.
#[test]
fn the_same_seed_draws_the_same_world() {
    for script in [
        StockScript::Continents,
        StockScript::Pangaea,
        StockScript::SmallContinents,
        StockScript::IslandPlates,
        StockScript::InlandSea,
        StockScript::Lakes,
    ] {
        let first = world(script, 74, 46, 77);
        let second = world(script, 74, 46, 77);
        assert_eq!(first.plot_types, second.plot_types, "{script:?}");
        assert_eq!(first.climate, second.climate, "{script:?}");
        let other = world(script, 74, 46, 78);
        assert_ne!(first.plot_types, other.plot_types, "{script:?}");
    }
}

/// The scripts are written for the stock sizes, but a board far under them
/// still has to come back with a world rather than a panic or a hang.
#[test]
fn a_board_under_the_stock_sizes_still_gets_a_world() {
    for script in [
        StockScript::Continents,
        StockScript::Pangaea,
        StockScript::SmallContinents,
        StockScript::IslandPlates,
        StockScript::InlandSea,
        StockScript::Lakes,
    ] {
        for (w, h) in [
            (8, 6),
            (10, 10),
            (12, 8),
            (20, 12),
            (24, 16),
            (30, 24),
            (40, 18),
        ] {
            for seed in 0..6 {
                let world = world(script, w, h, seed);
                assert_eq!(world.plot_types.len(), (w * h) as usize);
            }
        }
    }
}
