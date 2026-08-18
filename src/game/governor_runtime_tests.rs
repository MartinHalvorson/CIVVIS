use super::*;

fn appoint_established(
    game: &mut Game,
    pid: usize,
    governor: &str,
    city: u32,
    promotions: &[&str],
) {
    game.turn = 10;
    game.players[pid].governor_roster.insert(
        governor.to_string(),
        GovernorState {
            city: Some(city),
            assigned_turn: 0,
            disabled_until: 0,
            promotions: promotions
                .iter()
                .map(|promotion| promotion.to_string())
                .collect(),
        },
    );
    game.sync_governor_cities(pid);
    assert!(game.governor_established(pid, governor));
}

fn found_capital(game: &mut Game, pid: usize) -> u32 {
    let settler = game
        .player_unit_ids(pid)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let city = game.found_city_for(pid, game.units[&settler].pos, None);
    game.remove_unit(settler);
    city
}

fn set_district(game: &mut Game, city: u32, position: Pos, district: &str) {
    let tile = game.map.tiles.get_mut(&position).unwrap();
    tile.district = Some(Name::new(district));
    tile.improvement = None;
    tile.pillaged = false;
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(Name::new(district), position);
}

/// The interface shows a district's adjacency as a ledger, so every point
/// a district earns has to be attributable to a named source — and the
/// lines have to add up to what `district_yields` actually pays.
#[test]
fn adjacency_sources_account_for_every_point_a_district_earns() {
    let mut game = Game::new_full(2, 28, 18, 91_779, 200, 0, false);
    let capital = found_capital(&mut game, 0);
    let center = game.cities[&capital].pos;
    let site = game
        .nbrs(center)
        .into_iter()
        .find(|position| game.map.get(*position).is_some())
        .unwrap();
    // Flatten the ring around the site so the only sources left are the
    // ones this test puts there.
    let ring: Vec<Pos> = game
        .nbrs(site)
        .into_iter()
        .filter(|position| *position != center && game.map.get(*position).is_some())
        .collect();
    for position in &ring {
        let tile = game.map.tiles.get_mut(position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
    }
    for position in ring.iter().take(2) {
        game.map.tiles.get_mut(position).unwrap().terrain = crate::name!("mountain");
    }
    set_district(&mut game, capital, site, "campus");

    let sources = game.district_adjacency_sources(crate::name!("campus"), site);
    let mountain = sources
        .iter()
        .find(|source| source.source == "mountain")
        .expect("mountains pay the Campus and say so");
    assert_eq!(mountain.count, 2);
    assert_eq!(mountain.yields.science, 2.0);
    // The city center is a district, and one district is only half a
    // point. The ledger reports the banked half, which is what tells a
    // player the next adjacent district is worth a whole one.
    let district = sources
        .iter()
        .find(|source| source.source == "district")
        .expect("the city center counts as an adjacent district");
    assert_eq!(district.count, 1);
    assert_eq!(district.yields.science, 0.0);
    assert_eq!(district.raw.science, 0.5);

    // A second adjacent district completes the pair — and a PILLAGED one
    // does not count: live Rome's Campus lost its district point the turn
    // after its Holy Site was pillaged and got it back on repair (run
    // civvis-20260816T200454Z, t82-96).
    let holy_site = ring[2];
    set_district(&mut game, capital, holy_site, "holy_site");
    let paired = game.district_adjacency_sources(crate::name!("campus"), site);
    let district = paired.iter().find(|source| source.source == "district").unwrap();
    assert_eq!((district.count, district.yields.science), (2, 1.0));
    game.map.tiles.get_mut(&holy_site).unwrap().pillaged = true;
    let broken = game.district_adjacency_sources(crate::name!("campus"), site);
    let district = broken.iter().find(|source| source.source == "district").unwrap();
    assert_eq!((district.count, district.yields.science), (1, 0.0), "a pillaged district is not adjacent");
    game.map.tiles.get_mut(&holy_site).unwrap().pillaged = false;
    game.map.tiles.get_mut(&holy_site).unwrap().district = None;
    game.cities.get_mut(&capital).unwrap().districts.remove(&crate::name!("holy_site"));
    let sources = game.district_adjacency_sources(crate::name!("campus"), site);

    let sum = |sources: &[AdjacencySource]| {
        let mut total = Yields::default();
        for source in sources {
            total.add(source.yields);
        }
        total
    };
    let base = game.rules.districts["campus"].yields;
    assert_eq!(
        sum(&sources).science,
        game.district_yields(crate::name!("campus"), site).science - base.science
    );

    // A doubling policy card is a line of its own rather than a silent
    // change to the tiles' figures.
    game.players[0]
        .policies
        .insert(crate::name!("natural_philosophy"));
    let carded = game.district_adjacency_sources(crate::name!("campus"), site);
    let bonus = carded
        .iter()
        .find(|source| source.source == "adjacency_bonus")
        .expect("the policy card is itemized");
    assert_eq!(bonus.percent, 100.0);
    assert_eq!(bonus.yields.science, 2.0);
    assert_eq!(
        sum(&carded).science,
        game.district_yields(crate::name!("campus"), site).science - base.science
    );
}

#[test]
fn losing_a_governors_city_clears_live_and_legacy_assignments() {
    let mut game = Game::new_full(2, 28, 18, 91_779, 200, 0, false);
    let capital = found_capital(&mut game, 0);
    found_capital(&mut game, 1);
    let capital_pos = game.cities[&capital].pos;
    let second_pos = game
        .map
        .tiles
        .iter()
        .find_map(|(position, tile)| {
            (tile.owner_city.is_none()
                && game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.wdist(*position, capital_pos) >= 4)
                .then_some(*position)
        })
        .unwrap();
    let second = game.found_city_for(0, second_pos, Some("Governor Test".to_string()));
    appoint_established(&mut game, 0, "victor", second, &["garrison_commander"]);

    let mut captured = game.clone();
    captured.capture_city(second, 1);
    assert_eq!(
        captured.players[0].governor_roster["victor"].city,
        None,
        "a live ownership change displaces the Governor immediately"
    );

    // A checkpoint written by an older runtime can still contain the
    // assignment after the city was razed. Recreate that stale shape and
    // run the exact Loyalty path that used to index the missing city.
    let removed = game.cities.remove(&second).unwrap();
    game.city_by_pos.remove(&removed.pos);
    for position in removed.owned_tiles {
        if game.map.tiles[&position].owner_city == Some(second) {
            game.map.tiles.get_mut(&position).unwrap().owner_city = None;
        }
    }
    assert!(!game.governor_established(0, "victor"));
    game.process_loyalty(0);
    assert!(game.cities.contains_key(&capital));
}

#[test]
fn amani_executes_messenger_resources_puppeteer_and_emissary() {
    let mut game = Game::new_full(2, 26, 16, 91_780, 200, 1, false);
    found_capital(&mut game, 0);
    found_capital(&mut game, 1);
    let minor = game
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_barbarian)
        .unwrap()
        .id;
    let city_state = game.player_city_ids(minor)[0];
    appoint_established(
        &mut game,
        0,
        "amani",
        city_state,
        &["affluence", "foreign_investor", "emissary"],
    );

    assert_eq!(game.raw_envoys_at(0, minor), 0);
    assert_eq!(game.envoys_at(0, minor), 2);
    assert_eq!(game.suzerain_of(minor), None);

    game.players[0].techs = game.rules.techs.keys().cloned().collect();
    let resource_tiles: Vec<Pos> = game.cities[&city_state]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != game.cities[&city_state].pos)
        .take(2)
        .collect();
    assert_eq!(resource_tiles.len(), 2);
    for (position, terrain, resource, improvement) in [
        (resource_tiles[0], "plains", "silk", "plantation"),
        (resource_tiles[1], "coast", "oil", "offshore_oil_rig"),
    ] {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = Name::new(terrain);
        tile.feature = None;
        tile.resource = Some(Name::new(resource));
        tile.improvement = Some(Name::new(improvement));
        tile.pillaged = false;
    }
    assert_eq!(game.resource_access_count(0, "silk"), 1);
    assert_eq!(game.strategic_resource_rate(0, "oil"), 3.0);

    game.players[0].envoys.push((minor, 1));
    game.players[0]
        .governor_roster
        .get_mut("amani")
        .unwrap()
        .promotions
        .insert("puppeteer".to_string());
    assert_eq!(game.envoys_at(0, minor), 6);
    assert_eq!(game.suzerain_of(minor), Some(0));
    assert_eq!(game.strategic_resource_rate(0, "oil"), 6.0);

    let amani_position = game.cities[&city_state].pos;
    let target_position = game
        .wdisk(amani_position, 2)
        .into_iter()
        .find(|position| {
            game.map.tiles.contains_key(position)
                && game.map.tiles[position].owner_city.is_none()
                && game.rules.is_passable(&game.map.tiles[position])
                && !game.rules.is_water(&game.map.tiles[position])
        })
        .unwrap();
    let target = game.found_city_for(1, target_position, Some("Emissary Target".to_string()));
    game.cities.get_mut(&target).unwrap().loyalty = 50.0;
    let encoded = serde_json::to_string(&game).unwrap();
    let mut with_emissary: Game = serde_json::from_str(&encoded).unwrap();
    let mut without_emissary: Game = serde_json::from_str(&encoded).unwrap();
    without_emissary.players[0]
        .governor_roster
        .get_mut("amani")
        .unwrap()
        .promotions
        .remove("emissary");
    with_emissary.process_loyalty(1);
    without_emissary.process_loyalty(1);
    assert_eq!(
        with_emissary.cities[&target].loyalty - without_emissary.cities[&target].loyalty,
        -2.0
    );
}

