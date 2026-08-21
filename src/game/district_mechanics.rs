use super::*;

/// ★★★★ A wonder the host will not place must stop being chosen — IN THAT CITY.
///
/// Hanging Gardens wants a river, Great Bath floodplains, Temple of Artemis a
/// Camp/Pasture/Plantation beside it. CIVVIS picks the site from ITS terrain
/// model, the two rulesets disagree about the ground, and nothing carried the
/// refusal back: 370 `build_no_plot` wonder refusals over 20 live runs from only
/// 29 (run, city, wonder) combinations, 53 consecutive turns at worst.
///
/// ⚠⚠ The second assertion is the one that matters. A GLOBAL block would stop
/// the empire building that wonder anywhere the first time one city had no
/// river, trading a small waste for a large one.
#[test]
fn a_wonder_the_host_will_not_place_is_blocked_in_that_city_only() {
    let mut game = Game::new(4, 20, 20, 7, 500, 0);
    let mut ours: Vec<u32> = game
        .cities
        .values()
        .filter(|c| c.owner == 0)
        .map(|c| c.id)
        .collect();
    while ours.len() < 2 {
        let seed = ours.len() as i32;
        let pos = (seed * 5 + 6, seed * 5 + 6);
        if !game.map.tiles.contains_key(&pos) {
            break;
        }
        game.place_city(0, pos, None);
        ours = game
            .cities
            .values()
            .filter(|c| c.owner == 0)
            .map(|c| c.id)
            .collect();
    }
    assert!(ours.len() >= 2, "need two cities to prove the block is scoped");
    let (blocked_city, other_city) = (ours[0], ours[1]);
    let techs: Vec<Name> = game.rules.techs.keys().map(|t| Name::new(t.as_str())).collect();
    for tech in techs {
        game.players[0].techs.insert(tech);
    }
    let civics: Vec<Name> = game.rules.civics.keys().map(|c| Name::new(c.as_str())).collect();
    for civic in civics {
        game.players[0].civics.insert(civic);
    }
    for cid in [blocked_city, other_city] {
        if let Some(city) = game.cities.get_mut(&cid) {
            city.pop = 12;
        }
    }

    // ⚠ DISCOVERED, not hardcoded: which wonders a city can site depends on its
    // ground, so naming one would make the precondition fail on an unremarkable
    // fixture rather than on anything to do with this change.
    let wonder = game
        .rules
        .wonders
        .keys()
        .map(|name| Name::new(name.as_str()))
        .find(|name| {
            !game.wonder_sites(blocked_city, name.as_str()).is_empty()
                && !game.wonder_sites(other_city, name.as_str()).is_empty()
        })
        .expect("some wonder must be sitable in both cities for this to prove anything");

    game.blocked_wonders
        .entry(blocked_city)
        .or_default()
        .insert(wonder);

    assert!(
        game.wonder_sites(blocked_city, wonder.as_str()).is_empty(),
        "the host said it has no ground for {wonder:?} here, so nothing may be offered"
    );
    assert!(
        !game.wonder_sites(other_city, wonder.as_str()).is_empty(),
        "and a refusal in one city must not disarm the wonder empire-wide"
    );
}

fn emergency_game_with_capitals(players: usize, seed: u64, max_turns: u32) -> Game {
    let mut game = Game::new_full(players, 26, 16, seed, max_turns, 0, false);
    for player in 0..players {
        let settler = game
            .player_unit_ids(player)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(player, game.units[&settler].pos, None);
    }
    game
}

fn install_alliance(
    game: &mut Game,
    first: usize,
    second: usize,
    kind: &str,
    level: i32,
    points: f64,
) {
    let alliance = AllianceState {
        kind: kind.to_string(),
        points,
        level,
        ends: game.turn + 60,
    };
    game.players[first]
        .alliances
        .insert(second, alliance.clone());
    game.players[second].alliances.insert(first, alliance);
}

fn controlled_game() -> (Game, u32, Pos, Vec<Pos>) {
    let mut game = Game::new_full(1, 20, 14, 5150, 300, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    let city_position = game.cities[&city].pos;
    let district_position = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| {
            game.wdisk(*position, 1).len() == 7 && game.wdist(*position, city_position) > 4
        })
        .unwrap();
    let mut ring = game.nbrs(district_position);
    ring.sort();
    for position in std::iter::once(district_position).chain(ring.iter().copied()) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.owner_city = None;
        tile.river_edges = [false; 6];
    }
    game.map
        .tiles
        .get_mut(&district_position)
        .unwrap()
        .owner_city = Some(city);
    (game, city, district_position, ring.to_vec())
}

fn adjacency_value(game: &Game, district: &str, source: &str) -> f64 {
    game.rules.districts[district].adjacency[source].total()
}

#[test]
fn gathering_storm_catalog_contains_every_generic_and_unique_district() {
    let game = Game::new_full(1, 20, 14, 5151, 30, 0, false);
    let expected: BTreeSet<&str> = [
        "acropolis",
        "aerodrome",
        "aqueduct",
        "bath",
        "campus",
        "canal",
        "city_center",
        "commercial_hub",
        "copacabana",
        "cothon",
        "dam",
        "diplomatic_quarter",
        "encampment",
        "entertainment_complex",
        "government_plaza",
        "hansa",
        "harbor",
        "hippodrome",
        "holy_site",
        "ikanda",
        "industrial_zone",
        "lavra",
        "mbanza",
        "neighborhood",
        "observatory",
        "oppidum",
        "preserve",
        "royal_navy_dockyard",
        "seowon",
        "spaceport",
        "street_carnival",
        "suguba",
        "thanh",
        "theater_square",
        "water_park",
    ]
    .into_iter()
    .collect();
    let actual: BTreeSet<&str> = game.rules.districts.keys().map(|name| name.as_str()).collect();
    assert_eq!(actual, expected);

    for (name, spec) in &game.rules.districts {
        if let Some(base) = spec.replaces {
            assert!(
                game.rules.districts.contains_key(base.as_str()),
                "{name} replaces {base}"
            );
            assert!(
                spec.unique_to.is_some(),
                "{name} replacement has no civilization"
            );
        }
    }
    assert_eq!(
        game.rules.districts["ikanda"].placement,
        "not_adjacent_city"
    );
    assert_eq!(game.rules.districts["thanh"].placement, "not_adjacent_city");
    assert_eq!(game.rules.districts["acropolis"].placement, "hills");
    assert!(!game.rules.districts["thanh"].specialty);
    assert!(!game.rules.districts["spaceport"].specialty);
    assert!(!game.rules.districts["city_center"].buildable);
    assert_eq!(game.rules.districts["water_park"].cost, 54.0);
    assert_eq!(game.rules.districts["copacabana"].cost, 27.0);
    assert_eq!(game.rules.districts["dam"].max_per_city, None);
}

#[test]
fn district_great_person_points_include_unique_rates_and_lavra_building_gates() {
    let (mut game, city, position, _) = controlled_game();
    assert_eq!(
        game.rules.districts["theater_square"].great_person_points,
        BTreeMap::from([
            ("artist".to_string(), 1.0),
            ("musician".to_string(), 1.0),
            ("writer".to_string(), 1.0),
        ])
    );
    assert_eq!(
        game.rules.districts["lavra"].great_person_points["prophet"],
        2.0
    );
    assert_eq!(
        game.rules.districts["royal_navy_dockyard"].great_person_points["admiral"],
        2.0
    );
    assert!(game.rules.districts["thanh"].great_person_points.is_empty());

    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("lavra"), position);
    game.process_great_people(0);
    assert_eq!(game.players[0].gpp.get("prophet"), Some(&2.0));
    assert_eq!(game.players[0].gpp.get("writer"), None);
    assert_eq!(game.players[0].gpp.get("artist"), None);
    assert_eq!(game.players[0].gpp.get("musician"), None);

    game.cities.get_mut(&city).unwrap().buildings.extend([
        crate::name!("shrine"),
        crate::name!("temple"),
        crate::name!("cathedral"),
    ]);
    game.process_great_people(0);
    assert_eq!(
        game.players[0].gpp.get("prophet"),
        Some(&6.0),
        "the second tick includes Lavra, Shrine and Temple prophet points"
    );
    assert_eq!(game.players[0].gpp.get("writer"), Some(&1.0));
    assert_eq!(game.players[0].gpp.get("artist"), Some(&1.0));
    assert_eq!(game.players[0].gpp.get("musician"), Some(&1.0));

    let owned_before = game.cities[&city].owned_tiles.len();
    game.apply_great_person_district_effects(0);
    assert_eq!(game.cities[&city].owned_tiles.len(), owned_before + 1);

    let (mut spark, spark_city, spark_position, _) = controlled_game();
    spark.players[0].pantheon = Some("divine_spark".to_string());
    spark
        .cities
        .get_mut(&spark_city)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), spark_position);
    spark.process_great_people(0);
    assert_eq!(spark.players[0].gpp.get("writer"), Some(&1.0));
    assert_eq!(spark.players[0].gpp.get("artist"), Some(&1.0));
    assert_eq!(spark.players[0].gpp.get("musician"), Some(&1.0));
    spark
        .cities
        .get_mut(&spark_city)
        .unwrap()
        .buildings
        .push(crate::name!("amphitheater"));
    spark.process_great_people(0);
    assert_eq!(spark.players[0].gpp.get("writer"), Some(&4.0));
    assert_eq!(spark.players[0].gpp.get("artist"), Some(&2.0));
    assert_eq!(spark.players[0].gpp.get("musician"), Some(&2.0));
}

#[test]
fn unique_district_bonuses_are_data_driven_and_disable_when_pillaged() {
    let (mut game, city, position, _) = controlled_game();
    let effects = [
        ("acropolis", "envoys", 1.0),
        ("suguba", "gold_faith_purchase_discount_pct", 20.0),
        ("ikanda", "building_gold", 2.0),
        ("ikanda", "building_science", 1.0),
        ("cothon", "naval_settler_production_pct", 50.0),
        ("cothon", "naval_heal_full", 1.0),
        ("royal_navy_dockyard", "naval_movement", 1.0),
        ("royal_navy_dockyard", "foreign_continent_gold", 2.0),
        ("royal_navy_dockyard", "foreign_continent_loyalty", 4.0),
        ("hippodrome", "free_heavy_cavalry", 1.0),
        ("oppidum", "unlock_apprenticeship", 1.0),
        ("thanh", "tourism_after_flight", 1.0),
    ];
    for (district, effect, expected) in effects {
        assert_eq!(game.rules.districts[district].effects[effect], expected);
    }

    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("cothon"), position);
    assert_eq!(
        game.city_district_effect(&game.cities[&city], "naval_settler_production_pct"),
        50.0
    );
    let galley = game.spawn_unit("galley", 0, position);
    assert_eq!(game.unit_heal_rate(galley), 100);

    game.map.tiles.get_mut(&position).unwrap().pillaged = true;
    assert_eq!(
        game.city_district_effect(&game.cities[&city], "naval_settler_production_pct"),
        0.0
    );
    assert_eq!(game.unit_heal_rate(galley), 20);

    game.map.tiles.get_mut(&position).unwrap().pillaged = false;
    game.cities.get_mut(&city).unwrap().districts.clear();
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("ikanda"), position);
    assert_eq!(
        game.district_building_yields(&game.cities[&city], "barracks"),
        Yields {
            gold: 2.0,
            science: 1.0,
            ..Yields::default()
        }
    );
}

#[test]
fn every_stock_adjacency_source_has_its_exact_value() {
    let game = Game::new_full(1, 20, 14, 5152, 30, 0, false);
    let expected = [
        ("campus", "mountain", 1.0),
        ("campus", "rainforest", 0.5),
        ("campus", "district", 0.5),
        ("campus", "reef", 2.0),
        ("campus", "geothermal_fissure", 2.0),
        ("campus", "pamukkale", 2.0),
        ("campus", "government_plaza", 1.0),
        ("holy_site", "natural_wonder", 2.0),
        ("holy_site", "mountain", 1.0),
        ("holy_site", "forest", 0.5),
        ("holy_site", "district", 0.5),
        ("holy_site", "pamukkale", 1.0),
        ("holy_site", "government_plaza", 1.0),
        ("commercial_hub", "river", 2.0),
        ("commercial_hub", "harbor", 2.0),
        ("commercial_hub", "district", 0.5),
        ("commercial_hub", "pamukkale", 2.0),
        ("commercial_hub", "government_plaza", 1.0),
        ("harbor", "coast_resource", 1.0),
        ("harbor", "district", 0.5),
        ("harbor", "city_center", 2.0),
        ("harbor", "government_plaza", 1.0),
        ("theater_square", "wonder", 2.0),
        ("theater_square", "district", 0.5),
        ("theater_square", "entertainment_complex", 2.0),
        ("theater_square", "water_park", 2.0),
        ("theater_square", "pamukkale", 2.0),
        ("theater_square", "government_plaza", 1.0),
        ("industrial_zone", "quarry", 1.0),
        ("industrial_zone", "strategic_resource", 1.0),
        // Shipped Minel_HalfProduction: 1 Production per 2 adjacent Mines.
        ("industrial_zone", "mine", 0.5),
        ("industrial_zone", "lumber_mill", 0.5),
        ("industrial_zone", "district", 0.5),
        ("industrial_zone", "aqueduct", 2.0),
        ("industrial_zone", "canal", 2.0),
        ("industrial_zone", "dam", 2.0),
        ("industrial_zone", "government_plaza", 1.0),
        ("acropolis", "wonder", 2.0),
        ("acropolis", "district", 1.0),
        ("acropolis", "city_center", 1.0),
        ("acropolis", "entertainment_complex", 2.0),
        ("acropolis", "water_park", 2.0),
        ("acropolis", "pamukkale", 2.0),
        ("acropolis", "government_plaza", 1.0),
        ("observatory", "plantation", 2.0),
        ("observatory", "farm", 0.5),
        ("observatory", "district", 0.5),
        ("observatory", "great_barrier_reef", 2.0),
        ("observatory", "pamukkale", 2.0),
        ("observatory", "government_plaza", 1.0),
        ("seowon", "self", 4.0),
        ("seowon", "district", -1.0),
        ("seowon", "government_plaza", 1.0),
        ("thanh", "district", 2.0),
        ("suguba", "river", 2.0),
        ("suguba", "holy_site", 2.0),
        ("suguba", "district", 0.5),
        ("suguba", "pamukkale", 2.0),
        ("suguba", "government_plaza", 1.0),
        ("hansa", "commercial_hub", 2.0),
        ("hansa", "resource", 1.0),
        ("hansa", "district", 0.5),
        ("hansa", "aqueduct", 2.0),
        ("hansa", "canal", 2.0),
        ("hansa", "dam", 2.0),
        ("hansa", "government_plaza", 1.0),
        ("oppidum", "quarry", 2.0),
        ("oppidum", "strategic_resource", 2.0),
        ("oppidum", "government_plaza", 1.0),
    ];
    for &(district, source, value) in &expected {
        assert_eq!(
            adjacency_value(&game, district, source),
            value,
            "{district}/{source}"
        );
    }

    let expected_keys: BTreeSet<(&str, &str)> = expected
        .iter()
        .map(|(district, source, _)| (*district, *source))
        .collect();
    let inherited_replacements = ["lavra", "cothon", "royal_navy_dockyard"];
    let actual_keys: BTreeSet<(&str, &str)> = game
        .rules
        .districts
        .iter()
        .filter(|(district, _)| !inherited_replacements.contains(&district.as_str()))
        .flat_map(|(district, spec)| {
            spec.adjacency
                .keys()
                .map(move |source| (district.as_str(), source.as_str()))
        })
        .collect();
    assert_eq!(
        actual_keys, expected_keys,
        "the stock adjacency catalog must contain no missing or unexpected sources"
    );

    for (replacement, base) in [
        ("lavra", "holy_site"),
        ("cothon", "harbor"),
        ("royal_navy_dockyard", "harbor"),
    ] {
        assert_eq!(
            game.rules.districts[replacement].adjacency, game.rules.districts[base].adjacency,
            "{replacement} must inherit {base} adjacency"
        );
    }
}

#[test]
fn adjacency_rounding_policies_wonders_and_unique_families_are_runtime_correct() {
    let (mut game, city, position, ring) = controlled_game();
    game.map.tiles.get_mut(&ring[0]).unwrap().feature = Some(crate::name!("jungle"));
    game.map.tiles.get_mut(&ring[1]).unwrap().district = Some(crate::name!("encampment"));
    assert_eq!(
        game.district_yields(crate::name!("campus"), position).science,
        0.0,
        "one Rainforest and one district are separate half-point buckets"
    );
    game.map.tiles.get_mut(&ring[2]).unwrap().feature = Some(crate::name!("jungle"));
    game.map.tiles.get_mut(&ring[3]).unwrap().terrain = crate::name!("mountain");
    game.map.tiles.get_mut(&ring[4]).unwrap().feature = Some(crate::name!("reef"));
    game.map.tiles.get_mut(&ring[5]).unwrap().feature = Some(crate::name!("geothermal_fissure"));
    assert_eq!(game.district_yields(crate::name!("campus"), position).science, 6.0);
    game.players[0]
        .policies
        .insert(crate::name!("natural_philosophy"));
    assert_eq!(game.district_yields(crate::name!("campus"), position).science, 12.0);

    game.players[0].policies.clear();
    for neighbor in &ring {
        let tile = game.map.tiles.get_mut(neighbor).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.district = None;
        tile.wonder = None;
    }
    game.map.tiles.get_mut(&ring[0]).unwrap().wonder = Some(crate::name!("oracle"));
    assert_eq!(
        game.district_yields(crate::name!("theater_square"), position).culture,
        2.0
    );

    game.map.tiles.get_mut(&ring[0]).unwrap().wonder = None;
    game.map.tiles.get_mut(&ring[0]).unwrap().district = Some(crate::name!("suguba"));
    assert_eq!(
        game.district_yields(crate::name!("hansa"), position).production,
        2.0,
        "a unique Commercial Hub satisfies Hansa's family adjacency"
    );

    let center = game.cities[&city].pos;
    let harbor = game.nbrs(center)[0];
    for neighbor in game.nbrs(harbor) {
        game.map.tiles.get_mut(&neighbor).unwrap().district = None;
    }
    game.map.tiles.get_mut(&harbor).unwrap().owner_city = Some(city);
    let other = game
        .nbrs(harbor)
        .into_iter()
        .find(|neighbor| *neighbor != center)
        .unwrap();
    game.map.tiles.get_mut(&other).unwrap().district = Some(crate::name!("campus"));
    assert_eq!(
        game.district_yields(crate::name!("harbor"), harbor).gold,
        3.0,
        "the City Center counts as both its major source and a district"
    );
}

#[test]
fn neighborhood_and_preserve_housing_follow_tile_appeal() {
    let (mut game, _, position, ring) = controlled_game();
    assert_eq!(game.tile_appeal(position), 0);
    assert_eq!(game.district_housing("neighborhood", position), 4.0);
    assert_eq!(game.district_housing("preserve", position), 1.0);
    assert_eq!(game.district_housing("mbanza", position), 5.0);

    for neighbor in ring.iter().take(4) {
        game.map.tiles.get_mut(neighbor).unwrap().feature = Some(crate::name!("forest"));
    }
    assert_eq!(game.tile_appeal(position), 4);
    assert_eq!(game.district_housing("neighborhood", position), 6.0);
    assert_eq!(game.district_housing("preserve", position), 3.0);

    game.map.tiles.get_mut(&ring[0]).unwrap().pillaged = true;
    assert_eq!(game.tile_appeal(position), 3);
    assert_eq!(game.district_housing("neighborhood", position), 5.0);
}

/// The Water Mill names three resources and asks nothing of the tile
/// beyond carrying one: WATERMILL_* ship a RESOURCE_TYPE_MATCHES each for
/// Maize, Rice and Wheat with no improvement requirement. The Aquarium
/// pays a Reef, and separately a Coast tile that has a *visible* resource.
/// CIVVIS used to demand a Farm and then pay any Bonus resource, and to
/// pay Science for every coast tile whether or not anything was on it.
#[test]
fn water_mill_and_aquarium_pay_the_tiles_their_rows_name() {
    let (mut game, city, position, _) = controlled_game();
    // Measure the plot rows where they are computed. Going through
    // city_yields would fold in each building's own city yields and only
    // count tiles the city happens to be working.
    let yields = |game: &Game, position: Pos| {
        game.player_tile_yields(0, position, &game.map.tiles[&position])
    };
    let shape = |game: &mut Game, terrain: &str, resource: Option<&str>| {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = Name::new(terrain);
        tile.feature = None;
        tile.improvement = None;
        tile.resource = resource.map(Name::new);
    };

    shape(&mut game, "plains", Some("wheat"));
    let wheat_bare = yields(&game, position).food;
    shape(&mut game, "plains", Some("stone"));
    let stone_bare = yields(&game, position).food;
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("water_mill"));
    shape(&mut game, "plains", Some("wheat"));
    assert_eq!(
        yields(&game, position).food - wheat_bare,
        1.0,
        "unimproved Wheat earns it: the rows name the resource, not a Farm"
    );
    shape(&mut game, "plains", Some("stone"));
    assert_eq!(
        yields(&game, position).food - stone_bare,
        0.0,
        "a Bonus resource the card does not name earns nothing"
    );

    // The Aquarium half of this change - Reef separately from a Coast tile
    // with a visible resource - is evidenced by AQUARIUM_REEF_SCIENCE and
    // AQUARIUM_SEARESOURCE_SCIENCE but is not covered here: it is a Harbor
    // building, so exercising it needs a standing Harbor that this fixture
    // does not build.
}

