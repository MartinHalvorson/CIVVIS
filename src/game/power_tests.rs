use super::*;

fn power_game() -> (Game, u32) {
    let mut game = Game::new_full(1, 30, 20, 880_144, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.found_city_for(0, game.units[&settler].pos, None);
    game.players[0].techs = game.rules.techs.keys().cloned().collect();
    let city = game.player_city_ids(0)[0];
    for position in game.cities[&city].owned_tiles.clone() {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.resource = None;
        tile.improvement = None;
        tile.pillaged = false;
    }
    (game, city)
}

fn install_power_plant(game: &mut Game, city: u32, plant: &str) {
    let position = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    let tile = game.map.tiles.get_mut(&position).unwrap();
    tile.district = Some(crate::name!("industrial_zone"));
    tile.pillaged = false;
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("industrial_zone"), position);
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(Name::new(plant));
}

#[test]
fn coal_is_burned_in_whole_units_and_powered_yields_stop_when_stock_runs_out() {
    let (mut game, city) = power_game();
    install_power_plant(&mut game, city, "coal_power_plant");
    install_test_district(&mut game, city, "campus");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("research_lab"));
    game.players[0]
        .strategic_resources
        .insert(crate::name!("coal"), 2.0);
    let starting_score = game.players[0].era_score;

    game.process_power(0);
    assert_eq!(game.city_power_demand(&game.cities[&city]), 3.0);
    assert_eq!(game.city_power_supply(&game.cities[&city]), 4.0);
    assert!(game.city_is_powered(&game.cities[&city]));
    assert_eq!(game.strategic_stockpile(0, crate::name!("coal")), 1.0);
    assert_eq!(game.players[0].power_fuel_consumed["coal"], 1.0);
    assert_eq!(game.players[0].co2_emissions, 3_280.0);
    assert_eq!(game.players[0].era_score, starting_score + 3);
    let powered_science = game.city_yields(city).science;

    game.process_power(0);
    assert_eq!(game.strategic_stockpile(0, crate::name!("coal")), 0.0);
    assert_eq!(game.players[0].co2_emissions, 6_560.0);
    game.process_power(0);
    assert_eq!(game.city_power_supply(&game.cities[&city]), 0.0);
    assert!(!game.city_is_powered(&game.cities[&city]));
    let unpowered_science = game.city_yields(city).science;
    let powered_bonus = 5.0 * game.amenity_yield_mult(&game.cities[&city]);
    assert!((powered_science - unpowered_science - powered_bonus).abs() < 1e-9);
}

#[test]
fn renewable_power_prevents_fuel_burn() {
    let (mut game, city) = power_game();
    install_power_plant(&mut game, city, "coal_power_plant");
    install_test_district(&mut game, city, "campus");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("research_lab"));
    let renewable_tiles: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| {
            *position != game.cities[&city].pos && game.map.tiles[position].district.is_none()
        })
        .take(2)
        .collect();
    assert_eq!(renewable_tiles.len(), 2);
    for position in renewable_tiles {
        game.map.tiles.get_mut(&position).unwrap().improvement = Some(crate::name!("solar_farm"));
    }
    game.players[0]
        .strategic_resources
        .insert(crate::name!("coal"), 1.0);

    game.process_power(0);
    assert_eq!(game.city_power_supply(&game.cities[&city]), 4.0);
    assert!(game.city_is_powered(&game.cities[&city]));
    assert_eq!(game.strategic_stockpile(0, crate::name!("coal")), 1.0);
    assert!(game.players[0].power_fuel_consumed.is_empty());
    assert_eq!(game.players[0].co2_emissions, 0.0);
}

