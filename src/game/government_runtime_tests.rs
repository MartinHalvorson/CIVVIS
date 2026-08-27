use super::*;

fn one_city(seed: u64) -> (Game, u32) {
    let mut game = Game::new_full(1, 24, 16, seed, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    (game, city)
}

fn two_cities(seed: u64) -> (Game, u32, u32) {
    let mut game = Game::new_full(2, 26, 16, seed, 120, 0, false);
    let mut cities = Vec::new();
    for player in 0..2 {
        let settler = game
            .player_unit_ids(player)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        cities.push(game.found_city_for(player, game.units[&settler].pos, None));
    }
    (game, cities[0], cities[1])
}

fn install_alliance(game: &mut Game, first: usize, second: usize) {
    let alliance = AllianceState {
        kind: "economic".to_string(),
        points: 0.0,
        level: 1,
        ends: game.turn + 60,
    };
    game.players[first]
        .alliances
        .insert(second, alliance.clone());
    game.players[second].alliances.insert(first, alliance);
}

#[test]
fn market_economy_is_worth_what_the_far_city_owns() {
    let (mut game, home, abroad) = two_cities(88_206);
    game.routes.push(TradeRoute {
        origin: home,
        dest: abroad,
        owner: 0,
        ends: game.turn + 30,
    });
    let yields = |game: &Game| game.city_yields(home);
    let base = yields(&game);

    // Both cards ship as ADJUST_TRADE_ROUTE_YIELD_FOR_INTERNATIONAL, so a
    // route to another civilization earns them and a domestic one does not.
    game.players[0].policies = [crate::name!("trade_confederation")].into_iter().collect();
    assert_eq!(yields(&game).culture - base.culture, 1.0);
    assert_eq!(yields(&game).science - base.science, 1.0);

    game.players[0].policies = [crate::name!("market_economy")].into_iter().collect();
    assert_eq!(yields(&game).culture - base.culture, 2.0);
    assert_eq!(yields(&game).science - base.science, 2.0);

    // Its Gold is one per luxury and one per strategic resource the
    // DESTINATION owns, so it is worth nothing into a bare city and grows
    // with what the far city actually holds.
    let gold_for = |game: &mut Game, resources: &[&str]| {
        let tiles = game.cities[&abroad].owned_tiles.to_vec();
        for position in &tiles {
            game.map.tiles.get_mut(position).unwrap().resource = None;
        }
        for (position, resource) in tiles.iter().zip(resources) {
            game.map.tiles.get_mut(position).unwrap().resource = Some(Name::new(resource));
        }
        game.query_memo();
        game.city_yields(home).gold
    };
    let bare = gold_for(&mut game, &[]);
    assert_eq!(gold_for(&mut game, &["wine"]) - bare, 1.0, "one luxury");
    assert_eq!(
        gold_for(&mut game, &["wine", "iron"]) - bare,
        2.0,
        "plus one strategic"
    );
    assert_eq!(
        gold_for(&mut game, &["wine", "iron", "rice"]) - bare,
        2.0,
        "a bonus resource is worth nothing"
    );

    // A domestic route earns none of it.
    game.routes.clear();
    game.routes.push(TradeRoute {
        origin: home,
        dest: home,
        owner: 0,
        ends: game.turn + 30,
    });
    let domestic = yields(&game);
    game.players[0].policies.clear();
    assert_eq!(yields(&game).culture, domestic.culture);
    assert_eq!(yields(&game).science, domestic.science);
}

#[test]
fn the_unit_ladders_repeat_their_predecessors_eras_instead_of_succeeding_them() {
    let (mut game, city) = one_city(60_318);
    let pct = |game: &Game, unit: &str| {
        let item = Item::Unit {
            unit: Name::new(unit),
        };
        ((game.item_prod_mult(0, city, Some(&item)) - 1.0) * 100.0).round()
    };
    let card = |game: &mut Game, policy: &str| {
        game.players[0].policies = [Name::new(policy)].into_iter().collect();
    };

    // Each ADJUST_UNIT_TAG_ERA_PRODUCTION card ships one row per era, and
    // every card in a ladder REPEATS its predecessor's eras rather than
    // starting where that one stopped. Agoge covers Ancient-Classical
    // infantry; Feudal Contract covers those two AGAIN plus Medieval and
    // Renaissance, and Military First covers every era there is.
    card(&mut game, "agoge");
    assert_eq!(pct(&game, "warrior"), 50.0);
    assert_eq!(pct(&game, "man_at_arms"), 0.0);

    card(&mut game, "feudal_contract");
    assert_eq!(
        pct(&game, "warrior"),
        50.0,
        "Ancient melee is still covered"
    );
    assert_eq!(pct(&game, "musketman"), 50.0);
    assert_eq!(pct(&game, "line_infantry"), 0.0, "Industrial is past it");

    card(&mut game, "military_first");
    for unit in [
        "warrior",
        "man_at_arms",
        "line_infantry",
        "mechanized_infantry",
    ] {
        assert_eq!(pct(&game, unit), 50.0, "{unit}");
    }

    // The one hole Firaxis left: every infantry card after Agoge omits the
    // Classical row for ranged units, which is invisible unless a
    // civilization fields a Classical ranged unique.
    card(&mut game, "agoge");
    assert_eq!(pct(&game, "saka_horse_archer"), 50.0);
    for policy in ["feudal_contract", "grande_armee", "military_first"] {
        card(&mut game, policy);
        assert_eq!(pct(&game, "archer"), 50.0, "{policy} covers Ancient ranged");
        assert_eq!(
            pct(&game, "saka_horse_archer"),
            0.0,
            "{policy} skips Classical ranged"
        );
    }

    // The naval ladder runs the same way, and Press Gangs stops after the
    // Industrial era rather than covering every later hull.
    card(&mut game, "maritime_industries");
    assert_eq!(pct(&game, "galley"), 100.0);
    assert_eq!(pct(&game, "caravel"), 0.0);
    card(&mut game, "press_gangs");
    assert_eq!(pct(&game, "galley"), 100.0);
    assert_eq!(pct(&game, "ironclad"), 100.0);
    assert_eq!(pct(&game, "destroyer"), 0.0);

    // Strategic Air Force is one card covering two classes over different
    // windows: Information-era air, but Carriers from the Atomic era.
    card(&mut game, "strategic_air_force");
    assert_eq!(pct(&game, "fighter"), 0.0, "Atomic-era air is not covered");
    assert_eq!(pct(&game, "jet_fighter"), 50.0);
    assert_eq!(pct(&game, "jet_bomber"), 50.0);
    assert_eq!(pct(&game, "aircraft_carrier"), 50.0);
}

#[test]
fn the_wonder_cards_stop_at_the_era_their_arguments_name() {
    let (mut game, city) = one_city(33_812);
    let at = |game: &Game, wonder: &str| {
        let item = Item::Wonder {
            wonder: Name::new(wonder),
            pos: game.cities[&city].pos,
        };
        game.item_prod_mult(0, city, Some(&item))
    };
    // pyramids Ancient, colosseum Classical, hagia_sophia Medieval,
    // taj_mahal Renaissance, big_ben Industrial.
    let subjects = [
        "pyramids",
        "colosseum",
        "hagia_sophia",
        "taj_mahal",
        "big_ben",
    ];
    let base: Vec<f64> = subjects.iter().map(|w| at(&game, w)).collect();
    let gain = |game: &Game| -> Vec<f64> {
        subjects
            .iter()
            .enumerate()
            .map(|(i, w)| ((at(game, w) - base[i]) * 100.0).round())
            .collect()
    };

    // CORVEE_ANCIENTCLASSICALWONDER is StartEra ANCIENT, EndEra CLASSICAL.
    game.players[0].policies = [crate::name!("corvee")].into_iter().collect();
    assert_eq!(gain(&game), vec![15.0, 15.0, 0.0, 0.0, 0.0]);

    // GOTHICARCHITECTURE_MEDIEVALRENAISSANCEWONDER is named for a window it
    // does not have: its StartEra is ANCIENT, so it also pays the Ancient
    // and Classical wonders Corvee covered, through Renaissance.
    game.players[0].policies = [crate::name!("gothic_architecture")].into_iter().collect();
    assert_eq!(gain(&game), vec![15.0, 15.0, 15.0, 15.0, 0.0]);

    // SKYSCRAPERS_INDUSTRIALINFORMATION is likewise misnamed: ANCIENT to
    // FUTURE is every wonder in the game, so it stays ungated.
    game.players[0].policies = [crate::name!("skyscrapers")].into_iter().collect();
    assert_eq!(gain(&game), vec![15.0, 15.0, 15.0, 15.0, 15.0]);
}

#[test]
fn monarchy_pays_favor_for_each_walled_city_not_once() {
    let (mut game, home, _) = two_cities(70_441);
    game.players[0].government = Some("monarchy".to_string());
    let favor = |game: &mut Game| {
        game.players[0].diplomatic_favor = 0.0;
        game.process_diplomacy(0);
        game.players[0].diplomatic_favor
    };
    // Monarchy is Tier2, so the government itself pays 2 a turn.
    let bare = favor(&mut game);
    assert_eq!(bare, 2.0);

    // MONARCHY_STARFORT_FAVOR is a PLAYER_CITIES modifier gated on
    // BUILDING_STAR_FORT, which is Renaissance Walls. Earlier walls do not
    // qualify, and the bonus lands once per city that does.
    for building in ["walls", "medieval_walls"] {
        game.cities
            .get_mut(&home)
            .unwrap()
            .buildings
            .push(Name::new(building));
        assert_eq!(favor(&mut game), bare, "{building} is not a Star Fort");
    }
    game.cities
        .get_mut(&home)
        .unwrap()
        .buildings
        .push(crate::name!("renaissance_walls"));
    assert_eq!(favor(&mut game), bare + 2.0);
}

#[test]
fn wisselbanken_and_collectivization_pay_only_the_routes_they_name() {
    let (mut game, home, abroad) = two_cities(51_207);
    game.routes.push(TradeRoute {
        origin: home,
        dest: abroad,
        owner: 0,
        ends: game.turn + 30,
    });
    let food = |game: &Game| game.city_yields(home).food;
    let production = |game: &Game| game.city_yields(home).production;
    let (base_food, base_production) = (food(&game), production(&game));

    // WISSELBANKEN ships eight rows, and every one of them is
    // _FOR_ALLY_ROUTE or _FOR_SUZERAIN_ROUTE. A route to a civilization
    // that is neither pays nothing at all.
    game.players[0].policies = [crate::name!("wisselbanken")].into_iter().collect();
    assert_eq!(food(&game), base_food);
    assert_eq!(production(&game), base_production);

    install_alliance(&mut game, 0, 1);
    assert_eq!(food(&game), base_food + 2.0);
    assert_eq!(production(&game), base_production + 2.0);

    // COLLECTIVIZATION is ADJUST_TRADE_ROUTE_YIELD_FOR_DOMESTIC, so the
    // same allied foreign route it just paid for earns it nothing.
    game.players[0].policies = [crate::name!("collectivization")].into_iter().collect();
    assert_eq!(food(&game), base_food);
    assert_eq!(production(&game), base_production);

    game.routes.clear();
    game.routes.push(TradeRoute {
        origin: home,
        dest: home,
        owner: 0,
        ends: game.turn + 30,
    });
    let (domestic_food, domestic_production) = (food(&game), production(&game));
    game.players[0].policies.clear();
    assert_eq!(domestic_food - food(&game), 4.0);
    assert_eq!(domestic_production - production(&game), 2.0);
}

fn without_government(game: &Game) -> Game {
    let mut baseline = game.clone();
    baseline.players[0].government = None;
    baseline
}

fn assert_yield_delta(actual: Yields, baseline: Yields, expected: f64) {
    // These capitals sit in the Content amenity band, so only the tested
    // effect changes their yields.
    assert!((actual.food - baseline.food - expected).abs() < 1e-9);
    assert!((actual.production - baseline.production - expected).abs() < 1e-9);
    assert!((actual.gold - baseline.gold - expected).abs() < 1e-9);
    assert!((actual.science - baseline.science - expected).abs() < 1e-9);
    assert!((actual.culture - baseline.culture - expected).abs() < 1e-9);
    assert!((actual.faith - baseline.faith - expected).abs() < 1e-9);
}

#[test]
fn autocracy_counts_active_government_buildings_and_boosts_wonders() {
    let (mut game, city) = one_city(774_501);
    game.players[0].government = Some("autocracy".to_string());

    let baseline = without_government(&game);
    assert_yield_delta(game.city_yields(city), baseline.city_yields(city), 1.0);

    install_test_district(&mut game, city, "government_plaza");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("ancestral_hall"));
    let baseline = without_government(&game);
    assert_yield_delta(game.city_yields(city), baseline.city_yields(city), 2.0);

    let diplomatic_quarter = install_test_district(&mut game, city, "diplomatic_quarter");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("consulate"));
    let baseline = without_government(&game);
    assert_yield_delta(game.city_yields(city), baseline.city_yields(city), 3.0);

    game.cities
        .get_mut(&city)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("consulate"));
    let baseline = without_government(&game);
    assert_yield_delta(game.city_yields(city), baseline.city_yields(city), 2.0);
    game.cities
        .get_mut(&city)
        .unwrap()
        .pillaged_buildings
        .remove(&Name::new("consulate"));
    game.map
        .tiles
        .get_mut(&diplomatic_quarter)
        .unwrap()
        .pillaged = true;
    let baseline = without_government(&game);
    assert_yield_delta(game.city_yields(city), baseline.city_yields(city), 2.0);

    let wonder = Item::Wonder {
        wonder: crate::name!("pyramids"),
        pos: game.cities[&city].pos,
    };
    let baseline = without_government(&game);
    assert!(
        (game.item_prod_mult(0, city, Some(&wonder))
            - baseline.item_prod_mult(0, city, Some(&wonder))
            - 0.10)
            .abs()
            < 1e-9
    );
}