#[test]
fn liang_executes_district_fisheries_parks_and_water_works() {
    let mut game = Game::new_full(1, 24, 16, 91_781, 200, 0, false);
    let city = found_capital(&mut game, 0);
    // Fisheries carry the shipped Sailing prerequisite on top of Liang's
    // Aquaculture promotion, City Parks the Games and Recreation civic.
    game.players[0].techs.insert(crate::name!("sailing"));
    game.players[0].civics.insert(crate::name!("games_recreation"));
    appoint_established(
        &mut game,
        0,
        "liang",
        city,
        &["zoning_commissioner", "aquaculture", "parks_and_recreation"],
    );
    let district = Item::District {
        district: crate::name!("campus"),
        pos: game.cities[&city].owned_tiles[1],
    };
    assert_eq!(game.item_prod_mult(0, city, Some(&district)), 1.2);

    let center = game.cities[&city].pos;
    let mut owned: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != center)
        .collect();
    owned.sort();
    assert!(owned.len() >= 6);
    let water = owned[0];
    let park = owned
        .iter()
        .copied()
        .find(|position| *position != water && game.nbrs(*position).contains(&water))
        .unwrap();
    {
        let tile = game.map.tiles.get_mut(&water).unwrap();
        tile.terrain = crate::name!("coast");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.hills = false;
    }
    let sea_resource = game
        .nbrs(water)
        .into_iter()
        .find(|position| *position != park && game.map.tiles.contains_key(position))
        .unwrap();
    {
        let tile = game.map.tiles.get_mut(&sea_resource).unwrap();
        tile.terrain = crate::name!("coast");
        tile.feature = None;
        tile.resource = Some(crate::name!("fish"));
    }
    assert!(game
        .valid_improvements(0, water)
        .contains(&crate::name!("fishery")));
    let bare_water = game.player_tile_yields(0, water, &game.map.tiles[&water]);
    game.map.tiles.get_mut(&water).unwrap().improvement = Some(crate::name!("fishery"));
    let fishery = game.player_tile_yields(0, water, &game.map.tiles[&water]);
    assert_eq!(fishery.food - bare_water.food, 2.0);
    assert_eq!(fishery.production - bare_water.production, 1.0);

    {
        let tile = game.map.tiles.get_mut(&park).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.hills = false;
    }
    assert!(game
        .valid_improvements(0, park)
        .contains(&crate::name!("city_park")));
    let bare_park = game.player_tile_yields(0, park, &game.map.tiles[&park]);
    let center_appeal = game.tile_appeal(center);
    let amenities = game.city_local_amenities(&game.cities[&city]);
    game.map.tiles.get_mut(&park).unwrap().improvement = Some(crate::name!("city_park"));
    let developed_park = game.player_tile_yields(0, park, &game.map.tiles[&park]);
    assert_eq!(developed_park.culture - bare_park.culture, 3.0);
    assert_eq!(game.tile_appeal(center) - center_appeal, 2);
    assert_eq!(
        game.city_local_amenities(&game.cities[&city]) - amenities,
        1
    );

    let district_positions: Vec<Pos> = owned
        .into_iter()
        .filter(|position| *position != water && *position != park)
        .take(4)
        .collect();
    assert_eq!(district_positions.len(), 4);
    for (position, district) in
        district_positions
            .into_iter()
            .zip(["neighborhood", "aqueduct", "canal", "dam"])
    {
        set_district(&mut game, city, position, district);
    }
    let housing_before = game.city_housing(&game.cities[&city]);
    let amenities_before = game.city_local_amenities(&game.cities[&city]);
    game.players[0]
        .governor_roster
        .get_mut("liang")
        .unwrap()
        .promotions
        .insert("water_works".to_string());
    assert_eq!(game.city_housing(&game.cities[&city]) - housing_before, 4.0);
    assert_eq!(
        game.city_local_amenities(&game.cities[&city]) - amenities_before,
        2
    );
}

#[test]
fn magnus_executes_harvest_logistics_resources_power_and_integration() {
    let mut game = Game::new_full(1, 26, 16, 91_782, 200, 0, false);
    let city = found_capital(&mut game, 0);
    appoint_established(
        &mut game,
        0,
        "magnus",
        city,
        &[
            "surplus_logistics",
            "provision",
            "industrialist",
            "black_marketeer",
            "vertical_integration",
        ],
    );

    game.players[0].techs.insert(crate::name!("mining"));
    let harvest = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| {
            *position != game.cities[&city].pos && game.units_at(*position).is_empty()
        })
        .unwrap();
    {
        let tile = game.map.tiles.get_mut(&harvest).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = Some(crate::name!("forest"));
        tile.resource = None;
        tile.improvement = None;
    }
    let builder = game.spawn_test_unit("builder", 0, harvest);
    let production = game.cities[&city].production;
    game.do_improve(0, builder, "chop_woods").unwrap();
    // 20 shipped base x 1.5 Black Marketeer at the Ancient era.
    assert_eq!(game.cities[&city].production - production, 30.0);

    game.players[0].techs.insert(crate::name!("iron_working"));
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 4.0);
    let swordsman = Item::Unit {
        unit: crate::name!("swordsman"),
    };
    assert_eq!(game.unit_resource_cost(city, &swordsman), 4.0);
    assert!(game.commit_unit_resource(0, city, &swordsman));
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 0.0);

    let center = game.cities[&city].pos;
    let industrial = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != center && *position != harvest)
        .unwrap();
    set_district(&mut game, city, industrial, "industrial_zone");
    game.cities.get_mut(&city).unwrap().buildings.extend([
        crate::name!("factory"),
        crate::name!("oil_power_plant"),
        crate::name!("research_lab"),
    ]);
    game.players[0]
        .strategic_resources
        .insert(crate::name!("oil"), 1.0);
    game.process_power(0);
    assert_eq!(game.city_power_supply(&game.cities[&city]), 5.0);
    assert_eq!(game.players[0].power_fuel_consumed["oil"], 1.0);

    let mut without_industrialist = game.clone();
    without_industrialist.players[0]
        .governor_roster
        .get_mut("magnus")
        .unwrap()
        .promotions
        .remove("industrialist");
    assert!(
        (game.city_yields(city).production
            - without_industrialist.city_yields(city).production
            - 2.0)
            .abs()
            < 1e-9
    );

    let source_position = game
        .wdisk(center, 5)
        .into_iter()
        .filter(|position| game.wdist(*position, center) >= 4)
        .find(|position| {
            game.map.get(*position).is_some_and(|tile| {
                tile.owner_city.is_none()
                    && game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
            })
        })
        .unwrap();
    let source = game.found_city_for(0, source_position, Some("Integration".to_string()));
    game.routes.push(TradeRoute {
        origin: source,
        dest: city,
        owner: 0,
        ends: game.turn + 30,
    });
    let mut without_logistics = game.clone();
    without_logistics.players[0]
        .governor_roster
        .get_mut("magnus")
        .unwrap()
        .promotions
        .remove("surplus_logistics");
    assert_eq!(
        game.city_yields(source).food - without_logistics.city_yields(source).food,
        2.0
    );

    let source_industrial = game.cities[&source]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != source_position)
        .unwrap();
    set_district(&mut game, source, source_industrial, "industrial_zone");
    game.cities
        .get_mut(&source)
        .unwrap()
        .buildings
        .extend([crate::name!("factory"), crate::name!("nuclear_power_plant")]);
    let mut without_integration = game.clone();
    without_integration.players[0]
        .governor_roster
        .get_mut("magnus")
        .unwrap()
        .promotions
        .remove("vertical_integration");
    assert!(
        (game.city_yields(city).production
            - without_integration.city_yields(city).production
            - 6.0)
            .abs()
            < 1e-9
    );
}

#[test]
fn moksha_executes_pressure_combat_healing_faith_patronage_and_districts() {
    let mut game = Game::new_full(2, 28, 18, 91_783, 200, 0, false);
    let city = found_capital(&mut game, 0);
    game.players[0].religion = Some("Our Faith".to_string());
    game.players[1].religion = Some("Other Faith".to_string());
    appoint_established(
        &mut game,
        0,
        "moksha",
        city,
        &[
            "grand_inquisitor",
            "laying_on_of_hands",
            "citadel_of_god",
            "patron_saint",
            "divine_architect",
        ],
    );

    let center = game.cities[&city].pos;
    let rival_position = game
        .wdisk(center, 5)
        .into_iter()
        .filter(|position| game.wdist(*position, center) >= 4)
        .find(|position| {
            game.map.get(*position).is_some_and(|tile| {
                tile.owner_city.is_none()
                    && game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
            })
        })
        .unwrap();
    let rival = game.found_city_for(1, rival_position, Some("Pressure Target".to_string()));
    game.cities.get_mut(&city).unwrap().pressure =
        BTreeMap::from([("Our Faith".to_string(), 100.0)]);
    game.cities.get_mut(&rival).unwrap().pressure =
        BTreeMap::from([("Other Faith".to_string(), 100.0)]);

    let mut ordinary_source = game.clone();
    ordinary_source.players[0].governor_roster.clear();
    game.process_pressure(1);
    ordinary_source.process_pressure(1);
    assert_eq!(
        game.cities[&rival].pressure["Our Faith"]
            - ordinary_source.cities[&rival].pressure["Our Faith"],
        1.0
    );

    let foreign_before = game.cities[&city]
        .pressure
        .get("Other Faith")
        .copied()
        .unwrap_or(0.0);
    game.process_pressure(0);
    assert_eq!(
        game.cities[&city]
            .pressure
            .get("Other Faith")
            .copied()
            .unwrap_or(0.0),
        foreign_before
    );
    let missionary = game.spawn_test_unit("missionary", 1, center);
    let spread_before = game.cities[&city].pressure.clone();
    game.do_spread(1, missionary).unwrap();
    assert_eq!(game.cities[&city].pressure, spread_before);

    let campus = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != center)
        .unwrap();
    set_district(&mut game, city, campus, "campus");
    let mut without_moksha = game.clone();
    without_moksha.players[0].governor_roster.clear();
    assert!(
        (game.city_yields(city).faith - without_moksha.city_yields(city).faith - 2.0)
            .abs()
            < 1e-9
    );

    let inquisitor = game.spawn_test_unit("inquisitor", 0, center);
    game.units.get_mut(&inquisitor).unwrap().religion = Some("Our Faith".to_string());
    let mut without_inquisitor = game.clone();
    without_inquisitor.players[0]
        .governor_roster
        .get_mut("moksha")
        .unwrap()
        .promotions
        .remove("grand_inquisitor");
    assert_eq!(
        game.theological_strength(&game.units[&inquisitor])
            - without_inquisitor.theological_strength(&without_inquisitor.units[&inquisitor]),
        10.0
    );
    let warrior = game.spawn_test_unit("warrior", 0, center);
    game.units.get_mut(&warrior).unwrap().hp = 10;
    assert_eq!(game.unit_heal_rate(warrior), 100);

    let faith = game.players[0].faith;
    let granary_cost = game.rules.buildings["granary"].cost;
    assert!(game.complete_item(
        0,
        city,
        &Item::Building {
            building: crate::name!("granary"),
        },
    ));
    assert_eq!(game.players[0].faith - faith, granary_cost * 0.25);

    let apostle = game.place_new_unit("apostle", 0, center).unwrap();
    game.apply_training_district_effects(city, apostle);
    assert!(game.units[&apostle].extra_first_promotion);
    let first = game.available_promotions(apostle)[0];
    game.do_promote(0, apostle, &first).unwrap();
    game.units.get_mut(&apostle).unwrap().moves_left = 4.0;
    assert!(game.promotion_pending(apostle));
    let second = game.available_promotions(apostle)[0];
    game.do_promote(0, apostle, &second).unwrap();
    assert_eq!(game.units[&apostle].promotions.len(), 2);

    game.players[0].techs = game.rules.techs.keys().cloned().collect();
    game.players[0].civics = game.rules.civics.keys().cloned().collect();
    game.cities.get_mut(&city).unwrap().pop = 4;
    game.players[0].faith = 10_000.0;
    let industrial = game.district_sites(city, crate::name!("industrial_zone"))[0];
    let district = Item::District {
        district: crate::name!("industrial_zone"),
        pos: industrial,
    };
    let cost = game.item_cost_for_city(0, city, &district) * 4.0;
    let faith = game.players[0].faith;
    game.do_buy_district(0, city, "industrial_zone", industrial, "faith")
        .unwrap();
    assert_eq!(game.players[0].faith, faith - cost);
    assert!(game.city_has_district_family(&game.cities[&city], crate::name!("industrial_zone")));
}