#[test]
fn fuel_priority_is_efficiency_then_stockpile_then_technology() {
    let (mut game, target) = power_game();
    let target_pos = game.cities[&target].pos;
    let mut source_positions: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|position| {
            let distance = game.wdist(target_pos, *position);
            (3..=5).contains(&distance)
        })
        .collect();
    source_positions.sort();
    let oil_city = game.found_city_for(0, source_positions[0], Some("Oil Grid".to_string()));
    let coal_city = game.found_city_for(0, source_positions[1], Some("Coal Grid".to_string()));
    for city in game.player_city_ids(0) {
        for position in game.cities[&city].owned_tiles.clone() {
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.resource = None;
            tile.improvement = None;
        }
    }
    install_power_plant(&mut game, target, "nuclear_power_plant");
    install_power_plant(&mut game, oil_city, "oil_power_plant");
    install_power_plant(&mut game, coal_city, "coal_power_plant");
    for district in ["campus", "commercial_hub", "theater_square", "aerodrome"] {
        install_test_district(&mut game, target, district);
    }
    game.cities.get_mut(&target).unwrap().buildings.extend([
        crate::name!("research_lab"),
        crate::name!("stock_exchange"),
        crate::name!("broadcast_center"),
        crate::name!("airport"),
    ]);
    assert_eq!(game.city_power_demand(&game.cities[&target]), 10.0);

    for (resource, amount) in [("uranium", 1.0), ("oil", 10.0), ("coal", 20.0)] {
        game.players[0]
            .strategic_resources
            .insert(Name::new(resource), amount);
    }
    game.process_power(0);
    assert_eq!(game.city_power_supply(&game.cities[&target]), 16.0);
    assert_eq!(game.players[0].power_fuel_consumed["uranium"], 1.0);
    assert_eq!(game.strategic_stockpile(0, crate::name!("oil")), 10.0);
    assert_eq!(game.strategic_stockpile(0, crate::name!("coal")), 20.0);

    game.players[0]
        .strategic_resources
        .insert(crate::name!("uranium"), 0.0);
    game.process_power(0);
    assert_eq!(game.players[0].power_fuel_consumed["coal"], 3.0);
    assert_eq!(game.strategic_stockpile(0, crate::name!("oil")), 10.0);

    game.players[0]
        .strategic_resources
        .insert(crate::name!("coal"), 10.0);
    game.players[0]
        .strategic_resources
        .insert(crate::name!("oil"), 10.0);
    game.process_power(0);
    assert_eq!(game.players[0].power_fuel_consumed["oil"], 3.0);
    assert_eq!(game.strategic_stockpile(0, crate::name!("coal")), 10.0);
}

#[test]
fn carbon_recapture_reduces_lifetime_emissions_below_zero_and_awards_favor() {
    let (mut game, city) = power_game();
    install_power_plant(&mut game, city, "coal_power_plant");
    game.players[0]
        .civics
        .insert(crate::name!("global_warming_mitigation"));
    game.players[0].co2_emissions = 20_000.0;
    game.players[0].diplomatic_favor = 7.0;
    let project = Item::Project {
        project: crate::name!("carbon_recapture"),
    };

    assert!(game.can_produce(0, city, &project));
    assert_eq!(game.item_cost(&project), 400.0);
    assert!(game.complete_item(0, city, &project));
    assert_eq!(game.players[0].co2_emissions, -30_000.0);
    assert_eq!(game.players[0].diplomatic_favor, 37.0);
    assert_eq!(game.players[0].counters["project:carbon_recapture"], 1);

    assert!(game.complete_item(0, city, &project));
    assert_eq!(game.players[0].co2_emissions, -80_000.0);
    assert_eq!(game.players[0].diplomatic_favor, 67.0);
    assert_eq!(game.players[0].counters["project:carbon_recapture"], 2);
}

#[test]
fn unit_fuel_upkeep_emits_half_the_plant_rate_and_advanced_cells_halves_it() {
    let (mut game, city) = power_game();
    game.players[0]
        .techs
        .remove(&Name::new("advanced_power_cells"));
    game.spawn_unit("infantry", 0, game.cities[&city].pos);
    game.players[0]
        .strategic_resources
        .insert(crate::name!("oil"), 1.0);

    game.process_strategic_resources(0);
    assert_eq!(game.strategic_stockpile(0, crate::name!("oil")), 0.0);
    assert_eq!(game.players[0].co2_emissions, 980.0);

    game.players[0]
        .techs
        .insert(crate::name!("advanced_power_cells"));
    game.players[0]
        .strategic_resources
        .insert(crate::name!("oil"), 1.0);
    game.process_strategic_resources(0);
    assert_eq!(game.players[0].co2_emissions, 1_470.0);
}