/// Public Transport pays per Neighborhood and bands its Food and
/// Production by that district's own tile Appeal.
/// PUBLICTRANSPORT_NEIGHBORHOOD_GOLD is unconditional; the Charming rows
/// require PLOT_IS_APPEAL_BETWEEN MinimumAppeal 2 and the Breathtaking
/// rows another at 4, and the two bands stack. CIVVIS used to pay a flat
/// +3 Food and +1 Production once per city with any Neighborhood,
/// whatever its Appeal, and no Gold at all.
#[test]
fn public_transport_pays_each_neighborhood_by_its_appeal() {
    let (mut game, city, position, ring) = controlled_game();
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("neighborhood"), position);
    game.map.tiles.get_mut(&position).unwrap().district = Some(crate::name!("neighborhood"));
    let bare = game.city_yields(city);
    game.players[0]
        .policies
        .insert(crate::name!("public_transport"));

    // Appeal 0 is below Charming: the Gold is still paid, the rest is not.
    assert_eq!(game.tile_appeal(position), 0);
    let plain = game.city_yields(city);
    assert_eq!(plain.gold - bare.gold, 1.0);
    assert_eq!(plain.food, bare.food);
    assert_eq!(plain.production, bare.production);

    // Four adjacent Woods lift it to Breathtaking, so both bands pay.
    for neighbor in ring.iter().take(4) {
        game.map.tiles.get_mut(neighbor).unwrap().feature = Some(crate::name!("forest"));
    }
    assert_eq!(game.tile_appeal(position), 4);
    let lifted = game.city_yields(city);
    assert_eq!(lifted.food - bare.food, 4.0);
    assert_eq!(lifted.production - bare.production, 2.0);

    // Pillaging one drops it to Charming: the Breathtaking half goes.
    game.map.tiles.get_mut(&ring[0]).unwrap().pillaged = true;
    assert_eq!(game.tile_appeal(position), 3);
    let charming = game.city_yields(city);
    assert_eq!(charming.food - bare.food, 3.0);
    assert_eq!(charming.production - bare.production, 1.0);
}

#[test]
fn pillaged_district_disables_its_building_yields_points_and_route_capacity() {
    let (mut game, city, position, ring) = controlled_game();
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), position);
    game.map.tiles.get_mut(&position).unwrap().district = Some(crate::name!("campus"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("library"));
    game.cities.get_mut(&city).unwrap().pop = 1;

    let mut active = game.clone();
    active.process_great_people(0);
    assert_eq!(active.players[0].gpp.get("scientist"), Some(&2.0));
    game.map.tiles.get_mut(&position).unwrap().pillaged = true;
    let inactive_yields = game.city_yields(city);
    game.process_great_people(0);
    assert_eq!(game.players[0].gpp.get("scientist"), None);
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .retain(|building| building != "library");
    assert_eq!(game.city_yields(city), inactive_yields);

    let commercial = ring[0];
    game.map.tiles.get_mut(&commercial).unwrap().pillaged = false;
    game.map.tiles.get_mut(&commercial).unwrap().district = Some(crate::name!("commercial_hub"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("commercial_hub"), commercial);
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("market"));
    game.players[0].civics.insert(crate::name!("foreign_trade"));
    // Foreign Trade, the Commercial Hub itself, and the Market it hosts.
    assert_eq!(game.trade_capacity(0), 3);
    assert_eq!(game.route_yields(city, false).gold, 6.0);
    game.map.tiles.get_mut(&commercial).unwrap().pillaged = true;
    assert_eq!(game.trade_capacity(0), 1);
    // The route row is for the district's existence, not its working
    // state: a route made to Aquileia while its Diplomatic Quarter lay
    // pillaged paid the Quarter's Food and Production from its first turn
    // (run civvis-20260816T200454Z, t144), and Rome's pillaged Holy Site
    // and Campus never stopped paying Cumae's.
    assert_eq!(game.route_yields(city, false).gold, 6.0);
}

#[test]
fn specialty_threshold_bonuses_ignore_repeatable_green_districts() {
    let (mut game, city, position, ring) = controlled_game();
    for (district, site) in [
        ("neighborhood", position),
        ("canal", ring[0]),
        ("spaceport", ring[1]),
    ] {
        game.map.tiles.get_mut(&site).unwrap().district = Some(Name::new(district));
        game.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(Name::new(district), site);
    }
    let base_housing = game.city_housing(&game.cities[&city]);
    let base_amenities = game.city_local_amenities(&game.cities[&city]);
    game.players[0].policies.extend([
        crate::name!("insulae"),
        crate::name!("liberalism"),
        crate::name!("new_deal"),
    ]);
    assert_eq!(game.city_specialty_district_count(&game.cities[&city]), 0);
    assert_eq!(game.city_housing(&game.cities[&city]), base_housing);
    assert_eq!(
        game.city_local_amenities(&game.cities[&city]),
        base_amenities
    );

    for (district, site) in [("campus", ring[2]), ("holy_site", ring[3])] {
        game.map.tiles.get_mut(&site).unwrap().district = Some(Name::new(district));
        game.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(Name::new(district), site);
    }
    assert_eq!(game.city_specialty_district_count(&game.cities[&city]), 2);
    let mut without_threshold_policies = game.clone();
    without_threshold_policies.players[0].policies.clear();
    assert_eq!(
        game.city_housing(&game.cities[&city]),
        without_threshold_policies.city_housing(&without_threshold_policies.cities[&city])
            + 1.0
    );
    assert_eq!(
        game.city_local_amenities(&game.cities[&city]),
        without_threshold_policies
            .city_local_amenities(&without_threshold_policies.cities[&city])
            + 1
    );

    game.map.tiles.get_mut(&ring[4]).unwrap().district = Some(crate::name!("theater_square"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), ring[4]);
    assert_eq!(game.city_specialty_district_count(&game.cities[&city]), 3);
    let mut without_threshold_policies = game.clone();
    without_threshold_policies.players[0].policies.clear();
    assert_eq!(
        game.city_housing(&game.cities[&city]),
        without_threshold_policies.city_housing(&without_threshold_policies.cities[&city])
            + 5.0
    );
    assert_eq!(
        game.city_local_amenities(&game.cities[&city]),
        without_threshold_policies
            .city_local_amenities(&without_threshold_policies.cities[&city])
            + 3
    );

    game.players[0].government = Some("digital_democracy".to_string());
    assert_eq!(game.gov_effects(0).culture_per_district, 2.0);
    assert_eq!(
        game.gov_effects(0).culture_per_district
            * game.city_specialty_district_count(&game.cities[&city]) as f64,
        6.0
    );
}

#[test]
fn naval_passage_uses_active_district_and_wonder_effects() {
    let (mut game, city, position, _) = controlled_game();
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("canal"), position);
    game.map.tiles.get_mut(&position).unwrap().district = Some(crate::name!("canal"));
    let galley = game.spawn_unit("galley", 0, position);
    assert!(game.unit_can_traverse(galley, position));

    game.map.tiles.get_mut(&position).unwrap().pillaged = true;
    assert!(!game.unit_can_traverse(galley, position));
    game.map.tiles.get_mut(&position).unwrap().district = None;
    game.map.tiles.get_mut(&position).unwrap().wonder = Some(crate::name!("panama_canal"));
    assert!(game.unit_can_traverse(galley, position));
}

#[test]
fn aircraft_slots_belong_to_the_city_center_and_active_aerodrome_tile() {
    let (mut game, city, aerodrome, _) = controlled_game();
    let center = game.cities[&city].pos;
    game.map.tiles.get_mut(&aerodrome).unwrap().district = Some(crate::name!("aerodrome"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("aerodrome"), aerodrome);

    assert_eq!(game.air_capacity_at(0, center), 1);
    assert_eq!(game.air_capacity_at(0, aerodrome), 2);
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .extend([crate::name!("hangar"), crate::name!("airport")]);
    assert_eq!(game.air_capacity_at(0, center), 1);
    assert_eq!(
        game.air_capacity_at(0, aerodrome),
        4,
        "Gathering Storm adds one slot from each Aerodrome building"
    );

    let biplane = game.place_new_unit("biplane", 0, center).unwrap();
    assert_eq!(game.units[&biplane].pos, aerodrome);
    game.map.tiles.get_mut(&aerodrome).unwrap().pillaged = true;
    assert_eq!(game.air_capacity_at(0, aerodrome), 0);
    assert_eq!(game.air_capacity_at(0, center), 1);
}

#[test]
fn grove_and_sanctuary_yields_follow_appeal_and_unimproved_rules() {
    let (mut game, city, preserve, ring) = controlled_game();
    let target = ring[0];
    for position in game.nbrs(target) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.improvement = None;
        tile.wonder = None;
    }
    {
        let tile = game.map.tiles.get_mut(&preserve).unwrap();
        tile.district = Some(crate::name!("preserve"));
        tile.owner_city = Some(city);
        tile.pillaged = false;
    }
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("preserve"), preserve);
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .extend([crate::name!("grove"), crate::name!("sanctuary")]);
    game.map.tiles.get_mut(&target).unwrap().owner_city = Some(city);

    let appeal_sources: Vec<Pos> = game
        .nbrs(target)
        .into_iter()
        .filter(|position| *position != preserve)
        .take(3)
        .collect();
    game.map.tiles.get_mut(&appeal_sources[0]).unwrap().feature = Some(crate::name!("forest"));
    assert_eq!(game.tile_appeal(target), 2);
    let base = game.rules.tile_yields(&game.map.tiles[&target]);
    let charming = game.player_tile_yields(0, target, &game.map.tiles[&target]);
    assert_eq!(
        charming,
        Yields {
            food: base.food + 1.0,
            production: base.production,
            gold: base.gold + 1.0,
            science: base.science + 1.0,
            culture: base.culture,
            faith: base.faith + 1.0,
        }
    );

    for position in appeal_sources.iter().skip(1) {
        game.map.tiles.get_mut(position).unwrap().feature = Some(crate::name!("forest"));
    }
    assert_eq!(game.tile_appeal(target), 4);
    let breathtaking = game.player_tile_yields(0, target, &game.map.tiles[&target]);
    assert_eq!(
        breathtaking,
        Yields {
            food: base.food + 2.0,
            production: base.production + 2.0,
            gold: base.gold + 2.0,
            science: base.science + 2.0,
            culture: base.culture + 2.0,
            faith: base.faith + 2.0,
        },
        "Breathtaking replaces rather than stacks the Charming package"
    );

    game.map.tiles.get_mut(&target).unwrap().improvement = Some(crate::name!("farm"));
    let improved = game.player_tile_yields(0, target, &game.map.tiles[&target]);
    let buildings = std::mem::take(&mut game.cities.get_mut(&city).unwrap().buildings);
    let improved_without_preserve_buildings =
        game.player_tile_yields(0, target, &game.map.tiles[&target]);
    game.cities.get_mut(&city).unwrap().buildings = buildings;
    assert_eq!(improved, improved_without_preserve_buildings);

    game.map.tiles.get_mut(&target).unwrap().improvement = None;
    game.map.tiles.get_mut(&preserve).unwrap().pillaged = true;
    assert_eq!(
        game.player_tile_yields(0, target, &game.map.tiles[&target]),
        base,
        "a pillaged Preserve cannot project Grove or Sanctuary yields"
    );
}

#[test]
fn district_completion_applies_government_envoy_and_culture_bomb_effects() {
    let mut game = Game::new_full(1, 20, 14, 5153, 300, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    let center = game.cities[&city].pos;
    for position in game.cities[&city].owned_tiles.clone() {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
    }
    game.players[0].civics.extend([
        crate::name!("state_workforce"),
        crate::name!("diplomatic_service"),
        crate::name!("mysticism"),
    ]);

    let plaza = game.district_sites(city, crate::name!("government_plaza"))[0];
    assert!(game.complete_item(
        0,
        city,
        &Item::District {
            district: crate::name!("government_plaza"),
            pos: plaza,
        },
    ));
    // One from the civic the fixture researches, one from the Plaza.
    assert_eq!(game.governor_titles(0), 2);

    game.cities.get_mut(&city).unwrap().pop = 4;
    let quarter = game
        .district_sites(city, crate::name!("diplomatic_quarter"))
        .into_iter()
        .find(|position| game.wdist(*position, center) == 1)
        .unwrap();
    assert!(game.complete_item(
        0,
        city,
        &Item::District {
            district: crate::name!("diplomatic_quarter"),
            pos: quarter,
        },
    ));
    assert_eq!(game.players[0].envoys_free, 1);

    game.cities.get_mut(&city).unwrap().pop = 7;
    let preserve = game
        .wdisk(center, 2)
        .into_iter()
        .find(|position| game.wdist(*position, center) == 2)
        .unwrap();
    {
        let tile = game.map.tiles.get_mut(&preserve).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.owner_city = Some(city);
    }
    if !game.cities[&city].owned_tiles.contains(&preserve) {
        game.cities
            .get_mut(&city)
            .unwrap()
            .owned_tiles
            .push(preserve);
    }
    let claim = game
        .nbrs(preserve)
        .into_iter()
        .find(|position| game.map.tiles[position].owner_city.is_none())
        .unwrap();
    assert!(game.complete_item(
        0,
        city,
        &Item::District {
            district: crate::name!("preserve"),
            pos: preserve,
        },
    ));
    assert_eq!(game.map.tiles[&claim].owner_city, Some(city));
    assert!(game.cities[&city].owned_tiles.contains(&claim));
}

#[test]
fn gaul_specialty_districts_culture_bomb_from_nonadjacent_sites() {
    let mut game = Game::new_full(1, 20, 14, 5154, 100, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    game.players[0].civ = "Gaul".to_string();
    game.players[0].techs.insert(crate::name!("writing"));
    let center = game.cities[&city].pos;
    let site = game
        .wdisk(center, 2)
        .into_iter()
        .find(|position| game.wdist(*position, center) == 2)
        .unwrap();
    {
        let tile = game.map.tiles.get_mut(&site).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.hills = false;
        tile.owner_city = Some(city);
    }
    if !game.cities[&city].owned_tiles.contains(&site) {
        game.cities.get_mut(&city).unwrap().owned_tiles.push(site);
    }
    let claim = game
        .nbrs(site)
        .into_iter()
        .find(|position| game.map.tiles[position].owner_city.is_none())
        .unwrap();
    assert!(game.district_sites(city, crate::name!("campus")).contains(&site));

    assert!(game.complete_item(
        0,
        city,
        &Item::District {
            district: crate::name!("campus"),
            pos: site,
        },
    ));
    assert_eq!(game.map.tiles[&claim].owner_city, Some(city));
    assert!(game.cities[&city].owned_tiles.contains(&claim));
}

#[test]
fn district_costs_scale_with_tree_progress_and_spaceports_remain_fixed() {
    let mut game = Game::new_full(1, 20, 14, 5155, 100, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    let position = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    game.players[0].techs.clear();
    game.players[0].civics.clear();

    let campus = Item::District {
        district: crate::name!("campus"),
        pos: position,
    };
    let spaceport = Item::District {
        district: crate::name!("spaceport"),
        pos: position,
    };
    assert_eq!(game.item_cost_for_city(0, city, &campus), 54.0);
    assert_eq!(game.item_cost_for_city(0, city, &spaceport), 1_800.0);

    game.players[0].techs.extend([
        crate::name!("pottery"),
        crate::name!("animal_husbandry"),
        crate::name!("mining"),
        crate::name!("sailing"),
        crate::name!("astrology"),
    ]);
    assert_eq!(
        game.item_cost_for_city(0, city, &campus),
        83.0,
        "5 of 77 technologies truncate to 6% tree progress"
    );

    game.players[0].techs = game.rules.techs.keys().cloned().collect();
    game.players[0].civics = game.rules.civics.keys().cloned().collect();
    assert_eq!(game.item_cost_for_city(0, city, &campus), 540.0);
    assert_eq!(game.item_cost_for_city(0, city, &spaceport), 1_800.0);
}

#[test]
fn underbuilt_specialty_districts_receive_the_stock_discount() {
    let mut game = Game::new_full(1, 20, 14, 5156, 100, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    game.players[0].civ = "Rome".to_string();
    game.players[0].techs.clear();
    game.players[0].civics.clear();
    game.players[0]
        .techs
        .extend([crate::name!("writing"), crate::name!("astrology")]);
    let positions: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != game.cities[&city].pos)
        .take(3)
        .collect();
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), positions[0]);
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), positions[1]);

    let holy_site = Item::District {
        district: crate::name!("holy_site"),
        pos: positions[2],
    };
    let progress = (100.0 * 2.0 / game.rules.techs.len() as f64).floor() / 100.0;
    let scaled = (54.0 * (1.0 + 9.0 * progress)).floor();
    assert_eq!(
        game.item_cost_for_city(0, city, &holy_site),
        (scaled * 0.6).floor()
    );
    assert_eq!(
        game.district_cost_for_placement(0, "holy_site", true),
        scaled,
        "Reyna and Moksha cannot discount an empire's first district of a type"
    );

    game.players[0].civics.insert(crate::name!("mysticism"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), positions[2]);
    let preserve_scaled = (54.0 * (1.0 + 9.0 * progress)).floor();
    assert_eq!(
        game.district_cost_for_placement(0, "preserve", false),
        preserve_scaled,
        "Preserves participate in A/B/C but cannot themselves receive the discount"
    );
}