#[test]
fn classical_republic_requires_a_completed_district() {
    let (mut game, city) = one_city(774_502);
    game.players[0].government = Some("classical_republic".to_string());
    let baseline = without_government(&game);
    assert_eq!(
        game.city_housing(&game.cities[&city]),
        baseline.city_housing(&baseline.cities[&city])
    );
    assert_eq!(
        game.city_local_amenities(&game.cities[&city]),
        baseline.city_local_amenities(&baseline.cities[&city])
    );

    let district = install_test_district(&mut game, city, "campus");
    let baseline = without_government(&game);
    assert_eq!(
        game.city_housing(&game.cities[&city]),
        baseline.city_housing(&baseline.cities[&city]) + 1.0
    );
    assert_eq!(
        game.city_local_amenities(&game.cities[&city]),
        baseline.city_local_amenities(&baseline.cities[&city]) + 1
    );

    // A pillaged district still exists, so it continues satisfying the
    // government's city-level eligibility condition.
    game.map.tiles.get_mut(&district).unwrap().pillaged = true;
    let baseline = without_government(&game);
    assert_eq!(
        game.city_housing(&game.cities[&city]),
        baseline.city_housing(&baseline.cities[&city]) + 1.0
    );
}

#[test]
fn monarchy_counts_active_wall_levels_and_multiplies_influence() {
    let (mut game, city) = one_city(774_503);
    game.players[0].government = Some("monarchy".to_string());
    let baseline = without_government(&game);
    assert_eq!(
        game.city_housing(&game.cities[&city]),
        baseline.city_housing(&baseline.cities[&city])
    );

    game.cities.get_mut(&city).unwrap().buildings.extend([
        crate::name!("walls"),
        crate::name!("medieval_walls"),
        crate::name!("renaissance_walls"),
    ]);
    let baseline = without_government(&game);
    assert_eq!(
        game.city_housing(&game.cities[&city]),
        baseline.city_housing(&baseline.cities[&city]) + 3.0
    );
    game.cities
        .get_mut(&city)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("medieval_walls"));
    let baseline = without_government(&game);
    assert_eq!(
        game.city_housing(&game.cities[&city]),
        baseline.city_housing(&baseline.cities[&city]) + 2.0
    );

    let mut merchant = game.clone();
    merchant.players[0].government = Some("merchant_republic".to_string());
    game.players[0].influence = 0.0;
    merchant.players[0].influence = 0.0;
    game.begin_turn(0);
    merchant.begin_turn(0);
    assert!((game.players[0].influence - merchant.players[0].influence * 1.5).abs() < 1e-9);
}

