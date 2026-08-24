use crate::name::Name;
use super::*;

fn scientist_game(seed: u64) -> (Game, u32, Pos) {
    let mut game = Game::new_full(1, 24, 16, seed, 300, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    let campus = install_test_district(&mut game, city, "campus");
    (game, city, campus)
}

fn set_test_city_amenity_surplus(game: &mut Game, city: u32, surplus: i64) {
    game.observed_city_amenity_adjustments.remove(&city);
    let modeled = game.city_amenity_surplus(&game.cities[&city]);
    game.observed_city_amenity_adjustments
        .insert(city, surplus - modeled);
}

fn great_person_points(game: &Game, kind: &str) -> f64 {
    game.great_person_points_per_turn(0)
        .get(kind)
        .copied()
        .unwrap_or(0.0)
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn scottish_enlightenment_scales_happy_and_ecstatic_yields_and_gpp() {
    let (mut scotland, city, campus) = scientist_game(95_010);
    scotland.players[0].civ = "Scotland".to_string();
    install_test_district(&mut scotland, city, "industrial_zone");
    // Rome keeps precisely the same board and normal Happiness band. It is a
    // control for Scotland's source-specific portion of the additive percent
    // sum, independent of any unrelated city modifiers in the fixture.
    let mut rome = scotland.clone();
    rome.players[0].civ = "Rome".to_string();

    set_test_city_amenity_surplus(&mut scotland, city, 0);
    set_test_city_amenity_surplus(&mut rome, city, 0);
    assert_eq!(scotland.city_happiness(&scotland.cities[&city]), "content");
    let scotland_content = scotland.city_yields(city);
    let rome_content = rome.city_yields(city);
    assert_close(scotland_content.science, rome_content.science);
    assert_close(scotland_content.production, rome_content.production);
    assert_close(
        great_person_points(&scotland, "scientist"),
        great_person_points(&rome, "scientist"),
    );
    assert_close(
        great_person_points(&scotland, "engineer"),
        great_person_points(&rome, "engineer"),
    );

    set_test_city_amenity_surplus(&mut scotland, city, 3);
    set_test_city_amenity_surplus(&mut rome, city, 3);
    assert_eq!(scotland.city_happiness(&scotland.cities[&city]), "happy");
    let scotland_happy = scotland.city_yields(city);
    let rome_happy = rome.city_yields(city);
    // Scotland's +5% is half of the normal Happy +10% band, while its Great
    // Person modifiers give one point from each exact active district.
    assert_close(
        scotland_happy.science - rome_happy.science,
        (rome_happy.science - rome_content.science) / 2.0,
    );
    assert_close(
        scotland_happy.production - rome_happy.production,
        (rome_happy.production - rome_content.production) / 2.0,
    );
    assert_close(
        great_person_points(&scotland, "scientist") - great_person_points(&rome, "scientist"),
        1.0,
    );
    assert_close(
        great_person_points(&scotland, "engineer") - great_person_points(&rome, "engineer"),
        1.0,
    );

    set_test_city_amenity_surplus(&mut scotland, city, 5);
    set_test_city_amenity_surplus(&mut rome, city, 5);
    assert_eq!(scotland.city_happiness(&scotland.cities[&city]), "ecstatic");
    let scotland_ecstatic = scotland.city_yields(city);
    let rome_ecstatic = rome.city_yields(city);
    // Ecstatic doubles every Scottish Enlightenment amount: +10% yields and
    // two points from each matching district.
    assert_close(
        scotland_ecstatic.science - rome_ecstatic.science,
        (rome_ecstatic.science - rome_content.science) / 2.0,
    );
    assert_close(
        scotland_ecstatic.production - rome_ecstatic.production,
        (rome_ecstatic.production - rome_content.production) / 2.0,
    );
    assert_close(
        great_person_points(&scotland, "scientist") - great_person_points(&rome, "scientist"),
        2.0,
    );
    assert_close(
        great_person_points(&scotland, "engineer") - great_person_points(&rome, "engineer"),
        2.0,
    );

    // The source requirements name the active Campus exactly; a pillaged one
    // removes both its ordinary and Scottish Scientist points.
    scotland.map.tiles.get_mut(&campus).unwrap().pillaged = true;
    rome.map.tiles.get_mut(&campus).unwrap().pillaged = true;
    assert_close(
        great_person_points(&scotland, "scientist"),
        great_person_points(&rome, "scientist"),
    );
}

fn recruit_current_scientist(game: &mut Game) -> String {
    let expected = game
        .current_great_person("scientist")
        .unwrap()
        .0
        .to_string();
    let cost = game.gp_cost(0, "scientist");
    game.players[0].gpp.insert("scientist".to_string(), cost);
    game.claim_great_person(0, "scientist", None).unwrap();
    assert_eq!(game.players[0].great_people.last(), Some(&expected));
    expected
}

fn recruit_current_engineer(game: &mut Game) -> String {
    let expected = game.current_great_person("engineer").unwrap().0.to_string();
    let cost = game.gp_cost(0, "engineer");
    game.players[0].gpp.insert("engineer".to_string(), cost);
    game.claim_great_person(0, "engineer", None).unwrap();
    assert_eq!(game.players[0].great_people.last(), Some(&expected));
    expected
}

fn recruit_current_merchant(game: &mut Game) -> String {
    let expected = game.current_great_person("merchant").unwrap().0.to_string();
    let cost = game.gp_cost(0, "merchant");
    game.players[0].gpp.insert("merchant".to_string(), cost);
    game.claim_great_person(0, "merchant", None).unwrap();
    assert_eq!(game.players[0].great_people.last(), Some(&expected));
    expected
}

/// Move the global market on to a named individual by retiring everyone their
/// class offers first.
///
/// ⚠ THESE TESTS USED TO WALK THE QUEUE BY POSITION, and #2377 took the roster
/// from 65 individuals to 147. `Game::current_great_person` offers the lowest
/// era first and breaks ties alphabetically by id, so filling in Gathering
/// Storm's Classical Scientists put `aryabhata` in front of `hypatia` and every
/// "the third Scientist is Newton" assertion below became a statement about the
/// roster's length rather than about the person it names. Naming the person is
/// the assertion each of these tests was always making.
fn skip_to_great_person(game: &mut Game, id: &str) {
    let kind = game.rules.great_people[id].kind.clone();
    loop {
        let current = game
            .current_great_person(&kind)
            .expect("the class still offers somebody")
            .0
            .to_string();
        if current == id {
            return;
        }
        game.retired_great_people.insert(current);
    }
}

fn recruit_current_military_person(game: &mut Game, kind: &str) -> String {
    let expected = game.current_great_person(kind).unwrap().0.to_string();
    let cost = game.gp_cost(0, kind);
    game.players[0].gpp.insert(kind.to_string(), cost);
    game.claim_great_person(0, kind, None).unwrap();
    assert_eq!(game.players[0].great_people.last(), Some(&expected));
    expected
}

#[test]
fn named_scientists_grant_exact_buildings_science_and_era_boosts() {
    let (mut game, city, _) = scientist_game(95_001);
    let initial_science = game.city_yields(city).science;
    let initial_boosts = game.players[0].boosted_techs.clone();

    skip_to_great_person(&mut game, "hypatia");
    assert_eq!(recruit_current_scientist(&mut game), "hypatia");
    assert!(game.cities[&city]
        .buildings
        .contains(&crate::name!("library")));
    assert_eq!(game.players[0].boosted_techs, initial_boosts);
    assert!(
        (game.city_yields(city).science - initial_science
            - (game.rules.buildings["library"].yields.science + 1.0))
            .abs()
            < 1e-9
    );

    // Omar Khayyam is the Medieval Scientist the roster had no entry for. His
    // free Library is a no-op in a city that already has one -- exactly
    // Hypatia's case above -- while his +1 Science to Libraries still pays.
    let before_khayyam = game.city_yields(city).science;
    skip_to_great_person(&mut game, "omar_khayyam");
    assert_eq!(recruit_current_scientist(&mut game), "omar_khayyam");
    assert_eq!(game.players[0].boosted_techs, initial_boosts);
    assert!((game.city_yields(city).science - before_khayyam - 1.0).abs() < 1e-9);

    let before_newton = game.city_yields(city).science;
    skip_to_great_person(&mut game, "isaac_newton");
    assert_eq!(recruit_current_scientist(&mut game), "isaac_newton");
    assert!(game.cities[&city]
        .buildings
        .contains(&crate::name!("university")));
    assert_eq!(game.players[0].boosted_techs, initial_boosts);
    assert!(
        (game.city_yields(city).science - before_newton
            - (game.rules.buildings["university"].yields.science + 2.0))
            .abs()
            < 1e-9
    );

    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("research_lab"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .building_eras
        .insert(crate::name!("research_lab"), game.world_era);
    // Charles Darwin is the Industrial Scientist the roster had no entry for,
    // and grants what Newton does: the University is already standing, so only
    // the permanent +2 Science to Universities lands.
    let before_darwin = game.city_yields(city).science;
    skip_to_great_person(&mut game, "charles_darwin");
    assert_eq!(recruit_current_scientist(&mut game), "charles_darwin");
    assert!((game.city_yields(city).science - before_darwin - 2.0).abs() < 1e-9);

    let before_einstein = game.city_yields(city).science;
    let boosts_before_einstein = game.players[0].boosted_techs.clone();

    skip_to_great_person(&mut game, "albert_einstein");
    assert_eq!(recruit_current_scientist(&mut game), "albert_einstein");
    assert!((game.city_yields(city).science - before_einstein - 4.0).abs() < 1e-9);
    let new_boosts: Vec<&Name> = game.players[0]
        .boosted_techs
        .difference(&boosts_before_einstein)
        .collect();
    assert_eq!(new_boosts.len(), 1);
    assert!((5..=6).contains(&game.rules.techs[new_boosts[0]].era));

    let active_science = game.city_yields(city).science;
    game.cities
        .get_mut(&city)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("research_lab"));
    assert!((active_science - game.city_yields(city).science - 7.0).abs() < 1e-9);
    game.cities
        .get_mut(&city)
        .unwrap()
        .pillaged_buildings
        .remove(&Name::new("research_lab"));

    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(restored.city_yields(city), game.city_yields(city));
    assert_eq!(
        restored.players[0]
            .counters
            .get("great_person:research_lab_science"),
        Some(&4)
    );
}

#[test]
fn great_scientist_yield_bonuses_apply_to_unique_building_families() {
    let (mut game, city, _) = scientist_game(95_002);
    game.players[0].civ = "Arabia".to_string();
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .extend([crate::name!("library"), crate::name!("madrasa")]);
    game.players[0]
        .counters
        .insert("great_person:library_science".to_string(), 1);
    game.players[0]
        .counters
        .insert("great_person:university_science".to_string(), 2);
    let with_bonuses = game.city_yields(city).science;

    game.players[0]
        .counters
        .remove("great_person:library_science");
    game.players[0]
        .counters
        .remove("great_person:university_science");
    assert!((with_bonuses - game.city_yields(city).science - 3.0).abs() < 1e-9);
}

#[test]
fn named_engineers_apply_exact_charges_wonder_gates_and_workshop_culture() {
    let mut game = Game::new_full(1, 24, 16, 95_003, 300, 0, false);
    // Great Engineer behavior is civilization-independent. Pin the fixture so
    // map-generation additions cannot indirectly select a yield-changing
    // civilization and turn this into a unique-ability test.
    game.players[0].civ = "Rome".to_string();
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    install_test_district(&mut game, city, "industrial_zone");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("workshop"));
    let wonder_site = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();

    game.players[0].gpp.insert("engineer".to_string(), 60.0);
    assert_eq!(game.current_great_person("engineer").unwrap().1.era, 2);
    assert!(game.claim_great_person(0, "engineer", None).is_err());
    assert!(!game.retired_great_people.contains("imhotep"));

    game.cities.get_mut(&city).unwrap().queue = vec![Item::Wonder {
        wonder: crate::name!("pyramids"),
        pos: wonder_site,
    }];
    skip_to_great_person(&mut game, "imhotep");
    assert_eq!(recruit_current_engineer(&mut game), "imhotep");
    assert_eq!(game.cities[&city].production, 700.0);

    game.cities.get_mut(&city).unwrap().queue.clear();
    let culture_before = game.city_yields(city).culture;
    let boosts_before = game.players[0].boosted_techs.clone();
    skip_to_great_person(&mut game, "leonardo_da_vinci");
    assert_eq!(recruit_current_engineer(&mut game), "leonardo_da_vinci");
    assert!((game.city_yields(city).culture - culture_before - 3.0).abs() < 1e-9);
    let new_boosts: Vec<&Name> = game.players[0]
        .boosted_techs
        .difference(&boosts_before)
        .collect();
    assert_eq!(new_boosts.len(), 1);
    assert_eq!(game.rules.techs[new_boosts[0]].era, 5);

    game.cities.get_mut(&city).unwrap().production = 0.0;
    game.cities.get_mut(&city).unwrap().queue = vec![Item::Wonder {
        wonder: crate::name!("eiffel_tower"),
        pos: wonder_site,
    }];
    assert_eq!(game.current_great_person("engineer").unwrap().1.era, 4);
    skip_to_great_person(&mut game, "gustave_eiffel");
    assert_eq!(recruit_current_engineer(&mut game), "gustave_eiffel");
    assert_eq!(game.cities[&city].production, 960.0);

    let active_workshop_culture = game.city_yields(city).culture;
    game.cities
        .get_mut(&city)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("workshop"));
    assert!(active_workshop_culture - game.city_yields(city).culture >= 3.0 - 1e-9);
}