#[test]
fn late_great_person_cards_pay_their_shipped_amounts_per_building() {
    // Laissez-Faire, Nobel Prize and Military Organization each combine a
    // flat empire grant with a different amount per building tier; a
    // single number per card is not what the game ships.
    let mut game = Game::new_full(1, 24, 16, 91_921, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let center = game.cities[&city].pos;
    let sites: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != center)
        .collect();
    // Buildings only pay out while their district stands.
    for (index, district) in ["commercial_hub", "harbor", "campus", "industrial_zone", "encampment"]
        .into_iter()
        .enumerate()
    {
        set_district(&mut game, city, sites[index], district);
    }
    for building in [
        "bank",
        "stock_exchange",
        "seaport",
        "shipyard",
        "university",
        "research_lab",
        "factory",
        "coal_power_plant",
        "armory",
        "military_academy",
    ] {
        game.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(Name::new(building));
    }
    // Districts and buildings pay their own Great Person points, so
    // measure what each card adds on top rather than the total.
    let earned = |game: &mut Game, card: &str, kind: &str| {
        let mut collect = |policies: BTreeSet<Name>| {
            game.players[0].policies = policies;
            game.players[0].gpp.clear();
            game.process_great_people(0);
            game.players[0].gpp.get(kind).copied().unwrap_or(0.0)
        };
        let baseline = collect(BTreeSet::new());
        collect([Name::new(card)].into_iter().collect()) - baseline
    };
    // None of these three cards grants a flat, empire-wide point. Every
    // modifier they carry is a MODIFIER_PLAYER_CITIES_ADJUST_GREAT_PERSON_POINT
    // behind a CITY_HAS_BUILDING requirement — the whole chain from each
    // policy was walked to be sure, including chained ATTACH_MODIFIER.
    // Inspiration, Strategos and Revelation are the cards that really are
    // flat, and they ship as flat.
    // 2 from the Bank and 4 from the Stock Exchange.
    assert_eq!(earned(&mut game, "laissez_faire", "merchant"), 6.0);
    // 4 from the Seaport and 2 from the Shipyard.
    assert_eq!(earned(&mut game, "laissez_faire", "admiral"), 6.0);
    // 2 from the University and 4 from the Research Lab.
    assert_eq!(earned(&mut game, "nobel_prize", "scientist"), 6.0);
    // 2 from the Factory and 4 from the Coal Power Plant.
    assert_eq!(earned(&mut game, "nobel_prize", "engineer"), 6.0);
    // 2 from the Armory and 4 from the Military Academy.
    assert_eq!(earned(&mut game, "military_organization", "general"), 6.0);
}

/// Civilization VI turns the points of a Great Person class the empire
/// can no longer earn into Faith, one for one, each turn — the game core's
/// `GetFaithFromUnusedGreatPeoplePoints`. Measured on the live Settler
/// seat across seven games (run civvis-20260816T123936Z t219–239: the
/// balance grew by the Campus rate to the point once the last Great
/// Scientist anywhere was claimed, and by the Holy Site's Prophet rate
/// from the turn the map ran out of religions). The model banked the
/// points and paid nothing, so the empire's Faith read half the host's.
#[test]
fn unused_great_person_points_are_paid_out_as_faith() {
    let mut game = Game::new_full(1, 24, 16, 91_921, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let center = game.cities[&city].pos;
    let sites: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != center)
        .collect();
    set_district(&mut game, city, sites[0], "campus");
    set_district(&mut game, city, sites[1], "holy_site");
    let rate = game.great_person_points_per_turn(0);
    let scientist = rate["scientist"];
    let prophet = rate["prophet"];
    assert!(scientist > 0.0 && prophet > 0.0, "the two districts pay points: {rate:?}");

    // Every class still has someone to recruit: points bank, no Faith.
    assert!(game.great_person_class_earnable(0, "scientist"));
    assert!(game.great_person_class_earnable(0, "prophet"));
    assert_eq!(game.unused_great_person_faith(0), 0.0);
    let faith_before = game.players[0].faith;
    game.process_great_people(0);
    assert_eq!(game.players[0].faith, faith_before);
    assert_eq!(game.players[0].gpp["scientist"], scientist);

    // The host's timeline no longer lists a Great Scientist: the Campus
    // points are still counted (Firaxis's `GetPointsTotal` keeps growing)
    // and are paid out as Faith as well.
    game.players[0].live_great_person_exhausted =
        Some(["scientist".to_string()].into_iter().collect());
    assert!(!game.great_person_class_earnable(0, "scientist"));
    assert!(game.great_person_class_earnable(0, "prophet"));
    assert_eq!(game.unused_great_person_faith(0), scientist);
    assert_eq!(game.player_yield_extras(0).faith, scientist);
    let faith_before = game.players[0].faith;
    game.process_great_people(0);
    assert_eq!(game.players[0].faith, faith_before + scientist);
    assert_eq!(game.players[0].gpp["scientist"], 2.0 * scientist);

    // An empire that holds a religion cannot earn another Prophet: the
    // Holy Site's points become Faith too, whatever the host still offers.
    game.players[0].religion = Some("test_faith".to_string());
    assert!(!game.great_person_class_earnable(0, "prophet"));
    assert_eq!(game.unused_great_person_faith(0), scientist + prophet);
    game.players[0].religion = None;
    // ...and so is one whose Prophet is already pending.
    game.players[0].prophet_pending = true;
    assert!(!game.great_person_class_earnable(0, "prophet"));
    game.players[0].prophet_pending = false;
    assert!(game.great_person_class_earnable(0, "prophet"));

    // Nothing is paid under Anarchy: the empire collects no Faith at all.
    game.players[0].anarchy_turns = 2;
    assert_eq!(game.unused_great_person_faith(0), 0.0);
    let faith_before = game.players[0].faith;
    game.process_great_people(0);
    assert_eq!(game.players[0].faith, faith_before);
}

/// The turn processor is a separate claim path from the action menu. A
/// mirrored board must keep a locally ready native person pending until
/// Firaxis actually puts that class on its Great People screen, otherwise
/// the reconstruction retires a person the host never granted.
#[test]
fn live_offer_list_blocks_automatic_native_great_person_claims() {
    let mut game = Game::new_full(1, 24, 16, 91_923, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let center = game.cities[&city].pos;
    let campus = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != center)
        .expect("capital owns a Campus site");
    set_district(&mut game, city, campus, "campus");
    let cost = game.gp_cost(0, "scientist");
    game.players[0].gpp.insert("scientist".to_string(), cost);
    game.players[0].live_great_person_offers =
        Some(["merchant".to_string()].into_iter().collect());

    game.process_great_people(0);
    assert_eq!(
        game.players[0].gp_claimed.get("scientist").copied().unwrap_or(0),
        0,
        "a ready local Scientist remains pending while the host offers Merchant"
    );
    assert!(!game.retired_great_people.contains("hypatia"));

    game.players[0].live_great_person_offers =
        Some(["scientist".to_string()].into_iter().collect());
    game.process_great_people(0);
    assert_eq!(game.players[0].gp_claimed.get("scientist").copied(), Some(1));
}

#[test]
fn pingala_executes_population_gpp_space_and_curator_effects() {
    let mut game = Game::new_full(1, 24, 16, 91_784, 200, 0, false);
    let city = found_capital(&mut game, 0);
    appoint_established(
        &mut game,
        0,
        "pingala",
        city,
        &[
            "librarian",
            "connoisseur",
            "researcher",
            "grants",
            "space_initiative",
            "curator",
        ],
    );
    game.cities.get_mut(&city).unwrap().pop = 4;
    let center = game.cities[&city].pos;
    let sites: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != center)
        .take(2)
        .collect();
    set_district(&mut game, city, sites[0], "campus");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .extend([crate::name!("library"), crate::name!("amphitheater")]);

    let mut without_population = game.clone();
    for promotion in ["connoisseur", "researcher"] {
        without_population.players[0]
            .governor_roster
            .get_mut("pingala")
            .unwrap()
            .promotions
            .remove(promotion);
    }
    let with_yields = game.city_yields(city);
    let without_yields = without_population.city_yields(city);
    assert_eq!(game.governor_effect(0, city, "science_per_pop"), 1.0);
    assert_eq!(game.governor_effect(0, city, "culture_per_pop"), 1.0);
    assert!(with_yields.science > without_yields.science);
    assert!(with_yields.culture > without_yields.culture);

    let mut without_grants = game.clone();
    without_grants.players[0]
        .governor_roster
        .get_mut("pingala")
        .unwrap()
        .promotions
        .remove("grants");
    game.process_great_people(0);
    without_grants.process_great_people(0);
    assert_eq!(
        game.players[0].gpp["scientist"],
        2.0 * without_grants.players[0].gpp["scientist"]
    );

    set_district(&mut game, city, sites[1], "spaceport");
    let project = Item::Project {
        project: crate::name!("launch_earth_satellite"),
    };
    let mut without_space = game.clone();
    without_space.players[0]
        .governor_roster
        .get_mut("pingala")
        .unwrap()
        .promotions
        .remove("space_initiative");
    assert!(
        (game.item_prod_mult(0, city, Some(&project))
            - without_space.item_prod_mult(0, city, Some(&project))
            - 0.3)
            .abs()
            < 1e-9
    );

    game.players[0]
        .counters
        .insert("great_work:writing".to_string(), 1);
    let mut without_curator = game.clone();
    without_curator.players[0]
        .governor_roster
        .get_mut("pingala")
        .unwrap()
        .promotions
        .remove("curator");
    assert!(
        (game.tourism_per_turn(0) - without_curator.tourism_per_turn(0) - 2.0).abs() < 1e-9
    );
}