#[test]
fn theocracy_faith_per_population_requires_a_governor() {
    let (mut game, city) = one_city(774_504);
    game.cities.get_mut(&city).unwrap().pop = 2;
    game.players[0].government = Some("theocracy".to_string());
    let baseline = without_government(&game);
    assert_eq!(
        game.city_yields(city).faith,
        baseline.city_yields(city).faith
    );

    game.players[0]
        .civics
        .insert(crate::name!("state_workforce"));
    game.do_appoint_governor(0, "pingala", city).unwrap();
    let baseline = without_government(&game);
    assert!((game.city_yields(city).faith - baseline.city_yields(city).faith - 1.0).abs() < 1e-9);
}

#[test]
fn communism_gates_population_production_but_multiplies_science_empire_wide() {
    let (mut game, city) = one_city(774_505);
    game.cities.get_mut(&city).unwrap().pop = 2;
    game.players[0].government = Some("communism".to_string());
    let baseline = without_government(&game);
    assert_eq!(
        game.city_yields(city).production,
        baseline.city_yields(city).production
    );
    assert!(
        (game.city_yields(city).science - baseline.city_yields(city).science * 1.10).abs() < 1e-9
    );

    game.players[0]
        .civics
        .insert(crate::name!("state_workforce"));
    game.do_appoint_governor(0, "pingala", city).unwrap();
    let baseline = without_government(&game);
    assert!(
        (game.city_yields(city).production - baseline.city_yields(city).production - 1.2).abs()
            < 1e-9
    );
}