#[test]
fn immediate_great_people_require_stock_activation_sites_and_complete_work_capacity() {
    let mut game = Game::new_full(1, 24, 16, 95_008, 300, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);

    let scientist_cost = game.gp_cost(0, "scientist");
    game.players[0]
        .gpp
        .insert("scientist".to_string(), scientist_cost);
    let scientist = Action::RecruitGreatPerson {
        kind: "scientist".to_string(),
    };
    assert!(!game.can_activate_current_great_person(0, "scientist"));
    assert!(!game.legal_actions(0).contains(&scientist));
    assert!(game.claim_great_person(0, "scientist", None).is_err());
    let campus = install_test_district(&mut game, city, "campus");
    game.map.tiles.get_mut(&campus).unwrap().pillaged = true;
    assert!(!game.can_activate_current_great_person(0, "scientist"));
    game.map.tiles.get_mut(&campus).unwrap().pillaged = false;
    assert!(game.can_activate_current_great_person(0, "scientist"));
    assert!(game.legal_actions(0).contains(&scientist));
    game.claim_great_person(0, "scientist", None).unwrap();

    let wonder_site = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| {
            *position != game.cities[&city].pos && game.map.tiles[position].district.is_none()
        })
        .unwrap();
    game.cities.get_mut(&city).unwrap().queue = vec![Item::Wonder {
        wonder: crate::name!("pyramids"),
        pos: wonder_site,
    }];
    let engineer_cost = game.gp_cost(0, "engineer");
    game.players[0]
        .gpp
        .insert("engineer".to_string(), engineer_cost);
    game.claim_great_person(0, "engineer", None).unwrap();
    game.cities.get_mut(&city).unwrap().queue.clear();
    // Isidore of Miletus and Filippo Brunelleschi are wonder-gated like
    // Imhotep. Leonardo is the first Engineer whose gate is the Industrial
    // Zone, which is the gate this block is about.
    skip_to_great_person(&mut game, "leonardo_da_vinci");
    let leonardo_cost = game.gp_cost(0, "engineer");
    game.players[0]
        .gpp
        .insert("engineer".to_string(), leonardo_cost);
    assert!(!game.can_activate_current_great_person(0, "engineer"));
    install_test_district(&mut game, city, "industrial_zone");
    assert!(game.can_activate_current_great_person(0, "engineer"));

    let mut culture = Game::new_full(1, 24, 16, 95_009, 300, 0, false);
    let settler = culture
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| culture.units[unit].kind == "settler")
        .unwrap();
    let city = culture.found_city_for(0, culture.units[&settler].pos, None);
    let writer_cost = culture.gp_cost(0, "writer");
    culture.players[0]
        .gpp
        .insert("writer".to_string(), writer_cost);
    let writer = Action::RecruitGreatPerson {
        kind: "writer".to_string(),
    };
    assert!(culture.can_house_additional_great_work(0, "writing"));
    assert!(!culture.can_house_great_works(0, "writing", 2));
    assert!(!culture.can_activate_current_great_person(0, "writer"));
    assert!(!culture.legal_actions(0).contains(&writer));
    assert!(culture.claim_great_person(0, "writer", None).is_err());
    install_test_district(&mut culture, city, "theater_square");
    culture
        .cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("amphitheater"));
    assert!(culture.can_house_great_works(0, "writing", 2));
    assert!(culture.can_activate_current_great_person(0, "writer"));
    assert!(culture.legal_actions(0).contains(&writer));
    culture.claim_great_person(0, "writer", None).unwrap();
    assert_eq!(culture.housed_great_work_count(0, "writing"), 2);

    let mut religion = Game::new_full(1, 24, 16, 95_010, 300, 0, false);
    let settler = religion
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| religion.units[unit].kind == "settler")
        .unwrap();
    let city = religion.found_city_for(0, religion.units[&settler].pos, None);
    let prophet_cost = religion.gp_cost(0, "prophet");
    religion.players[0]
        .gpp
        .insert("prophet".to_string(), prophet_cost);
    assert!(!religion.can_activate_current_great_person(0, "prophet"));
    let holy_site = install_test_district(&mut religion, city, "holy_site");
    religion.map.tiles.get_mut(&holy_site).unwrap().pillaged = true;
    assert!(!religion.can_activate_current_great_person(0, "prophet"));
    religion.map.tiles.get_mut(&holy_site).unwrap().pillaged = false;
    assert!(religion.can_activate_current_great_person(0, "prophet"));
    religion.claim_great_person(0, "prophet", None).unwrap();
    assert!(religion.players[0].prophet_pending);
    assert!(!religion.can_activate_current_great_person(0, "prophet"));
}

