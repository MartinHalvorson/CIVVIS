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
    assert_eq!(recruit_current_scientist(&mut game), "omar_khayyam");
    assert_eq!(game.players[0].boosted_techs, initial_boosts);
    assert!((game.city_yields(city).science - before_khayyam - 1.0).abs() < 1e-9);

    let before_newton = game.city_yields(city).science;
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
    assert_eq!(recruit_current_scientist(&mut game), "charles_darwin");
    assert!((game.city_yields(city).science - before_darwin - 2.0).abs() < 1e-9);

    let before_einstein = game.city_yields(city).science;
    let boosts_before_einstein = game.players[0].boosted_techs.clone();

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
    assert_eq!(recruit_current_engineer(&mut game), "imhotep");
    assert_eq!(game.cities[&city].production, 700.0);

    game.cities.get_mut(&city).unwrap().queue.clear();
    let culture_before = game.city_yields(city).culture;
    let boosts_before = game.players[0].boosted_techs.clone();
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
    assert_eq!(recruit_current_merchant(&mut game), "jakob_fugger");
    assert_eq!(game.players[0].gold - gold_before_fugger, 240.0);

    let gold_before_smith = game.players[0].gold;
    assert_eq!(recruit_current_merchant(&mut game), "adam_smith");
    assert_eq!(game.players[0].gold - gold_before_smith, 420.0);

    let rockefeller_route_gold = game.city_yields(merchant_city).gold;
    let capacity_before_rockefeller = game.trade_capacity(0);
    let gold_before_rockefeller = game.players[0].gold;
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

    assert_eq!(
        recruit_current_military_person(&mut game, "general"),
        "hannibal_barca"
    );
    assert!(game.promotion_pending(target));
    assert_eq!(game.units[&untouched].xp, 0);

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
    assert_eq!(
        recruit_current_military_person(&mut game, "general"),
        "joan_of_arc"
    );
    assert!(game.promotion_pending(target));
    assert_eq!(game.units[&target].formation, 1);

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

    // A newly exposed person is priced in the world era at reveal time.
    game.world_era = 3;
    let hypatia_cost = game.gp_cost(0, "scientist");
    game.players[0]
        .gpp
        .insert("scientist".to_string(), hypatia_cost);
    game.claim_great_person(0, "scientist", None).unwrap();
    // The Medieval Scientist the roster gained when the era chain was filled
    // in; the market used to skip straight from Hypatia to Newton.
    assert_eq!(
        game.current_great_person("scientist").unwrap().0,
        "omar_khayyam"
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