#[test]
fn placed_districts_lock_cost_occupy_capacity_and_resume_after_research() {
    let mut game = Game::new_full(1, 20, 14, 5157, 100, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    game.players[0].civ = "Rome".to_string();
    game.players[0].techs.clear();
    game.players[0].civics.clear();
    game.players[0]
        .techs
        .extend([crate::name!("writing"), crate::name!("mining")]);
    let position = game.district_sites(city, crate::name!("campus"))[0];
    {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = Some(crate::name!("forest"));
        tile.improvement = Some(crate::name!("lumber_mill"));
        tile.resource = None;
    }
    let campus = Item::District {
        district: crate::name!("campus"),
        pos: position,
    };
    game.do_produce(0, city, &campus).unwrap();
    let locked_cost = game.item_cost_for_city(0, city, &campus);
    let foundation = game.map.tiles[&position]
        .district_foundation
        .as_ref()
        .unwrap();
    assert_eq!(foundation.district, "campus");
    assert_eq!(foundation.cost, locked_cost);
    assert!(game.map.tiles[&position].district.is_none());
    assert!(game.map.tiles[&position].feature.is_none());
    assert!(game.map.tiles[&position].improvement.is_none());
    assert_eq!(
        game.player_tile_yields(0, position, &game.map.tiles[&position]),
        Yields::default()
    );
    assert!(!game.city_has_district_family(&game.cities[&city], crate::name!("campus")));
    assert_eq!(game.district_sites(city, crate::name!("campus")), vec![position]);
    assert!(game.district_sites(city, crate::name!("holy_site")).is_empty());
    assert!(!game
        .city_citizen_plan(city)
        .worked_tiles
        .contains(&position));
    assert!(game.valid_improvements(0, position).is_empty());

    let saved = serde_json::to_value(&game).unwrap();
    let restored: Game = serde_json::from_value(saved).unwrap();
    assert_eq!(
        restored.map.tiles[&position]
            .district_foundation
            .as_ref()
            .unwrap()
            .cost,
        locked_cost
    );

    game.players[0].techs = game.rules.techs.keys().cloned().collect();
    game.players[0].civics = game.rules.civics.keys().cloned().collect();
    assert_eq!(game.item_cost_for_city(0, city, &campus), locked_cost);
    assert!(game.can_produce(0, city, &campus));

    assert!(game.complete_item(0, city, &campus));
    assert!(game.map.tiles[&position].district_foundation.is_none());
    assert_eq!(
        game.map.tiles[&position].district,
        Some(crate::name!("campus"))
    );
    assert!(game.city_has_district_family(&game.cities[&city], crate::name!("campus")));
}

#[test]
fn special_placement_rules_cover_land_water_features_and_city_distance() {
    let mut game = Game::new_full(1, 20, 14, 5154, 300, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    let center = game.cities[&city].pos;
    game.cities.get_mut(&city).unwrap().pop = 100;
    game.players[0].techs = game.rules.techs.keys().cloned().collect();
    game.players[0].civics = game.rules.civics.keys().cloned().collect();
    let owned = game.wdisk(center, 3);
    game.cities.get_mut(&city).unwrap().owned_tiles = owned.clone();
    for position in owned {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.owner_city = Some(city);
        tile.river_edges = [false; 6];
    }
    let near = game.nbrs(center)[0];
    let far = game
        .wdisk(center, 2)
        .into_iter()
        .find(|position| game.wdist(*position, center) == 2)
        .unwrap();
    let is_site = |game: &Game, district: &str, position: Pos| {
        game.district_sites(city, Name::new(district)).contains(&position)
    };

    assert!(!is_site(&game, "encampment", near));
    assert!(is_site(&game, "encampment", far));
    assert!(!is_site(&game, "preserve", near));
    assert!(is_site(&game, "preserve", far));

    game.map.tiles.get_mut(&far).unwrap().hills = true;
    assert!(is_site(&game, "acropolis", far));
    assert!(!is_site(&game, "aerodrome", far));
    assert!(!is_site(&game, "spaceport", far));
    game.map.tiles.get_mut(&far).unwrap().hills = false;
    assert!(is_site(&game, "aerodrome", far));
    assert!(is_site(&game, "spaceport", far));

    game.map.tiles.get_mut(&far).unwrap().terrain = crate::name!("lake");
    assert!(is_site(&game, "harbor", far));
    assert!(is_site(&game, "water_park", far));
    game.map.tiles.get_mut(&far).unwrap().terrain = crate::name!("coast");
    assert!(is_site(&game, "water_park", far));
    game.map.tiles.get_mut(&far).unwrap().feature = Some(crate::name!("reef"));
    assert!(!is_site(&game, "harbor", far));
    assert!(!is_site(&game, "water_park", far));

    {
        let tile = game.map.tiles.get_mut(&far).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = Some(crate::name!("grassland_floodplains"));
    }
    assert!(
        is_site(&game, "campus", far),
        "Gathering Storm allows ordinary districts on Floodplains"
    );
    let far_neighbors = game.nbrs(far);
    assert!(game.map.set_river_edge(far, far_neighbors[0], true));
    assert!(game.map.set_river_edge(far, far_neighbors[1], true));
    assert!(is_site(&game, "dam", far));

    game.map.tiles.get_mut(&far_neighbors[0]).unwrap().district = Some(crate::name!("dam"));
    assert!(
        !is_site(&game, "dam", far),
        "a placed Dam reserves its connected river"
    );
    game.map.tiles.get_mut(&far_neighbors[0]).unwrap().district = None;
    game.map.clear_rivers();
    assert!(game.map.set_river_edge(far, far_neighbors[0], true));
    assert!(game.map.set_river_edge(far, far_neighbors[3], true));
    assert!(
        !is_site(&game, "dam", far),
        "two unrelated rivers touching opposite edges are not one traversing river"
    );

    let center_edge = game.map.direction_to(near, center).unwrap();
    game.map.tiles.get_mut(&near).unwrap().feature = None;
    let water_neighbor = game.nbrs(near)[(center_edge + 1) % 6];
    assert!(game.map.set_river_edge(near, water_neighbor, true));
    assert!(is_site(&game, "aqueduct", near));
    assert!(!is_site(&game, "aqueduct", far));
}

#[test]
fn repeatable_districts_preserve_every_position_and_stack_local_effects() {
    let mut game = Game::new_full(1, 20, 14, 51_541, 300, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    let center = game.cities[&city].pos;
    game.cities.get_mut(&city).unwrap().pop = 100;
    game.players[0].techs = game.rules.techs.keys().cloned().collect();
    game.players[0].civics = game.rules.civics.keys().cloned().collect();
    let owned = game.wdisk(center, 3);
    game.cities.get_mut(&city).unwrap().owned_tiles = owned.clone();
    for position in owned {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
        tile.owner_city = Some(city);
    }

    let housing_before = game.city_housing(&game.cities[&city]);
    let neighborhood_positions: Vec<Pos> = game
        .district_sites(city, crate::name!("neighborhood"))
        .into_iter()
        .take(2)
        .collect();
    assert_eq!(neighborhood_positions.len(), 2);
    for position in &neighborhood_positions {
        assert!(game.complete_item(
            0,
            city,
            &Item::District {
                district: crate::name!("neighborhood"),
                pos: *position,
            },
        ));
    }
    assert_eq!(
        game.cities[&city].districts.positions(crate::name!("neighborhood")),
        neighborhood_positions.as_slice()
    );
    let neighborhood_housing: f64 = neighborhood_positions
        .iter()
        .map(|position| game.district_housing("neighborhood", *position))
        .sum();
    assert_eq!(
        game.city_housing(&game.cities[&city]) - housing_before,
        neighborhood_housing
    );

    let spaceport_position = game.district_sites(city, crate::name!("spaceport"))[0];
    assert!(game.complete_item(
        0,
        city,
        &Item::District {
            district: crate::name!("spaceport"),
            pos: spaceport_position,
        },
    ));
    assert!(game
        .district_sites(city, crate::name!("spaceport"))
        .into_iter()
        .any(|position| position != spaceport_position));

    let restored: Game = serde_json::from_value(serde_json::to_value(&game).unwrap()).unwrap();
    assert_eq!(
        restored.cities[&city].districts.positions(crate::name!("neighborhood")),
        neighborhood_positions.as_slice()
    );
    assert_eq!(restored.cities[&city].districts.len(), 3);
}

#[test]
fn barbarian_camps_open_with_fortified_guards_and_recon() {
    let game = Game::new_full(2, 44, 30, 88_100, 100, 0, true);
    let barbarian = game.barb_pid.unwrap();

    assert_eq!(game.barb_camp_guards.len(), game.barb_camps.len());
    assert_eq!(game.barb_scout_homes.len(), game.barb_camps.len());
    for camp in game.barb_camps.keys() {
        let guard_id = game
            .barb_camp_guards
            .get(camp)
            .copied()
            .expect("every camp has a standing guard");
        let guard = &game.units[&guard_id];
        assert_eq!(guard.owner, barbarian);
        assert_eq!(guard.pos, *camp);
        assert_eq!(guard.kind, "spearman");
        assert_eq!(
            game.rules.units[&guard.kind].promotion_class,
            "anti_cavalry"
        );
        assert!(guard.fortified);
        assert_eq!(guard.moves_left, 0.0);
        assert_eq!(guard.fortify_turns, 1);

        let recon_id = game
            .barb_scout_homes
            .iter()
            .find_map(|(unit, home)| (*home == *camp).then_some(*unit))
            .expect("every camp has a recon unit");
        let recon = &game.units[&recon_id];
        assert_eq!(recon.owner, barbarian);
        let is_naval = game.barb_naval_camps.contains(camp);
        assert_eq!(recon.kind == "galley", is_naval);
        assert_eq!(
            game.rules.is_water(&game.map.tiles[&recon.pos]),
            is_naval,
            "naval recon must live on water and land recon on land"
        );
        if is_naval {
            assert!(
                game.nbrs(*camp).into_iter().any(|neighbor| {
                    game.map
                        .get(neighbor)
                        .is_some_and(|tile| game.rules.is_water(tile))
                }),
                "a naval camp must be coastal"
            );
        }
    }

    let saved = serde_json::to_value(&game).unwrap();
    let restored: Game = serde_json::from_value(saved).unwrap();
    assert_eq!(restored.barb_naval_camps, game.barb_naval_camps);
    assert_eq!(restored.barb_camp_guards, game.barb_camp_guards);
    assert_eq!(restored.barb_scout_homes, game.barb_scout_homes);
}

#[test]
fn alerted_naval_barbarian_camps_reconnoiter_and_produce_ships() {
    let mut game = Game::new_full(1, 44, 30, 88_101, 100, 0, true);
    let barbarian = game.barb_pid.unwrap();

    // Strip the generated world to one controlled outpost, retaining the
    // real map so the test exercises the same land/water placement rules.
    for unit in game.player_unit_ids(barbarian) {
        game.remove_unit(unit);
    }
    for camp in game.barb_camps.keys().copied().collect::<Vec<_>>() {
        game.map.tiles.get_mut(&camp).unwrap().improvement = None;
    }
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    game.barb_camp_guards.clear();
    game.barb_scout_homes.clear();
    game.barb_scout_targets.clear();

    let camp = game
        .map
        .tiles
        .iter()
        .find_map(|(position, tile)| {
            if game.rules.is_water(tile)
                || !game.rules.is_passable(tile)
                || tile.owner_city.is_some()
                || game.city_at(*position).is_some()
                || !game.units_at(*position).is_empty()
            {
                return None;
            }
            let water_neighbors = game
                .nbrs(*position)
                .into_iter()
                .filter(|neighbor| {
                    game.map
                        .get(*neighbor)
                        .is_some_and(|tile| game.rules.is_water(tile))
                })
                .count();
            (water_neighbors >= 2).then_some(*position)
        })
        .expect("the generated map has a land tile with two water neighbors");
    game.map.tiles.get_mut(&camp).unwrap().improvement = Some(crate::name!("barbarian_camp"));
    game.barb_camps.insert(camp, game.turn);
    game.barb_naval_camps.insert(camp);
    game.spawn_barbarian_camp_units(camp);

    let recon_id = game
        .barb_scout_homes
        .iter()
        .find_map(|(unit, home)| (*home == camp).then_some(*unit))
        .unwrap();
    assert_eq!(game.units[&recon_id].kind, "galley");
    assert!(game.rules.is_water(&game.map.tiles[&game.units[&recon_id].pos]));

    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("the target civilization still has its opening Settler");
    let target = game.units[&settler].pos;
    game.found_city_for(0, target, None);
    game.barb_camp_targets.insert(camp, target);
    game.barb_alerted_until
        .insert(camp, game.turn + BARBARIAN_SCOUT_ALERT_TURNS);
    game.barb_camps.insert(camp, game.turn);

    let naval_before = game
        .player_unit_ids(barbarian)
        .into_iter()
        .filter(|unit| game.rules.units[&game.units[unit].kind].domain.as_deref() == Some("sea"))
        .count();
    game.barbarian_phase();
    let naval_after = game
        .player_unit_ids(barbarian)
        .into_iter()
        .filter(|unit| game.rules.units[&game.units[unit].kind].domain.as_deref() == Some("sea"))
        .count();
    assert!(naval_after > naval_before, "a naval camp should produce a ship");
}

#[test]
fn barbarian_scout_reports_a_city_and_alerts_its_home_camp() {
    let mut game = Game::new_full(2, 26, 16, 88_101, 100, 0, true);
    let barbarian = game.barb_pid.unwrap();
    let home = *game.barb_camps.keys().next().unwrap();
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let city_position = game.units[&settler].pos;
    game.found_city_for(0, city_position, None);
    let scout_position = game
        .nbrs(city_position)
        .into_iter()
        .find(|position| {
            game.map
                .get(*position)
                .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        })
        .unwrap();
    let scout = game.spawn_unit("scout", barbarian, scout_position);
    game.barb_scout_homes.insert(scout, home);

    game.barbarian_scout_phase(barbarian);
    assert_eq!(game.barb_scout_targets.get(&scout), Some(&city_position));

    let report_position = game
        .nbrs(home)
        .into_iter()
        .find(|position| game.map.tiles.contains_key(position))
        .unwrap();
    game.relocate(scout, report_position);
    game.barbarian_scout_phase(barbarian);
    assert_eq!(game.barb_camp_targets.get(&home), Some(&city_position));
    assert!(game.barb_alerted_until[&home] > game.turn);
    assert!(game.barb_camps[&home] <= game.turn + 1);
}

/// ★★★★★ A SETTLER WALKING PAST THE CAMP WAS NOT A SIGHTING.
///
/// The report accepted only a CITY, so a camp's whole raid throughput was one
/// Scout's round trip to a settlement — and the empire's WALKERS, which is
/// what a Civilization VI barbarian actually takes, could not start a raid at
/// all. MEASURED at 0.47 civilians lost to barbarians per game (`ai_eval`, 72
/// seat-games, 6p/150t/online) against **8 Settlers in 104 turns** on the live
/// Civilization VI seat. A tuning value inside the alert window cannot close
/// a seventeen-fold gap when the window mostly never opens.
///
/// The Scout still has to walk home before anything happens, so this is a
/// report and not a chase; the position it carries is where our people were
/// standing when it saw them.
#[test]
fn a_barbarian_scout_reports_the_settler_it_sees_with_no_city_anywhere() {
    let mut game = Game::new_full(2, 26, 16, 88_101, 100, 0, true);
    let barbarian = game.barb_pid.unwrap();
    let home = *game.barb_camps.keys().next().unwrap();
    game.barb_scout_homes.clear();
    game.barb_scout_targets.clear();
    // No city on the board at all: every opening Settler is removed, so the
    // only thing left to report is a walker.
    for unit in game
        .units
        .values()
        .filter(|unit| !game.players[unit.owner].is_barbarian)
        .map(|unit| unit.id)
        .collect::<Vec<_>>()
    {
        game.remove_unit(unit);
    }
    assert!(
        game.cities.is_empty(),
        "the fixture must have no settlement"
    );

    let road =
        game.map
            .tiles
            .keys()
            .copied()
            .filter(|position| {
                game.wdist(*position, home) > 2
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
                    && game.units_at(*position).is_empty()
            })
            .min()
            .expect("open ground away from the camp");
    let settler = game.spawn_unit("settler", 0, road);
    let scout_position = game
        .nbrs(road)
        .into_iter()
        .find(|position| {
            game.map
                .get(*position)
                .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
                && game.units_at(*position).is_empty()
        })
        .expect("open ground beside the Settler");
    let scout = game.spawn_unit("scout", barbarian, scout_position);
    game.barb_scout_homes.insert(scout, home);

    game.barbarian_scout_phase(barbarian);
    assert_eq!(
        game.barb_scout_targets.get(&scout),
        Some(&road),
        "a Scout standing next to an undefended Settler has seen something worth raiding"
    );

    // And the report still has to be carried home before the camp raises anybody.
    assert!(
        !game.barb_alerted_until.contains_key(&home),
        "a sighting alone must not alert the camp — the Scout walks back first"
    );
    let doorstep = game
        .nbrs(home)
        .into_iter()
        .find(|position| game.map.tiles.contains_key(position))
        .unwrap();
    game.relocate(scout, doorstep);
    game.barbarian_scout_phase(barbarian);
    assert_eq!(game.barb_camp_targets.get(&home), Some(&road));
    assert!(game.barb_alerted_until[&home] > game.turn);
    let _ = settler;
}

#[test]
fn barbarian_scouts_report_only_cities_they_can_really_see() {
    let mut game = Game::new_full(1, 26, 16, 88_102, 100, 0, true);
    let barbarian = game.barb_pid.expect("barbarian-enabled fixture");
    let home = *game.barb_camps.keys().next().expect("opening camp");
    game.barb_scout_homes.clear();
    game.barb_scout_targets.clear();

    // Discover a two-hex ray of open land, then put Woods in every legal
    // corridor between its endpoints. A distance-only check incorrectly
    // reports the city through cover; the Scout's actual vision does not.
    let (origin, target, blockers) = game
        .map
        .tiles
        .keys()
        .copied()
        .find_map(|origin| {
            game.nbrs(origin).into_iter().find_map(|first| {
                game.nbrs(first).into_iter().find_map(|target| {
                    if game.wdist(origin, target) != 2 {
                        return None;
                    }
                    let blockers: Vec<Pos> = game
                        .nbrs(origin)
                        .into_iter()
                        .filter(|middle| game.nbrs(*middle).contains(&target))
                        .collect();
                    let land = |position: Pos| {
                        game.map.get(position).is_some_and(|tile| {
                            game.rules.is_passable(tile)
                                && !game.rules.is_water(tile)
                                && tile.owner_city.is_none()
                                && game.city_at(position).is_none()
                                && game.units_at(position).is_empty()
                        })
                    };
                    (land(origin)
                        && land(target)
                        && !blockers.is_empty()
                        && blockers.iter().copied().all(land))
                    .then_some((origin, target, blockers))
                })
            })
        })
        .expect("fixture needs an unobstructed two-hex land ray");
    for position in blockers.iter().copied().chain([origin, target]) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
    }
    for blocker in &blockers {
        game.map.tiles.get_mut(blocker).unwrap().feature = Some(crate::name!("forest"));
    }
    game.found_city_for(0, target, Some("Hidden City".to_string()));
    let scout = game.spawn_unit("scout", barbarian, origin);
    game.barb_scout_homes.insert(scout, home);

    assert!(
        !game.unit_visible_tiles(scout).contains(&target),
        "the Woods block the Scout's direct view of the City Center"
    );
    game.barbarian_scout_phase(barbarian);
    assert!(
        !game.barb_scout_targets.contains_key(&scout),
        "a city behind terrain cover cannot start a report"
    );

    for blocker in blockers {
        game.map.tiles.get_mut(&blocker).unwrap().feature = None;
    }
    assert!(game.unit_visible_tiles(scout).contains(&target));
    game.barbarian_scout_phase(barbarian);
    assert_eq!(game.barb_scout_targets.get(&scout), Some(&target));
}

#[test]
fn a_scout_report_raises_one_finite_home_bound_raiding_party() {
    let mut game = Game::new_full(2, 44, 30, 88_103, 100, 0, true);
    let home = *game.barb_camps.keys().next().expect("opening camp");
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("opening Settler");
    let target = game.units[&settler].pos;
    game.found_city_for(0, target, Some("Raid Target".to_string()));

    // No other camp matters to the expected party. The report is the only
    // source of this camp's raiders, and the standing Scout is removed so it
    // cannot begin a second report after this test's alert expires.
    for scout in game
        .barb_scout_homes
        .iter()
        .filter_map(|(unit, camp)| (*camp == home).then_some(*unit))
        .collect::<Vec<_>>()
    {
        game.remove_unit(scout);
    }
    game.barb_camp_targets.insert(home, target);
    game.barb_alerted_until
        .insert(home, game.turn + BARBARIAN_SCOUT_ALERT_TURNS);
    game.barb_camps.insert(home, game.turn);

    let wanted = game.barbarian_raid_force_size();
    for _ in 0..20 {
        game.barbarian_phase();
        let raised = game
            .barb_raider_homes
            .values()
            .filter(|camp| **camp == home)
            .count();
        if raised == wanted {
            break;
        }
        game.turn += 1;
    }
    let party: Vec<u32> = game
        .barb_raider_homes
        .iter()
        .filter_map(|(unit, camp)| (*camp == home).then_some(*unit))
        .collect();
    assert_eq!(party.len(), wanted, "one report raises exactly one party");
    let ranged = party
        .iter()
        .filter(|unit| game.rules.units[game.units[unit].kind].has_ranged_attack())
        .count();
    assert_eq!(ranged, game.barbarian_raid_ranged_size());
    assert!(
        party
            .iter()
            .all(|unit| game.barbarian_unit_home(*unit) == Some(home)),
        "each raider remains assigned to the outpost that raised it"
    );

    let restored: Game = serde_json::from_value(serde_json::to_value(&game).unwrap()).unwrap();
    assert_eq!(restored.barb_raider_homes, game.barb_raider_homes);

    let alert_end = game.barb_alerted_until[&home];
    game.turn = alert_end;
    game.barbarian_phase();
    assert!(!game.barb_alerted_until.contains_key(&home));
    assert!(
        !game.barb_camp_targets.contains_key(&home),
        "the reported target ends with the raid window"
    );
    let raised = party.len();
    for _ in 0..8 {
        game.turn += 1;
        game.barbarian_phase();
    }
    assert_eq!(
        game.barb_raider_homes
            .values()
            .filter(|camp| **camp == home)
            .count(),
        raised,
        "an unalerted camp does not turn one report into an endless army"
    );
}

/// Camp placement reads its clearance off a disk around each city and
/// camp. Measuring instead from every land tile is what made a land-heavy
/// globe hang in setup: a sphere answers a distance past its cached ring
/// with an A* search of the world, and a third of the camp target is
/// placed before turn one. Twenty-one seats is where the size table moves
/// up to a globe large enough for that to matter — at twenty it still
/// finished in under two seconds, and at twenty-one it had not finished
/// after three minutes. The floors it may place under are unchanged.
#[test]
fn camps_keep_their_clearance_on_a_land_heavy_globe_without_searching_it() {
    let size = MapSize::for_players(21);
    let (width, height) = size.dimensions(MapTopology::Planet);
    let started = std::time::Instant::now();
    let game = Game::new_with(GameOptions {
        barbarians: true,
        map_script: MapScript::LandOnly,
        map_topology: MapTopology::Planet,
        ..GameOptions::new(21, width, height, 88_207, 100, 0)
    });
    let elapsed = started.elapsed();

    assert!(!game.barb_camps.is_empty(), "a land-heavy globe seats camps");
    for camp in game.barb_camps.keys() {
        for city in game.cities.values() {
            assert!(
                game.wdist(*camp, city.pos) >= 4,
                "camp {camp:?} sits {} from a city",
                game.wdist(*camp, city.pos)
            );
        }
        for other in game.barb_camps.keys().filter(|other| *other != camp) {
            assert!(
                game.wdist(*camp, *other) >= 7,
                "camps {camp:?} and {other:?} sit {} apart",
                game.wdist(*camp, *other)
            );
        }
    }
    assert!(
        elapsed < std::time::Duration::from_secs(120),
        "setting up a land-heavy globe took {elapsed:?}"
    );
}

/// An arena's economy is granted, never earned, and identically to both
/// sides: one city per side, producing exactly the flat Production figure
/// and no Food to grow on, flat Gold per side to upgrade with, and a
/// technology every `turns_per_tech` turns whatever era it is. Culture
/// stays at zero, so no civic ever completes and no policy ever lands
/// mid-battle. The grants are raised from the stock arena's zero here,
/// because it is the paying that is under test.
#[test]
fn an_arena_economy_is_granted_rather_than_earned() {
    let rules = TacticsRules {
        cities: 1,
        production: 30,
        gold: 30,
        turns_per_tech: 5,
        ..TacticsRules::default()
    };
    let mut game = Game::new_with(GameOptions {
        map_script: MapScript::Battlefield,
        tactics: rules,
        ..GameOptions::new(2, 10, 10, 90_411, 250, 0)
    });
    for seat in 0..2 {
        let cities: Vec<u32> = game
            .cities
            .values()
            .filter(|city| city.owner == seat)
            .map(|city| city.id)
            .collect();
        assert_eq!(cities.len(), 1, "seat {seat} opens with the one granted city");
        let yields = game.city_yields(cities[0]);
        assert_eq!(yields.production, 30.0, "the city pays the flat grant");
        for (name, value) in [
            ("food", yields.food),
            ("gold", yields.gold),
            ("science", yields.science),
            ("culture", yields.culture),
        ] {
            assert_eq!(value, 0.0, "an arena city pays no {name}");
        }
    }

    // A side that has met the tech pace has also banked its gold, and has
    // spent no Culture on a civics tree that is not there. Research is
    // chosen here rather than waited for: `begin_turn` pays the pace, the
    // AI turn picks the target, and this test is about the paying.
    let cheapest = game
        .rules
        .techs
        .keys()
        .filter(|name| !game.players[0].techs.contains(*name))
        .min_by(|a, b| {
            game.tech_cost(a.as_str())
                .total_cmp(&game.tech_cost(b.as_str()))
        })
        .copied()
        .expect("an opening arena has a tree left to climb");
    game.players[0].research = Some(cheapest.to_string());
    let techs_before = game.players[0].techs.len();
    let gold_before = game.players[0].gold;
    for _ in 0..6 {
        game.begin_turn(0);
    }
    assert!(
        game.players[0].gold >= gold_before + 5.0 * 30.0,
        "six turns of the flat grant must reach the treasury"
    );
    assert_eq!(game.players[0].culture_lifetime, 0.0, "an arena pays no Culture");
    assert!(game.players[0].civics.is_empty(), "no civic completes on an arena");
    assert!(
        game.players[0].techs.len() > techs_before,
        "a five-turn pace must land a technology inside six turns"
    );

    // The build menu is what fights and what helps it fight, and nothing
    // else — including at the no-city setting, where the rule still holds
    // for a city captured from the other side.
    let city = *game.cities.keys().next().unwrap();
    let owner = game.cities[&city].owner;
    for kind in ["warrior", "battering_ram"] {
        assert!(
            game.arena_allows_production(&Item::Unit { unit: Name::new(kind) }),
            "{kind} is a fighting unit and belongs on an arena"
        );
    }
    for kind in ["settler", "builder", "trader"] {
        assert!(
            !game.arena_allows_production(&Item::Unit { unit: Name::new(kind) }),
            "{kind} is empire-building and does not belong on an arena"
        );
    }
    assert!(
        !game.can_produce(owner, city, &Item::Building { building: crate::name!("monument") }),
        "an arena builds no buildings"
    );
    assert!(
        !game.can_produce(owner, city, &Item::Unit { unit: crate::name!("settler") }),
        "an arena builds no Settlers"
    );
}

/// The no-city arena still techs and still banks upgrade money: the grant
/// is per side, not per city, which is what keeps the zero setting a real
/// option rather than a side that can do nothing but walk forward. Gold
/// is granted here because the stock arena grants none.
#[test]
fn a_city_less_arena_still_collects_gold_and_science() {
    let mut game = Game::new_with(GameOptions {
        map_script: MapScript::Battlefield,
        tactics: TacticsRules { cities: 0, gold: 30, ..TacticsRules::default() },
        ..GameOptions::new(2, 10, 10, 90_411, 250, 0)
    });
    assert!(game.cities.is_empty());
    let cheapest = game
        .rules
        .techs
        .keys()
        .filter(|name| !game.players[0].techs.contains(*name))
        .min_by(|a, b| {
            game.tech_cost(a.as_str())
                .total_cmp(&game.tech_cost(b.as_str()))
        })
        .copied()
        .expect("an opening arena has a tree left to climb");
    game.players[0].research = Some(cheapest.to_string());
    let gold_before = game.players[0].gold;
    let techs_before = game.players[0].techs.len();
    for _ in 0..6 {
        game.begin_turn(0);
    }
    assert!(
        game.players[0].gold > gold_before,
        "a side with no city still collects its Gold"
    );
    assert!(
        game.players[0].techs.len() > techs_before,
        "a side with no city still researches"
    );
}

fn flag_arena(seed: u64) -> Game {
    Game::new_with(GameOptions {
        map_script: MapScript::Battlefield,
        // Cities asked for on purpose: flags must replace them.
        tactics: TacticsRules { flag: true, cities: 1, ..TacticsRules::default() },
        ..GameOptions::new(2, 20, 20, seed, 250, 0)
    })
}

/// A flag battle opens with one flag per side, standing where that
/// side's city would have stood, and no cities anywhere.
#[test]
fn a_flag_arena_gives_every_side_a_flag_of_its_own() {
    let game = flag_arena(90_412);
    assert!(game.cities.is_empty(), "flags replace the city objective outright");
    assert_eq!(game.arena_flags.len(), 2, "a flag each, not one in the middle");
    for seat in 0..2 {
        let flag = game.arena_flags[&seat];
        assert!(
            game.rules.is_passable(&game.map.tiles[&flag])
                && !game.rules.is_water(&game.map.tiles[&flag]),
            "seat {seat} holds a flag on ground no army could stand on"
        );
        // Its own army is the garrison: a side opens sitting on its flag.
        assert!(
            game.units_at(flag).iter().all(|uid| game.units[uid].owner == seat),
            "seat {seat}'s flag opens held by somebody else"
        );
    }
    // Symmetric: neither side opens nearer the enemy flag than the other.
    let march = |seat: usize| {
        let enemy = game.arena_enemy_flag(seat, game.arena_flags[&seat]).unwrap();
        game.wdist(game.arena_flags[&seat], enemy)
    };
    assert_eq!(march(0), march(1), "one side has a shorter run at the enemy flag");

    // Only the shape that asked for flags gets them.
    let plain = Game::new_with(GameOptions {
        map_script: MapScript::Battlefield,
        ..GameOptions::new(2, 20, 20, 90_412, 250, 0)
    });
    assert!(plain.arena_flags.is_empty());
    let world = Game::new_full(2, 24, 16, 90_412, 100, 0, false);
    assert!(world.arena_flags.is_empty());
}

/// Taking the ENEMY's flag wins. Standing on your own is a garrison and
/// decides nothing — otherwise every battle would end on turn one, with
/// both armies already deployed around their own flags.
#[test]
fn taking_the_enemy_flag_wins_and_holding_your_own_does_not() {
    let mut game = flag_arena(90_413);
    let ours = game.arena_flags[&0];
    let theirs = game.arena_flags[&1];

    // Seat 0 walks onto its own flag: nothing happens.
    let friendly = game
        .units
        .values()
        .find(|unit| unit.owner == 0)
        .expect("side one opens with an army")
        .id;
    game.relocate(friendly, ours);
    assert_eq!(game.winner, None, "a side cannot capture its own flag");
    assert!(!game.is_finished());

    // Now onto the enemy's, and the battle is over.
    game.relocate(friendly, theirs);
    assert_eq!(game.winner, Some(0), "the side that took the enemy flag has won");
    assert_eq!(game.victory_type.as_deref(), Some(FLAG_VICTORY));
    assert!(game.is_finished());
}

/// The enemy flag is what a side aims at, and it is the other side's.
#[test]
fn the_objective_is_the_other_sides_flag() {
    let game = flag_arena(90_415);
    for seat in 0..2 {
        let own = game.arena_flags[&seat];
        let target = game.arena_enemy_flag(seat, own).expect("an enemy flag to take");
        assert_ne!(target, own);
        assert_eq!(game.arena_flag_holder(target), Some(1 - seat));
    }
    // A world has no flags, so nothing to aim at.
    let world = Game::new_full(2, 24, 16, 90_415, 100, 0, false);
    assert_eq!(world.arena_enemy_flag(0, (0, 0)), None);
}

/// The flags survive a save, and a save written before the shape existed
/// still opens — with no flags rather than a refusal.
#[test]
fn the_flags_survive_a_save_and_older_saves_open_without_them() {
    let game = flag_arena(90_414);
    let encoded = serde_json::to_value(&game).unwrap();
    let restored: Game = serde_json::from_value(encoded).unwrap();
    assert_eq!(restored.arena_flags, game.arena_flags);

    let mut older = serde_json::to_value(&game).unwrap();
    older
        .as_object_mut()
        .unwrap()
        .remove("arena_flags")
        .expect("the field is written");
    let old_save: Game = serde_json::from_value(older).unwrap();
    assert!(old_save.arena_flags.is_empty());
}

/// A zero tech pace freezes the tree: both sides fight the whole battle
/// with the units their starting era gave them. This is the setting a
/// sweep uses to isolate one era's matchups, so it has to hold for the
/// length of a battle rather than merely start slow.
#[test]
fn a_zero_tech_pace_freezes_the_tree() {
    let mut game = Game::new_with(GameOptions {
        map_script: MapScript::Battlefield,
        tactics: TacticsRules { turns_per_tech: 0, gold: 30, ..TacticsRules::default() },
        ..GameOptions::new(2, 10, 10, 90_411, 250, 0)
    });
    let cheapest = game
        .rules
        .techs
        .keys()
        .filter(|name| !game.players[0].techs.contains(*name))
        .min_by(|a, b| {
            game.tech_cost(a.as_str())
                .total_cmp(&game.tech_cost(b.as_str()))
        })
        .copied()
        .expect("an opening arena has a tree left to climb");
    game.players[0].research = Some(cheapest.to_string());
    let techs_before = game.players[0].techs.len();
    let era_before = game.player_era(0);
    for _ in 0..40 {
        game.begin_turn(0);
    }
    assert_eq!(game.arena_side_yields(0).science, 0.0, "a zero pace pays no Science");
    assert_eq!(
        game.players[0].research_progress, 0.0,
        "a frozen tree banks no progress"
    );
    assert_eq!(
        game.players[0].techs.len(),
        techs_before,
        "no technology completes at a zero pace"
    );
    assert_eq!(game.player_era(0), era_before, "the era cannot move either");
    // Gold is a separate grant and keeps coming when it is granted: a
    // frozen tree still upgrades, it just cannot unlock anything new to
    // upgrade into.
    assert!(game.players[0].gold > 0.0);
}

/// The stock arena is two standing armies and no reinforcements. Each
/// side is dropped in with its company and its city, and then nothing:
/// the city collects no Production so it never banks a point toward a
/// unit, and the side banks no Gold so it never upgrades one. Whatever is
/// on the field at turn one is the whole battle. The grant is then raised
/// on the same game to show the same driver does pay when asked — so the
/// zero above is the arena's answer and not the test's.
#[test]
fn the_stock_arena_never_reinforces_either_side() {
    let mut game = Game::new_with(GameOptions {
        map_script: MapScript::Battlefield,
        ..GameOptions::new(2, 10, 10, 90_411, 250, 0)
    });
    assert_eq!(game.tactics, TacticsRules::default());
    let dealt: Vec<usize> = (0..2).map(|seat| game.player_unit_ids(seat).len()).collect();
    assert!(dealt.iter().all(|count| *count > 0), "each side opens with an army: {dealt:?}");
    assert_eq!(dealt[0], dealt[1], "and the same army: {dealt:?}");
    let mut cities = Vec::new();
    for seat in 0..2 {
        let held: Vec<u32> = game
            .cities
            .values()
            .filter(|city| city.owner == seat)
            .map(|city| city.id)
            .collect();
        assert_eq!(held.len(), 1, "seat {seat} still holds a city to defend");
        assert_eq!(
            game.city_yields(held[0]).production,
            0.0,
            "the stock city collects no Production"
        );
        assert_eq!(game.arena_side_yields(seat).gold, 0.0, "and the side banks no Gold");
        // Queue a Warrior anyway, so the test is about the paying and
        // not about whether the AI chose to build.
        game.cities.get_mut(&held[0]).unwrap().queue =
            vec![Item::Unit { unit: crate::name!("warrior") }];
        cities.push(held[0]);
    }
    let gold_before: Vec<f64> = (0..2).map(|seat| game.players[seat].gold).collect();
    // Thirty turns of the paying — long enough for a 30-Production city
    // to have finished several Warriors. The arena's own turn is what
    // pays cities and grants, so it is what is driven here.
    for _ in 0..30 {
        for seat in 0..2 {
            game.begin_turn(seat);
        }
    }
    for seat in 0..2 {
        assert_eq!(
            game.cities[&cities[seat]].production,
            0.0,
            "seat {seat} banked Production toward a unit it was never granted"
        );
        assert_eq!(
            game.player_unit_ids(seat).len(),
            dealt[seat],
            "seat {seat} fielded a unit it was not dealt"
        );
        assert_eq!(
            game.players[seat].gold, gold_before[seat],
            "seat {seat} banked Gold the stock arena does not grant"
        );
    }

    // Raised, the same grants reach the same city and the same treasury:
    // the setting is real, and zero was the default rather than the rule.
    game.tactics.production = 30;
    game.tactics.gold = 30;
    for _ in 0..3 {
        for seat in 0..2 {
            game.begin_turn(seat);
        }
    }
    for seat in 0..2 {
        assert!(
            game.cities[&cities[seat]].production > 0.0
                || game.player_unit_ids(seat).len() > dealt[seat],
            "seat {seat} was granted Production and neither banked nor spent it"
        );
        assert!(
            game.players[seat].gold > gold_before[seat],
            "seat {seat} was granted Gold and banked none"
        );
    }
}

/// A Tactics battlefield opens as a two-sided arena: flat and walled even
/// when a globe was asked for, the requested city-states refused, no
/// barbarian third force — and, at the no-city setting, two armies and
/// nothing else. Never a Settler, at either setting: the arena's city is
/// placed at setup or there is none. No difficulty handicap units either,
/// because an arena that hands one side a bonus has decided the battle at
/// setup. The two rosters are identical, set out over their own ends of
/// the field rather than stacked on one hex, and the two sides open at
/// war with each other having already met.
#[test]
fn a_battlefield_game_opens_as_a_two_sided_arena() {
    let game = Game::new_with(GameOptions {
        map_script: MapScript::Battlefield,
        map_topology: MapTopology::Planet,
        // Deity: the handicap table would hand every AI seat extra units.
        difficulty: "deity".to_string(),
        tactics: TacticsRules { cities: 0, ..TacticsRules::default() },
        ..GameOptions::new(2, 10, 10, 90_411, 250, 3)
    });
    assert!(game.map.sphere().is_none(), "the arena must be flat");
    assert_eq!((game.map.width, game.map.height), (10, 10));
    assert!(game.is_arena());
    assert!(game.barb_pid.is_none(), "no third force on a battlefield");
    // Two majors and the dormant Free Cities seat: the three city-states
    // asked for were refused by the arena.
    assert_eq!(game.players.len(), 3);
    assert!(game.players.iter().all(|player| !player.is_minor || player.is_free_city));
    assert!(game.cities.is_empty(), "the no-city arena opens with no city");

    let roster = |seat: usize| {
        let mut kinds: Vec<&str> = game
            .units
            .values()
            .filter(|unit| unit.owner == seat)
            .map(|unit| unit.kind.as_str())
            .collect();
        kinds.sort_unstable();
        kinds
    };
    assert_eq!(
        roster(0),
        Game::battlefield_army(game.map.tiles.len(), MapScript::Battlefield)
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .flat_map(|kind| {
                std::iter::repeat_n(
                    kind,
                    Game::battlefield_army(game.map.tiles.len(), MapScript::Battlefield)
                        .iter()
                        .filter(|other| **other == kind)
                        .count(),
                )
            })
            .collect::<Vec<_>>(),
        "seat 0 opens with the arena roster and nothing else"
    );
    assert_eq!(roster(0), roster(1), "the two armies must be even");
    assert!(
        !roster(0).contains(&"settler"),
        "no Settler is dropped onto an arena"
    );

    for seat in 0..2 {
        let held: std::collections::BTreeSet<Pos> = game
            .units
            .values()
            .filter(|unit| unit.owner == seat)
            .map(|unit| unit.pos)
            .collect();
        assert_eq!(
            held.len(),
            roster(seat).len(),
            "seat {seat} must be set out over the field, not stacked on one hex"
        );
    }
    // The two sides face each other across the field and are already
    // fighting: nothing on an arena has to be declared or discovered.
    assert!(game.is_at_war(0, 1));
    assert!(game.has_met(0, 1) && game.has_met(1, 0));
    // Nothing on the arena exists to be developed, and every hex of it is
    // ground both sides can walk.
    assert!(game.map.tiles.values().all(|tile| tile.resource.is_none()));
    assert!(game.map.tiles.values().all(|tile| game.rules.is_passable(tile)));
    assert!(game.map.tiles.values().all(|tile| !game.rules.is_water(tile)));
}

/// Which era arms a custom arena is the arena's own setting: one rung
/// fixes every battle, Random resolves through the shared per-seed roll
/// with the Future left out of the hat, and a Customize pool opens with
/// its latest rung researched while dealing both armies across the whole
/// spread — a Warrior company beside what an Ancient start could never
/// field. A named battle ignores all of it and keeps its own era.
#[test]
fn the_arena_era_choice_arms_the_battle() {
    use crate::setup::TacticsEra;
    let arena = |era: TacticsEra, seed: u64| {
        Game::new_with(GameOptions {
            map_script: MapScript::Battlefield,
            tactics: TacticsRules { cities: 0, era, ..TacticsRules::default() },
            ..GameOptions::new(2, 10, 10, seed, 250, 0)
        })
    };
    let roster = |game: &Game, seat: usize| {
        let mut kinds: Vec<Name> = game
            .units
            .values()
            .filter(|unit| unit.owner == seat)
            .map(|unit| unit.kind)
            .collect();
        kinds.sort_unstable();
        kinds
    };

    // One rung, every battle.
    let fixed = arena(TacticsEra::Fixed(4), 90_411);
    assert_eq!(fixed.start_era, 4);
    assert_eq!(fixed.world_era, 4);

    // Random is the shared roll, so the same seed replays the same
    // battle, the Future never comes up, and the seeds do scatter.
    for seed in [1u64, 2, 3, 500, 90_411] {
        let rolled = arena(TacticsEra::Random, seed);
        assert_eq!(rolled.start_era, crate::setup::random_battle_era(seed));
        assert!(
            rolled.start_era < crate::setup::start_era_from_id("future").unwrap(),
            "the Future stays out of the hat"
        );
    }
    let scattered: BTreeSet<usize> =
        (0..16).map(|seed| arena(TacticsEra::Random, seed).start_era).collect();
    assert!(scattered.len() > 1, "sixteen seeds must not all land on one era");

    // A pool of Ancient and Information opens with the Information era's
    // research and deals both sides the identical cross-era mix.
    let pooled = arena(TacticsEra::Pool(1 | 1 << 7), 90_411);
    assert_eq!(pooled.start_era, 7);
    assert_eq!(roster(&pooled, 0), roster(&pooled, 1), "the two armies must stay even");
    let ancient: BTreeSet<Name> = roster(&arena(TacticsEra::Fixed(0), 90_411), 0)
        .into_iter()
        .collect();
    let pooled_kinds: BTreeSet<Name> = roster(&pooled, 0).into_iter().collect();
    assert!(
        pooled_kinds.iter().any(|kind| ancient.contains(kind)),
        "the pool keeps its Ancient rung on the field: {pooled_kinds:?}"
    );
    assert!(
        pooled_kinds.iter().any(|kind| !ancient.contains(kind)),
        "and fields what an Ancient start never could: {pooled_kinds:?}"
    );

    // A named battle keeps the era it was fought in whatever the arena
    // control says: the server hands Gettysburg its own Industrial start
    // — see `new_game_params` — and the arena's era choice must not
    // reach past it, in either direction.
    for asked in [TacticsEra::Fixed(0), TacticsEra::Random, TacticsEra::Pool(1)] {
        let gettysburg = Game::new_with(GameOptions {
            map_script: MapScript::Gettysburg,
            start_era: 4,
            tactics: TacticsRules { era: asked, ..TacticsRules::default() },
            ..GameOptions::new(2, 26, 20, 7, 250, 0)
        });
        assert_eq!(gettysburg.start_era, 4, "a scenario's era is the battle's, not {asked:?}");
    }
}

#[test]
fn a_tactics_planet_game_opens_with_opposite_cities() {
    let game = Game::new_with(GameOptions {
        map_script: MapScript::TacticsPlanet,
        map_topology: MapTopology::Planet,
        ..GameOptions::new(2, 40, 18, 90_412, 250, 3)
    });
    assert!(game.map.sphere().is_some(), "the Tactics planet must be a globe");
    assert!(game.is_arena());
    assert_eq!(game.players.iter().filter(|player| !player.is_minor).count(), 2);
    let cities: Vec<Pos> = (0..2)
        .map(|owner| {
            let owned: Vec<Pos> = game
                .cities
                .values()
                .filter(|city| city.owner == owner)
                .map(|city| city.pos)
                .collect();
            assert_eq!(owned.len(), 1, "seat {owner} opens with one city");
            owned[0]
        })
        .collect();
    assert!(game.wdist(cities[0], cities[1]) >= 8, "{cities:?}");
    assert!(game.is_at_war(0, 1));
}

/// The appeal memo must never answer differently from the computation it
/// stands in for, on any tile of a real world.
///
/// This is the whole risk of caching a derived figure: appeal reads the
/// tile, its six neighbours, and the owning city's wonders and Governor,
/// so a memo that outlived any of those would hand the settle and district
/// planners a stale board and quietly change what the AI builds. Checked
/// against every tile rather than a sample, and from inside a scope
/// against the uncached figure taken outside one, so the two paths are
/// compared rather than the cache being compared with itself.
#[test]
fn the_appeal_memo_agrees_with_the_computation_it_replaces() {
    use crate::ai::{AdvancedAi, Ai};
    let mut game = Game::new(4, 32, 22, 5_150, 250, 3);
    let mut ais = AdvancedAi::fleet(&game);
    // Play in far enough that cities own tiles, wonders exist and
    // improvements have been laid, which is when appeal stops being a
    // function of terrain alone.
    for _ in 0..40 {
        let pid = game.current;
        ais[pid].take_turn(&mut game, pid);
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &Action::EndTurn);
        }
    }

    let tiles: Vec<Pos> = game.map.tiles.keys().copied().collect();
    assert!(tiles.len() > 400, "a real world, not a stub");
    let uncached: Vec<i32> = tiles.iter().map(|pos| game.tile_appeal(*pos)).collect();
    assert!(uncached.iter().any(|appeal| *appeal != 0), "appeal must vary");

    let memo = game.query_memo();
    for (pos, want) in tiles.iter().zip(&uncached) {
        // Twice, so the second read is the one served from the map.
        assert_eq!(game.tile_appeal(*pos), *want, "{pos:?}");
        assert_eq!(game.tile_appeal(*pos), *want, "{pos:?} on the memoized read");
    }
    drop(memo);

    // And the scope really did close: the next read recomputes rather than
    // serving whatever the dropped guard left behind.
    for (pos, want) in tiles.iter().zip(&uncached) {
        assert_eq!(game.tile_appeal(*pos), *want, "{pos:?} after the scope closed");
    }
}