#[test]
fn named_merchants_annex_tiles_and_apply_exact_trade_and_oil_effects() {
    let mut game = Game::new_full(2, 28, 18, 95_004, 300, 0, false);
    let mut cities = Vec::new();
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        cities.push(game.found_city_for(pid, game.units[&settler].pos, None));
    }
    let merchant_city = cities[0];
    let foreign_city = cities[1];
    install_test_district(&mut game, merchant_city, "commercial_hub");
    game.players[0].civics.insert(crate::name!("foreign_trade"));

    let gold_before_crassus = game.players[0].gold;
    let envoys_before_crassus = game.players[0].envoys_free;
    let tiles_before_crassus = game.cities[&merchant_city].owned_tiles.len();
    skip_to_great_person(&mut game, "marcus_licinius_crassus");
    assert_eq!(
        recruit_current_merchant(&mut game),
        "marcus_licinius_crassus"
    );
    assert_eq!(game.players[0].gold - gold_before_crassus, 180.0);
    assert_eq!(game.players[0].envoys_free, envoys_before_crassus);
    assert_eq!(
        game.cities[&merchant_city].owned_tiles.len() - tiles_before_crassus,
        3
    );

    game.routes.push(TradeRoute {
        origin: foreign_city,
        dest: merchant_city,
        owner: 1,
        ends: game.turn + 30,
    });
    let foreign_origin_gold = game.city_yields(foreign_city).gold;
    let merchant_destination_gold = game.city_yields(merchant_city).gold;
    let traders_before = game
        .units
        .values()
        .filter(|unit| unit.owner == 0 && unit.kind == "trader")
        .count();
    let capacity_before = game.trade_capacity(0);
    let gold_before_marco = game.players[0].gold;
    skip_to_great_person(&mut game, "marco_polo");
    assert_eq!(recruit_current_merchant(&mut game), "marco_polo");
    assert_eq!(game.trade_capacity(0) - capacity_before, 1);
    assert_eq!(game.players[0].gold, gold_before_marco);
    assert_eq!(
        game.units
            .values()
            .filter(|unit| unit.owner == 0 && unit.kind == "trader")
            .count()
            - traders_before,
        1
    );
    let free_trader = game
        .units
        .values()
        .find(|unit| unit.owner == 0 && unit.kind == "trader")
        .unwrap();
    assert_ne!(
        free_trader.pos, game.cities[&merchant_city].pos,
        "the free Trader must obey civilian stacking around the activation city"
    );
    assert!(
        (game.city_yields(foreign_city).gold - foreign_origin_gold - 2.0).abs() < 1e-9
    );
    assert!(
        (game.city_yields(merchant_city).gold - merchant_destination_gold - 2.0).abs()
            < 1e-9
    );

    for position in game.cities[&foreign_city].owned_tiles.clone() {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.resource = None;
        tile.improvement = None;
        tile.pillaged = false;
    }
    let resource_tiles: Vec<Pos> = game.cities[&foreign_city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != game.cities[&foreign_city].pos)
        .take(2)
        .collect();
    assert_eq!(resource_tiles.len(), 2);
    for (position, resource, improvement) in [
        (resource_tiles[0], "iron", "mine"),
        (resource_tiles[1], "horses", "pasture"),
    ] {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.resource = Some(Name::new(resource));
        tile.improvement = Some(Name::new(improvement));
        tile.pillaged = false;
    }
    game.routes.push(TradeRoute {
        origin: merchant_city,
        dest: foreign_city,
        owner: 0,
        ends: game.turn + 30,
    });
    game.players[0].techs.insert(crate::name!("refining"));
    // The Renaissance and Industrial Merchants the roster had no entry for.
    // `gold` pays once per charge, as Crassus' 60 x 3 shows above, and these
    // two carry one charge each.
    let gold_before_fugger = game.players[0].gold;
    skip_to_great_person(&mut game, "jakob_fugger");
    assert_eq!(recruit_current_merchant(&mut game), "jakob_fugger");
    assert_eq!(game.players[0].gold - gold_before_fugger, 240.0);

    let gold_before_smith = game.players[0].gold;
    skip_to_great_person(&mut game, "adam_smith");
    assert_eq!(recruit_current_merchant(&mut game), "adam_smith");
    assert_eq!(game.players[0].gold - gold_before_smith, 420.0);

    let rockefeller_route_gold = game.city_yields(merchant_city).gold;
    let capacity_before_rockefeller = game.trade_capacity(0);
    let gold_before_rockefeller = game.players[0].gold;
    skip_to_great_person(&mut game, "john_rockefeller");
    assert_eq!(recruit_current_merchant(&mut game), "john_rockefeller");
    assert_eq!(game.trade_capacity(0), capacity_before_rockefeller);
    assert_eq!(game.players[0].gold, gold_before_rockefeller);
    assert!(
        (game.city_yields(merchant_city).gold - rockefeller_route_gold - 4.0).abs() < 1e-9
    );
    assert_eq!(
        game.trade_route_yields(0, foreign_city).gold - game.route_yields(foreign_city, false).gold,
        4.0
    );
    game.map.tiles.get_mut(&resource_tiles[0]).unwrap().pillaged = true;
    assert_eq!(
        game.trade_route_yields(0, foreign_city).gold - game.route_yields(foreign_city, false).gold,
        2.0
    );
    game.map.tiles.get_mut(&resource_tiles[0]).unwrap().pillaged = false;
    assert_eq!(game.strategic_resource_rate(0, "oil"), 3.0);
    game.process_strategic_resources(0);
    assert_eq!(game.strategic_stockpile(0, crate::name!("oil")), 3.0);

    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(
        restored.city_yields(merchant_city),
        game.city_yields(merchant_city)
    );
    assert_eq!(restored.strategic_resource_rate(0, "oil"), 3.0);
}

