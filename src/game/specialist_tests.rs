use super::*;

fn specialist_game() -> (Game, u32, Pos) {
    let mut game = Game::new_full(1, 24, 16, 774_301, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.found_city_for(0, game.units[&settler].pos, None);
    let city = game.player_city_ids(0)[0];
    let district = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    for position in game.cities[&city].owned_tiles.clone() {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("desert");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
    }
    game.cities.get_mut(&city).unwrap().districts.clear();
    (game, city, district)
}

fn install_district(game: &mut Game, city: u32, position: Pos, district: &str) {
    game.map.tiles.get_mut(&position).unwrap().district = Some(Name::new(district));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(Name::new(district), position);
}

#[test]
fn every_specialist_family_has_its_official_base_job_yield() {
    let cases = [
        (
            "campus",
            "library",
            Yields {
                science: 2.0,
                ..Yields::default()
            },
        ),
        (
            "holy_site",
            "shrine",
            Yields {
                faith: 2.0,
                ..Yields::default()
            },
        ),
        (
            "encampment",
            "barracks",
            Yields {
                production: 1.0,
                gold: 2.0,
                ..Yields::default()
            },
        ),
        (
            "harbor",
            "lighthouse",
            Yields {
                food: 1.0,
                gold: 2.0,
                ..Yields::default()
            },
        ),
        (
            "commercial_hub",
            "market",
            Yields {
                gold: 4.0,
                ..Yields::default()
            },
        ),
        (
            "industrial_zone",
            "workshop",
            Yields {
                production: 2.0,
                ..Yields::default()
            },
        ),
        (
            "theater_square",
            "amphitheater",
            Yields {
                culture: 2.0,
                ..Yields::default()
            },
        ),
    ];
    for (district, building, expected) in cases {
        let (mut game, city, position) = specialist_game();
        install_district(&mut game, city, position, district);
        game.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(Name::new(building));
        let jobs = game.city_specialist_jobs(&game.cities[&city]);
        assert_eq!(jobs, vec![(district.to_string(), expected)], "{district}");
    }
}

#[test]
fn campus_slots_are_worked_and_tier_three_yield_applies_to_every_scientist() {
    let (mut game, city, position) = specialist_game();
    install_district(&mut game, city, position, "campus");
    game.cities.get_mut(&city).unwrap().pop = 1;
    let before = game.city_yields(city);
    assert!(game.city_citizen_plan(city).specialists.is_empty());

    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("library"));
    assert_eq!(game.city_citizen_plan(city).specialists, vec!["campus"]);
    let after_library = game.city_yields(city);
    assert!((after_library.science - before.science - 4.0).abs() < 1e-9);

    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("university"));
    game.cities.get_mut(&city).unwrap().pop = 2;
    assert_eq!(
        game.city_citizen_plan(city).specialists,
        vec!["campus", "campus"]
    );
    // Keep population constant across the yield delta so amenity scaling
    // cannot obscure the tier-three building and specialist effects.
    game.cities.get_mut(&city).unwrap().pop = 3;
    assert_eq!(game.city_citizen_plan(city).specialists.len(), 2);
    let before_lab = game.city_yields(city);

    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("research_lab"));
    let jobs = game.city_specialist_jobs(&game.cities[&city]);
    assert_eq!(jobs.len(), 3);
    assert!(jobs.iter().all(|(_, yields)| yields.science == 3.0));
    assert_eq!(game.city_citizen_plan(city).specialists.len(), 3);
    let expected_delta = 8.0 * game.amenity_yield_mult(&game.cities[&city]);
    assert!(
        (game.city_yields(city).science - before_lab.science - expected_delta).abs() < 1e-9
    );

    game.cities
        .get_mut(&city)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("research_lab"));
    let jobs = game.city_specialist_jobs(&game.cities[&city]);
    assert_eq!(jobs.len(), 2);
    assert!(jobs.iter().all(|(_, yields)| yields.science == 2.0));
    assert_eq!(game.city_citizen_plan(city).specialists.len(), 2);

    game.map.tiles.get_mut(&position).unwrap().pillaged = true;
    assert!(game.city_specialist_jobs(&game.cities[&city]).is_empty());
    assert!(game.city_citizen_plan(city).specialists.is_empty());
}

#[test]
fn worship_and_each_power_plant_add_their_per_specialist_bonus() {
    let (mut game, city, position) = specialist_game();
    install_district(&mut game, city, position, "holy_site");
    game.cities.get_mut(&city).unwrap().buildings = vec![
        crate::name!("shrine"),
        crate::name!("temple"),
        crate::name!("pagoda"),
    ];
    let jobs = game.city_specialist_jobs(&game.cities[&city]);
    assert_eq!(jobs.len(), 3);
    assert!(jobs.iter().all(|(_, yields)| yields.faith == 3.0));

    for power_plant in ["coal_power_plant", "oil_power_plant", "nuclear_power_plant"] {
        let (mut game, city, position) = specialist_game();
        install_district(&mut game, city, position, "industrial_zone");
        game.cities.get_mut(&city).unwrap().buildings = vec![
            crate::name!("workshop"),
            crate::name!("factory"),
            Name::new(power_plant),
        ];
        let jobs = game.city_specialist_jobs(&game.cities[&city]);
        assert_eq!(jobs.len(), 3, "{power_plant}");
        assert!(
            jobs.iter().all(|(_, yields)| yields.production == 3.0),
            "{power_plant}"
        );
    }
}

