//! Compare the bounded governor with its original exhaustive swap scan.
use super::*;

struct ReferenceMode(bool);

impl Drop for ReferenceMode {
    fn drop(&mut self) {
        CITIZEN_REFERENCE_IMPROVEMENTS.with(|mode| mode.set(self.0));
    }
}

fn plan(
    game: &Game,
    city: u32,
    weights: Option<Yields>,
    reference: bool,
) -> (CitizenPlan, u64, u64) {
    let _restore =
        ReferenceMode(CITIZEN_REFERENCE_IMPROVEMENTS.with(|mode| mode.replace(reference)));
    CITIZEN_IMPROVEMENT_PAIRS.with(|count| count.set(0));
    CITIZEN_BOUNDED_PLANS.with(|count| count.set(0));
    let result = game.city_citizen_plan_weighted(city, weights);
    (
        result,
        CITIZEN_IMPROVEMENT_PAIRS.with(std::cell::Cell::get),
        CITIZEN_BOUNDED_PLANS.with(std::cell::Cell::get),
    )
}

fn city() -> (Game, u32, Vec<Pos>) {
    let mut game = Game::new_full(1, 32, 20, 92_214, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|id| game.units[id].kind == "settler")
        .unwrap();
    let center = game.units[&settler].pos;
    let city = game.found_city_for(0, center, None);
    game.remove_unit(settler);
    let positions = game.wdisk(center, 2);
    game.cities.get_mut(&city).unwrap().owned_tiles = positions.clone();
    game.cities.get_mut(&city).unwrap().queue.clear();
    for position in &positions {
        let tile = game.map.tiles.get_mut(position).unwrap();
        tile.owner_city = Some(city);
        tile.terrain = crate::name!("desert");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
        tile.pillaged = false;
    }
    game.board_written();
    (
        game,
        city,
        positions.into_iter().filter(|pos| *pos != center).collect(),
    )
}

fn compare(game: &Game, city: u32, weights: Option<Yields>) -> (u64, u64) {
    let (expected, exhaustive_pairs, reference_bounded) = plan(game, city, weights, true);
    let (actual, bounded_pairs, optimized_bounded) = plan(game, city, weights, false);
    assert_eq!(reference_bounded, 0);
    assert_eq!(optimized_bounded, 1);
    assert_eq!(actual, expected, "weights={weights:?}");
    assert!(bounded_pairs <= exhaustive_pairs);
    (bounded_pairs, exhaustive_pairs)
}

#[test]
fn an_already_ordered_assignment_needs_no_improvement_pairs() {
    let (mut game, city, _) = city();
    game.cities.get_mut(&city).unwrap().pop = 3;
    let (bounded, exhaustive) = compare(&game, city, None);
    assert!(exhaustive > 0, "the original pass checked unselected jobs");
    assert_eq!(bounded, 0, "no later zero-value job can improve the prefix");
}

#[test]
fn nutrition_weights_populations_and_locked_tiles_match_exhaustive_assignments() {
    let (mut game, city, jobs) = city();
    let terrains = [
        crate::name!("desert"),
        crate::name!("grassland"),
        crate::name!("plains"),
        crate::name!("tundra"),
    ];
    let weights = [
        None,
        Some(Yields {
            food: 0.0,
            production: 2.2,
            gold: 0.7,
            science: 1.2,
            culture: 1.2,
            faith: 0.4,
        }),
        Some(Yields {
            food: 1.0,
            production: 1.0 + 5e-10,
            gold: 1.0,
            science: 1.0,
            culture: 1.0,
            faith: 1.0,
        }),
        Some(Yields {
            food: -0.5,
            production: 3.0,
            gold: 0.0,
            science: 2.0,
            culture: -0.25,
            faith: 0.0,
        }),
    ];
    for pattern in 0..8 {
        for (index, pos) in jobs.iter().enumerate() {
            let tile = game.map.tiles.get_mut(pos).unwrap();
            tile.terrain = terrains[(index + pattern) % terrains.len()];
            tile.hills = (index + pattern) % 3 == 0;
            tile.feature = ((index + pattern) % 5 == 0).then_some(crate::name!("forest"));
            tile.improvement = match (index + pattern) % 7 {
                0 => Some(crate::name!("farm")),
                1 => Some(crate::name!("mine")),
                _ => None,
            };
        }
        for population in [0, 1, 3, 6, 12, 30] {
            game.cities.get_mut(&city).unwrap().pop = population;
            game.cities.get_mut(&city).unwrap().queue = if pattern % 2 == 0 {
                vec![Item::Unit {
                    unit: crate::name!("settler"),
                }]
            } else {
                Vec::new()
            };
            for locked in [false, true] {
                game.players[0].citizen_locks.clear();
                if locked {
                    game.players[0]
                        .citizen_locks
                        .insert(city, [jobs[2], *jobs.last().unwrap()].into_iter().collect());
                }
                game.board_written();
                for weights in weights {
                    compare(&game, city, weights);
                }
            }
        }
    }
}

#[test]
fn specialist_slots_and_forced_specialists_keep_the_same_assignments() {
    let (mut game, city, jobs) = city();
    let site = jobs[0];
    game.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("campus"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), site);
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("library"));
    assert!(!game.city_specialist_jobs(&game.cities[&city]).is_empty());
    for population in [1, 3, 8, 30] {
        game.cities.get_mut(&city).unwrap().pop = population;
        for locked in [false, true] {
            game.players[0].specialist_locks.clear();
            if locked {
                game.players[0]
                    .specialist_locks
                    .entry(city)
                    .or_default()
                    .insert("campus".to_owned(), 1);
            }
            game.board_written();
            compare(&game, city, None);
            compare(
                &game,
                city,
                Some(Yields {
                    science: 10.0,
                    food: 0.0,
                    ..Yields::default()
                }),
            );
        }
    }
}

#[test]
fn nonfinite_values_retain_the_original_exhaustive_arithmetic() {
    let (mut game, city, jobs) = city();
    game.cities.get_mut(&city).unwrap().pop = 1;
    for pos in jobs {
        let tile = game.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.hills = true;
    }
    game.board_written();
    let weights = Some(Yields {
        food: 1.0,
        production: f64::INFINITY,
        gold: 1.0,
        science: 1.0,
        culture: 1.0,
        faith: 1.0,
    });
    let (expected, exhaustive_pairs, _) = plan(&game, city, weights, true);
    let (actual, fallback_pairs, bounded) = plan(&game, city, weights, false);
    assert_eq!(actual, expected);
    assert_eq!(fallback_pairs, exhaustive_pairs);
    assert!(fallback_pairs > 0);
    assert_eq!(bounded, 0);
}