#[test]
fn named_generals_promote_or_form_exactly_one_land_unit() {
    let mut game = Game::new_full(1, 24, 16, 95_005, 300, 0, false);
    let position = game.player_unit_ids(0).into_iter().next().unwrap();
    let position = game.units[&position].pos;
    let target = game.spawn_unit("swordsman", 0, position);
    let untouched = game.spawn_unit("warrior", 0, position);

    skip_to_great_person(&mut game, "hannibal_barca");
    assert_eq!(
        recruit_current_military_person(&mut game, "general"),
        "hannibal_barca"
    );
    assert!(game.promotion_pending(target));
    assert_eq!(game.units[&untouched].xp, 0);

    skip_to_great_person(&mut game, "el_cid");
    assert_eq!(
        recruit_current_military_person(&mut game, "general"),
        "el_cid"
    );
    assert_eq!(game.units[&target].formation, 1);
    assert_eq!(game.units[&untouched].formation, 0);

    // Joan of Arc is the Renaissance General the roster had no entry for, so
    // the chain no longer jumps an era from Classical to Industrial. She
    // promotes rather than forms: `land_unit_formation` names the level to
    // reach, not a step, so a second level-one General is a no-op.
    skip_to_great_person(&mut game, "joan_of_arc");
    assert_eq!(
        recruit_current_military_person(&mut game, "general"),
        "joan_of_arc"
    );
    assert!(game.promotion_pending(target));
    assert_eq!(game.units[&target].formation, 1);

    skip_to_great_person(&mut game, "napoleon_bonaparte");
    assert_eq!(
        recruit_current_military_person(&mut game, "general"),
        "napoleon_bonaparte"
    );
    assert_eq!(game.units[&target].formation, 2);
    assert_eq!(game.units[&untouched].formation, 0);
}

