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

/// `Player::citizens_grow_into_the_cap` (`gene_screen --seat-host-citizens`):
/// at the housing cap the citizen strategy ordinarily trades Food away (the
/// Food weight times 0.55, no growth surplus); with the flag the city keeps a
/// growth surplus and its Food weight, as the host's governor plays the live
/// seat. With room to grow the flag changes nothing.
#[test]
fn a_capped_city_keeps_its_food_only_when_it_grows_into_the_cap() {
    let (mut game, city) = settler_city();
    game.cities.get_mut(&city).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("warrior"),
    }];
    let housing = game.city_housing(&game.cities[&city]);
    game.cities.get_mut(&city).unwrap().pop = housing.floor() as i32;
    let pop = f64::from(game.cities[&city].pop);
    assert!(housing - pop <= 1.0, "fixture: at the cap");
    assert!(game.city_amenity_surplus(&game.cities[&city]) >= -2);

    let off = game.citizen_strategy(city);
    assert_eq!(
        off.food_target,
        2.0 * pop,
        "off: no growth surplus at the cap"
    );
    game.players[0].citizens_grow_into_the_cap = true;
    let on = game.citizen_strategy(city);
    assert_eq!(
        on.food_target,
        2.0 * pop + 1.0,
        "on: the cap keeps a surplus"
    );
    assert!(
        (on.weights.food * 0.55 - off.weights.food).abs() < 1e-9,
        "on: the Food weight is not halved ({} against {})",
        on.weights.food,
        off.weights.food
    );

    // Room to grow: identical either way.
    game.cities.get_mut(&city).unwrap().pop = 1;
    let roomy_on = game.citizen_strategy(city);
    game.players[0].citizens_grow_into_the_cap = false;
    let roomy_off = game.citizen_strategy(city);
    assert_eq!(roomy_on.food_target, roomy_off.food_target);
    assert_eq!(roomy_on.weights.food, roomy_off.weights.food);
}

/// The flag defaults off, and a saved player written before it existed loads
/// with it off.
#[test]
fn citizens_grow_into_the_cap_defaults_off_and_old_saves_load_off() {
    let (game, _) = settler_city();
    let player = game.players[0].clone();
    assert!(!player.citizens_grow_into_the_cap);
    let mut value = serde_json::to_value(&player).expect("a Player serializes");
    let object = value.as_object_mut().expect("a Player is an object");
    assert!(object.remove("citizens_grow_into_the_cap").is_some());
    let old: Player = serde_json::from_value(value).expect("an old save loads");
    assert!(!old.citizens_grow_into_the_cap);

    let mut on = player.clone();
    on.citizens_grow_into_the_cap = true;
    let round: Player =
        serde_json::from_value(serde_json::to_value(&on).unwrap()).expect("round trip");
    assert!(round.citizens_grow_into_the_cap);
}
