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

/// `CitizenJobKey` replaced the formatted text the citizen plan broke ties on.
/// Its derived order must be that text's order, or a tie resolves differently.
/// Civilization VI lets a player lock a citizen to a tile. The governor seats
/// that citizen first and places everyone else around it, and no later pass
/// moves it, including the one that covers consumption.
#[test]
fn a_citizen_placed_by_hand_stays_where_it_was_put() {
    let (mut game, city) = settler_city();
    game.cities.get_mut(&city).unwrap().queue.clear();
    let before = game.city_citizen_plan(city);
    assert_eq!(before.worked_tiles.len(), 3);
    // A tile the governor did not choose, and would not: the poorest it owns.
    let center = game.cities[&city].pos;
    let mut spare: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|pos| *pos != center && !before.worked_tiles.contains(pos))
        .filter(|pos| {
            let tile = &game.map.tiles[pos];
            tile.district.is_none() && tile.wonder.is_none()
        })
        .collect();
    spare.sort_by(|a, b| {
        let value = |pos: &Pos| game.workable_tile_yields(*pos).total();
        value(a).partial_cmp(&value(b)).unwrap().then(a.cmp(b))
    });
    let placed = *spare.first().expect("a tile the governor left unworked");

    // Nothing placed is the shipped plan, to the tile.
    game.players[0].citizen_locks.insert(city, BTreeSet::new());
    assert_eq!(game.city_citizen_plan(city), before);

    game.players[0]
        .citizen_locks
        .entry(city)
        .or_default()
        .insert(placed);
    game.board_written();
    let held = game.city_citizen_plan(city);
    assert!(held.worked_tiles.contains(&placed), "{held:?}");
    assert_eq!(held.worked_tiles.len(), before.worked_tiles.len());

    // Favouring anything else does not pull the placed citizen off its tile.
    game.players[0].city_directives.insert(
        city,
        CityDirective {
            emphasis: Yields {
                food: 4.0,
                production: 4.0,
                ..Yields::default()
            },
            ..CityDirective::default()
        },
    );
    game.board_written();
    assert!(game.city_citizen_plan(city).worked_tiles.contains(&placed));
    game.players[0].city_directives.clear();

    // More placed than there are citizens: the first few are seated and the
    // governor has nobody left to place.
    let all: BTreeSet<Pos> = spare
        .iter()
        .copied()
        .chain(before.worked_tiles.iter().copied())
        .collect();
    assert!(all.len() > 3);
    game.players[0].citizen_locks.insert(city, all.clone());
    game.board_written();
    let full = game.city_citizen_plan(city);
    assert_eq!(full.worked_tiles.len(), 3);
    assert!(full.worked_tiles.iter().all(|pos| all.contains(pos)));

    // Taken off again, the governor's own plan comes back exactly.
    game.players[0].citizen_locks.remove(&city);
    game.board_written();
    assert_eq!(game.city_citizen_plan(city), before);
}

/// A campus with a library on land the test city owns, for the two tests
/// below: one specialist slot the governor would not fill by itself.
fn campus_city() -> (Game, u32, Pos) {
    let (mut game, city) = settler_city();
    let center = game.cities[&city].pos;
    let site = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != center)
        .expect("land beside the city");
    game.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("campus"));
    let held = game.cities.get_mut(&city).unwrap();
    held.queue.clear();
    held.districts.insert(crate::name!("campus"), site);
    held.buildings.push(crate::name!("library"));
    game.board_written();
    (game, city, site)
}

/// Civilization VI lets a player seat a specialist in a district. The governor
/// leaves a slot empty while there is land to work; a specialist placed by
/// hand takes the slot and keeps it.
#[test]
fn a_specialist_placed_by_hand_takes_its_slot() {
    let (mut game, city, site) = campus_city();
    let slots = game.city_specialist_slots(city);
    assert_eq!(slots.len(), 1, "{slots:?}");
    assert_eq!((slots[0].0.as_str(), slots[0].1), ("campus", site));
    assert!(slots[0].2 >= 1);

    let before = game.city_citizen_plan(city);
    assert!(before.specialists.is_empty(), "{before:?}");
    game.players[0]
        .specialist_locks
        .entry(city)
        .or_default()
        .insert("campus".to_string(), 1);
    game.board_written();
    let held = game.city_citizen_plan(city);
    assert_eq!(held.specialists, vec!["campus".to_string()]);
    assert_eq!(held.worked_tiles.len() + held.specialists.len(), 3);

    // Released, the governor's own plan comes back exactly.
    game.players[0].specialist_locks.clear();
    game.board_written();
    assert_eq!(game.city_citizen_plan(city), before);
}

/// An agent rebuilds every city directive on the turns it plays. A focus a
/// person set used to live there and was erased by handing the seat to
/// auto-play; it is the seat's own now.
#[test]
fn a_seats_focus_outlives_the_directives_an_agent_rewrites() {
    let (mut game, city, _) = campus_city();
    let before = game.citizen_strategy(city).weights;
    game.players[0].citizen_focus.insert(
        city,
        Yields {
            gold: 4.0,
            ..Yields::default()
        },
    );
    assert_eq!(game.citizen_strategy(city).weights.gold, before.gold + 4.0);
    game.players[0].city_directives.clear();
    game.players[0]
        .city_directives
        .insert(city, CityDirective::default());
    game.board_written();
    assert_eq!(game.citizen_strategy(city).weights.gold, before.gold + 4.0);
    assert_eq!(game.citizen_strategy(city).weights.food, before.food);
}

#[test]
fn citizen_job_keys_order_exactly_as_their_text_did() {
    let values = [
        -99_999, -1_000, -257, -100, -12, -10, -9, -3, -1, 0, 1, 2, 9, 10, 11, 99, 100, 256, 1_000,
        99_999,
    ];
    let mut keys: Vec<(String, CitizenJobKey)> = Vec::new();
    for x in values {
        for y in values {
            keys.push((format!("tile:{x:+06}:{y:+06}"), CitizenJobKey::tile((x, y))));
        }
    }
    for (index, district) in ["campus", "commercial_hub", "holy_site", "theater_square"]
        .into_iter()
        .enumerate()
    {
        let text = format!("specialist:{district}:{index:03}");
        keys.push((text.clone(), CitizenJobKey::Specialist(text)));
    }
    for (left_text, left_key) in &keys {
        for (right_text, right_key) in &keys {
            assert_eq!(
                left_key.cmp(right_key),
                left_text.cmp(right_text),
                "{left_text} vs {right_text}"
            );
        }
    }
}

/// A tile can still name a city that has been removed; the founding check
/// treats that city as owning nothing instead of panicking on the lookup.
#[test]
fn founding_on_a_tile_whose_owner_city_is_gone_does_not_panic() {
    let mut g = Game::new_full(2, 24, 16, 7_961, 300, 0, false);
    let settler = g
        .player_unit_ids(1)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .expect("major 1 starts with a settler");
    let pos = g.units[&settler].pos;
    let cid = g.found_city_for(1, pos, None);
    g.remove_unit(settler);
    let owned = g
        .map
        .disk(pos, 1)
        .into_iter()
        .find(|tile| {
            *tile != pos
                && g.map.get(*tile).is_some_and(|t| {
                    t.owner_city == Some(cid) && g.rules.is_passable(t) && !g.rules.is_water(t)
                })
        })
        .expect("a land tile the city owns");
    g.cities.remove(&cid);
    assert_eq!(
        g.map.get(owned).unwrap().owner_city,
        Some(cid),
        "the stale owner stays"
    );
    let ours = g.spawn_unit("settler", 0, owned);
    let _ = g.can_found_city(ours);
}