#[test]
fn named_admirals_apply_exact_unit_trade_building_and_flanking_effects() {
    let mut game = Game::new_full(
        2, 28, 18, crate::rng::fixture_seed("ADMIRAL", 95_009), 300, 0, false,
    );
    let mut cities = Vec::new();
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        cities.push(game.found_city_for(pid, game.units[&settler].pos, None));
    }
    let admiral_city = cities[0];
    let foreign_city = cities[1];
    let harbor = install_test_district(&mut game, admiral_city, "harbor");
    game.map.tiles.get_mut(&harbor).unwrap().terrain = crate::name!("coast");
    game.players[0].civics.extend([
        crate::name!("foreign_trade"),
        crate::name!("military_tradition"),
    ]);

    let formation_ship = game.spawn_unit("galley", 0, harbor);
    skip_to_great_person(&mut game, "gaius_duilius");
    assert_eq!(
        recruit_current_military_person(&mut game, "admiral"),
        "gaius_duilius"
    );
    assert_eq!(game.units[&formation_ship].formation, 1);

    let quadrireme = Item::Unit {
        unit: crate::name!("quadrireme"),
    };
    let quadriremes = game
        .units
        .values()
        .filter(|unit| unit.owner == 0 && unit.kind == "quadrireme")
        .count();
    let naval_ranged_production = game.item_prod_mult(0, admiral_city, Some(&quadrireme));
    skip_to_great_person(&mut game, "themistocles");
    assert_eq!(
        recruit_current_military_person(&mut game, "admiral"),
        "themistocles"
    );
    assert_eq!(
        game.units
            .values()
            .filter(|unit| unit.owner == 0 && unit.kind == "quadrireme")
            .count()
            - quadriremes,
        1
    );
    assert!(
        (game.item_prod_mult(0, admiral_city, Some(&quadrireme)) - naval_ranged_production - 0.20)
            .abs()
            < 1e-9
    );

    game.routes.push(TradeRoute {
        origin: foreign_city,
        dest: admiral_city,
        owner: 1,
        ends: game.turn + 30,
    });
    let origin_gold = game.city_yields(foreign_city).gold;
    let destination_gold = game.city_yields(admiral_city).gold;
    let capacity = game.trade_capacity(0);
    let traders = game
        .units
        .values()
        .filter(|unit| unit.owner == 0 && unit.kind == "trader")
        .count();
    skip_to_great_person(&mut game, "zheng_he");
    assert_eq!(
        recruit_current_military_person(&mut game, "admiral"),
        "zheng_he"
    );
    assert_eq!(game.trade_capacity(0) - capacity, 1);
    assert_eq!(
        game.units
            .values()
            .filter(|unit| unit.owner == 0 && unit.kind == "trader")
            .count()
            - traders,
        1
    );
    assert!((game.city_yields(foreign_city).gold - origin_gold - 2.0).abs() < 1e-9);
    assert!((game.city_yields(admiral_city).gold - destination_gold - 2.0).abs() < 1e-9);

    skip_to_great_person(&mut game, "santa_cruz");
    assert_eq!(
        recruit_current_military_person(&mut game, "admiral"),
        "santa_cruz"
    );
    assert_eq!(game.units[&formation_ship].formation, 2);

    let target = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.nbrs(*position).len() == 6)
        .unwrap();
    let ring = game.nbrs(target);
    for position in std::iter::once(target).chain(ring.iter().copied()) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("coast");
        tile.feature = None;
    }
    let attacker = game.spawn_unit("galley", 0, ring[0]);
    game.spawn_unit("galley", 0, ring[1]);
    game.spawn_unit("galley", 1, target);
    assert_eq!(game.flanking_bonus(attacker, target), 2.0);
    skip_to_great_person(&mut game, "horatio_nelson");
    assert_eq!(
        recruit_current_military_person(&mut game, "admiral"),
        "horatio_nelson"
    );
    assert!(game.cities[&admiral_city]
        .buildings
        .contains(&crate::name!("lighthouse")));
    assert!(game.cities[&admiral_city]
        .buildings
        .contains(&crate::name!("shipyard")));
    assert_eq!(game.flanking_bonus(attacker, target), 3.0);

    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(restored.flanking_bonus(attacker, target), 3.0);
    assert!(
        (restored.item_prod_mult(0, admiral_city, Some(&quadrireme))
            - naval_ranged_production
            - 0.20)
            .abs()
            < 1e-9
    );
}