/// A naval battle opens with two fleets afloat, each based on its own
/// island, and nothing on land to fight with.
///
/// The roster is the part that has to be checked by domain rather than by
/// name. `deploy_battlefield_army` searches for the first free tile
/// outward from the seat, and the seat is an island — so a squadron built
/// from the land company, or a water filter that let dry ground through,
/// would still deploy successfully and simply put galleys on hills. Every
/// unit being a sea unit standing on water is what says the mode is
/// actually naval.
#[test]
fn a_tactics_ocean_game_opens_with_two_fleets_afloat() {
    let game = Game::new_with(GameOptions {
        map_script: MapScript::TacticsOcean,
        map_topology: MapTopology::Planet,
        ..GameOptions::new(2, 40, 18, 90_412, 250, 3)
    });
    assert!(game.map.sphere().is_some(), "the Tactics ocean must be a globe");
    assert!(game.is_arena());
    assert!(game.is_at_war(0, 1));

    let roster = |seat: usize| {
        let mut kinds: Vec<&str> = game
            .units
            .values()
            .filter(|unit| unit.owner == seat)
            .map(|unit| unit.kind.as_str())
            .collect();
        kinds.sort_unstable();
        kinds
    };
    assert!(!roster(0).is_empty(), "seat 0 opens with a fleet");
    assert_eq!(roster(0), roster(1), "the two fleets must be even");

    for unit in game.units.values() {
        let domain = game.rules.units[unit.kind.as_str()].domain.as_deref();
        assert_eq!(domain, Some("sea"), "{} is not a ship", unit.kind);
        let tile = &game.map.tiles[&unit.pos];
        assert!(game.rules.is_water(tile), "{} opens on dry ground", unit.kind);
        assert!(game.rules.is_passable(tile), "{} opens on water it cannot enter", unit.kind);
    }

    // The cities are the one thing still ashore, one island each.
    for owner in 0..2 {
        let owned: Vec<Pos> = game
            .cities
            .values()
            .filter(|city| city.owner == owner)
            .map(|city| city.pos)
            .collect();
        assert_eq!(owned.len(), 1, "seat {owner} opens with one city");
        assert!(!game.rules.is_water(&game.map.tiles[&owned[0]]), "a city needs ground");
    }
}