#[test]
fn democracy_trade_bonus_requires_an_ally_or_suzerained_city_state() {
    let (mut game, origin, destination) = two_cities(774_506);
    game.players[0].government = Some("democracy".to_string());
    game.routes.push(TradeRoute {
        origin,
        dest: destination,
        owner: 0,
        ends: game.turn + 30,
    });
    let baseline = without_government(&game);
    assert_eq!(game.city_yields(origin), baseline.city_yields(origin));
    assert_eq!(
        game.city_yields(destination),
        baseline.city_yields(destination)
    );

    install_alliance(&mut game, 0, 1);
    let baseline = without_government(&game);
    let origin_yields = game.city_yields(origin);
    let origin_baseline = baseline.city_yields(origin);
    let destination_yields = game.city_yields(destination);
    let destination_baseline = baseline.city_yields(destination);
    assert_eq!(origin_yields.food, origin_baseline.food + 4.0);
    assert!((origin_yields.production - origin_baseline.production - 4.0).abs() < 1e-9);
    assert_eq!(destination_yields.food, destination_baseline.food + 4.0);
    assert!((destination_yields.production - destination_baseline.production - 4.0).abs() < 1e-9);

    game.players[0].alliances.clear();
    game.players[1].alliances.clear();
    game.players[1].is_minor = true;
    game.players[0].envoys.push((1, 3));
    let baseline = without_government(&game);
    assert!(
        (game.city_yields(origin).production - baseline.city_yields(origin).production - 4.0).abs()
            < 1e-9
    );
    assert!(
        (game.city_yields(destination).production
            - baseline.city_yields(destination).production
            - 4.0)
            .abs()
            < 1e-9
    );

    game.players[1].is_minor = false;
    install_alliance(&mut game, 0, 1);
    game.routes.clear();
    game.process_diplomacy(0);
    assert_eq!(game.players[0].alliances[&1].points, 1.25);
}