#[test]
fn great_person_eras_offer_prices_and_patronage_follow_stock_rules() {
    let (mut game, _, _) = scientist_game(95_007);

    for (id, era, cost) in [
        ("donatello", 3, 240.0),
        ("rembrandt", 4, 420.0),
        ("claude_monet", 6, 960.0),
        ("antonio_vivaldi", 4, 420.0),
        ("ludwig_van_beethoven", 4, 420.0),
        ("liu_tianhua", 5, 660.0),
        ("leo_tolstoy", 5, 660.0),
    ] {
        let person = &game.rules.great_people[id];
        assert_eq!((person.era, person.cost), (era, cost));
    }

    // Classical non-art people are one era ahead of an Ancient world.
    assert_eq!(game.gp_cost(0, "scientist"), 78.0);
    // Imhotep is two eras ahead: floor(120 * 1.6^2) = 307.
    assert_eq!(game.gp_cost(0, "engineer"), 307.0);
    // Art people and Prophets never receive the ahead-of-era multiplier.
    assert_eq!(game.gp_cost(0, "writer"), 60.0);
    assert_eq!(game.gp_cost(0, "artist"), 240.0);
    assert_eq!(game.gp_cost(0, "musician"), 420.0);
    assert_eq!(game.gp_cost(0, "prophet"), 60.0);

    let locked_engineer_cost = game.gp_cost(0, "engineer");
    game.world_era = 2;
    assert_eq!(game.gp_cost(0, "engineer"), locked_engineer_cost);
    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(restored.gp_cost(0, "engineer"), locked_engineer_cost);

    // A newly exposed person is priced in the world era at reveal time. Retire
    // the rest of the Classical bench so the next reveal crosses an era.
    game.world_era = 3;
    let classical_bench: Vec<String> = game
        .rules
        .great_people
        .iter()
        .filter(|(id, spec)| spec.kind == "scientist" && spec.era == 1 && id.as_str() != "hypatia")
        .map(|(id, _)| id.to_string())
        .collect();
    game.retired_great_people.extend(classical_bench);
    let hypatia_cost = game.gp_cost(0, "scientist");
    game.players[0]
        .gpp
        .insert("scientist".to_string(), hypatia_cost);
    game.claim_great_person(0, "scientist", None).unwrap();
    // The first Medieval Scientist the shipped game offers; the market used to
    // skip straight from Hypatia to Newton.
    assert_eq!(
        game.current_great_person("scientist").unwrap().0,
        "abu_al_qasim_al_zahrawi"
    );
    assert_eq!(game.gp_cost(0, "scientist"), 120.0);

    let (mut patronage, _, _) = scientist_game(95_008);
    assert_eq!(
        patronage.great_person_patronage_price(0, "scientist", "gold"),
        Some(1_370.0)
    );
    assert_eq!(
        patronage.great_person_patronage_price(0, "scientist", "faith"),
        Some(930.0)
    );
    patronage.players[0].gold = 1_369.0;
    assert!(patronage
        .claim_great_person(0, "scientist", Some("gold"))
        .is_err());
    patronage.players[0].gold = 1_370.0;
    patronage
        .claim_great_person(0, "scientist", Some("gold"))
        .unwrap();
    assert_eq!(patronage.players[0].gold, 0.0);
}

/// Every effect key the roster spells is one the engine actually reads.
///
/// ★★★★ A GREAT PERSON WHOSE EFFECT NEVER FIRES IS WORSE THAN AN ABSENT ONE.
/// `Game::current_great_person` offers one individual per class at a time, so a
/// name with a misspelt or unimplemented effect key does not merely grant
/// nothing — it *holds the class* until its price is paid, and the whole era's
/// Campus, Harbour and Theatre Square output goes through it. The roster is
/// hand-written JSON keyed by strings that `named_great_person_effect` reads by
/// string, and nothing else in the type system connects the two.
///
/// This is the guard for that: the union of every key in the shipped roster,
/// against the list this test states, which is the list the engine branches on.
/// Adding a person with a new key fails here until the branch exists.
#[test]
fn every_effect_key_in_the_roster_is_read_by_the_engine() {
    // Read off `Game::named_great_person_effect`, `great_person_effect` and
    // `validate_great_person_activation` -- the three places a key is spent.
    const READ_BY_THE_ENGINE: [&str; 34] = [
        "ancient_classical_wonder_multiplier",
        "annex_tile",
        "city_production",
        "destination_foreign_trade_gold",
        "envoys",
        "found_religion",
        "free_library",
        "free_lighthouse",
        "free_quadrireme",
        "free_shipyard",
        "free_trader",
        "free_university",
        "gold",
        "great_work_art",
        "great_work_music",
        "great_work_writing",
        "land_unit_formation",
        "land_unit_promotion_level",
        "libraries_science",
        "military_promotion",
        "modern_atomic_tech_boosts",
        "modern_tech_boosts",
        "naval_flanking_bonus_pct",
        "naval_promotion",
        "naval_ranged_production_pct",
        "naval_unit_formation",
        "oil_per_turn",
        "research_labs_science",
        "strategic_destination_trade_gold",
        "tech_boosts",
        "trade_capacity",
        "universities_science",
        "wonder_production",
        "workshops_culture",
    ];
    let rules = crate::rules::Rules::shipped();
    for (id, spec) in rules.great_people.iter() {
        assert!(
            !spec.effects.is_empty(),
            "{id} would consume a recruitment slot and grant nothing"
        );
        // `ancient_classical_wonder_multiplier` only scales `wonder_production`.
        assert!(
            !spec
                .effects
                .contains_key("ancient_classical_wonder_multiplier")
                || spec.effects.contains_key("wonder_production"),
            "{id} scales a wonder grant it does not make"
        );
        for key in spec.effects.keys() {
            assert!(
                READ_BY_THE_ENGINE.contains(&key.as_str()),
                "{id} carries effect {key:?}, which no engine branch spends"
            );
        }
    }
}