/// A naval battle can actually be won: a galley takes the enemy port.
///
/// This is the clause that makes the mode a battle rather than a timed
/// exhibition, and it is not obvious from either half on its own. A city
/// can only be occupied by a unit that can stand on its tile, and every
/// unit in a naval battle is a ship — so the mode reads like one whose
/// objective no side can reach. It is reachable because a naval melee
/// unit is Civ 6's exception: `passable_for` lets a sea class enter a
/// City Center, which is what lets a galley attack and capture a coastal
/// city. The port is on an islet, so it is coastal by construction.
///
/// Pinned here on a real ocean arena rather than on a hand-built board,
/// because what is being claimed is about this map type: that the city
/// the mode seats is one the fleet it deals can take.
#[test]
fn a_galley_can_take_the_naval_arenas_port() {
    let mut game = Game::new_with(GameOptions {
        map_script: MapScript::TacticsOcean,
        map_topology: MapTopology::Planet,
        ..GameOptions::new(2, 40, 18, 90_412, 250, 3)
    });
    let (city, port) = game
        .cities
        .iter()
        .find(|(_, city)| city.owner == 1)
        .map(|(id, city)| (*id, city.pos))
        .expect("seat 1 opens with a port");

    // Water alongside it, which an islet always has — and which the
    // defending squadron is sitting on, because it deployed outward from
    // this very tile. That the port opens ringed by its own fleet is the
    // mode working; it is not what this test is about, so one berth is
    // cleared to stand the attacker in.
    let berth = game
        .nbrs(port)
        .into_iter()
        .find(|pos| {
            game.map
                .get(*pos)
                .is_some_and(|tile| game.rules.is_water(tile) && game.rules.is_passable(tile))
        })
        .expect("a port has water alongside it");
    for defender in game.units_at(berth) {
        game.remove_unit(defender);
    }
    let galley = game.spawn_unit("galley", 0, berth);

    // Taken down to its last point by the bombardment a fleet arrives
    // with, so what is under test is the capture rather than the grind.
    game.cities.get_mut(&city).unwrap().hp = 1;
    assert!(
        game.legal_actions(0).into_iter().any(|action| matches!(
            action,
            Action::Attack { unit, target } if unit == galley && target == port
        )),
        "a galley alongside an enemy port must be able to attack it"
    );
    game.apply(0, &Action::Attack { unit: galley, target: port }).expect("the assault resolves");
    assert_eq!(game.cities[&city].owner, 0, "the port changes hands");
}

/// A naval flag stands on the water off its own shore, never on the island
/// itself.
///
/// This is the dry-ground rule of #1456 seen from the other element, and
/// it fails the same way: a flag no enemy unit can ever reach turns
/// capture-the-flag into a mode that can only end on the clock. On land
/// that meant keeping the flag off the water; at sea it means keeping it
/// off the land, because the only units in the battle are ships.
#[test]
fn a_naval_flag_stands_on_water_a_fleet_can_reach() {
    for seed in [90_412u64, 5_115, 61_020] {
        let game = Game::new_with(GameOptions {
            map_script: MapScript::TacticsOcean,
            map_topology: MapTopology::Planet,
            tactics: TacticsRules { flag: true, ..TacticsRules::default() },
            ..GameOptions::new(2, 40, 18, seed, 250, 3)
        });
        assert_eq!(game.arena_flags.len(), 2, "seed {seed}: every side gets a flag");
        for (seat, flag) in &game.arena_flags {
            let tile = &game.map.tiles[flag];
            assert!(
                game.rules.is_water(tile),
                "seed {seed}: seat {seat}'s flag is on land no fleet can take"
            );
            assert!(game.rules.is_passable(tile), "seed {seed}: seat {seat}'s flag is unsailable");
        }
    }
}

/// The fog is a match setting, and it has to be symmetric either way:
/// both sides fogged or neither, never one. Fogged is now the default, so
/// what this pins is that both settings are still real and that neither
/// deals one commander a view the other does not have.
#[test]
fn an_arena_is_fogged_only_when_the_match_asked_for_it() {
    let arena = |fog: bool| {
        Game::new_with(GameOptions {
            map_script: MapScript::Battlefield,
            tactics: TacticsRules { fog, ..TacticsRules::default() },
            ..GameOptions::new(2, 16, 16, 7_704, 250, 0)
        })
    };

    // Lifted, the field entire, for both commanders alike.
    let open = arena(false);
    assert!(open.is_arena());
    for seat in 0..2 {
        assert_eq!(
            open.player_visibility(seat).len(),
            open.map.tiles.len(),
            "seat {seat} must see the whole unfogged field"
        );
    }

    // Fogged, each side sees only what its own units can — which on an
    // arena is a fraction of the field, because the two armies are set
    // down out of each other's sight.
    let fogged = arena(true);
    for seat in 0..2 {
        let seen = fogged.player_visibility(seat);
        assert!(
            !seen.is_empty(),
            "seat {seat} must still see the ground it stands on"
        );
        assert!(
            seen.len() < fogged.map.tiles.len(),
            "seat {seat} must not see the whole fogged field"
        );
    }
    // Symmetric: the rule is the arena's, not a handicap given to a side.
    // Two armies do not see the same number of hexes — they stand on
    // different ground, and sight carries differently over it — so what
    // has to match is the rule, which is that each side opens out of the
    // other's sight and has to go and find it.
    for (viewer, hidden) in [(0, 1), (1, 0)] {
        let seen = fogged.player_visibility(viewer);
        assert!(
            fogged
                .units
                .values()
                .filter(|unit| unit.owner == hidden)
                .all(|unit| !seen.contains(&unit.pos)),
            "seat {viewer} must open with seat {hidden}'s army still to find"
        );
    }
    // And it is the fog that hid the ground, not a different battlefield:
    // the two arenas are the same field.
    assert_eq!(open.map.tiles.len(), fogged.map.tiles.len());
}

/// Unique units are a match setting, and switching them off has to reach
/// both the opening roster and the build menu — and, less obviously, the
/// suppression that a replacement normally applies to the stock unit it
/// replaces. Refusing the Hoplite alone would leave Greece unable to
/// field a Spearman either.
#[test]
fn an_arena_fields_unique_units_only_when_the_match_asked_for_them() {
    let arena = |unique_units: bool| {
        Game::new_with(GameOptions {
            map_script: MapScript::Battlefield,
            tactics: TacticsRules { unique_units, ..TacticsRules::default() },
            // Greece replaces the Spearman with the Hoplite; the Aztecs
            // replace the Warrior with the Eagle Warrior. Both are in the
            // opening roster.
            civs: vec!["Greece".to_string(), "Aztec".to_string()],
            ..GameOptions::new(2, 10, 10, 3_141, 250, 0)
        })
    };
    let roster = |game: &Game, seat: usize| {
        let mut kinds: Vec<String> = game
            .units
            .values()
            .filter(|unit| unit.owner == seat)
            .map(|unit| unit.kind.to_string())
            .collect();
        kinds.sort_unstable();
        kinds.dedup();
        kinds
    };

    let even = arena(false);
    assert_eq!(
        roster(&even, 0),
        roster(&even, 1),
        "with unique units off the two sides field the identical roster"
    );
    assert!(!roster(&even, 0).iter().any(|kind| kind == "hoplite"));
    assert!(!roster(&even, 1).iter().any(|kind| kind == "eagle_warrior"));
    // Greece keeps its Spearman rather than losing it to a Hoplite it is
    // not allowed to have.
    assert!(roster(&even, 0).iter().any(|kind| kind == "spearman"));
    assert_eq!(even.player_unit_replacement(0, crate::name!("spearman")), crate::name!("spearman"));
    assert!(!even.arena_allows_production(&Item::Unit { unit: crate::name!("hoplite") }));

    let own = arena(true);
    assert!(
        roster(&own, 0).iter().any(|kind| kind == "hoplite"),
        "Greece fields Hoplites: {:?}",
        roster(&own, 0)
    );
    assert!(!roster(&own, 0).iter().any(|kind| kind == "spearman"));
    assert!(
        roster(&own, 1).iter().any(|kind| kind == "eagle_warrior"),
        "the Aztecs field Eagle Warriors: {:?}",
        roster(&own, 1)
    );
    assert!(own.arena_allows_production(&Item::Unit { unit: crate::name!("hoplite") }));
    // Either way an arena builds only things that fight.
    for game in [&even, &own] {
        assert!(!game.arena_allows_production(&Item::Unit { unit: crate::name!("settler") }));
    }
}

/// A square arena seats its two sides in opposite corners, a little way
/// in; a long one seats them at opposite ends. The inset is scaled rather
/// than fixed because a step along the offset diagonal is two hexes of
/// real distance, so three hexes in costs twelve hexes of approach — the
/// Field's whole margin, and more than the Square has to give.
#[test]
fn a_square_arena_seats_its_sides_in_opposite_corners() {
    let seats = |width: i32, height: i32| {
        let game = Game::new_with(GameOptions {
            map_script: MapScript::Battlefield,
            ..GameOptions::new(2, width, height, 4_242, 250, 0)
        });
        let mut seats: Vec<(i32, i32)> = (0..2)
            .map(|seat| {
                let city = game
                    .cities
                    .values()
                    .find(|city| city.owner == seat)
                    .expect("the stock arena economy seats a city per side");
                crate::hex::axial_to_offset(city.pos.0, city.pos.1)
            })
            .collect();
        seats.sort_unstable();
        let apart = {
            let sides: Vec<Pos> = (0..2)
                .map(|seat| {
                    game.cities.values().find(|city| city.owner == seat).unwrap().pos
                })
                .collect();
            game.wdist(sides[0], sides[1])
        };
        (seats, apart)
    };

    // The Field: three hexes in from opposite corners, exactly as asked.
    let (field, field_apart) = seats(20, 20);
    assert_eq!(field, vec![(3, 3), (16, 16)]);
    assert_eq!(field_apart, 19);

    // The Square: the same corners at the size it is. Three hexes in
    // would leave the two sides four apart and deploying into each other.
    let (square, square_apart) = seats(10, 10);
    assert_eq!(square, vec![(1, 1), (8, 8)]);
    assert!(square_apart >= 10, "{square_apart}");

    // The March is a rectangle, and a rectangle's diagonal is barely
    // longer than its length: it keeps the two end walls.
    let (march, march_apart) = seats(10, 20);
    assert!(
        march.iter().all(|(col, _)| *col == 5),
        "a long arena fights up and down the field: {march:?}"
    );
    assert_eq!(march.iter().map(|(_, row)| *row).collect::<Vec<_>>(), vec![0, 19]);
    assert_eq!(march_apart, 19);
}

/// Nothing walks off a battlefield. The arena's four walls are the map's
/// own edges: a hex on the east wall has no neighbour to the east, the
/// west wall is the width of the field away rather than one step through
/// a seam, and an archer standing on one wall is out of range of the far
/// one. On a Civ world's cylinder all three are the other way round.
#[test]
fn an_arena_is_walled_where_a_world_wraps() {
    let arena = Game::new_with(GameOptions {
        map_script: MapScript::Battlefield,
        ..GameOptions::new(2, 10, 10, 4_411, 250, 0)
    });
    let world = Game::new_full(2, 44, 26, 4_411, 250, 0, false);
    for (game, wraps) in [(&arena, false), (&world, true)] {
        let width = game.map.width;
        let west = crate::hex::offset_to_axial(0, 4);
        let east = crate::hex::offset_to_axial(width - 1, 4);
        assert_eq!(
            game.nbrs(west).iter().any(|neighbor| *neighbor == east),
            wraps,
            "the two edge columns are neighbours only on a cylinder"
        );
        assert_eq!(game.wdist(west, east) == 1, wraps);
        if !wraps {
            assert_eq!(
                game.wdist(west, east),
                width - 1,
                "an arena's far wall is the whole field away"
            );
            assert_eq!(
                game.nbrs(west).len(),
                3,
                "a hex in the middle of a wall has three neighbours, not six"
            );
        }
    }
    // And no unit may step through the wall.
    let west = crate::hex::offset_to_axial(0, 4);
    let east = crate::hex::offset_to_axial(arena.map.width - 1, 4);
    let mut arena = arena;
    assert!(arena.units_at(west).is_empty(), "the wall hex is free to stand on");
    let uid = arena.spawn_test_unit("horseman", 0, west);
    assert!(
        !arena.can_move(uid, east),
        "a unit on the west wall must not step through it to the east one"
    );
    assert!(
        arena.nbrs(west).iter().all(|next| arena.can_move(uid, *next)),
        "and it may still walk every way the field continues"
    );
}

/// Last army standing. A side on an arena is its army: it has no city to
/// lose and no Settler to found one with, so the ordinary elimination
/// test — no city and no Settler — would end both sides at the first
/// casualty. What ends a side here is its last unit, and that ends the
/// battle.
#[test]
fn the_last_army_standing_takes_the_field() {
    let mut game = Game::new_with(GameOptions {
        map_script: MapScript::Battlefield,
        // The no-city arena: a side is exactly its army, so losing the
        // army is losing the battle with nothing left to rebuild from.
        tactics: TacticsRules { cities: 0, ..TacticsRules::default() },
        ..GameOptions::new(2, 10, 10, 7_311, 250, 0)
    });
    let losing: Vec<u32> = game.player_unit_ids(1);
    assert!(losing.len() > 1);
    for uid in &losing[1..] {
        game.remove_unit(*uid);
        game.check_elimination(1);
        assert!(game.players[1].alive, "a side with a unit left is still in the battle");
        assert!(game.winner.is_none());
    }
    game.remove_unit(losing[0]);
    game.check_elimination(1);
    game.check_domination();
    assert!(!game.players[1].alive, "a side with no units left has lost the field");
    assert_eq!(game.winner, Some(0));
    assert_eq!(game.victory_type.as_deref(), Some("domination"));
}

/// The stock arena seats a city a side but grants it nothing, so a side
/// whose last unit falls has no way back onto the field and the battle
/// is over — the annihilating side wins by domination even if it has no
/// melee left to walk into the empty city. Grant the city Production, or
/// the side Gold, and the same side is still in the battle: the next
/// unit off the queue, or the next one bought, puts it back on the field.
#[test]
fn a_side_with_a_city_it_cannot_field_from_falls_with_its_last_unit() {
    let arena = |production: u32, gold: u32| {
        Game::new_with(GameOptions {
            map_script: MapScript::Battlefield,
            tactics: TacticsRules { cities: 1, production, gold, ..TacticsRules::default() },
            ..GameOptions::new(2, 10, 10, 7_311, 250, 0)
        })
    };
    // The stock grants: nothing behind the army.
    let mut stock = arena(0, 0);
    assert_eq!(stock.tactics.production, 0);
    assert_eq!(stock.tactics.gold, 0);
    assert!(stock.cities.values().any(|city| city.owner == 1), "seat 1 holds a city");
    for uid in stock.player_unit_ids(1) {
        stock.remove_unit(uid);
    }
    stock.check_elimination(1);
    stock.check_domination();
    assert!(!stock.players[1].alive, "a city that can never field a unit is not a way back");
    assert!(stock.cities.values().any(|city| city.owner == 1), "the empty city still stands");
    assert_eq!(stock.winner, Some(0), "the last army standing takes the field");
    assert_eq!(stock.victory_type.as_deref(), Some("domination"));

    // Either grant keeps a side with a city in the battle.
    for (production, gold) in [(30, 0), (0, 30)] {
        let mut reinforced = arena(production, gold);
        for uid in reinforced.player_unit_ids(1) {
            reinforced.remove_unit(uid);
        }
        reinforced.check_elimination(1);
        reinforced.check_domination();
        assert!(
            reinforced.players[1].alive,
            "with {production} Production and {gold} Gold the city can field a unit again"
        );
        assert_eq!(reinforced.winner, None);
    }
}

/// An arena has no empire behind it, so it runs none of an empire's
/// bookkeeping against the army: no city may be founded on it, an army
/// costs nothing to keep — bankruptcy would otherwise disband the units
/// the mode exists to fight with — and nothing heals, so damage taken in
/// a battle is damage kept.
#[test]
fn an_arena_has_no_economy_behind_its_army() {
    let mut game = Game::new_with(GameOptions {
        map_script: MapScript::Battlefield,
        tactics: TacticsRules { cities: 0, ..TacticsRules::default() },
        ..GameOptions::new(2, 10, 10, 5_150, 250, 0)
    });
    let uid = game.player_unit_ids(0)[0];
    assert_eq!(game.unit_heal_rate(uid), 0, "nothing heals on an arena");
    // Every unit of both armies is free of upkeep, including the ones
    // whose own ruleset row charges maintenance.
    assert!(game
        .units
        .values()
        .any(|unit| game.rules.units[unit.kind].maintenance > 0.0));
    for seat in 0..2 {
        game.players[seat].gold = 0.0;
        let before = game.player_unit_ids(seat).len();
        game.settle_gold_budget(seat, 0.0);
        assert_eq!(
            game.player_unit_ids(seat).len(),
            before,
            "an arena army must not be disbanded for a deficit it cannot close"
        );
        assert_eq!(game.players[seat].bankruptcy_amenity_penalty, 0);
    }
    // A Settler that reached an arena some other way still founds nothing.
    let start = game.units[&game.player_unit_ids(0)[0]].pos;
    let settler = game.spawn_test_unit("settler", 0, start);
    assert!(game
        .apply(0, &Action::FoundCity { unit: settler })
        .is_err_and(|refusal| refusal.contains("battlefield")));
    assert!(game.cities.is_empty());
}

/// What a Tactics game tells a client it can be won by. Four of the six
/// lanes need cities, and Score cannot be won because the deadline is a
/// draw. The victory tracker is therefore told only about last-army-
/// standing Domination, whatever a lobby left in the checkboxes.
#[test]
fn an_arena_publishes_only_the_lanes_a_battle_can_be_decided_by() {
    let mut game = Game::new_with(GameOptions {
        map_script: MapScript::Battlefield,
        ..GameOptions::new(2, 10, 10, 6_161, 250, 0)
    });
    // However the world was set up — here, every lane switched on.
    game.victory_conditions = VictoryConditions {
        science: true,
        culture: true,
        religious: true,
        diplomatic: true,
        domination: true,
        score: true,
    };
    game.required_victory_types = 3;
    let published = game.effective_victory_conditions();
    for lane in ["science", "culture", "religious", "diplomatic"] {
        assert!(!published.is_enabled(lane), "{lane} has nowhere to happen on an arena");
        assert!(!game.set_winner(0, lane), "{lane} must not decide a battle");
    }
    assert!(published.is_enabled("domination"));
    assert!(!published.is_enabled("score"));
    assert!(!game.set_winner(0, "score"), "the deadline is not a victory lane");
    // And a battle is decided once, however many types were required.
    assert_eq!(game.effective_required_victories(), 1);

    // A Civ world publishes exactly what it was set up with.
    let mut world = Game::new_full(2, 44, 26, 6_161, 250, 0, false);
    world.victory_conditions.science = false;
    assert_eq!(world.effective_victory_conditions(), world.victory_conditions);
}

/// Reaching the selected deadline with both sides alive ends the battle
/// without inventing a score winner, even when one army is far ahead.
#[test]
fn a_tactics_deadline_is_a_terminal_draw() {
    let mut game = Game::new_with(GameOptions {
        map_script: MapScript::Battlefield,
        tactics: TacticsRules { turn_limit: 50, ..TacticsRules::default() },
        ..GameOptions::new(2, 10, 10, 2_215, 50, 0)
    });
    assert_eq!(game.score(0), game.score(1), "the two armies open even");
    // Half of one side's army falls, but advantage is not victory.
    for uid in game.player_unit_ids(1).into_iter().take(4) {
        game.remove_unit(uid);
    }
    assert!(game.score(0) > game.score(1));
    game.current = 1;
    game.turn = 50;
    game.do_end_turn();

    assert!(game.is_finished());
    assert!(game.is_draw());
    assert_eq!(game.winner, None);
    assert_eq!(game.winning_players(), Vec::<usize>::new());
    assert_eq!(game.victory_type.as_deref(), Some(DRAW_RESULT));
    assert_eq!(game.victory_label().as_deref(), Some(DRAW_RESULT));
    assert_eq!(game.turn, 51, "the result is counted on the final wrap");
    assert_eq!(game.reported_turn(), 50);
    assert_eq!(
        game.apply(game.current, &Action::EndTurn),
        Err("game over".to_string()),
        "a draw must stop the engine just like a victory"
    );
}

