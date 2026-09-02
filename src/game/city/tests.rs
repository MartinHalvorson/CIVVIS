use super::*;

fn settler_city() -> (Game, u32) {
    let mut game = Game::new_full(1, 24, 16, 91_986, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("the opening roster includes a Settler");
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    game.remove_unit(settler);
    let city_state = game.cities.get_mut(&city).unwrap();
    city_state.pop = 3;
    city_state.queue = vec![Item::Unit {
        unit: crate::name!("settler"),
    }];
    (game, city)
}

#[test]
fn a_settler_banks_only_the_post_population_drop_growth_cost() {
    let (mut game, city) = settler_city();
    let post_settler_growth = game.growth_cost(2);
    let consumption = 2.0 * game.cities[&city].pop as f64;

    // Even strong empire and local food preferences cannot bid past the
    // amount the smaller city can actually use after the Settler completes.
    game.players[0].citizen_food_bias = 4.0;
    game.players[0].city_directives.insert(
        city,
        CityDirective {
            emphasis: Yields {
                food: 4.0,
                ..Yields::default()
            },
            role: CityRole::Breadbasket,
            ..CityDirective::default()
        },
    );
    game.cities.get_mut(&city).unwrap().food = post_settler_growth - 0.25;

    let final_quarter = game.citizen_strategy(city);
    assert_eq!(final_quarter.focus, "expansion");
    assert_eq!(final_quarter.weights.food, 0.0);
    assert_eq!(final_quarter.food_target, consumption + 0.25);

    game.cities.get_mut(&city).unwrap().food = post_settler_growth;
    let funded = game.citizen_strategy(city);
    assert_eq!(funded.weights.food, 0.0);
    assert_eq!(
        funded.food_target, consumption,
        "once the post-drop growth is funded, only current nutrition remains"
    );

    game.cities.get_mut(&city).unwrap().food = post_settler_growth + 5.0;
    assert_eq!(
        game.citizen_strategy(city).food_target,
        consumption,
        "food already beyond the useful threshold must not raise the target"
    );
}

#[test]
fn settler_citizens_buy_the_food_budget_without_maximizing_food() {
    let (mut game, city) = settler_city();
    let center = game.cities[&city].pos;
    let jobs: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != center)
        .collect();
    assert!(jobs.len() >= 4);

    // Three pure-food jobs compete with one production job. Two food jobs
    // exactly cover the remaining budget; preferring the old food tier would
    // take all three and waste the third citizen's production.
    for position in &jobs {
        let tile = game.map.tiles.get_mut(position).unwrap();
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
    for position in &jobs[..3] {
        game.map.tiles.get_mut(position).unwrap().terrain = crate::name!("grassland");
    }
    game.map.tiles.get_mut(&jobs[0]).unwrap().improvement = Some(crate::name!("farm"));
    let production = jobs[3];
    let tile = game.map.tiles.get_mut(&production).unwrap();
    tile.hills = true;
    tile.improvement = Some(crate::name!("mine"));

    let post_settler_growth = game.growth_cost(2);
    game.cities.get_mut(&city).unwrap().food = post_settler_growth - 0.25;
    let plan = game.city_citizen_plan(city);

    assert!(plan.worked_tiles.contains(&production));
    assert_eq!(
        plan.worked_tiles
            .iter()
            .filter(|position| jobs[..3].contains(position))
            .count(),
        2,
        "only the two food jobs needed to satisfy the budget should be worked"
    );
    let collected_food = game.workable_tile_yields(center).food.max(2.0)
        + plan
            .worked_tiles
            .iter()
            .map(|position| game.workable_tile_yields(*position).food)
            .sum::<f64>();
    assert!(collected_food + 1e-9 >= plan.strategy.food_target);
}

#[test]
fn provision_settlers_keep_the_ordinary_growth_appetite() {
    let (mut game, city) = settler_city();
    game.turn = 10;
    game.players[0].governor_roster.insert(
        "magnus".to_string(),
        GovernorState {
            city: Some(city),
            assigned_turn: 0,
            disabled_until: 0,
            promotions: BTreeSet::from(["provision".to_string()]),
        },
    );
    game.sync_governor_cities(0);
    assert!(!game.settler_consumes_population(0, city));

    let strategy = game.citizen_strategy(city);
    assert!(strategy.weights.food > 0.0);
    assert!(strategy.food_target > 2.0 * game.cities[&city].pop as f64);
}