/// Every Writer, Artist and Musician signs the number of works Firaxis ships.
///
/// The three great-work classes are complete against Gathering Storm -- 29, 23
/// and 18 individuals -- and their whole shipped effect is the works they
/// create, taken per individual from the `GreatWorks` table. Two Musicians
/// (Dimitrie Cantemir, Scott Joplin) leave three where the other sixteen leave
/// two, so a class-wide constant would be wrong.
#[test]
fn every_great_work_person_signs_the_works_the_game_ships() {
    let mut game = Game::new_full(1, 24, 16, 95_011, 300, 0, false);
    let mut expected: std::collections::BTreeMap<&str, i64> = std::collections::BTreeMap::new();
    let people: Vec<(String, crate::rules::GreatPersonSpec)> = game
        .rules
        .great_people
        .iter()
        .map(|(id, spec)| (id.to_string(), spec.clone()))
        .collect();
    let mut seen = std::collections::BTreeMap::new();
    for (id, spec) in &people {
        for (effect, kind) in [
            ("great_work_writing", "writing"),
            ("great_work_art", "art"),
            ("great_work_music", "music"),
        ] {
            let Some(count) = spec.effects.get(effect).copied() else {
                continue;
            };
            assert!(count >= 1.0, "{id} creates {count} works of {kind}");
            *expected.entry(kind).or_default() += count as i64;
            *seen.entry(spec.kind.clone()).or_insert(0usize) += 1;
        }
        game.named_great_person_effect(0, spec);
    }
    for (kind, works) in &expected {
        assert_eq!(
            game.players[0]
                .counters
                .get(&format!("great_work:{kind}"))
                .copied()
                .unwrap_or(0),
            *works,
            "the roster's {kind} works did not all reach the player"
        );
        assert_eq!(
            game.players[0]
                .great_work_pieces
                .iter()
                .filter(|piece| piece.kind == *kind)
                .count() as i64,
            *works
        );
    }
    // Sun Tzu is a General who leaves one work of writing -- The Art of War --
    // which is why the sweep is over effects rather than over classes.
    assert_eq!(seen.get("writer").copied(), Some(29));
    assert_eq!(seen.get("artist").copied(), Some(23));
    assert_eq!(seen.get("musician").copied(), Some(18));
    assert_eq!(seen.get("general").copied(), Some(1));
    assert_eq!(
        game.rules.great_people["dimitrie_cantemir"]
            .effects
            .get("great_work_music"),
        Some(&3.0)
    );
    assert_eq!(
        game.rules.great_people["scott_joplin"]
            .effects
            .get("great_work_music"),
        Some(&3.0)
    );
}

/// The Scientists and Engineers added from the shipped roster fire.
#[test]
fn added_scientists_and_engineers_grant_eurekas_and_wonder_production() {
    let (mut game, city, _) = scientist_game(95_012);

    // Aryabhata's three Eurekas, then Alfred Nobel's single Modern/Atomic one.
    skip_to_great_person(&mut game, "aryabhata");
    let before = game.players[0].boosted_techs.clone();
    assert_eq!(recruit_current_scientist(&mut game), "aryabhata");
    assert_eq!(
        game.players[0].boosted_techs.difference(&before).count(),
        3,
        "Aryabhata triggers three Eurekas"
    );

    skip_to_great_person(&mut game, "alfred_nobel");
    let before_nobel = game.players[0].boosted_techs.clone();
    assert_eq!(recruit_current_scientist(&mut game), "alfred_nobel");
    let nobel: Vec<&Name> = game.players[0]
        .boosted_techs
        .difference(&before_nobel)
        .collect();
    assert_eq!(nobel.len(), 1);
    assert!((5..=6).contains(&game.rules.techs[nobel[0]].era));

    // Isidore of Miletus pays 215 per charge into the wonder under
    // construction, twice, and is refused when nothing is being built.
    install_test_district(&mut game, city, "industrial_zone");
    let wonder_site = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| {
            *position != game.cities[&city].pos && game.map.tiles[position].district.is_none()
        })
        .unwrap();
    skip_to_great_person(&mut game, "isidore_of_miletus");
    let engineer_cost = game.gp_cost(0, "engineer");
    game.players[0]
        .gpp
        .insert("engineer".to_string(), engineer_cost);
    assert!(game.claim_great_person(0, "engineer", None).is_err());
    game.cities.get_mut(&city).unwrap().production = 0.0;
    game.cities.get_mut(&city).unwrap().queue = vec![Item::Wonder {
        wonder: crate::name!("pyramids"),
        pos: wonder_site,
    }];
    assert_eq!(recruit_current_engineer(&mut game), "isidore_of_miletus");
    assert_eq!(game.cities[&city].production, 430.0);

    game.cities.get_mut(&city).unwrap().production = 0.0;
    skip_to_great_person(&mut game, "filippo_brunelleschi");
    assert_eq!(recruit_current_engineer(&mut game), "filippo_brunelleschi");
    assert_eq!(game.cities[&city].production, 630.0);
}