#[test]
fn royal_society_spends_all_builder_charges_and_can_finish_a_project() {
    let (mut game, city, position) = specialist_game();
    install_district(&mut game, city, position, "spaceport");
    install_test_district(&mut game, city, "government_plaza");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("royal_society"));
    let project = Item::Project {
        project: crate::name!("launch_earth_satellite"),
    };
    game.cities.get_mut(&city).unwrap().queue = vec![project.clone()];
    let builder = game.spawn_unit("builder", 0, position);
    game.units.get_mut(&builder).unwrap().charges = 3;

    assert_eq!(game.project_contribution_target(0, city), Some(position));
    assert!(game.legal_actions(0).contains(&Action::ContributeProject {
        unit: builder,
        city
    }));
    game.do_contribute_project(0, builder, city).unwrap();
    assert!(!game.units.contains_key(&builder));
    assert_eq!(game.cities[&city].production, 54.0);
    assert_eq!(game.cities[&city].queue, vec![project.clone()]);

    let second = game.spawn_unit("builder", 0, position);
    assert!(!game.can_contribute_project(0, second, city));
    game.turn += 1;
    game.cities.get_mut(&city).unwrap().production = 890.0;
    game.units.get_mut(&second).unwrap().charges = 1;
    game.do_contribute_project(0, second, city).unwrap();
    assert!(!game.units.contains_key(&second));
    assert!(game.cities[&city].queue.is_empty());
    assert_eq!(game.cities[&city].production, 8.0);
    assert!(game.players[0]
        .science_projects
        .contains("launch_earth_satellite"));
}

#[test]
fn royal_society_uses_an_active_spaceport_when_another_is_pillaged() {
    let (mut game, city, first) = specialist_game();
    let second = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos && *position != first)
        .unwrap();
    install_district(&mut game, city, first, "spaceport");
    install_district(&mut game, city, second, "spaceport");
    install_test_district(&mut game, city, "government_plaza");
    game.map.tiles.get_mut(&first).unwrap().pillaged = true;
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("royal_society"));
    game.cities.get_mut(&city).unwrap().queue = vec![Item::Project {
        project: crate::name!("launch_earth_satellite"),
    }];

    assert_eq!(game.project_contribution_target(0, city), Some(second));
}

#[test]
fn military_engineers_accelerate_engineering_districts_and_can_finish_them() {
    let (mut game, city, position) = specialist_game();
    game.players[0].techs.insert(crate::name!("engineering"));
    let center = game.cities[&city].pos;
    let center_edge = game.map.direction_to(position, center).unwrap();
    let river_edge = (0..6).find(|edge| *edge != center_edge).unwrap();
    game.map.tiles.get_mut(&position).unwrap().river_edges[river_edge] = true;
    let aqueduct = Item::District {
        district: crate::name!("aqueduct"),
        pos: position,
    };
    assert!(game.district_sites(city, crate::name!("aqueduct")).contains(&position));
    game.cities.get_mut(&city).unwrap().queue = vec![aqueduct.clone()];

    let mut england = game.clone();
    england.players[0].civ = "England".to_string();
    let engineer = game.spawn_unit("military_engineer", 0, position);
    let english_engineer = england.spawn_unit("military_engineer", 0, position);
    assert!(game.legal_actions(0).contains(&Action::ContributeDistrict {
        unit: engineer,
        city,
    }));

    let cost = game.item_cost_for_city(0, city, &aqueduct);
    let english_cost = england.item_cost_for_city(0, city, &aqueduct);
    game.do_contribute_district(0, engineer, city).unwrap();
    england
        .do_contribute_district(0, english_engineer, city)
        .unwrap();
    assert!((game.cities[&city].production - cost * 0.2).abs() < 1e-9);
    assert!((england.cities[&city].production - english_cost * 0.4).abs() < 1e-9);
    assert_eq!(game.units[&engineer].charges, 1);
    assert_eq!(game.units[&engineer].moves_left, 0.0);

    game.cities.get_mut(&city).unwrap().production = cost - 1.0;
    let finisher = game.spawn_unit("military_engineer", 0, position);
    game.units.get_mut(&finisher).unwrap().charges = 1;
    game.do_contribute_district(0, finisher, city).unwrap();
    assert!(!game.units.contains_key(&finisher));
    assert!(game.cities[&city].queue.is_empty());
    assert!(game.cities[&city].districts.contains_key(crate::name!("aqueduct")));
    assert!((game.cities[&city].production - (cost * 0.2 - 1.0)).abs() < 1e-9);
}