#[test]
fn dark_to_golden_threshold_creates_a_three_dedication_heroic_age() {
    let mut game = Game::new_full(2, 24, 16, 88_102, 100, 0, false);
    game.players[0].age = "dark".to_string();
    game.players[0].era_score = game.players[0].golden_age_threshold;
    game.players[0].techs.insert(crate::name!("horseback_riding"));
    game.players[1].techs.insert(crate::name!("horseback_riding"));
    // Half the majors reaching Classical starts the ten-turn warning.
    game.turn = 40;
    game.process_eras();
    game.turn = 50;
    game.process_eras();
    assert_eq!(game.players[0].age, "heroic");
    assert_eq!(game.players[0].dedication_choices, 3);
    game.do_choose_dedication(0, "monumentality").unwrap();
    assert!(game.players[0].dedications.contains("monumentality"));

    let position = game.units.values().next().unwrap().pos;
    let builder = game.spawn_unit("builder", 0, position);
    assert_eq!(
        game.unit_base_max_moves(builder),
        game.rules.units["builder"].moves + 2.0
    );
}

#[test]
fn alliance_routes_progression_and_favor_match_gathering_storm() {
    let game = emergency_game_with_capitals(2, 88_102, 300);
    let first = game.player_city_ids(0)[0];
    let second = game.player_city_ids(1)[0];
    let mut routed = game.clone();
    routed.routes.push(TradeRoute {
        origin: first,
        dest: second,
        owner: 0,
        ends: routed.turn + 30,
    });
    let base_origin = routed.city_yields(first);
    let base_destination = routed.city_yields(second);
    // The capitals are Content, so the alliance route yields are unscaled.
    for (kind, outbound, inbound) in [
        ("research", 2.0, 1.0),
        ("cultural", 2.0, 1.0),
        ("economic", 4.0, 2.0),
        ("religious", 2.0, 1.0),
    ] {
        let mut allied = routed.clone();
        install_alliance(&mut allied, 0, 1, kind, 1, 0.0);
        let origin = allied.city_yields(first);
        let destination = allied.city_yields(second);
        let origin_difference = match kind {
            "research" => origin.science - base_origin.science,
            "cultural" => origin.culture - base_origin.culture,
            "economic" => origin.gold - base_origin.gold,
            "religious" => origin.faith - base_origin.faith,
            _ => unreachable!(),
        };
        let destination_difference = match kind {
            "research" => destination.science - base_destination.science,
            "cultural" => destination.culture - base_destination.culture,
            "economic" => destination.gold - base_destination.gold,
            "religious" => destination.faith - base_destination.faith,
            _ => unreachable!(),
        };
        assert!((origin_difference - outbound).abs() < 1e-9, "{kind}");
        assert!((destination_difference - inbound).abs() < 1e-9, "{kind}");
    }

    install_alliance(&mut routed, 0, 1, "economic", 1, 79.0);
    let favor_before = routed.players[0].diplomatic_favor;
    routed.process_diplomacy(0);
    assert_eq!(routed.players[0].alliances[&1].points, 80.25);
    assert_eq!(routed.players[0].alliances[&1].level, 2);
    assert_eq!(
        routed.players[0].diplomatic_favor - favor_before,
        2.0,
        "a governmentless player receives only the Level 2 alliance's Favor"
    );
    routed.routes.clear();
    routed.players[0].alliances.get_mut(&1).unwrap().points = 239.0;
    routed.players[1].alliances.get_mut(&0).unwrap().points = 239.0;
    routed.process_diplomacy(0);
    assert_eq!(routed.players[0].alliances[&1].points, 240.0);
    assert_eq!(routed.players[0].alliances[&1].level, 3);

    routed.turn = routed.players[0].alliances[&1].ends;
    routed.players[0].civics.insert(crate::name!("civil_service"));
    routed.players[1].civics.insert(crate::name!("civil_service"));
    routed.record_contact(0, 1);
    routed
        .do_propose_deal(0, 1, 0.0, 0.0, false, true, false, Some("economic"))
        .unwrap();
    let renewal = routed.pending_deals.last().unwrap().id;
    routed.do_accept_deal(1, renewal).unwrap();
    assert_eq!(routed.players[0].alliances[&1].points, 240.0);
    assert_eq!(routed.players[0].alliances[&1].level, 3);
}

#[test]
fn governments_generate_stock_influence_envoys_and_favor_by_tier() {
    let cases = [
        ("chiefdom", 1.0, 100.0, 1, 0.0),
        ("autocracy", 3.0, 100.0, 1, 1.0),
        ("oligarchy", 3.0, 100.0, 1, 1.0),
        ("classical_republic", 3.0, 100.0, 1, 1.0),
        ("monarchy", 5.0, 150.0, 2, 2.0),
        ("merchant_republic", 5.0, 150.0, 2, 2.0),
        ("theocracy", 5.0, 150.0, 2, 2.0),
        ("communism", 7.0, 200.0, 3, 3.0),
        ("democracy", 7.0, 200.0, 3, 3.0),
        ("fascism", 7.0, 200.0, 3, 3.0),
        ("corporate_libertarianism", 9.0, 250.0, 4, 4.0),
        ("digital_democracy", 9.0, 250.0, 4, 4.0),
        ("synthetic_technocracy", 9.0, 250.0, 4, 4.0),
    ];
    for (government, base_rate, threshold, envoys, favor) in cases {
        let mut game = emergency_game_with_capitals(1, 88_109, 300);
        game.players[0].government = Some(government.to_string());
        let spec = &game.rules.governments[government];
        assert_eq!(spec.influence_per_turn, base_rate, "{government}");
        assert_eq!(spec.influence_threshold, threshold, "{government}");
        assert_eq!(spec.envoys_per_threshold, envoys, "{government}");
        assert_eq!(spec.diplomatic_favor_per_turn, favor, "{government}");

        let actual_rate = if government == "monarchy" {
            base_rate * 1.5
        } else {
            base_rate
        };
        game.process_influence(0);
        assert_eq!(game.players[0].influence, actual_rate, "{government}");

        game.players[0].influence = threshold * 2.0 - actual_rate;
        game.players[0].envoys_free = 0;
        game.process_influence(0);
        assert_eq!(game.players[0].influence, 0.0, "{government}");
        assert_eq!(game.players[0].envoys_free, envoys * 2, "{government}");

        let favor_before = game.players[0].diplomatic_favor;
        game.process_diplomacy(0);
        assert_eq!(
            game.players[0].diplomatic_favor - favor_before,
            favor,
            "{government}"
        );
    }

    let mut governmentless = emergency_game_with_capitals(1, 88_110, 300);
    governmentless.process_influence(0);
    governmentless.process_diplomacy(0);
    assert_eq!(governmentless.players[0].influence, 0.0);
    assert_eq!(governmentless.players[0].diplomatic_favor, 0.0);
}

#[test]
fn research_alliance_shares_eurekas_and_level_three_science() {
    let mut game = emergency_game_with_capitals(2, 88_103, 300);
    let city = game.player_city_ids(0)[0];
    game.turn = 30;
    game.players[1].techs.insert(crate::name!("writing"));
    install_alliance(&mut game, 0, 1, "research", 2, 80.0);
    game.process_diplomacy(0);
    assert!(game.players[0].boosted_techs.contains(&crate::name!("writing")));

    // ALLIANCE_SCIENCE_SHARING_FROM_ALLY is MODIFIER_ALLIANCE_PLAYERS_
    // SCIENCE_FROM_ALLY at 10 with NO requirement set -- the exact shape of
    // the Cultural tier's CULTURE_FROM_ALLY 10 and TOURISM_FROM_ALLY 20. It
    // is a tenth of what the ALLY makes, unconditionally, not a tenth added
    // to your own cities and not gated on researching the same technology.
    let copied = game
        .player_city_ids(1)
        .into_iter()
        .map(|city| game.city_yields(city).science)
        .sum::<f64>()
        * 0.10;
    assert!(copied > 0.0);
    // Level is re-derived from points every turn, so raise both: the
    // shipped ALLIANCE_LEVEL_THREE_XP is 960, which is 240 of these.
    for (holder, partner) in [(0usize, 1usize), (1, 0)] {
        let alliance = game.players[holder].alliances.get_mut(&partner).unwrap();
        alliance.level = 3;
        alliance.points = 240.0;
    }
    let mut baseline = game.clone();
    baseline.players[0].alliances.clear();
    baseline.players[1].alliances.clear();

    // The ally's Science does not touch the allied player's own cities.
    assert_eq!(game.city_yields(city).science, baseline.city_yields(city).science);

    game.begin_turn(0);
    baseline.begin_turn(0);
    let gained = game.players[0].research_progress + game.players[0].research_overflow;
    let without = baseline.players[0].research_progress + baseline.players[0].research_overflow;
    assert!((gained - without - copied).abs() < 1e-9, "{gained} - {without} != {copied}");
}

#[test]
fn a_level_three_military_alliance_trains_its_units_promoted() {
    // ALLIANCE_FREE_UNIT_UPGRADE is a misnomer: COLLECTION_ALLIANCE_TRAINED
    // _UNITS with EFFECT_ADJUST_UNIT_GRANT_EXPERIENCE at -1, which is the
    // amount every FREE_PROMOTION row in the game carries -- Hetairoi,
    // Corbaci, Nau, City Defender, and the Terracotta Army. It grants a
    // promotion, not a discount.
    let mut game = emergency_game_with_capitals(2, 88_106, 300);
    let city = game.player_city_ids(0)[0];
    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    let trained_xp = |game: &mut Game| {
        let before: BTreeSet<u32> = game.player_unit_ids(0).into_iter().collect();
        assert!(game.complete_item(0, city, &warrior));
        let uid = game
            .player_unit_ids(0)
            .into_iter()
            .find(|id| !before.contains(id))
            .expect("a trained Warrior");
        let xp = game.units[&uid].xp;
        // Reuse the same legal training tile for each alliance tier; the
        // test is about granted experience, not accumulating a garrison.
        game.remove_unit(uid);
        xp
    };
    assert_eq!(trained_xp(&mut game), 0, "no alliance, no promotion");

    install_alliance(&mut game, 0, 1, "military", 3, 240.0);
    let promoted = trained_xp(&mut game);
    assert_eq!(
        promoted,
        Game::promotion_threshold(1),
        "trained already able to promote"
    );

    // Level 2 is not enough: the free promotion is the Level 3 reward.
    install_alliance(&mut game, 0, 1, "military", 2, 80.0);
    assert_eq!(trained_xp(&mut game), 0);
}

#[test]
fn military_alliance_shares_war_production_vision_and_promotions() {
    let mut game = emergency_game_with_capitals(3, 88_104, 300);
    let city = game.player_city_ids(0)[0];
    let position = game.cities[&city].pos;
    let item = Item::Unit {
        unit: crate::name!("warrior"),
    };
    let baseline = game.item_prod_mult(0, city, Some(&item));
    install_alliance(&mut game, 0, 1, "military", 2, 80.0);
    game.at_war.insert(pair(1, 2));
    assert_eq!(game.item_prod_mult(0, city, Some(&item)), baseline + 0.15);
    assert_eq!(game.vs_bonus(0, 2), 5.0);

    let first_sight = *game.map.tiles.keys().next().unwrap();
    let last_sight = *game.map.tiles.keys().next_back().unwrap();
    game.players[0].explored = [first_sight].into_iter().collect();
    game.players[1].explored = [last_sight].into_iter().collect();
    game.process_diplomacy(0);
    assert!(game.players[0].explored.contains(&last_sight));
    assert!(game.players[1].explored.contains(&first_sight));

    game.players[0].alliances.get_mut(&1).unwrap().level = 3;
    game.players[1].alliances.get_mut(&0).unwrap().level = 3;
    let warrior = game.spawn_unit("warrior", 0, position);
    assert_eq!(
        game.units[&warrior].xp,
        Game::promotion_threshold(game.units[&warrior].level)
    );
    let allied_position = game.cities[&game.player_city_ids(1)[0]].pos;
    let allied_unit = game.spawn_unit("warrior", 1, allied_position);
    let observed = crate::obs::observation(&game, 0);
    assert!(observed["units"]
        .as_array()
        .unwrap()
        .iter()
        .any(|unit| unit["id"] == serde_json::json!(allied_unit)));
    let submarine = game.spawn_unit("submarine", 2, allied_position);
    game.spawn_unit("destroyer", 1, allied_position);
    let observed = crate::obs::observation(&game, 0);
    assert!(
        observed["units"]
            .as_array()
            .unwrap()
            .iter()
            .any(|unit| unit["id"] == serde_json::json!(submarine)),
        "the alliance must share units detected by the ally, not only ordinary map sight"
    );
}

#[test]
fn cultural_religious_and_economic_alliance_levels_execute() {
    let mut cultural = emergency_game_with_capitals(2, 88_105, 300);
    let origin = cultural.player_city_ids(0)[0];
    let destination = cultural.player_city_ids(1)[0];
    let district_position = cultural.cities[&origin]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != cultural.cities[&origin].pos)
        .unwrap();
    cultural
        .cities
        .get_mut(&origin)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), district_position);
    cultural
        .map
        .tiles
        .get_mut(&district_position)
        .unwrap()
        .district = Some(crate::name!("campus"));
    cultural.routes.push(TradeRoute {
        origin,
        dest: destination,
        owner: 0,
        ends: cultural.turn + 30,
    });
    let mut cultural_baseline = cultural.clone();
    install_alliance(&mut cultural, 0, 1, "cultural", 2, 80.0);
    cultural.process_great_people(0);
    cultural_baseline.process_great_people(0);
    assert_eq!(
        cultural.players[0].gpp["scientist"] - cultural_baseline.players[0].gpp["scientist"],
        1.0
    );

    let mut cultural_level_three = emergency_game_with_capitals(2, 88_108, 300);
    let copied_culture = cultural_level_three
        .player_city_ids(1)
        .into_iter()
        .map(|city| cultural_level_three.city_yields(city).culture)
        .sum::<f64>()
        * 0.10;
    let copied_tourism = cultural_level_three.tourism_per_turn(1) * 0.20;
    install_alliance(&mut cultural_level_three, 0, 1, "cultural", 3, 240.0);
    let mut level_three_baseline = cultural_level_three.clone();
    level_three_baseline.players[0].alliances.clear();
    level_three_baseline.players[1].alliances.clear();
    cultural_level_three.begin_turn(0);
    level_three_baseline.begin_turn(0);
    assert!(
        (cultural_level_three.players[0].civic_overflow
            - level_three_baseline.players[0].civic_overflow
            - copied_culture)
            .abs()
            < 1e-9
    );
    assert!(
        (cultural_level_three.players[0].tourism_lifetime
            - level_three_baseline.players[0].tourism_lifetime
            - copied_tourism)
            .abs()
            < 1e-9
    );

    let mut religion = emergency_game_with_capitals(3, 88_106, 300);
    let own_city = religion.player_city_ids(0)[0];
    let allied_city = religion.player_city_ids(1)[0];
    religion.players[0].religion = Some("Own".to_string());
    religion.players[1].religion = Some("Allied".to_string());
    religion.players[2].religion = Some("Third".to_string());
    religion.cities.get_mut(&own_city).unwrap().pop = 10;
    religion.cities.get_mut(&own_city).unwrap().pressure =
        BTreeMap::from([("Own".to_string(), 60.0), ("Allied".to_string(), 40.0)]);
    religion.cities.get_mut(&own_city).unwrap().atheist_pressure = 0.0;
    religion.cities.get_mut(&allied_city).unwrap().pressure =
        BTreeMap::from([("Allied".to_string(), 100.0)]);
    religion.cities.get_mut(&allied_city).unwrap().atheist_pressure = 0.0;
    install_alliance(&mut religion, 0, 1, "religious", 3, 240.0);
    let mut religious_baseline = religion.clone();
    religious_baseline.players[0].alliances.clear();
    religious_baseline.players[1].alliances.clear();
    // A difference of two sums, so the last bit of the mantissa depends on
    // what else the city happens to be producing. Compare the number, not
    // its representation.
    let allied_faith =
        religion.city_yields(own_city).faith - religious_baseline.city_yields(own_city).faith;
    assert!(
        (allied_faith - 3.2).abs() < 1e-9,
        "four allied-religion followers pass through the city's 80% yield modifier, got {allied_faith}"
    );
    religion.routes.push(TradeRoute {
        origin: own_city,
        dest: allied_city,
        owner: 0,
        ends: religion.turn + 30,
    });
    let pressure_before = religion.cities[&own_city].pressure["Allied"];
    religion.process_pressure(0);
    assert_eq!(
        religion.cities[&own_city].pressure["Allied"],
        pressure_before
    );
    assert_eq!(religion.religious_alliance_combat_bonus(0, "Third"), 10.0);
    assert_eq!(religion.religious_alliance_combat_bonus(0, "Allied"), 0.0);

    let mut economic = emergency_game_with_capitals(3, 88_107, 300);
    economic.players[2].is_minor = true;
    economic.players[0].government = Some("chiefdom".to_string());
    economic.players[1].envoys.push((2, 3));
    install_alliance(&mut economic, 0, 1, "economic", 2, 80.0);
    let mut economic_baseline = economic.clone();
    economic_baseline.players[0].alliances.clear();
    economic_baseline.players[1].alliances.clear();
    economic.begin_turn(0);
    economic_baseline.begin_turn(0);
    assert_eq!(
        economic.players[0].influence - economic_baseline.players[0].influence,
        1.0
    );
}

#[test]
fn denouncement_unlocks_formal_war_and_alliances_level_each_turn() {
    let mut game = Game::new_full(2, 24, 16, 88_103, 100, 0, false);
    game.record_contact(0, 1);
    game.do_denounce(0, 1).unwrap();
    game.turn += 5;
    game.do_declare_war_with_casus_belli(0, 1, "formal_war")
        .unwrap();
    assert!(game.is_at_war(0, 1));
    assert_eq!(game.players[1].grievances.get(&0), Some(&125.0));

    game.at_war.clear();
    game.players[0].grievances.clear();
    game.players[1].grievances.clear();
    game.players[0].civics.insert(crate::name!("civil_service"));
    game.players[1].civics.insert(crate::name!("civil_service"));
    game.players[0].friends_until.insert(1, game.turn + 30);
    game.players[1].friends_until.insert(0, game.turn + 30);
    let alliance = AllianceState {
        kind: "economic".to_string(),
        points: 79.0,
        level: 1,
        ends: game.turn + 30,
    };
    game.players[0].alliances.insert(1, alliance.clone());
    game.players[1].alliances.insert(0, alliance);
    game.process_diplomacy(0);
    assert_eq!(game.players[0].alliances[&1].level, 2);
    assert!(game.has_open_borders(0, 1));
    assert!(game.do_declare_war(0, 1).is_err());
}

#[test]
fn emergencies_wait_for_medieval_and_queue_behind_a_regular_session() {
    let make_objective = |game: &mut Game| {
        let position = game
            .map
            .tiles
            .keys()
            .copied()
            .find(|position| game.city_at(*position).is_none())
            .unwrap();
        game.found_city_for(1, position, Some("Emergency Trigger".to_string()))
    };
    let mut ancient = emergency_game_with_capitals(3, 88_103, 300);
    let city = make_objective(&mut ancient);
    ancient.capture_city(city, 0);
    ancient.do_keep_city(0, city).unwrap();
    assert!(ancient.pending_emergencies.is_empty());
    assert!(ancient.congress.is_none());

    let mut medieval = emergency_game_with_capitals(3, 88_106, 300);
    medieval.world_era = 2;
    medieval.congress = Some(CongressSession {
        convened: medieval.turn,
        closes: medieval.turn + 5,
        resolutions: vec![CongressResolution {
            id: "regular_test".to_string(),
            title: "Regular Session".to_string(),
            choices: vec!["A:test".to_string(), "B:test".to_string()],
            ballots: BTreeMap::new(),
        }],
    });
    let city = make_objective(&mut medieval);
    medieval.capture_city(city, 0);
    medieval.do_keep_city(0, city).unwrap();
    assert_eq!(
        medieval.congress.as_ref().unwrap().resolutions[0].id,
        "regular_test"
    );
    assert_eq!(medieval.pending_emergencies.len(), 1);
    medieval.turn = medieval.congress.as_ref().unwrap().closes;
    medieval.process_congress();
    assert!(medieval.congress.as_ref().unwrap().resolutions[0]
        .id
        .starts_with("emergency:"));
}