/// The Merchants, Generals and Admirals added from the shipped roster fire.
#[test]
fn added_merchants_generals_and_admirals_fire_their_exact_effects() {
    let mut game = Game::new_full(2, 28, 18, 95_013, 300, 0, false);
    let mut cities = Vec::new();
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        cities.push(game.found_city_for(pid, game.units[&settler].pos, None));
    }
    let (home, foreign) = (cities[0], cities[1]);
    install_test_district(&mut game, home, "commercial_hub");
    let harbor = install_test_district(&mut game, home, "harbor");
    game.map.tiles.get_mut(&harbor).unwrap().terrain = crate::name!("coast");
    game.players[0].civics.insert(crate::name!("foreign_trade"));

    // Zhang Qian: +1 Trade Route capacity and +2 Gold on both ends of a
    // foreign route into the activation city.
    game.routes.push(TradeRoute {
        origin: foreign,
        dest: home,
        owner: 1,
        ends: game.turn + 30,
    });
    let origin_gold = game.city_yields(foreign).gold;
    let destination_gold = game.city_yields(home).gold;
    let capacity = game.trade_capacity(0);
    skip_to_great_person(&mut game, "zhang_qian");
    assert_eq!(recruit_current_merchant(&mut game), "zhang_qian");
    assert_eq!(game.trade_capacity(0) - capacity, 1);
    assert!((game.city_yields(foreign).gold - origin_gold - 2.0).abs() < 1e-9);
    assert!((game.city_yields(home).gold - destination_gold - 2.0).abs() < 1e-9);

    // Zhou Daguan: three envoys and no Gold.
    let gold = game.players[0].gold;
    let envoys = game.players[0].envoys_free;
    skip_to_great_person(&mut game, "zhou_daguan");
    assert_eq!(recruit_current_merchant(&mut game), "zhou_daguan");
    assert_eq!(game.players[0].envoys_free - envoys, 3);
    assert_eq!(game.players[0].gold, gold);

    // John Jacob Astor: 500 Gold and two envoys, both at once.
    let gold = game.players[0].gold;
    let envoys = game.players[0].envoys_free;
    skip_to_great_person(&mut game, "john_jacob_astor");
    assert_eq!(recruit_current_merchant(&mut game), "john_jacob_astor");
    assert_eq!(game.players[0].gold - gold, 500.0);
    assert_eq!(game.players[0].envoys_free - envoys, 2);

    // Sun Tzu is a General gated on a writing slot, not on a unit: the Art of
    // War is a Great Work.
    let works = game.players[0]
        .counters
        .get("great_work:writing")
        .copied()
        .unwrap_or(0);
    skip_to_great_person(&mut game, "sun_tzu");
    assert_eq!(
        recruit_current_military_person(&mut game, "general"),
        "sun_tzu"
    );
    assert_eq!(
        game.players[0]
            .counters
            .get("great_work:writing")
            .copied()
            .unwrap_or(0)
            - works,
        1
    );

    // Genghis Khan promotes exactly one land unit, the strongest.
    let position = game.cities[&home].pos;
    let target = game.spawn_unit("swordsman", 0, position);
    let untouched = game.spawn_unit("warrior", 0, position);
    skip_to_great_person(&mut game, "genghis_khan");
    assert_eq!(
        recruit_current_military_person(&mut game, "general"),
        "genghis_khan"
    );
    assert!(game.promotion_pending(target));
    assert_eq!(game.units[&untouched].xp, 0);

    // Amina's single envoy, and Douglas MacArthur's Oil per turn.
    let envoys = game.players[0].envoys_free;
    skip_to_great_person(&mut game, "ana_nzinga");
    assert_eq!(
        recruit_current_military_person(&mut game, "general"),
        "ana_nzinga"
    );
    assert_eq!(game.players[0].envoys_free - envoys, 1);

    // Oil has to be visible before its rate is anything but zero.
    game.players[0].techs.insert(crate::name!("refining"));
    let oil = game.strategic_resource_rate(0, "oil");
    skip_to_great_person(&mut game, "douglas_macarthur");
    assert_eq!(
        recruit_current_military_person(&mut game, "general"),
        "douglas_macarthur"
    );
    assert_eq!(game.strategic_resource_rate(0, "oil") - oil, 1.0);

    // Ferdinand Magellan's 300 Gold and Ching Shih's 500.
    for (id, gold) in [("ferdinand_magellan", 300.0), ("ching_shih", 500.0)] {
        let before = game.players[0].gold;
        skip_to_great_person(&mut game, id);
        assert_eq!(recruit_current_military_person(&mut game, "admiral"), id);
        assert_eq!(game.players[0].gold - before, gold);
    }
}

/// The person each class opens with, pinned.
///
/// ⚠ `current_great_person` orders by era and then **alphabetically by id**,
/// which means a roster addition can take the opening pick. #2377 moved exactly
/// two of the nine: Aryabhata now opens the Scientist queue ahead of Hypatia
/// (all four Classical Scientists start with a letter below hers except Zhang
/// Heng), and Andrei Rublev opens the Artist queue ahead of Donatello. This
/// test exists so the next roster change reports the same thing rather than
/// discovering it in a recorded game.
#[test]
fn the_opening_offer_of_every_class_is_pinned() {
    let game = Game::new_full(1, 24, 16, 95_014, 300, 0, false);
    for (kind, id) in [
        ("admiral", "gaius_duilius"),
        ("artist", "andrey_rublev"),
        ("engineer", "imhotep"),
        ("general", "hannibal_barca"),
        ("merchant", "marcus_licinius_crassus"),
        ("musician", "antonio_vivaldi"),
        ("prophet", "confucius"),
        ("scientist", "aryabhata"),
        ("writer", "bhasa"),
    ] {
        assert_eq!(
            game.current_great_person(kind).map(|(id, _)| id),
            Some(id),
            "the {kind} queue opens somewhere else"
        );
    }
}
