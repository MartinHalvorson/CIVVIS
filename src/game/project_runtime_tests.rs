use super::*;

fn project_game() -> (Game, u32, Vec<Pos>) {
    let mut game = Game::new_full(1, 24, 16, 774_255, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.found_city_for(0, game.units[&settler].pos, None);
    let city = game.player_city_ids(0)[0];
    let positions = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != game.cities[&city].pos)
        .collect();
    (game, city, positions)
}

fn install_district(game: &mut Game, city: u32, position: Pos, district: &str) {
    let tile = game.map.tiles.get_mut(&position).unwrap();
    tile.district = Some(Name::new(district));
    tile.pillaged = false;
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(Name::new(district), position);
}

#[test]
fn stock_district_projects_scale_cost_and_completion_points_with_tree_progress() {
    let mut game = Game::new_full(1, 24, 16, 774_251, 120, 0, false);
    let project = Item::Project {
        project: crate::name!("campus_research_grants"),
    };
    assert_eq!(game.item_cost_for(0, &project), 25.0);
    for technology in game.rules.techs.keys().take(5).cloned().collect::<Vec<_>>() {
        game.players[0].techs.insert(technology);
    }
    assert_eq!(game.game_progress_ratio(0), 0.06);
    assert_eq!(game.item_cost_for(0, &project), 46.0);

    game.players[0].techs = game.rules.techs.keys().cloned().collect();
    assert_eq!(game.item_cost_for(0, &project), 375.0);
    assert_eq!(
        game.item_cost_for(
            0,
            &Item::Project {
                project: crate::name!("launch_earth_satellite")
            }
        ),
        900.0,
        "fixed city projects must not inherit district-project scaling"
    );

    let (mut completion, city, positions) = project_game();
    install_district(&mut completion, city, positions[0], "campus");
    let before = completion.players[0]
        .gpp
        .get("scientist")
        .copied()
        .unwrap_or(0.0);
    assert!(completion.complete_item(0, city, &project));
    assert_eq!(completion.players[0].gpp["scientist"] - before, 10.0);
}

#[test]
fn district_projects_convert_live_production_and_stop_when_the_district_is_pillaged() {
    let (mut game, city, positions) = project_game();
    install_district(&mut game, city, positions[0], "campus");
    let project = Item::Project {
        project: crate::name!("campus_research_grants"),
    };
    assert!(game.can_produce(0, city, &project));
    let base = game.city_yields(city);
    game.cities.get_mut(&city).unwrap().queue = vec![project.clone()];
    // The conversion is a city yield (the host lists "+N from Campus
    // Research Grants" in the city's own ledger), so `city_yields` carries
    // it while the project runs, and turn processing pays exactly that.
    let running = game.city_yields(city);
    assert!((running.science - base.science - 0.15 * base.production).abs() < 1e-9,
        "15% of the Production rate as Science: {} -> {}", base.science, running.science);
    let observed = game.process_city(0, city);
    assert!((observed.science - running.science).abs() < 1e-9);

    game.cities.get_mut(&city).unwrap().production = 0.0;
    game.map.tiles.get_mut(&positions[0]).unwrap().pillaged = true;
    assert!(!game.can_produce(0, city, &project));
    let stalled = game.process_city(0, city);
    assert_eq!(game.cities[&city].production, 0.0);
    assert_eq!(stalled.science, game.city_yields(city).science);
}

#[test]
fn city_states_cannot_run_ordinary_district_projects() {
    let (mut game, city, positions) = project_game();
    install_district(&mut game, city, positions[0], "campus");
    install_district(&mut game, city, positions[1], "entertainment_complex");
    game.players[0].is_minor = true;
    let grants = Item::Project {
        project: crate::name!("campus_research_grants"),
    };
    let loyalty = Item::Project {
        project: crate::name!("bread_and_circuses"),
    };

    assert!(!game.can_produce(0, city, &grants));
    assert!(!game.can_produce(0, city, &loyalty));
    game.players[0].is_barbarian = true;
    assert!(!game.can_produce(0, city, &grants));
}

#[test]
fn industrial_zone_logistics_supplies_full_power_without_burning_fuel() {
    let (mut game, city, positions) = project_game();
    install_district(&mut game, city, positions[0], "industrial_zone");
    install_district(&mut game, city, positions[1], "campus");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .extend([crate::name!("factory"), crate::name!("research_lab")]);
    game.cities.get_mut(&city).unwrap().queue = vec![Item::Project {
        project: crate::name!("industrial_zone_logistics"),
    }];
    game.process_power(0);
    let demand = game.city_power_demand(&game.cities[&city]);
    assert!(demand > 0.0);
    assert_eq!(game.city_power_supply(&game.cities[&city]), demand);
    assert!(game.players[0].power_fuel_consumed.is_empty());

    game.map.tiles.get_mut(&positions[0]).unwrap().pillaged = true;
    game.process_power(0);
    assert!(!game.city_is_powered(&game.cities[&city]));
}

#[test]
fn bread_and_circuses_projects_pressure_from_either_entertainment_district() {
    let mut game = Game::new_full(2, 24, 16, 774_259, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let source = game.found_city_for(0, game.units[&settler].pos, None);
    let source_pos = game.cities[&source].pos;
    let target_pos = game
        .wdisk(source_pos, 6)
        .into_iter()
        .filter(|position| game.wdist(source_pos, *position) >= 3)
        .find(|position| game.city_at(*position).is_none())
        .unwrap();
    let target = game.found_city_for(1, target_pos, Some("Pressure Target".to_string()));
    let district = game.cities[&source]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != source_pos)
        .unwrap();
    install_district(&mut game, source, district, "water_park");
    game.cities.get_mut(&source).unwrap().pop = 10;
    game.cities.get_mut(&target).unwrap().pop = 5;
    game.cities.get_mut(&target).unwrap().loyalty = 50.0;
    let project = Item::Project {
        project: crate::name!("bread_and_circuses"),
    };
    assert!(game.can_produce(0, source, &project));

    let mut baseline = game.clone();
    game.cities.get_mut(&source).unwrap().queue = vec![project.clone()];
    assert_eq!(
        game.city_active_project_effect(&game.cities[&source], "citizen_loyalty_pressure"),
        0.5
    );
    baseline.process_loyalty(1);
    game.process_loyalty(1);
    assert!(
        game.cities[&target].loyalty < baseline.cities[&target].loyalty,
        "the active project must exert stronger offensive citizen pressure"
    );

    game.cities.get_mut(&source).unwrap().loyalty = 70.0;
    assert!(game.complete_item(0, source, &project));
    assert_eq!(game.cities[&source].loyalty, 90.0);
    game.map.tiles.get_mut(&district).unwrap().pillaged = true;
    assert!(!game.can_produce(0, source, &project));
    assert_eq!(
        game.city_active_project_effect(&game.cities[&source], "citizen_loyalty_pressure"),
        0.0
    );
}