#[test]
fn democracy_discounts_gold_unit_building_and_district_purchases() {
    let (mut game, city) = one_city(774_507);
    vacate_land_combat_purchase_slot(&mut game, 0, city);
    game.players[0].government = Some("democracy".to_string());
    game.players[0].gold = 100_000.0;
    game.players[0].techs.insert(crate::name!("pottery"));
    game.players[0].techs.insert(crate::name!("writing"));

    let baseline = without_government(&game);
    assert!(
        (game
            .building_gold_purchase_cost(0, city, "granary")
            .unwrap()
            - baseline
                .building_gold_purchase_cost(0, city, "granary")
                .unwrap()
                * 0.85)
            .abs()
            < 1e-9
    );

    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    let unit_cost = game.item_cost_for(0, &warrior) * 4.0 * 0.85;
    let gold_before = game.players[0].gold;
    game.do_buy(0, city, "warrior", "gold").unwrap();
    assert!((gold_before - game.players[0].gold - unit_cost).abs() < 1e-9);

    game.turn = 10;
    game.players[0].governor_roster.insert(
        "reyna".to_string(),
        GovernorState {
            city: Some(city),
            assigned_turn: 0,
            disabled_until: 0,
            promotions: BTreeSet::from(["contractor".to_string()]),
        },
    );
    game.sync_governor_cities(0);
    let district_site = game.district_sites(city, crate::name!("campus"))[0];
    let district_cost = game.district_cost_for_placement(0, "campus", true) * 4.0 * 0.85;
    let gold_before = game.players[0].gold;
    game.do_buy_district(0, city, "campus", district_site, "gold")
        .unwrap();
    assert!((gold_before - game.players[0].gold - district_cost).abs() < 1e-9);
}