#[test]
fn excess_lifetime_pollution_reduces_favor_and_is_capped_at_twenty() {
    let (mut game, _) = power_game();
    let rival = game.players.len();
    game.players.push(Player::new(rival, "Rival", false));
    game.players[0].co2_emissions = 60_000.0;
    game.players[rival].co2_emissions = 0.0;
    assert_eq!(game.carbon_favor_penalty(0), 10.0);
    assert_eq!(game.carbon_favor_penalty(rival), 0.0);

    game.players[0].co2_emissions = -50_000.0;
    game.players[rival].co2_emissions = 10_000.0;
    assert_eq!(game.carbon_favor_penalty(rival), 10.0);

    game.players[0].co2_emissions = 200_000.0;
    game.players[rival].co2_emissions = 0.0;
    assert_eq!(game.carbon_favor_penalty(0), 20.0);
}

#[test]
fn climate_phases_are_map_scaled_irreversible_and_save_stable() {
    let (mut game, _) = power_game();
    assert!(game
        .map
        .tiles
        .values()
        .any(|tile| (1..=3).contains(&tile.coastal_lowland)));
    assert!(game
        .map
        .tiles
        .values()
        .all(|tile| tile.coastal_lowland <= 3));

    game.players[0].co2_emissions = 750_000.0;
    assert_eq!(game.co2_per_climate_point(), 250_000.0);
    assert_eq!(game.climate_points(), 3);
    game.process_climate();
    assert_eq!(game.climate_phase, 2);

    game.players[0].co2_emissions = -100_000.0;
    game.process_climate();
    assert_eq!(game.climate_phase, 2, "climate phases never reverse");

    let saved = serde_json::to_string(&game).unwrap();
    let restored: Game = serde_json::from_str(&saved).unwrap();
    assert_eq!(restored.climate_phase, 2);
    assert_eq!(restored.climate_points(), 0);
    assert!(restored
        .map
        .tiles
        .values()
        .any(|tile| tile.coastal_lowland > 0 || tile.submerged));
}

#[test]
fn lowland_bands_flood_then_submerge_while_barriers_restore_and_protect() {
    let (mut game, city) = power_game();
    for tile in game.map.tiles.values_mut() {
        tile.coastal_lowland = 0;
        tile.flooded = false;
        tile.submerged = false;
    }
    let positions: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != game.cities[&city].pos)
        .take(3)
        .collect();
    assert_eq!(positions.len(), 3);
    for (index, position) in positions.iter().copied().enumerate() {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.pillaged = false;
        tile.district = None;
        tile.wonder = None;
        tile.coastal_lowland = index as u8 + 1;
    }
    game.cities.get_mut(&city).unwrap().pop = 10;
    assert!(game
        .valid_improvements(0, positions[0])
        .contains(&crate::name!("farm")));
    assert!(game
        .district_sites(city, crate::name!("campus"))
        .contains(&positions[0]));
    game.map.tiles.get_mut(&positions[0]).unwrap().improvement = Some(crate::name!("farm"));

    game.apply_climate_phase(2);
    assert!(game.map.tiles[&positions[0]].flooded);
    assert!(game.map.tiles[&positions[0]].pillaged);
    assert!(game.valid_improvements(0, positions[0]).is_empty());
    assert!(!game
        .district_sites(city, crate::name!("campus"))
        .contains(&positions[0]));
    assert_eq!(
        game.player_tile_yields(0, positions[0], &game.map.tiles[&positions[0]]),
        Yields::default()
    );
    assert!(!game.map.tiles[&positions[1]].flooded);

    game.climate_phase = 2;
    game.players[0].techs.insert(crate::name!("computers"));
    let barrier = Item::Building {
        building: crate::name!("flood_barrier"),
    };
    assert_eq!(game.item_cost_for_city(0, city, &barrier), 480.0);
    assert!(game.complete_item(0, city, &barrier));
    assert!(!game.map.tiles[&positions[0]].flooded);
    assert!(!game.map.tiles[&positions[0]].pillaged);
    game.map.tiles.get_mut(&positions[0]).unwrap().improvement = None;
    assert!(game
        .valid_improvements(0, positions[0])
        .contains(&crate::name!("farm")));

    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .retain(|building| building != "flood_barrier");
    game.apply_climate_phase(4);
    assert!(game.map.tiles[&positions[0]].submerged);
    assert_eq!(game.map.tiles[&positions[0]].terrain, "coast");
    assert!(game.map.tiles[&positions[0]].improvement.is_none());

    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("flood_barrier"));
    game.apply_climate_phase(6);
    assert!(!game.map.tiles[&positions[1]].submerged);
}