#[test]
fn city_state_emergency_votes_form_a_coalition_and_reward_liberation() {
    let mut game = emergency_game_with_capitals(5, 88_104, 300);
    game.world_era = 2;
    game.players[1].is_minor = true;
    game.players[2].envoys.push((1, 3));
    game.players[3].envoys.push((1, 1));
    // A target may have an entirely valid pact with a civilization that
    // is excluded from the Emergency ballot by that alliance. Unlike an
    // ordinary declaration, the Emergency must not call that pact into
    // the war.
    game.players[4].envoys.push((1, 1));
    install_alliance(&mut game, 0, 4, "military", 1, 0.0);
    game.players[0].defensive_pacts.insert(4, game.turn + 30);
    game.players[4].defensive_pacts.insert(0, game.turn + 30);
    game.players[2].diplomatic_favor = 10.0;
    let objective = game.player_city_ids(1)[0];
    let first_sight = *game.map.tiles.keys().next().unwrap();
    let last_sight = *game.map.tiles.keys().next_back().unwrap();
    game.players[2].explored = [first_sight].into_iter().collect();
    game.players[3].explored = [last_sight].into_iter().collect();

    game.capture_city(objective, 0);
    game.do_keep_city(0, objective).unwrap();
    assert!(!game.players[1].alive);
    assert_eq!(game.pending_emergencies.len(), 1);
    let queued: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(queued.pending_emergencies, game.pending_emergencies);
    assert_eq!(queued.congress, game.congress);
    let resolution = game.congress.as_ref().unwrap().resolutions[0].id.clone();
    assert_eq!(game.pending_emergencies[0].kind, "city_state");
    assert!(game.legal_actions(0).contains(&Action::CongressVote {
        resolution: Name::new(&resolution.clone()),
        choice: "B:oppose".to_string(),
        votes: 1,
    }));
    assert!(!game.legal_actions(0).iter().any(
        |action| matches!(action, Action::CongressVote { choice, .. } if choice == "A:support")
    ));
    game.do_congress_vote(0, &resolution, "B:oppose", 1)
        .unwrap();
    game.do_congress_vote(2, &resolution, "A:support", 2)
        .unwrap();
    game.do_congress_vote(3, &resolution, "A:support", 1)
        .unwrap();
    for member in [2, 3] {
        game.players[member].diplomatic_missions.insert(
            0,
            DiplomaticMission {
                kind: "delegation".to_string(),
                sent: game.turn,
            },
        );
        game.players[0].diplomatic_missions.insert(
            member,
            DiplomaticMission {
                kind: "embassy".to_string(),
                sent: game.turn,
            },
        );
        game.players[member]
            .promises
            .entry(0)
            .or_default()
            .insert("no_spying".to_string(), game.turn + 30);
        game.players[0]
            .promises
            .entry(member)
            .or_default()
            .insert("no_settling".to_string(), game.turn + 30);
    }

    // A Special Session can remain open after an eligible supporter has
    // settled with the target. Resolving the old ballot must not reopen
    // that pair's war inside the newly signed treaty.
    let mut treaty = game.clone();
    let closes = treaty.congress.as_ref().unwrap().closes;
    treaty
        .peace_treaties
        .insert(pair(0, 3), closes + treaty.standard_duration(10));
    treaty.turn = closes;
    treaty.process_congress();
    assert_eq!(
        treaty.active_emergencies[0].members,
        [2].into_iter().collect()
    );
    assert!(treaty.is_at_war(0, 2));
    assert!(!treaty.is_at_war(0, 3));
    assert!(treaty.peace_treaty_until(0, 3).is_some());

    game.at_war.insert(pair(2, 3));
    game.open_war_record(2, 3);
    game.turn = game.congress.as_ref().unwrap().closes;
    game.process_congress();

    assert!(game.pending_emergencies.is_empty());
    assert_eq!(game.active_emergencies.len(), 1);
    assert_eq!(
        game.active_emergencies[0].members,
        [2, 3].into_iter().collect()
    );
    assert!(game.is_at_war(0, 2));
    assert!(game.is_at_war(0, 3));
    assert!(!game.is_at_war(2, 3));
    assert!(
        !game.is_at_war(2, 4) && !game.is_at_war(3, 4),
        "an Emergency does not trigger the target's Defensive Pact"
    );
    assert!(
        game.has_defensive_pact(0, 4),
        "the uninvolved pact remains in force"
    );
    for member in [2, 3] {
        assert!(!game.players[member].diplomatic_missions.contains_key(&0));
        assert!(!game.players[0].diplomatic_missions.contains_key(&member));
        assert!(!game.players[member].promises.contains_key(&0));
        assert!(!game.players[0].promises.contains_key(&member));
    }
    let coalition_ceasefire = game
        .concluded_wars
        .iter()
        .find(|war| pair(war.aggressor, war.defender) == pair(2, 3))
        .expect("the coalition keeps the war it interrupted in the chronicle");
    assert_eq!(coalition_ceasefire.ended, Some(game.turn));
    assert_eq!(
        coalition_ceasefire.highlights.last().unwrap().kind,
        "coalition"
    );
    assert!(!game
        .legal_actions(2)
        .contains(&Action::DeclareWar { player: 3 }));
    assert!(game.do_declare_war(2, 3).is_err());
    assert!(game.players[2].open_borders_until[&3] > game.turn);
    assert!(game.players[3].open_borders_until[&2] > game.turn);
    assert!(game.players[2].explored.contains(&last_sight));
    assert!(game.players[3].explored.contains(&first_sight));
    assert_eq!(game.players[0].grievances.get(&2), None);
    assert!(game.do_make_peace(2, 0).is_err());

    game.cities.get_mut(&objective).unwrap().loyalty = 20.0;
    let mut loyalty_baseline = game.clone();
    loyalty_baseline.active_emergencies.clear();
    game.process_loyalty(0);
    loyalty_baseline.process_loyalty(0);
    assert_eq!(
        game.cities[&objective].loyalty,
        loyalty_baseline.cities[&objective].loyalty + 20.0
    );

    let target_position = game.cities[&objective].pos;
    let member = game.spawn_test_unit("warrior", 2, target_position);
    let target_position_2 = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.wdist(*position, target_position) >= 2)
        .unwrap();
    let target = game.spawn_test_unit("warrior", 0, target_position_2);
    let mut baseline = game.clone();
    baseline.active_emergencies.clear();
    assert_eq!(
        game.unit_base_max_moves(member),
        baseline.unit_base_max_moves(member) + 1.0
    );
    assert_eq!(
        game.matchup_bonus(member, &game.units[&target], true),
        baseline.matchup_bonus(member, &baseline.units[&target], true) + 2.0
    );

    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(restored.active_emergencies, game.active_emergencies);
    let mut failed = game.clone();
    failed.turn = failed.active_emergencies[0].ends;
    failed.process_emergencies();
    assert_eq!(
        failed.players[0].counters["emergency_city_state_route_gold"],
        2
    );
    game.capture_city(objective, 2);
    game.do_liberate_city(2, objective).unwrap();
    assert!(game.active_emergencies.is_empty());
    assert_eq!(game.cities[&objective].owner, 1);
    assert_eq!(game.players[2].diplomatic_favor, 200.0);
    assert_eq!(game.players[2].counters["emergency_gold_per_envoy"], 1);
    assert_eq!(game.players[3].diplomatic_favor, 0.0);
    // The reward pays the PLAYER per placed Envoy (a top-bar term, like
    // Merchant Confederation), never a line of the capital's ledger.
    let member_capital = game.player_city_ids(2)[0];
    let mut without_reward = game.clone();
    without_reward.players[2]
        .counters
        .remove("emergency_gold_per_envoy");
    assert!(
        (game.player_policy_yields(2).gold - without_reward.player_policy_yields(2).gold - 3.0)
            .abs()
            < 1e-9
    );
    assert!(
        (game.city_yields(member_capital).gold - without_reward.city_yields(member_capital).gold)
            .abs()
            < 1e-9,
        "the capital's own yields carry none of it"
    );
}

#[test]
fn military_emergency_times_out_into_target_favor_and_city_strike_strength() {
    let mut game = emergency_game_with_capitals(3, 88_105, 300);
    game.world_era = 2;
    game.players[2].diplomatic_favor = 10.0;
    let position = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.city_at(*position).is_none())
        .unwrap();
    let objective = game.found_city_for(1, position, Some("Emergency City".to_string()));
    game.capture_city(objective, 0);
    game.do_keep_city(0, objective).unwrap();
    assert_eq!(game.pending_emergencies[0].kind, "military");
    let resolution = game.congress.as_ref().unwrap().resolutions[0].id.clone();
    game.do_congress_vote(0, &resolution, "B:oppose", 1)
        .unwrap();
    game.do_congress_vote(2, &resolution, "A:support", 2)
        .unwrap();
    game.turn = game.congress.as_ref().unwrap().closes;
    game.process_congress();
    let end = game.active_emergencies[0].ends;
    assert!(game.is_at_war(0, 2));
    assert!(game.do_make_peace(0, 2).is_err());

    let mut success = game.clone();
    success.capture_city(objective, 2);
    success.do_liberate_city(2, objective).unwrap();
    assert_eq!(success.players[2].diplomatic_favor, 150.0);
    assert_eq!(success.players[2].counters["emergency_heal_in:0"], 5);
    let enemy_city = success.player_city_ids(0)[0];
    let healer = success.spawn_test_unit("warrior", 2, success.cities[&enemy_city].pos);
    let mut without_reward = success.clone();
    without_reward.players[2]
        .counters
        .remove("emergency_heal_in:0");
    assert_eq!(
        success.unit_heal_rate(healer),
        without_reward.unit_heal_rate(healer) + 5
    );

    game.turn = end;
    game.process_emergencies();
    assert!(game.active_emergencies.is_empty());
    assert_eq!(game.players[0].diplomatic_favor, 200.0);
    assert_eq!(game.players[0].counters["emergency_city_strike_vs:2"], 2);
    assert!(game.do_make_peace(0, 2).is_ok());
}

#[test]
fn a_pending_offer_cannot_settle_a_war_before_its_minimum_turns() {
    let mut game = Game::new_full(2, 18, 10, 79_021, 200, 0, false);
    game.record_contact(0, 1);
    game.do_declare_war(0, 1).unwrap();
    game.players[0].gold = 100.0;
    let earliest = game
        .peace_available_at(0, 1)
        .expect("a war declared this turn cannot be settled yet");

    // An offer written for an earlier war stays pending across the peace,
    // the treaty and the next declaration.
    game.pending_deals.push(DiplomaticDeal {
        id: 401,
        from: 0,
        to: 1,
        give_gold: 50.0,
        request_gold: 0.0,
        open_borders: false,
        friendship: false,
        peace: true,
        alliance: None,
        defensive_pact: false,
        joint_war_target: None,
        promise: None,
        demand: false,
        expires: earliest + 5,
    });

    // Live, funded and unexpired — and still refused, because the war it
    // would end is being fought now and is one turn old.
    assert!(game.do_accept_deal(1, 401).is_err());
    assert!(game.is_at_war(0, 1));

    // The same offer settles it once the shipped minimum has run.
    game.turn = earliest;
    game.do_accept_deal(1, 401).unwrap();
    assert!(!game.is_at_war(0, 1));
    assert!(game.concluded_wars.last().unwrap().peace_terms[0]
        .terms
        .iter()
        .any(|term| term.contains("50 Gold")));
}

#[test]
fn emergency_combat_qualifies_a_remote_member_and_blocks_deal_peace() {
    let mut game = emergency_game_with_capitals(2, 88_106, 300);
    let target_city = game.player_city_ids(1)[0];
    game.at_war.insert(pair(0, 1));
    game.active_emergencies.push(Emergency {
        id: 901,
        kind: "military".to_string(),
        target: 1,
        city: target_city,
        original_owner: 0,
        members: [0].into_iter().collect(),
        contributions: BTreeMap::new(),
        started: game.turn,
        ends: game.turn + 30,
    });

    assert!(game
        .do_propose_deal(0, 1, 0.0, 0.0, false, false, true, None)
        .is_err());
    game.pending_deals.push(DiplomaticDeal {
        id: 902,
        from: 0,
        to: 1,
        give_gold: 0.0,
        request_gold: 0.0,
        open_borders: false,
        friendship: false,
        peace: true,
        alliance: None,
        defensive_pact: false,
        joint_war_target: None,
        promise: None,
        demand: false,
        expires: game.turn + 10,
    });
    assert!(game.do_accept_deal(1, 902).is_err());
    assert!(game.is_at_war(0, 1));

    let (attacker_pos, defender_pos) = game
        .map
        .tiles
        .iter()
        .filter(|(position, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.city_at(**position).is_none()
                && game.units_at(**position).is_empty()
        })
        .find_map(|(position, _)| {
            game.nbrs(*position).into_iter().find_map(|neighbor| {
                game.map.get(neighbor).and_then(|tile| {
                    (game.rules.is_passable(tile)
                        && !game.rules.is_water(tile)
                        && game.city_at(neighbor).is_none()
                        && game.units_at(neighbor).is_empty())
                    .then_some((*position, neighbor))
                })
            })
        })
        .expect("test map has an open land skirmish");
    let attacker = game.spawn_test_unit("warrior", 0, attacker_pos);
    game.spawn_test_unit("warrior", 1, defender_pos);
    game.do_attack(0, attacker, defender_pos).unwrap();
    assert!(game.active_emergencies[0].contributions[&0] > 0);

    game.resolve_emergency(901, true);
    assert_eq!(game.players[0].diplomatic_favor, 100.0);
}

#[test]
fn negotiated_peace_cedes_occupied_cities_and_own_cities_cannot_be_razed() {
    let mut game = emergency_game_with_capitals(2, 88_107, 300);
    let position = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| {
            game.city_at(*position).is_none()
                && game
                    .cities
                    .values()
                    .all(|city| game.wdist(city.pos, *position) >= 4)
        })
        .unwrap();
    let city = game.found_city_for(1, position, Some("Ceded City".to_string()));
    game.at_war.insert(pair(0, 1));
    game.capture_city(city, 0);
    game.do_keep_city(0, city).unwrap();
    assert_eq!(game.cities[&city].occupied_from, Some(1));

    // A war runs its shipped minimum before anyone can settle it.
    game.turn += 10;
    game.do_make_peace(0, 1).unwrap();
    assert_eq!(game.cities[&city].occupied_from, None);

    game.at_war.insert(pair(0, 1));
    game.capture_city(city, 1);
    assert!(!game
        .pending_city_capture_actions(1)
        .contains(&Action::RazeCity { city }));
    assert!(game.do_raze_city(1, city).is_err());
    assert!(game.cities.contains_key(&city));
}

#[test]
fn obsolete_units_leave_the_production_menu_but_stay_on_the_map() {
    let mut game = emergency_game_with_capitals(2, 5_501, 300);
    let city = game.player_city_ids(0)[0];
    let slinger = Item::Unit {
        unit: crate::name!("slinger"),
    };
    assert!(game.can_produce(0, city, &slinger));
    let veteran = game
        .place_new_unit("slinger", 0, game.cities[&city].pos)
        .unwrap();

    game.players[0].techs.insert(crate::name!("machinery"));
    assert!(!game.can_produce(0, city, &slinger));
    assert!(game
        .apply(
            0,
            &Action::Produce {
                city,
                item: slinger.clone()
            }
        )
        .is_err());
    // The Slinger already in the field is untouched; only the menu closed.
    assert_eq!(game.units[&veteran].kind, "slinger");
}

#[test]
fn gold_upgrades_advance_a_unit_and_keep_its_same_class_promotions() {
    let mut game = emergency_game_with_capitals(2, 5_502, 300);
    // Rome upgrades Warriors into Legions; this case is about the generic
    // path, so pin a civilization with no melee replacement.
    game.players[0].civ = "Egypt".to_string();
    let city = game.player_city_ids(0)[0];
    let pos = game.cities[&city].pos;
    let warrior = game.place_new_unit("warrior", 0, pos).unwrap();
    game.players[0].techs.insert(crate::name!("iron_working"));
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 60.0);
    game.players[0].gold = 500.0;
    {
        let unit = game.units.get_mut(&warrior).unwrap();
        unit.xp = 40;
        unit.level = 3;
        unit.hp = 62;
        unit.promotions.insert(crate::name!("battlecry"));
    }

    // Swordsman 90 - Warrior 40 = 50 Production of difference.
    let (target, gold, _) = game.unit_upgrade_price(0, "warrior").unwrap();
    assert_eq!(target, "swordsman");
    assert_eq!(gold, 110.0);

    assert!(game
        .legal_actions(0)
        .contains(&Action::UpgradeUnit { unit: warrior }));
    game.apply(0, &Action::UpgradeUnit { unit: warrior })
        .unwrap();
    let unit = &game.units[&warrior];
    assert_eq!(unit.kind, "swordsman");
    assert_eq!(unit.hp, 62); // damage carries across the upgrade
    assert_eq!(unit.xp, 40);
    assert_eq!(unit.level, 3);
    assert!(unit.promotions.contains(&Name::new("battlecry"))); // melee promotion kept
    assert_eq!(unit.moves_left, 0.0); // the upgrade is the unit's turn
    assert_eq!(game.players[0].gold, 390.0);
    assert!(game.strategic_stockpile(0, crate::name!("iron")) < 60.0);
}

#[test]
fn upgrades_need_friendly_territory_an_unspent_turn_and_the_treasury() {
    let mut game = emergency_game_with_capitals(2, 5_503, 300);
    game.players[0].techs.insert(crate::name!("iron_working"));
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 60.0);
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    let wild = game
        .map
        .tiles
        .iter()
        .find(|(pos, tile)| {
            tile.owner_city.is_none() && !game.rules.is_water(tile) && **pos != home
        })
        .map(|(pos, _)| *pos)
        .unwrap();

    let stray = game.place_new_unit("warrior", 0, wild).unwrap();
    game.players[0].gold = 500.0;
    assert!(game.unit_gold_upgrade_offer(0, stray).is_none()); // no friendly land

    let garrison = game.place_new_unit("warrior", 0, home).unwrap();
    game.players[0].gold = 100.0;
    assert!(game.unit_gold_upgrade_offer(0, garrison).is_none()); // too poor

    game.players[0].gold = 500.0;
    game.units.get_mut(&garrison).unwrap().acted = true;
    assert!(game.unit_gold_upgrade_offer(0, garrison).is_none()); // already moved

    game.units.get_mut(&garrison).unwrap().acted = false;
    assert!(game.unit_gold_upgrade_offer(0, garrison).is_some());
}