#[test]
fn fascism_applies_combat_production_bonuses_and_a_weariness_penalty() {
    let (mut game, city) = one_city(774_508);
    game.players[0].government = Some("fascism".to_string());
    let baseline = without_government(&game);
    for unit in ["warrior", "builder"] {
        let item = Item::Unit {
            unit: Name::new(unit),
        };
        assert!(
            (game.item_prod_mult(0, city, Some(&item))
                - baseline.item_prod_mult(0, city, Some(&item))
                - 0.50)
                .abs()
                < 1e-9
        );
    }
    let warrior = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "warrior")
        .unwrap();
    assert!(
        (game.unit_strength(&game.units[&warrior], false)
            - baseline.unit_strength(&baseline.units[&warrior], false)
            - 5.0)
            .abs()
            < 1e-9
    );
    // FASCISM_WAR_WEARINESS is +20, a penalty — see
    // fascism_endures_war_worse_than_every_other_government.
    assert_eq!(game.war_weariness_multiplier(0, false), 1.2);
    assert_eq!(game.war_weariness_multiplier(0, true), 1.2);
}

#[test]
fn corporate_libertarianism_gates_production_and_rewards_improved_resources() {
    let (mut game, city) = one_city(774_509);
    game.players[0].government = Some("corporate_libertarianism".to_string());
    let baseline = without_government(&game);
    assert_eq!(
        game.city_yields(city).production,
        baseline.city_yields(city).production
    );
    assert!(
        (game.city_yields(city).science - baseline.city_yields(city).science * 0.90).abs() < 1e-9
    );

    let commercial_hub = install_test_district(&mut game, city, "commercial_hub");
    let baseline = without_government(&game);
    assert!(
        (game.city_yields(city).production - baseline.city_yields(city).production * 1.10).abs()
            < 1e-9
    );
    install_test_district(&mut game, city, "encampment");
    let baseline = without_government(&game);
    assert!(
        (game.city_yields(city).production - baseline.city_yields(city).production * 1.10).abs()
            < 1e-9,
        "the two eligible districts grant one city modifier, not two"
    );
    game.map.tiles.get_mut(&commercial_hub).unwrap().pillaged = true;
    let encampment = game.cities[&city]
        .districts
        .iter()
        .find_map(|(district, position)| {
            game.district_is_family(district, crate::name!("encampment"))
                .then_some(*position)
        })
        .unwrap();
    game.map.tiles.get_mut(&encampment).unwrap().pillaged = true;
    let baseline = without_government(&game);
    assert_eq!(
        game.city_yields(city).production,
        baseline.city_yields(city).production
    );

    game.players[0].techs.insert(crate::name!("bronze_working"));
    let center = game.cities[&city].pos;
    game.map.tiles.get_mut(&center).unwrap().resource = Some(crate::name!("iron"));
    let resource_tile = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != center && game.map.tiles[position].district.is_none())
        .unwrap();
    let tile = game.map.tiles.get_mut(&resource_tile).unwrap();
    tile.resource = Some(crate::name!("iron"));
    tile.improvement = Some(crate::name!("mine"));
    tile.pillaged = false;
    let baseline = without_government(&game);
    assert_eq!(
        game.strategic_resource_rate(0, "iron"),
        baseline.strategic_resource_rate(0, "iron") + 1.0,
        "the improved source receives +1; the city-center source does not"
    );
    game.map.tiles.get_mut(&resource_tile).unwrap().pillaged = true;
    let baseline = without_government(&game);
    assert_eq!(
        game.strategic_resource_rate(0, "iron"),
        baseline.strategic_resource_rate(0, "iron")
    );
}