#[test]
fn the_chariot_line_rolls_faster_on_clear_ground() {
    let mut game = Game::new_full(1, 24, 16, 91_995, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let site = game.nbrs(game.cities[&city].pos)[0];
    {
        let tile = game.map.tiles.get_mut(&site).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
    }
    let chariot = game.spawn_unit("heavy_chariot", 0, site);
    let base = game.rules.units["heavy_chariot"].moves;
    assert_eq!(game.unit_max_moves(chariot), base + 1.0);

    // Woods, Rainforest and Hills all stop it.
    game.map.tiles.get_mut(&site).unwrap().feature = Some(crate::name!("forest"));
    assert_eq!(game.unit_max_moves(chariot), base);
    game.map.tiles.get_mut(&site).unwrap().feature = None;
    game.map.tiles.get_mut(&site).unwrap().hills = true;
    assert_eq!(game.unit_max_moves(chariot), base);
}

#[test]
fn rationalism_pays_in_halves_not_a_flat_double() {
    let mut game = Game::new_full(1, 24, 16, 91_989, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let site = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    set_district(&mut game, city, site, "campus");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("library"));
    game.cities.get_mut(&city).unwrap().pop = 4;
    let plain = game.city_yields(city).science;
    game.players[0].policies = [crate::name!("rationalism")].into_iter().collect();

    // A small city with a low-adjacency Campus gets nothing at all. Under
    // the old flat +100% the Library's 2 Science doubled here.
    assert_eq!(game.city_yields(city).science, plain);
    // Nor does doubling that low adjacency with Natural Philosophy reach
    // the clause: REQUIREMENT_CITY_HAS_HIGH_ADJACENCY_DISTRICT reads the
    // district's own adjacency, before the percentage cards (live Ostia's
    // Campus at "+6" — 3 doubled — earned nothing from Rationalism, run
    // civvis-20260816T233226Z t153-169).
    {
        // Two mountains beside the Campus: raw adjacency 2 (plus the
        // banked half-point of the adjacent centre), doubled to 4+.
        let ring: Vec<Pos> = game
            .nbrs(site)
            .into_iter()
            .filter(|position| *position != game.cities[&city].pos && game.map.get(*position).is_some())
            .collect();
        for position in ring.iter().take(2) {
            let tile = game.map.tiles.get_mut(position).unwrap();
            tile.terrain = crate::name!("mountain");
            tile.feature = None;
            tile.hills = false;
            tile.improvement = None;
            tile.district = None;
        }
        let raw = game.district_yields(crate::name!("campus"), site).science;
        assert!(raw >= 2.0 && raw < 4.0, "raw adjacency below the clause: {raw}");
        game.players[0].policies.insert(crate::name!("natural_philosophy"));
        assert!(game.district_yields(crate::name!("campus"), site).science >= 4.0);
        let with_both = game.city_yields(city).science;
        game.players[0].policies.remove(&crate::name!("rationalism"));
        let philosophy_alone = game.city_yields(city).science;
        assert!(
            (with_both - philosophy_alone).abs() < 1e-9,
            "a doubled adjacency does not open the clause: {philosophy_alone} -> {with_both}"
        );
        game.players[0].policies.remove(&crate::name!("natural_philosophy"));
        game.players[0].policies.insert(crate::name!("rationalism"));
    }

    // Fifteen Population pays one half of the card, and exactly half:
    // doubling the card's rating doubles what that half is worth.
    game.cities.get_mut(&city).unwrap().pop = 15;
    let with_card = game.city_yields(city).science;
    game.players[0].policies.clear();
    let without = game.city_yields(city).science;
    let half = with_card - without;
    assert!(half > 0.0, "15 Population earns one half of the card");

    std::sync::Arc::make_mut(&mut game.rules)
        .policies
        .get_mut("rationalism")
        .unwrap()
        .effects
        .insert("campus_building_science_pct".to_string(), 200.0);
    game.players[0].policies = [crate::name!("rationalism")].into_iter().collect();
    assert!(
        ((game.city_yields(city).science - without) - 2.0 * half).abs() < 1e-9,
        "the Population clause is exactly half the card"
    );
}

#[test]
fn finest_hour_and_the_pillage_cards_pay_what_they_ship_with() {
    let mut game = Game::new_full(1, 24, 16, 91_983, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let air = |game: &Game, unit: &str| {
        game.item_prod_mult(
            0,
            city,
            Some(&Item::Unit {
                unit: Name::new(unit),
            }),
        )
    };
    let bomber = air(&game, "bomber");
    let jet = air(&game, "jet_bomber");
    game.players[0].policies = [crate::name!("finest_hour")].into_iter().collect();
    // Modern and Atomic air units only. The Jet Bomber is Information era
    // and stays outside the window.
    assert_eq!(air(&game, "bomber"), bomber + 0.5);
    assert_eq!(air(&game, "jet_bomber"), jet);

    // Gathering Storm reduced Raid and Total War to +50%.
    for card in ["raid", "total_war"] {
        assert_eq!(
            game.rules.policies[card].effects["pillage_yield_pct"],
            50.0,
            "{card} adds half to pillage yields"
        );
    }
}

#[test]
fn unique_improvements_pay_their_conditional_clauses() {
    let mut game = Game::new_full(1, 24, 16, 91_977, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let centre = game.cities[&city].pos;
    let (kurgan, pasture) = (game.nbrs(centre)[0], game.nbrs(centre)[1]);
    for position in [kurgan, pasture] {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.improvement = None;
        tile.pillaged = false;
        tile.river_edges = [false; 6];
    }
    let faith = |game: &Game, at: Pos| {
        game.player_tile_yields(0, at, &game.map.tiles[&at]).faith
    };
    game.map.tiles.get_mut(&kurgan).unwrap().improvement = Some(crate::name!("kurgan"));
    let bare = faith(&game, kurgan);
    game.map.tiles.get_mut(&pasture).unwrap().improvement = Some(crate::name!("pasture"));
    // One Faith per adjacent Pasture, doubling once Stirrups obsoletes it.
    assert_eq!(faith(&game, kurgan), bare + 1.0);
    game.players[0].techs.insert(crate::name!("stirrups"));
    assert_eq!(faith(&game, kurgan), bare + 2.0);

    // The Sphinx gains Culture once Natural History is in.
    game.map.tiles.get_mut(&kurgan).unwrap().improvement = Some(crate::name!("sphinx"));
    let culture = game.player_tile_yields(0, kurgan, &game.map.tiles[&kurgan]).culture;
    game.players[0].civics.insert(crate::name!("natural_history"));
    assert_eq!(
        game.player_tile_yields(0, kurgan, &game.map.tiles[&kurgan]).culture,
        culture + 1.0
    );

    // The Ziggurat starts at 2 Science, gains Culture beside a river, and
    // gains another Culture at Natural History.
    game.players[0].civics.remove(&crate::name!("natural_history"));
    game.map.tiles.get_mut(&kurgan).unwrap().improvement = Some(crate::name!("ziggurat"));
    let plain = game.player_tile_yields(0, kurgan, &game.map.tiles[&kurgan]);
    assert_eq!(plain.science, 2.0);
    game.map.tiles.get_mut(&kurgan).unwrap().river_edges[0] = true;
    let riverside = game.player_tile_yields(0, kurgan, &game.map.tiles[&kurgan]);
    assert_eq!(riverside.culture, plain.culture + 1.0);
    game.players[0].civics.insert(crate::name!("natural_history"));
    assert_eq!(
        game.player_tile_yields(0, kurgan, &game.map.tiles[&kurgan]).culture,
        riverside.culture + 1.0
    );
}

#[test]
fn rock_hewn_church_matches_firaxis_placement_yields_appeal_and_tourism() {
    let mut game = Game::new_full(1, 24, 16, 91_976, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let centre = game.cities[&city].pos;
    let church = game.nbrs(centre)[0];
    let neighbors: Vec<Pos> = game.nbrs(church).into_iter().collect();
    let mountain = neighbors.iter().copied().find(|at| *at != centre).unwrap();
    let hill = neighbors
        .iter()
        .copied()
        .find(|at| *at != centre && *at != mountain)
        .unwrap();
    let volcanic = neighbors
        .iter()
        .copied()
        .find(|at| *at != centre && *at != mountain && *at != hill)
        .unwrap();
    let flat = neighbors
        .iter()
        .copied()
        .find(|at| *at != centre && *at != mountain && *at != hill && *at != volcanic)
        .unwrap();

    for position in std::iter::once(church).chain(neighbors.iter().copied()) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.owner_city = Some(city);
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
        if !game.cities[&city].owned_tiles.contains(&position) {
            game.cities.get_mut(&city).unwrap().owned_tiles.push(position);
        }
    }
    game.map.tiles.get_mut(&church).unwrap().hills = true;
    game.map.tiles.get_mut(&mountain).unwrap().terrain = crate::name!("mountain");
    game.map.tiles.get_mut(&hill).unwrap().hills = true;
    game.map.tiles.get_mut(&volcanic).unwrap().feature = Some(crate::name!("volcanic_soil"));
    game.players[0].civics.insert(crate::name!("drama_poetry"));

    assert!(!game
        .valid_improvements(0, church)
        .contains(&crate::name!("rock_hewn_church")));
    game.players[0].civ = "Ethiopia".to_string();
    assert!(game
        .valid_improvements(0, church)
        .contains(&crate::name!("rock_hewn_church")));
    assert!(game
        .valid_improvements(0, volcanic)
        .contains(&crate::name!("rock_hewn_church")));
    assert!(!game
        .valid_improvements(0, flat)
        .contains(&crate::name!("rock_hewn_church")));

    let adjacent_appeal = game.tile_appeal(flat);
    let site_appeal = game.tile_appeal(church).max(0) as f64;
    let bare_faith = game.player_tile_yields(0, church, &game.map.tiles[&church]).faith;
    game.map.tiles.get_mut(&church).unwrap().improvement =
        Some(crate::name!("rock_hewn_church"));
    let church_faith = game.player_tile_yields(0, church, &game.map.tiles[&church]).faith;
    assert_eq!(church_faith - bare_faith, 1.0 + site_appeal + 2.0);
    assert_eq!(game.tile_appeal(flat), adjacent_appeal + 1);

    for adjacent in [hill, volcanic] {
        assert!(!game
            .valid_improvements(0, adjacent)
            .contains(&crate::name!("rock_hewn_church")));
    }

    let before_flight = game
        .tourism_by_tile(0)
        .get(&church)
        .copied()
        .unwrap_or(0.0);
    game.players[0].techs.insert(crate::name!("flight"));
    let after_flight = game
        .tourism_by_tile(0)
        .get(&church)
        .copied()
        .unwrap_or(0.0);
    assert_eq!(after_flight - before_flight, church_faith - bare_faith);
}

#[test]
fn a_mine_accepts_flat_volcanic_soil_without_a_resource() {
    let mut game = Game::new_full(1, 24, 16, 91_977, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let mine = game.nbrs(game.cities[&city].pos)[0];
    {
        let tile = game.map.tiles.get_mut(&mine).unwrap();
        tile.owner_city = Some(city);
        tile.terrain = crate::name!("plains");
        tile.feature = Some(crate::name!("volcanic_soil"));
        tile.resource = None;
        tile.hills = false;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
    }
    if !game.cities[&city].owned_tiles.contains(&mine) {
        game.cities.get_mut(&city).unwrap().owned_tiles.push(mine);
    }
    game.players[0].techs.insert(crate::name!("mining"));

    assert!(game
        .valid_improvements(0, mine)
        .contains(&crate::name!("mine")));
}

#[test]
fn a_terrace_farm_accepts_flat_volcanic_soil() {
    let mut game = Game::new_full(1, 24, 16, 91_978, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let terrace = game.nbrs(game.cities[&city].pos)[0];
    {
        let tile = game.map.tiles.get_mut(&terrace).unwrap();
        tile.owner_city = Some(city);
        tile.terrain = crate::name!("plains");
        tile.feature = Some(crate::name!("volcanic_soil"));
        tile.resource = None;
        tile.hills = false;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
    }
    if !game.cities[&city].owned_tiles.contains(&terrace) {
        game.cities
            .get_mut(&city)
            .unwrap()
            .owned_tiles
            .push(terrace);
    }
    game.players[0].civ = "Inca".to_string();

    assert!(game
        .valid_improvements(0, terrace)
        .contains(&crate::name!("terrace_farm")));
}

#[test]
fn pairidaeza_matches_firaxis_identity_adjacency_progression_and_tourism() {
    let mut game = Game::new_full(1, 24, 16, 91_977, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let centre = game.cities[&city].pos;
    let garden = game.nbrs(centre)[0];
    let neighbors: Vec<Pos> = game.nbrs(garden).into_iter().collect();
    let holy = neighbors.iter().copied().find(|at| *at != centre).unwrap();
    let appeal_target = neighbors
        .iter()
        .copied()
        .find(|at| *at != centre && *at != holy)
        .unwrap();
    for position in std::iter::once(garden).chain(neighbors.iter().copied()) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.owner_city = Some(city);
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
        if !game.cities[&city].owned_tiles.contains(&position) {
            game.cities.get_mut(&city).unwrap().owned_tiles.push(position);
        }
    }
    game.map.tiles.get_mut(&holy).unwrap().district = Some(crate::name!("holy_site"));
    game.players[0].civics.insert(crate::name!("early_empire"));

    assert!(!game
        .valid_improvements(0, garden)
        .contains(&crate::name!("pairidaeza")));
    game.players[0].civ = "Persia".to_string();
    assert!(game
        .valid_improvements(0, garden)
        .contains(&crate::name!("pairidaeza")));

    let bare = game.player_tile_yields(0, garden, &game.map.tiles[&garden]);
    let appeal = game.tile_appeal(appeal_target);
    game.map.tiles.get_mut(&garden).unwrap().improvement = Some(crate::name!("pairidaeza"));
    let early = game.player_tile_yields(0, garden, &game.map.tiles[&garden]);
    assert_eq!(early.gold - bare.gold, 3.0, "2 base plus 1 beside the city centre");
    assert_eq!(early.culture - bare.culture, 2.0, "1 base plus 1 beside a Holy Site");
    assert_eq!(game.tile_appeal(appeal_target), appeal + 1);

    game.players[0].civics.insert(crate::name!("diplomatic_service"));
    let late = game.player_tile_yields(0, garden, &game.map.tiles[&garden]);
    assert_eq!(late.culture - early.culture, 1.0);
    let before_flight = game.tourism_by_tile(0).get(&garden).copied().unwrap_or(0.0);
    game.players[0].techs.insert(crate::name!("flight"));
    let after_flight = game.tourism_by_tile(0).get(&garden).copied().unwrap_or(0.0);
    assert_eq!(after_flight - before_flight, late.culture - bare.culture);
}

#[test]
fn cahokia_mound_matches_firaxis_suzerain_placement_and_progression() {
    let mut game = Game::new_full(2, 24, 16, 91_975, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let centre = game.cities[&city].pos;
    let mound = game.nbrs(centre)[0];
    let neighbors: Vec<Pos> = game.nbrs(mound).into_iter().collect();
    for position in std::iter::once(mound).chain(neighbors.iter().copied()) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.owner_city = Some(city);
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
        if !game.cities[&city].owned_tiles.contains(&position) {
            game.cities.get_mut(&city).unwrap().owned_tiles.push(position);
        }
    }
    let district_sites: Vec<Pos> = neighbors
        .iter()
        .copied()
        .filter(|position| *position != centre)
        .take(2)
        .collect();
    game.map.tiles.get_mut(&district_sites[0]).unwrap().district =
        Some(crate::name!("campus"));
    game.map.tiles.get_mut(&district_sites[1]).unwrap().district =
        Some(crate::name!("theater_square"));
    let adjacent_site = neighbors
        .iter()
        .copied()
        .find(|position| *position != centre && !district_sites.contains(position))
        .unwrap();

    game.players[1].is_minor = true;
    game.players[1].civ = "Cahokia".to_string();
    assert!(!game
        .valid_improvements(0, mound)
        .contains(&crate::name!("mound")));
    game.players[0].envoys.push((1, 3));
    assert!(game
        .valid_improvements(0, mound)
        .contains(&crate::name!("mound")));
    game.map.tiles.get_mut(&mound).unwrap().hills = true;
    assert!(!game
        .valid_improvements(0, mound)
        .contains(&crate::name!("mound")));
    game.map.tiles.get_mut(&mound).unwrap().hills = false;

    let housing_before = game.city_housing(&game.cities[&city]);
    let amenities_before = game.city_local_amenities(&game.cities[&city]);
    let bare = game.player_tile_yields(0, mound, &game.map.tiles[&mound]);
    game.map.tiles.get_mut(&mound).unwrap().improvement = Some(crate::name!("mound"));
    let initial = game.player_tile_yields(0, mound, &game.map.tiles[&mound]);
    assert_eq!(initial.gold - bare.gold, 3.0);
    assert_eq!(initial.food, bare.food);
    assert_eq!(game.city_housing(&game.cities[&city]), housing_before + 1.0);
    assert_eq!(game.city_local_amenities(&game.cities[&city]), amenities_before + 1);
    assert!(!game
        .valid_improvements(0, adjacent_site)
        .contains(&crate::name!("mound")));

    game.players[0].civics.insert(crate::name!("feudalism"));
    let medieval = game.player_tile_yields(0, mound, &game.map.tiles[&mound]);
    assert_eq!(medieval.food, initial.food + 1.0);
    game.players[0].techs.insert(crate::name!("replaceable_parts"));
    let mechanized = game.player_tile_yields(0, mound, &game.map.tiles[&mound]);
    assert_eq!(mechanized.food, initial.food + 2.0);
    game.players[0]
        .civics
        .insert(crate::name!("cultural_heritage"));
    assert_eq!(game.city_housing(&game.cities[&city]), housing_before + 2.0);

    let second = game
        .wdisk(centre, 3)
        .into_iter()
        .find(|position| {
            game.map.tiles.contains_key(position)
                && game.city_at(*position).is_none()
                && game.wdist(*position, mound) > 1
        })
        .unwrap();
    {
        let tile = game.map.tiles.get_mut(&second).unwrap();
        tile.owner_city = Some(city);
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.improvement = Some(crate::name!("mound"));
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
    }
    if !game.cities[&city].owned_tiles.contains(&second) {
        game.cities.get_mut(&city).unwrap().owned_tiles.push(second);
    }
    assert_eq!(game.city_local_amenities(&game.cities[&city]), amenities_before + 1);
    game.players[0].civics.insert(crate::name!("natural_history"));
    assert_eq!(game.city_local_amenities(&game.cities[&city]), amenities_before + 2);
}

#[test]
fn armagh_monastery_matches_firaxis_placement_faith_and_religious_healing() {
    let mut game = Game::new_full(2, 24, 16, 91_982, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let monastery = game.nbrs(game.cities[&city].pos)[0];
    {
        let tile = game.map.tiles.get_mut(&monastery).unwrap();
        tile.owner_city = Some(city);
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
    }
    if !game.cities[&city].owned_tiles.contains(&monastery) {
        game.cities.get_mut(&city).unwrap().owned_tiles.push(monastery);
    }

    game.players[1].is_minor = true;
    game.players[1].civ = "Armagh".to_string();
    assert!(!game
        .valid_improvements(0, monastery)
        .contains(&crate::name!("monastery")));
    game.players[0].envoys.push((1, 3));
    assert!(game
        .valid_improvements(0, monastery)
        .contains(&crate::name!("monastery")));
    // The shipped Armagh terrain rows include each base terrain and its Hills
    // variant; this is not a flat-only improvement.
    game.map.tiles.get_mut(&monastery).unwrap().hills = true;
    assert!(game
        .valid_improvements(0, monastery)
        .contains(&crate::name!("monastery")));
    game.map.tiles.get_mut(&monastery).unwrap().hills = false;

    let bare = game.player_tile_yields(0, monastery, &game.map.tiles[&monastery]);
    game.map.tiles.get_mut(&monastery).unwrap().improvement =
        Some(crate::name!("monastery"));
    let improved = game.player_tile_yields(0, monastery, &game.map.tiles[&monastery]);
    assert_eq!(improved.faith - bare.faith, 2.0);

    let missionary = game.spawn_unit("missionary", 0, monastery);
    assert_eq!(game.unit_heal_rate(missionary), 15);
    game.map.tiles.get_mut(&monastery).unwrap().pillaged = true;
    assert_eq!(game.unit_heal_rate(missionary), 0);
}

#[test]
fn cree_mekewap_matches_firaxis_placement_yields_and_housing_progression() {
    let mut game = Game::new_full(1, 24, 16, 91_977, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let centre = game.cities[&city].pos;
    let mekewap = game.nbrs(centre)[0];
    let neighbors: Vec<Pos> = game.nbrs(mekewap).into_iter().collect();
    for position in std::iter::once(mekewap).chain(neighbors.iter().copied()) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.owner_city = Some(city);
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
        if !game.cities[&city].owned_tiles.contains(&position) {
            game.cities.get_mut(&city).unwrap().owned_tiles.push(position);
        }
    }
    let resource_sites: Vec<Pos> = neighbors
        .iter()
        .copied()
        .filter(|position| *position != centre)
        .take(3)
        .collect();

    game.players[0].techs.insert(crate::name!("pottery"));
    assert!(!game
        .valid_improvements(0, mekewap)
        .contains(&crate::name!("mekewap")));
    game.players[0].civ = "Cree".to_string();
    assert!(!game
        .valid_improvements(0, mekewap)
        .contains(&crate::name!("mekewap")));
    game.map.tiles.get_mut(&resource_sites[0]).unwrap().resource =
        Some(crate::name!("wheat"));
    game.map.tiles.get_mut(&resource_sites[1]).unwrap().resource =
        Some(crate::name!("rice"));
    game.map.tiles.get_mut(&resource_sites[2]).unwrap().resource =
        Some(crate::name!("silk"));
    assert!(game
        .valid_improvements(0, mekewap)
        .contains(&crate::name!("mekewap")));

    let housing_before = game.city_housing(&game.cities[&city]);
    let bare = game.player_tile_yields(0, mekewap, &game.map.tiles[&mekewap]);
    game.map.tiles.get_mut(&mekewap).unwrap().improvement =
        Some(crate::name!("mekewap"));
    let initial = game.player_tile_yields(0, mekewap, &game.map.tiles[&mekewap]);
    assert_eq!(initial.production - bare.production, 1.0);
    assert_eq!(initial.food - bare.food, 1.0);
    assert_eq!(initial.gold - bare.gold, 1.0);
    assert_eq!(game.city_housing(&game.cities[&city]), housing_before + 1.0);

    game.players[0].civics.insert(crate::name!("civil_service"));
    let civil_service =
        game.player_tile_yields(0, mekewap, &game.map.tiles[&mekewap]);
    assert_eq!(civil_service.production, initial.production + 1.0);
    assert_eq!(game.city_housing(&game.cities[&city]), housing_before + 2.0);
    game.players[0].civics.insert(crate::name!("conservation"));
    let conservation =
        game.player_tile_yields(0, mekewap, &game.map.tiles[&mekewap]);
    assert_eq!(conservation.food, initial.food + 1.0);
    game.players[0].techs.insert(crate::name!("cartography"));
    let cartography = game.player_tile_yields(0, mekewap, &game.map.tiles[&mekewap]);
    assert_eq!(cartography.gold, initial.gold + 2.0);

    let adjacent_site = neighbors
        .iter()
        .copied()
        .find(|position| {
            *position != centre
                && game
                    .nbrs(*position)
                    .iter()
                    .any(|candidate| resource_sites.contains(candidate))
        })
        .unwrap();
    assert!(!game
        .valid_improvements(0, adjacent_site)
        .contains(&crate::name!("mekewap")));
}

#[test]
fn samarkand_trading_dome_matches_firaxis_placement_yields_and_routes() {
    let mut game = Game::new_full(3, 24, 16, 91_980, 200, 0, false);
    let origin = found_capital(&mut game, 0);
    let _destination = found_capital(&mut game, 1);
    let centre = game.cities[&origin].pos;
    let dome = game.nbrs(centre)[0];
    let neighbors: Vec<Pos> = game.nbrs(dome).into_iter().collect();
    for position in std::iter::once(dome).chain(neighbors.iter().copied()) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.owner_city = Some(origin);
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
        if !game.cities[&origin].owned_tiles.contains(&position) {
            game.cities.get_mut(&origin).unwrap().owned_tiles.push(position);
        }
    }
    let luxury = neighbors.iter().copied().find(|pos| *pos != centre).unwrap();
    let adjacent_dome = neighbors
        .iter()
        .copied()
        .find(|pos| *pos != centre && *pos != luxury)
        .unwrap();
    game.map.tiles.get_mut(&luxury).unwrap().resource = Some(crate::name!("silk"));

    game.players[2].is_minor = true;
    game.players[2].civ = "Samarkand".to_string();
    assert!(!game
        .valid_improvements(0, dome)
        .contains(&crate::name!("trading_dome")));
    game.players[0].envoys.push((2, 3));
    assert!(game
        .valid_improvements(0, dome)
        .contains(&crate::name!("trading_dome")));
    game.map.tiles.get_mut(&dome).unwrap().hills = true;
    assert!(game
        .valid_improvements(0, dome)
        .contains(&crate::name!("trading_dome")));
    game.map.tiles.get_mut(&dome).unwrap().hills = false;

    let bare = game.player_tile_yields(0, dome, &game.map.tiles[&dome]);
    game.map.tiles.get_mut(&dome).unwrap().improvement =
        Some(crate::name!("trading_dome"));
    let improved = game.player_tile_yields(0, dome, &game.map.tiles[&dome]);
    assert_eq!(improved.gold - bare.gold, 3.0);
    assert_eq!(game.trading_dome_origin_route_gold(origin), 1.0);
    assert!(!game
        .valid_improvements(0, adjacent_dome)
        .contains(&crate::name!("trading_dome")));
    game.map.tiles.get_mut(&dome).unwrap().pillaged = true;
    assert_eq!(game.trading_dome_origin_route_gold(origin), 0.0);
}

#[test]
fn granada_alcazar_matches_firaxis_placement_yields_tourism_and_defense() {
    let mut game = Game::new_full(2, 24, 16, 91_978, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let centre = game.cities[&city].pos;
    let alcazar = game.nbrs(centre)[0];
    let neighbors: Vec<Pos> = game.nbrs(alcazar).into_iter().collect();
    for position in std::iter::once(alcazar).chain(neighbors.iter().copied()) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.owner_city = Some(city);
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
        if !game.cities[&city].owned_tiles.contains(&position) {
            game.cities.get_mut(&city).unwrap().owned_tiles.push(position);
        }
    }

    game.players[1].is_minor = true;
    game.players[1].civ = "Granada".to_string();
    assert!(!game
        .valid_improvements(0, alcazar)
        .contains(&crate::name!("alcazar")));
    game.players[0].envoys.push((1, 3));
    assert!(game
        .valid_improvements(0, alcazar)
        .contains(&crate::name!("alcazar")));
    game.map.tiles.get_mut(&alcazar).unwrap().hills = true;
    assert!(game
        .valid_improvements(0, alcazar)
        .contains(&crate::name!("alcazar")));
    game.map.tiles.get_mut(&alcazar).unwrap().hills = false;
    game.map.tiles.get_mut(&alcazar).unwrap().feature = Some(crate::name!("forest"));
    assert!(!game
        .valid_improvements(0, alcazar)
        .contains(&crate::name!("alcazar")));
    game.map.tiles.get_mut(&alcazar).unwrap().feature = None;

    let appeal = game.tile_appeal(alcazar).max(0) as f64;
    let bare = game.player_tile_yields(0, alcazar, &game.map.tiles[&alcazar]);
    game.map.tiles.get_mut(&alcazar).unwrap().improvement = Some(crate::name!("alcazar"));
    let improved = game.player_tile_yields(0, alcazar, &game.map.tiles[&alcazar]);
    assert_eq!(improved.culture - bare.culture, 2.0);
    assert_eq!(improved.science - bare.science, appeal * 0.5);
    assert_eq!(game.tile_defense_bonus(alcazar), 4.0);
    let adjacent = neighbors
        .iter()
        .copied()
        .find(|position| *position != centre)
        .unwrap();
    assert!(!game
        .valid_improvements(0, adjacent)
        .contains(&crate::name!("alcazar")));

    let before_flight = game
        .tourism_by_tile(0)
        .get(&alcazar)
        .copied()
        .unwrap_or(0.0);
    game.players[0].techs.insert(crate::name!("flight"));
    let after_flight = game
        .tourism_by_tile(0)
        .get(&alcazar)
        .copied()
        .unwrap_or(0.0);
    assert_eq!(after_flight - before_flight, 2.0);

    let warrior = game.spawn_unit("warrior", 0, adjacent);
    game.relocate(warrior, alcazar);
    assert_eq!(game.units[&warrior].fortify_turns, 2);
    assert_eq!(game.unit_strength(&game.units[&warrior], true), 26.0);
    game.begin_turn(0);
    assert_eq!(game.units[&warrior].fortify_turns, 2);
    game.map.tiles.get_mut(&alcazar).unwrap().pillaged = true;
    assert_eq!(game.tile_defense_bonus(alcazar), 0.0);
}

#[test]
fn caguana_batey_matches_firaxis_placement_adjacency_and_tourism() {
    let mut game = Game::new_full(2, 24, 16, 91_979, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let centre = game.cities[&city].pos;
    let batey = game.nbrs(centre)[0];
    let neighbors: Vec<Pos> = game.nbrs(batey).into_iter().collect();
    for position in std::iter::once(batey).chain(neighbors.iter().copied()) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.owner_city = Some(city);
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
        if !game.cities[&city].owned_tiles.contains(&position) {
            game.cities.get_mut(&city).unwrap().owned_tiles.push(position);
        }
    }

    game.players[1].is_minor = true;
    game.players[1].civ = "Caguana".to_string();
    assert!(!game
        .valid_improvements(0, batey)
        .contains(&crate::name!("batey")));
    game.players[0].envoys.push((1, 3));
    assert!(game
        .valid_improvements(0, batey)
        .contains(&crate::name!("batey")));
    game.map.tiles.get_mut(&batey).unwrap().hills = true;
    assert!(!game
        .valid_improvements(0, batey)
        .contains(&crate::name!("batey")));
    game.map.tiles.get_mut(&batey).unwrap().hills = false;
    game.map.tiles.get_mut(&batey).unwrap().feature = Some(crate::name!("forest"));
    assert!(!game
        .valid_improvements(0, batey)
        .contains(&crate::name!("batey")));
    game.map.tiles.get_mut(&batey).unwrap().feature = None;

    let bonus = neighbors
        .iter()
        .copied()
        .find(|position| *position != centre)
        .unwrap();
    let entertainment = neighbors
        .iter()
        .copied()
        .find(|position| *position != centre && *position != bonus)
        .unwrap();
    let adjacent_batey = neighbors
        .iter()
        .copied()
        .find(|position| {
            *position != centre && *position != bonus && *position != entertainment
        })
        .unwrap();
    game.map.tiles.get_mut(&bonus).unwrap().resource = Some(crate::name!("wheat"));
    set_district(
        &mut game,
        city,
        entertainment,
        "entertainment_complex",
    );

    let bare = game.player_tile_yields(0, batey, &game.map.tiles[&batey]);
    game.map.tiles.get_mut(&batey).unwrap().improvement = Some(crate::name!("batey"));
    let initial = game.player_tile_yields(0, batey, &game.map.tiles[&batey]);
    assert_eq!(initial.culture - bare.culture, 3.0);
    assert!(!game
        .valid_improvements(0, adjacent_batey)
        .contains(&crate::name!("batey")));

    game.players[0].civics.insert(crate::name!("exploration"));
    let exploration = game.player_tile_yields(0, batey, &game.map.tiles[&batey]);
    assert_eq!(exploration.culture - bare.culture, 5.0);

    let before_flight = game.tourism_by_tile(0).get(&batey).copied().unwrap_or(0.0);
    game.players[0].techs.insert(crate::name!("flight"));
    let after_flight = game.tourism_by_tile(0).get(&batey).copied().unwrap_or(0.0);
    assert_eq!(after_flight - before_flight, 5.0);
}

#[test]
fn the_cliffs_of_dover_are_worth_twice_an_ordinary_natural_wonder() {
    // Features.Appeal is +2 for most natural wonders but +4 for the Cliffs
    // of Dover and Uluru, which CIVVIS flattened to a single +2 for every
    // wonder. Woods and an Oasis are +1; Rainforest, Marsh and Floodplains
    // are -1.
    let rules = crate::rules::Rules::embedded();
    assert_eq!(rules.features["cliffs_of_dover"].appeal, 4.0);
    assert_eq!(rules.features["uluru"].appeal, 4.0);
    for wonder in ["yosemite", "matterhorn", "pamukkale", "great_barrier_reef"] {
        assert_eq!(rules.features[wonder].appeal, 2.0, "{wonder}");
    }
    assert_eq!(rules.features["forest"].appeal, 1.0);
    assert_eq!(rules.features["oasis"].appeal, 1.0);
    for drab in ["jungle", "marsh", "floodplains"] {
        assert_eq!(rules.features[drab].appeal, -1.0, "{drab}");
    }

    // And the difference reaches a tile: the Cliffs are worth two more
    // than Yosemite to the same neighbour.
    let mut game = Game::new_full(1, 24, 16, 91_972, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let centre = game.cities[&city].pos;
    let site = game.nbrs(centre)[0];
    for position in [centre, site] {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.feature = None;
        tile.improvement = None;
        tile.pillaged = false;
    }
    let bare = game.tile_appeal(centre);
    game.map.tiles.get_mut(&site).unwrap().feature = Some(crate::name!("yosemite"));
    assert_eq!(game.tile_appeal(centre), bare + 2);
    game.map.tiles.get_mut(&site).unwrap().feature = Some(crate::name!("cliffs_of_dover"));
    assert_eq!(game.tile_appeal(centre), bare + 4);
}

#[test]
fn every_wall_tier_carries_its_shipped_outer_defence() {
    // Buildings.OuterDefenseHitPoints, which the ratchet's Buildings
    // projection did not read. Every wall tier is 100 and Georgia's Tsikhe
    // is the one that ships 200 -- the reason it is worth taking over the
    // Renaissance Walls it replaces.
    let rules = crate::rules::Rules::embedded();
    for wall in ["walls", "medieval_walls", "renaissance_walls"] {
        assert_eq!(rules.buildings[wall].outer_defense, 100, "{wall}");
    }
    assert_eq!(rules.buildings["tsikhe"].outer_defense, 200);
    assert_eq!(
        rules.buildings["tsikhe"].replaces,
        Some(crate::name!("renaissance_walls"))
    );
}

#[test]
fn mines_and_quarries_lower_the_appeal_of_their_neighbours() {
    let mut game = Game::new_full(1, 24, 16, 91_971, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let centre = game.cities[&city].pos;
    let site = game.nbrs(centre)[0];
    for position in [centre, site] {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.feature = None;
        tile.improvement = None;
        tile.pillaged = false;
    }
    let before = game.tile_appeal(centre);
    game.map.tiles.get_mut(&site).unwrap().improvement = Some(crate::name!("mine"));
    assert_eq!(game.tile_appeal(centre), before - 1);
    // Improvements.Appeal for the Sphinx is +2, the same as a City Park
    // and an Ice Hockey Rink; the +1 tier is the Chateau and Golf Course.
    game.map.tiles.get_mut(&site).unwrap().improvement = Some(crate::name!("sphinx"));
    assert_eq!(game.tile_appeal(centre), before + 2);
    // Pillaging stops the grant and costs the tile its own Appeal point.
    game.map.tiles.get_mut(&site).unwrap().pillaged = true;
    assert_eq!(game.tile_appeal(centre), before - 1);
}

#[test]
fn lumber_mills_reach_rainforest_only_at_mercantilism() {
    let mut game = Game::new_full(1, 24, 16, 91_963, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let site = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    {
        let tile = game.map.tiles.get_mut(&site).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = Some(crate::name!("jungle"));
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    game.players[0].techs.insert(crate::name!("construction"));
    assert!(!game.valid_improvements(0, site).contains(&crate::name!("lumber_mill")));
    game.players[0].civics.insert(crate::name!("mercantilism"));
    assert!(game.valid_improvements(0, site).contains(&crate::name!("lumber_mill")));

    // Woods never needed the civic.
    game.map.tiles.get_mut(&site).unwrap().feature = Some(crate::name!("forest"));
    game.players[0].civics.remove(&Name::new("mercantilism"));
    assert!(game.valid_improvements(0, site).contains(&crate::name!("lumber_mill")));
}

#[test]
fn governor_promotions_speed_their_district_buildings() {
    // Connoisseur, Divine Architect and Provision each pair a headline
    // effect with a Production bonus for one district's buildings.
    let mut game = Game::new_full(1, 24, 16, 91_951, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let sites: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != game.cities[&city].pos)
        .collect();
    for (index, district) in ["theater_square", "holy_site", "industrial_zone"]
        .into_iter()
        .enumerate()
    {
        set_district(&mut game, city, sites[index], district);
    }
    let speed = |game: &Game, building: &str| {
        game.item_prod_mult(
            0,
            city,
            Some(&Item::Building {
                building: Name::new(building),
            }),
        )
    };
    let before = [
        speed(&game, "amphitheater"),
        speed(&game, "shrine"),
        speed(&game, "workshop"),
    ];
    appoint_established(&mut game, 0, "pingala", city, &["connoisseur"]);
    appoint_established(&mut game, 0, "moksha", city, &["divine_architect"]);
    appoint_established(&mut game, 0, "magnus", city, &["provision"]);
    assert_eq!(speed(&game, "amphitheater"), before[0] + 0.2);
    assert_eq!(speed(&game, "shrine"), before[1] + 0.2);
    assert_eq!(speed(&game, "workshop"), before[2] + 0.2);
}

#[test]
fn base_governor_abilities_arrive_with_the_appointment() {
    // GovernorPromotions marks Land Acquisition and Librarian
    // BaseAbility, at Level 0 — they arrive with the establishment rather
    // than costing a title, the same way Magnus' Groundbreaker, Liang's
    // Guildmaster, Amani's Messenger, Moksha's Bishop and Victor's Redoubt
    // already did here.
    let mut game = Game::new_full(1, 24, 16, 91_947, 200, 0, false);
    let city = found_capital(&mut game, 0);
    appoint_established(&mut game, 0, "reyna", city, &[]);
    appoint_established(&mut game, 0, "pingala", city, &[]);
    assert_eq!(game.governor_effect(0, city, "border_growth_pct"), 20.0);
    assert_eq!(
        game.governor_effect(0, city, "incoming_foreign_trade_gold"),
        3.0
    );
    assert_eq!(game.governor_effect(0, city, "science_pct"), 15.0);
    assert_eq!(game.governor_effect(0, city, "culture_pct"), 15.0);

    // Every governor carries exactly the five promotions the shipped
    // promotion set holds beside its base ability.
    for governor in ["amani", "liang", "magnus", "moksha", "pingala", "reyna", "victor"] {
        assert_eq!(
            game.rules.governors[governor].promotions.len(),
            5,
            "{governor} should hold five promotions beside its base ability"
        );
    }
}

#[test]
fn reyna_executes_routes_borders_adjacency_forestry_purchases_and_renewables() {
    let mut game = Game::new_full(2, 26, 16, 91_785, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let rival = found_capital(&mut game, 1);
    appoint_established(
        &mut game,
        0,
        "reyna",
        city,
        &[
            "land_acquisition",
            "harbormaster",
            "forestry_management",
            "tax_collector",
            "contractor",
            "renewable_subsidizer",
        ],
    );
    game.cities.get_mut(&city).unwrap().pop = 4;
    game.routes.push(TradeRoute {
        origin: rival,
        dest: city,
        owner: 1,
        ends: game.turn + 30,
    });

    let mut base_reyna = game.clone();
    base_reyna.players[0]
        .governor_roster
        .get_mut("reyna")
        .unwrap()
        .promotions
        .remove("tax_collector");
    let mut no_reyna = base_reyna.clone();
    no_reyna.players[0].governor_roster.clear();
    assert_eq!(
        base_reyna.governor_effect(0, city, "incoming_foreign_trade_gold"),
        3.0
    );
    assert!(base_reyna.city_yields(city).gold > no_reyna.city_yields(city).gold);
    // Borders grow on the city's Culture (the shipped rule).
    let base_border = no_reyna.city_yields(city).culture;
    base_reyna.process_city(0, city);
    no_reyna.process_city(0, city);
    assert_eq!(base_reyna.cities[&city].border_culture, base_border * 1.2);
    assert_eq!(no_reyna.cities[&city].border_culture, base_border);

    let mut without_tax = game.clone();
    without_tax.players[0]
        .governor_roster
        .get_mut("reyna")
        .unwrap()
        .promotions
        .remove("tax_collector");
    assert_eq!(game.governor_effect(0, city, "gold_per_pop"), 2.0);
    assert!(game.city_yields(city).gold > without_tax.city_yields(city).gold);

    let center = game.cities[&city].pos;
    let sites: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != center)
        .take(4)
        .collect();
    set_district(&mut game, city, sites[0], "commercial_hub");
    game.map.tiles.get_mut(&sites[0]).unwrap().river_edges[0] = true;
    let mut without_harbormaster = game.clone();
    without_harbormaster.players[0]
        .governor_roster
        .get_mut("reyna")
        .unwrap()
        .promotions
        .remove("harbormaster");
    assert_eq!(
        game.district_yields(crate::name!("commercial_hub"), sites[0]).gold
            - without_harbormaster
                .district_yields(crate::name!("commercial_hub"), sites[0])
                .gold,
        2.0
    );

    {
        let tile = game.map.tiles.get_mut(&sites[1]).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = Some(crate::name!("forest"));
        tile.improvement = None;
        tile.pillaged = false;
    }
    // Forestry Management pays its Appeal once per adjacent owned tile that
    // carries an unimproved feature, so the target has to touch exactly one
    // of them for the delta below to be 1. Which tile that is depends on
    // the map, so pick it by the predicate rather than by position.
    let unimproved_neighbours = |game: &Game, position: Pos| {
        game.nbrs(position)
            .into_iter()
            .filter(|neighbour| {
                let tile = &game.map.tiles[neighbour];
                tile.owner_city == Some(city)
                    && tile.improvement.is_none()
                    && tile.feature.is_some()
            })
            .count()
    };
    let appeal_target = game
        .nbrs(sites[1])
        .into_iter()
        .find(|position| game.map.tiles[position].owner_city == Some(city))
        .unwrap();
    let mut without_forestry = game.clone();
    without_forestry.players[0]
        .governor_roster
        .get_mut("reyna")
        .unwrap()
        .promotions
        .remove("forestry_management");
    assert_eq!(
        game.player_tile_yields(0, sites[1], &game.map.tiles[&sites[1]])
            .gold
            - without_forestry
                .player_tile_yields(0, sites[1], &without_forestry.map.tiles[&sites[1]],)
                .gold,
        2.0
    );
    // One Appeal per adjacent owned tile carrying an unimproved feature,
    // which is the rule rather than a property of this particular map.
    assert_eq!(
        game.tile_appeal(appeal_target) - without_forestry.tile_appeal(appeal_target),
        unimproved_neighbours(&game, appeal_target) as i32
    );
    assert!(
        unimproved_neighbours(&game, appeal_target) > 0,
        "the fixture must give Forestry Management something to pay on"
    );

    {
        let tile = game.map.tiles.get_mut(&sites[2]).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.improvement = Some(crate::name!("solar_farm"));
        tile.pillaged = false;
    }
    let mut without_renewables = game.clone();
    without_renewables.players[0]
        .governor_roster
        .get_mut("reyna")
        .unwrap()
        .promotions
        .remove("renewable_subsidizer");
    assert_eq!(
        game.player_tile_yields(0, sites[2], &game.map.tiles[&sites[2]])
            .gold
            - without_renewables
                .player_tile_yields(0, sites[2], &without_renewables.map.tiles[&sites[2]],)
                .gold,
        2.0
    );
    assert_eq!(
        game.city_renewable_power(&game.cities[&city])
            - without_renewables.city_renewable_power(&without_renewables.cities[&city]),
        2.0
    );
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("hydroelectric_dam"));
    without_renewables
        .cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("hydroelectric_dam"));
    set_district(&mut game, city, sites[3], "dam");
    set_district(&mut without_renewables, city, sites[3], "dam");
    assert!(
        game.city_yields(city).gold > without_renewables.city_yields(city).gold,
        "the hydroelectric dam receives Reyna's renewable Gold"
    );
    assert_eq!(
        game.city_renewable_power(&game.cities[&city])
            - without_renewables.city_renewable_power(&without_renewables.cities[&city]),
        4.0
    );
    game.cities
        .get_mut(&city)
        .unwrap()
        .wonders
        .insert(crate::name!("biosphere"), center);
    without_renewables
        .cities
        .get_mut(&city)
        .unwrap()
        .wonders
        .insert(crate::name!("biosphere"), center);
    assert!(
        (game.tourism_per_turn(0) - without_renewables.tourism_per_turn(0) - 12.0).abs() < 1e-9,
        "the Biosphere triples Reyna's four bonus renewable Power into Tourism"
    );

    game.players[0].techs = game.rules.techs.keys().cloned().collect();
    game.players[0].civics = game.rules.civics.keys().cloned().collect();
    game.players[0].gold = 10_000.0;
    let district_site = game
        .district_sites(city, crate::name!("government_plaza"))
        .into_iter()
        .next()
        .unwrap();
    let district = Item::District {
        district: crate::name!("government_plaza"),
        pos: district_site,
    };
    let cost = game.item_cost_for_city(0, city, &district) * 4.0;
    let gold = game.players[0].gold;
    game.do_buy_district(0, city, "government_plaza", district_site, "gold")
        .unwrap();
    assert_eq!(game.players[0].gold, gold - cost);
    assert!(game.city_has_district_family(&game.cities[&city], crate::name!("government_plaza")));
}

#[test]
fn victor_executes_defense_loyalty_resources_strikes_promotions_air_and_nukes() {
    let mut game = Game::new_full(2, 28, 18, 91_786, 200, 0, false);
    let city = found_capital(&mut game, 0);
    let rival = found_capital(&mut game, 1);
    appoint_established(
        &mut game,
        0,
        "victor",
        city,
        &[
            "garrison_commander",
            "defense_logistics",
            "embrasure",
            "air_defense_initiative",
            "arms_race_proponent",
        ],
    );
    game.at_war.insert(pair(0, 1));

    let victor = &game.rules.governors["victor"];
    assert_eq!(
        victor.promotions["embrasure"].requires,
        vec!["garrison_commander", "defense_logistics"]
    );
    assert_eq!(
        victor.promotions["air_defense_initiative"].requires,
        vec!["embrasure"]
    );
    assert_eq!(
        victor.promotions["arms_race_proponent"].requires,
        vec!["embrasure"]
    );

    let mut without_victor = game.clone();
    without_victor.players[0].governor_roster.clear();
    without_victor.sync_governor_cities(0);
    assert_eq!(
        game.city_strength(city) - without_victor.city_strength(city),
        5.0
    );

    let center = game.cities[&city].pos;
    let mut owned: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != center)
        .collect();
    owned.sort();
    assert!(owned.len() >= 4);
    for position in &owned {
        let tile = game.map.tiles.get_mut(position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
    }

    let defender = game.spawn_test_unit("warrior", 0, owned[0]);
    let mut without_commander = game.clone();
    without_commander.players[0]
        .governor_roster
        .get_mut("victor")
        .unwrap()
        .promotions
        .remove("garrison_commander");
    assert_eq!(
        game.unit_strength(&game.units[&defender], true)
            - without_commander.unit_strength(&without_commander.units[&defender], true),
        5.0
    );
    assert_eq!(
        game.unit_strength(&game.units[&defender], false),
        without_commander.unit_strength(&without_commander.units[&defender], false)
    );

    game.players[0].techs.insert(crate::name!("bronze_working"));
    {
        let tile = game.map.tiles.get_mut(&owned[1]).unwrap();
        tile.resource = Some(crate::name!("iron"));
        tile.improvement = Some(crate::name!("mine"));
        tile.pillaged = false;
    }
    let mut without_logistics = game.clone();
    without_logistics.players[0]
        .governor_roster
        .get_mut("victor")
        .unwrap()
        .promotions
        .remove("defense_logistics");
    assert_eq!(
        game.strategic_resource_rate(0, "iron")
            - without_logistics.strategic_resource_rate(0, "iron"),
        1.0
    );

    let mut with_loyalty = game.clone();
    let mut without_loyalty = without_commander.clone();
    with_loyalty.cities.get_mut(&city).unwrap().loyalty = 50.0;
    without_loyalty.cities.get_mut(&city).unwrap().loyalty = 50.0;
    with_loyalty.process_loyalty(0);
    without_loyalty.process_loyalty(0);
    assert_eq!(
        with_loyalty.cities[&city].loyalty - without_loyalty.cities[&city].loyalty,
        4.0
    );

    let (encampment_position, strike_position) = owned
        .iter()
        .copied()
        .filter(|position| {
            *position != owned[0] && *position != owned[1] && game.wdist(center, *position) == 1
        })
        .find_map(|encampment| {
            game.nbrs(center)
                .into_iter()
                .find(|target| {
                    *target != encampment
                        && *target != owned[0]
                        && *target != owned[1]
                        && game.wdist(encampment, *target) == 1
                        && game.city_at(*target).is_none()
                        && game.units_at(*target).is_empty()
                        && game.map.get(*target).is_some_and(|tile| {
                            game.rules.is_passable(tile) && !game.rules.is_water(tile)
                        })
                })
                .map(|target| (encampment, target))
        })
        .expect("test map has adjacent city and Encampment strike geometry");
    {
        let tile = game.map.tiles.get_mut(&strike_position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
    }
    set_district(&mut game, city, encampment_position, "encampment");
    {
        let defended_city = game.cities.get_mut(&city).unwrap();
        defended_city.wall_hp = 100;
        defended_city.encampment_hp = 100;
        defended_city.encampment_wall_hp = 100;
    }
    let strike_target = game.spawn_test_unit("modern_armor", 1, strike_position);
    game.do_city_strike(0, city, strike_position).unwrap();
    game.do_city_strike(0, city, strike_position).unwrap();
    assert!(game.do_city_strike(0, city, strike_position).is_err());
    assert_eq!(game.cities[&city].extra_strikes_used, 1);
    assert!(game.units.contains_key(&strike_target));
    game.do_encampment_strike(0, city, strike_position).unwrap();
    game.do_encampment_strike(0, city, strike_position).unwrap();
    assert!(game.do_encampment_strike(0, city, strike_position).is_err());
    assert_eq!(game.cities[&city].encampment_extra_strikes_used, 1);

    for position in game.nbrs(center) {
        if game
            .map
            .get(position)
            .is_some_and(|tile| game.rules.is_passable(tile))
            && !game.units_at(position).iter().any(|unit| {
                game.units[unit].owner == 1
                    && game.rules.units[game.units[unit].kind].class == "military"
            })
        {
            game.spawn_test_unit("warrior", 1, position);
        }
    }
    assert!(!game.city_under_siege(city));
    let mut siegeable = game.clone();
    siegeable.players[0]
        .governor_roster
        .get_mut("victor")
        .unwrap()
        .promotions
        .remove("defense_logistics");
    assert!(siegeable.city_under_siege(city));

    let trainee = game.spawn_test_unit("warrior", 0, center);
    game.apply_training_district_effects(city, trainee);
    assert!(game.promotion_pending(trainee));
    assert_eq!(game.units[&trainee].xp, Game::promotion_threshold(1));

    let anti_air = game.spawn_test_unit("anti_air_gun", 0, owned[0]);
    let bomber = game.spawn_test_unit("bomber", 1, game.cities[&rival].pos);
    let mut without_air_defense = game.clone();
    without_air_defense.players[0]
        .governor_roster
        .get_mut("victor")
        .unwrap()
        .promotions
        .remove("air_defense_initiative");
    let bomber_state = game.units[&bomber].clone();
    let without_bomber_state = without_air_defense.units[&bomber].clone();
    assert_eq!(
        game.air_interception_strength(&bomber_state, game.units[&anti_air].pos)
            - without_air_defense.air_interception_strength(
                &without_bomber_state,
                without_air_defense.units[&anti_air].pos,
            ),
        25.0
    );

    let nuclear = Item::Project {
        project: crate::name!("build_nuclear_device"),
    };
    let mut without_arms_race = game.clone();
    without_arms_race.players[0]
        .governor_roster
        .get_mut("victor")
        .unwrap()
        .promotions
        .remove("arms_race_proponent");
    assert!(
        (game.item_prod_mult(0, city, Some(&nuclear))
            - without_arms_race.item_prod_mult(0, city, Some(&nuclear))
            - 0.3)
            .abs()
            < 1e-9
    );
}