#[test]
fn the_war_ledger_records_a_declaration_its_cost_and_its_peace() {
    let mut game = emergency_game_with_capitals(3, 5_505, 300);
    game.turn = 40;
    game.record_contact(0, 1);
    let opening_strength = [game.military_power(0).round() as i64, game.military_power(1).round() as i64];
    game.do_declare_war(0, 1).unwrap();

    let key = pair(0, 1);
    let war = game.wars.get(&key).expect("the declaration opened a war");
    let defender_capital = game
        .cities
        .values()
        .find(|city| city.owner == 1 && city.is_capital)
        .unwrap()
        .pos;
    assert_eq!((war.aggressor, war.defender, war.started), (0, 1, 40));
    assert_eq!((war.declarer, war.target), (0, 1));
    assert_eq!(war.participants.len(), 2);
    assert_eq!(war.participants[0].strength, opening_strength[0]);
    assert_eq!(war.participants[1].strength, opening_strength[1]);
    assert_eq!(war.highlights[0].kind, "declared");
    assert_eq!(
        war.theater,
        [WarTheaterSite {
            turn: 40,
            pos: defender_capital,
        }],
        "the Watch action has a useful target before the first battle"
    );
    game.players[2].explored.remove(&defender_capital);
    let third_party_view = crate::obs::observation(&game, 2);
    let public_war = third_party_view["wars"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["aggressor"] == 0 && entry["defender"] == 1)
        .expect("a war is public knowledge to the civilizations watching it");
    assert!(
        public_war["theater"].as_array().unwrap().is_empty(),
        "the public declaration must not reveal an unexplored battlefield"
    );

    // A casualty on each side, then a city taken, is the whole vocabulary
    // of the log: two tallies and one moment worth naming.
    let capital = game.player_city_ids(1)[0];
    let capital_name = game.cities[&capital].name.clone();
    let (loser, near) = {
        let city = &game.cities[&capital];
        (city.owner, city.pos)
    };
    let victim = game.spawn_unit("warrior", 1, near);
    let victim_unit = game.units[&victim].clone();
    game.record_kill(0, Some("warrior"), &victim_unit);
    let mine = game.spawn_unit("warrior", 0, near);
    let mine_unit = game.units[&mine].clone();
    game.record_kill(1, Some("warrior"), &mine_unit);
    game.turn = 47;
    game.capture_city(capital, 0);

    let war = game.wars.get(&key).unwrap();
    assert_eq!(war.losses_for(1).units, 1);
    assert_eq!(war.losses_for(1).unit_kinds["warrior"], 1);
    assert_eq!(war.losses_for(0).units, 1);
    assert_eq!(war.losses_for(loser).cities, 1);
    assert_eq!(war.losses_for(loser).city_names, [capital_name]);
    let taken = war.highlights.last().unwrap();
    assert_eq!(
        (taken.kind.as_str(), taken.turn, taken.actor),
        ("capital_captured", 47, 0),
        "a capital changing hands is named as the moment it is"
    );
    assert!(taken.city.is_some());

    game.turn = 52;
    game.do_make_peace(0, 1).unwrap();
    assert!(game.wars.is_empty(), "peace closes the front");
    let concluded = game.concluded_wars.last().unwrap();
    assert_eq!(concluded.ended, Some(52));
    assert_eq!(concluded.highlights.last().unwrap().kind, "peace");
    assert_eq!(concluded.losses_for(1).cities, 1);
    assert_eq!(concluded.peace_terms.len(), 1);
    assert!(concluded.peace_terms[0]
        .terms
        .iter()
        .any(|term| term == "Current borders recognized"));
    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    let restored_war = restored.concluded_wars.last().unwrap();
    assert_eq!(restored_war.participants, concluded.participants);
    assert_eq!(restored_war.losses_for(1).unit_kinds["warrior"], 1);
    assert_eq!(restored_war.peace_terms, concluded.peace_terms);
    let seen = crate::obs::observation(&game, 0)["wars"].clone();
    let entry = seen
        .as_array()
        .unwrap()
        .iter()
        .find(|war| war["defender"] == 1)
        .expect("a concluded war stays in the log")
        .clone();
    assert_eq!(entry["outcome"], "peace");
    assert!(entry["victor"].is_null(), "a peace has no conqueror");
    let turns = entry["highlights"]
        .as_array()
        .unwrap()
        .iter()
        .map(|moment| moment["turn"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert!(turns.windows(2).all(|pair| pair[0] <= pair[1]));
    assert_eq!(entry["peace_terms"][0]["turn"], 52);
    let aftermath = crate::obs::observation_spectator(&game, 0)["wars"][0]["theater"].clone();
    assert!(
        aftermath
            .as_array()
            .is_some_and(|sites| !sites.is_empty()),
        "a concluded war exposes its durable battlefield to the full-map spectator"
    );
    game.turn = 92;
    assert_eq!(
        crate::obs::observation_spectator(&game, 0)["wars"][0]["theater"],
        aftermath,
        "later history must not move the View aftermath target"
    );
}

/// Headless rollouts switch the per-action ledger re-sync off. The
/// unconditional syncs — declarations, peaces, turn boundaries — must be
/// unaffected, because reports and saved games read the ledger there; and
/// the switch itself must be runtime-only so a save never comes back with
/// a stale spectator ledger.
#[test]
fn a_disabled_war_ledger_still_syncs_at_declarations_and_turn_boundaries() {
    let mut game = emergency_game_with_capitals(2, 5_512, 300);
    game.set_war_ledger(false);
    game.turn = 40;
    game.record_contact(0, 1);
    let start = game.military_power(0).round() as i64;
    game.do_declare_war(0, 1).unwrap();
    let participant = game.wars[&pair(0, 1)]
        .participants
        .iter()
        .find(|participant| participant.player == 0)
        .unwrap();
    assert_eq!(participant.strength, start, "a declaration always syncs");

    let capital = game.player_city_ids(0)[0];
    let reinforcement = game.spawn_unit("warrior", 0, game.cities[&capital].pos);
    let peak = game.military_power(0).round() as i64;
    assert!(peak > start);
    game.do_end_turn();
    let participant = game.wars[&pair(0, 1)]
        .participants
        .iter()
        .find(|participant| participant.player == 0)
        .unwrap();
    assert_eq!(
        participant.peak_strength,
        Some(peak),
        "a turn boundary always syncs"
    );
    game.remove_unit(reinforcement);

    let restored: Game =
        serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert!(
        restored.track_war_ledger,
        "the switch is runtime-only; a restored save narrates again"
    );
}

#[test]
fn war_strength_is_unit_only_and_saw_action_counts_distinct_units() {
    let mut game = emergency_game_with_capitals(2, 5_510, 300);
    game.turn = 40;
    game.record_contact(0, 1);
    let start = game.military_power(0).round() as i64;
    game.do_declare_war(0, 1).unwrap();

    let participant = game.wars[&pair(0, 1)]
        .participants
        .iter()
        .find(|participant| participant.player == 0)
        .unwrap();
    assert_eq!(participant.strength, start);
    assert_eq!(participant.peak_strength, Some(start));
    assert!(participant.saw_action_units.is_empty());

    // District defense never enters any of the three military measures.
    let capital = game.player_city_ids(0)[0];
    game.city_take_damage(1, capital, 20, 0.5, false);
    game.sync_war_log();
    let participant = game.wars[&pair(0, 1)]
        .participants
        .iter()
        .find(|participant| participant.player == 0)
        .unwrap();
    assert_eq!(participant.peak_strength, Some(start));
    assert!(participant.saw_action_units.is_empty());

    // Peak uses the same health-adjusted unit sum as the HUD.
    let reinforcement = game.spawn_unit("warrior", 0, game.cities[&capital].pos);
    game.units.get_mut(&reinforcement).unwrap().hp = 50;
    let peak = game.military_power(0).round() as i64;
    assert!(peak > start);
    game.do_end_turn();

    let participant = game.wars[&pair(0, 1)]
        .participants
        .iter()
        .find(|participant| participant.player == 0)
        .unwrap();
    assert_eq!(participant.peak_strength, Some(peak));

    game.remove_unit(reinforcement);
    game.do_end_turn();

    // Both sides of a real combat saw action. Their pre-combat military
    // values survive casualties and are keyed by unit, so a later action
    // by the same unit can raise but never duplicate its contribution.
    let (attacker_pos, defender_pos) = game
        .map
        .tiles
        .iter()
        .filter(|(position, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.city_at(**position).is_none()
                && game.units_at(**position).is_empty()
        })
        .find_map(|(position, _)| {
            game.nbrs(*position).into_iter().find_map(|neighbor| {
                game.map.get(neighbor).and_then(|tile| {
                    (game.rules.is_passable(tile)
                        && !game.rules.is_water(tile)
                        && game.city_at(neighbor).is_none()
                        && game.units_at(neighbor).is_empty())
                    .then_some((*position, neighbor))
                })
            })
        })
        .expect("test map has an open land skirmish");
    let attacker = game.spawn_test_unit("warrior", 0, attacker_pos);
    let defender = game.spawn_test_unit("warrior", 1, defender_pos);
    let attacker_before = game.units[&attacker].clone();
    let defender_before = game.units[&defender].clone();
    let attacker_value = game.rules.units["warrior"].strength.round() as i64;
    let action_peak = peak.max(game.military_power(0).round() as i64);
    game.do_attack(0, attacker, defender_pos).unwrap();

    let war = &game.wars[&pair(0, 1)];
    let attacker_party = war
        .participants
        .iter()
        .find(|participant| participant.player == 0)
        .unwrap();
    let defender_party = war
        .participants
        .iter()
        .find(|participant| participant.player == 1)
        .unwrap();
    assert_eq!(attacker_party.saw_action_units[&attacker], attacker_value);
    assert_eq!(defender_party.saw_action_units[&defender], attacker_value);
    game.record_war_unit_participation(&attacker_before, 1);
    game.record_war_unit_participation(&defender_before, 0);
    assert_eq!(
        game.wars[&pair(0, 1)]
            .participants
            .iter()
            .find(|participant| participant.player == 0)
            .unwrap()
            .saw_action_units
            .len(),
        1,
        "repeat action by one unit is not double-counted"
    );

    let observed = crate::obs::observation(&game, 0);
    let party = observed["wars"][0]["parties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|party| party["player"] == 0)
        .unwrap();
    assert_eq!(party["strength_start"], start);
    assert_eq!(party["strength_peak"], action_peak);
    assert_eq!(party["strength_saw_action"], attacker_value);
    assert!(party.get("strength_total").is_none());

    game.turn = 50;
    game.do_make_peace(0, 1).unwrap();
    let concluded = game.concluded_wars.last().unwrap();
    let participant = concluded
        .participants
        .iter()
        .find(|participant| participant.player == 0)
        .unwrap();
    assert!(participant.peak_strength.unwrap() >= action_peak);
    assert_eq!(participant.saw_action_units[&attacker], attacker_value);
    let observed = crate::obs::observation(&game, 0);
    let party = observed["wars"][0]["parties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|party| party["player"] == 0)
        .unwrap();
    assert_eq!(party["strength_saw_action"], attacker_value);
}

#[test]
fn a_city_strike_counts_the_defending_unit_but_not_city_defense() {
    let mut game = emergency_game_with_capitals(2, 5_511, 300);
    let capital = game.player_city_ids(0)[0];
    game.cities
        .get_mut(&capital)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    game.cities.get_mut(&capital).unwrap().wall_hp = 100;
    let origin = game.cities[&capital].pos;
    let target = game
        .nbrs(origin)
        .into_iter()
        .find(|position| {
            game.rules.is_passable(&game.map.tiles[position])
                && game.city_at(*position).is_none()
                && game.units_at(*position).is_empty()
        })
        .unwrap();
    let defender_id = game.spawn_unit("warrior", 1, target);
    let defender_value = game.rules.units["warrior"].strength.round() as i64;
    game.reveal(0, target, 2);
    game.do_declare_war(0, 1).unwrap();

    game.apply(
        0,
        &Action::CityStrike {
            city: capital,
            target,
        },
    )
    .unwrap();

    let participant = game.wars[&pair(0, 1)]
        .participants
        .iter()
        .find(|participant| participant.player == 0)
        .unwrap();
    assert!(participant.saw_action_units.is_empty());
    assert_eq!(
        participant.peak_strength,
        Some(game.military_power(0).round() as i64),
        "the striking city's defense is never military strength"
    );
    let defender = game.wars[&pair(0, 1)]
        .participants
        .iter()
        .find(|participant| participant.player == 1)
        .unwrap();
    assert_eq!(defender.saw_action_units[&defender_id], defender_value);
}

#[test]
fn a_defensive_pact_is_one_conflict_with_an_early_exit() {
    let mut game = emergency_game_with_capitals(3, 5_506, 300);
    game.turn = 40;
    game.record_contact(0, 1);
    let alliance = AllianceState {
        kind: "military".to_string(),
        points: 0.0,
        level: 1,
        ends: 100,
    };
    game.players[1].alliances.insert(2, alliance.clone());
    game.players[2].alliances.insert(1, alliance);
    game.players[1].defensive_pacts.insert(2, 100);
    game.players[2].defensive_pacts.insert(1, 100);
    game.do_declare_war(0, 1).unwrap();

    assert_eq!(game.wars.len(), 2, "the engine retains two combat fronts");
    let conflict_ids = game
        .wars
        .values()
        .map(|war| war.conflict)
        .collect::<BTreeSet<_>>();
    assert_eq!(conflict_ids.len(), 1, "both fronts share one declaration");

    let observed = crate::obs::observation(&game, 0);
    let conflicts = observed["wars"].as_array().unwrap();
    assert_eq!(conflicts.len(), 1, "the client receives one combined conflict");
    assert_eq!(conflicts[0]["aggressor"], 0);
    assert_eq!(conflicts[0]["defender"], 1);
    let right_side = conflicts[0]["parties"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|party| party["declarer_side"] == false)
        .map(|party| party["player"].as_u64().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(right_side, [1, 2].into_iter().collect());

    game.turn = 50;
    game.do_make_peace(0, 2).unwrap();
    assert!(game.is_at_war(0, 1));
    assert!(!game.is_at_war(0, 2));
    let observed = crate::obs::observation(&game, 0);
    let conflict = &observed["wars"][0];
    assert!(conflict["ended"].is_null(), "the principal war continues");
    let ally = conflict["parties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|party| party["player"] == 2)
        .unwrap();
    assert_eq!(ally["exited"], 50, "the infobox can say when the ally peaced out");
    assert_eq!(conflict["peace_terms"][0]["turn"], 50);
}

/// Without the shipped peace treaty two civilizations re-declare the turn
/// after signing, and what is one war reads in the log as a dozen.
#[test]
fn a_peace_treaty_binds_for_the_shipped_ten_turns() {
    let mut game = emergency_game_with_capitals(2, 5_505, 300);
    game.turn = 20;
    game.record_contact(0, 1);
    game.do_declare_war(0, 1).unwrap();
    game.turn = 25;
    assert!(
        game.do_make_peace(0, 1).is_err(),
        "a war runs its shipped ten turns before it can be settled"
    );
    game.turn = 30;
    game.do_make_peace(0, 1).unwrap();

    assert_eq!(game.peace_treaty_until(0, 1), Some(40));
    assert!(
        game.do_declare_war(0, 1).is_err(),
        "the ink is not dry on the treaty"
    );
    game.players[1].denounced_until.insert(0, game.turn + 20);
    assert!(
        game.do_declare_war_with_casus_belli(1, 0, "formal_war").is_err(),
        "no justification reopens a war the treaty closed"
    );
    assert!(
        !game
            .legal_actions(0)
            .iter()
            .any(|action| matches!(action, Action::DeclareWar { player: 1 })),
        "an agent is never offered a declaration it cannot make"
    );

    game.turn = 40;
    assert_eq!(game.peace_treaty_until(0, 1), None);
    game.do_declare_war(1, 0).unwrap();
    assert_eq!(
        game.concluded_wars.len(),
        1,
        "the first war is one entry, and the second is its own"
    );
    assert_eq!(game.wars.len(), 1);
    assert_eq!(game.wars[&pair(0, 1)].started, 40);
}

#[test]
fn defensive_pacts_do_not_reopen_a_front_inside_its_peace_treaty() {
    let mut game = emergency_game_with_capitals(3, 5_506, 300);
    game.turn = 20;
    game.record_contact(0, 1);
    game.record_contact(0, 2);
    game.do_declare_war(0, 1).unwrap();
    game.turn = 30;
    game.do_make_peace(0, 1).unwrap();

    let alliance = AllianceState {
        kind: "military".to_string(),
        points: 0.0,
        level: 1,
        ends: 80,
    };
    game.players[1].alliances.insert(2, alliance.clone());
    game.players[2].alliances.insert(1, alliance);
    game.players[1].defensive_pacts.insert(2, 80);
    game.players[2].defensive_pacts.insert(1, 80);

    game.turn = 31;
    game.do_declare_war(0, 2).unwrap();
    assert!(game.is_at_war(0, 2));
    assert!(game.peace_treaty_until(0, 1).is_some());
    assert!(
        !game.is_at_war(0, 1),
        "honoring an ally cannot break the treaty it signed last turn"
    );
    assert!(
        !game.wars.contains_key(&pair(0, 1)),
        "the protected pair must not open a second war record"
    );
}

/// A war can end without anyone signing anything. The ledger has to say so
/// — and has to stop showing it as a war still being fought.
#[test]
fn a_conquest_closes_the_war_it_ended() {
    let mut game = emergency_game_with_capitals(2, 5_505, 300);
    game.turn = 30;
    game.record_contact(0, 1);
    game.do_declare_war(0, 1).unwrap();
    let capital = game.player_city_ids(1)[0];
    game.turn = 44;
    game.capture_city(capital, 0);
    for uid in game.player_unit_ids(1) {
        game.remove_unit(uid);
    }
    game.check_elimination(1);

    assert!(!game.players[1].alive);
    assert!(
        game.wars.is_empty(),
        "nobody signs a peace with a civilization that no longer exists"
    );
    let war = game.concluded_wars.last().unwrap();
    assert_eq!(war.ended, Some(44), "the war ended the turn its loser did");
    let closing = war.highlights.last().unwrap();
    assert_eq!(
        (closing.kind.as_str(), closing.actor, closing.subject),
        ("conquest", 0, 1)
    );
    let seen = crate::obs::observation(&game, 0)["wars"].clone();
    let entry = seen
        .as_array()
        .unwrap()
        .iter()
        .find(|war| war["defender"] == 1)
        .expect("the conquest stays in the log")
        .clone();
    assert_eq!(entry["outcome"], "conquest");
    assert_eq!(entry["victor"], 0);
    assert_eq!(entry["sides"][1]["cities_lost"], 1);
}

#[test]
fn a_city_state_dragged_in_by_its_suzerain_fights_its_suzerains_war() {
    let mut game = emergency_game_with_capitals(2, 5_506, 300);
    let city_state = game.players.len();
    game.players
        .push(Player::new(city_state, "Valletta", true));
    game.turn = 15;
    for _ in 0..3 {
        game.players[1].envoys.push((city_state, 999));
    }
    assert_eq!(game.suzerain_of(city_state), Some(1));

    game.record_contact(0, 1);
    game.do_declare_war(0, 1).unwrap();
    assert!(
        game.is_at_war(0, city_state),
        "a Suzerain brings its city-state"
    );
    assert_eq!(
        game.wars.len(),
        1,
        "one declaration is one war, however many city-states it drags in"
    );
    assert!(!game.wars.contains_key(&pair(0, city_state)));

    // The city-state cannot repeatedly sign a no-op bilateral peace. Its
    // war is derived from its Suzerain, and will end with that principal
    // relation. This used to add a fresh diplomacy event every AI turn
    // while leaving the city-state at war.
    game.turn = 25;
    let notes_before = game.events.len();
    assert!(game.do_make_peace(city_state, 0).is_err());
    assert!(game.is_at_war(city_state, 0));
    assert_eq!(game.events.len(), notes_before);

    let war = &game.wars[&pair(0, 1)];
    let client = war
        .participants
        .iter()
        .find(|participant| participant.player == city_state)
        .expect("the city-state is listed beneath its Suzerain");
    assert_eq!(client.suzerain, Some(1));
    assert_eq!(client.entered, 15);

    // What the city-state loses is a cost of the war its patron declared,
    // but remains itemized under the city-state that actually lost it.
    let position = game.cities[&game.player_city_ids(0)[0]].pos;
    let levy = game.spawn_unit("warrior", city_state, position);
    let levy_unit = game.units[&levy].clone();
    game.record_kill(0, Some("warrior"), &levy_unit);
    let war = &game.wars[&pair(0, 1)];
    assert_eq!(war.losses_for(1).units, 0);
    assert_eq!(
        war.losses_for(city_state).units,
        1,
        "the summary names the civilization that paid the loss"
    );
    assert_eq!(war.losses_for(city_state).unit_kinds["warrior"], 1);

    // Losing suzerainty takes the city-state back out of the war without
    // touching the war itself.
    game.players[1].envoys.clear();
    game.sync_war_log();
    assert!(!game.is_at_war(0, city_state));
    assert_eq!(game.wars.len(), 1);
    assert!(game.concluded_wars.is_empty());
    let client = game.wars[&pair(0, 1)]
        .participants
        .iter()
        .find(|participant| participant.player == city_state)
        .unwrap();
    assert_eq!(client.exited, Some(25));

    game.turn = 27;
    for _ in 0..3 {
        game.players[1].envoys.push((city_state, 999));
    }
    game.sync_war_log();
    let intervals = game.wars[&pair(0, 1)]
        .participants
        .iter()
        .filter(|participant| participant.player == city_state)
        .collect::<Vec<_>>();
    assert_eq!(intervals.len(), 2);
    assert_eq!(intervals[1].entered, 27);
    assert_eq!(intervals[1].exited, None);

    // Two stretches are still one belligerent. The log gives an entity one
    // section carrying its whole involvement, so the observation merges the
    // intervals rather than sending the same city-state twice — and the
    // toll it paid is counted once however many times it was dragged in.
    let observed = crate::obs::observation(&game, 0);
    let seen = observed["wars"][0]["parties"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|party| party["player"] == city_state)
        .collect::<Vec<_>>();
    assert_eq!(seen.len(), 1, "one belligerent is one entry");
    assert_eq!(seen[0]["entered"], 15);
    assert!(seen[0]["exited"].is_null(), "it is back in the war");
    assert_eq!(
        seen[0]["intervals"],
        serde_json::json!([
            {"entered": 15, "exited": 25},
            {"entered": 27, "exited": null},
        ])
    );
    assert_eq!(seen[0]["units_lost"], 1);
    assert_eq!(seen[0]["unit_kinds"]["warrior"], 1);
}

/// Ranking the cities a war took needs two facts that do not survive the
/// event: a razed city has no population left to read, and a captured one
/// keeps growing under its new owner. Both are recorded as the city falls.
#[test]
fn a_captured_city_is_recorded_with_what_ranks_it() {
    let mut game = emergency_game_with_capitals(2, 5_507, 300);
    game.turn = 30;
    game.record_contact(0, 1);
    game.do_declare_war(0, 1).unwrap();
    let capital = game.player_city_ids(1)[0];
    game.cities.get_mut(&capital).unwrap().pop = 7;
    let name = game.cities[&capital].name.clone();
    game.capture_city(capital, 0);

    let losses = game.wars[&pair(0, 1)].losses_for(1).city_losses.clone();
    assert_eq!(losses.len(), 1);
    assert_eq!(losses[0].name, name);
    assert_eq!(losses[0].turn, 30);
    assert_eq!(losses[0].pop, 7, "the population that changed hands");
    assert!(
        losses[0].capital,
        "its own capital is the largest loss a civilization can take"
    );

    let observed = crate::obs::observation(&game, 0);
    let party = observed["wars"][0]["parties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|party| party["player"] == 1)
        .unwrap()
        .clone();
    assert_eq!(party["city_losses"][0]["name"], name);
    assert_eq!(party["city_losses"][0]["pop"], 7);
    assert_eq!(party["city_losses"][0]["capital"], true);
    assert_eq!(party["city_names"][0], name);
}

#[test]
fn a_civilizations_unique_unit_stands_in_for_the_upgrade_target() {
    let mut game = emergency_game_with_capitals(2, 5_504, 300);
    game.players[0].civ = "Rome".to_string();
    game.players[0].techs.clear();
    game.players[0].techs.insert(crate::name!("iron_working"));
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 40.0);
    let (target, _, _) = game.unit_upgrade_price(0, "warrior").unwrap();
    assert_eq!(target, "legion");
    // Rome's Legion carries the shipped Swordsman upgrade path onward.
    assert_eq!(game.rules.units["legion"].upgrade_to.as_deref(), Some("man_at_arms"));
}

#[test]
fn tagma_replaces_knights_buffs_its_formation_and_is_the_hippodrome_reward() {
    let mut game = emergency_game_with_capitals(2, 5_505, 300);
    game.players[0].civ = "Byzantium".to_string();
    game.players[0].civics.insert(crate::name!("divine_right"));
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 100.0);
    let city = game.player_city_ids(0)[0];
    assert!(!game.can_produce(
        0,
        city,
        &Item::Unit {
            unit: crate::name!("knight"),
        },
    ));
    assert!(game.can_produce(
        0,
        city,
        &Item::Unit {
            unit: crate::name!("tagma"),
        },
    ));
    let tagma = &game.rules.units["tagma"];
    assert_eq!(tagma.cost, 180.0);
    assert_eq!(tagma.maintenance, 3.0);
    assert_eq!(tagma.upgrade_to.as_deref(), Some("tank"));

    let warrior = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "warrior")
        .unwrap();
    let baseline = game.unit_unembarked_strength(&game.units[&warrior]);
    let adjacent = game
        .nbrs(game.units[&warrior].pos)
        .into_iter()
        .find(|position| {
            game.rules.is_passable(&game.map.tiles[position])
                && !game.rules.is_water(&game.map.tiles[position])
                && game.units_at(*position).is_empty()
        })
        .unwrap();
    game.spawn_test_unit("tagma", 0, adjacent);
    assert_eq!(
        game.unit_unembarked_strength(&game.units[&warrior]),
        baseline + 4.0
    );

    let before = game.player_unit_ids(0).len();
    game.grant_heavy_cavalry(0, city);
    let granted: Vec<_> = game
        .player_unit_ids(0)
        .into_iter()
        .skip(before)
        .map(|unit| game.units[&unit].kind.as_str())
        .collect();
    assert_eq!(granted, vec!["tagma"]);
}

#[test]
fn taxis_counts_every_holy_city_following_byzantiums_religion() {
    let mut game = emergency_game_with_capitals(2, 5_506, 300);
    game.players[0].civ = "Byzantium".to_string();
    game.players[0].religion = Some("Eastern Orthodoxy".to_string());
    game.players[1].religion = Some("Rival Faith".to_string());
    let byzantine_city = game.player_city_ids(0)[0];
    let rival_city = game.player_city_ids(1)[0];
    game.players[0].holy_city = Some(byzantine_city);
    game.players[1].holy_city = Some(rival_city);
    for city in [byzantine_city, rival_city] {
        game.cities
            .get_mut(&city)
            .unwrap()
            .pressure
            .insert("Eastern Orthodoxy".to_string(), 1_000.0);
    }
    assert_eq!(game.taxis_holy_city_strength(0), 6.0);
}