#[test]
fn digital_democracy_applies_city_bonuses_and_unit_penalty() {
    let (mut game, city) = one_city(774_510);
    game.players[0].government = Some("digital_democracy".to_string());
    let baseline = without_government(&game);
    assert_eq!(
        game.city_local_amenities(&game.cities[&city]),
        baseline.city_local_amenities(&baseline.cities[&city]) + 2
    );
    // The two government Amenities can lift the city a happiness band, so
    // compare the yields with each side's own band factored out.
    let band = |g: &Game| Game::amenity_yield_mult_for(g.city_amenity_surplus(&g.cities[&city]));
    assert!(
        (game.city_yields(city).culture / band(&game)
            - baseline.city_yields(city).culture / band(&baseline))
        .abs()
            < 1e-9
    );
    let warrior = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "warrior")
        .unwrap();
    assert!(
        (game.unit_strength(&game.units[&warrior], false)
            - baseline.unit_strength(&baseline.units[&warrior], false)
            + 3.0)
            .abs()
            < 1e-9
    );

    install_test_district(&mut game, city, "campus");
    let baseline = without_government(&game);
    assert!(
        (game.city_yields(city).culture / band(&game)
            - (baseline.city_yields(city).culture / band(&baseline) + 2.0))
            .abs()
            < 1e-9
    );
}

#[test]
fn synthetic_technocracy_powers_cities_boosts_projects_and_reduces_tourism() {
    let (mut game, city) = one_city(774_511);
    game.players[0].government = Some("synthetic_technocracy".to_string());
    install_test_district(&mut game, city, "campus");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("research_lab"));
    let baseline = without_government(&game);
    assert_eq!(game.city_power_demand(&game.cities[&city]), 3.0);
    assert_eq!(game.city_power_supply(&game.cities[&city]), 3.0);
    assert_eq!(baseline.city_power_supply(&baseline.cities[&city]), 0.0);
    assert!(game.city_is_powered(&game.cities[&city]));
    assert!(!baseline.city_is_powered(&baseline.cities[&city]));

    let project = Item::Project {
        project: crate::name!("campus_research_grants"),
    };
    assert!(
        (game.item_prod_mult(0, city, Some(&project))
            - baseline.item_prod_mult(0, city, Some(&project))
            - 0.30)
            .abs()
            < 1e-9
    );

    let city_position = game.cities[&city].pos;
    game.cities
        .get_mut(&city)
        .unwrap()
        .wonders
        .insert(crate::name!("pyramids"), city_position);
    let baseline = without_government(&game);
    assert!((game.tourism_per_turn(0) - baseline.tourism_per_turn(0) * 0.90).abs() < 1e-9);
}
